use spacetimedb::{table, reducer, Identity, ReducerContext, Table};
use noise::{NoiseFn, Perlin};
use log::info; 

// Expose internal modules to the SpacetimeDB compilation tree
pub mod movement;
pub mod combat;

// Bring generated accessor traits into scope
use crate::movement::{transform, player_session};
use crate::combat::{health, hitbox_history};

// ----------------------------------------------------------------------------
// MULTIPLAYER SCHEMAS & ENTITY STATE
// ----------------------------------------------------------------------------

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
}

#[table(accessor = resource_stockpile, public)]
#[derive(Clone)]
pub struct ResourceStockpile {
    #[primary_key] pub entity_id: u64, 
    pub wood: u32, 
    pub ore: u32, 
    pub food: u32,
}

#[table(accessor = ground_loot, public)]
#[derive(Clone)]
pub struct GroundLoot {
    #[primary_key] #[auto_inc] pub loot_id: u64,
    pub item_type: String, 
    pub x: f32, pub y: f32, pub z: f32,
}

#[table(accessor = resource_node, public)]
#[derive(Clone)]
pub struct ResourceNode {
    #[primary_key] #[auto_inc] pub node_id: u64,
    pub node_type: String, 
    pub x: f32, pub y: f32, pub z: f32,
    pub health: u32,
}

#[table(accessor = combat_event, public)]
#[derive(Clone)]
pub struct CombatEvent {
    #[primary_key] #[auto_inc] pub id: u64,
    pub event_type: String, 
    pub x: f32, pub y: f32, pub z: f32,
}

// ----------------------------------------------------------------------------
// LIFECYCLE REDUCERS
// ----------------------------------------------------------------------------

#[spacetimedb::reducer(init)]
pub fn init(_ctx: &ReducerContext) {}

#[spacetimedb::reducer(client_connected)]
pub fn client_connected(ctx: &ReducerContext) {
    let sender = ctx.sender();
    
    if ctx.db.resource_node().iter().count() == 0 {
        info!("World is empty. Bootstrapping synchronized procedural mountains, rivers, and resource nodes...");
        let mut seed = ctx.timestamp.to_micros_since_unix_epoch() as u64; 
        
        let mut spawned_positions: Vec<(f32, f32)> = Vec::new();
        let mut attempts = 0;
        
        while spawned_positions.len() < 200 && attempts < 3000 {
            attempts += 1;
            let rx = (prng(&mut seed) * 140.0) - 70.0; 
            let rz = (prng(&mut seed) * 140.0) - 70.0;
            
            let mut overlaps = false;
            for (px, pz) in &spawned_positions {
                let dist_sq = (px - rx) * (px - rx) + (pz - rz) * (pz - rz);
                if dist_sq < 16.0 { 
                    overlaps = true;
                    break;
                }
            }
            
            if overlaps { continue; }

            let ry = get_terrain_height(rx, rz);
            
            if ry > 1.5 && ry < 25.0 {
                spawned_positions.push((rx, rz));
                let type_roll = prng(&mut seed);
                
                let (node_type, health) = if ry > 12.0 { 
                    ("Rock", 6) 
                } else if type_roll < 0.5 { 
                    ("Tree", 4) 
                } else { 
                    ("Bush", 2) 
                };

                ctx.db.resource_node().insert(ResourceNode { 
                    node_id: 0, node_type: node_type.into(), x: rx, y: ry, z: rz, health
                });
            }
        }
    }

    if let Some(mut player) = ctx.db.player().identity().find(sender) {
        player.is_online = true;
        ctx.db.player().entity_id().update(player);
        info!("Player resumed session: {}", sender.to_hex());
    } else {
        let inserted_player = ctx.db.player().insert(Player { 
            entity_id: 0, 
            identity: sender, 
            is_online: true 
        });
        
        let entity_id = inserted_player.entity_id;

        // Architectural Note: Provision new schemas for movement and combat.
        ctx.db.transform().insert(movement::Transform {
            entity_id,
            x: 0.0, y: 20.0, z: 0.0,
            last_processed_tick: 0,
        });

        ctx.db.player_session().insert(movement::PlayerSession {
            identity: sender,
            entity_id,
        });

        ctx.db.health().insert(combat::Health {
            entity_id,
            current: 100.0,
            max: 100.0,
        });
        
        ctx.db.hitbox_history().insert(combat::HitboxHistory {
            entity_id,
            snapshots: Vec::new(),
        });

        ctx.db.resource_stockpile().insert(ResourceStockpile {
            entity_id, wood: 0, ore: 0, food: 0,
        });
        
        ctx.db.player_perspective().insert(PlayerPerspective {
            entity_id, camera_mode: "FPS".to_string(),
        });
        
        info!("Provisioned new player profile for identity: {}", sender.to_hex());
    }
}

#[spacetimedb::reducer(client_disconnected)]
pub fn client_disconnected(ctx: &ReducerContext) {
    let sender = ctx.sender();
    if let Some(mut player) = ctx.db.player().identity().find(sender) {
        player.is_online = false;
        ctx.db.player().entity_id().update(player);
        info!("Player disconnected and state persisted: {}", sender.to_hex());
    }
}

// ----------------------------------------------------------------------------
// GAME LOGIC REDUCERS (AUTHORITATIVE STATE)
// ----------------------------------------------------------------------------

#[reducer]
pub fn set_camera_mode(ctx: &ReducerContext, mode: String) {
    let sender = ctx.sender();
    let Some(player) = ctx.db.player().identity().find(sender) else { return; };
    
    if let Some(mut perspective) = ctx.db.player_perspective().entity_id().find(player.entity_id) {
        perspective.camera_mode = mode.clone();
        ctx.db.player_perspective().entity_id().update(perspective);
    } else {
        ctx.db.player_perspective().insert(PlayerPerspective {
            entity_id: player.entity_id,
            camera_mode: mode.clone(),
        });
    }
    
    info!("Player {} dynamically transitioned to {} mode spatial partitioning.", player.entity_id, mode);
}

#[reducer]
pub fn gather_loot(ctx: &ReducerContext, loot_id: u64) {
    let sender = ctx.sender();
    let Some(player) = ctx.db.player().identity().find(sender) else { return; };
    let Some(loot) = ctx.db.ground_loot().loot_id().find(loot_id) else { return; };
    
    let Some(mut stockpile) = ctx.db.resource_stockpile().entity_id().find(player.entity_id) else { return; };

    ctx.db.ground_loot().loot_id().delete(loot_id);
    
    match loot.item_type.as_str() {
        "Flint" | "Ore" => stockpile.ore = stockpile.ore.saturating_add(1),
        "Berry" => stockpile.food = stockpile.food.saturating_add(1),
        "Wood" | "Branch" => stockpile.wood = stockpile.wood.saturating_add(1),
        _ => {}
    }
    
    ctx.db.resource_stockpile().entity_id().update(stockpile);
}

#[reducer]
pub fn swing_tool(ctx: &ReducerContext, px: f32, py: f32, pz: f32, dx: f32, dy: f32, dz: f32) {
    let sender = ctx.sender();
    if ctx.db.player().identity().find(sender).is_none() { return; }

    let mut hit_node = None;
    let mut min_dist = 4.0_f32; 

    for node in ctx.db.resource_node().iter() {
        let dist_x = node.x - px; 
        let dist_y = node.y - py; 
        let dist_z = node.z - pz;
        let dist = (dist_x * dist_x + dist_y * dist_y + dist_z * dist_z).sqrt();

        if dist < min_dist {
            let dot = (dist_x / dist) * dx + (dist_y / dist) * dy + (dist_z / dist) * dz;
            if dot > 0.8 { 
                min_dist = dist;
                hit_node = Some(node);
            }
        }
    }

    if let Some(node) = hit_node {
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
            let item_type = match node.node_type.as_str() {
                "Tree" => "Wood", "Rock" => "Ore", "Bush" => "Berry", _ => "Wood"
            };
            ctx.db.ground_loot().insert(GroundLoot {
                loot_id: 0, item_type: item_type.into(),
                x: node.x, y: node.y + 0.5, z: node.z,
            });
        }
    }
}

// ----------------------------------------------------------------------------
// UTILITY FUNCTIONS & PROCEDURAL TERRAIN FORMULAS
// ----------------------------------------------------------------------------

fn get_terrain_height(x: f32, z: f32) -> f32 {
    let scale = 0.015; 
    let base_height_amp = 18.0; 
    let noise_elevation = Perlin::new(42); 

    let nx = x as f64 * scale; 
    let nz = z as f64 * scale;

    let mut elevation = noise_elevation.get([nx, nz]) * 0.6
        + noise_elevation.get([nx * 2.0, nz * 2.0]) * 0.3
        + noise_elevation.get([nx * 4.0, nz * 4.0]) * 0.1;
    
    elevation = (elevation + 1.0) * 0.5;
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

    y
}

fn prng(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13; 
    *seed ^= *seed >> 7; 
    *seed ^= *seed << 17;
    (*seed as u32 as f32) / (u32::MAX as f32)
}