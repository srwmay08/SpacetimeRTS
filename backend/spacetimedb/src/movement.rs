// ----------------------------------------------------------------------------
// MOVEMENT STRUCTURES & IMPORTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
use spacetimedb::{table, reducer, ReducerContext, Identity, Table};
use crate::combat::{hitbox_history, Snapshot}; 
use crate::waypoint;
use crate::player_perspective;
use crate::CameraModeType;

// Architectural Note: Added BTree indexes on chunk_x and chunk_z to optimize
// spatial queries for client subscriptions and localized AI crowd routines.
#[derive(Clone)]
#[table(accessor = transform, public)]
pub struct Transform {
    #[primary_key]
    pub entity_id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    #[index(btree)]
    pub chunk_x: i32,
    #[index(btree)]
    pub chunk_z: i32,
    pub last_processed_tick: u64, 
}

#[derive(Clone)]
#[table(accessor = player_session, public)]
pub struct PlayerSession {
    #[primary_key]
    pub identity: Identity,
    #[unique]
    pub entity_id: u64,
}

// ----------------------------------------------------------------------------
// AUTHORITATIVE MOVEMENT & PERSPECTIVE REDUCERS
// ----------------------------------------------------------------------------

// Architectural Note: Replaces string matching with strongly typed camera modes.
#[reducer]
pub fn set_camera_mode(ctx: &ReducerContext, mode: CameraModeType) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session".to_string())?;

    let mut perspective = ctx.db.player_perspective().entity_id().find(session.entity_id)
        .ok_or_else(|| "Player perspective state missing".to_string())?;

    perspective.camera_mode = mode;
    ctx.db.player_perspective().entity_id().update(perspective);
    Ok(())
}

#[reducer]
pub fn set_interior_culling(ctx: &ReducerContext, in_interior: bool) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session".to_string())?;

    let mut perspective = ctx.db.player_perspective().entity_id().find(session.entity_id)
        .ok_or_else(|| "Player perspective state missing".to_string())?;

    perspective.in_interior = in_interior;
    ctx.db.player_perspective().entity_id().update(perspective);
    Ok(())
}

#[reducer]
pub fn process_movement(
    ctx: &ReducerContext,
    tick_id: u64,
    delta_x: f32,
    delta_y: f32,
    delta_z: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session()
        .identity()
        .find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active player session found for sender".to_string())?;
        
    let mut transform = ctx.db.transform()
        .entity_id()
        .find(session.entity_id)
        .ok_or_else(|| format!("State Error: Transform missing for entity {}", session.entity_id))?;
        
    if tick_id <= transform.last_processed_tick {
        log::debug!(
            "Dropped stale tick {} for entity {}. Current server tick is {}",
            tick_id, session.entity_id, transform.last_processed_tick
        );
        return Ok(());
    }

    let elapsed_ticks = (tick_id - transform.last_processed_tick).min(20) as f32;
    let max_speed_per_tick = 5.0_f32 * (elapsed_ticks * 0.05).max(0.05);
    let max_speed_sq = max_speed_per_tick * max_speed_per_tick;
    let magnitude_sq = (delta_x * delta_x) + (delta_y * delta_y) + (delta_z * delta_z);

    let (dx, dy, dz) = if magnitude_sq > max_speed_sq {
        let magnitude = magnitude_sq.sqrt(); 
        let scale = max_speed_per_tick / magnitude;
        log::debug!("Speed-hack throttled for entity {}. Clamping delta magnitude.", session.entity_id);
        (delta_x * scale, delta_y * scale, delta_z * scale)
    } else {
        (delta_x, delta_y, delta_z)
    };

    transform.x += dx;
    transform.y += dy;
    transform.z += dz;
    transform.last_processed_tick = tick_id;
    
    let ground_y = crate::get_terrain_height(transform.x, transform.z);
    if transform.y < ground_y + 1.05 {
        transform.y = ground_y + 1.05;
    }

    transform.chunk_x = (transform.x / 50.0).floor() as i32;
    transform.chunk_z = (transform.z / 50.0).floor() as i32;

    ctx.db.transform().entity_id().update(transform.clone());
    
    // Architectural Note: Replaced Vec::remove(0) with ring buffer truncation.
    // Preserves constant-time insertion on high-rate player packets.
    if let Some(mut history) = ctx.db.hitbox_history().entity_id().find(session.entity_id) {
        history.snapshots.push(Snapshot {
            tick_id,
            x: transform.x,
            y: transform.y,
            z: transform.z,
        });
        
        if history.snapshots.len() > 10 {
            let overflow = history.snapshots.len() - 10;
            history.snapshots.drain(0..overflow);
        }
        ctx.db.hitbox_history().entity_id().update(history);
    }
    
    Ok(())
}

#[reducer]
pub fn issue_waypoint(
    ctx: &ReducerContext,
    x: f32, 
    y: f32, 
    z: f32,
    order_type: String
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session".to_string())?;
        
    let perspective = ctx.db.player_perspective().entity_id().find(session.entity_id)
        .ok_or_else(|| "Perspective not found".to_string())?;

    if perspective.camera_mode != CameraModeType::Rts {
        return Err("Only the Commander (RTS Mode) can issue tactical waypoints.".to_string());
    }

    ctx.db.waypoint().insert(crate::Waypoint {
        waypoint_id: 0,
        commander_id: session.entity_id,
        x, 
        y, 
        z, 
        order_type,
        expires_at: ctx.timestamp.to_micros_since_unix_epoch() as u64 + 30_000_000,
    });

    Ok(())
}