// ----------------------------------------------------------------------------
// BUILDING CORE STRUCTURES & IMPORTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Manages modular structural assembly, anchor stability, and
// destruction cascades. Starter pieces (Foundations, Workbenches, and Campfires)
// anchor directly to terrain or voxels without requiring a pre-existing Workbench,
// while advanced pieces (Walls, Floors, Roofs, Ramps) enforce active workbench proximity.

use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::{player_session, transform};
use crate::inventory;
use crate::CombatEvent;
use crate::combat_event;
use crate::nav_event;
use crate::player_perspective;
use crate::waypoint;
use crate::CameraModeType;
use crate::voxel;

#[derive(SpacetimeType, Clone, Debug)]
pub struct SocketDef {
    pub name: String,
    pub offset_x: f32,
    pub offset_y: f32,
    pub offset_z: f32,
}

#[table(accessor = material_properties, public)]
#[derive(Clone, Debug)]
pub struct MaterialProperties {
    #[primary_key]
    pub material_name: String,
    pub max_support: f32,
    pub min_support: f32,
    pub horizontal_loss_coeff: f32,
    pub vertical_loss_coeff: f32,
}

#[table(accessor = structure_edge, public)]
#[derive(Clone, Debug)]
pub struct StructureEdge {
    #[primary_key] #[auto_inc]
    pub edge_id: u64,
    #[index(btree)]
    pub parent_id: u64,
    #[index(btree)]
    pub child_id: u64,
    pub distance_h: f32,
    pub distance_v: f32,
}

#[table(accessor = structure, public)]
#[derive(Clone)]
pub struct Structure {
    #[primary_key] #[auto_inc]
    pub structure_id: u64,
    #[index(btree)]
    pub parent_id: Option<u64>,
    pub piece_type: String,
    pub stability: u32,
    pub current_support: f32,
    pub is_grounded: bool,

    pub is_blueprint: bool,
    pub construction_progress: u32,

    pub current_health: f32,
    pub max_health: f32,

    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub rot_w: f32,
    #[index(btree)]
    pub owner_id: u64,
}

/// Architectural Note: Per-door open/closed state.
/// Kept in its own table (instead of a new `Structure` column) so the existing
/// `structure` schema, its generated bindings and every `Structure { .. }` literal stay
/// untouched. A missing row means "closed", so rows are created lazily on the first toggle
/// and removed again when the door is destroyed (see `destroy_structure_internal`).
/// Clients subscribe to this table and animate the door leaf towards `is_open`; the
/// server never simulates a hinge, which keeps doors jitter-free and desync-free.
#[table(accessor = door_state, public)]
#[derive(Clone)]
pub struct DoorState {
    #[primary_key]
    pub structure_id: u64,
    pub is_open: bool,
}

/// Maximum distance (m) between a player and a door for `toggle_door` to be accepted.
/// Slightly larger than the client's 3m prompt radius to tolerate network latency.
pub const DOOR_INTERACT_RANGE: f32 = 5.0;

// ----------------------------------------------------------------------------
// BUILDING LOGIC & REDUCERS
// ----------------------------------------------------------------------------

/// Strips faction prefix (e.g. "HighElf_", "Human_", "DarkElf_") to get canonical piece name.
pub fn base_piece_type(piece_type: &str) -> &str {
    if let Some((_, base)) = piece_type.split_once('_') {
        base
    } else if let Some((_, base)) = piece_type.split_once(':') {
        base
    } else {
        piece_type
    }
}

pub fn get_piece_max_health(piece_type: &str) -> f32 {
    match base_piece_type(piece_type) {
        "Foundation" => 400.0,
        "Wall" => 200.0,
        "Window" => 150.0,
        "Door" => 120.0,
        "Floor" => 150.0,
        "Roof" => 150.0,
        "Ramp" => 250.0,
        "Workbench" => 150.0,
        "Campfire" => 60.0,
        _ => 100.0,
    }
}

/// Stability lost when a piece of this type is attached to its parent, or `None` for
/// unknown piece types. Single source of truth for `place_structure` and for the NPC
/// building templates, so scripted buildings obey exactly the same structural rules as
/// player-placed ones.
pub fn get_piece_decay(piece_type: &str) -> Option<u32> {
    match base_piece_type(piece_type) {
        "Workbench" | "Campfire" | "Bed" | "Foundation" => Some(0),
        "Wall" | "Window" | "Door" => Some(20),
        "Floor" => Some(25),
        "Roof" => Some(30),
        "Ramp" => Some(25),
        _ => None,
    }
}

/// Wood cost (total, paid over 4 hammer swings) and stone cost for a piece type.
/// Every value must stay divisible by 4 so each swing contributes exactly 25%.
pub fn get_piece_cost(piece_type: &str) -> (u32, u32) {
    match base_piece_type(piece_type) {
        "Workbench" => (8, 0),
        "Campfire" => (4, 4),
        "Foundation" => (20, 0),
        "Wall" => (8, 0),
        "Window" => (8, 0),
        "Door" => (12, 0),
        "Floor" => (12, 0),
        "Roof" => (12, 0),
        "Ramp" => (16, 0),
        _ => (4, 0),
    }
}

/// Material classification for structural calculations.
pub fn get_piece_material(piece_type: &str) -> &'static str {
    match base_piece_type(piece_type) {
        "Foundation" => "Stone",
        _ => "Wood",
    }
}

/// Looks up material properties from DB or supplies authoritative physics presets:
/// - Wood: max_support = 100.0, min_support = 10.0, horizontal_loss_coeff = 22.5, vertical_loss_coeff = 4.5
/// - Stone: max_support = 150.0, min_support = 15.0, horizontal_loss_coeff = 35.0, vertical_loss_coeff = 3.0
pub fn get_material_properties_or_default(ctx: &ReducerContext, material_name: &str) -> (f32, f32, f32, f32) {
    if let Some(mat) = ctx.db.material_properties().material_name().find(&material_name.to_string()) {
        (mat.max_support, mat.min_support, mat.horizontal_loss_coeff, mat.vertical_loss_coeff)
    } else {
        match material_name {
            "Stone" => (150.0, 15.0, 35.0, 3.0),
            _ => (100.0, 10.0, 22.5, 4.5), // Wood
        }
    }
}

pub fn seed_default_materials(ctx: &ReducerContext) {
    if ctx.db.material_properties().material_name().find(&"Wood".to_string()).is_none() {
        ctx.db.material_properties().insert(MaterialProperties {
            material_name: "Wood".to_string(),
            max_support: 100.0,
            min_support: 10.0,
            horizontal_loss_coeff: 22.5,
            vertical_loss_coeff: 4.5,
        });
    }
    if ctx.db.material_properties().material_name().find(&"Stone".to_string()).is_none() {
        ctx.db.material_properties().insert(MaterialProperties {
            material_name: "Stone".to_string(),
            max_support: 150.0,
            min_support: 15.0,
            horizontal_loss_coeff: 35.0,
            vertical_loss_coeff: 3.0,
        });
    }
}

/// Computes support inherited by a child piece from a supporting parent piece using the DAG formula:
/// S_current = min(S_parent, max_support) - (d_h * c_h + d_v * c_v)
pub fn calculate_inherited_support(
    ctx: &ReducerContext,
    parent: &Structure,
    child_piece_type: &str,
    child_x: f32,
    child_y: f32,
    child_z: f32,
) -> (f32, f32, f32, bool) {
    let material = get_piece_material(child_piece_type);
    let (max_support, min_support, c_h, c_v) = get_material_properties_or_default(ctx, material);

    // If parent is a Foundation, child rests directly on the foundation's 4m x 4m footprint
    let (d_h, d_v) = if base_piece_type(&parent.piece_type) == "Foundation" {
        let raw_dh = ((child_x - parent.x).powi(2) + (child_z - parent.z).powi(2)).sqrt();
        let eff_dh = (raw_dh - 2.0).max(0.0);
        let eff_dv = ((child_y - parent.y).abs() - 2.0).max(0.0);
        (eff_dh, eff_dv)
    } else {
        let dh = ((child_x - parent.x).powi(2) + (child_z - parent.z).powi(2)).sqrt();
        let dv = (child_y - parent.y).abs();
        (dh, dv)
    };

    let base_support = parent.current_support.min(max_support);
    let degradation = d_h * c_h + d_v * c_v;
    let support = base_support - degradation;
    let is_viable = support >= min_support;

    (support, d_h, d_v, is_viable)
}

/// Whether a structure of this type still stops projectiles / hitscan rays.
/// An open door leaves a gap in the wall, so shots (and NPC repulsion) pass through it.
/// Windows deliberately DO block projectiles: they only let *sight* through (the client
/// excludes the `Glass` collision layer from vision rays).
pub fn blocks_projectiles(piece_type: &str, door_open: bool) -> bool {
    !(base_piece_type(piece_type) == "Door" && door_open)
}

/// Looks up the open state of a door. Missing row == closed.
pub fn is_door_open(ctx: &ReducerContext, structure_id: u64) -> bool {
    ctx.db.door_state().structure_id().find(structure_id).map_or(false, |d| d.is_open)
}

/// Convenience wrapper combining `blocks_projectiles` with the DB lookup. Only doors incur
/// a table read, so the high-frequency physics rebuild stays cheap.
pub fn structure_blocks_projectiles(ctx: &ReducerContext, s: &Structure) -> bool {
    if base_piece_type(&s.piece_type) != "Door" {
        return true;
    }
    blocks_projectiles(&s.piece_type, is_door_open(ctx, s.structure_id))
}

/// Authoritative door state change usable by reducers AND by NPC programs (e.g. a peasant
/// opening the door of its cottage). Emits a `NavEvent` so navigation meshes refresh
/// around the doorway.
pub fn set_door_open_internal(ctx: &ReducerContext, structure_id: u64, open: bool) -> Result<(), String> {
    let structure = ctx.db.structure().structure_id().find(structure_id)
        .ok_or_else(|| "Structure not found.".to_string())?;

    if base_piece_type(&structure.piece_type) != "Door" {
        return Err("Structure is not a door.".to_string());
    }
    if structure.is_blueprint {
        return Err("Door is still a blueprint.".to_string());
    }

    match ctx.db.door_state().structure_id().find(structure_id) {
        Some(mut state) => {
            if state.is_open == open {
                return Ok(());
            }
            state.is_open = open;
            ctx.db.door_state().structure_id().update(state);
        }
        None => {
            ctx.db.door_state().insert(DoorState { structure_id, is_open: open });
        }
    }

    ctx.db.nav_event().insert(crate::NavEvent {
        id: 0,
        min_x: structure.x - 3.0,
        min_y: structure.y - 3.0,
        min_z: structure.z - 3.0,
        max_x: structure.x + 3.0,
        max_y: structure.y + 3.0,
        max_z: structure.z + 3.0,
    });
    Ok(())
}

/// Player-facing door toggle. Range-checked against the server-side transform so a
/// modified client cannot open doors across the map.
#[reducer]
pub fn toggle_door(ctx: &ReducerContext, structure_id: u64) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let structure = ctx.db.structure().structure_id().find(structure_id)
        .ok_or_else(|| "Structure not found.".to_string())?;

    let player_t = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or_else(|| "Player transform not found.".to_string())?;

    let dist_sq = (player_t.x - structure.x).powi(2)
        + (player_t.y - structure.y).powi(2)
        + (player_t.z - structure.z).powi(2);
    if dist_sq > DOOR_INTERACT_RANGE * DOOR_INTERACT_RANGE {
        return Err("Too far from the door.".to_string());
    }

    let currently_open = is_door_open(ctx, structure_id);
    set_door_open_internal(ctx, structure_id, !currently_open)
}

pub fn is_covered(ctx: &ReducerContext, x: f32, y: f32, z: f32) -> bool {
    for s in ctx.db.structure().iter() {
        if base_piece_type(&s.piece_type) == "Roof" && !s.is_blueprint {
            let dist_sq = (s.x - x).powi(2) + (s.z - z).powi(2);
            if dist_sq <= 9.0 && s.y > y && (s.y - y) < 10.0 {
                return true;
            }
        }
    }
    false
}

#[reducer]
pub fn place_structure(
    ctx: &ReducerContext,
    parent_id: Option<u64>,
    piece_type: String,
    x: f32,
    y: f32,
    z: f32,
    rot_x: f32,
    rot_y: f32,
    rot_z: f32,
    rot_w: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let perspective = ctx.db.player_perspective().entity_id().find(session.entity_id)
        .ok_or_else(|| "Perspective not found.".to_string())?;

    let is_fps_builder = if perspective.camera_mode == CameraModeType::Fps {
        if let Some(inv) = ctx.db.inventory().entity_id().find(session.entity_id) {
            inv.slots.iter().any(|s| s.item_type == "Hammer" && s.count > 0)
        } else {
            false
        }
    } else {
        true
    };

    if !is_fps_builder && perspective.camera_mode != CameraModeType::Rts {
        return Err("Building blueprints requires RTS Mode or an equipped Hammer.".to_string());
    }

    // Architectural Note: Starter Piece Exemption.
    // Foundations, Workbenches, and Campfires can be placed freely on terrain/voxels
    // to establish a camp. Advanced superstructures (Walls, Floors, Roofs, Ramps)
    // strictly require being within 20m of a constructed, active Workbench.
    let is_starter_piece = matches!(base_piece_type(piece_type.as_str()), "Foundation" | "Workbench" | "Campfire");
    if !is_starter_piece {
        let mut near_workbench = false;
        for s in ctx.db.structure().iter().filter(|s| base_piece_type(&s.piece_type) == "Workbench" && !s.is_blueprint) {
            if (s.x - x).powi(2) + (s.z - z).powi(2) <= 400.0 {
                near_workbench = true;
                break;
            }
        }
        if !near_workbench {
            return Err("Construction of walls, floors, roofs, and ramps requires a nearby active Workbench.".to_string());
        }
    }

    let (support, d_h, d_v, stability, is_grounded, chosen_parent_id);

    if matches!(base_piece_type(piece_type.as_str()), "Foundation" | "Ramp" | "Workbench" | "Campfire") && parent_id.is_none() {
        let ground_y = crate::get_terrain_height(x, z);
        let voxel_mat = voxel::get_voxel_at(ctx, x, y - 0.5, z);

        if (y - ground_y).abs() < 4.0 || voxel_mat.is_solid() {
            is_grounded = true;
            let material = get_piece_material(&piece_type);
            let (max_sup, _, _, _) = get_material_properties_or_default(ctx, material);
            support = max_sup;
            stability = 100;
            chosen_parent_id = None;
            d_h = 0.0;
            d_v = 0.0;
        } else {
            return Err("Foundations, Workbenches, and Campfires must anchor to terrain or solid voxels.".to_string());
        }
    } else if let Some(pid) = parent_id {
        let parent = ctx.db.structure().structure_id().find(pid)
            .ok_or_else(|| "Parent structure not found in database.".to_string())?;

        let (init_sup, init_dh, init_dv, _) = calculate_inherited_support(ctx, &parent, &piece_type, x, y, z);
        let mut best_sup = init_sup;
        let mut best_dh = init_dh;
        let mut best_dv = init_dv;
        let mut best_pid = pid;

        // Multi-ground path evaluation: adopt maximum support path from any candidate supporting parent
        for other in ctx.db.structure().iter() {
            if other.structure_id != pid && !other.is_blueprint {
                let dist_sq = (other.x - x).powi(2) + (other.z - z).powi(2);
                if dist_sq <= 25.0 && (other.y - y).abs() <= 4.0 {
                    let (cand_sup, cand_dh, cand_dv, cand_viable) = calculate_inherited_support(ctx, &other, &piece_type, x, y, z);
                    if cand_viable && cand_sup > best_sup {
                        best_sup = cand_sup;
                        best_dh = cand_dh;
                        best_dv = cand_dv;
                        best_pid = other.structure_id;
                    }
                }
            }
        }

        let material = get_piece_material(&piece_type);
        let (max_sup, min_sup, _, _) = get_material_properties_or_default(ctx, material);

        if best_sup < min_sup {
            return Err(format!("Structural integrity depleted ({:.1} < {:.1}). Cannot support additional mass.", best_sup, min_sup));
        }

        support = best_sup;
        stability = (best_sup / max_sup * 100.0).clamp(1.0, 100.0) as u32;
        is_grounded = false;
        chosen_parent_id = Some(best_pid);
        d_h = best_dh;
        d_v = best_dv;
    } else {
        return Err("Piece must anchor to terrain/voxels or snap to a valid parent structure.".to_string());
    }

    let max_hp = get_piece_max_health(&piece_type);

    let new_row = ctx.db.structure().insert(Structure {
        structure_id: 0,
        parent_id: chosen_parent_id,
        piece_type: piece_type.clone(),
        stability,
        current_support: support,
        is_grounded,
        x,
        y,
        z,
        rot_x,
        rot_y,
        rot_z,
        rot_w,
        owner_id: session.entity_id,
        is_blueprint: true,
        construction_progress: 0,
        current_health: 1.0,
        max_health: max_hp,
    });

    if let Some(pid) = chosen_parent_id {
        ctx.db.structure_edge().insert(StructureEdge {
            edge_id: 0,
            parent_id: pid,
            child_id: new_row.structure_id,
            distance_h: d_h,
            distance_v: d_v,
        });
    }

    ctx.db.waypoint().insert(crate::Waypoint {
        waypoint_id: 0,
        commander_id: session.entity_id,
        x,
        y: y + 2.0,
        z,
        order_type: "Build".to_string(),
        expires_at: ctx.timestamp.to_micros_since_unix_epoch() as u64 + 120_000_000,
    });

    Ok(())
}

#[reducer]
pub fn contribute_construction(ctx: &ReducerContext, structure_id: u64) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let mut structure = ctx.db.structure().structure_id().find(structure_id)
        .ok_or_else(|| "Structure not found.".to_string())?;

    if !structure.is_blueprint {
        return Err("Structure is already fully constructed.".to_string());
    }

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Player inventory not found.".to_string())?;

    let has_hammer = inv.slots.iter().any(|s| s.item_type == "Hammer" && s.count > 0);
    if !has_hammer {
        return Err("You must equip a Hammer to contribute materials.".to_string());
    }

    // Architectural Note: Evenly divisible construction costs by 4 swings (25% per hammer hit).
    // The table lives in `get_piece_cost` (Window = 8 wood, Door = 12 wood, etc.).
    let (wood_cost, stone_cost) = get_piece_cost(structure.piece_type.as_str());

    let wood_swing = wood_cost / 4;
    let stone_swing = stone_cost / 4;

    if wood_swing > 0 && !crate::has_item(&inv, "Wood", wood_swing) {
        return Err(format!("Insufficient Wood. Need {} per hammer swing.", wood_swing));
    }
    if stone_swing > 0 && !crate::has_item(&inv, "Stone", stone_swing) {
        return Err(format!("Insufficient Stone. Need {} per hammer swing.", stone_swing));
    }

    if wood_swing > 0 {
        crate::remove_item(&mut inv, "Wood", wood_swing);
    }
    if stone_swing > 0 {
        crate::remove_item(&mut inv, "Stone", stone_swing);
    }

    structure.construction_progress += 25;
    structure.current_health = (structure.max_health * (structure.construction_progress as f32 / 100.0)).max(1.0);

    if structure.construction_progress >= 100 {
        structure.is_blueprint = false;
        structure.construction_progress = 100;
        structure.current_health = structure.max_health;

        ctx.db.nav_event().insert(crate::NavEvent {
            id: 0,
            min_x: structure.x - 3.0,
            min_y: structure.y - 3.0,
            min_z: structure.z - 3.0,
            max_x: structure.x + 3.0,
            max_y: structure.y + 3.0,
            max_z: structure.z + 3.0,
        });
    }

    ctx.db.structure().structure_id().update(structure);
    ctx.db.inventory().entity_id().update(inv);
    Ok(())
}

#[reducer]
pub fn repair_structure(ctx: &ReducerContext, structure_id: u64) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    let has_hammer = inv.slots.iter().any(|s| s.item_type == "Hammer" && s.count > 0);
    if !has_hammer {
        return Err("You must equip a Hammer to repair structures.".to_string());
    }

    let mut structure = ctx.db.structure().structure_id().find(structure_id)
        .ok_or_else(|| "Structure not found.".to_string())?;

    if structure.is_blueprint {
        return Err("Cannot repair an incomplete blueprint.".to_string());
    }

    if structure.current_health >= structure.max_health {
        return Err("Structure is already at full health.".to_string());
    }

    structure.current_health = structure.max_health;
    ctx.db.structure().structure_id().update(structure.clone());

    ctx.db.combat_event().insert(CombatEvent {
        id: 0,
        event_type: "RepairStructure".to_string(),
        x: structure.x,
        y: structure.y + 1.0,
        z: structure.z,
    });

    log::debug!("Player {} repaired structure {}", session.entity_id, structure_id);
    Ok(())
}

pub fn damage_structure(ctx: &ReducerContext, structure_id: u64, amount: f32) {
    if let Some(mut structure) = ctx.db.structure().structure_id().find(structure_id) {
        structure.current_health = (structure.current_health - amount).max(0.0);
        if structure.current_health <= 0.0 {
            let _ = destroy_structure_internal(ctx, structure_id);
        } else {
            ctx.db.structure().structure_id().update(structure);
        }
    }
}

pub fn invalidate_structures_at(ctx: &ReducerContext, vx: f32, vy: f32, vz: f32) {
    let target_ids: Vec<u64> = ctx.db.structure().iter()
        .filter(|s| s.is_grounded && (s.x - vx).abs() <= 2.5 && (s.y - vy).abs() <= 2.5 && (s.z - vz).abs() <= 2.5)
        .map(|s| s.structure_id)
        .collect();

    for id in target_ids {
        log::debug!("Grounding voxel voided beneath structure {}. Inducing collapse.", id);
        let _ = destroy_structure_internal(ctx, id);
    }
}

/// Recalculates support values across the DAG of Support using BFS multi-source relaxation.
/// Identifies and collapses any structures disconnected from ground or whose support drops below min_support.
pub fn recalculate_structural_integrity(ctx: &ReducerContext) -> Vec<u64> {
    use std::collections::BTreeMap;

    let mut structures: BTreeMap<u64, Structure> = ctx.db.structure().iter().map(|s| (s.structure_id, s)).collect();
    if structures.is_empty() {
        return Vec::new();
    }

    let mut current_supports: BTreeMap<u64, f32> = BTreeMap::new();
    let mut best_parents: BTreeMap<u64, Option<u64>> = BTreeMap::new();
    let mut queue = Vec::new();

    for (id, s) in &structures {
        if s.is_grounded {
            let material = get_piece_material(&s.piece_type);
            let (max_support, _, _, _) = get_material_properties_or_default(ctx, material);
            current_supports.insert(*id, max_support);
            best_parents.insert(*id, None);
            queue.push(*id);
        } else {
            current_supports.insert(*id, 0.0);
            best_parents.insert(*id, s.parent_id);
        }
    }

    let mut outgoing_edges: BTreeMap<u64, Vec<(u64, f32, f32)>> = BTreeMap::new();
    for edge in ctx.db.structure_edge().iter() {
        outgoing_edges.entry(edge.parent_id).or_default().push((edge.child_id, edge.distance_h, edge.distance_v));
    }

    // Fallback: also map existing parent_id links from structure table
    for (id, s) in &structures {
        if let Some(pid) = s.parent_id {
            if let Some(parent) = structures.get(&pid) {
                let has_edge = outgoing_edges.get(&pid).map_or(false, |list| list.iter().any(|(cid, _, _)| *cid == *id));
                if !has_edge {
                    let (_, dh, dv, _) = calculate_inherited_support(ctx, parent, &s.piece_type, s.x, s.y, s.z);
                    outgoing_edges.entry(pid).or_default().push((*id, dh, dv));
                }
            }
        }
    }

    let mut head = 0;
    while head < queue.len() {
        let parent_id = queue[head];
        head += 1;

        let parent_sup = current_supports.get(&parent_id).copied().unwrap_or(0.0);

        if let Some(edges) = outgoing_edges.get(&parent_id) {
            for &(child_id, dh, dv) in edges {
                if let Some(child) = structures.get(&child_id) {
                    if child.is_grounded {
                        continue;
                    }
                    let child_mat = get_piece_material(&child.piece_type);
                    let (max_sup, min_sup, c_h, c_v) = get_material_properties_or_default(ctx, child_mat);
                    let inherited = parent_sup.min(max_sup) - (dh * c_h + dv * c_v);

                    if inherited >= min_sup {
                        let existing = current_supports.get(&child_id).copied().unwrap_or(0.0);
                        if inherited > existing {
                            current_supports.insert(child_id, inherited);
                            best_parents.insert(child_id, Some(parent_id));
                            queue.push(child_id);
                        }
                    }
                }
            }
        }
    }

    let mut collapsed = Vec::new();
    for (id, s) in structures.iter_mut() {
        if s.is_grounded {
            continue;
        }
        let child_mat = get_piece_material(&s.piece_type);
        let (max_sup, min_sup, _, _) = get_material_properties_or_default(ctx, child_mat);
        let final_support = current_supports.get(id).copied().unwrap_or(0.0);

        if final_support < min_sup {
            collapsed.push(*id);
        } else {
            s.current_support = final_support;
            s.stability = (final_support / max_sup * 100.0).clamp(1.0, 100.0) as u32;
            s.parent_id = best_parents.get(id).copied().flatten();
            ctx.db.structure().structure_id().update(s.clone());
        }
    }

    for id in &collapsed {
        if let Some(structure) = ctx.db.structure().structure_id().find(*id) {
            ctx.db.structure().structure_id().delete(*id);
            ctx.db.door_state().structure_id().delete(*id);

            let edge_ids: Vec<u64> = ctx.db.structure_edge().iter()
                .filter(|e| e.parent_id == *id || e.child_id == *id)
                .map(|e| e.edge_id)
                .collect();
            for eid in edge_ids {
                ctx.db.structure_edge().edge_id().delete(eid);
            }

            ctx.db.combat_event().insert(CombatEvent {
                id: 0,
                event_type: "StructureCollapse".to_string(),
                x: structure.x,
                y: structure.y,
                z: structure.z,
            });

            ctx.db.nav_event().insert(crate::NavEvent {
                id: 0,
                min_x: structure.x - 3.0,
                min_y: structure.y - 3.0,
                min_z: structure.z - 3.0,
                max_x: structure.x + 3.0,
                max_y: structure.y + 3.0,
                max_z: structure.z + 3.0,
            });
        }
    }

    collapsed
}

pub fn destroy_structure_internal(ctx: &ReducerContext, target_structure_id: u64) -> Result<(), String> {
    if let Some(structure) = ctx.db.structure().structure_id().find(target_structure_id) {
        ctx.db.structure().structure_id().delete(target_structure_id);
        ctx.db.door_state().structure_id().delete(target_structure_id);

        let edge_ids: Vec<u64> = ctx.db.structure_edge().iter()
            .filter(|e| e.parent_id == target_structure_id || e.child_id == target_structure_id)
            .map(|e| e.edge_id)
            .collect();
        for eid in edge_ids {
            ctx.db.structure_edge().edge_id().delete(eid);
        }

        ctx.db.combat_event().insert(CombatEvent {
            id: 0,
            event_type: "StructureCollapse".to_string(),
            x: structure.x,
            y: structure.y,
            z: structure.z,
        });

        ctx.db.nav_event().insert(crate::NavEvent {
            id: 0,
            min_x: structure.x - 3.0,
            min_y: structure.y - 3.0,
            min_z: structure.z - 3.0,
            max_x: structure.x + 3.0,
            max_y: structure.y + 3.0,
            max_z: structure.z + 3.0,
        });

        recalculate_structural_integrity(ctx);
    }
    Ok(())
}

/// Ticks automatic construction progress for all blueprints over a 3-second period.
/// Blueprints advance by 4% per 100ms tick, completing smoothly and restoring full health.
pub fn process_autobuild_tick(ctx: &ReducerContext) {
    let mut completed_nav_events = Vec::new();
    for mut s in ctx.db.structure().iter().filter(|s| s.is_blueprint) {
        let increment = 4; // Completes in 25 ticks (2.5s - 3.0s)
        s.construction_progress = (s.construction_progress + increment).min(100);
        s.current_health = (s.max_health * (s.construction_progress as f32 / 100.0)).max(1.0);

        if s.construction_progress >= 100 {
            s.is_blueprint = false;
            s.construction_progress = 100;
            s.current_health = s.max_health;
            completed_nav_events.push((s.x, s.y, s.z));
        }
        ctx.db.structure().structure_id().update(s);
    }

    for (x, y, z) in completed_nav_events {
        ctx.db.nav_event().insert(crate::NavEvent {
            id: 0,
            min_x: x - 3.0,
            min_y: y - 3.0,
            min_z: z - 3.0,
            max_x: x + 3.0,
            max_y: y + 3.0,
            max_z: z + 3.0,
        });
    }
}

#[reducer]
pub fn destroy_structure(ctx: &ReducerContext, target_structure_id: u64) -> Result<(), String> {
    let _session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    destroy_structure_internal(ctx, target_structure_id)
}