// ----------------------------------------------------------------------------
// BUILDING CORE STRUCTURES & IMPORTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Manages modular structural assembly, anchor stability, and
// destruction cascades. Starter pieces (Foundations, Workbenches, and Campfires)
// anchor directly to terrain or voxels without requiring a pre-existing Workbench,
// while advanced pieces (Walls, Floors, Roofs, Ramps) enforce active workbench proximity.

use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::player_session;
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

#[table(accessor = structure, public)]
#[derive(Clone)]
pub struct Structure {
    #[primary_key] #[auto_inc]
    pub structure_id: u64,
    #[index(btree)]
    pub parent_id: Option<u64>,
    pub piece_type: String,
    pub stability: u32,
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

// ----------------------------------------------------------------------------
// BUILDING LOGIC & REDUCERS
// ----------------------------------------------------------------------------

pub fn get_piece_max_health(piece_type: &str) -> f32 {
    match piece_type {
        "Foundation" => 400.0,
        "Wall" => 200.0,
        "Floor" => 150.0,
        "Roof" => 150.0,
        "Ramp" => 250.0,
        "Workbench" => 150.0,
        "Campfire" => 60.0,
        _ => 100.0,
    }
}

pub fn is_covered(ctx: &ReducerContext, x: f32, y: f32, z: f32) -> bool {
    for s in ctx.db.structure().iter() {
        if s.piece_type == "Roof" && !s.is_blueprint {
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
    let is_starter_piece = matches!(piece_type.as_str(), "Foundation" | "Workbench" | "Campfire");
    if !is_starter_piece {
        let mut near_workbench = false;
        for s in ctx.db.structure().iter().filter(|s| s.piece_type == "Workbench" && !s.is_blueprint) {
            if (s.x - x).powi(2) + (s.z - z).powi(2) <= 400.0 {
                near_workbench = true;
                break;
            }
        }
        if !near_workbench {
            return Err("Construction of walls, floors, roofs, and ramps requires a nearby active Workbench.".to_string());
        }
    }

    let decay_penalty = match piece_type.as_str() {
        "Workbench" | "Campfire" | "Bed" | "Foundation" => 0,
        "Wall" => 20,
        "Floor" => 25,
        "Roof" => 30,
        "Ramp" => 25,
        _ => return Err(format!("Unknown piece type: {}", piece_type)),
    };

    let stability: u32;
    let is_grounded: bool;

    if (piece_type == "Foundation" || piece_type == "Ramp" || piece_type == "Workbench" || piece_type == "Campfire") && parent_id.is_none() {
        let ground_y = crate::get_terrain_height(x, z);
        let voxel_mat = voxel::get_voxel_at(ctx, x, y - 0.5, z);

        if (y - ground_y).abs() < 4.0 || voxel_mat.is_solid() {
            is_grounded = true;
            stability = 100;
        } else {
            return Err("Foundations, Workbenches, and Campfires must anchor to terrain or solid voxels.".to_string());
        }
    } else if let Some(pid) = parent_id {
        let parent = ctx.db.structure().structure_id().find(pid)
            .ok_or_else(|| "Parent structure not found in database.".to_string())?;

        if parent.stability <= decay_penalty {
            return Err("Structural integrity depleted. Cannot support additional mass.".to_string());
        }
        stability = parent.stability.saturating_sub(decay_penalty);
        is_grounded = false;
    } else {
        return Err("Piece must anchor to terrain/voxels or snap to a valid parent structure.".to_string());
    }

    let max_hp = get_piece_max_health(&piece_type);

    ctx.db.structure().insert(Structure {
        structure_id: 0,
        parent_id,
        piece_type: piece_type.clone(),
        stability,
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

    // Architectural Note: Evenly divisible construction costs by 4 swings (25% per hammer hit)
    let (wood_cost, stone_cost) = match structure.piece_type.as_str() {
        "Workbench" => (8, 0),    // 2 wood per swing
        "Campfire" => (4, 4),     // 1 wood, 1 stone per swing
        "Foundation" => (20, 0),  // 5 wood per swing
        "Wall" => (8, 0),         // 2 wood per swing
        "Floor" => (12, 0),       // 3 wood per swing
        "Roof" => (12, 0),        // 3 wood per swing
        "Ramp" => (16, 0),        // 4 wood per swing
        _ => (4, 0),
    };

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

pub fn destroy_structure_internal(ctx: &ReducerContext, target_structure_id: u64) -> Result<(), String> {
    let mut collapse_queue = vec![target_structure_id];
    let mut index = 0;

    while index < collapse_queue.len() {
        let current_id = collapse_queue[index];
        for child in ctx.db.structure().iter().filter(|s| s.parent_id == Some(current_id)) {
            collapse_queue.push(child.structure_id);
        }
        index += 1;
    }

    for id in &collapse_queue {
        if let Some(structure) = ctx.db.structure().structure_id().find(*id) {
            ctx.db.structure().structure_id().delete(*id);

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
    Ok(())
}

#[reducer]
pub fn destroy_structure(ctx: &ReducerContext, target_structure_id: u64) -> Result<(), String> {
    let _session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    destroy_structure_internal(ctx, target_structure_id)
}