// ----------------------------------------------------------------------------
// LETHAL FALL HAZARDS & RUINS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: "Valheim-style" falling trees and collapsing towers WITHOUT running
// rigid-body physics on the server. The handshake is:
//
//   1. SERVER SWEEP  - `spawn_fall_hazard` inserts a public `FallHazard` row describing the
//                      fall as pure math (origin + horizontal direction + length + radius)
//                      and schedules `hazard_impact_tick` for the impact moment.
//   2. CLIENT ANIM   - clients see the row and animate a purely visual entity (no physics
//                      body) along that exact vector for `duration_ms`.
//   3. SERVER KILL   - at the impact moment the scheduled reducer kills every living entity
//                      whose coordinates lie inside the swept volume
//                      (`point_in_fall_sweep`), then deletes the hazard row.
//   4. CLIENT RUIN   - the same reducer inserts the harvestable ruin as a regular
//                      `resource_node` (so it reuses swing_tool harvesting and the client's
//                      node replication). Its facing is published through `node_facing`.
//
// Determinism: the module only uses `ctx.timestamp` and table state - no OS clocks,
// no unseeded randomness, no HashMap iteration (AI_RULES.md section 2.1).

use spacetimedb::{reducer, table, ReducerContext, ScheduleAt, Table};
use std::time::Duration;

use crate::ai::harvestable_corpse;
use crate::combat::{apply_damage, health};
use crate::movement::transform;
use crate::{resource_node, ResourceNode};

/// Fall duration (ms) shared by client animation and server impact timer.
pub const TREE_FALL_DURATION_MS: u32 = 2500;
/// Visual/mathematical height of a standard tree at scale 1.0 (matches the client's
/// `FallingTree` tip offset of 18m).
pub const TREE_HEIGHT: f32 = 18.0;
/// Half-width (m) of the lethal corridor along a falling tree.
pub const TREE_KILL_RADIUS: f32 = 1.6;
/// Vertical tolerance (m) around the hazard origin inside which entities are considered
/// "under" the trunk. Generous so slopes and jumping cannot dodge the sweep.
pub const HAZARD_VERTICAL_REACH: f32 = 6.0;
/// Damage large enough to kill anything regardless of max health / god mode.
const LETHAL_DAMAGE: f32 = 1.0e9;

/// Hazard kind strings shared with the client (`client/src/hazards.rs`).
pub const KIND_FALLING_TREE: &str = "FallingTree";
pub const KIND_COLLAPSING_TOWER: &str = "CollapsingTower";

/// Public hazard description. Contains everything a client needs to animate the fall and
/// everything the impact reducer needs to resolve it.
#[table(accessor = fall_hazard, public)]
#[derive(Clone)]
pub struct FallHazard {
    #[primary_key]
    #[auto_inc]
    pub hazard_id: u64,
    /// `KIND_FALLING_TREE` or `KIND_COLLAPSING_TOWER`.
    pub kind: String,
    /// The resource node this tree used to be (0 for non-tree hazards). Clients derive the
    /// tree species/variant from it so the falling mesh matches the removed one.
    pub source_node_id: u64,
    /// Base of the falling object (world space).
    pub origin_x: f32,
    pub origin_y: f32,
    pub origin_z: f32,
    /// Horizontal unit vector the object falls towards.
    pub dir_x: f32,
    pub dir_z: f32,
    /// Length of the lethal sweep == object height (m).
    pub length: f32,
    /// Half-width of the lethal corridor (m).
    pub radius: f32,
    pub started_at_micros: u64,
    /// Authoritative impact time (server clock). Clients animate from their own first-seen
    /// time over `duration_ms` to stay immune to client/server clock skew.
    pub impact_at_micros: u64,
    pub duration_ms: u32,
    /// Visual scale multiplier (tree node scale).
    pub scale: f32,
    /// Ruin spawned at impact: resource node type, hits to harvest, scale and tool.
    pub ruin_node_type: String,
    pub ruin_health: u32,
    pub ruin_scale: f32,
    pub ruin_required_tool: String,
}

/// One-shot impact timer. Scheduled with `ScheduleAt::Time`, so SpacetimeDB removes the row
/// after the reducer runs.
#[table(accessor = hazard_impact_timer, scheduled(hazard_impact_tick))]
#[derive(Clone)]
pub struct HazardImpactTimer {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub hazard_id: u64,
}

/// Horizontal facing for oriented resource nodes (fallen logs). `resource_node` has no
/// rotation column and we avoid altering that schema, so ruins publish their direction here.
#[table(accessor = node_facing, public)]
#[derive(Clone)]
pub struct NodeFacing {
    #[primary_key]
    pub node_id: u64,
    pub dir_x: f32,
    pub dir_z: f32,
}

/// Plain-Rust description used to create a hazard (not stored directly).
#[derive(Clone, Debug)]
pub struct FallHazardSpec {
    pub kind: &'static str,
    pub source_node_id: u64,
    pub origin: (f32, f32, f32),
    pub dir: (f32, f32),
    pub length: f32,
    pub radius: f32,
    pub duration_ms: u32,
    pub scale: f32,
    pub ruin_node_type: &'static str,
    pub ruin_health: u32,
    pub ruin_scale: f32,
    pub ruin_required_tool: &'static str,
}

// ----------------------------------------------------------------------------
// PURE GEOMETRY (unit tested in tests/hazard_tests.rs)
// ----------------------------------------------------------------------------

/// Normalizes a horizontal direction; falls back to +X for (near) zero vectors so a
/// degenerate input can never produce NaN geometry.
pub fn normalize_dir(dx: f32, dz: f32) -> (f32, f32) {
    let len_sq = dx * dx + dz * dz;
    if len_sq < 1.0e-6 || !len_sq.is_finite() {
        return (1.0, 0.0);
    }
    let inv = 1.0 / len_sq.sqrt();
    (dx * inv, dz * inv)
}

/// True when the point lies inside the swept corridor: between the origin and `length`
/// along `dir`, within `radius` sideways, and within `HAZARD_VERTICAL_REACH` vertically.
/// The point behind the origin (the logger standing at the stump) is always safe.
#[allow(clippy::too_many_arguments)]
pub fn point_in_fall_sweep(
    px: f32, py: f32, pz: f32,
    ox: f32, oy: f32, oz: f32,
    dir_x: f32, dir_z: f32,
    length: f32, radius: f32,
) -> bool {
    let rx = px - ox;
    let rz = pz - oz;

    let along = rx * dir_x + rz * dir_z;
    if along < 0.0 || along > length {
        return false;
    }

    // 2D cross product == signed sideways distance from the fall axis.
    let sideways = (rz * dir_x - rx * dir_z).abs();
    if sideways > radius {
        return false;
    }

    (py - oy).abs() <= HAZARD_VERTICAL_REACH
}

/// Where the harvestable ruin lands: a fraction of the way along the fall axis, so it is
/// reachable from the stump instead of at the far tip of an 18m tree.
pub fn ruin_position(origin_x: f32, origin_z: f32, dir_x: f32, dir_z: f32, length: f32) -> (f32, f32) {
    let offset = length * 0.3;
    (origin_x + dir_x * offset, origin_z + dir_z * offset)
}

// ----------------------------------------------------------------------------
// HAZARD CREATION
// ----------------------------------------------------------------------------

/// Inserts the hazard row and schedules its impact. Returns the new `hazard_id`.
pub fn spawn_fall_hazard(ctx: &ReducerContext, spec: &FallHazardSpec) -> u64 {
    let now_micros = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    let duration = Duration::from_millis(spec.duration_ms as u64);
    let (dir_x, dir_z) = normalize_dir(spec.dir.0, spec.dir.1);

    let hazard = ctx.db.fall_hazard().insert(FallHazard {
        hazard_id: 0,
        kind: spec.kind.to_string(),
        source_node_id: spec.source_node_id,
        origin_x: spec.origin.0,
        origin_y: spec.origin.1,
        origin_z: spec.origin.2,
        dir_x,
        dir_z,
        length: spec.length,
        radius: spec.radius,
        started_at_micros: now_micros,
        impact_at_micros: now_micros + duration.as_micros() as u64,
        duration_ms: spec.duration_ms,
        scale: spec.scale,
        ruin_node_type: spec.ruin_node_type.to_string(),
        ruin_health: spec.ruin_health,
        ruin_scale: spec.ruin_scale,
        ruin_required_tool: spec.ruin_required_tool.to_string(),
    });

    ctx.db.hazard_impact_timer().insert(HazardImpactTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Time(ctx.timestamp + duration),
        hazard_id: hazard.hazard_id,
    });

    hazard.hazard_id
}

/// Fells a tree resource node: the caller has already removed the node row. The tree topples
/// away from `from_x/from_z` (the logger), so the logger is behind the stump and safe while
/// anyone standing on the far side of the tree is crushed.
pub fn fell_tree(ctx: &ReducerContext, node: &ResourceNode, from_x: f32, from_z: f32) -> u64 {
    let (dir_x, dir_z) = normalize_dir(node.x - from_x, node.z - from_z);
    spawn_fall_hazard(ctx, &FallHazardSpec {
        kind: KIND_FALLING_TREE,
        source_node_id: node.node_id,
        origin: (node.x, node.y, node.z),
        dir: (dir_x, dir_z),
        length: TREE_HEIGHT * node.scale,
        radius: TREE_KILL_RADIUS,
        duration_ms: TREE_FALL_DURATION_MS,
        scale: node.scale,
        ruin_node_type: "FallenLog",
        ruin_health: 3,
        ruin_scale: node.scale,
        ruin_required_tool: "None",
    })
}

/// Topples a tower of `height` metres from (x, y, z) towards `dir`. Leaves a Rubble pile that
/// can be mined for stone. Intended for future tower pieces / siege collapse programs.
pub fn collapse_tower(ctx: &ReducerContext, x: f32, y: f32, z: f32, dir_x: f32, dir_z: f32, height: f32) -> u64 {
    spawn_fall_hazard(ctx, &FallHazardSpec {
        kind: KIND_COLLAPSING_TOWER,
        source_node_id: 0,
        origin: (x, y, z),
        dir: (dir_x, dir_z),
        length: height,
        radius: 2.5,
        duration_ms: TREE_FALL_DURATION_MS,
        scale: 1.0,
        ruin_node_type: "Rubble",
        ruin_health: 6,
        ruin_scale: 1.0,
        ruin_required_tool: "Pickaxe",
    })
}

// ----------------------------------------------------------------------------
// IMPACT RESOLUTION
// ----------------------------------------------------------------------------

/// Scheduled reducer: kills everything in the swept corridor and leaves the ruin behind.
#[reducer]
pub fn hazard_impact_tick(ctx: &ReducerContext, timer: HazardImpactTimer) -> Result<(), String> {
    // Scheduled reducers are callable by any client unless guarded; only the scheduler
    // (the module's own identity) may resolve an impact.
    if ctx.sender() != ctx.database_identity() {
        return Err("hazard_impact_tick may only be invoked by the scheduler.".to_string());
    }

    let Some(hazard) = ctx.db.fall_hazard().hazard_id().find(timer.hazard_id) else {
        return Ok(());
    };

    resolve_fall_impact(ctx, &hazard);
    ctx.db.fall_hazard().hazard_id().delete(hazard.hazard_id);
    Ok(())
}

fn resolve_fall_impact(ctx: &ReducerContext, h: &FallHazard) {
    // 1. Kill sweep. Collect first, mutate after: apply_damage edits the same tables
    //    we are iterating. Iteration order is the transform table's primary-key order,
    //    which is deterministic.
    let victims: Vec<u64> = ctx.db.transform().iter()
        .filter(|t| ctx.db.harvestable_corpse().entity_id().find(t.entity_id).is_none())
        .filter(|t| ctx.db.health().entity_id().find(t.entity_id).is_some())
        .filter(|t| point_in_fall_sweep(
            t.x, t.y, t.z,
            h.origin_x, h.origin_y, h.origin_z,
            h.dir_x, h.dir_z,
            h.length, h.radius,
        ))
        .map(|t| t.entity_id)
        .collect();

    for entity_id in victims {
        log::debug!("Hazard {} ({}) crushed entity {}", h.hazard_id, h.kind, entity_id);
        apply_damage(ctx, entity_id, LETHAL_DAMAGE);
    }

    // 2. Ruin. Spawned as an ordinary resource node so it is harvested by swing_tool and
    //    replicated by the client's existing node sync.
    let (rx, rz) = ruin_position(h.origin_x, h.origin_z, h.dir_x, h.dir_z, h.length);
    let ry = crate::get_terrain_height(rx, rz);

    let node = ctx.db.resource_node().insert(ResourceNode {
        node_id: 0,
        node_type: h.ruin_node_type.clone(),
        x: rx,
        y: ry,
        z: rz,
        chunk_x: (rx / 50.0).floor() as i32,
        chunk_z: (rz / 50.0).floor() as i32,
        health: h.ruin_health,
        scale: h.ruin_scale,
        required_tool: h.ruin_required_tool.clone(),
    });

    ctx.db.node_facing().insert(NodeFacing {
        node_id: node.node_id,
        dir_x: h.dir_x,
        dir_z: h.dir_z,
    });
}
