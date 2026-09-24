// ----------------------------------------------------------------------------
// COMBAT MODULE & AUTHORITATIVE RESOLUTION (SpacetimeDB v2.x / Rust 2024)
// ----------------------------------------------------------------------------
// Architectural Note: Implements the server-authoritative combat validation loop.
// Projectiles are authoritatively tracked through `ActiveProjectile` records with
// ballistic simulation (gravity, drag, ray swept checks). Hit registration matches
// against `HitboxHistory` snapshots using client tick IDs to provide responsive,
// anti-cheat verified lag compensation. Player avatars persist across disconnections,
// remaining vulnerable to combat damage and dropping corpse containers upon death.

use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::player_session;
use crate::CombatEvent;
use crate::combat_event;
use crate::movement::transform;
use crate::inventory;
use crate::ai::{npc_brain, pet_component, peasant, harvestable_corpse};
use crate::building::structure;
use crate::voxel;

// Architectural Note: Bringing the `player` accessor trait into scope is strictly
// required by SpacetimeDB v2.x so `ctx.db.player()` can be called during damage
// resolution to check avatar persistence across disconnect states without E0599.
use crate::player;

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

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileKind {
    Arrow,
    CatapultRock,
    TrebuchetShell,
    BallistaSpear,
    MagicMissile,
}

/// Architectural Note: Authoritative ballistic trajectory simulation table.
/// Ticked during the high-frequency server simulation loop to calculate swept
/// collision against voxels, dynamic hitboxes, and modular structure colliders.
#[derive(Clone)]
#[table(accessor = active_projectile, public)]
pub struct ActiveProjectile {
    #[primary_key] #[auto_inc]
    pub projectile_id: u64,
    pub shooter_id: u64,
    pub kind: ProjectileKind,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub vel_x: f32,
    pub vel_y: f32,
    pub vel_z: f32,
    pub gravity: f32,
    pub drag: f32,
    pub damage: f32,
    pub blast_radius: f32,
    pub start_tick: u64,
    pub lifetime: f32,
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

/// Architectural Note: Applies authoritative combat damage to any entity.
/// Disconnected/offline players retain their physical avatars in the world.
/// If an offline player dies, their inventory is spilled into a `HarvestableCorpse`,
/// and their avatar is reset to origin while maintaining disconnected state.
pub fn apply_damage(ctx: &ReducerContext, target_id: u64, amount: f32) {
    let Some(mut hp) = ctx.db.health().entity_id().find(target_id) else {
        return;
    };

    hp.current = (hp.current - amount).max(0.0);

    if hp.current == 0.0 {
        // Query `Player` table directly to verify player identity regardless of online/offline status
        if let Some(player) = ctx.db.player().entity_id().find(target_id) {
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

                // Relocate avatar to safe respawn point; remains persistent if offline
                transform.x = 0.0;
                transform.z = 0.0;
                transform.y = crate::get_terrain_height(0.0, 0.0) + 1.05;
                ctx.db.transform().entity_id().update(transform);
            }

            log::debug!(
                "Player {} (is_online: {}) died from combat damage and was respawned at origin coordinates.",
                target_id, player.is_online
            );
        } else {
            // Target is an NPC, Pet, or Peasant
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
// PROJECTILE BALLISTICS TICK SYSTEM
// ----------------------------------------------------------------------------

/// Architectural Note: Server-Authoritative Ballistic Simulation Loop.
/// Executed during the high-frequency server tick (16ms) to integrate trajectory,
/// execute swept raycasts against entities, and mutate the voxel grid on impact.
pub fn process_projectiles_tick(ctx: &ReducerContext, dt: f32) {
    let projectiles: Vec<ActiveProjectile> = ctx.db.active_projectile().iter().collect();

    for mut proj in projectiles {
        let prev_x = proj.pos_x;
        let prev_y = proj.pos_y;
        let prev_z = proj.pos_z;

        // Apply drag and gravity
        proj.vel_x *= 1.0 - (proj.drag * dt);
        proj.vel_z *= 1.0 - (proj.drag * dt);
        proj.vel_y = (proj.vel_y - (proj.gravity * dt)) * (1.0 - (proj.drag * dt));

        proj.pos_x += proj.vel_x * dt;
        proj.pos_y += proj.vel_y * dt;
        proj.pos_z += proj.vel_z * dt;
        proj.lifetime -= dt;

        let segment_dx = proj.pos_x - prev_x;
        let segment_dy = proj.pos_y - prev_y;
        let segment_dz = proj.pos_z - prev_z;
        let segment_len = (segment_dx * segment_dx + segment_dy * segment_dy + segment_dz * segment_dz).sqrt();

        let mut hit_detected = false;
        let mut hit_point = (proj.pos_x, proj.pos_y, proj.pos_z);
        let mut hit_target_id = None;

        if segment_len > 0.001 {
            let ndx = segment_dx / segment_len;
            let ndy = segment_dy / segment_len;
            let ndz = segment_dz / segment_len;

            // 1. Ray sweep against entity hitboxes
            for history in ctx.db.hitbox_history().iter() {
                if history.entity_id == proj.shooter_id {
                    continue;
                }
                if ctx.db.harvestable_corpse().entity_id().find(history.entity_id).is_some() {
                    continue;
                }

                if let Some(snap) = history.snapshots.last() {
                    let radius = 0.9_f32;
                    let ocx = prev_x - snap.x;
                    let ocy = prev_y - (snap.y + 1.0);
                    let ocz = prev_z - snap.z;

                    let b = (ocx * ndx + ocy * ndy + ocz * ndz) * 2.0;
                    let c = ocx * ocx + ocy * ocy + ocz * ocz - (radius * radius);
                    let discriminant = b * b - 4.0 * c;

                    if discriminant >= 0.0 {
                        let dist = (-b - discriminant.sqrt()) / 2.0;
                        if dist > 0.0 && dist <= segment_len {
                            hit_detected = true;
                            hit_point = (prev_x + ndx * dist, prev_y + ndy * dist, prev_z + ndz * dist);
                            hit_target_id = Some(history.entity_id);
                            break;
                        }
                    }
                }
            }

            // 2. Check collision with destructible voxel terrain
            if !hit_detected {
                let voxel_mat = voxel::get_voxel_at(ctx, proj.pos_x, proj.pos_y, proj.pos_z);
                if voxel_mat.is_solid() {
                    hit_detected = true;
                    hit_point = (proj.pos_x, proj.pos_y, proj.pos_z);
                }
            }

            // 3. Check collision with modular structures
            if !hit_detected {
                for s in ctx.db.structure().iter() {
                    let dx = s.x - proj.pos_x;
                    let dy = s.y - proj.pos_y;
                    let dz = s.z - proj.pos_z;
                    if (dx * dx + dy * dy + dz * dz) <= 4.0 {
                        hit_detected = true;
                        hit_point = (s.x, s.y, s.z);
                        crate::building::damage_structure(ctx, s.structure_id, proj.damage);
                        break;
                    }
                }
            }
        }

        if hit_detected || proj.lifetime <= 0.0 {
            if hit_detected {
                if let Some(target_id) = hit_target_id {
                    apply_damage(ctx, target_id, proj.damage);
                }

                // If explosive or siege projectile, mutate the voxel grid
                if proj.blast_radius > 0.0 {
                    voxel::mutate_voxel_sphere(
                        ctx,
                        hit_point.0,
                        hit_point.1,
                        hit_point.2,
                        proj.blast_radius,
                        proj.damage,
                    );
                }

                ctx.db.combat_event().insert(CombatEvent {
                    id: 0,
                    event_type: match proj.kind {
                        ProjectileKind::CatapultRock | ProjectileKind::TrebuchetShell => "SiegeImpact".to_string(),
                        ProjectileKind::BallistaSpear => "BallistaImpact".to_string(),
                        _ => "ProjectileHit".to_string(),
                    },
                    x: hit_point.0,
                    y: hit_point.1,
                    z: hit_point.2,
                });
            }

            ctx.db.active_projectile().projectile_id().delete(proj.projectile_id);
        } else {
            ctx.db.active_projectile().projectile_id().update(proj);
        }
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
    dir_x: f32, dir_y: f32, dir_z: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let mut hit_entity = None;
    let mut closest_dist = f32::MAX;
    let mut hit_location = (0.0, 0.0, 0.0);

    let dir_len_sq = dir_x * dir_x + dir_y * dir_y + dir_z * dir_z;
    if dir_len_sq < 0.0001 {
        return Err("Invalid firing vector: near-zero magnitude.".to_string());
    }
    let inv_len = 1.0 / dir_len_sq.sqrt();
    let (ndx, ndy, ndz) = (dir_x * inv_len, dir_y * inv_len, dir_z * inv_len);

    for history in ctx.db.hitbox_history().iter() {
        if history.entity_id == session.entity_id {
            continue;
        }
        if ctx.db.harvestable_corpse().entity_id().find(history.entity_id).is_some() {
            continue;
        }

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
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

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

    let speed = 48.0;

    // Spawn authoritative ballistic projectile
    ctx.db.active_projectile().insert(ActiveProjectile {
        projectile_id: 0,
        shooter_id: session.entity_id,
        kind: ProjectileKind::Arrow,
        pos_x: origin_x + ndx * 0.8,
        pos_y: origin_y + ndy * 0.8,
        pos_z: origin_z + ndz * 0.8,
        vel_x: ndx * speed,
        vel_y: ndy * speed,
        vel_z: ndz * speed,
        gravity: 4.8,
        drag: 0.001,
        damage: arrow_damage,
        blast_radius: 0.0,
        start_tick: client_tick,
        lifetime: 5.0,
    });

    log::debug!("Player {} fired authoritative arrow projectile.", session.entity_id);
    Ok(())
}