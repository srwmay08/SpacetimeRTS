use spacetimedb::{table, reducer, Identity, ReducerContext, Table, ScheduleAt, SpacetimeType};
use std::time::Duration;
use noise::{NoiseFn, Perlin}; 

pub mod movement;
pub mod combat;
pub mod building;
pub mod ai; 

use crate::movement::{transform, player_session};
use crate::combat::{health, hitbox_history};

#[table(accessor = high_frequency_timer, scheduled(high_frequency_tick))]
#[derive(Clone)]
pub struct HighFrequencyTimer {
    #[primary_key] #[auto_inc] pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}

#[table(accessor = low_frequency_timer, scheduled(low_frequency_tick))]
#[derive(Clone)]
pub struct LowFrequencyTimer {
    #[primary_key] #[auto_inc] pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}

#[table(accessor = player, public)]
#[derive(Clone)]
pub struct Player {
    #[primary_key] #[auto_inc] pub entity_id: u64,
    #[unique] pub identity: Identity,
    pub is_online: bool, 
}

#[table(accessor = player_perspective, public)]
#[derive(Clone)]
pub struct PlayerPerspective {
    #[primary_key] pub entity_id: u64,
    pub camera_mode: String, 
    pub in_interior: bool, 
}

#[derive(SpacetimeType, Clone)]
pub struct InventorySlot {
    pub item_type: String,
    pub count: u32,
}

#[table(accessor = inventory, public)]
#[derive(Clone)]
pub struct Inventory {
    #[primary_key] pub entity_id: u64, 
    pub slots: Vec<InventorySlot>,
}

#[table(accessor = resource_node, public)]
#[derive(Clone)]
pub struct ResourceNode {
    #[primary_key] #[auto_inc] pub node_id: u64,
    pub node_type: String, 
    pub x: f32, pub y: f32, pub z: f32,
    pub health: u32,
    pub scale: f32, 
    pub required_tool: String, 
}

#[table(accessor = respawn_bush_timer, scheduled(respawn_bush_tick))]
#[derive(Clone)]
pub struct RespawnBushTimer {
    #[primary_key] #[auto_inc] pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub node_id: u64,
}

#[table(accessor = combat_event, public)]
#[derive(Clone)]
pub struct CombatEvent {
    #[primary_key] #[auto_inc] pub id: u64,
    pub event_type: String, 
    pub x: f32, pub y: f32, pub z: f32,
}

#[table(accessor = nav_event, public)]
#[derive(Clone)]
pub struct NavEvent {
    #[primary_key] #[auto_inc] pub id: u64,
    pub min_x: f32, pub min_y: f32, pub min_z: f32,
    pub max_x: f32, pub max_y: f32, pub max_z: f32,
}

// ----------------------------------------------------------------------------
// HELPER FUNCTIONS
// ----------------------------------------------------------------------------

pub fn add_item(inventory: &mut Inventory, item_type: &str, mut amount: u32) {
    for slot in inventory.slots.iter_mut() {
        if slot.item_type == item_type && slot.count < 50 {
            let space = 50 - slot.count;
            if amount <= space {
                slot.count += amount;
                amount = 0;
                break;
            } else {
                slot.count = 50;
                amount -= space;
            }
        }
    }
    
    while amount > 0 && inventory.slots.len() < 16 {
        let add_amt = amount.min(50);
        inventory.slots.push(InventorySlot {
            item_type: item_type.to_string(),
            count: add_amt,
        });
        amount -= add_amt;
    }
}

pub fn remove_item(inventory: &mut Inventory, item_type: &str, mut amount: u32) -> bool {
    let total: u32 = inventory.slots.iter().filter(|s| s.item_type == item_type).map(|s| s.count).sum();
    if total < amount { return false; }
    
    for slot in inventory.slots.iter_mut() {
        if slot.item_type == item_type {
            if slot.count >= amount {
                slot.count -= amount;
                break;
            } else {
                amount -= slot.count;
                slot.count = 0;
            }
        }
    }
    inventory.slots.retain(|s| s.count > 0);
    true
}

// ----------------------------------------------------------------------------
// LIFECYCLE & TICK REDUCERS
// ----------------------------------------------------------------------------

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) {
    ctx.db.high_frequency_timer().insert(HighFrequencyTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(Duration::from_millis(16).into()), 
    });

    ctx.db.low_frequency_timer().insert(LowFrequencyTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(Duration::from_millis(100).into()), 
    });
}

#[reducer]
pub fn high_frequency_tick(_ctx: &ReducerContext, _timer: HighFrequencyTimer) {}

#[reducer]
pub fn low_frequency_tick(ctx: &ReducerContext, _timer: LowFrequencyTimer) {
    crate::ai::process_ai_tick(ctx);
}

#[spacetimedb::reducer(client_connected)]
pub fn client_connected(ctx: &ReducerContext) {
    let sender = ctx.sender();
    
    if ctx.db.resource_node().iter().count() == 0 {
        let mut seed = ctx.timestamp.to_micros_since_unix_epoch() as u64; 
        
        let mut spawned_positions: Vec<(f32, f32)> = Vec::new();
        let mut attempts = 0;
        
        while spawned_positions.len() < 800 && attempts < 5000 {
            attempts += 1;
            let rx = (prng(&mut seed) * 400.0) - 200.0; 
            let rz = (prng(&mut seed) * 400.0) - 200.0;
            
            let mut overlaps = false;
            for (px, pz) in &spawned_positions {
                if (px - rx) * (px - rx) + (pz - rz) * (pz - rz) < 16.0 { 
                    overlaps = true;
                    break;
                }
            }
            if overlaps { continue; }

            let ry = get_terrain_height(rx, rz);
            if ry > 1.5 && ry < 25.0 {
                spawned_positions.push((rx, rz));
                let type_roll = prng(&mut seed);
                
                let (node_type, scale, health, req_tool) = if ry > 12.0 { 
                    let sc = 0.5 + prng(&mut seed) * 2.5; 
                    ("Rock", sc, (4.0 * sc) as u32, "None")
                } else if type_roll < 0.5 { 
                    let sc = 0.5 + prng(&mut seed) * 2.0; 
                    let tool = if sc > 1.5 { "Axe" } else { "None" };
                    ("Tree", sc, (3.0 * sc) as u32, tool)
                } else { 
                    ("Bush", 1.0, 1, "None")
                };

                ctx.db.resource_node().insert(ResourceNode { 
                    node_id: 0, node_type: node_type.into(), x: rx, y: ry, z: rz, health, scale, required_tool: req_tool.into()
                });
            }
        }
    }

    if let Some(mut player) = ctx.db.player().identity().find(sender) {
        player.is_online = true;
        ctx.db.player().entity_id().update(player);
    } else {
        let inserted_player = ctx.db.player().insert(Player { 
            entity_id: 0, identity: sender, is_online: true 
        });
        
        let entity_id = inserted_player.entity_id;
        let spawn_y = get_terrain_height(0.0, 0.0) + 10.0;

        ctx.db.transform().insert(movement::Transform {
            entity_id, x: 0.0, y: spawn_y, z: 0.0, chunk_x: 0, chunk_z: 0, last_processed_tick: 0,
        });

        ctx.db.player_session().insert(movement::PlayerSession {
            identity: sender, entity_id,
        });

        ctx.db.health().insert(combat::Health {
            entity_id, current: 100.0, max: 100.0,
        });
        
        ctx.db.hitbox_history().insert(combat::HitboxHistory {
            entity_id, snapshots: Vec::new(),
        });

        ctx.db.inventory().insert(Inventory {
            entity_id, 
            slots: vec![
                InventorySlot { item_type: "Axe".into(), count: 1 },
                InventorySlot { item_type: "Wood".into(), count: 150 }, 
            ],
        });
        
        ctx.db.player_perspective().insert(PlayerPerspective {
            entity_id, camera_mode: "FPS".to_string(), in_interior: false,
        });
    }
}

#[spacetimedb::reducer(client_disconnected)]
pub fn client_disconnected(ctx: &ReducerContext) {
    let sender = ctx.sender();
    if let Some(mut player) = ctx.db.player().identity().find(sender) {
        player.is_online = false;
        ctx.db.player().entity_id().update(player);
    }
}

// ----------------------------------------------------------------------------
// INTERACTION & COMBAT REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn set_camera_mode(ctx: &ReducerContext, mode: String) {
    let sender = ctx.sender();
    let Some(player) = ctx.db.player().identity().find(sender) else { return; };
    if let Some(mut perspective) = ctx.db.player_perspective().entity_id().find(player.entity_id) {
        perspective.camera_mode = mode.clone();
        ctx.db.player_perspective().entity_id().update(perspective);
    }
}

#[reducer]
pub fn set_interior_culling(ctx: &ReducerContext, is_inside: bool) -> Result<(), String> {
    let sender = ctx.sender();
    let player = ctx.db.player().identity().find(sender).ok_or("Unauthorized")?;
    if let Some(mut perspective) = ctx.db.player_perspective().entity_id().find(player.entity_id) {
        perspective.in_interior = is_inside;
        ctx.db.player_perspective().entity_id().update(perspective);
    }
    Ok(())
}

#[reducer]
pub fn interact_node(ctx: &ReducerContext, node_id: u64) -> Result<(), String> {
    let sender = ctx.sender();
    let player = ctx.db.player().identity().find(sender).ok_or("Unauthorized")?;
    
    let mut inventory = ctx.db.inventory().entity_id().find(player.entity_id).unwrap_or_else(|| Inventory {
        entity_id: player.entity_id,
        slots: vec![],
    });
    
    let mut node = ctx.db.resource_node().node_id().find(node_id).ok_or("Node not found")?;
    if node.node_type != "Bush" { return Err("Entity not interactable".into()); }
    if node.health == 0 { return Err("Berries depleted".into()); }
    
    add_item(&mut inventory, "Berry", 5);
    
    if ctx.db.inventory().entity_id().find(player.entity_id).is_some() {
        ctx.db.inventory().entity_id().update(inventory);
    } else {
        ctx.db.inventory().insert(inventory);
    }
    
    node.health = 0; 
    ctx.db.resource_node().node_id().update(node);
    
    ctx.db.respawn_bush_timer().insert(RespawnBushTimer {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(Duration::from_secs(60).into()),
        node_id,
    });
    
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
pub fn swing_tool(ctx: &ReducerContext, px: f32, py: f32, pz: f32, dx: f32, dy: f32, dz: f32) {
    let sender = ctx.sender();
    let Some(player) = ctx.db.player().identity().find(sender) else { return; };
    
    let mut inventory = ctx.db.inventory().entity_id().find(player.entity_id).unwrap_or_else(|| Inventory {
        entity_id: player.entity_id,
        slots: vec![],
    });

    let mut hit_node = None;
    let mut min_dist = 12.0_f32; 

    for node in ctx.db.resource_node().iter() {
        let dist_x = node.x - px; 
        let dist_y = node.y - py; 
        let dist_z = node.z - pz;
        let dist = (dist_x * dist_x + dist_y * dist_y + dist_z * dist_z).sqrt();

        if dist < min_dist {
            let dot = (dist_x / dist) * dx + (dist_y / dist) * dy + (dist_z / dist) * dz;
            if dot > 0.5 { 
                min_dist = dist;
                hit_node = Some(node);
            }
        }
    }

    if let Some(node) = hit_node {
        if node.required_tool == "Axe" {
            let has_axe = inventory.slots.iter().any(|s| s.item_type == "Axe" && s.count > 0);
            if !has_axe { return; }
        }

        let event_type = format!("Hit{}", node.node_type);
        ctx.db.combat_event().insert(CombatEvent {
            id: 0, event_type, 
            x: node.x, y: node.y + 1.0, z: node.z
        });

        if node.health > 1 {
            let mut updated = node.clone();
            updated.health = updated.health.saturating_sub(1);
            ctx.db.resource_node().node_id().update(updated);
        } else {
            ctx.db.resource_node().node_id().delete(node.node_id);
            
            let base_amount = match node.node_type.as_str() { "Tree" => 5, "Rock" => 3, _ => 1 };
            let amount = (base_amount as f32 * node.scale).ceil() as u32;
            let item = match node.node_type.as_str() { "Tree" => "Wood", "Rock" => "Ore", _ => "Wood" };
            
            add_item(&mut inventory, item, amount);
            
            if ctx.db.inventory().entity_id().find(player.entity_id).is_some() {
                ctx.db.inventory().entity_id().update(inventory);
            } else {
                ctx.db.inventory().insert(inventory);
            }
        }
    }
}

// ----------------------------------------------------------------------------
// UTILITIES
// ----------------------------------------------------------------------------

pub fn get_terrain_height(x: f32, z: f32) -> f32 {
    let scale = 0.015; 
    let base_height_amp = 18.0; 
    let noise_elevation = Perlin::new(42); 

    let nx = x as f64 * scale; 
    let nz = z as f64 * scale;

    let mut elevation = noise_elevation.get([nx, nz]) * 0.6
        + noise_elevation.get([nx * 2.0, nz * 2.0]) * 0.3
        + noise_elevation.get([nx * 4.0, nz * 4.0]) * 0.1;
    
    elevation = (elevation + 1.0) * 0.5;
    
    // Architectural Note: Critical safety clamp. Ensures fractional exponents 
    // never attempt to process negative bases, avoiding NaN propagation and subsequent WASM traps.
    elevation = elevation.max(0.001);
    
    let mut y = (elevation.powf(1.4)) as f32 * base_height_amp;

    let river_factor = (x * 0.04).cos().abs() * 3.5;
    if river_factor < 2.0 { y = (y - (2.0 - river_factor)).max(0.5); }

    let lake_dist = ((x + 35.0) * (x + 35.0) + (z + 35.0) * (z + 35.0)).sqrt();
    if lake_dist < 25.0 {
        let basin_depth = (1.0 - (lake_dist / 25.0)).max(0.0) * 4.0;
        y = (y - basin_depth).max(0.2);
    }
    
    if y.is_nan() { y = 0.5; }
    y
}

fn prng(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13; 
    *seed ^= *seed >> 7; 
    *seed ^= *seed << 17;
    (*seed as u32 as f32) / (u32::MAX as f32)
}