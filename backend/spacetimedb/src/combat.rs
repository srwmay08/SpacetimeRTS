use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table}; // Architectural Note: Added 'Table' trait to resolve .iter() scope error in SpacetimeDB v2.x.
use crate::movement::player_session; // Architectural Note: v2.x requires explicitly bringing traits into scope.
use crate::CombatEvent; // Architectural Note: Required to broadcast global damage confirmation packets.
use crate::combat_event; // Architectural Note: Explicitly import v2.x generated table accessor trait.

// Architectural Note: Defines a single frame of historical state for an entity.
// We derive SpacetimeType explicitly here because it is an embedded struct, not a root table.
#[derive(SpacetimeType, Clone)]
pub struct Snapshot {
    pub tick_id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// Architectural Note: Stores the rolling 500ms lag compensation buffer per entity.
// The `#[table]` macro automatically derives `SpacetimeType` for root schemas.
#[derive(Clone)]
#[table(accessor = hitbox_history, public)]
pub struct HitboxHistory {
    #[primary_key]
    pub entity_id: u64,
    pub snapshots: Vec<Snapshot>,
}

#[derive(Clone)]
#[table(accessor = health, public)]
pub struct Health {
    #[primary_key]
    pub entity_id: u64,
    pub current: f32,
    pub max: f32,
}

/// Evaluates a client's hitscan weapon fire event against historical hitboxes.
/// Architectural Note: By passing the client's local tick_id, the server can rewind the 
/// physical state of all targets to exactly where the client saw them when they fired, 
/// eliminating perceived latency while maintaining absolute server authority over hit registration.
#[reducer]
pub fn fire_weapon(
    ctx: &ReducerContext, 
    client_tick: u64, 
    origin_x: f32, origin_y: f32, origin_z: f32, 
    dir_x: f32, dir_y: f32, dir_z: f32
) -> Result<(), String> {
    // Authenticate the shooter
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    let mut hit_entity = None;
    let mut closest_dist = f32::MAX;
    let mut hit_location = (0.0, 0.0, 0.0);

    // 1. Lag Compensation: Rewind and Intersect
    for history in ctx.db.hitbox_history().iter() {
        if history.entity_id == session.entity_id { continue; } // Prevent self-damage

        // Find the server snapshot closest to the client's reported timestamp
        let closest_snapshot = history.snapshots.iter()
            .min_by_key(|s| (s.tick_id as i64 - client_tick as i64).abs());

        if let Some(snap) = closest_snapshot {
            // Basic Ray-Sphere Intersection (Hitbox radius of 0.8 units)
            // Architectural Note: This math is highly optimized to avoid unnecessary allocations, 
            // protecting the compute energy (TeV) economy of the SpacetimeDB instance.
            let radius = 0.8_f32;
            let cx = snap.x; let cy = snap.y + 1.0; let cz = snap.z; // Center of mass
            
            let ocx = origin_x - cx; 
            let ocy = origin_y - cy; 
            let ocz = origin_z - cz;
            
            let b = (ocx * dir_x + ocy * dir_y + ocz * dir_z) * 2.0;
            let c = ocx * ocx + ocy * ocy + ocz * ocz - (radius * radius);
            let discriminant = b * b - 4.0 * c;

            if discriminant > 0.0 {
                let dist = (-b - discriminant.sqrt()) / 2.0;
                if dist > 0.0 && dist < closest_dist {
                    closest_dist = dist;
                    hit_entity = Some(history.entity_id);
                    // Architectural Note: Calculate the exact 3D coordinate of the impact
                    // for the global damage confirmation packet.
                    hit_location = (
                        origin_x + dir_x * dist,
                        origin_y + dir_y * dist,
                        origin_z + dir_z * dist,
                    );
                }
            }
        }
    }

    // 2. Authoritative Damage Application & Global Sync
    if let Some(target_id) = hit_entity {
        if let Some(mut hp) = ctx.db.health().entity_id().find(target_id) {
            hp.current = (hp.current - 20.0).max(0.0);
            ctx.db.health().entity_id().update(hp.clone());
            
            // Architectural Note: Push the damage confirmation packet to the global CombatEvent table.
            // This transactionally guarantees that if the health bar updates for the RTS view, 
            // the micro-engagement view will simultaneously render the blood splatter/impact.
            ctx.db.combat_event().insert(CombatEvent {
                id: 0,
                event_type: "HitPlayer".to_string(),
                x: hit_location.0,
                y: hit_location.1,
                z: hit_location.2,
            });
            
            log::info!("Hit validated via Lag Compensation! Entity {} shot entity {}. HP remaining: {}", session.entity_id, target_id, hp.current);
        }
    } else {
        // Architectural Note: We purposely do NOT insert a CombatEvent on a miss.
        // The client relies on its own immediate prediction to render the tracer, keeping
        // server TeV costs completely minimal during suppression fire.
    }

    Ok(())
}