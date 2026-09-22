// ----------------------------------------------------------------------------
// MOVEMENT STRUCTURES & IMPORTS
// ----------------------------------------------------------------------------
use spacetimedb::{table, reducer, ReducerContext, Identity, Table};
use crate::combat::{hitbox_history, Snapshot}; // Architectural Note: Required for lag compensation appending.
use crate::waypoint;
use crate::player_perspective;

/// Represents the physical manifestation of an entity in the game world.
/// We store the `last_processed_tick` directly on the transform to ensure 
/// that whenever a client receives a state sync, they know exactly which 
/// local inputs have already been applied by the server.
#[derive(Clone)]
#[table(accessor = transform, public)]
pub struct Transform {
    #[primary_key]
    pub entity_id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub last_processed_tick: u64, 
}

/// Links a player's connection identity to their in-game physical entity.
#[derive(Clone)]
#[table(accessor = player_session, public)]
pub struct PlayerSession {
    #[primary_key]
    pub identity: Identity,
    #[unique]
    pub entity_id: u64,
}
// ----------------------------------------------------------------------------
// AUTHORITATIVE MOVEMENT REDUCERS
// ----------------------------------------------------------------------------
/// Processes a client's movement request.
/// 
/// **Architecture Note:** We process a `delta` (velocity * dt) rather than an 
/// absolute position. This prevents a compromised client from teleporting. 
/// We enforce a strict maximum magnitude check to protect the server's economy.
#[reducer]
pub fn process_movement(
    ctx: &ReducerContext,
    tick_id: u64,
    delta_x: f32,
    delta_y: f32,
    delta_z: f32,
) -> Result<(), String> {
    // 1. Authenticate and resolve the sender's entity identity
    let session = ctx.db.player_session()
        .identity()
        .find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active player session found for sender".to_string())?;
        
    // 2. Retrieve the entity's current authoritative transform
    let mut transform = ctx.db.transform()
        .entity_id()
        .find(session.entity_id)
        .ok_or_else(|| format!("State Error: Transform missing for entity {}", session.entity_id))?;
        
    // 3. Drop stale ticks to prevent out-of-order UDP/TCP packet replay issues
    if tick_id <= transform.last_processed_tick {
        log::debug!(
            "Dropped stale tick {} for entity {}. Current server tick is {}",
            tick_id, session.entity_id, transform.last_processed_tick
        );
        return Ok(());
    }

    // 4. Validate movement magnitude (Basic Anti-Speedhack)
    let max_speed = 5.0_f32; 
    let max_speed_sq = max_speed * max_speed;
    let magnitude_sq = (delta_x * delta_x) + (delta_y * delta_y) + (delta_z * delta_z);

    let (dx, dy, dz) = if magnitude_sq > max_speed_sq {
        let magnitude = magnitude_sq.sqrt(); 
        let scale = max_speed / magnitude;
        log::debug!("Speedhack mitigated for entity {}. Scaling delta.", session.entity_id);
        (delta_x * scale, delta_y * scale, delta_z * scale)
    } else {
        (delta_x, delta_y, delta_z)
    };

    // 5. Apply the validated state changes
    transform.x += dx;
    transform.y += dy;
    transform.z += dz;
    transform.last_processed_tick = tick_id;
    
    // Architectural Note: Absolute Server-Side Terrain Clamp.
    // Adjusted to 1.05 to mirror the client-side friction-nullifying hover epsilon.
    // This perfectly synchronizes backend validation with front-end visual physics.
    let ground_y = crate::get_terrain_height(transform.x, transform.z);
    if transform.y < ground_y + 1.05 {
        transform.y = ground_y + 1.05;
    }

    transform.chunk_x = (transform.x / 50.0).floor() as i32;
    transform.chunk_z = (transform.z / 50.0).floor() as i32;

    // 6. Commit the updated transform back to SpacetimeDB
    ctx.db.transform().entity_id().update(transform.clone());
    
    // 7. Architectural Note: Append to Lag Compensation Buffer.
    // 10 snapshots at a 20Hz network tick rate provides a rolling 500ms rewind history.
    if let Some(mut history) = ctx.db.hitbox_history().entity_id().find(session.entity_id) {
        history.snapshots.push(Snapshot {
            tick_id,
            x: transform.x,
            y: transform.y,
            z: transform.z,
        });
        
        if history.snapshots.len() > 10 {
            history.snapshots.remove(0); 
        }
        ctx.db.hitbox_history().entity_id().update(history.clone());
    }
    
    Ok(())
}

/// Architectural Note: 2. Command Structure (Live Orders & HUD Integration)
/// Allows the Commander to project intent onto the physical map for the FPS players.
#[reducer]
pub fn issue_waypoint(
    ctx: &ReducerContext,
    x: f32, y: f32, z: f32,
    order_type: String
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;
        
    let perspective = ctx.db.player_perspective().entity_id().find(session.entity_id)
        .ok_or("Perspective not found")?;

    // Symbiotic Dependency: Only the Commander operates the macro-layer orders
    if perspective.camera_mode != "RTS" {
        return Err("Only the Commander (RTS Mode) can issue tactical waypoints.".to_string());
    }

    ctx.db.waypoint().insert(crate::Waypoint {
        waypoint_id: 0,
        commander_id: session.entity_id,
        x, y, z, order_type,
        expires_at: ctx.timestamp.to_micros_since_unix_epoch() as u64 + 30_000_000, // 30s HUD Expiry
    });

    Ok(())
}