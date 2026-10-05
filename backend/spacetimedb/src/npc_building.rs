// ----------------------------------------------------------------------------
// NPC BUILDING TEMPLATES & PROGRAMS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Data-driven building "blueprints" that NPC programs (villages,
// bandit camps, settler AI, quest scripts, admin tooling) can stamp into the world in a
// single call. A template is just a static list of pieces with offsets relative to the
// building origin; `plan_building` turns it into world-space pieces using the SAME
// stability/decay rules as player construction (`get_piece_decay`), and
// `spawn_npc_building` inserts them as finished `structure` rows.
//
// Why a pure planning step? `plan_building` has no database access, so it is unit tested
// directly (tests/npc_building_tests.rs) and is trivially deterministic: it only uses exact
// quarter-turn rotation tables (no trig) and caller-supplied ground heights.
//
// Coordinate conventions (mirroring the client's socket layout in client/src/building.rs):
//   * Foundations are 4m x 1m x 4m cuboids; `y` is the cuboid CENTRE (terrain + 0.5).
//   * Wall-family pieces (Wall / Window / Door) are 4m x 3m x 0.4m, centred 2.0m above their
//     foundation centre, on the foundation's N/E/S/W edge (2.0m from the centre).
//   * `yaw_steps` counts quarter turns about +Y (Bevy `Quat::from_rotation_y`).

use spacetimedb::{reducer, ReducerContext, Table};

use crate::building::{
    get_piece_decay, get_piece_max_health, structure, Structure,
};
use crate::movement::{player_session, transform};
use crate::nav_event;

/// Foundation cuboid half-height: foundation centre sits this far above the terrain.
pub const FOUNDATION_LIFT: f32 = 0.5;
/// Wall-family centre height above the foundation centre.
pub const WALL_DY: f32 = 2.0;
/// Roof centre height above the foundation centre (wall top at +3.5, roof is 0.2 thick).
pub const ROOF_DY: f32 = 3.6;
/// Grid pitch (m) of foundation placement.
pub const GRID: f32 = 4.0;

/// One piece of a template. `dx/dz` are offsets from the building origin in template space
/// (before the building's own yaw is applied); `dy` is relative to the anchoring foundation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemplatePiece {
    pub piece_type: &'static str,
    /// Index of the supporting piece (must be lower than this piece's own index).
    /// `None` is only legal for Foundations, which anchor directly to the terrain.
    pub parent: Option<usize>,
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    /// Quarter turns of the piece itself, before the building yaw is added.
    pub yaw_steps: u8,
}

const fn piece(
    piece_type: &'static str,
    parent: Option<usize>,
    dx: f32,
    dy: f32,
    dz: f32,
    yaw_steps: u8,
) -> TemplatePiece {
    TemplatePiece { piece_type, parent, dx, dy, dz, yaw_steps }
}

/// 4x4m one-room hut: closed N and W walls, a window on the E wall, a door on the S wall.
static HUT: [TemplatePiece; 6] = [
    piece("Foundation", None, 0.0, 0.0, 0.0, 0),
    piece("Wall", Some(0), 0.0, WALL_DY, -2.0, 0),   // North
    piece("Wall", Some(0), -2.0, WALL_DY, 0.0, 1),   // West
    piece("Window", Some(0), 2.0, WALL_DY, 0.0, 3),  // East
    piece("Door", Some(0), 0.0, WALL_DY, 2.0, 2),    // South
    piece("Roof", Some(1), 0.0, ROOF_DY, 0.0, 0),
];

/// 8x4m two-room cottage: windows on the north face of the east room and on the east gable,
/// a door on the south face of the west room. The two rooms share an open interior.
static COTTAGE: [TemplatePiece; 10] = [
    piece("Foundation", None, 0.0, 0.0, 0.0, 0),
    piece("Foundation", None, 4.0, 0.0, 0.0, 0),
    piece("Wall", Some(0), 0.0, WALL_DY, -2.0, 0),    // North, west room
    piece("Window", Some(1), 4.0, WALL_DY, -2.0, 0),  // North, east room
    piece("Wall", Some(0), -2.0, WALL_DY, 0.0, 1),    // West gable
    piece("Window", Some(1), 6.0, WALL_DY, 0.0, 3),   // East gable
    piece("Door", Some(0), 0.0, WALL_DY, 2.0, 2),     // South, west room
    piece("Wall", Some(1), 4.0, WALL_DY, 2.0, 2),     // South, east room
    piece("Roof", Some(2), 0.0, ROOF_DY, 0.0, 0),
    piece("Roof", Some(3), 4.0, ROOF_DY, 0.0, 0),
];

/// 4x4m guard post: windows on N, E and W so sentries can see out, a door on the south.
static GUARD_POST: [TemplatePiece; 6] = [
    piece("Foundation", None, 0.0, 0.0, 0.0, 0),
    piece("Window", Some(0), 0.0, WALL_DY, -2.0, 0),
    piece("Window", Some(0), -2.0, WALL_DY, 0.0, 1),
    piece("Window", Some(0), 2.0, WALL_DY, 0.0, 3),
    piece("Door", Some(0), 0.0, WALL_DY, 2.0, 2),
    piece("Roof", Some(1), 0.0, ROOF_DY, 0.0, 0),
];

/// 4x4m High Elf Sanctum: Pristine mint-green marble walls, pointed window, lapis roof.
static HIGH_ELF_SANCTUM: [TemplatePiece; 6] = [
    piece("HighElf_Foundation", None, 0.0, 0.0, 0.0, 0),
    piece("HighElf_Wall", Some(0), 0.0, WALL_DY, -2.0, 0),
    piece("HighElf_Wall", Some(0), -2.0, WALL_DY, 0.0, 1),
    piece("HighElf_Window", Some(0), 2.0, WALL_DY, 0.0, 3),
    piece("HighElf_Door", Some(0), 0.0, WALL_DY, 2.0, 2),
    piece("HighElf_Roof", Some(1), 0.0, ROOF_DY, 0.0, 0),
];

/// 4x4m Dark Elf Spire: Cavern stone pillars, spiked windows, glowing neon rune veins.
static DARK_ELF_SPIRE: [TemplatePiece; 6] = [
    piece("DarkElf_Foundation", None, 0.0, 0.0, 0.0, 0),
    piece("DarkElf_Wall", Some(0), 0.0, WALL_DY, -2.0, 0),
    piece("DarkElf_Window", Some(0), -2.0, WALL_DY, 0.0, 1),
    piece("DarkElf_Window", Some(0), 2.0, WALL_DY, 0.0, 3),
    piece("DarkElf_Door", Some(0), 0.0, WALL_DY, 2.0, 2),
    piece("DarkElf_Roof", Some(1), 0.0, ROOF_DY, 0.0, 0),
];

/// All registered template names (lower-case console names).
pub fn template_names() -> &'static [&'static str] {
    &["hut", "cottage", "guardpost", "highelf", "sanctum", "darkelf", "spire"]
}

/// Looks up a template by case-insensitive name.
pub fn get_building_template(name: &str) -> Option<&'static [TemplatePiece]> {
    match name.trim().to_ascii_lowercase().as_str() {
        "hut" => Some(&HUT),
        "cottage" => Some(&COTTAGE),
        "guardpost" | "guard_post" | "guard post" => Some(&GUARD_POST),
        "highelf" | "sanctum" | "high_elf" => Some(&HIGH_ELF_SANCTUM),
        "darkelf" | "spire" | "dark_elf" => Some(&DARK_ELF_SPIRE),
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// PURE PLANNING
// ----------------------------------------------------------------------------

/// A template piece resolved into world space.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedPiece {
    pub piece_type: &'static str,
    pub parent: Option<usize>,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Rotation quaternion as (x, y, z, w).
    pub rot: (f32, f32, f32, f32),
    pub stability: u32,
    pub is_grounded: bool,
}

/// Rotates a horizontal template offset by `steps` quarter turns about +Y, matching
/// Bevy's `Quat::from_rotation_y(steps * PI/2)`: (x, z) -> (x cos t + z sin t, -x sin t + z cos t).
/// Implemented with an exact lookup table, so no trigonometry is involved.
pub fn rotate_offset(dx: f32, dz: f32, steps: u8) -> (f32, f32) {
    match steps % 4 {
        0 => (dx, dz),
        1 => (dz, -dx),
        2 => (-dx, -dz),
        _ => (-dz, dx),
    }
}

/// Exact quaternion (x, y, z, w) for `steps` quarter turns about +Y.
pub fn quarter_turn_quat(steps: u8) -> (f32, f32, f32, f32) {
    const S: f32 = std::f32::consts::FRAC_1_SQRT_2;
    match steps % 4 {
        0 => (0.0, 0.0, 0.0, 1.0),
        1 => (0.0, S, 0.0, S),
        2 => (0.0, 1.0, 0.0, 0.0),
        _ => (0.0, S, 0.0, -S),
    }
}

/// Resolves `template` into world-space pieces rooted at (`origin_x`, `origin_z`).
/// `ground_height` supplies the terrain height for each foundation.
///
/// Fails when a template is structurally invalid: unknown piece type, a non-foundation
/// without a parent, a parent that does not precede the child, or a parent whose remaining
/// stability cannot support the child (identical to the player-facing rule).
pub fn plan_building(
    template: &[TemplatePiece],
    origin_x: f32,
    origin_z: f32,
    yaw_steps: u8,
    ground_height: impl Fn(f32, f32) -> f32,
) -> Result<Vec<PlannedPiece>, String> {
    let yaw = yaw_steps % 4;
    let mut planned: Vec<PlannedPiece> = Vec::with_capacity(template.len());
    // Y of the foundation each piece is anchored to, so wall/roof heights follow slopes.
    let mut anchor_y: Vec<f32> = Vec::with_capacity(template.len());

    for (i, tp) in template.iter().enumerate() {
        let decay = get_piece_decay(tp.piece_type)
            .ok_or_else(|| format!("Template piece {} has unknown type '{}'.", i, tp.piece_type))?;

        let (ox, oz) = rotate_offset(tp.dx, tp.dz, yaw);
        let x = origin_x + ox;
        let z = origin_z + oz;
        let rot = quarter_turn_quat((tp.yaw_steps % 4 + yaw) % 4);

        match tp.parent {
            None => {
                if tp.piece_type != "Foundation" {
                    return Err(format!(
                        "Template piece {} ('{}') has no parent but only Foundations may anchor to terrain.",
                        i, tp.piece_type
                    ));
                }
                let y = ground_height(x, z) + FOUNDATION_LIFT;
                anchor_y.push(y);
                planned.push(PlannedPiece {
                    piece_type: tp.piece_type,
                    parent: None,
                    x, y, z, rot,
                    stability: 100,
                    is_grounded: true,
                });
            }
            Some(pi) => {
                if pi >= i {
                    return Err(format!("Template piece {} references parent {} which does not precede it.", i, pi));
                }
                let parent_stability = planned[pi].stability;
                if parent_stability <= decay {
                    return Err(format!(
                        "Template piece {} ('{}') exceeds structural integrity (parent stability {}, decay {}).",
                        i, tp.piece_type, parent_stability, decay
                    ));
                }
                let base_y = anchor_y[pi];
                anchor_y.push(base_y);
                planned.push(PlannedPiece {
                    piece_type: tp.piece_type,
                    parent: Some(pi),
                    x,
                    y: base_y + tp.dy,
                    z,
                    rot,
                    stability: parent_stability - decay,
                    is_grounded: false,
                });
            }
        }
    }

    Ok(planned)
}

/// Snaps a coordinate to the 4m building grid.
pub fn snap_to_grid(v: f32) -> f32 {
    (v / GRID).round() * GRID
}

// ----------------------------------------------------------------------------
// SPAWNING
// ----------------------------------------------------------------------------

/// Stamps a finished building into the world on behalf of an NPC program.
///
/// * `owner_id` - entity id credited as owner (0 == world / unowned NPC settlement).
/// * Pieces are created as completed (non-blueprint) structures with full health.
/// * Fails (without inserting anything) if the site overlaps an existing structure.
///
/// Returns the inserted `structure_id`s in template order, so callers can keep handles to
/// e.g. the door for later `set_door_open_internal` calls.
pub fn spawn_npc_building(
    ctx: &ReducerContext,
    template_name: &str,
    origin_x: f32,
    origin_z: f32,
    yaw_steps: u8,
    owner_id: u64,
) -> Result<Vec<u64>, String> {
    let template = get_building_template(template_name).ok_or_else(|| {
        format!("Unknown building template '{}'. Available: {:?}", template_name, template_names())
    })?;

    let planned = plan_building(template, origin_x, origin_z, yaw_steps, crate::get_terrain_height)?;

    // Site check: no existing structure may overlap any foundation footprint.
    for p in planned.iter().filter(|p| p.parent.is_none()) {
        let obstructed = ctx.db.structure().iter().any(|s| {
            (s.x - p.x).abs() < GRID - 0.1 && (s.z - p.z).abs() < GRID - 0.1 && (s.y - p.y).abs() < 3.0
        });
        if obstructed {
            return Err("Building site is obstructed by an existing structure.".to_string());
        }
    }

    let mut ids: Vec<u64> = Vec::with_capacity(planned.len());
    let (mut min_x, mut min_y, mut min_z) = (f32::MAX, f32::MAX, f32::MAX);
    let (mut max_x, mut max_y, mut max_z) = (f32::MIN, f32::MIN, f32::MIN);

    for p in &planned {
        let max_hp = get_piece_max_health(p.piece_type);
        let row = ctx.db.structure().insert(Structure {
            structure_id: 0,
            parent_id: p.parent.map(|pi| ids[pi]),
            piece_type: p.piece_type.to_string(),
            stability: p.stability,
            is_grounded: p.is_grounded,
            is_blueprint: false,
            construction_progress: 100,
            current_health: max_hp,
            max_health: max_hp,
            x: p.x,
            y: p.y,
            z: p.z,
            rot_x: p.rot.0,
            rot_y: p.rot.1,
            rot_z: p.rot.2,
            rot_w: p.rot.3,
            owner_id,
        });
        ids.push(row.structure_id);

        min_x = min_x.min(p.x); min_y = min_y.min(p.y); min_z = min_z.min(p.z);
        max_x = max_x.max(p.x); max_y = max_y.max(p.y); max_z = max_z.max(p.z);
    }

    // One NavEvent over the whole footprint (instead of one per piece) so navmesh
    // consumers rebuild once for the entire building.
    ctx.db.nav_event().insert(crate::NavEvent {
        id: 0,
        min_x: min_x - 3.0,
        min_y: min_y - 3.0,
        min_z: min_z - 3.0,
        max_x: max_x + 3.0,
        max_y: max_y + 3.0,
        max_z: max_z + 3.0,
    });

    Ok(ids)
}

/// Playtesting reducer (same trust level as the other `admin_*` console reducers):
/// stamps a template into the world. Without explicit coordinates the building is placed on
/// the 4m grid, 8m north (-Z) of the calling player.
#[reducer]
pub fn admin_spawn_building(
    ctx: &ReducerContext,
    template: String,
    yaw_steps: u8,
    x: Option<f32>,
    z: Option<f32>,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let (ox, oz) = match (x, z) {
        (Some(x), Some(z)) => (snap_to_grid(x), snap_to_grid(z)),
        _ => {
            let t = ctx.db.transform().entity_id().find(session.entity_id)
                .ok_or_else(|| "Player transform not found.".to_string())?;
            (snap_to_grid(t.x), snap_to_grid(t.z - 8.0))
        }
    };

    let ids = spawn_npc_building(ctx, &template, ox, oz, yaw_steps, 0)?;
    log::debug!(
        "ADMIN: Player {} spawned '{}' ({} pieces) at ({:.1}, {:.1})",
        session.entity_id, template, ids.len(), ox, oz
    );
    Ok(())
}

/// Stamps a BuildingTemplate into the world, either as finished buildings or as blueprints.
/// Pieces are placed on the 4m grid at origin_x, origin_z.
pub fn spawn_template_structures(
    ctx: &ReducerContext,
    template: &crate::templates::BuildingTemplate,
    origin_x: f32,
    origin_z: f32,
    owner_id: u64,
    as_blueprint: bool,
) -> Result<Vec<u64>, String> {
    let mut ids: Vec<u64> = Vec::with_capacity(template.blocks.len());
    let (mut min_x, mut min_y, mut min_z) = (f32::MAX, f32::MAX, f32::MAX);
    let (mut max_x, mut max_y, mut max_z) = (f32::MIN, f32::MIN, f32::MIN);

    for (offset, piece) in &template.blocks {
        let (x, y_storey, z) = offset.to_world_pos(origin_x, 0.0, origin_z);
        let ground_y = crate::get_terrain_height(x, z);
        let y = ground_y + FOUNDATION_LIFT + y_storey;
        let piece_type = piece.to_piece_name(template.faction);
        let max_hp = get_piece_max_health(piece_type);

        let row = ctx.db.structure().insert(Structure {
            structure_id: 0,
            parent_id: None,
            piece_type: piece_type.to_string(),
            stability: 100,
            is_grounded: offset.1 == 0,
            is_blueprint: as_blueprint,
            construction_progress: if as_blueprint { 0 } else { 100 },
            current_health: if as_blueprint { 1.0 } else { max_hp },
            max_health: max_hp,
            x,
            y,
            z,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            rot_w: 1.0,
            owner_id,
        });
        ids.push(row.structure_id);

        min_x = min_x.min(x); min_y = min_y.min(y); min_z = min_z.min(z);
        max_x = max_x.max(x); max_y = max_y.max(y); max_z = max_z.max(z);
    }

    // Refresh navigation meshes across the entire blueprint footprint
    ctx.db.nav_event().insert(crate::NavEvent {
        id: 0,
        min_x: min_x - 3.0,
        min_y: min_y - 3.0,
        min_z: min_z - 3.0,
        max_x: max_x + 3.0,
        max_y: max_y + 3.0,
        max_z: max_z + 3.0,
    });

    Ok(ids)
}

