use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::player_session;
use crate::CombatEvent;
use crate::combat_event;
use crate::movement::transform;

// Architectural Note: Imported `peasant` trait to resolve E0599 compiler error. 
// SpacetimeDB v2.x generates table accessors as traits that must be explicitly brought into scope.
use crate::ai::{npc_brain, pet_component, peasant};

// ----------------------------------------------------------------------------
// DATA STRUCTURES
// ----------------------------------------------------------------------------

#[derive(SpacetimeType, Clone)]
pub struct Snapshot {
    pub tick_id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

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

/// Architectural Note: Defines the fundamental alignments for the threat system.
#[derive(SpacetimeType, Clone, Debug, PartialEq)]
pub enum Faction {
    Player,
    Villager, 
    Wildlife, 
    Goblin,   
}

#[derive(SpacetimeType, Clone, Debug, PartialEq)]
pub enum FactionStanding {
    Ally,
    Neutral,
    KillOnSight,
}

#[derive(Clone)]
#[table(accessor = faction_component, public)]
pub struct FactionComponent {
    #[primary_key]
    pub entity_id: u64,
    pub faction: Faction,
}

// ----------------------------------------------------------------------------
// CORE LOGIC & HELPERS
// ----------------------------------------------------------------------------

/// Architectural Note: Static relationship matrix. Evaluates how Faction A feels about Faction B.
pub fn get_standing(a: &Faction, b: &Faction) -> FactionStanding {
    match (a, b) {
        (Faction::Player, Faction::Villager) => FactionStanding::Neutral, 
        (Faction::Villager, Faction::Player) => FactionStanding::Neutral,
        (Faction::Player, Faction::Goblin) => FactionStanding::KillOnSight,
        (Faction::Goblin, Faction::Player) => FactionStanding::KillOnSight,
        (Faction::Goblin, Faction::Villager) => FactionStanding::KillOnSight,
        (Faction::Villager, Faction::Goblin) => FactionStanding::KillOnSight,
        
        (Faction::Wildlife, Faction::Player) => FactionStanding::KillOnSight,
        (Faction::Wildlife, Faction::Villager) => FactionStanding::KillOnSight,
        (Faction::Player, Faction::Wildlife) => FactionStanding::Neutral,
        (Faction::Villager, Faction::Wildlife) => FactionStanding::Neutral,
        
        _ => FactionStanding::Neutral,
    }
}

/// Architectural Note: Centralized authoritative damage application. 
/// Automatically handles component cleanup, entity deletion, and authoritative player respawns.
pub fn apply_damage(ctx: &ReducerContext, target_id: u64, amount: f32) {
    if let Some(mut hp) = ctx.db.health().entity_id().find(target_id) {
        hp.current = (hp.current - amount).max(0.0);
        
        if hp.current == 0.0 {
            // Architectural Note: Check if the dying entity is tied to an active player session.
            if ctx.db.player_session().entity_id().find(target_id).is_some() {
                hp.current = hp.max;
                ctx.db.health().entity_id().update(hp);

                if let Some(mut transform) = ctx.db.transform().entity_id().find(target_id) {
                    transform.x = 0.0;
                    transform.z = 0.0;
                    transform.y = crate::get_terrain_height(0.0, 0.0) + 10.0;
                    ctx.db.transform().entity_id().update(transform);
                }
                
                log::info!("Player {} died and respawned at the origin.", target_id);
            } else {
                ctx.db.health().entity_id().delete(target_id);
                ctx.db.transform().entity_id().delete(target_id);
                ctx.db.faction_component().entity_id().delete(target_id);
                ctx.db.npc_brain().entity_id().delete(target_id);
                ctx.db.pet_component().entity_id().delete(target_id);
                
                if ctx.db.peasant().entity_id().find(target_id).is_some() {
                    ctx.db.peasant().entity_id().delete(target_id);
                }
                
                log::info!("Entity {} reached 0 HP and was removed from the world.", target_id);
            }
        } else {
            ctx.db.health().entity_id().update(hp);
        }
    }
}

// ----------------------------------------------------------------------------
// REDUCERS
// ----------------------------------------------------------------------------

/// Evaluates a client's hitscan weapon fire event against historical hitboxes.
#[reducer]
pub fn fire_weapon(
    ctx: &ReducerContext, 
    client_tick: u64, 
    origin_x: f32, origin_y: f32, origin_z: f32, 
    dir_x: f32, dir_y: f32, dir_z: f32
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    let mut hit_entity = None;
    let mut closest_dist = f32::MAX;
    let mut hit_location = (0.0, 0.0, 0.0);

    // 1. Lag Compensation: Rewind and Intersect
    for history in ctx.db.hitbox_history().iter() {
        if history.entity_id == session.entity_id { continue; } 

        let closest_snapshot = history.snapshots.iter()
            .min_by_key(|s| (s.tick_id as i64 - client_tick as i64).abs());

        if let Some(snap) = closest_snapshot {
            let radius = 0.8_f32;
            let cx = snap.x; let cy = snap.y + 1.0; let cz = snap.z; 
            
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
        apply_damage(ctx, target_id, 20.0);
        
        ctx.db.combat_event().insert(CombatEvent {
            id: 0,
            event_type: "HitPlayer".to_string(),
            x: hit_location.0,
            y: hit_location.1,
            z: hit_location.2,
        });
        
        log::info!("Hit validated via Lag Compensation! Entity {} shot entity {}.", session.entity_id, target_id);
    }

    Ok(())
}