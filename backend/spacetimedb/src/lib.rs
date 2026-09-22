// ----------------------------------------------------------------------------
// CORE MODULE IMPORTS & GLOBAL TABLES
// ----------------------------------------------------------------------------
use spacetimedb::{table, reducer, Identity, ReducerContext, Table, ScheduleAt, SpacetimeType};
use std::time::Duration;
use noise::{NoiseFn, Perlin}; 

pub mod movement;
pub mod combat;
pub mod building;
pub mod ai; 

use crate::movement::{transform, player_session};
use crate::combat::{health, hitbox_history, faction_component, Faction};
use crate::ai::{npc_brain, AiType, BrainState, harvestable_corpse};
use crate::building::{structure, Structure};

// Architectural Note: GlobalState tracks the 24-hour Server Time of Day cycle 
// to dynamically alter line of sight, spawn rules, and unit abilities.
#[table(accessor = global_state, public)]
#[derive(Clone)]
pub struct GlobalState {
    #[primary_key] pub id: u32,
    pub time_of_day: f32, 
}

// Architectural Note: Waypoints act as the central nervous system for Commander-to-FPS squad coordination,
// replacing verbal callouts with hard visual HUD markers.
#[table(accessor = waypoint, public)]
#[derive(Clone)]
pub struct Waypoint {
    #[primary_key] #[auto_inc] pub waypoint_id: u64,
    pub commander_id: u64,
    pub x: f32, pub y: f32, pub z: f32,
    pub order_type: String, 
    pub expires_at: u64,
}

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
    pub discovered_items: Vec<String>,
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
    pub max_x: f32, pub max_y: f32, max_z: f32,
}

// ----------------------------------------------------------------------------
// HELPER FUNCTIONS
// ----------------------------------------------------------------------------

pub fn add_item(inventory: &mut Inventory, item_type: &str, mut amount: u32) {
    if !inventory.discovered_items.contains(&item_type.to_string()) {
        inventory.discovered_items.push(item_type.to_string());
    }

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

pub fn has_item(inventory: &Inventory, item_type: &str, amount: u32) -> bool {
    let total: u32 = inventory.slots.iter().filter(|s| s.item_type == item_type).map(|s| s.count).sum();
    total >= amount
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
    
    ctx.db.global_state().insert(GlobalState { id: 0, time_of_day: 8.0 });
}

#[reducer]
pub fn high_frequency_tick(_ctx: &ReducerContext, _timer: HighFrequencyTimer) {}

#[reducer]
pub fn low_frequency_tick(ctx: &ReducerContext, _timer: LowFrequencyTimer) {
    crate::ai::process_ai_tick(ctx);
    crate::ai::process_npc_brain_tick(ctx, 0.1); 
    
    // Architectural Note: 3. Map Dynamics (Time-of-Day Dynamics)
    // Progresses the global server time loop. Scaled so a full day cycle takes roughly 48 real-time minutes.
    if let Some(mut state) = ctx.db.global_state().id().find(0) {
        state.time_of_day += 0.005; 
        if state.time_of_day >= 24.0 { state.time_of_day -= 24.0; }
        ctx.db.global_state().id().update(state);
    }
    
    // Architectural Note: Expire waypoints to maintain the HUD/Attention Economy
    let now = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    let expired: Vec<u64> = ctx.db.waypoint().iter()
        .filter(|w| w.expires_at < now)
        .map(|w| w.waypoint_id)
        .collect();
        
    for id in expired {
        ctx.db.waypoint().waypoint_id().delete(id);
    }
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
                } else if type_roll < 0.15 { 
                    ("Flint", 0.5, 1, "None")
                } else if type_roll < 0.3 {
                    ("LooseStone", 0.5, 1, "None")
                } else if type_roll < 0.5 {
                    ("Branch", 0.5, 1, "None")
                } else if type_roll < 0.8 { 
                    let sc = 0.5 + prng(&mut seed) * 2.0; 
                    let tool = if sc > 1.5 { "Stone Axe" } else { "None" };
                    ("Tree", sc, (3.0 * sc) as u32, tool)
                } else { 
                    ("Bush", 1.0, 1, "None")
                };

                ctx.db.resource_node().insert(ResourceNode { 
                    node_id: 0, node_type: node_type.into(), x: rx, y: ry, z: rz, health, scale, required_tool: req_tool.into()
                });
            }
        }

        let base_x = 0.0;
        let base_z = 20.0; 
        let base_y = get_terrain_height(base_x, base_z) + 1.0; 
        
        ctx.db.structure().insert(Structure {
            structure_id: 1, parent_id: None, piece_type: "Foundation".into(),
            stability: 100, is_grounded: true, x: base_x, y: base_y, z: base_z,
            rot_x: 0.0, rot_y: 0.0, rot_z: 0.0, rot_w: 1.0, owner_id: 0,
            is_blueprint: false, construction_progress: 100, // Fix for E0063
        });

        let mut npc_id = ctx.timestamp.to_micros_since_unix_epoch() as u64 + 10000;

        for _ in 0..15 {
            let mut nx = 0.0; let mut nz = 0.0;
            let mut valid = false;
            for _ in 0..10 {
                nx = (prng(&mut seed) * 400.0) - 200.0;
                nz = (prng(&mut seed) * 400.0) - 200.0;
                let mut overlaps = false;
                for (px, pz) in &spawned_positions {
                    if (px - nx) * (px - nx) + (pz - nz) * (pz - nz) < 9.0 { overlaps = true; break; }
                }
                if !overlaps { valid = true; break; }
            }
            if valid {
                spawned_positions.push((nx, nz));
                let ny = get_terrain_height(nx, nz) + 1.5;
                ctx.db.transform().insert(movement::Transform { entity_id: npc_id, x: nx, y: ny, z: nz, chunk_x: (nx/50.0) as i32, chunk_z: (nz/50.0) as i32, last_processed_tick: 0 });
                ctx.db.health().insert(combat::Health { entity_id: npc_id, current: 30.0, max: 30.0 });
                ctx.db.faction_component().insert(combat::FactionComponent { entity_id: npc_id, faction: Faction::Wildlife });
                ctx.db.npc_brain().insert(crate::ai::NpcBrain { 
                    entity_id: npc_id, ai_type: AiType::Deer, state: BrainState::Idle, target_id: None, timer: 0.0,
                    home_x: nx, home_z: nz, wander_x: nx, wander_z: nz
                });
                npc_id += 1;
            }
        }
    }

    if let Some(mut player) = ctx.db.player().identity().find(sender) {
        player.is_online = true;
        let entity_id = player.entity_id;
        ctx.db.player().entity_id().update(player);
        
        if ctx.db.transform().entity_id().find(entity_id).is_none() {
            let spawn_y = get_terrain_height(0.0, 0.0) + 10.0;
            ctx.db.transform().insert(movement::Transform {
                entity_id, x: 0.0, y: spawn_y, z: 0.0, chunk_x: 0, chunk_z: 0, last_processed_tick: 0,
            });
        }
        
        if ctx.db.health().entity_id().find(entity_id).is_none() {
            ctx.db.health().insert(combat::Health {
                entity_id, current: 100.0, max: 100.0,
            });
        }
        
        if ctx.db.faction_component().entity_id().find(entity_id).is_none() {
            ctx.db.faction_component().insert(combat::FactionComponent {
                entity_id, faction: Faction::Player,
            });
        }
        
        if ctx.db.hitbox_history().entity_id().find(entity_id).is_none() {
            ctx.db.hitbox_history().insert(combat::HitboxHistory {
                entity_id, snapshots: Vec::new(),
            });
        }
        
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
        
        ctx.db.faction_component().insert(combat::FactionComponent {
            entity_id, faction: Faction::Player,
        });

        ctx.db.hitbox_history().insert(combat::HitboxHistory {
            entity_id, snapshots: Vec::new(),
        });

        ctx.db.inventory().insert(Inventory {
            entity_id, 
            slots: vec![],
            discovered_items: vec![],
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
// PROGRESSION REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn consume_item(ctx: &ReducerContext, item_name: String) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;
    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or("Inventory not found")?;

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
            log::info!("Player {} consumed {} for {} HP.", session.entity_id, item_name, heal_amount);
        }
    } else {
        return Err("Item is not consumable.".to_string());
    }

    ctx.db.inventory().entity_id().update(inv);
    Ok(())
}

#[reducer]
pub fn craft_item(ctx: &ReducerContext, item_name: String) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or("Inventory not found")?;

    let player_transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or("Transform not found")?;

    let requires_workbench = matches!(item_name.as_str(), "Leather Tunic" | "Crude Bow" | "Flint Arrow");
    if requires_workbench {
        let mut valid_workbench = false;
        for s in ctx.db.structure().iter().filter(|s| s.piece_type == "Workbench") {
            let dist_sq = (s.x - player_transform.x).powi(2) + (s.z - player_transform.z).powi(2);
            if dist_sq < 100.0 { 
                if crate::building::is_covered(ctx, s.x, s.y, s.z) {
                    valid_workbench = true;
                    break;
                }
            }
        }
        if !valid_workbench {
            return Err("Crafting this item requires a Workbench with adequate roof cover nearby.".into());
        }
    }

    match item_name.as_str() {
        "Hammer" => {
            if !has_item(&inv, "Branch", 1) || !has_item(&inv, "LooseStone", 1) {
                return Err("Missing materials: 1 Branch, 1 LooseStone.".into());
            }
            remove_item(&mut inv, "Branch", 1);
            remove_item(&mut inv, "LooseStone", 1);
            add_item(&mut inv, "Hammer", 1);
        }
        "Stone Axe" => {
            if !has_item(&inv, "Branch", 1) || !has_item(&inv, "Flint", 1) {
                return Err("Missing materials: 1 Branch, 1 Flint.".into());
            }
            remove_item(&mut inv, "Branch", 1);
            remove_item(&mut inv, "Flint", 1);
            add_item(&mut inv, "Stone Axe", 1);
        }
        "Club" => {
            if !has_item(&inv, "Branch", 2) {
                return Err("Missing materials: 2 Branch.".into());
            }
            remove_item(&mut inv, "Branch", 2);
            add_item(&mut inv, "Club", 1);
        }
        "Torch" => {
            if !has_item(&inv, "Branch", 1) || !has_item(&inv, "Resin", 1) {
                return Err("Missing materials: 1 Branch, 1 Resin.".into());
            }
            remove_item(&mut inv, "Branch", 1);
            remove_item(&mut inv, "Resin", 1);
            add_item(&mut inv, "Torch", 1);
        }
        "Crude Bow" => {
            if !has_item(&inv, "Wood", 10) || !has_item(&inv, "Leather Scraps", 8) {
                return Err("Missing materials: 10 Wood, 8 Leather Scraps.".into());
            }
            remove_item(&mut inv, "Wood", 10);
            remove_item(&mut inv, "Leather Scraps", 8);
            add_item(&mut inv, "Crude Bow", 1);
        }
        _ => return Err("Unknown or locked crafting recipe.".into()),
    }

    ctx.db.inventory().entity_id().update(inv);
    Ok(())
}

// ----------------------------------------------------------------------------
// INTERACTION REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn interact_node(ctx: &ReducerContext, node_id: u64) -> Result<(), String> {
    let sender = ctx.sender();
    let player = ctx.db.player().identity().find(sender).ok_or("Unauthorized")?;
    
    let mut inventory = ctx.db.inventory().entity_id().find(player.entity_id).unwrap();
    let mut node = ctx.db.resource_node().node_id().find(node_id).ok_or("Node not found")?;
    
    if node.node_type != "Bush" { return Err("Entity not interactable".into()); }
    if node.health == 0 { return Err("Berries depleted".into()); }
    
    add_item(&mut inventory, "Berry", 2);
    
    ctx.db.inventory().entity_id().update(inventory);
    
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
    
    let mut inventory = ctx.db.inventory().entity_id().find(player.entity_id).unwrap();
    let mut hit_node = None;
    let mut hit_entity = None;
    let mut min_dist = 12.0_f32; 

    for node in ctx.db.resource_node().iter() {
        let dist = ((node.x - px).powi(2) + (node.y - py).powi(2) + (node.z - pz).powi(2)).sqrt();
        if dist < min_dist {
            let dot = ((node.x - px) / dist) * dx + ((node.y - py) / dist) * dy + ((node.z - pz) / dist) * dz;
            if dot > 0.5 { min_dist = dist; hit_node = Some(node); }
        }
    }

    for t in ctx.db.transform().iter() {
        if t.entity_id == player.entity_id { continue; } 
        if ctx.db.health().entity_id().find(t.entity_id).is_none() { continue; } 
        let dist = ((t.x - px).powi(2) + (t.y - py).powi(2) + (t.z - pz).powi(2)).sqrt();
        if dist < min_dist {
            let dot = ((t.x - px) / dist) * dx + ((t.y - py) / dist) * dy + ((t.z - pz) / dist) * dz;
            if dot > 0.5 { min_dist = dist; hit_entity = Some(t); hit_node = None; }
        }
    }

    if let Some(target) = hit_entity {
        if let Some(corpse) = ctx.db.harvestable_corpse().entity_id().find(target.entity_id) {
            ctx.db.combat_event().insert(CombatEvent { id: 0, event_type: "HitPlayer".into(), x: target.x, y: target.y + 0.5, z: target.z });
            if let Some(mut hp) = ctx.db.health().entity_id().find(target.entity_id) {
                if hp.current > 1.0 {
                    hp.current -= 1.0; ctx.db.health().entity_id().update(hp);
                } else {
                    add_item(&mut inventory, &corpse.loot_item, corpse.amount);
                    ctx.db.inventory().entity_id().update(inventory);
                    ctx.db.health().entity_id().delete(target.entity_id);
                    ctx.db.transform().entity_id().delete(target.entity_id);
                    ctx.db.faction_component().entity_id().delete(target.entity_id);
                    ctx.db.npc_brain().entity_id().delete(target.entity_id);
                    ctx.db.harvestable_corpse().entity_id().delete(target.entity_id);
                }
            }
            return;
        }
        
        crate::combat::apply_damage(ctx, target.entity_id, 20.0);
        ctx.db.combat_event().insert(CombatEvent { id: 0, event_type: "HitPlayer".into(), x: target.x, y: target.y + 1.0, z: target.z });
        return; 
    }

    if let Some(node) = hit_node {
        if node.required_tool == "Stone Axe" {
            let has_axe = inventory.slots.iter().any(|s| s.item_type == "Stone Axe" && s.count > 0);
            if !has_axe { return; }
        }

        let event_type = format!("Hit{}", node.node_type);
        ctx.db.combat_event().insert(CombatEvent { id: 0, event_type, x: node.x, y: node.y + 1.0, z: node.z });

        if node.health > 1 {
            let mut updated = node.clone();
            updated.health = updated.health.saturating_sub(1);
            ctx.db.resource_node().node_id().update(updated);
        } else {
            ctx.db.resource_node().node_id().delete(node.node_id);
            
            let amount = (match node.node_type.as_str() { "Tree" => 5, "Rock" => 3, _ => 1 } as f32 * node.scale).ceil() as u32;
            let item = match node.node_type.as_str() { 
                "Tree" => "Wood", "Rock" => "Stone", "Branch" => "Branch", "Flint" => "Flint", "LooseStone" => "LooseStone", _ => "Wood" 
            };
            
            add_item(&mut inventory, item, amount);
            if node.node_type == "Tree" {
                if inventory.slots.iter().any(|s| s.item_type == "Stone Axe" && s.count > 0) {
                    add_item(&mut inventory, "Resin", 1);
                }
            }
            ctx.db.inventory().entity_id().update(inventory);
        }
    }
}

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

pub fn prng(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13; 
    *seed ^= *seed >> 7; 
    *seed ^= *seed << 17;
    (*seed as u32 as f32) / (u32::MAX as f32)
}