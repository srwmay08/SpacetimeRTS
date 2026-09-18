use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::{transform, player_session, Transform}; 
use crate::inventory; 
use crate::resource_node;
use crate::respawn_bush_timer; 
use crate::building::{structure, Structure}; // Architectural Note: Imported to expose custom player-built obstacle data to the AI.

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

#[table(accessor = peasant, public)]
#[derive(Clone, PartialEq)]
pub struct Peasant {
    #[primary_key]
    pub entity_id: u64,
    pub owner_id: u64,
    pub state: AiState,
    pub carrying_item: String,
    pub carrying_amount: u32,
    pub last_harvest_target: Option<u64>,
    pub consecutive_stuck_ticks: u32,
    pub auto_gather_type: String, 
}

#[reducer]
pub fn spawn_peasant(ctx: &ReducerContext) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or("Inventory not found")?;

    if !crate::remove_item(&mut inv, "Wood", 20) {
        return Err("Insufficient Wood to spawn a peasant.".into());
    }
    
    ctx.db.inventory().entity_id().update(inv);

    let entity_id = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    let spawn_transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or("Player transform missing")?;
        
    let spawn_y = crate::get_terrain_height(spawn_transform.x, spawn_transform.z) + 1.5;

    ctx.db.transform().insert(Transform {
        entity_id,
        x: spawn_transform.x + 2.0, 
        y: spawn_y, 
        z: spawn_transform.z + 2.0,
        chunk_x: (spawn_transform.x / 50.0).floor() as i32,
        chunk_z: (spawn_transform.z / 50.0).floor() as i32,
        last_processed_tick: 0,
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

    log::info!("COMMAND ACCEPTED: Spawned new Peasant {} for Player {}", entity_id, session.entity_id);
    Ok(())
}

#[reducer]
pub fn command_peasant(
    ctx: &ReducerContext,
    peasant_entity_id: u64,
    command_type: String, 
    target_x: f32, target_y: f32, target_z: f32,
    target_id: u64,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    let mut peasant = ctx.db.peasant().entity_id().find(peasant_entity_id)
        .ok_or("Peasant not found")?;

    if peasant.owner_id != session.entity_id {
        return Err("Unauthorized: You do not own this unit.".into());
    }

    log::info!("COMMAND ACCEPTED: Peasant {} assigned to Task: [{}]", peasant_entity_id, command_type);

    peasant.state = match command_type.as_str() {
        "MoveTo" => { 
            peasant.auto_gather_type = "None".to_string(); 
            peasant.last_harvest_target = None;
            AiState::MoveTo(Position { x: target_x, y: target_y, z: target_z }) 
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
        "Idle" | _ => { 
            peasant.auto_gather_type = "None".to_string(); 
            peasant.last_harvest_target = None;
            AiState::Idle 
        },
    };

    ctx.db.peasant().entity_id().update(peasant);
    Ok(())
}

pub fn process_ai_tick(ctx: &ReducerContext) {
    let peasants: Vec<Peasant> = ctx.db.peasant().iter().collect();
    let all_transforms: Vec<Transform> = ctx.db.transform().iter().collect();

    let dt = 0.1_f32; 
    let speed = 6.0_f32; 
    let max_carry = 10;

    for p in peasants {
        let Some(mut peasant) = ctx.db.peasant().entity_id().find(p.entity_id) else { continue; };
        let initial_peasant = peasant.clone();

        let Some(mut transform) = ctx.db.transform().entity_id().find(peasant.entity_id) else { continue; };
        let initial_transform = transform.clone();
        
        if !transform.x.is_finite() || !transform.y.is_finite() || !transform.z.is_finite() {
            transform.x = 0.0; transform.y = 10.0; transform.z = 0.0;
        }

        let original_pos = (transform.x, transform.z);
        let mut apply_movement = false;
        let mut desired_velocity = (0.0_f32, 0.0_f32);

        match peasant.state.clone() {
            AiState::Idle => {}
            AiState::AutoGather(ref target_type) => {
                let mut nearest_node = None;
                let mut min_dist_sq = 1_000_000.0_f32;

                for node in ctx.db.resource_node().iter() {
                    if node.health > 0 {
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
                                    "Rock" => "Ore".to_string(),
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
                // Architectural Note: Unit Separation Field
                for other in &all_transforms {
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

                // Architectural Note: Vector Field Repulsion for Structures.
                // Prevents peasants from ghosting through custom built walls and foundations.
                // We purposefully add a tangential component (oz, -ox) to "swirl" the agent 
                // around corners, preventing a dead-stop stall when hitting a flat wall.
                for s in ctx.db.structure().iter() {
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