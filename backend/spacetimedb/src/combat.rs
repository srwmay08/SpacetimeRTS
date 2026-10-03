// ----------------------------------------------------------------------------
// COMBAT MODULE & AUTHORITATIVE RESOLUTION (SpacetimeDB v2.x / Rust 2024)
// ----------------------------------------------------------------------------
// Architectural Note: Implements the server-authoritative combat validation loop.
// Features a classless weapon system supporting 1H, 2H, polearms, brawling,
// runestaves, dual-wielding (dual melee, hybrid melee/ranged, dual ranged),
// and ballistic simulation for sniper rifles, shotguns, revolvers, bows, and spells.
// Hit registration matches against `HitboxHistory` snapshots using client tick IDs
// for responsive, anti-cheat verified lag compensation.

use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::player_session;
use crate::CombatEvent;
use crate::combat_event;
use crate::movement::transform;
use crate::inventory;
use crate::ai::{npc_brain, pet_component, peasant, harvestable_corpse};
use crate::building::structure;
use crate::voxel;
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
    HandCrossbowBolt,
    RevolverBullet,
    ShotgunPellet,
    SniperBullet,
    FireballBall,
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

/// Architectural Note: Classless player combat skill and weapon experience table.
/// Any player can train in any weapon type without class restrictions.
#[derive(Clone)]
#[table(accessor = weapon_skill, public)]
pub struct WeaponSkill {
    #[primary_key]
    pub entity_id: u64,
    pub generic_physical: u32,
    pub edged_xp: u32,
    pub pointed_xp: u32,
    pub blunt_xp: u32,
    pub two_handed_xp: u32,
    pub polearm_xp: u32,
    pub brawling_xp: u32,
    pub missile_xp: u32,
    pub firearm_xp: u32,
    pub runestaff_xp: u32,
}

/// Architectural Note: Equipment loadout table tracking MainHand and OffHand items.
/// Enforces physical constraints (2H occupies both hands; 1H can be dual-wielded).
#[derive(Clone)]
#[table(accessor = equipment_loadout, public)]
pub struct EquipmentLoadout {
    #[primary_key]
    pub entity_id: u64,
    pub main_hand: String,
    pub off_hand: String,
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

pub fn is_weapon_two_handed(weapon_name: &str) -> bool {
    matches!(
        weapon_name,
        "Greatsword" | "Maul" | "Halberd" | "Spear" | "Longbow" | "Shotgun" | "Sniper Rifle" | "Runestaff"
    )
}

pub fn apply_damage(ctx: &ReducerContext, target_id: u64, amount: f32) {
    let Some(mut hp) = ctx.db.health().entity_id().find(target_id) else {
        return;
    };

    hp.current = (hp.current - amount).max(0.0);

    if hp.current == 0.0 {
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

pub fn process_projectiles_tick(ctx: &ReducerContext, dt: f32) {
    let projectile_count = ctx.db.active_projectile().iter().count();
    if projectile_count == 0 {
        return;
    }
    
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

            if let Some((user_data, hx, hy, hz, _toi)) = crate::physics::cast_ray(
                prev_x, prev_y, prev_z,
                ndx, ndy, ndz,
                segment_len,
                proj.shooter_id,
            ) {
                let is_structure = (user_data >> 64) == 1;
                let target_id = (user_data & 0xFFFFFFFFFFFFFFFF) as u64;
                
                hit_detected = true;
                hit_point = (hx, hy, hz);
                
                if is_structure {
                    crate::building::damage_structure(ctx, target_id, proj.damage);
                } else {
                    hit_target_id = Some(target_id);
                }
            }

            if !hit_detected {
                let voxel_mat = voxel::get_voxel_at(ctx, proj.pos_x, proj.pos_y, proj.pos_z);
                if voxel_mat.is_solid() {
                    hit_detected = true;
                    hit_point = (proj.pos_x, proj.pos_y, proj.pos_z);
                }
            }
        }

        if hit_detected || proj.lifetime <= 0.0 {
            if hit_detected {
                if let Some(target_id) = hit_target_id {
                    apply_damage(ctx, target_id, proj.damage);
                }

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

                let event_type = match proj.kind {
                    ProjectileKind::CatapultRock | ProjectileKind::TrebuchetShell => "SiegeImpact".to_string(),
                    ProjectileKind::BallistaSpear => "BallistaImpact".to_string(),
                    ProjectileKind::FireballBall => "FireballExplosion".to_string(),
                    ProjectileKind::RevolverBullet => "BulletHit".to_string(),
                    ProjectileKind::SniperBullet => "SniperImpact".to_string(),
                    ProjectileKind::ShotgunPellet => "PelletHit".to_string(),
                    ProjectileKind::HandCrossbowBolt => "BoltHit".to_string(),
                    _ => "ProjectileHit".to_string(),
                };

                ctx.db.combat_event().insert(CombatEvent {
                    id: 0,
                    event_type,
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
// LOADOUT & WEAPON REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn equip_weapon(
    ctx: &ReducerContext,
    slot: String,
    weapon_name: String,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    if !crate::has_item(&inv, &weapon_name, 1) && weapon_name != "Unarmed" {
        return Err(format!("Weapon '{}' not in inventory.", weapon_name));
    }

    let is_two_handed = is_weapon_two_handed(&weapon_name);

    let mut loadout = ctx.db.equipment_loadout().entity_id().find(session.entity_id)
        .unwrap_or(EquipmentLoadout {
            entity_id: session.entity_id,
            main_hand: "None".to_string(),
            off_hand: "None".to_string(),
        });

    log::debug!("Player {} equipped {} in {}", session.entity_id, weapon_name, slot);

    if slot == "MainHand" {
        if is_two_handed && loadout.off_hand != "None" {
            return Err("Cannot equip two-handed weapon while off-hand is occupied.".to_string());
        }
        loadout.main_hand = weapon_name;
    } else if slot == "OffHand" {
        if is_weapon_two_handed(&loadout.main_hand) {
            return Err("Cannot equip off-hand when main-hand is two-handed.".to_string());
        }
        if is_two_handed {
            return Err("Cannot equip a two-handed weapon in off-hand.".to_string());
        }
        loadout.off_hand = weapon_name;
    } else {
        return Err("Invalid slot: Must be 'MainHand' or 'OffHand'.".to_string());
    }

    if ctx.db.equipment_loadout().entity_id().find(session.entity_id).is_some() {
        ctx.db.equipment_loadout().entity_id().update(loadout);
    } else {
        ctx.db.equipment_loadout().insert(loadout);
    }

    Ok(())
}

#[reducer]
pub fn unequip_weapon(
    ctx: &ReducerContext,
    slot: String,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let mut loadout = ctx.db.equipment_loadout().entity_id().find(session.entity_id)
        .ok_or_else(|| "Equipment loadout not found.".to_string())?;

    if slot == "MainHand" {
        loadout.main_hand = "None".to_string();
    } else if slot == "OffHand" {
        loadout.off_hand = "None".to_string();
    } else {
        return Err("Invalid slot: Must be 'MainHand' or 'OffHand'.".to_string());
    }

    ctx.db.equipment_loadout().entity_id().update(loadout);
    Ok(())
}

#[reducer]
pub fn fire_ranged_weapon(
    ctx: &ReducerContext,
    slot: String,
    client_tick: u64,
    origin_x: f32, origin_y: f32, origin_z: f32,
    dir_x: f32, dir_y: f32, dir_z: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let loadout = ctx.db.equipment_loadout().entity_id().find(session.entity_id)
        .ok_or_else(|| "No equipment loadout found.".to_string())?;

    let weapon_name = match slot.as_str() {
        "MainHand" => &loadout.main_hand,
        "OffHand" => &loadout.off_hand,
        _ => return Err("Invalid hand slot specified.".to_string()),
    };

    let dir_len_sq = dir_x * dir_x + dir_y * dir_y + dir_z * dir_z;
    if dir_len_sq < 0.0001 {
        return Err("Invalid firing vector: near-zero magnitude.".to_string());
    }
    let inv_len = 1.0 / dir_len_sq.sqrt();
    let (ndx, ndy, ndz) = (dir_x * inv_len, dir_y * inv_len, dir_z * inv_len);

    match weapon_name.as_str() {
        "Revolver" => {
            ctx.db.active_projectile().insert(ActiveProjectile {
                projectile_id: 0,
                shooter_id: session.entity_id,
                kind: ProjectileKind::RevolverBullet,
                pos_x: origin_x + ndx * 0.6,
                pos_y: origin_y + ndy * 0.6,
                pos_z: origin_z + ndz * 0.6,
                vel_x: ndx * 180.0,
                vel_y: ndy * 180.0,
                vel_z: ndz * 180.0,
                gravity: 2.0,
                drag: 0.0005,
                damage: 46.0,
                blast_radius: 0.0,
                start_tick: client_tick,
                lifetime: 2.0,
            });
            award_combat_xp(ctx, session.entity_id, "Firearm", 25);
        }
        "Hand Crossbow" => {
            ctx.db.active_projectile().insert(ActiveProjectile {
                projectile_id: 0,
                shooter_id: session.entity_id,
                kind: ProjectileKind::HandCrossbowBolt,
                pos_x: origin_x + ndx * 0.6,
                pos_y: origin_y + ndy * 0.6,
                pos_z: origin_z + ndz * 0.6,
                vel_x: ndx * 42.0,
                vel_y: ndy * 42.0,
                vel_z: ndz * 42.0,
                gravity: 5.5,
                drag: 0.001,
                damage: 28.0,
                blast_radius: 0.0,
                start_tick: client_tick,
                lifetime: 3.0,
            });
            award_combat_xp(ctx, session.entity_id, "Missile", 20);
        }
        "Shotgun" => {
            let pellet_spread = [
                (0.0, 0.0), (-0.03, 0.02), (0.03, 0.02), (-0.02, -0.03), (0.02, -0.03),
                (0.05, 0.0), (-0.05, 0.0), (0.0, 0.04), (0.0, -0.04), (0.04, 0.04),
                (-0.04, -0.04), (0.03, -0.02)
            ];
            for (sx, sy) in pellet_spread {
                let px = ndx + sx;
                let py = ndy + sy;
                let pz = ndz;
                let plen = (px * px + py * py + pz * pz).sqrt();
                ctx.db.active_projectile().insert(ActiveProjectile {
                    projectile_id: 0,
                    shooter_id: session.entity_id,
                    kind: ProjectileKind::ShotgunPellet,
                    pos_x: origin_x + px * 0.5,
                    pos_y: origin_y + py * 0.5,
                    pos_z: origin_z + pz * 0.5,
                    vel_x: (px / plen) * 120.0,
                    vel_y: (py / plen) * 120.0,
                    vel_z: (pz / plen) * 120.0,
                    gravity: 3.5,
                    drag: 0.004,
                    damage: 18.0,
                    blast_radius: 0.0,
                    start_tick: client_tick,
                    lifetime: 1.0,
                });
            }
            award_combat_xp(ctx, session.entity_id, "Firearm", 30);
        }
        "Sniper Rifle" => {
            ctx.db.active_projectile().insert(ActiveProjectile {
                projectile_id: 0,
                shooter_id: session.entity_id,
                kind: ProjectileKind::SniperBullet,
                pos_x: origin_x + ndx * 0.9,
                pos_y: origin_y + ndy * 0.9,
                pos_z: origin_z + ndz * 0.9,
                vel_x: ndx * 820.0,
                vel_y: ndy * 820.0,
                vel_z: ndz * 820.0,
                gravity: 0.8,
                drag: 0.0001,
                damage: 160.0,
                blast_radius: 0.0,
                start_tick: client_tick,
                lifetime: 4.0,
            });
            award_combat_xp(ctx, session.entity_id, "Firearm", 45);
        }
        "Runestaff" => {
            ctx.db.active_projectile().insert(ActiveProjectile {
                projectile_id: 0,
                shooter_id: session.entity_id,
                kind: ProjectileKind::MagicMissile,
                pos_x: origin_x + ndx * 0.8,
                pos_y: origin_y + ndy * 0.8,
                pos_z: origin_z + ndz * 0.8,
                vel_x: ndx * 55.0,
                vel_y: ndy * 55.0,
                vel_z: ndz * 55.0,
                gravity: 0.0,
                drag: 0.0,
                damage: 35.0,
                blast_radius: 0.0,
                start_tick: client_tick,
                lifetime: 4.0,
            });
            award_combat_xp(ctx, session.entity_id, "Runestaff", 25);
        }
        "Crude Bow" | "Longbow" => {
            ctx.db.active_projectile().insert(ActiveProjectile {
                projectile_id: 0,
                shooter_id: session.entity_id,
                kind: ProjectileKind::Arrow,
                pos_x: origin_x + ndx * 0.8,
                pos_y: origin_y + ndy * 0.8,
                pos_z: origin_z + ndz * 0.8,
                vel_x: ndx * 50.0,
                vel_y: ndy * 50.0,
                vel_z: ndz * 50.0,
                gravity: 4.8,
                drag: 0.001,
                damage: 32.0,
                blast_radius: 0.0,
                start_tick: client_tick,
                lifetime: 5.0,
            });
            award_combat_xp(ctx, session.entity_id, "Missile", 20);
        }
        _ => return Err(format!("Weapon '{}' in {} cannot fire ranged projectiles.", weapon_name, slot)),
    }

    Ok(())
}

fn award_combat_xp(ctx: &ReducerContext, entity_id: u64, category: &str, amount: u32) {
    if let Some(mut skill) = ctx.db.weapon_skill().entity_id().find(entity_id) {
        match category {
            "Edged" => skill.edged_xp = skill.edged_xp.saturating_add(amount),
            "Pointed" => skill.pointed_xp = skill.pointed_xp.saturating_add(amount),
            "Blunt" => skill.blunt_xp = skill.blunt_xp.saturating_add(amount),
            "TwoHanded" => skill.two_handed_xp = skill.two_handed_xp.saturating_add(amount),
            "Polearm" => skill.polearm_xp = skill.polearm_xp.saturating_add(amount),
            "Brawling" => skill.brawling_xp = skill.brawling_xp.saturating_add(amount),
            "Missile" => skill.missile_xp = skill.missile_xp.saturating_add(amount),
            "Firearm" => skill.firearm_xp = skill.firearm_xp.saturating_add(amount),
            "Runestaff" => skill.runestaff_xp = skill.runestaff_xp.saturating_add(amount),
            _ => {}
        }
        skill.generic_physical = (skill.generic_physical + (amount / 10)).min(100);
        ctx.db.weapon_skill().entity_id().update(skill);
    }
}

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
    let mut hit_location = (0.0, 0.0, 0.0);

    let dir_len_sq = dir_x * dir_x + dir_y * dir_y + dir_z * dir_z;
    if dir_len_sq < 0.0001 {
        return Err("Invalid firing vector: near-zero magnitude.".to_string());
    }
    let inv_len = 1.0 / dir_len_sq.sqrt();
    let (ndx, ndy, ndz) = (dir_x * inv_len, dir_y * inv_len, dir_z * inv_len);

    let mut colliders = rapier3d::prelude::ColliderSet::new();
    for history in ctx.db.hitbox_history().iter() {
        if history.entity_id == session.entity_id { continue; }
        if ctx.db.harvestable_corpse().entity_id().find(history.entity_id).is_some() { continue; }

        if let Some(snap) = history.snapshots.iter().min_by_key(|s| (s.tick_id as i64 - client_tick as i64).abs()) {
            let col = rapier3d::prelude::ColliderBuilder::capsule_y(0.5, 0.4)
                .translation(rapier3d::prelude::Vector::new(snap.x, snap.y + 0.5, snap.z))
                .user_data(history.entity_id as u128)
                .build();
            colliders.insert(col);
        }
    }
    
    for s in ctx.db.structure().iter() {
        let col = rapier3d::prelude::ColliderBuilder::cuboid(1.25, 1.25, 1.25)
            .translation(rapier3d::prelude::Vector::new(s.x, s.y, s.z))
            .user_data((s.structure_id as u128) | (1 << 64))
            .build();
        colliders.insert(col);
    }

    let mut query_pipeline = rapier3d::prelude::QueryPipeline::new();
    query_pipeline.update(&colliders);

    let ray = rapier3d::prelude::Ray::new(
        rapier3d::prelude::Point::new(origin_x, origin_y, origin_z),
        rapier3d::prelude::Vector::new(ndx, ndy, ndz)
    );

    let rigid_bodies = rapier3d::prelude::RigidBodySet::new();
    if let Some((handle, toi)) = query_pipeline.cast_ray(
        &rigid_bodies, &colliders, &ray, 300.0, true, rapier3d::prelude::QueryFilter::default()
    ) {
        let user_data = colliders[handle].user_data;
        let is_structure = (user_data >> 64) == 1;
        let target_id = (user_data & 0xFFFFFFFFFFFFFFFF) as u64;
        let hit_pt = ray.point_at(toi);
        
        hit_location = (hit_pt.x, hit_pt.y, hit_pt.z);
        
        if is_structure {
            crate::building::damage_structure(ctx, target_id, 20.0);
        } else {
            hit_entity = Some(target_id);
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

        award_combat_xp(ctx, session.entity_id, "Edged", 15);
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

    let has_bow = inv.slots.iter().any(|s| (s.item_type == "Crude Bow" || s.item_type == "Longbow") && s.count > 0);
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

    award_combat_xp(ctx, session.entity_id, "Missile", 20);
    log::debug!("Player {} fired authoritative arrow projectile.", session.entity_id);
    Ok(())
}