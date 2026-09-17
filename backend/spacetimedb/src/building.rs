use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::player_session;
use crate::resource_stockpile; 
use crate::CombatEvent; // Architectural Note: Required to broadcast collapse visual effects.
use crate::combat_event; 

/// Defines a localized snap point on a modular piece for server-side validation.
#[derive(SpacetimeType, Clone, Debug)]
pub struct SocketDef {
    pub name: String,
    pub offset_x: f32,
    pub offset_y: f32,
    pub offset_z: f32,
}

/// Authoritative database table storing all placed player structures.
#[table(accessor = structure, public)]
#[derive(Clone)]
pub struct Structure {
    #[primary_key] #[auto_inc]
    pub structure_id: u64,
    
    // Architectural Note: Establishes a strict parent-child stability graph. 
    // If a piece loses its parent, it triggers a recursive physical collapse.
    pub parent_id: Option<u64>, 
    
    pub piece_type: String,
    
    // Architectural Note: Load-bearing calculations to prevent floating geometry.
    pub stability: u32,         
    pub is_grounded: bool,      
    
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub rot_w: f32,
    pub owner_id: u64,
}

/// Authoritative reducer to validate material costs, structural integrity, and persist structures.
/// Architectural Note: Signature updated to require `parent_id` from the client's socket raycast.
#[reducer]
pub fn place_structure(
    ctx: &ReducerContext,
    parent_id: Option<u64>, 
    piece_type: String,
    x: f32, y: f32, z: f32,
    rot_x: f32, rot_y: f32, rot_z: f32, rot_w: f32,
) -> Result<(), String> {
    // Authenticate builder session
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    let mut stockpile = ctx.db.resource_stockpile().entity_id().find(session.entity_id)
        .ok_or("Stockpile not found")?;

    // Architectural Note: Hardcoded material costs and structural decay penalties.
    let (wood_cost, decay_penalty) = match piece_type.as_str() {
        "Foundation" => (20, 0),   // Foundations are grounded, no inherited decay.
        "Wall"       => (10, 20),
        "Floor"      => (15, 25),
        "Roof"       => (15, 30),
        "Ramp"       => (20, 25),
        _ => return Err(format!("Unknown piece type: {}", piece_type)),
    };

    if stockpile.wood < wood_cost {
        return Err("Not enough wood in stockpile to build structure".to_string());
    }

    // Architectural Note: Structural Integrity & Physics Validation
    let mut stability = 0;
    let mut is_grounded = false;

    if piece_type == "Foundation" && parent_id.is_none() {
        let ground_y = crate::get_terrain_height(x, z);
        if (y - ground_y).abs() < 2.5 { 
            is_grounded = true;
            stability = 100; // Max structural integrity for grounded units.
        } else {
            return Err("Foundations must be physically anchored to the terrain mesh.".to_string());
        }
    } else if let Some(pid) = parent_id {
        let parent = ctx.db.structure().structure_id().find(pid)
            .ok_or("Parent structure not found in the database")?;
            
        if parent.stability <= decay_penalty {
            return Err("Structural integrity depleted. Cannot support additional mass.".to_string());
        }
        stability = parent.stability - decay_penalty;
    } else {
        return Err("Piece must be grounded to terrain or snapped to a valid parent structure.".to_string());
    }

    // Deduct resources and insert authoritative record
    stockpile.wood -= wood_cost;
    ctx.db.resource_stockpile().entity_id().update(stockpile);

    ctx.db.structure().insert(Structure {
        structure_id: 0,
        parent_id,
        piece_type: piece_type.clone(),
        stability,
        is_grounded,
        x, y, z,
        rot_x, rot_y, rot_z, rot_w,
        owner_id: session.entity_id,
    });

    log::info!("Player {} placed {}. Stability: {}/100", session.entity_id, piece_type, stability);
    Ok(())
}

/// Processes explosive or manual destruction and enforces physical collapse.
/// Architectural Note: Implements an iterative DFS traversal to find and destroy all child geometry 
/// structurally dependent on the destroyed root piece without risking a Wasm stack overflow.
#[reducer]
pub fn destroy_structure(
    ctx: &ReducerContext,
    target_structure_id: u64
) -> Result<(), String> {
    let _session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    let mut collapse_queue = vec![target_structure_id];
    let mut index = 0;

    // Traverse the stability graph to identify all orphaned children
    while index < collapse_queue.len() {
        let current_id = collapse_queue[index];
        
        for child in ctx.db.structure().iter().filter(|s| s.parent_id == Some(current_id)) {
            collapse_queue.push(child.structure_id);
        }
        index += 1;
    }

    // Process the physical collapse cascade
    for id in collapse_queue.iter() {
        if let Some(structure) = ctx.db.structure().structure_id().find(*id) {
            ctx.db.structure().structure_id().delete(*id);
            
            // Generate visual combat event for the collapse for clients to render debris
            ctx.db.combat_event().insert(CombatEvent { 
                id: 0,
                event_type: "StructureCollapse".to_string(),
                x: structure.x,
                y: structure.y,
                z: structure.z,
            });
        }
    }

    log::info!("Structural collapse triggered. Destroyed {} interdependent entities.", collapse_queue.len());
    Ok(())
}