// ============================================================================
// File: backend/spacetimedb/src/harvesting.rs
// ============================================================================
// Resource gathering, terrain height sampling, world node spawning, and tool swing reducers.

use spacetimedb::{table, reducer, ReducerContext, Table, ScheduleAt};
use std::time::Duration;
use noise::{NoiseFn, Perlin};
use spacetime_rts_logic::{ItemKind, ResourceNodeType};

use crate::movement::{player_session, transform};
use crate::combat::{health, faction_component};
use crate::ai::{harvestable_corpse, npc_brain};
use crate::player_inventory::{inventory, add_item, remove_item};
use crate::{CombatEvent, RespawnBushTimer, global_state, respawn_bush_timer, combat_event};
use crate::hazard::node_facing;

#[table(accessor = resource_node, public)]
#[derive(Clone)]
pub struct ResourceNode {
    #[primary_key] #[auto_inc] pub node_id: u64,
    pub node_type: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    #[index(btree)]
    pub chunk_x: i32,
    #[index(btree)]
    pub chunk_z: i32,
    pub health: u32,
    pub scale: f32,
    pub required_tool: String,
}

// ----------------------------------------------------------------------------
// PROGRESSION & HARVESTING REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn consume_item(ctx: &ReducerContext, item_name: String) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;
    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    if !remove_item(&mut inv, &item_name, 1) {
        return Err(format!("You do not have any {}.", item_name));
    }

    if item_name == "Berry" || item_name == "Honey" || item_name == "Cooked Meat" {
        if let Some(mut hp) = ctx.db.health().entity_id().find(session.entity_id) {
            let heal_amount = match item_name.as_str() {
                "Berry" => 10.0,
                "Honey" => 25.0,
                "Cooked Meat" => 40.0,
                _ => 0.0,
            };
            hp.current = (hp.current + heal_amount).min(hp.max);
            ctx.db.health().entity_id().update(hp);
            log::debug!("Player {} consumed {} for {} HP.", session.entity_id, item_name, heal_amount);
        }
    } else {
        return Err("Item is not consumable.".to_string());
    }

    ctx.db.inventory().entity_id().update(inv);
    Ok(())
}

#[reducer]
pub fn interact_node(ctx: &ReducerContext, node_id: u64) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active player session.".to_string())?;

    let mut inventory = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found for entity.".to_string())?;

    let mut node = ctx.db.resource_node().node_id().find(node_id)
        .ok_or_else(|| "Node not found.".to_string())?;

    let node_kind = ResourceNodeType::from_str(&node.node_type)
        .ok_or_else(|| "Entity not interactable.".to_string())?;

    match node_kind {
        ResourceNodeType::Bush => {
            if node.health == 0 {
                return Err("Berries depleted.".into());
            }
            let (drop_item, count) = node_kind.default_drop();
            add_item(&mut inventory, drop_item.as_str(), count);
            ctx.db.inventory().entity_id().update(inventory);

            node.health = 0;
            ctx.db.resource_node().node_id().update(node);

            let has_timer = ctx.db.respawn_bush_timer().iter().any(|t| t.node_id == node_id);
            if !has_timer {
                ctx.db.respawn_bush_timer().insert(RespawnBushTimer {
                    scheduled_id: 0,
                    scheduled_at: ScheduleAt::Interval(Duration::from_secs(60).into()),
                    node_id,
                });
            }
        }
        ResourceNodeType::Branch | ResourceNodeType::Flint | ResourceNodeType::LooseStone => {
            let (drop_item, count) = node_kind.default_drop();
            add_item(&mut inventory, drop_item.as_str(), count);
            ctx.db.inventory().entity_id().update(inventory);
            ctx.db.resource_node().node_id().delete(node_id);
        }
        _ => return Err("Entity not interactable.".into()),
    }

    Ok(())
}

#[reducer]
pub fn respawn_bush_tick(ctx: &ReducerContext, timer: RespawnBushTimer) {
    if let Some(mut node) = ctx.db.resource_node().node_id().find(timer.node_id) {
        node.health = 1;
        ctx.db.resource_node().node_id().update(node);
    }
}

#[reducer]
pub fn swing_tool(ctx: &ReducerContext, px: f32, py: f32, pz: f32, dx: f32, dy: f32, dz: f32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active player session.".to_string())?;

    let mut inventory = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory record missing for player.".to_string())?;

    let mut hit_node = None;
    let mut hit_entity = None;
    let mut min_dist = 12.0_f32;

    for node in ctx.db.resource_node().iter() {
        let dist = ((node.x - px).powi(2) + (node.y - py).powi(2) + (node.z - pz).powi(2)).sqrt();
        if dist < min_dist {
            let dot = ((node.x - px) / dist) * dx + ((node.y - py) / dist) * dy + ((node.z - pz) / dist) * dz;
            if dot > 0.5 {
                min_dist = dist;
                hit_node = Some(node);
            }
        }
    }

    for t in ctx.db.transform().iter() {
        if t.entity_id == session.entity_id { continue; }
        if ctx.db.health().entity_id().find(t.entity_id).is_none() { continue; }
        let dist = ((t.x - px).powi(2) + (t.y - py).powi(2) + (t.z - pz).powi(2)).sqrt();
        if dist < min_dist {
            let dot = ((t.x - px) / dist) * dx + ((t.y - py) / dist) * dy + ((t.z - pz) / dist) * dz;
            if dot > 0.5 {
                min_dist = dist;
                hit_entity = Some(t);
                hit_node = None;
            }
        }
    }

    if let Some(target) = hit_entity {
        if let Some(corpse) = ctx.db.harvestable_corpse().entity_id().find(target.entity_id) {
            ctx.db.combat_event().insert(CombatEvent {
                id: 0,
                event_type: "HitPlayer".into(),
                x: target.x,
                y: target.y + 0.5,
                z: target.z,
            });

            if let Some(mut hp) = ctx.db.health().entity_id().find(target.entity_id) {
                if hp.current > 1.0 {
                    hp.current -= 1.0;
                    ctx.db.health().entity_id().update(hp);
                } else {
                    add_item(&mut inventory, &corpse.loot_item, corpse.amount);
                    ctx.db.inventory().entity_id().update(inventory);
                    ctx.db.health().entity_id().delete(target.entity_id);
                    ctx.db.transform().entity_id().delete(target.entity_id);
                    ctx.db.faction_component().entity_id().delete(target.entity_id);
                    if ctx.db.npc_brain().entity_id().find(target.entity_id).is_some() {
                        ctx.db.npc_brain().entity_id().delete(target.entity_id);
                    }
                    ctx.db.harvestable_corpse().entity_id().delete(target.entity_id);
                }
            } else {
                add_item(&mut inventory, &corpse.loot_item, corpse.amount);
                ctx.db.inventory().entity_id().update(inventory);
                ctx.db.transform().entity_id().delete(target.entity_id);
                ctx.db.faction_component().entity_id().delete(target.entity_id);
                if ctx.db.npc_brain().entity_id().find(target.entity_id).is_some() {
                    ctx.db.npc_brain().entity_id().delete(target.entity_id);
                }
                ctx.db.harvestable_corpse().entity_id().delete(target.entity_id);
            }
            return Ok(());
        }

        crate::combat::apply_damage(ctx, target.entity_id, 20.0);
        ctx.db.combat_event().insert(CombatEvent {
            id: 0,
            event_type: "HitPlayer".into(),
            x: target.x,
            y: target.y + 1.0,
            z: target.z,
        });
        return Ok(());
    }

    // Check 3D DDA raymarch against authoritative voxel grid up to 4.5m melee reach
    let has_pickaxe = inventory.slots.iter().any(|s| s.item_type == "Pickaxe" && s.count > 0);
    let voxel_hit = crate::voxel::dda_raymarch_voxel(ctx, px, py, pz, dx, dy, dz, 4.5);

    // Determine whether voxel terrain or discrete resource node is closer
    let should_hit_voxel = match (hit_node.as_ref(), voxel_hit.as_ref()) {
        (None, Some(_)) => true,
        (Some(node), Some(v_hit)) => {
            let node_dist_sq = (node.x - px).powi(2) + (node.y - py).powi(2) + (node.z - pz).powi(2);
            v_hit.distance * v_hit.distance < node_dist_sq
        }
        _ => false,
    };

    if should_hit_voxel {
        if let Some(v_hit) = voxel_hit {
            let wx = (v_hit.vx as f32 + 0.5) * crate::voxel::VOXEL_SIZE;
            let wy = (v_hit.vy as f32 + 0.5) * crate::voxel::VOXEL_SIZE;
            let wz = (v_hit.vz as f32 + 0.5) * crate::voxel::VOXEL_SIZE;

            if v_hit.material == crate::voxel::VoxelMaterial::Bedrock {
                ctx.db.combat_event().insert(CombatEvent {
                    id: 0,
                    event_type: "DeflectBedrock".into(),
                    x: wx,
                    y: wy,
                    z: wz,
                });
                return Ok(());
            }

            let is_hard_rock = matches!(
                v_hit.material,
                crate::voxel::VoxelMaterial::Stone
                    | crate::voxel::VoxelMaterial::IronOre
                    | crate::voxel::VoxelMaterial::Ruby
            );
            if !has_pickaxe && is_hard_rock {
                return Ok(());
            }

            let tool_dmg = if has_pickaxe { 160.0 } else { 45.0 };
            match crate::voxel::mine_rock_chunk(ctx, v_hit.vx, v_hit.vy, v_hit.vz, dx, dy, dz, tool_dmg) {
                Ok(crate::voxel::MineChunkOutcome::Cracked { hits_remaining: _, primary_mat }) => {
                    let event_type = match primary_mat {
                        crate::voxel::VoxelMaterial::IronOre | crate::voxel::VoxelMaterial::Ruby => "CrackOre".into(),
                        _ => "CrackRock".into(),
                    };
                    ctx.db.combat_event().insert(CombatEvent {
                        id: 0,
                        event_type,
                        x: wx,
                        y: wy,
                        z: wz,
                    });
                }
                Ok(crate::voxel::MineChunkOutcome::Shattered { primary_mat, voxels_excavated: _ }) => {
                    let event_type = match primary_mat {
                        crate::voxel::VoxelMaterial::IronOre => "HitIronOre".into(),
                        crate::voxel::VoxelMaterial::Ruby => "HitRuby".into(),
                        crate::voxel::VoxelMaterial::CollapsedRubble => "HitRubble".into(),
                        _ => "HitRock".into(),
                    };
                    ctx.db.combat_event().insert(CombatEvent {
                        id: 0,
                        event_type,
                        x: wx,
                        y: wy,
                        z: wz,
                    });

                    let (drop_item, count) = match primary_mat {
                        crate::voxel::VoxelMaterial::IronOre => ("IronOre", 3),
                        crate::voxel::VoxelMaterial::Ruby => ("Ruby", 2),
                        crate::voxel::VoxelMaterial::CollapsedRubble => ("LooseStone", 3),
                        crate::voxel::VoxelMaterial::Stone => ("Stone", 3),
                        crate::voxel::VoxelMaterial::Dirt | crate::voxel::VoxelMaterial::Sand => ("LooseStone", 2),
                        _ => ("Stone", 2),
                    };

                    add_item(&mut inventory, drop_item, count);

                    // Collapsed rubble buried loot payout!
                    if primary_mat == crate::voxel::VoxelMaterial::CollapsedRubble {
                        let mut loot_seed = ctx.timestamp.to_micros_since_unix_epoch() as u64 ^ (v_hit.vx as u64).wrapping_mul(31);
                        let loot_roll = prng(&mut loot_seed);
                        if loot_roll < 0.35 {
                            add_item(&mut inventory, "Ruby", 1);
                        } else if loot_roll < 0.70 {
                            add_item(&mut inventory, "IronOre", 2);
                        }
                    }

                    ctx.db.inventory().entity_id().update(inventory);
                }
                Err(_) => {}
            }
            return Ok(());
        }
    }

    if let Some(node) = hit_node {
        let node_kind = ResourceNodeType::from_str(&node.node_type);
        if let Some(tool_req) = node_kind.and_then(|k| k.required_tool()) {
            let has_tool = inventory.slots.iter().any(|s| {
                ItemKind::from_name(&s.item_type).map_or(false, |k| tool_req.is_satisfied_by(k)) && s.count > 0
            });
            if !has_tool {
                return Ok(());
            }
        }

        let event_type = format!("Hit{}", node.node_type);
        ctx.db.combat_event().insert(CombatEvent { id: 0, event_type, x: node.x, y: node.y + 1.0, z: node.z });

        if node.health > 1 {
            let mut updated = node.clone();
            updated.health = updated.health.saturating_sub(1);
            ctx.db.resource_node().node_id().update(updated);
        } else {
            ctx.db.resource_node().node_id().delete(node.node_id);
            // Architectural Note: Ruins (FallenLog / Rubble) may carry a facing row; remove it with the node.
            ctx.db.node_facing().node_id().delete(node.node_id);

            // Architectural Note: Valheim-style felling. A standing Tree does not pay out
            // instantly: it topples away from the logger as a server-scheduled fall hazard
            // (lethal to anything under the trunk at impact) and leaves a harvestable
            // FallenLog ruin. The wood is collected from that log (see hazard.rs).
            if node_kind == Some(ResourceNodeType::Tree) {
                crate::hazard::fell_tree(ctx, &node, px, pz);
                return Ok(());
            }

            let (drop_item, base_count) = node_kind.map(|k| k.default_drop()).unwrap_or((ItemKind::Wood, 1));
            let amount = (base_count as f32 * node.scale).ceil() as u32;

            add_item(&mut inventory, drop_item.as_str(), amount);

            // Collapsed rubble node unearths buried treasure
            if node.node_type == "CollapsedRubble" {
                let mut seed = ctx.timestamp.to_micros_since_unix_epoch() as u64 ^ node.node_id;
                let roll = prng(&mut seed);
                if roll < 0.35 {
                    add_item(&mut inventory, "Ruby", 1);
                } else if roll < 0.70 {
                    add_item(&mut inventory, "IronOre", 2);
                }
            }

            // Resin is tapped from felled timber; it moved from the standing Tree to its FallenLog ruin.
            if node.node_type == "FallenLog" && inventory.slots.iter().any(|s| s.item_type == "Stone Axe" && s.count > 0) {
                add_item(&mut inventory, "Resin", 1);
            }
            ctx.db.inventory().entity_id().update(inventory);
        }
    }

    Ok(())
}

// ----------------------------------------------------------------------------
// WORLD GENERATION & RESOURCE SPAWNING
// ----------------------------------------------------------------------------

pub fn spawn_world_resource_nodes(ctx: &ReducerContext) {
    let existing: Vec<u64> = ctx.db.resource_node().iter().map(|n| n.node_id).collect();
    for id in existing {
        ctx.db.resource_node().node_id().delete(id);
    }

    let seed_base = ctx.db.global_state().id().find(0).map(|s| s.world_seed).unwrap_or(42);
    let mut seed = (seed_base as u64) ^ 0x9E37_79B9_7F4A_7C15;
    let mut total_spawned = 0usize;

    // Fast 2D spatial grid for instantaneous overlap testing at high density
    let cell_size = 10.0f32;
    let grid_dim = 42usize; // -210 to +210 on X and Z
    let mut grid: Vec<Vec<(f32, f32, f32)>> = vec![Vec::with_capacity(32); grid_dim * grid_dim];

    let check_overlap = |x: f32, z: f32, spacing: f32, grid: &[Vec<(f32, f32, f32)>]| -> bool {
        let gx = ((x + 210.0) / cell_size).floor() as i32;
        let gz = ((z + 210.0) / cell_size).floor() as i32;
        for ix in (gx - 1).max(0)..=(gx + 1).min(grid_dim as i32 - 1) {
            for iz in (gz - 1).max(0)..=(gz + 1).min(grid_dim as i32 - 1) {
                let idx = (ix as usize) * grid_dim + (iz as usize);
                for &(px, pz, ps) in &grid[idx] {
                    let min_d = spacing.max(ps);
                    if (px - x) * (px - x) + (pz - z) * (pz - z) < min_d * min_d {
                        return true;
                    }
                }
            }
        }
        false
    };

    let insert_spatial = |x: f32, z: f32, spacing: f32, grid: &mut [Vec<(f32, f32, f32)>]| {
        let gx = ((x + 210.0) / cell_size).floor() as i32;
        let gz = ((z + 210.0) / cell_size).floor() as i32;
        if gx >= 0 && gx < grid_dim as i32 && gz >= 0 && gz < grid_dim as i32 {
            grid[(gx as usize) * grid_dim + (gz as usize)].push((x, z, spacing));
        }
    };

    // 1. Generate 32-44 forest centers across the world map
    let num_forests = 32 + (prng(&mut seed) * 12.0) as usize;
    let mut forest_centers: Vec<(f32, f32, f32, &'static str)> = Vec::with_capacity(num_forests);

    for _ in 0..num_forests {
        let fx = (prng(&mut seed) * 360.0) - 180.0;
        let fz = (prng(&mut seed) * 360.0) - 180.0;
        let fr = 24.0 + prng(&mut seed) * 36.0; // 24-60m radius
        let fy = get_terrain_height(fx, fz);
        let biome = get_biome(fy);
        let species = match biome {
            Biome::Lowland => if prng(&mut seed) < 0.60 { "Oak" } else { "Round" },
            Biome::Hill => if prng(&mut seed) < 0.70 { "Pine" } else { "Oak" },
            Biome::Mountain => if prng(&mut seed) < 0.75 { "Pine" } else { "Dead" },
        };
        forest_centers.push((fx, fz, fr, species));
    }

    // 2. Spawn trees around forest centers with gaussian falloff
    for &(fx, fz, fr, dominant_species) in &forest_centers {
        let cluster_size = (60.0 + prng(&mut seed) * 40.0) as usize;

        for _ in 0..cluster_size {
            let angle = prng(&mut seed) * std::f32::consts::TAU;
            let dist = gaussian_rand(&mut seed).abs() * fr;
            let rx = fx + angle.cos() * dist;
            let rz = fz + angle.sin() * dist;

            if rx.abs() > 195.0 || rz.abs() > 195.0 {
                continue;
            }

            let zone = get_zone(dist, fr);
            let req_spacing = get_spacing(zone, &mut seed) * 0.75;

            if check_overlap(rx, rz, req_spacing, &grid) {
                continue;
            }

            let ry = get_terrain_height(rx, rz);
            if ry > 1.5 && ry < 25.0 {
                let biome = get_biome(ry);
                let sc = match biome {
                    Biome::Lowland => 0.9 + prng(&mut seed) * 1.2,
                    Biome::Hill => 0.8 + prng(&mut seed) * 1.0,
                    Biome::Mountain => 0.6 + prng(&mut seed) * 0.8,
                };

                let tree_type = if prng(&mut seed) < 0.75 {
                    dominant_species
                } else {
                    pick_tree_type(biome, &mut seed)
                };

                ctx.db.resource_node().insert(ResourceNode {
                    node_id: 0,
                    node_type: format!("Tree:{}", tree_type),
                    x: rx,
                    y: ry,
                    z: rz,
                    chunk_x: (rx / 50.0).floor() as i32,
                    chunk_z: (rz / 50.0).floor() as i32,
                    health: 3,
                    scale: sc,
                    required_tool: "Stone Axe".into(),
                });
                insert_spatial(rx, rz, req_spacing, &mut grid);
                total_spawned += 1;
            }
        }
    }

    // 3. Populate ground clutter (Bushes, LooseStones, Branches, Flints)
    let total_clutter = 2800;
    for _ in 0..total_clutter {
        let rx = (prng(&mut seed) * 380.0) - 190.0;
        let rz = (prng(&mut seed) * 380.0) - 190.0;
        let ry = get_terrain_height(rx, rz);

        if ry <= 1.0 || ry >= 25.0 {
            continue;
        }

        let roll = prng(&mut seed);
        let (node_type, scale, req_spacing, health, required_tool) = if roll < 0.40 {
            ("LooseStone", 0.7 + prng(&mut seed) * 0.5, 3.0, 1, "None")
        } else if roll < 0.70 {
            ("Branch", 0.8 + prng(&mut seed) * 0.4, 2.5, 1, "None")
        } else if roll < 0.90 {
            ("Bush", 0.9 + prng(&mut seed) * 0.6, 3.5, 1, "None")
        } else {
            ("Flint", 0.6 + prng(&mut seed) * 0.4, 4.0, 1, "None")
        };

        if !check_overlap(rx, rz, req_spacing, &grid) {
            ctx.db.resource_node().insert(ResourceNode {
                node_id: 0,
                node_type: node_type.into(),
                x: rx,
                y: ry,
                z: rz,
                chunk_x: (rx / 50.0).floor() as i32,
                chunk_z: (rz / 50.0).floor() as i32,
                health,
                scale,
                required_tool: required_tool.into(),
            });
            insert_spatial(rx, rz, req_spacing, &mut grid);
            total_spawned += 1;
        }
    }

    log::debug!("Authoritative world resource nodes spawned: {}", total_spawned);
}

// ----------------------------------------------------------------------------
// SEEDING & PROCEDURAL TERRAIN MATH
// ----------------------------------------------------------------------------

static ACTIVE_WORLD_SEED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(42);
static NOISE_ELEVATION: std::sync::OnceLock<std::sync::RwLock<Perlin>> = std::sync::OnceLock::new();

#[inline]
pub fn get_noise_elevation() -> Perlin {
    *NOISE_ELEVATION.get_or_init(|| std::sync::RwLock::new(Perlin::new(42))).read().unwrap()
}

pub fn set_noise_elevation_seed(seed: u32) {
    if let Ok(mut lock) = NOISE_ELEVATION.get_or_init(|| std::sync::RwLock::new(Perlin::new(42))).write() {
        *lock = Perlin::new(seed);
    }
}

pub fn get_active_world_seed() -> u32 {
    ACTIVE_WORLD_SEED.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn apply_world_seed(seed: u32) {
    ACTIVE_WORLD_SEED.store(seed, std::sync::atomic::Ordering::Relaxed);
    set_noise_elevation_seed(seed);
    crate::voxel::set_subterrain_seeds(seed.wrapping_add(1296), seed.wrapping_add(1297));
}

pub fn ensure_world_seed_synced(ctx: &ReducerContext) -> u32 {
    let seed = ctx.db.global_state().id().find(0).map(|s| s.world_seed).unwrap_or(42);
    if get_active_world_seed() != seed {
        apply_world_seed(seed);
    }
    seed
}

pub fn get_terrain_height_seeded(x: f32, z: f32, seed: u32) -> f32 {
    let scale = 0.015;
    let base_height_amp = 18.0;
    let noise_elevation = Perlin::new(seed);

    let nx = x as f64 * scale;
    let nz = z as f64 * scale;

    let mut elevation = noise_elevation.get([nx, nz]) * 0.6
        + noise_elevation.get([nx * 2.0, nz * 2.0]) * 0.3
        + noise_elevation.get([nx * 4.0, nz * 4.0]) * 0.1;

    elevation = (elevation + 1.0) * 0.5;
    elevation = elevation.max(0.001);

    let mut y = (elevation.powf(1.4)) as f32 * base_height_amp;

    let river_factor = (x * 0.04).cos().abs() * 3.5;
    if river_factor < 2.0 {
        y = (y - (2.0 - river_factor)).max(0.5);
    }

    let lake_dist = ((x + 35.0) * (x + 35.0) + (z + 35.0) * (z + 35.0)).sqrt();
    if lake_dist < 25.0 {
        let basin_depth = (1.0 - (lake_dist / 25.0)).max(0.0) * 4.0;
        y = (y - basin_depth).max(0.2);
    }

    if y.is_nan() { y = 0.5; }
    y
}

pub fn get_terrain_height(x: f32, z: f32) -> f32 {
    let scale = 0.015;
    let base_height_amp = 18.0;
    let noise_elevation = get_noise_elevation();

    let nx = x as f64 * scale;
    let nz = z as f64 * scale;

    let mut elevation = noise_elevation.get([nx, nz]) * 0.6
        + noise_elevation.get([nx * 2.0, nz * 2.0]) * 0.3
        + noise_elevation.get([nx * 4.0, nz * 4.0]) * 0.1;

    elevation = (elevation + 1.0) * 0.5;
    elevation = elevation.max(0.001);

    let mut y = (elevation.powf(1.4)) as f32 * base_height_amp;

    let river_factor = (x * 0.04).cos().abs() * 3.5;
    if river_factor < 2.0 {
        y = (y - (2.0 - river_factor)).max(0.5);
    }

    let lake_dist = ((x + 35.0) * (x + 35.0) + (z + 35.0) * (z + 35.0)).sqrt();
    if lake_dist < 25.0 {
        let basin_depth = (1.0 - (lake_dist / 25.0)).max(0.0) * 4.0;
        y = (y - basin_depth).max(0.2);
    }

    if y.is_nan() { y = 0.5; }
    y
}

pub fn prng(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed as u32 as f32) / (u32::MAX as f32)
}

pub fn gaussian_rand(seed: &mut u64) -> f32 {
    let u1 = (prng(seed) as f64).max(1e-10);
    let u2 = prng(seed) as f64;
    ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()) as f32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    Lowland,   // 1.5 - 8.0m elevation
    Hill,      // 8.0 - 18.0m elevation
    Mountain,  // 18.0 - 25.0m elevation
}

pub fn get_biome(elevation: f32) -> Biome {
    if elevation <= 8.0 {
        Biome::Lowland
    } else if elevation <= 18.0 {
        Biome::Hill
    } else {
        Biome::Mountain
    }
}

pub fn pick_tree_type(biome: Biome, seed: &mut u64) -> &'static str {
    let roll = prng(seed);
    match biome {
        Biome::Lowland => {
            if roll < 0.50 {
                "Oak"
            } else if roll < 0.80 {
                "Round"
            } else {
                "Pine"
            }
        }
        Biome::Hill => {
            if roll < 0.60 {
                "Pine"
            } else if roll < 0.90 {
                "Oak"
            } else {
                "Dead"
            }
        }
        Biome::Mountain => {
            if roll < 0.70 {
                "Pine"
            } else {
                "Dead"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    ForestCore,
    ForestEdge,
    Clearing,
    Open,
}

pub fn get_zone(dist_from_center: f32, forest_radius: f32) -> Zone {
    let t = dist_from_center / forest_radius.max(1.0);
    if t < 0.5 {
        Zone::ForestCore
    } else if t < 0.8 {
        Zone::ForestEdge
    } else if t < 1.2 {
        Zone::Clearing
    } else {
        Zone::Open
    }
}

pub fn get_spacing(zone: Zone, seed: &mut u64) -> f32 {
    match zone {
        Zone::ForestCore => 2.0 + prng(seed) * 1.0,
        Zone::ForestEdge => 4.0 + prng(seed) * 2.0,
        Zone::Clearing => 8.0 + prng(seed) * 4.0,
        Zone::Open => 15.0 + prng(seed) * 10.0,
    }
}
