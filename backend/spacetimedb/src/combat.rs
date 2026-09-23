// ----------------------------------------------------------------------------
// COMBAT MODULE & AUTHORITATIVE RESOLUTION (SpacetimeDB v2.x / Rust 2024)
// ----------------------------------------------------------------------------
use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::player_session;
use crate::CombatEvent;
use crate::combat_event;
use crate::movement::transform;
use crate::inventory;
use crate::ai::{npc_brain, pet_component, peasant, harvestable_corpse};
use crate::building::structure;

// ----------------------------------------------------------------------------
// DATA STRUCTURES & SCHEMAS
// ----------------------------------------------------------------------------

#[derive(SpacetimeType, Clone, Debug, PartialEq)]
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

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Faction {
    #[default]
    Player,
    Villager, 
    Wildlife, 
    Goblin,   
}

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
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

pub fn get_standing(a: &Faction, b: &Faction) -> FactionStanding {
    match (a, b) {
        (Faction::Player, Faction::Villager) | (Faction::Villager, Faction::Player) => FactionStanding::Neutral,
        (Faction::Player, Faction::Goblin) | (Faction::Goblin, Faction::Player) => FactionStanding::KillOnSight,
        (Faction::Goblin, Faction::Villager) | (Faction::Villager, Faction::Goblin) => FactionStanding::KillOnSight,
        (Faction::Wildlife, Faction::Player) | (Faction::Wildlife, Faction::Villager) => FactionStanding::KillOnSight,
        (Faction::Player, Faction::Wildlife) | (Faction::Villager, Faction::Wildlife) => FactionStanding::Neutral,
        _ => FactionStanding::Neutral,
    }
}

pub fn apply_damage(ctx: &ReducerContext, target_id: u64, amount: f32) {
    let Some(mut hp) = ctx.db.health().entity_id().find(target_id) else {
        return;
    };

    hp.current = (hp.current - amount).max(0.0);

    if hp.current == 0.0 {
        if ctx.db.player_session().entity_id().find(target_id).is_some() {
            hp.current = hp.max;
            ctx.db.health().entity_id().update(hp);

            if let Some(mut transform) = ctx.db.transform().entity_id().find(target_id) {
                if let Some(mut inv) = ctx.db.inventory().entity_id().find(target_id) {
                    let mut salt = 1000u64;
                    for slot in &inv.slots {
                        if slot.count > 0 {
                            let corpse_id = ((ctx.timestamp.to_micros_since_unix_epoch() as u64) << 16)
                                ^ (target_id.wrapping_add(salt));
                            salt += 1;

                            ctx.db.harvestable_corpse().insert(crate::ai::HarvestableCorpse {
                                entity_id: corpse_id,
                                loot_item: slot.item_type.clone(),
                                amount: slot.count,
                            });

                            ctx.db.transform().insert(crate::movement::Transform {
                                entity_id: corpse_id,
                                x: transform.x,
                                y: transform.y,
                                z: transform.z,
                                chunk_x: transform.chunk_x,
                                chunk_z: transform.chunk_z,
                                last_processed_tick: 0,
                            });
                        }
                    }
                    inv.slots.clear();
                    ctx.db.inventory().entity_id().update(inv);
                }

                transform.x = 0.0;
                transform.z = 0.0;
                transform.y = crate::get_terrain_height(0.0, 0.0) + 1.05;
                ctx.db.transform().entity_id().update(transform);
            }

            log::debug!("Player {} died and was respawned at origin coordinates.", target_id);
        } else {
            ctx.db.health().entity_id().delete(target_id);
            ctx.db.transform().entity_id().delete(target_id);
            ctx.db.faction_component().entity_id().delete(target_id);
            ctx.db.npc_brain().entity_id().delete(target_id);
            ctx.db.pet_component().entity_id().delete(target_id);

            if ctx.db.peasant().entity_id().find(target_id).is_some() {
                ctx.db.peasant().entity_id().delete(target_id);
            }

            log::debug!("Entity {} reached 0 HP and was cleaned from active database tables.", target_id);
        }
    } else {
        ctx.db.health().entity_id().update(hp);
    }
}

// ----------------------------------------------------------------------------
// COMBAT REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn fire_weapon(
    ctx: &ReducerContext, 
    client_tick: u64, 
    origin_x: f32, origin_y: f32, origin_z: f32, 
    dir_x: f32, dir_y: f32, dir_z: f32
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session".to_string())?;

    let mut hit_entity = None;
    let mut closest_dist = f32::MAX;
    let mut hit_location = (0.0, 0.0, 0.0);

    let dir_len_sq = dir_x * dir_x + dir_y * dir_y + dir_z * dir_z;
    if dir_len_sq < 0.0001 {
        return Err("Invalid firing vector: near-zero magnitude".to_string());
    }
    let inv_len = 1.0 / dir_len_sq.sqrt();
    let (ndx, ndy, ndz) = (dir_x * inv_len, dir_y * inv_len, dir_z * inv_len);

    for history in ctx.db.hitbox_history().iter() {
        if history.entity_id == session.entity_id { continue; }
        if ctx.db.harvestable_corpse().entity_id().find(history.entity_id).is_some() { continue; }

        let closest_snapshot = history.snapshots.iter()
            .min_by_key(|s| (s.tick_id as i64 - client_tick as i64).abs());

        if let Some(snap) = closest_snapshot {
            let radius = 0.8_f32;
            let cx = snap.x; 
            let cy = snap.y + 1.0; 
            let cz = snap.z; 

            let ocx = origin_x - cx; 
            let ocy = origin_y - cy; 
            let ocz = origin_z - cz;

            let b = (ocx * ndx + ocy * ndy + ocz * ndz) * 2.0;
            let c = ocx * ocx + ocy * ocy + ocz * ocz - (radius * radius);
            let discriminant = b * b - 4.0 * c;

            if discriminant >= 0.0 {
                let dist = (-b - discriminant.sqrt()) / 2.0;
                if dist > 0.0 && dist < closest_dist {
                    closest_dist = dist;
                    hit_entity = Some(history.entity_id);
                    hit_location = (
                        origin_x + ndx * dist,
                        origin_y + ndy * dist,
                        origin_z + ndz * dist,
                    );
                }
            }
        }
    }

    if let Some(target_id) = hit_entity {
        apply_damage(ctx, target_id, 20.0);

        ctx.db.combat_event().insert(CombatEvent {
            id: 0,
            event_type: "HitPlayer".to_string(),
            x: hit_location.0,
            y: hit_location.1,
            z: hit_location.2,
        });

        log::debug!("Hit validated via Lag Compensation: Entity {} hit {}", session.entity_id, target_id);
    }

    Ok(())
}

#[reducer]
pub fn fire_bow(
    ctx: &ReducerContext,
    client_tick: u64,
    origin_x: f32, origin_y: f32, origin_z: f32,
    dir_x: f32, dir_y: f32, dir_z: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session".to_string())?;

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found".to_string())?;

    let has_bow = inv.slots.iter().any(|s| s.item_type == "Crude Bow" && s.count > 0);
    if !has_bow {
        return Err("You must have a Bow equipped to fire.".to_string());
    }

    let arrow_type = if crate::has_item(&inv, "Flint Arrow", 1) {
        "Flint Arrow"
    } else if crate::has_item(&inv, "Wood Arrow", 1) {
        "Wood Arrow"
    } else {
        return Err("No arrows remaining in inventory.".to_string());
    };

    crate::remove_item(&mut inv, arrow_type, 1);
    ctx.db.inventory().entity_id().update(inv);

    let arrow_damage = if arrow_type == "Flint Arrow" { 35.0 } else { 20.0 };

    let dir_len_sq = dir_x * dir_x + dir_y * dir_y + dir_z * dir_z;
    if dir_len_sq < 0.0001 {
        return Err("Invalid arrow direction vector.".to_string());
    }
    let inv_len = 1.0 / dir_len_sq.sqrt();
    let (ndx, ndy, ndz) = (dir_x * inv_len, dir_y * inv_len, dir_z * inv_len);

    let mut hit_entity = None;
    let mut closest_dist = 80.0_f32;
    let mut hit_location = (0.0, 0.0, 0.0);

    for history in ctx.db.hitbox_history().iter() {
        if history.entity_id == session.entity_id { continue; }
        if ctx.db.harvestable_corpse().entity_id().find(history.entity_id).is_some() { continue; }

        let closest_snapshot = history.snapshots.iter()
            .min_by_key(|s| (s.tick_id as i64 - client_tick as i64).abs());

        if let Some(snap) = closest_snapshot {
            let radius = 1.0_f32;
            let cx = snap.x; 
            let cy = snap.y + 1.0; 
            let cz = snap.z; 

            let ocx = origin_x - cx; 
            let ocy = origin_y - cy; 
            let ocz = origin_z - cz;

            let b = (ocx * ndx + ocy * ndy + ocz * ndz) * 2.0;
            let c = ocx * ocx + ocy * ocy + ocz * ocz - (radius * radius);
            let discriminant = b * b - 4.0 * c;

            if discriminant >= 0.0 {
                let dist = (-b - discriminant.sqrt()) / 2.0;
                if dist > 0.0 && dist < closest_dist {
                    closest_dist = dist;
                    hit_entity = Some(history.entity_id);
                    hit_location = (origin_x + ndx * dist, origin_y + ndy * dist, origin_z + ndz * dist);
                }
            }
        }
    }

    if let Some(target_id) = hit_entity {
        apply_damage(ctx, target_id, arrow_damage);
        ctx.db.combat_event().insert(CombatEvent {
            id: 0,
            event_type: "ArrowHit".to_string(),
            x: hit_location.0,
            y: hit_location.1,
            z: hit_location.2,
        });
        log::debug!("Entity {} shot Entity {} with {} for {} damage.", session.entity_id, target_id, arrow_type, arrow_damage);
    } else {
        // Check for structure collision
        for s in ctx.db.structure().iter() {
            let dx = s.x - origin_x;
            let dy = s.y - origin_y;
            let dz = s.z - origin_z;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist < 60.0 {
                let dot = (dx / dist) * ndx + (dy / dist) * ndy + (dz / dist) * ndz;
                if dot > 0.98 {
                    crate::building::damage_structure(ctx, s.structure_id, arrow_damage * 0.5);
                    ctx.db.combat_event().insert(CombatEvent {
                        id: 0,
                        event_type: "HitStructure".to_string(),
                        x: s.x,
                        y: s.y + 1.0,
                        z: s.z,
                    });
                    break;
                }
            }
        }
    }

    Ok(())
}