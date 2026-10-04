// ----------------------------------------------------------------------------
// CORE MODULE IMPORTS & GLOBAL SCHEMAS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Implements authoritative server state for the game.
// Features an autonomous NPC population manager ensuring all 4 mob species
// (Deer, Boars, Goblins, Peasants) remain actively populated in the world.

use spacetimedb::{table, reducer, Identity, ReducerContext, Table, ScheduleAt, SpacetimeType};
use std::time::Duration;
use noise::{NoiseFn, Perlin};

pub mod movement;
pub mod combat;
pub mod building;
pub mod ai;
pub mod voxel;
pub mod spatial;
pub mod physics;
pub mod armory;
pub mod bestiary;

use crate::movement::{transform, player_session};
use crate::combat::{health, hitbox_history, faction_component, Faction, weapon_skill, equipment_loadout};
use crate::ai::{npc_brain, AiType, BrainState, harvestable_corpse, peasant, pet_component};
use crate::building::{structure, Structure};
use crate::voxel::voxel_chunk;

pub const CANONICAL_ITEMS: &[&str] = &[
    "1h Axe",
    "1h Black Jack",
    "1h Hammer",
    "1h Ranged Hand Crossbow",
    "1h Ranged Orb",
    "1h Ranged Revolver",
    "1h Ranged Wand",
    "1h Sword",
    "1h Tiger Claws",
    "2h Axe",
    "2h Hammer",
    "2h Ranged Long Bow",
    "2h Ranged Runestaff",
    "2h Ranged Shotgun",
    "2h Ranged Sniper Rifle",
    "2h Sword",
    "Bag of Sewn Evil-Eye",
    "Ballista",
    "Battering Ram",
    "Berry",
    "Black Jack",
    "Bouncy Bomb Ammo",
    "Bouncy Bomb Launcher",
    "Bow",
    "Branch",
    "Catapult",
    "Cestus",
    "Club",
    "Cooked Meat",
    "Crossbow",
    "Crossbow Bolt",
    "Crude Bow",
    "Dagger",
    "Dragonflight Haversack",
    "Elder Wood",
    "Flint",
    "Flint Arrow",
    "Flint Spear",
    "Frying Pan",
    "Greataxe",
    "Greatsword",
    "Halberd",
    "Hammer",
    "Hand Crossbow",
    "Handaxe",
    "Holy Mackerel",
    "Honey",
    "Iron Boots",
    "Iron Chestplate",
    "Iron Greaves",
    "Iron Helmet",
    "Iron Ingot",
    "Javelin",
    "Knuckle-Duster",
    "Leather Bag",
    "Leather Scraps",
    "Long Bow",
    "Longbow",
    "Longsword",
    "LooseStone",
    "Maul",
    "Orb",
    "Pickaxe",
    "Polearm Javelin - Thrown",
    "Polearm Spear",
    "Polearm Trident",
    "Rapier",
    "Resin",
    "Revolver",
    "Revolver Ammo",
    "Runestaff",
    "Shotgun",
    "Shotgun Shell",
    "Shralok Pack",
    "Small Pouch",
    "Sniper Ammo",
    "Sniper Rifle",
    "Spear",
    "Stone",
    "Stone Axe",
    "Tiger Claws",
    "Torch",
    "Traveler's Backpack",
    "Trebuchet",
    "Trident",
    "Unarmed",
    "Wand",
    "Warhammer",
    "Wood",
    "Wood Arrow",
    "Wooden Shield",
];

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CameraModeType {
    #[default]
    Fps,
    Rts,
}

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceNodeType {
    Bush,
    Branch,
    Flint,
    LooseStone,
    Tree,
    Rock,
}

impl ResourceNodeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bush => "Bush",
            Self::Branch => "Branch",
            Self::Flint => "Flint",
            Self::LooseStone => "LooseStone",
            Self::Tree => "Tree",
            Self::Rock => "Rock",
        }
    }
}

#[table(accessor = global_state, public)]
#[derive(Clone)]
pub struct GlobalState {
    #[primary_key] pub id: u32,
    pub time_of_day: f32,
    #[default(0)]
    pub last_npc_check: u64,
}

#[table(accessor = waypoint, public)]
#[derive(Clone)]
pub struct Waypoint {
    #[primary_key] #[auto_inc] pub waypoint_id: u64,
    pub commander_id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub order_type: String,
    #[index(btree)]
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
    pub camera_mode: CameraModeType,
    pub in_interior: bool,
}

#[derive(SpacetimeType, Clone, Debug, PartialEq, Eq)]
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
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[table(accessor = nav_event, public)]
#[derive(Clone)]
pub struct NavEvent {
    #[primary_key] #[auto_inc] pub id: u64,
    pub min_x: f32,
    pub min_y: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub max_z: f32,
}

#[derive(SpacetimeType, Clone, Debug, PartialEq, Eq)]
pub struct RecipeIngredient {
    pub item_type: String,
    pub count: u32,
}

#[table(accessor = recipe_definition, public)]
#[derive(Clone)]
pub struct RecipeDefinition {
    #[primary_key]
    pub recipe_id: String,
    pub output_item: String,
    pub output_count: u32,
    pub required_station: String,
    pub requires_roof: bool,
    pub ingredients: Vec<RecipeIngredient>,
}

// ----------------------------------------------------------------------------
// INVENTORY UTILITIES
// ----------------------------------------------------------------------------

pub fn ensure_inventory_capacity(inventory: &mut Inventory) {
    while inventory.slots.len() < 16 {
        inventory.slots.push(InventorySlot {
            item_type: String::new(),
            count: 0,
        });
    }
}

pub fn add_item(inventory: &mut Inventory, item_type: &str, mut amount: u32) {
    if item_type.is_empty() || amount == 0 {
        return;
    }

    if !inventory.discovered_items.iter().any(|d| d == item_type) {
        inventory.discovered_items.push(item_type.to_string());
    }

    ensure_inventory_capacity(inventory);

    for slot in inventory.slots.iter_mut() {
        if slot.item_type == item_type && slot.count > 0 && slot.count < 50 {
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

    if amount > 0 {
        for slot in inventory.slots.iter_mut() {
            if slot.count == 0 || slot.item_type.is_empty() {
                let add_amt = amount.min(50);
                slot.item_type = item_type.to_string();
                slot.count = add_amt;
                amount -= add_amt;
                if amount == 0 {
                    break;
                }
            }
        }
    }
}

pub fn remove_item(inventory: &mut Inventory, item_type: &str, mut amount: u32) -> bool {
    ensure_inventory_capacity(inventory);

    let total: u32 = inventory.slots.iter()
        .filter(|s| s.item_type == item_type && s.count > 0)
        .map(|s| s.count)
        .sum();

    if total < amount {
        return false;
    }

    for slot in inventory.slots.iter_mut() {
        if slot.item_type == item_type && slot.count > 0 {
            if slot.count >= amount {
                slot.count -= amount;
                if slot.count == 0 {
                    slot.item_type.clear();
                }
                break;
            } else {
                amount -= slot.count;
                slot.count = 0;
                slot.item_type.clear();
            }
        }
    }
    true
}

pub fn has_item(inventory: &Inventory, item_type: &str, amount: u32) -> bool {
    let total: u32 = inventory.slots.iter()
        .filter(|s| s.item_type == item_type && s.count > 0)
        .map(|s| s.count)
        .sum();
    total >= amount
}

// ----------------------------------------------------------------------------
// AUTHORITATIVE WORLD GENERATION & NPC POPULATION ASSURANCE
// ----------------------------------------------------------------------------

// Architectural Note: Autonomous NPC Population Manager.
// Decoupled from resource_node counts to guarantee that Deer, Boars, Goblins,
// and Peasants remain present in the world. Positions initial groups within visible
// perimeter of player origin (12m - 45m) so creatures are immediately observable.
pub fn ensure_npc_population(ctx: &ReducerContext) {
    let current_count = ctx.db.npc_brain().iter().count();
    if current_count >= 16 {
        return;
    }

    let mut seed = (ctx.timestamp.to_micros_since_unix_epoch() as u64) ^ 0x5EED_A110;
    let mut base_id = (ctx.timestamp.to_micros_since_unix_epoch() as u64) << 10;

    let archetypes = [
        // Immediate Visible Perimeter (12m - 35m from origin)
        (AiType::Deer, 35.0, Faction::Wildlife, 14.0, 16.0),
        (AiType::Deer, 35.0, Faction::Wildlife, -16.0, 14.0),
        (AiType::Deer, 35.0, Faction::Wildlife, 20.0, -18.0),
        (AiType::Boar, 65.0, Faction::Wildlife, -22.0, -16.0),
        (AiType::Boar, 65.0, Faction::Wildlife, 26.0, 22.0),
        (AiType::Goblin, 50.0, Faction::Goblin, -32.0, 26.0),
        (AiType::Goblin, 50.0, Faction::Goblin, 34.0, -28.0),
        (AiType::Peasant, 60.0, Faction::Villager, 4.0, 12.0),
        (AiType::Peasant, 60.0, Faction::Villager, -8.0, 16.0),
    ];

    for (ai_type, hp_val, faction, def_x, def_z) in archetypes {
        base_id += 1;
        let entity_id = base_id;

        let jx = (prng(&mut seed) * 8.0) - 4.0;
        let jz = (prng(&mut seed) * 8.0) - 4.0;
        let sx = def_x + jx;
        let sz = def_z + jz;
        let sy = get_terrain_height(sx, sz) + 1.05;

        ctx.db.transform().insert(movement::Transform {
            entity_id,
            x: sx,
            y: sy,
            z: sz,
            chunk_x: (sx / 50.0).floor() as i32,
            chunk_z: (sz / 50.0).floor() as i32,
            last_processed_tick: 0,
        });

        ctx.db.health().insert(combat::Health { entity_id, current: hp_val, max: hp_val });
        ctx.db.faction_component().insert(combat::FactionComponent { entity_id, faction });
        ctx.db.npc_brain().insert(crate::ai::NpcBrain {
            entity_id,
            ai_type,
            state: BrainState::Idle,
            target_id: None,
            timer: 0.0,
            home_x: sx,
            home_z: sz,
            wander_x: sx,
            wander_z: sz,
        });

        if ai_type == AiType::Peasant {
            ctx.db.peasant().insert(crate::ai::Peasant {
                entity_id,
                owner_id: 0,
                state: crate::ai::AiState::Idle,
                carrying_item: "None".to_string(),
                carrying_amount: 0,
                last_harvest_target: None,
                consecutive_stuck_ticks: 0,
                auto_gather_type: "None".to_string(),
            });
        }
    }

    // Secondary ring (40m - 70m)
    for i in 0..7 {
        base_id += 1;
        let entity_id = base_id;
        let angle = prng(&mut seed) * std::f32::consts::TAU;
        let dist = 42.0 + prng(&mut seed) * 28.0;
        let sx = angle.cos() * dist;
        let sz = angle.sin() * dist;
        let sy = get_terrain_height(sx, sz) + 1.05;

        let (ai_type, hp_val, faction) = match i % 3 {
            0 => (AiType::Deer, 35.0, Faction::Wildlife),
            1 => (AiType::Boar, 65.0, Faction::Wildlife),
            _ => (AiType::Goblin, 50.0, Faction::Goblin),
        };

        ctx.db.transform().insert(movement::Transform {
            entity_id,
            x: sx,
            y: sy,
            z: sz,
            chunk_x: (sx / 50.0).floor() as i32,
            chunk_z: (sz / 50.0).floor() as i32,
            last_processed_tick: 0,
        });

        ctx.db.health().insert(combat::Health { entity_id, current: hp_val, max: hp_val });
        ctx.db.faction_component().insert(combat::FactionComponent { entity_id, faction });
        ctx.db.npc_brain().insert(crate::ai::NpcBrain {
            entity_id,
            ai_type,
            state: BrainState::Idle,
            target_id: None,
            timer: 0.0,
            home_x: sx,
            home_z: sz,
            wander_x: sx,
            wander_z: sz,
        });
    }

    log::debug!("NPC Population Reconciled: 16 active world creatures confirmed.");
}

// ----------------------------------------------------------------------------
// LIFECYCLE REDUCERS
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

    ctx.db.global_state().insert(GlobalState { id: 0, time_of_day: 8.0, last_npc_check: 0 });

    seed_authoritative_recipes(ctx);
    ensure_npc_population(ctx);
}

fn seed_authoritative_recipes(ctx: &ReducerContext) {
    let recipes = [
        RecipeDefinition {
            recipe_id: "Hammer".to_string(),
            output_item: "Hammer".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 1 },
                RecipeIngredient { item_type: "LooseStone".to_string(), count: 1 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Stone Axe".to_string(),
            output_item: "Stone Axe".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 1 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 1 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Pickaxe".to_string(),
            output_item: "Pickaxe".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 2 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Club".to_string(),
            output_item: "Club".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Torch".to_string(),
            output_item: "Torch".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 1 },
                RecipeIngredient { item_type: "Resin".to_string(), count: 1 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Crude Bow".to_string(),
            output_item: "Crude Bow".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 10 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 4 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Flint Arrow".to_string(),
            output_item: "Flint Arrow".to_string(),
            output_count: 20,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 8 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Wood Arrow".to_string(),
            output_item: "Wood Arrow".to_string(),
            output_count: 20,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 8 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Wooden Shield".to_string(),
            output_item: "Wooden Shield".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 10 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Flint Spear".to_string(),
            output_item: "Flint Spear".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 5 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Bow".to_string(),
            output_item: "Bow".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 12 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 4 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Crossbow".to_string(),
            output_item: "Crossbow".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 15 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 5 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 4 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Hand Crossbow".to_string(),
            output_item: "Hand Crossbow".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 8 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 3 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Revolver".to_string(),
            output_item: "Revolver".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "LooseStone".to_string(), count: 10 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 8 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 4 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Shotgun".to_string(),
            output_item: "Shotgun".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 15 },
                RecipeIngredient { item_type: "LooseStone".to_string(), count: 10 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 6 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Crossbow Bolt".to_string(),
            output_item: "Crossbow Bolt".to_string(),
            output_count: 15,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 6 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 3 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Revolver Ammo".to_string(),
            output_item: "Revolver Ammo".to_string(),
            output_count: 12,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "LooseStone".to_string(), count: 4 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 2 },
                RecipeIngredient { item_type: "Resin".to_string(), count: 1 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Shotgun Shell".to_string(),
            output_item: "Shotgun Shell".to_string(),
            output_count: 8,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 4 },
                RecipeIngredient { item_type: "LooseStone".to_string(), count: 3 },
                RecipeIngredient { item_type: "Resin".to_string(), count: 1 },
            ],
        },
    ];

    for recipe in recipes {
        ctx.db.recipe_definition().insert(recipe);
    }
}

#[reducer]
pub fn high_frequency_tick(ctx: &ReducerContext, _timer: HighFrequencyTimer) {
    // P1 Fix: Rebuild spatial grid at start of tick for broad-phase optimization
    crate::spatial::rebuild_spatial_grid(ctx);
    
    // RAPiER CACHE REBUILD: Build stateless QueryPipeline from DB state
    crate::physics::rebuild_physics_cache(ctx);
    
    crate::combat::process_projectiles_tick(ctx, 0.016);
}

#[reducer]
pub fn low_frequency_tick(ctx: &ReducerContext, _timer: LowFrequencyTimer) {
    crate::ai::process_ai_tick(ctx);
    crate::ai::process_npc_brain_tick(ctx, 0.1);

    if let Some(mut state) = ctx.db.global_state().id().find(0) {
        state.time_of_day += 0.005;
        if state.time_of_day >= 24.0 {
            state.time_of_day -= 24.0;
        }

        // P2 Fix: Throttle NPC population checks to every 10 seconds instead of every 100ms tick
        // This relies on the deterministic GlobalState rather than an AtomicU64 to preserve rollback safety.
        let now_micros = ctx.timestamp.to_micros_since_unix_epoch() as u64;
        if now_micros.saturating_sub(state.last_npc_check) > 10_000_000 {
            state.last_npc_check = now_micros;
            if ctx.db.npc_brain().iter().count() < 8 {
                ensure_npc_population(ctx);
            }
        }

        ctx.db.global_state().id().update(state);
    }

    let now = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    let expired_ids: Vec<u64> = ctx.db.waypoint().iter()
        .filter(|w| w.expires_at < now)
        .map(|w| w.waypoint_id)
        .collect();

    for id in expired_ids {
        ctx.db.waypoint().waypoint_id().delete(id);
    }
}

pub fn spawn_world_resource_nodes(ctx: &ReducerContext) {
    let existing: Vec<u64> = ctx.db.resource_node().iter().map(|n| n.node_id).collect();
    for id in existing {
        ctx.db.resource_node().node_id().delete(id);
    }

    let mut seed = ctx.timestamp.to_micros_since_unix_epoch() as u64;
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

    // 1. Generate 144-192 forest centers across the world map (further doubled groves)
    let num_forests = 144 + (prng(&mut seed) * 48.0) as usize;
    let mut forest_centers: Vec<(f32, f32, f32, &'static str)> = Vec::with_capacity(num_forests);

    for _ in 0..num_forests {
        let fx = (prng(&mut seed) * 360.0) - 180.0;
        let fz = (prng(&mut seed) * 360.0) - 180.0;
        let fr = 24.0 + prng(&mut seed) * 36.0; // 24-60m radius
        let fy = get_terrain_height(fx, fz);
        let biome = get_biome(fy);
        // Species clump grouping: Each forest has a dominant species archetype!
        let species = match biome {
            Biome::Lowland => if prng(&mut seed) < 0.60 { "Oak" } else { "Round" },
            Biome::Hill => if prng(&mut seed) < 0.70 { "Pine" } else { "Oak" },
            Biome::Mountain => if prng(&mut seed) < 0.75 { "Pine" } else { "Dead" },
        };
        forest_centers.push((fx, fz, fr, species));
    }

    // 2. Spawn trees around forest centers with gaussian falloff (dense groves: 280-460 trees per cluster)
    for &(fx, fz, fr, dominant_species) in &forest_centers {
        let cluster_size = (280.0 + prng(&mut seed) * 180.0) as usize;

        for _ in 0..cluster_size {
            let angle = prng(&mut seed) * std::f32::consts::TAU;
            let dist = gaussian_rand(&mut seed).abs() * fr;
            let rx = fx + angle.cos() * dist;
            let rz = fz + angle.sin() * dist;

            if rx.abs() > 195.0 || rz.abs() > 195.0 {
                continue;
            }

            let zone = get_zone(dist, fr);
            let req_spacing = get_spacing(zone, &mut seed) * 0.25;

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
                let tool = if sc > 1.2 { "Stone Axe" } else { "None" };
                let health = (3.0 * sc).max(1.0) as u32;

                // Clump species coherence: 85% dominant species of this grove, 15% understory mix
                let tree_type = if prng(&mut seed) < 0.85 {
                    match dominant_species {
                        "Oak" => "Tree:Oak",
                        "Pine" => "Tree:Pine",
                        "Round" => "Tree:Round",
                        _ => "Tree:Dead",
                    }
                } else {
                    match biome {
                        Biome::Lowland => if prng(&mut seed) < 0.5 { "Tree:Round" } else { "Tree:Oak" },
                        Biome::Hill => if prng(&mut seed) < 0.6 { "Tree:Pine" } else { "Tree:Oak" },
                        Biome::Mountain => if prng(&mut seed) < 0.7 { "Tree:Pine" } else { "Tree:Dead" },
                    }
                };

                ctx.db.resource_node().insert(ResourceNode {
                    node_id: 0,
                    node_type: tree_type.into(),
                    x: rx,
                    y: ry,
                    z: rz,
                    chunk_x: (rx / 50.0).floor() as i32,
                    chunk_z: (rz / 50.0).floor() as i32,
                    health,
                    scale: sc,
                    required_tool: tool.into(),
                });
                insert_spatial(rx, rz, req_spacing, &mut grid);
                total_spawned += 1;

                // Understory vegetation in ForestCore
                if zone == Zone::ForestCore && prng(&mut seed) < 0.30 {
                    let ux = rx + (prng(&mut seed) * 3.0 - 1.5);
                    let uz = rz + (prng(&mut seed) * 3.0 - 1.5);
                    if ux.abs() <= 195.0 && uz.abs() <= 195.0 && !check_overlap(ux, uz, 1.0, &grid) {
                        let uy = get_terrain_height(ux, uz);
                        if uy > 1.5 && uy < 25.0 {
                            let u_type = if prng(&mut seed) < 0.6 { "Bush" } else { "Branch" };
                            ctx.db.resource_node().insert(ResourceNode {
                                node_id: 0,
                                node_type: u_type.into(),
                                x: ux,
                                y: uy,
                                z: uz,
                                chunk_x: (ux / 50.0).floor() as i32,
                                chunk_z: (uz / 50.0).floor() as i32,
                                health: 1,
                                scale: 0.8,
                                required_tool: "None".into(),
                            });
                            insert_spatial(ux, uz, 1.0, &mut grid);
                            total_spawned += 1;
                        }
                    }
                }

                // Fallen logs in clearings
                if zone == Zone::Clearing && prng(&mut seed) < 0.12 {
                    let lx = rx + (prng(&mut seed) * 6.0 - 3.0);
                    let lz = rz + (prng(&mut seed) * 6.0 - 3.0);
                    if lx.abs() <= 195.0 && lz.abs() <= 195.0 && !check_overlap(lx, lz, 2.4, &grid) {
                        let ly = get_terrain_height(lx, lz);
                        if ly > 1.5 && ly < 25.0 {
                            ctx.db.resource_node().insert(ResourceNode {
                                node_id: 0,
                                node_type: "FallenLog".into(),
                                x: lx,
                                y: ly,
                                z: lz,
                                chunk_x: (lx / 50.0).floor() as i32,
                                chunk_z: (lz / 50.0).floor() as i32,
                                health: 2,
                                scale: 1.0,
                                required_tool: "None".into(),
                            });
                            insert_spatial(lx, lz, 2.4, &mut grid);
                            total_spawned += 1;
                        }
                    }
                }
            }
        }
    }

    // 3. Add scattered lone trees outside forests (5,120 lone trees, doubled from 2,560)
    for _ in 0..5120 {
        let rx = (prng(&mut seed) * 380.0) - 190.0;
        let rz = (prng(&mut seed) * 380.0) - 190.0;
        let req_spacing = get_spacing(Zone::Open, &mut seed) * 0.25;

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
            let tool = if sc > 1.2 { "Stone Axe" } else { "None" };
            let health = (3.0 * sc).max(1.0) as u32;

            let tree_type = match biome {
                Biome::Lowland => if prng(&mut seed) < 0.5 { "Tree:Oak" } else { "Tree:Round" },
                Biome::Hill => if prng(&mut seed) < 0.6 { "Tree:Pine" } else { "Tree:Oak" },
                Biome::Mountain => if prng(&mut seed) < 0.7 { "Tree:Pine" } else { "Tree:Dead" },
            };

            ctx.db.resource_node().insert(ResourceNode {
                node_id: 0,
                node_type: tree_type.into(),
                x: rx,
                y: ry,
                z: rz,
                chunk_x: (rx / 50.0).floor() as i32,
                chunk_z: (rz / 50.0).floor() as i32,
                health,
                scale: sc,
                required_tool: tool.into(),
            });
            insert_spatial(rx, rz, req_spacing, &mut grid);
            total_spawned += 1;
        }
    }

    // 4. Spawn non-tree mineral and foraging resources up to target density (~48,000 nodes)
    let mut attempts = 0;
    while total_spawned < 48000 && attempts < 56000 {
        attempts += 1;
        let rx = (prng(&mut seed) * 390.0) - 195.0;
        let rz = (prng(&mut seed) * 390.0) - 195.0;
        let req_spacing = 2.4;

        if check_overlap(rx, rz, req_spacing, &grid) {
            continue;
        }

        let ry = get_terrain_height(rx, rz);
        if ry > 1.5 && ry < 25.0 {
            let biome = get_biome(ry);
            let type_roll = prng(&mut seed);

            let (node_type, scale, health, req_tool) = if biome == Biome::Mountain || ry > 14.0 {
                let sc = 0.6 + prng(&mut seed) * 2.4;
                ("Rock", sc, (4.0 * sc) as u32, "Pickaxe")
            } else if type_roll < 0.22 {
                ("Flint", 0.6, 1, "None")
            } else if type_roll < 0.44 {
                ("LooseStone", 0.6, 1, "None")
            } else if type_roll < 0.66 {
                ("Branch", 0.6, 1, "None")
            } else if type_roll < 0.88 {
                ("Bush", 1.0, 1, "None")
            } else {
                let sc = 0.5 + prng(&mut seed) * 1.8;
                ("Rock", sc, (4.0 * sc) as u32, "Pickaxe")
            };

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
                required_tool: req_tool.into(),
            });
            insert_spatial(rx, rz, req_spacing, &mut grid);
            total_spawned += 1;
        }
    }
}

#[reducer]
pub fn admin_respawn_resources(ctx: &ReducerContext) {
    spawn_world_resource_nodes(ctx);
}

#[spacetimedb::reducer(client_connected)]
pub fn client_connected(ctx: &ReducerContext) {
    let sender = ctx.sender();

    let node_count = ctx.db.resource_node().iter().count();
    if node_count < 28000 {
        spawn_world_resource_nodes(ctx);
    }

    if ctx.db.structure().iter().count() == 0 {
        let base_x = 0.0;
        let base_z = 15.0;
        let base_y = get_terrain_height(base_x, base_z) + 0.5;

        ctx.db.structure().insert(Structure {
            structure_id: 1,
            parent_id: None,
            piece_type: "Foundation".into(),
            stability: 100,
            is_grounded: true,
            x: base_x,
            y: base_y,
            z: base_z,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            rot_w: 1.0,
            owner_id: 0,
            is_blueprint: false,
            construction_progress: 100,
            current_health: 400.0,
            max_health: 400.0,
        });

        ctx.db.structure().insert(Structure {
            structure_id: 2,
            parent_id: Some(1),
            piece_type: "Workbench".into(),
            stability: 100,
            is_grounded: false,
            x: base_x,
            y: base_y + 0.6,
            z: base_z,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            rot_w: 1.0,
            owner_id: 0,
            is_blueprint: false,
            construction_progress: 100,
            current_health: 150.0,
            max_health: 150.0,
        });
    }

    ensure_npc_population(ctx);

    if ctx.db.voxel_chunk().iter().count() == 0 {
        for cz in -2..=2 {
            for cy in 0..=3 {
                for cx in -2..=2 {
                    crate::voxel::ensure_or_create_chunk(ctx, cx, cy, cz);
                }
            }
        }
    }

    if let Some(mut player) = ctx.db.player().identity().find(sender) {
        player.is_online = true;
        let entity_id = player.entity_id;
        ctx.db.player().entity_id().update(player);

        if ctx.db.player_session().identity().find(sender).is_none() {
            ctx.db.player_session().insert(movement::PlayerSession {
                identity: sender,
                entity_id,
            });
        }

        if ctx.db.transform().entity_id().find(entity_id).is_none() {
            let spawn_y = get_terrain_height(0.0, 0.0) + 1.05;
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

        if ctx.db.weapon_skill().entity_id().find(entity_id).is_none() {
            ctx.db.weapon_skill().insert(combat::WeaponSkill {
                entity_id,
                generic_physical: 10,
                edged_xp: 100,
                pointed_xp: 100,
                blunt_xp: 100,
                two_handed_xp: 100,
                polearm_xp: 100,
                brawling_xp: 100,
                missile_xp: 100,
                firearm_xp: 100,
                runestaff_xp: 100,
            });
        }

        if ctx.db.equipment_loadout().entity_id().find(entity_id).is_none() {
            ctx.db.equipment_loadout().insert(combat::EquipmentLoadout {
                entity_id,
                main_hand: "None".to_string(),
                off_hand: "None".to_string(),
            });
        }

        if let Some(mut inv) = ctx.db.inventory().entity_id().find(entity_id) {
            ensure_inventory_capacity(&mut inv);
            ctx.db.inventory().entity_id().update(inv);
        } else {
            let mut slots = Vec::with_capacity(16);
            for _ in 0..16 {
                slots.push(InventorySlot { item_type: String::new(), count: 0 });
            }
            ctx.db.inventory().insert(Inventory {
                entity_id,
                slots,
                discovered_items: Vec::new(),
            });
        }
    } else {
        let inserted_player = ctx.db.player().insert(Player {
            entity_id: 0, identity: sender, is_online: true
        });

        let entity_id = inserted_player.entity_id;
        let spawn_y = get_terrain_height(0.0, 0.0) + 1.05;

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

        ctx.db.weapon_skill().insert(combat::WeaponSkill {
            entity_id,
            generic_physical: 10,
            edged_xp: 100,
            pointed_xp: 100,
            blunt_xp: 100,
            two_handed_xp: 100,
            polearm_xp: 100,
            brawling_xp: 100,
            missile_xp: 100,
            firearm_xp: 100,
            runestaff_xp: 100,
        });

        ctx.db.equipment_loadout().insert(combat::EquipmentLoadout {
            entity_id,
            main_hand: "None".to_string(),
            off_hand: "None".to_string(),
        });

        let mut slots = Vec::with_capacity(16);
        for _ in 0..16 {
            slots.push(InventorySlot { item_type: String::new(), count: 0 });
        }

        ctx.db.inventory().insert(Inventory {
            entity_id,
            slots,
            discovered_items: Vec::new(),
        });

        ctx.db.player_perspective().insert(PlayerPerspective {
            entity_id, camera_mode: CameraModeType::Fps, in_interior: false,
        });
    }
}

#[spacetimedb::reducer(client_disconnected)]
pub fn client_disconnected(ctx: &ReducerContext) {
    let sender = ctx.sender();
    if let Some(mut player) = ctx.db.player().identity().find(sender) {
        player.is_online = false;
        let entity_id = player.entity_id;
        ctx.db.player().entity_id().update(player);
        log::debug!("Player {} disconnected. Avatar preserved in world state.", entity_id);
    }
}

// ----------------------------------------------------------------------------
// INVENTORY INTERACTION & REORGANIZATION REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn swap_inventory_slots(ctx: &ReducerContext, from_slot: u32, to_slot: u32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    if from_slot >= 16 || to_slot >= 16 {
        return Err("Slot index out of bounds.".to_string());
    }

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    ensure_inventory_capacity(&mut inv);

    let from_idx = from_slot as usize;
    let to_idx = to_slot as usize;

    if from_idx == to_idx {
        return Ok(());
    }

    if inv.slots[from_idx].item_type == inv.slots[to_idx].item_type
        && !inv.slots[from_idx].item_type.is_empty()
        && inv.slots[from_idx].count > 0
    {
        let current_to = inv.slots[to_idx].count;
        let available = 50u32.saturating_sub(current_to);
        if available > 0 {
            let move_amt = inv.slots[from_idx].count.min(available);
            inv.slots[to_idx].count += move_amt;
            inv.slots[from_idx].count -= move_amt;
            if inv.slots[from_idx].count == 0 {
                inv.slots[from_idx].item_type.clear();
            }
            ctx.db.inventory().entity_id().update(inv);
            return Ok(());
        }
    }

    inv.slots.swap(from_idx, to_idx);
    ctx.db.inventory().entity_id().update(inv);
    Ok(())
}

#[reducer]
pub fn drop_inventory_item(ctx: &ReducerContext, slot_index: u32, mut amount: u32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    if slot_index >= 16 {
        return Err("Slot index out of bounds.".to_string());
    }

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    ensure_inventory_capacity(&mut inv);

    let slot_idx = slot_index as usize;
    if inv.slots[slot_idx].count == 0 || inv.slots[slot_idx].item_type.is_empty() {
        return Err("Slot is empty.".to_string());
    }

    let item_type = inv.slots[slot_idx].item_type.clone();
    if amount == 0 || amount > inv.slots[slot_idx].count {
        amount = inv.slots[slot_idx].count;
    }

    inv.slots[slot_idx].count -= amount;
    if inv.slots[slot_idx].count == 0 {
        inv.slots[slot_idx].item_type.clear();
    }
    ctx.db.inventory().entity_id().update(inv);

    let transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or_else(|| "Player transform missing.".to_string())?;

    let drop_id = ((ctx.timestamp.to_micros_since_unix_epoch() as u64) << 16)
        ^ (session.entity_id.wrapping_add(slot_index as u64 + 7777));

    let drop_x = transform.x + 1.8;
    let drop_z = transform.z + 1.8;
    let drop_y = get_terrain_height(drop_x, drop_z) + 0.35;

    ctx.db.harvestable_corpse().insert(crate::ai::HarvestableCorpse {
        entity_id: drop_id,
        loot_item: item_type.clone(),
        amount,
    });

    ctx.db.transform().insert(crate::movement::Transform {
        entity_id: drop_id,
        x: drop_x,
        y: drop_y,
        z: drop_z,
        chunk_x: (drop_x / 50.0).floor() as i32,
        chunk_z: (drop_z / 50.0).floor() as i32,
        last_processed_tick: 0,
    });

    ctx.db.health().insert(crate::combat::Health {
        entity_id: drop_id,
        current: 1.0,
        max: 1.0,
    });

    log::debug!("Player {} dropped {}x '{}' into world.", session.entity_id, amount, item_type);
    Ok(())
}

// ----------------------------------------------------------------------------
// ADMIN PLAYTESTING CONSOLE REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn admin_give_item(ctx: &ReducerContext, item_type: String, amount: u32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    if !CANONICAL_ITEMS.contains(&item_type.as_str()) {
        return Err(format!(
            "Invalid item format '{}'. Item must match canonical casing exactly: {:?}",
            item_type, CANONICAL_ITEMS
        ));
    }

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    add_item(&mut inv, &item_type, amount);
    ctx.db.inventory().entity_id().update(inv);

    log::debug!("ADMIN: Player {} granted {}x '{}'", session.entity_id, amount, item_type);
    Ok(())
}

#[reducer]
pub fn admin_teleport(ctx: &ReducerContext, x: f32, z: f32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let mut transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or_else(|| "Transform not found.".to_string())?;

    transform.x = x;
    transform.z = z;
    transform.y = get_terrain_height(x, z) + 1.05;
    transform.chunk_x = (x / 50.0).floor() as i32;
    transform.chunk_z = (z / 50.0).floor() as i32;

    ctx.db.transform().entity_id().update(transform);
    log::debug!("ADMIN: Player {} teleported to ({:.1}, {:.1})", session.entity_id, x, z);
    Ok(())
}

#[reducer]
pub fn admin_heal(ctx: &ReducerContext, amount: f32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let mut hp = ctx.db.health().entity_id().find(session.entity_id)
        .ok_or_else(|| "Health record missing.".to_string())?;

    if amount <= 0.0 {
        hp.current = hp.max;
    } else {
        hp.current = (hp.current + amount).min(hp.max);
    }

    ctx.db.health().entity_id().update(hp);
    log::debug!("ADMIN: Player {} healed to {:.0} HP", session.entity_id, amount);
    Ok(())
}

#[reducer]
pub fn admin_god_mode(ctx: &ReducerContext) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let mut hp = ctx.db.health().entity_id().find(session.entity_id)
        .ok_or_else(|| "Health record missing.".to_string())?;

    hp.max = 99999.0;
    hp.current = 99999.0;
    ctx.db.health().entity_id().update(hp);

    log::debug!("ADMIN: God mode enabled for player {}", session.entity_id);
    Ok(())
}

#[reducer]
pub fn admin_set_time(ctx: &ReducerContext, time_of_day: f32) -> Result<(), String> {
    let mut state = ctx.db.global_state().id().find(0)
        .ok_or_else(|| "Global state missing.".to_string())?;

    state.time_of_day = time_of_day.rem_euclid(24.0);
    ctx.db.global_state().id().update(state);

    log::debug!("ADMIN: World time updated to {:.1}", time_of_day);
    Ok(())
}

#[reducer]
pub fn admin_clear_inventory(ctx: &ReducerContext) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    inv.slots.clear();
    ensure_inventory_capacity(&mut inv);
    ctx.db.inventory().entity_id().update(inv);

    log::debug!("ADMIN: Player {} inventory cleared.", session.entity_id);
    Ok(())
}

#[reducer]
pub fn admin_spawn_npc(ctx: &ReducerContext, ai_type_str: String, count: u32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let p_transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or_else(|| "Transform missing.".to_string())?;

    let ai_type = match ai_type_str.to_lowercase().as_str() {
        "boar" => AiType::Boar,
        "goblin" => AiType::Goblin,
        "peasant" => AiType::Peasant,
        _ => AiType::Deer,
    };

    let mut seed = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    let spawn_count = count.clamp(1, 25);

    for i in 0..spawn_count {
        let offset_x = (prng(&mut seed) * 12.0) - 6.0;
        let offset_z = (prng(&mut seed) * 12.0) - 6.0;
        let sx = p_transform.x + offset_x;
        let sz = p_transform.z + offset_z;
        let sy = get_terrain_height(sx, sz) + 1.05;

        let entity_id = ((ctx.timestamp.to_micros_since_unix_epoch() as u64) << 12)
            ^ (session.entity_id.wrapping_add(i as u64 * 313 + 555));

        ctx.db.transform().insert(movement::Transform {
            entity_id,
            x: sx,
            y: sy,
            z: sz,
            chunk_x: (sx / 50.0).floor() as i32,
            chunk_z: (sz / 50.0).floor() as i32,
            last_processed_tick: 0,
        });

        let hp_val = match ai_type {
            AiType::Boar => 65.0,
            AiType::Goblin => 50.0,
            AiType::Peasant => 60.0,
            _ => 35.0,
        };

        ctx.db.health().insert(combat::Health { entity_id, current: hp_val, max: hp_val });

        let faction = match ai_type {
            AiType::Goblin => Faction::Goblin,
            AiType::Peasant => Faction::Villager,
            _ => Faction::Wildlife,
        };
        ctx.db.faction_component().insert(combat::FactionComponent { entity_id, faction });

        ctx.db.npc_brain().insert(crate::ai::NpcBrain {
            entity_id,
            ai_type,
            state: BrainState::Idle,
            target_id: None,
            timer: 0.0,
            home_x: sx,
            home_z: sz,
            wander_x: sx,
            wander_z: sz,
        });

        if ai_type == AiType::Peasant {
            ctx.db.peasant().insert(crate::ai::Peasant {
                entity_id,
                owner_id: session.entity_id,
                state: crate::ai::AiState::Idle,
                carrying_item: "None".to_string(),
                carrying_amount: 0,
                last_harvest_target: None,
                consecutive_stuck_ticks: 0,
                auto_gather_type: "None".to_string(),
            });
        }
    }

    log::debug!("ADMIN: Spawned {}x {:?} around player {}", spawn_count, ai_type, session.entity_id);
    Ok(())
}

#[reducer]
pub fn admin_detonate(ctx: &ReducerContext, radius: f32, power: f32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let p_transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or_else(|| "Transform missing.".to_string())?;

    let safe_radius = radius.clamp(1.0, 16.0);
    crate::voxel::mutate_voxel_sphere(ctx, p_transform.x, p_transform.y - 0.5, p_transform.z, safe_radius, power);

    ctx.db.combat_event().insert(CombatEvent {
        id: 0,
        event_type: "ExplosionBlast".to_string(),
        x: p_transform.x,
        y: p_transform.y,
        z: p_transform.z,
    });

    log::debug!("ADMIN: Blast of radius {:.1} detonated at player {}", safe_radius, session.entity_id);
    Ok(())
}

#[reducer]
pub fn admin_kill_all_npcs(ctx: &ReducerContext) -> Result<(), String> {
    let victim_ids: Vec<u64> = ctx.db.npc_brain().iter()
        .map(|b| b.entity_id)
        .collect();

    for id in victim_ids {
        ctx.db.npc_brain().entity_id().delete(id);
        ctx.db.health().entity_id().delete(id);
        ctx.db.transform().entity_id().delete(id);
        ctx.db.faction_component().entity_id().delete(id);
        if ctx.db.peasant().entity_id().find(id).is_some() {
            ctx.db.peasant().entity_id().delete(id);
        }
        if ctx.db.pet_component().entity_id().find(id).is_some() {
            ctx.db.pet_component().entity_id().delete(id);
        }
    }

    log::debug!("ADMIN: Cleaned all active NPC brain entities.");
    Ok(())
}

// ----------------------------------------------------------------------------
// PROGRESSION & INTERACTION REDUCERS
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
pub fn craft_item(ctx: &ReducerContext, recipe_id: String) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let recipe = ctx.db.recipe_definition().recipe_id().find(&recipe_id)
        .ok_or_else(|| format!("Unknown crafting recipe: {}", recipe_id))?;

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    let player_transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or_else(|| "Transform not found.".to_string())?;

    if recipe.required_station != "None" {
        let mut valid_station = false;
        for s in ctx.db.structure().iter().filter(|s| s.piece_type == recipe.required_station && !s.is_blueprint) {
            let dist_sq = (s.x - player_transform.x).powi(2) + (s.z - player_transform.z).powi(2);
            if dist_sq <= 400.0 {
                if !recipe.requires_roof || crate::building::is_covered(ctx, s.x, s.y, s.z) {
                    valid_station = true;
                    break;
                }
            }
        }
        if !valid_station {
            return Err(format!(
                "Crafting '{}' requires an active constructed {} nearby.",
                recipe.output_item, recipe.required_station
            ));
        }
    }

    for ing in &recipe.ingredients {
        if !has_item(&inv, &ing.item_type, ing.count) {
            return Err(format!("Missing materials: {}x {}.", ing.count, ing.item_type));
        }
    }

    for ing in &recipe.ingredients {
        remove_item(&mut inv, &ing.item_type, ing.count);
    }

    add_item(&mut inv, &recipe.output_item, recipe.output_count);
    ctx.db.inventory().entity_id().update(inv);

    log::debug!(
        "Player {} authoritatively crafted {}x {}",
        session.entity_id, recipe.output_count, recipe.output_item
    );
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

    match node.node_type.as_str() {
        "Bush" => {
            if node.health == 0 {
                return Err("Berries depleted.".into());
            }
            add_item(&mut inventory, "Berry", 2);
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
        "Branch" => {
            add_item(&mut inventory, "Branch", 1);
            ctx.db.inventory().entity_id().update(inventory);
            ctx.db.resource_node().node_id().delete(node_id);
        }
        "Flint" => {
            add_item(&mut inventory, "Flint", 1);
            ctx.db.inventory().entity_id().update(inventory);
            ctx.db.resource_node().node_id().delete(node_id);
        }
        "LooseStone" => {
            add_item(&mut inventory, "LooseStone", 1);
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

    if let Some(node) = hit_node {
        if node.required_tool == "Stone Axe" {
            let has_axe = inventory.slots.iter().any(|s| s.item_type == "Stone Axe" && s.count > 0);
            if !has_axe {
                return Ok(());
            }
        } else if node.required_tool == "Pickaxe" {
            let has_pick = inventory.slots.iter().any(|s| s.item_type == "Pickaxe" && s.count > 0);
            if !has_pick {
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

            let amount = (match node.node_type.as_str() {
                "Tree" | "FallenLog" => 6,
                "Rock" => 4,
                _ => 1,
            } as f32 * node.scale).ceil() as u32;

            let item = match node.node_type.as_str() {
                "Tree" | "FallenLog" => "Wood",
                "Rock" => "Stone",
                "Branch" => "Branch",
                "Flint" => "Flint",
                "LooseStone" => "LooseStone",
                _ => "Wood",
            };

            add_item(&mut inventory, item, amount);
            if node.node_type == "Tree" && inventory.slots.iter().any(|s| s.item_type == "Stone Axe" && s.count > 0) {
                add_item(&mut inventory, "Resin", 1);
            }
            ctx.db.inventory().entity_id().update(inventory);
        }
    }

    Ok(())
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

#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
mod native_spacetimedb_stubs {
    #[unsafe(no_mangle)] pub unsafe extern "C" fn datastore_insert_bsatn() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn datastore_update_bsatn() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn index_id_from_name() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn procedure_sleep_until() {}
    #[unsafe(no_mangle)] pub unsafe extern "C" fn bytes_sink_write() {}
    #[unsafe(no_mangle)] pub unsafe extern "C" fn bytes_source_remaining_length() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn bytes_source_read() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn table_id_from_name() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn get_jwt() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn console_log() {}
    #[unsafe(no_mangle)] pub unsafe extern "C" fn identity() {}
    #[unsafe(no_mangle)] pub unsafe extern "C" fn console_timer_start() {}
    #[unsafe(no_mangle)] pub unsafe extern "C" fn console_timer_end() {}
    #[unsafe(no_mangle)] pub unsafe extern "C" fn datastore_table_scan_bsatn() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn datastore_index_scan_point_bsatn() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn datastore_index_scan_range_bsatn() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn datastore_delete_by_index_scan_point_bsatn() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn datastore_delete_by_index_scan_range_bsatn() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn row_iter_bsatn_advance() -> u32 { 0 }
    #[unsafe(no_mangle)] pub unsafe extern "C" fn row_iter_bsatn_close() {}
}