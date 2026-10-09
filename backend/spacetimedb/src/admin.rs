// ============================================================================
// File: backend/spacetimedb/src/admin.rs
// ============================================================================
// Admin playtesting console reducers, cheat commands, and world seed manipulation.

use spacetimedb::{reducer, ReducerContext, Table};
use crate::movement::{self, player_session, transform};
use crate::combat::{self, health, hitbox_history, faction_component, Faction};
use crate::ai::{npc_brain, peasant, pet_component, AiType, BrainState};
use crate::player_inventory::{inventory, add_item, ensure_inventory_capacity};
use crate::{GlobalState, CombatEvent, CANONICAL_ITEMS, global_state, combat_event};
use crate::harvesting::{get_terrain_height, apply_world_seed, spawn_world_resource_nodes, prng};
use crate::voxel::voxel_chunk;

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
    transform.last_processed_tick = transform.last_processed_tick.wrapping_add(100_000);

    if let Some(mut history) = ctx.db.hitbox_history().entity_id().find(session.entity_id) {
        history.snapshots.clear();
        history.snapshots.push(crate::combat::Snapshot {
            tick_id: transform.last_processed_tick,
            x: transform.x,
            y: transform.y,
            z: transform.z,
        });
        ctx.db.hitbox_history().entity_id().update(history);
    }

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

#[reducer]
pub fn admin_set_world_seed(ctx: &ReducerContext, seed: u32) -> Result<(), String> {
    apply_world_seed(seed);
    if let Some(mut state) = ctx.db.global_state().id().find(0) {
        state.world_seed = seed;
        ctx.db.global_state().id().update(state);
    } else {
        ctx.db.global_state().insert(GlobalState {
            id: 0,
            time_of_day: 8.0,
            last_npc_check: 0,
            world_seed: seed,
        });
    }

    // Clear player-modified voxel chunks so excavations from previous seed don't hang in mid-air
    let chunk_keys: Vec<u64> = ctx.db.voxel_chunk().iter().map(|c| c.chunk_key).collect();
    for key in chunk_keys {
        ctx.db.voxel_chunk().chunk_key().delete(key);
    }

    // Deterministically re-spawn resource nodes based on the new seed
    spawn_world_resource_nodes(ctx);

    // Re-snap existing NPCs to the new terrain surface
    for mut transform in ctx.db.transform().iter() {
        transform.y = get_terrain_height(transform.x, transform.z) + 1.05;
        ctx.db.transform().entity_id().update(transform);
    }

    Ok(())
}

#[reducer]
pub fn admin_randomize_world_seed(ctx: &ReducerContext) -> Result<(), String> {
    let ts = ctx.timestamp.to_micros_since_unix_epoch() as u32;
    let rand_seed = (ts.wrapping_mul(1664525).wrapping_add(1013904223)) % 999_999 + 1;
    admin_set_world_seed(ctx, rand_seed)
}

#[reducer]
pub fn admin_respawn_resources(ctx: &ReducerContext) {
    spawn_world_resource_nodes(ctx);
}
