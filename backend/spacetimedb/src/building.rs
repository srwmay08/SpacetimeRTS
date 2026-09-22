// ----------------------------------------------------------------------------
// BUILDING CORE STRUCTURES & IMPORTS
// ----------------------------------------------------------------------------
use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::player_session;
use crate::inventory; 
use crate::CombatEvent; 
use crate::combat_event; 
use crate::nav_event;
use crate::player_perspective; 
use crate::waypoint;

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
    pub parent_id: Option<u64>, 
    pub piece_type: String,
    pub stability: u32,         
    pub is_grounded: bool,      
    
    // Architectural Note: 4. Economy, Survival, & Base Building
    // Integrates Commander Blueprints vs Field Construction logic
    pub is_blueprint: bool,
    pub construction_progress: u32,
    
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub rot_w: f32,
    pub owner_id: u64,
}

// ----------------------------------------------------------------------------
// BUILDING LOGIC & REDUCERS
// ----------------------------------------------------------------------------
/// Architectural Note: Checks if a coordinate is protected by a Roof piece.
/// Used to validate if a Workbench is covered for crafting, or if a Campfire is safe from rain.
pub fn is_covered(ctx: &ReducerContext, x: f32, y: f32, z: f32) -> bool {
    for s in ctx.db.structure().iter() {
        if s.piece_type == "Roof" && !s.is_blueprint {
            let dist_sq = (s.x - x).powi(2) + (s.z - z).powi(2);
            // Assuming roof piece provides a roughly 3m radius of cover directly beneath it.
            if dist_sq <= 9.0 {
                if s.y > y && (s.y - y) < 10.0 {
                    return true;
                }
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
    x: f32, y: f32, z: f32,
    rot_x: f32, rot_y: f32, rot_z: f32, rot_w: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    let perspective = ctx.db.player_perspective().entity_id().find(session.entity_id)
        .ok_or("Perspective not found")?;

    // Architectural Note: 2. Command Structure - Symbiotic Dependency
    // The Commander operates from a macro perspective to issue orders and drop blueprints,
    // but relies on FPS players to physically supply the materials.
    if perspective.camera_mode != "RTS" {
        return Err("Only the Commander (RTS Mode) can place building blueprints.".to_string());
    }

    // Valheim Core Rule: You must build within the radius of a Workbench (Except the Workbench and Campfire itself)
    if piece_type != "Workbench" && piece_type != "Campfire" {
        let mut near_workbench = false;
        for s in ctx.db.structure().iter().filter(|s| s.piece_type == "Workbench" && !s.is_blueprint) {
            if (s.x - x).powi(2) + (s.z - z).powi(2) < 400.0 { // 20m Build Radius
                near_workbench = true;
                break;
            }
        }
        if !near_workbench {
            return Err("Building requires the active working area of a constructed Workbench.".to_string());
        }
    }

    let decay_penalty = match piece_type.as_str() {
        "Workbench"   => 0, 
        "Campfire"    => 0,
        "Bed"         => 0,
        "Foundation"  => 0,   
        "Wall"        => 20,
        "Floor"       => 25,
        "Roof"        => 30,
        "Ramp"        => 25,
        _ => return Err(format!("Unknown piece type: {}", piece_type)),
    };

    let stability: u32;
    let is_grounded: bool;

    if (piece_type == "Foundation" || piece_type == "Ramp" || piece_type == "Workbench" || piece_type == "Campfire") && parent_id.is_none() {
        let ground_y = crate::get_terrain_height(x, z);
        if (y - ground_y).abs() < 3.0 { 
            is_grounded = true;
            stability = 100; 
        } else {
            return Err("Foundations, Workbenches, and Campfires must be physically anchored to the terrain mesh.".to_string());
        }
    } else if let Some(pid) = parent_id {
        let parent = ctx.db.structure().structure_id().find(pid)
            .ok_or("Parent structure not found in the database")?;
            
        if parent.stability <= decay_penalty {
            return Err("Structural integrity depleted. Cannot support additional mass.".to_string());
        }
        stability = parent.stability - decay_penalty;
        is_grounded = false;
    } else {
        return Err("Piece must be grounded to terrain or snapped to a valid parent structure.".to_string());
    }

    // Architectural Note: Blueprint Placement. Initial resource cost is 0.
    ctx.db.structure().insert(Structure {
        structure_id: 0, parent_id, piece_type: piece_type.clone(),
        stability, is_grounded, x, y, z, rot_x, rot_y, rot_z, rot_w, owner_id: session.entity_id,
        is_blueprint: true, construction_progress: 0,
    });

    // Automatically assign a Build Waypoint to the FPS players
    ctx.db.waypoint().insert(crate::Waypoint {
        waypoint_id: 0,
        commander_id: session.entity_id,
        x, y: y + 2.0, z,
        order_type: "Build".to_string(),
        expires_at: ctx.timestamp.to_micros_since_unix_epoch() as u64 + 120_000_000, 
    });

    Ok(())
}

/// Architectural Note: 4. Economy & Base Building
/// FPS players must physically walk up to a blueprint and strike it with a hammer,
/// transferring raw materials from their inventory to advance the construction state.
#[reducer]
pub fn contribute_construction(ctx: &ReducerContext, structure_id: u64) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;
        
    let mut structure = ctx.db.structure().structure_id().find(structure_id)
        .ok_or("Structure not found")?;
        
    if !structure.is_blueprint {
        return Err("Structure is already fully constructed.".to_string());
    }

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .unwrap_or_else(|| crate::Inventory { entity_id: session.entity_id, slots: vec![], discovered_items: vec![] });
        
    let has_hammer = inv.slots.iter().any(|s| s.item_type == "Hammer" && s.count > 0);
    if !has_hammer {
        return Err("You must equip a Hammer to contribute materials.".to_string());
    }

    let (wood_cost, stone_cost) = match structure.piece_type.as_str() {
        "Workbench"   => (10, 0), 
        "Campfire"    => (2, 5),
        "Foundation"  => (2, 0),   
        "Wall"        => (2, 0),
        "Floor"       => (2, 0),
        "Roof"        => (2, 0),
        "Ramp"        => (2, 0),
        _ => (2, 0),
    };

    // Calculate micro-swing cost (4 swings to build)
    let wood_swing = (wood_cost as f32 * 0.25).ceil() as u32;
    let stone_swing = (stone_cost as f32 * 0.25).ceil() as u32;

    if !crate::has_item(&inv, "Wood", wood_swing) && wood_swing > 0 {
        return Err(format!("Insufficient Wood. Need {} per hammer swing.", wood_swing));
    }
    if !crate::has_item(&inv, "Stone", stone_swing) && stone_swing > 0 {
        return Err(format!("Insufficient Stone. Need {} per hammer swing.", stone_swing));
    }

    crate::remove_item(&mut inv, "Wood", wood_swing);
    crate::remove_item(&mut inv, "Stone", stone_swing);
    
    structure.construction_progress += 25;
    
    if structure.construction_progress >= 100 {
        structure.is_blueprint = false;
        structure.construction_progress = 100;
        
        ctx.db.nav_event().insert(crate::NavEvent {
            id: 0,
            min_x: structure.x - 3.0, min_y: structure.y - 3.0, min_z: structure.z - 3.0,
            max_x: structure.x + 3.0, max_y: structure.y + 3.0, max_z: structure.z + 3.0,
        });
    }
    
    ctx.db.structure().structure_id().update(structure);
    ctx.db.inventory().entity_id().update(inv);
    Ok(())
}

#[reducer]
pub fn destroy_structure(
    ctx: &ReducerContext,
    target_structure_id: u64
) -> Result<(), String> {
    let _session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    let mut collapse_queue = vec![target_structure_id];
    let mut index = 0;

    while index < collapse_queue.len() {
        let current_id = collapse_queue[index];
        for child in ctx.db.structure().iter().filter(|s| s.parent_id == Some(current_id)) {
            collapse_queue.push(child.structure_id);
        }
        index += 1;
    }

    for id in collapse_queue.iter() {
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
                min_x: structure.x - 3.0, min_y: structure.y - 3.0, min_z: structure.z - 3.0,
                max_x: structure.x + 3.0, max_y: structure.y + 3.0, max_z: structure.z + 3.0,
            });
        }
    }
    Ok(())
}