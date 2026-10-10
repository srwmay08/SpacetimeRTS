// ----------------------------------------------------------------------------
// CORE MODULE IMPORTS & GLOBAL SCHEMAS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Implements authoritative server state for the game.
// Features an autonomous NPC population manager ensuring all 4 mob species
// (Deer, Boars, Goblins, Peasants) remain actively populated in the world.

use spacetimedb::{table, reducer, Identity, ReducerContext, Table, ScheduleAt, SpacetimeType};
use std::time::Duration;

pub mod movement;
pub mod combat;
pub mod building;
pub mod ai;
pub mod voxel;
pub mod spatial;
pub mod physics;
pub mod armory;
pub mod bestiary;
pub mod hazard;
pub mod npc_building;
pub mod templates;
pub mod player_inventory;
pub mod crafting;
pub mod harvesting;
pub mod admin;

pub use player_inventory::*;
pub use crafting::*;
pub use harvesting::*;
pub use admin::*;

use crate::movement::{transform, player_session};
use crate::combat::{health, hitbox_history, faction_component, Faction, weapon_skill, equipment_loadout};
#[allow(unused_imports)]
use crate::ai::{npc_brain, AiType, BrainState, harvestable_corpse, peasant, pet_component, npc_state, NpcAction, NpcState};
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
    "Dragon Helm",
    "Dragonflight Haversack",
    "Elder Wood",
    "Flint",
    "Flint Arrow",
    "Flint Spear",
    "Frying Pan",
    "Golden Dragon Helm",
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
    "Potion:Healing",
    "Potion:Mana",
    "Potion:Speed",
    "Potion:Stamina",
    "Raw Meat",
    "Revolver",
    "Rifle Round",
    "Rogue's Knapsack",
    "Round Shield",
    "Rune:Earth",
    "Rune:Fire",
    "Rune:Light",
    "Rune:Water",
    "Rune:Wind",
    "Runestaff",
    "Satchel",
    "Shield",
    "Short Sword",
    "Shotgun",
    "Shotgun Shell",
    "Slingshot",
    "Small Pouch",
    "Sniper Rifle",
    "Spear",
    "Stone Axe",
    "Stone Hammer",
    "Sword",
    "Tiger Claws",
    "Torch",
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

#[table(accessor = global_state, public)]
#[derive(Clone)]
pub struct GlobalState {
    #[primary_key] pub id: u32,
    pub time_of_day: f32,
    #[default(0)]
    pub last_npc_check: u64,
    #[default(42)]
    pub world_seed: u32,
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

    let needed = 16usize.saturating_sub(ctx.db.npc_brain().iter().count());
    for _ in 0..needed {
        base_id += 1;
        let entity_id = base_id;

        let angle = prng(&mut seed) * std::f32::consts::TAU;
        let dist = 18.0 + prng(&mut seed) * 36.0;
        let sx = angle.cos() * dist;
        let sz = angle.sin() * dist;
        let sy = get_terrain_height(sx, sz) + 1.05;

        let roll = (prng(&mut seed) * 3.0) as u32;
        let (ai_type, hp_val, faction) = match roll {
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

    let initial_seed = 42;
    ctx.db.global_state().insert(GlobalState { id: 0, time_of_day: 8.0, last_npc_check: 0, world_seed: initial_seed });
    apply_world_seed(initial_seed);

    seed_authoritative_recipes(ctx);
    crate::building::seed_default_materials(ctx);
    ensure_npc_population(ctx);
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
    crate::building::process_autobuild_tick(ctx);

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

#[spacetimedb::reducer(client_connected)]
pub fn client_connected(ctx: &ReducerContext) {
    ensure_world_seed_synced(ctx);
    let sender = ctx.sender();

    let node_count = ctx.db.resource_node().iter().count();
    if node_count < 4500 {
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
            current_support: 150.0,
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
            current_support: 100.0,
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