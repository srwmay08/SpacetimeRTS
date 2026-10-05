// ----------------------------------------------------------------------------
// AI IMPORTS & DEPENDENCIES (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::{transform, player_session, Transform}; 
use crate::inventory; 
use crate::resource_node;
use crate::respawn_bush_timer; 
use crate::building::structure; 
use crate::combat::{health, faction_component, Faction, active_projectile, equipment_loadout};
use crate::combat_event;

// ----------------------------------------------------------------------------
// PEASANT AI STRUCTURES
// ----------------------------------------------------------------------------

#[derive(SpacetimeType, Clone, Debug, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(SpacetimeType, Clone, Debug, PartialEq)]
pub enum AiState {
    Idle,
    MoveTo(Position),
    Harvest(u64),
    Return(u64),
    Deposit(u64),
    AutoGather(String),
}

// Architectural Note: Indexed owner_id for high-speed ownership lookups in multi-unit RTS selections.
#[table(accessor = peasant, public)]
#[derive(Clone, PartialEq)]
pub struct Peasant {
    #[primary_key]
    pub entity_id: u64,
    #[index(btree)]
    pub owner_id: u64,
    pub state: AiState,
    pub carrying_item: String,
    pub carrying_amount: u32,
    pub last_harvest_target: Option<u64>,
    pub consecutive_stuck_ticks: u32,
    pub auto_gather_type: String, 
}

// ----------------------------------------------------------------------------
// NPC STATE MACHINE & DOORWAY INTERACTION STRUCTURES
// ----------------------------------------------------------------------------
// Architectural Note: Deterministic state machine governing NPC doorway interaction
// and blueprint construction. All state progression and action intervals rely exclusively
// on ctx.timestamp without local clocks or randomized hashes.

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcAction {
    Idle,
    Patrolling,
    PathingToDoor,
    InteractingWithDoor,
    ConstructingBlueprint,
}

#[table(accessor = npc_state, public)]
#[derive(Clone, PartialEq, Debug)]
pub struct NpcState {
    #[primary_key]
    pub entity_id: u64,
    pub current_action: NpcAction,
    pub target_coords: Option<Position>,
    pub task_entity_id: Option<u64>, // ID of the door or building blueprint
    pub last_tick: u64,
}

// ----------------------------------------------------------------------------
// THREAT NPC & PET STRUCTURES
// ----------------------------------------------------------------------------

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiType { Friendly, Deer, Boar, Goblin, Peasant }

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrainState { Idle, Fleeing, Chasing, Attacking, Warning, Corpse }

// Architectural Note: Added index on ai_type and state to accelerate state-machine tick queries.
#[table(accessor = npc_brain, public)]
#[derive(Clone, PartialEq)]
pub struct NpcBrain {
    #[primary_key]
    pub entity_id: u64,
    #[index(btree)]
    pub ai_type: AiType,
    #[index(btree)]
    pub state: BrainState,
    pub target_id: Option<u64>,
    pub timer: f32, 
    pub home_x: f32,
    pub home_z: f32,
    pub wander_x: f32,
    pub wander_z: f32,
}

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetStance { Stay, Follow, Aggressive, Defensive }

#[table(accessor = pet_component, public)]
#[derive(Clone, PartialEq)]
pub struct PetComponent {
    #[primary_key]
    pub entity_id: u64,
    #[index(btree)]
    pub owner_id: u64, 
    pub stance: PetStance,
}

#[table(accessor = harvestable_corpse, public)]
#[derive(Clone, PartialEq)]
pub struct HarvestableCorpse {
    #[primary_key]
    pub entity_id: u64,
    pub loot_item: String,
    pub amount: u32,
}

// ----------------------------------------------------------------------------
// HELPER: SPATIAL CHUNK FILTERING
// ----------------------------------------------------------------------------
// Architectural Note: Public spatial neighborhood query used by AI simulation tick
// to constrain broad-phase visibility and proximity queries to adjacent 50m chunks.
#[inline]
pub fn is_chunk_adjacent(cx1: i32, cz1: i32, cx2: i32, cz2: i32) -> bool {
    (cx1 - cx2).abs() <= 1 && (cz1 - cz2).abs() <= 1
}

// ----------------------------------------------------------------------------
// PET COMMAND REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn change_pet_stance(ctx: &ReducerContext, pet_entity_id: u64, new_stance: PetStance) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session".to_string())?;
        
    let mut pet = ctx.db.pet_component().entity_id().find(pet_entity_id)
        .ok_or_else(|| "Pet not found".to_string())?;
    
    if pet.owner_id != session.entity_id {
        return Err("Not your pet".to_string());
    }

    pet.stance = new_stance;
    ctx.db.pet_component().entity_id().update(pet);
    Ok(())
}

// ----------------------------------------------------------------------------
// PEASANT COMMAND REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn spawn_peasant(ctx: &ReducerContext) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session".to_string())?;

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found".to_string())?;

    if !crate::remove_item(&mut inv, "Wood", 20) {
        return Err("Insufficient Wood to spawn a peasant.".to_string());
    }
    
    ctx.db.inventory().entity_id().update(inv);

    let entity_id = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    let spawn_transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or_else(|| "Player transform missing".to_string())?;
        
    let mut seed = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    let offset_x = (crate::prng(&mut seed) * 4.0) - 2.0;
    let offset_z = (crate::prng(&mut seed) * 4.0) - 2.0;

    let spawn_x = spawn_transform.x + 2.0 + offset_x;
    let spawn_z = spawn_transform.z + 2.0 + offset_z;
    let spawn_y = crate::get_terrain_height(spawn_x, spawn_z) + 1.5;

    ctx.db.transform().insert(Transform {
        entity_id,
        x: spawn_x, 
        y: spawn_y, 
        z: spawn_z,
        chunk_x: (spawn_x / 50.0).floor() as i32,
        chunk_z: (spawn_z / 50.0).floor() as i32,
        last_processed_tick: 0,
    });

    ctx.db.health().insert(crate::combat::Health {
        entity_id,
        current: 50.0,
        max: 50.0,
    });

    ctx.db.faction_component().insert(crate::combat::FactionComponent {
        entity_id,
        faction: Faction::Villager,
    });

    ctx.db.npc_brain().insert(NpcBrain {
        entity_id,
        ai_type: AiType::Peasant,
        state: BrainState::Idle,
        target_id: None,
        timer: 0.0,
        home_x: spawn_x,
        home_z: spawn_z,
        wander_x: spawn_x,
        wander_z: spawn_z,
    });

    ctx.db.peasant().insert(Peasant {
        entity_id,
        owner_id: session.entity_id,
        state: AiState::Idle,
        carrying_item: "None".to_string(),
        carrying_amount: 0,
        last_harvest_target: None,
        consecutive_stuck_ticks: 0,
        auto_gather_type: "None".to_string(),
    });

    ctx.db.npc_state().insert(NpcState {
        entity_id,
        current_action: NpcAction::Idle,
        target_coords: None,
        task_entity_id: None,
        last_tick: ctx.timestamp.to_micros_since_unix_epoch() as u64,
    });
    Ok(())
}

#[reducer]
pub fn command_peasant(
    ctx: &ReducerContext,
    peasant_entity_id: u64,
    command_type: String, 
    target_x: f32, 
    target_y: f32, 
    target_z: f32,
    target_id: u64,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session".to_string())?;

    let mut peasant = ctx.db.peasant().entity_id().find(peasant_entity_id)
        .ok_or_else(|| "Peasant not found".to_string())?;

    if peasant.owner_id != session.entity_id {
        return Err("Unauthorized: You do not own this unit.".to_string());
    }

    if let Some(brain) = ctx.db.npc_brain().entity_id().find(peasant_entity_id) {
        if brain.state == BrainState::Fleeing {
            return Err("Unit is currently fleeing from enemies and cannot process commands.".to_string());
        }
    }

    peasant.state = match command_type.as_str() {
        "MoveTo" => { 
            peasant.auto_gather_type = "None".to_string(); 
            peasant.last_harvest_target = None;

            let angle = (peasant_entity_id % 12) as f32 * (std::f32::consts::PI / 6.0);
            let radius = 1.2 * ((peasant_entity_id % 3) as f32 + 1.0);
            let scattered_x = target_x + angle.cos() * radius;
            let scattered_z = target_z + angle.sin() * radius;

            AiState::MoveTo(Position { x: scattered_x, y: target_y, z: scattered_z }) 
        },
        "Harvest" => {
            peasant.auto_gather_type = "None".to_string();
            peasant.last_harvest_target = Some(target_id);
            AiState::Harvest(target_id)
        },
        "Return" => { 
            peasant.auto_gather_type = "None".to_string(); 
            peasant.last_harvest_target = None;
            AiState::Return(session.entity_id) 
        },
        "AutoTree" => { 
            peasant.auto_gather_type = "Tree".to_string(); 
            AiState::AutoGather("Tree".to_string()) 
        },
        "AutoRock" => { 
            peasant.auto_gather_type = "Rock".to_string(); 
            AiState::AutoGather("Rock".to_string()) 
        },
        "AutoBush" => { 
            peasant.auto_gather_type = "Bush".to_string(); 
            AiState::AutoGather("Bush".to_string()) 
        },
        "AutoAll"  => { 
            peasant.auto_gather_type = "All".to_string(); 
            AiState::AutoGather("All".to_string()) 
        },
        "Construct" | "Build" => {
            peasant.auto_gather_type = "None".to_string();
            peasant.last_harvest_target = None;
            let now_micros = ctx.timestamp.to_micros_since_unix_epoch() as u64;
            let target_coords = if target_x != 0.0 || target_z != 0.0 {
                Some(Position { x: target_x, y: target_y, z: target_z })
            } else {
                None
            };
            match ctx.db.npc_state().entity_id().find(peasant_entity_id) {
                Some(mut st) => {
                    st.current_action = NpcAction::ConstructingBlueprint;
                    st.task_entity_id = Some(target_id);
                    st.target_coords = target_coords;
                    st.last_tick = now_micros;
                    ctx.db.npc_state().entity_id().update(st);
                }
                None => {
                    ctx.db.npc_state().insert(NpcState {
                        entity_id: peasant_entity_id,
                        current_action: NpcAction::ConstructingBlueprint,
                        target_coords,
                        task_entity_id: Some(target_id),
                        last_tick: now_micros,
                    });
                }
            }
            AiState::Idle
        },
        "Door" | "ToggleDoor" => {
            peasant.auto_gather_type = "None".to_string();
            peasant.last_harvest_target = None;
            let now_micros = ctx.timestamp.to_micros_since_unix_epoch() as u64;
            let target_coords = if target_x != 0.0 || target_z != 0.0 {
                Some(Position { x: target_x, y: target_y, z: target_z })
            } else {
                None
            };
            match ctx.db.npc_state().entity_id().find(peasant_entity_id) {
                Some(mut st) => {
                    st.current_action = NpcAction::PathingToDoor;
                    st.task_entity_id = Some(target_id);
                    st.target_coords = target_coords;
                    st.last_tick = now_micros;
                    ctx.db.npc_state().entity_id().update(st);
                }
                None => {
                    ctx.db.npc_state().insert(NpcState {
                        entity_id: peasant_entity_id,
                        current_action: NpcAction::PathingToDoor,
                        target_coords,
                        task_entity_id: Some(target_id),
                        last_tick: now_micros,
                    });
                }
            }
            AiState::Idle
        },
        _ => { 
            peasant.auto_gather_type = "None".to_string(); 
            peasant.last_harvest_target = None;
            AiState::Idle 
        },
    };

    ctx.db.peasant().entity_id().update(peasant);
    Ok(())
}

// ----------------------------------------------------------------------------
// NPC STATE MACHINE REDUCERS (Doorways & Blueprint Construction)
// ----------------------------------------------------------------------------
// Architectural Note: Server-authoritative reducer executing deterministic AI actions.
// Timing delays (e.g. 500ms door swing, 1s hammer intervals) strictly utilize ctx.timestamp
// rather than local clocks to safeguard multi-node determinism and rollback safety.

#[reducer]
pub fn tick_npc_ai(ctx: &ReducerContext, npc_id: u64) -> Result<(), String> {
    let mut npc = ctx.db.npc_state().entity_id().find(npc_id).ok_or("NPC not found")?;
    let now_micros = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    let time_since_last_action = now_micros.saturating_sub(npc.last_tick);

    match npc.current_action {
        NpcAction::PathingToDoor => {
            if let Some(door_id) = npc.task_entity_id {
                // Check if door is closed via BTreeMap-backed database lookup
                if let Some(door) = ctx.db.structure().structure_id().find(door_id) {
                    if crate::building::base_piece_type(&door.piece_type) == "Door" {
                        let is_open = crate::building::is_door_open(ctx, door_id);
                        if !is_open {
                            crate::building::set_door_open_internal(ctx, door_id, true)?;
                            npc.current_action = NpcAction::InteractingWithDoor;
                            npc.last_tick = now_micros;
                        } else {
                            npc.current_action = NpcAction::Patrolling;
                            npc.last_tick = now_micros;
                        }
                    } else {
                        npc.current_action = NpcAction::Idle;
                        npc.task_entity_id = None;
                    }
                } else {
                    npc.current_action = NpcAction::Idle;
                    npc.task_entity_id = None;
                }
            } else {
                npc.current_action = NpcAction::Idle;
            }
        }
        NpcAction::InteractingWithDoor => {
            // Wait deterministically for the client interpolation (e.g. 500ms door swing)
            if time_since_last_action >= 500_000 {
                npc.current_action = NpcAction::Patrolling;
                npc.last_tick = now_micros;
            }
        }
        NpcAction::ConstructingBlueprint => {
            if let Some(blueprint_id) = npc.task_entity_id {
                if time_since_last_action >= 1_000_000 { // 1 second hammer intervals
                    if let Some(mut block) = ctx.db.structure().structure_id().find(blueprint_id) {
                        if block.is_blueprint {
                            block.current_health = (block.current_health + 10.0).min(block.max_health);
                            block.construction_progress = ((block.current_health / block.max_health.max(1.0)) * 100.0) as u32;

                            if block.current_health >= block.max_health || block.construction_progress >= 100 {
                                block.is_blueprint = false;
                                block.construction_progress = 100;
                                block.current_health = block.max_health;
                                npc.current_action = NpcAction::Idle; // Task complete
                                npc.task_entity_id = None;
                            }
                            ctx.db.structure().structure_id().update(block);
                        } else {
                            npc.current_action = NpcAction::Idle;
                            npc.task_entity_id = None;
                        }
                    } else {
                        npc.current_action = NpcAction::Idle;
                        npc.task_entity_id = None;
                    }
                    npc.last_tick = now_micros;
                }
            } else {
                npc.current_action = NpcAction::Idle;
            }
        }
        _ => {}
    }

    ctx.db.npc_state().entity_id().update(npc);
    Ok(())
}

#[reducer]
pub fn assign_npc_task(
    ctx: &ReducerContext,
    npc_id: u64,
    action: NpcAction,
    task_entity_id: Option<u64>,
    target_x: Option<f32>,
    target_y: Option<f32>,
    target_z: Option<f32>,
) -> Result<(), String> {
    let now_micros = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    let target_coords = match (target_x, target_y, target_z) {
        (Some(x), Some(y), Some(z)) => Some(Position { x, y, z }),
        _ => None,
    };

    match ctx.db.npc_state().entity_id().find(npc_id) {
        Some(mut state) => {
            state.current_action = action;
            state.task_entity_id = task_entity_id;
            state.target_coords = target_coords;
            state.last_tick = now_micros;
            ctx.db.npc_state().entity_id().update(state);
        }
        None => {
            ctx.db.npc_state().insert(NpcState {
                entity_id: npc_id,
                current_action: action,
                target_coords,
                task_entity_id,
                last_tick: now_micros,
            });
        }
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// SERVER TICK ROUTINES (Optimized for minimal allocations & index lookups)
// ----------------------------------------------------------------------------

pub fn process_npc_brain_tick(ctx: &ReducerContext, dt: f32) {
    let brains: Vec<NpcBrain> = ctx.db.npc_brain().iter().collect();

    for mut brain in brains {
        let initial_brain = brain.clone();
        if brain.state == BrainState::Corpse { continue; }
        
        let Some(mut transform) = ctx.db.transform().entity_id().find(brain.entity_id) else { continue; };
        let hp = ctx.db.health().entity_id().find(brain.entity_id);
        
        let mut seed: u64 = (ctx.timestamp.to_micros_since_unix_epoch() as u64) ^ brain.entity_id;
        let mut dx = 0.0;
        let mut dz = 0.0;
        let mut current_speed = 0.0;

        let my_faction = ctx.db.faction_component().entity_id().find(brain.entity_id)
            .map(|f| f.faction)
            .unwrap_or(Faction::Wildlife);

        // Architectural Note: Replaces global transform scans with localized chunk-bounded queries
        // directly within the database index iterators, eliminating redundant allocations.
        let local_entities: Vec<Transform> = ctx.db.transform().iter()
            .filter(|t| is_chunk_adjacent(transform.chunk_x, transform.chunk_z, t.chunk_x, t.chunk_z))
            .collect();

        match brain.ai_type {
            AiType::Friendly => {
                if let Some(h) = hp {
                    if h.current < h.max * 0.8 && brain.state != BrainState::Attacking {
                        brain.state = BrainState::Attacking;
                        let mut nearest = None;
                        let mut min_d = f32::MAX;
                        for t in &local_entities {
                            if t.entity_id == brain.entity_id { continue; }
                            if ctx.db.harvestable_corpse().entity_id().find(t.entity_id).is_some() { continue; }
                            
                            let dist = (t.x - transform.x).powi(2) + (t.z - transform.z).powi(2);
                            if dist < min_d { 
                                min_d = dist; 
                                nearest = Some(t.entity_id); 
                            }
                        }
                        brain.target_id = nearest;
                    }
                }
            }
            AiType::Deer => {
                let mut nearest_threat = None;
                let mut min_d = 625.0; 
                
                for t in &local_entities {
                    if t.entity_id == brain.entity_id { continue; }
                    if ctx.db.harvestable_corpse().entity_id().find(t.entity_id).is_some() { continue; }
                    
                    if let Some(fac) = ctx.db.faction_component().entity_id().find(t.entity_id) {
                        if fac.faction == Faction::Player {
                            let dist_sq = (t.x - transform.x).powi(2) + (t.z - transform.z).powi(2);
                            if dist_sq < min_d { 
                                min_d = dist_sq; 
                                nearest_threat = Some(t.entity_id); 
                            }
                        }
                    }
                }

                if let Some(h) = hp {
                    if (h.current < h.max || nearest_threat.is_some()) && brain.state != BrainState::Fleeing {
                        brain.state = BrainState::Fleeing;
                        brain.target_id = nearest_threat;
                    }
                }
            }
            AiType::Boar => {
                let mut nearest_threat = None;
                let mut min_dist = 100.0; 
                for t in &local_entities {
                    if t.entity_id == brain.entity_id { continue; }
                    if ctx.db.harvestable_corpse().entity_id().find(t.entity_id).is_some() { continue; }
                    
                    let dist_sq = (t.x - transform.x).powi(2) + (t.z - transform.z).powi(2);
                    if dist_sq <= min_dist { 
                        if let Some(other_faction) = ctx.db.faction_component().entity_id().find(t.entity_id) {
                            if crate::combat::get_standing(&my_faction, &other_faction.faction) == crate::combat::FactionStanding::KillOnSight {
                                min_dist = dist_sq;
                                nearest_threat = Some(t.entity_id);
                            }
                        }
                    }
                }

                if let Some(threat_id) = nearest_threat {
                    if brain.state == BrainState::Idle {
                        brain.state = BrainState::Warning;
                        brain.timer = 0.0;
                    } else if brain.state == BrainState::Warning {
                        brain.timer += dt;
                        if brain.timer > 3.0 {
                            brain.state = BrainState::Attacking;
                            brain.target_id = Some(threat_id);
                        }
                    }
                } else {
                    brain.state = BrainState::Idle;
                    brain.timer = 0.0;
                }
            }
            AiType::Goblin => {
                if brain.state == BrainState::Idle {
                    let mut found_target = None;
                    for t in &local_entities {
                        if t.entity_id == brain.entity_id { continue; }
                        if ctx.db.harvestable_corpse().entity_id().find(t.entity_id).is_some() { continue; }
                        
                        let dist_sq = (t.x - transform.x).powi(2) + (t.z - transform.z).powi(2);
                        if dist_sq <= 400.0 { 
                            if let Some(other_faction) = ctx.db.faction_component().entity_id().find(t.entity_id) {
                                if crate::combat::get_standing(&my_faction, &other_faction.faction) == crate::combat::FactionStanding::KillOnSight {
                                    found_target = Some(t.entity_id);
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(target) = found_target {
                        brain.state = BrainState::Chasing;
                        brain.target_id = Some(target);
                    }
                }
            }
            AiType::Peasant => {
                let mut danger = false;
                for other_brain in ctx.db.npc_brain().iter() {
                    if other_brain.entity_id == brain.entity_id { continue; }
                    let is_attacking = other_brain.state == BrainState::Attacking;
                    
                    if is_attacking || other_brain.ai_type == AiType::Goblin {
                        if let Some(t) = ctx.db.transform().entity_id().find(other_brain.entity_id) {
                            if !is_chunk_adjacent(transform.chunk_x, transform.chunk_z, t.chunk_x, t.chunk_z) {
                                continue;
                            }
                            if ctx.db.harvestable_corpse().entity_id().find(t.entity_id).is_some() { continue; }
                            
                            let dist_sq = (t.x - transform.x).powi(2) + (t.z - transform.z).powi(2);
                            if dist_sq < 400.0 {
                                danger = true;
                                break;
                            }
                        }
                    }
                }
                if danger && brain.state != BrainState::Fleeing {
                    brain.state = BrainState::Fleeing;
                } else if !danger && brain.state == BrainState::Fleeing {
                    brain.state = BrainState::Idle;
                }
            }
        }

        match brain.state {
            BrainState::Idle => {
                let dist_sq = (brain.wander_x - transform.x).powi(2) + (brain.wander_z - transform.z).powi(2);
                if dist_sq < 4.0 {
                    if crate::prng(&mut seed) < 0.05 {
                        if brain.ai_type == AiType::Goblin {
                            brain.wander_x = brain.home_x + (crate::prng(&mut seed) * 30.0 - 15.0);
                            brain.wander_z = brain.home_z + (crate::prng(&mut seed) * 30.0 - 15.0);
                        } else {
                            brain.wander_x = transform.x + (crate::prng(&mut seed) * 60.0 - 30.0);
                            brain.wander_z = transform.z + (crate::prng(&mut seed) * 60.0 - 30.0);
                        }
                    }
                } else {
                    dx = brain.wander_x - transform.x;
                    dz = brain.wander_z - transform.z;
                    current_speed = 2.0; 
                }
            }
            BrainState::Fleeing => {
                if let Some(target) = brain.target_id {
                    if let Some(t) = ctx.db.transform().entity_id().find(target) {
                        dx = transform.x - t.x; 
                        dz = transform.z - t.z;
                        let dist_sq = dx * dx + dz * dz;
                        if dist_sq > 1600.0 { 
                            brain.state = BrainState::Idle;
                            brain.target_id = None;
                        } else {
                            current_speed = 7.0;
                        }
                    } else {
                        brain.state = BrainState::Idle;
                        brain.target_id = None;
                    }
                } else {
                    dx = transform.x - brain.home_x;
                    dz = transform.z - brain.home_z;
                    current_speed = 6.0;
                    brain.timer += dt;
                    if brain.timer > 4.0 {
                        brain.state = BrainState::Idle;
                        brain.timer = 0.0;
                    }
                }
            }
            BrainState::Chasing | BrainState::Attacking => {
                if let Some(target) = brain.target_id {
                    if let Some(t) = ctx.db.transform().entity_id().find(target) {
                        dx = t.x - transform.x; 
                        dz = t.z - transform.z;
                        
                        let dist_sq = dx * dx + dz * dz;
                        
                        if dist_sq > 2500.0 {
                            brain.state = BrainState::Idle;
                            brain.target_id = None;
                        } else {
                            let arch = crate::bestiary::get_archetype_by_ai_type(brain.ai_type);
                            let loadout = ctx.db.equipment_loadout().entity_id().find(brain.entity_id);
                            let weapon_name = loadout
                                .as_ref()
                                .and_then(|l| if !l.main_hand.is_empty() && l.main_hand != "None" { Some(l.main_hand.as_str()) } else { None })
                                .unwrap_or(arch.default_main_hand);
                            let weapon_def = crate::armory::get_weapon_def(weapon_name);

                            current_speed = arch.run_speed;
                            let max_reach = weapon_def.attack_range;
                            let max_reach_sq = max_reach * max_reach;

                            if dist_sq <= max_reach_sq {
                                current_speed = 0.0;
                                brain.timer += dt;
                                if brain.timer >= weapon_def.cooldown_secs {
                                    if let Some(proj_kind) = weapon_def.projectile_kind {
                                        let dist = dist_sq.sqrt().max(0.01);
                                        let ndx = dx / dist;
                                        let ndz = dz / dist;
                                        let dy = (t.y + 0.5) - (transform.y + 0.5);
                                        let ndy = (dy / dist).clamp(-1.0, 1.0);

                                        ctx.db.active_projectile().insert(crate::combat::ActiveProjectile {
                                            projectile_id: 0,
                                            shooter_id: brain.entity_id,
                                            kind: proj_kind,
                                            pos_x: transform.x + ndx * 0.6,
                                            pos_y: transform.y + 0.4,
                                            pos_z: transform.z + ndz * 0.6,
                                            vel_x: ndx * weapon_def.projectile_speed,
                                            vel_y: ndy * weapon_def.projectile_speed,
                                            vel_z: ndz * weapon_def.projectile_speed,
                                            gravity: weapon_def.gravity,
                                            drag: weapon_def.drag,
                                            damage: weapon_def.base_damage,
                                            blast_radius: 0.0,
                                            start_tick: 0,
                                            lifetime: 3.0,
                                        });
                                    } else {
                                        crate::combat::apply_damage(ctx, target, weapon_def.base_damage);
                                        ctx.db.combat_event().insert(crate::CombatEvent {
                                            id: 0, 
                                            event_type: "HitPlayer".to_string(),
                                            x: t.x, 
                                            y: t.y + 1.0, 
                                            z: t.z
                                        });
                                    }
                                    brain.timer = 0.0;
                                }
                            } else {
                                brain.timer = 0.0;
                            }
                        }
                    } else {
                        brain.state = BrainState::Idle;
                        brain.target_id = None;
                    }
                } else {
                    brain.state = BrainState::Idle;
                }
            }
            BrainState::Warning => {
                current_speed = 0.0;
            }
            BrainState::Corpse => {}
        }

        let mut sep_x = 0.0;
        let mut sep_z = 0.0;
        
        for other in &local_entities {
            if other.entity_id == brain.entity_id { continue; }
            let ox = transform.x - other.x;
            let oz = transform.z - other.z;
            let odist_sq = ox * ox + oz * oz;
            if odist_sq < 9.0 && odist_sq > 0.001 {
                let odist = odist_sq.sqrt().max(0.01);
                sep_x += (ox / odist) * (3.0 - odist);
                sep_z += (oz / odist) * (3.0 - odist);
            }
        }

        let mut final_vx = 0.0;
        let mut final_vz = 0.0;
        
        if current_speed > 0.0 {
            let dist = (dx * dx + dz * dz).sqrt().max(0.001);
            final_vx = (dx / dist) * current_speed;
            final_vz = (dz / dist) * current_speed;
        }

        final_vx += sep_x * 2.5;
        final_vz += sep_z * 2.5;

        let v_sq = final_vx * final_vx + final_vz * final_vz;
        if v_sq > 0.0001 {
            let v_mag = v_sq.sqrt().max(0.01);
            let speed_cap = current_speed.max(3.0); 
            let move_speed = v_mag.min(speed_cap);
            
            transform.x += (final_vx / v_mag) * move_speed * dt;
            transform.z += (final_vz / v_mag) * move_speed * dt;
            
            let ground_y = crate::get_terrain_height(transform.x, transform.z);
            transform.y = ground_y + 1.05;
            
            transform.chunk_x = (transform.x / 50.0).floor() as i32;
            transform.chunk_z = (transform.z / 50.0).floor() as i32;
            
            ctx.db.transform().entity_id().update(transform);
        }

        if brain != initial_brain {
            ctx.db.npc_brain().entity_id().update(brain);
        }
    }
}

pub fn process_ai_tick(ctx: &ReducerContext) {
    let peasants: Vec<Peasant> = ctx.db.peasant().iter().collect();

    let dt = 0.1_f32; 
    let speed = 6.0_f32; 
    let max_carry = 10;

    for p in peasants {
        let Some(mut peasant) = ctx.db.peasant().entity_id().find(p.entity_id) else { continue; };
        let initial_peasant = peasant.clone();

        if let Some(brain) = ctx.db.npc_brain().entity_id().find(peasant.entity_id) {
            if brain.state == BrainState::Fleeing || brain.state == BrainState::Corpse {
                continue; 
            }
        }

        let Some(mut transform) = ctx.db.transform().entity_id().find(peasant.entity_id) else { continue; };
        let initial_transform = transform.clone();
        
        if !transform.x.is_finite() || !transform.y.is_finite() || !transform.z.is_finite() {
            transform.x = 0.0; 
            transform.y = 10.0; 
            transform.z = 0.0;
        }

        let original_pos = (transform.x, transform.z);
        let mut apply_movement = false;
        let mut desired_velocity = (0.0_f32, 0.0_f32);

        match peasant.state.clone() {
            AiState::Idle => {
                if let Some(npc_st) = ctx.db.npc_state().entity_id().find(peasant.entity_id) {
                    match npc_st.current_action {
                        NpcAction::PathingToDoor => {
                            if let Some(door_id) = npc_st.task_entity_id {
                                if let Some(door) = ctx.db.structure().structure_id().find(door_id) {
                                    let dx = door.x - transform.x;
                                    let dz = door.z - transform.z;
                                    let dist_sq = dx * dx + dz * dz;
                                    if dist_sq <= 12.25 {
                                        let _ = tick_npc_ai(ctx, peasant.entity_id);
                                    } else if dist_sq > 0.0001 {
                                        let dist = dist_sq.sqrt().max(0.01);
                                        desired_velocity = ((dx / dist) * speed, (dz / dist) * speed);
                                        apply_movement = true;
                                    }
                                }
                            }
                        }
                        NpcAction::InteractingWithDoor => {
                            let _ = tick_npc_ai(ctx, peasant.entity_id);
                        }
                        NpcAction::ConstructingBlueprint => {
                            if let Some(bp_id) = npc_st.task_entity_id {
                                if let Some(bp) = ctx.db.structure().structure_id().find(bp_id) {
                                    let dx = bp.x - transform.x;
                                    let dz = bp.z - transform.z;
                                    let dist_sq = dx * dx + dz * dz;
                                    if dist_sq <= 16.0 {
                                        let _ = tick_npc_ai(ctx, peasant.entity_id);
                                    } else if dist_sq > 0.0001 {
                                        let dist = dist_sq.sqrt().max(0.01);
                                        desired_velocity = ((dx / dist) * speed, (dz / dist) * speed);
                                        apply_movement = true;
                                    }
                                }
                            }
                        }
                        NpcAction::Patrolling => {
                            if let Some(ref target) = npc_st.target_coords {
                                let dx = target.x - transform.x;
                                let dz = target.z - transform.z;
                                let dist_sq = dx * dx + dz * dz;
                                if dist_sq < 1.5 {
                                    let mut updated = npc_st.clone();
                                    updated.current_action = NpcAction::Idle;
                                    ctx.db.npc_state().entity_id().update(updated);
                                } else if dist_sq > 0.0001 {
                                    let dist = dist_sq.sqrt().max(0.01);
                                    desired_velocity = ((dx / dist) * speed, (dz / dist) * speed);
                                    apply_movement = true;
                                }
                            }
                        }
                        NpcAction::Idle => {}
                    }
                }
            }
            AiState::AutoGather(ref target_type) => {
                let mut nearest_node = None;
                let mut min_dist_sq = 1_000_000.0_f32;

                // Architectural Note: Filter nodes using chunk bounds around peasant
                for node in ctx.db.resource_node().iter() {
                    if node.health > 0 && is_chunk_adjacent(transform.chunk_x, transform.chunk_z, node.chunk_x, node.chunk_z) {
                        let matches_type = target_type == "All" || &node.node_type == target_type;
                        if matches_type {
                            let dx = node.x - transform.x;
                            let dz = node.z - transform.z;
                            let dist_sq = dx * dx + dz * dz;
                            if dist_sq < min_dist_sq {
                                min_dist_sq = dist_sq;
                                nearest_node = Some(node.node_id);
                            }
                        }
                    }
                }

                if let Some(node_id) = nearest_node {
                    peasant.last_harvest_target = Some(node_id);
                    peasant.state = AiState::Harvest(node_id);
                } else {
                    peasant.state = AiState::Return(peasant.owner_id);
                }
            }
            AiState::MoveTo(pos) => {
                let dx = pos.x - transform.x;
                let dz = pos.z - transform.z;
                let dist_sq = dx * dx + dz * dz;
                
                if dist_sq < 1.0 {
                    peasant.state = AiState::Idle;
                } else if dist_sq > 0.0001 { 
                    let dist = dist_sq.sqrt().max(0.01);
                    desired_velocity = ((dx / dist) * speed, (dz / dist) * speed);
                    apply_movement = true;
                }
            }
            AiState::Harvest(target_id) => {
                if let Some(node) = ctx.db.resource_node().node_id().find(target_id) {
                    if node.health == 0 {
                        peasant.state = if peasant.auto_gather_type != "None" {
                            AiState::AutoGather(peasant.auto_gather_type.clone())
                        } else {
                            AiState::Idle
                        };
                    } else {
                        let dx = node.x - transform.x;
                        let dz = node.z - transform.z;
                        let dist_sq = dx * dx + dz * dz;
                        
                        if dist_sq < 16.0 { 
                            if peasant.consecutive_stuck_ticks >= 10 {
                                peasant.consecutive_stuck_ticks = 0;

                                peasant.carrying_item = match node.node_type.as_str() {
                                    "Tree" => "Wood".to_string(),
                                    "Rock" => "Stone".to_string(),
                                    _ => "Berry".to_string(),
                                };
                                
                                if peasant.carrying_amount < max_carry {
                                    peasant.carrying_amount += 1;
                                }
                                
                                let mut updated_node = node.clone();
                                updated_node.health = updated_node.health.saturating_sub(1);
                                
                                if updated_node.health == 0 {
                                    if updated_node.node_type == "Bush" {
                                        ctx.db.resource_node().node_id().update(updated_node.clone());
                                        let has_timer = ctx.db.respawn_bush_timer().iter().any(|t| t.node_id == updated_node.node_id);
                                        if !has_timer {
                                            ctx.db.respawn_bush_timer().insert(crate::RespawnBushTimer {
                                                scheduled_id: 0,
                                                scheduled_at: spacetimedb::ScheduleAt::Interval(std::time::Duration::from_secs(60).into()),
                                                node_id: updated_node.node_id,
                                            });
                                        }
                                    } else {
                                        ctx.db.resource_node().node_id().delete(updated_node.node_id);
                                    }
                                    
                                    if peasant.carrying_amount >= max_carry {
                                        peasant.state = AiState::Return(peasant.owner_id);
                                    } else if peasant.auto_gather_type != "None" {
                                        peasant.state = AiState::AutoGather(peasant.auto_gather_type.clone());
                                    } else {
                                        peasant.state = AiState::Idle;
                                    }
                                } else {
                                    ctx.db.resource_node().node_id().update(updated_node);
                                    if peasant.carrying_amount >= max_carry {
                                        peasant.state = AiState::Return(peasant.owner_id);
                                    }
                                }
                            } else {
                                peasant.consecutive_stuck_ticks += 1;
                            }
                        } else if dist_sq > 0.0001 { 
                            let dist = dist_sq.sqrt().max(0.01);
                            desired_velocity = ((dx / dist) * speed, (dz / dist) * speed);
                            apply_movement = true;
                        }
                    }
                } else {
                    peasant.last_harvest_target = None;
                    peasant.state = if peasant.auto_gather_type != "None" {
                        AiState::AutoGather(peasant.auto_gather_type.clone())
                    } else {
                        AiState::Idle
                    };
                }
            }
            AiState::Return(stockpile_id) => {
                if let Some(stockpile_t) = ctx.db.transform().entity_id().find(stockpile_id) {
                    let dx = stockpile_t.x - transform.x;
                    let dz = stockpile_t.z - transform.z;
                    let dist_sq = dx * dx + dz * dz;

                    if dist_sq < 16.0 {
                        peasant.state = AiState::Deposit(stockpile_id);
                    } else if dist_sq > 0.0001 { 
                        let dist = dist_sq.sqrt().max(0.01);
                        desired_velocity = ((dx / dist) * speed, (dz / dist) * speed);
                        apply_movement = true;
                    }
                } else {
                    peasant.state = AiState::Idle;
                }
            }
            AiState::Deposit(stockpile_id) => {
                if peasant.consecutive_stuck_ticks >= 10 {
                    peasant.consecutive_stuck_ticks = 0;

                    if let Some(mut inv) = ctx.db.inventory().entity_id().find(stockpile_id) {
                        crate::add_item(&mut inv, &peasant.carrying_item, peasant.carrying_amount);
                        ctx.db.inventory().entity_id().update(inv);
                    }
                    
                    peasant.carrying_amount = 0;
                    peasant.carrying_item = "None".to_string();
                    
                    peasant.state = if peasant.auto_gather_type != "None" {
                        AiState::AutoGather(peasant.auto_gather_type.clone())
                    } else if let Some(prev_target) = peasant.last_harvest_target {
                        AiState::Harvest(prev_target)
                    } else {
                        AiState::Idle
                    };
                } else {
                    peasant.consecutive_stuck_ticks += 1;
                }
            }
        }

        if apply_movement {
            let mut sep_x = 0.0;
            let mut sep_z = 0.0;
            
            if peasant.consecutive_stuck_ticks < 5 {
                // Architectural Note: Evaluate unit crowds in-place without vector allocation
                for other in ctx.db.transform().iter().filter(|t| is_chunk_adjacent(transform.chunk_x, transform.chunk_z, t.chunk_x, t.chunk_z)) {
                    if other.entity_id == peasant.entity_id { continue; }
                    let ox = transform.x - other.x;
                    let oz = transform.z - other.z;
                    let odist_sq = ox * ox + oz * oz;
                    
                    if odist_sq < 9.0 && odist_sq > 0.001 { 
                        let odist = odist_sq.sqrt().max(0.01);
                        sep_x += (ox / odist) * (3.0 - odist);
                        sep_z += (oz / odist) * (3.0 - odist);
                    }
                }

                // Architectural Note: Localized structure repulsion without cloning all structures into RAM
                // Architectural Note: Open doors are walkable gaps, so they exert no repulsion.
                for s in ctx.db.structure().iter()
                    .filter(|s| (s.x - transform.x).abs() <= 5.0 && (s.z - transform.z).abs() <= 5.0)
                    .filter(|s| crate::building::structure_blocks_projectiles(ctx, s))
                {
                    let ox = transform.x - s.x;
                    let oz = transform.z - s.z;
                    let odist_sq = ox * ox + oz * oz;
                    
                    if odist_sq < 10.0 && odist_sq > 0.001 {
                        let odist = odist_sq.sqrt().max(0.01);
                        let push = (3.16 - odist) * 2.5; 
                        
                        sep_x += (ox / odist) * push + (oz / odist) * (push * 0.4);
                        sep_z += (oz / odist) * push - (ox / odist) * (push * 0.4);
                    }
                }
            }

            let final_vx = desired_velocity.0 + sep_x * 2.5;
            let final_vz = desired_velocity.1 + sep_z * 2.5;
            
            let v_sq = final_vx * final_vx + final_vz * final_vz;
            if v_sq > 0.0001 {
                let v_mag = v_sq.sqrt().max(0.01);
                transform.x += (final_vx / v_mag) * speed * dt;
                transform.z += (final_vz / v_mag) * speed * dt;
            }
            
            if transform.x.is_nan() || transform.z.is_nan() {
                transform.x = original_pos.0;
                transform.z = original_pos.1;
            }
            
            let ground_y = crate::get_terrain_height(transform.x, transform.z);
            transform.y = ground_y + 1.05;

            let dist_moved_sq = (transform.x - original_pos.0).powi(2) + (transform.z - original_pos.1).powi(2);
            if dist_moved_sq < 0.01 {
                peasant.consecutive_stuck_ticks += 1;
                if peasant.consecutive_stuck_ticks >= 10 {
                    transform.x += 1.0;
                    transform.z += 1.0;
                    peasant.consecutive_stuck_ticks = 0;
                }
            } else {
                peasant.consecutive_stuck_ticks = 0;
            }

            transform.chunk_x = (transform.x / 50.0).floor() as i32;
            transform.chunk_z = (transform.z / 50.0).floor() as i32;
        } 

        if peasant != initial_peasant {
            ctx.db.peasant().entity_id().update(peasant);
        }

        let transform_changed = (transform.x - initial_transform.x).abs() > 0.001 
                             || (transform.y - initial_transform.y).abs() > 0.001 
                             || (transform.z - initial_transform.z).abs() > 0.001
                             || transform.chunk_x != initial_transform.chunk_x
                             || transform.chunk_z != initial_transform.chunk_z;

        if transform_changed {
            ctx.db.transform().entity_id().update(transform);
        }
    }
}