use bevy::prelude::{Transform as BevyTransform, *};
use bevy::window::PrimaryWindow;
use avian3d::prelude::*;
use tracing::info;

// Note: Assuming `module_bindings` is exposed at the crate root.
use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::prediction::ClientTick; // Architectural Note: Importing the rolling tick for lag compensation
use crate::module_bindings::swing_tool_reducer::swing_tool; 
use crate::module_bindings::gather_loot_reducer::gather_loot; 
use crate::module_bindings::fire_weapon_reducer::fire_weapon; // Architectural Note: v2.x explicit trait import.

// ----------------------------------------------------------------------------
// EVENTS & ENUMS
// ----------------------------------------------------------------------------

/// Abstract representations of player intent, independent of hardware bindings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VirtualAction {
    Primary,
    Secondary,
    Interact,
}

/// Tracks the lifecycle of a discrete input action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionState {
    JustPressed,
    Pressed,
    JustReleased,
}

/// Event fired when a hardware input successfully maps to a `VirtualAction`.
#[derive(Event, Debug)]
pub struct ActionEvent {
    pub action: VirtualAction,
    pub state: ActionState,
    pub cursor_pos: Option<Vec2>,
}

// ----------------------------------------------------------------------------
// INPUT ROUTING
// ----------------------------------------------------------------------------

/// Translates raw mouse and keyboard states into abstract `ActionEvent`s.
/// Architectural Note: This separation allows us to easily implement key rebinding 
/// in the future, or simulate inputs for headless integration tests.
pub fn input_router_system(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut action_events: EventWriter<ActionEvent>,
) {
    let cursor_pos = window_query.get_single().ok().and_then(|w| w.cursor_position());

    if mouse.just_pressed(MouseButton::Left) {
        action_events.send(ActionEvent { action: VirtualAction::Primary, state: ActionState::JustPressed, cursor_pos });
    }
    if mouse.pressed(MouseButton::Left) {
        action_events.send(ActionEvent { action: VirtualAction::Primary, state: ActionState::Pressed, cursor_pos });
    }
    if mouse.just_released(MouseButton::Left) {
        action_events.send(ActionEvent { action: VirtualAction::Primary, state: ActionState::JustReleased, cursor_pos });
    }

    if mouse.just_pressed(MouseButton::Right) {
        action_events.send(ActionEvent { action: VirtualAction::Secondary, state: ActionState::JustPressed, cursor_pos });
    }
    
    if keys.just_pressed(KeyCode::KeyE) {
        action_events.send(ActionEvent { action: VirtualAction::Interact, state: ActionState::JustPressed, cursor_pos });
    }
}

// ----------------------------------------------------------------------------
// ACTION DISPATCHING
// ----------------------------------------------------------------------------

/// Consumes `ActionEvent`s and applies context-aware logic depending on the active `CameraMode`.
/// Architectural Note: Handles local state prediction (like the swing timer) while 
/// dispatching authoritative actions (like gathering loot) to SpacetimeDB reducers.
pub fn context_aware_action_dispatcher(
    mut commands: Commands,
    mut action_events: EventReader<ActionEvent>,
    camera_mode: Res<State<CameraMode>>,
    keys: Res<ButtonInput<KeyCode>>,
    
    mut swing_state: ResMut<SwingState>,
    fps_camera_query: Query<&GlobalTransform, With<FpsCamera>>,
    player_query: Query<(Entity, &BevyTransform), With<PlayerBody>>,
    loot_query: Query<&GroundLootItem>,
    
    rts_camera_query: Query<(&Camera, &GlobalTransform), With<RtsCameraChild>>,
    spatial_query: SpatialQuery,
    selected_query: Query<Entity, With<Selected>>,
    selectable_query: Query<(Entity, &BevyTransform), With<Selectable>>,
    mut selection_state: ResMut<SelectionState>,
    conn: Res<SpacetimeConnection>,
    tick: Res<ClientTick>, // Architectural Note: Inject the rolling local simulation tick for lag compensation.
) {
    let multi_select = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    for event in action_events.read() {
        match camera_mode.get() {
            CameraMode::FPS => {
                match event.action {
                    VirtualAction::Primary if event.state == ActionState::JustPressed => {
                        // Architectural Note: Instant Client-Side Prediction Raycast & Firing
                        if !swing_state.is_swinging {
                            swing_state.is_swinging = true;
                            if let Ok(cam_transform) = fps_camera_query.get_single() {
                                let origin = cam_transform.translation();
                                let dir = cam_transform.forward();
                                
                                // Architectural Note: Transmit Fire Event with Tick Timestamp for Lag Comp
                                let _ = conn.db.reducers.fire_weapon(
                                    tick.0, 
                                    origin.x, origin.y, origin.z, 
                                    dir.x, dir.y, dir.z
                                );
                            }
                        }
                    }
                    VirtualAction::Interact if event.state == ActionState::JustPressed => {
                        if let Ok((player_entity, player_transform)) = player_query.get_single() {
                            let intersections = spatial_query.shape_intersections(
                                &Collider::sphere(3.5), 
                                player_transform.translation, 
                                Quat::IDENTITY, 
                                SpatialQueryFilter::from_excluded_entities([player_entity]), 
                            );
                            
                            for entity in intersections {
                                if let Ok(loot) = loot_query.get(entity) {
                                    let _ = conn.db.reducers.gather_loot(loot.loot_id);
                                    break; // Only pick up one item per interaction press
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            CameraMode::RTS => {
                match event.action {
                    VirtualAction::Primary => {
                        let Some(cursor_pos) = event.cursor_pos else { continue; };
                        let Ok((camera, cam_transform)) = rts_camera_query.get_single() else { continue; };
                        
                        match event.state {
                            ActionState::JustPressed => {
                                selection_state.is_dragging = true;
                                selection_state.start_pos = Some(cursor_pos);
                                selection_state.end_pos = Some(cursor_pos);

                                if !multi_select {
                                    for entity in selected_query.iter() {
                                        commands.entity(entity).remove::<Selected>();
                                    }
                                }
                            }
                            ActionState::Pressed => {
                                if selection_state.is_dragging {
                                    selection_state.end_pos = Some(cursor_pos);
                                }
                            }
                            ActionState::JustReleased => {
                                selection_state.is_dragging = false;
                                let start = selection_state.start_pos.unwrap_or(cursor_pos);
                                let dist = start.distance(cursor_pos);

                                if dist < 5.0 {
                                    // Click Selection or Move Command
                                    if let Some(ray) = camera.viewport_to_world(cam_transform, cursor_pos) {
                                        if let Some(hit) = spatial_query.cast_ray(
                                            ray.origin,
                                            ray.direction,
                                            1000.0,
                                            true,
                                            SpatialQueryFilter::from_mask([GameLayer::Unit, GameLayer::Terrain, GameLayer::Environment]),
                                        ) {
                                            if selectable_query.contains(hit.entity) {
                                                commands.entity(hit.entity).insert(Selected);
                                            } else {
                                                let hit_point = ray.origin + ray.direction * hit.time_of_impact;
                                                info!("RTS Target Acquired: Issuing NavMesh pathfinding move command to {}.", hit_point);
                                                
                                                for selected_entity in selected_query.iter() {
                                                    commands.entity(selected_entity).insert(NavTarget(hit_point));
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // Marquee Selection Box
                                    let min_x = start.x.min(cursor_pos.x);
                                    let max_x = start.x.max(cursor_pos.x);
                                    let min_y = start.y.min(cursor_pos.y);
                                    let max_y = start.y.max(cursor_pos.y);

                                    for (entity, transform) in selectable_query.iter() {
                                        if let Some(screen_pos) = camera.world_to_viewport(cam_transform, transform.translation) {
                                            if screen_pos.x >= min_x && screen_pos.x <= max_x && screen_pos.y >= min_y && screen_pos.y <= max_y {
                                                commands.entity(entity).insert(Selected);
                                            }
                                        }
                                    }
                                }

                                selection_state.start_pos = None;
                                selection_state.end_pos = None;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------
// MOVEMENT SYSTEMS
// ----------------------------------------------------------------------------

/// Applies Avian3D forces to selected RTS units moving toward a `NavTarget`.
/// Architectural Note: This is currently client-side prediction logic. Eventually,
/// RTS pathfinding nodes should be validated server-side by a SpacetimeDB reducer.
pub fn rts_navmesh_movement_system(
    mut commands: Commands,
    mut query: Query<(Entity, &BevyTransform, &mut LinearVelocity, &NavTarget)>,
) {
    for (entity, transform, mut velocity, target) in query.iter_mut() {
        let dir = target.0 - transform.translation;
        let dist = Vec2::new(dir.x, dir.z).length(); 

        if dist < 1.0 {
            commands.entity(entity).remove::<NavTarget>();
            velocity.x = 0.0;
            velocity.z = 0.0;
        } else {
            let move_dir = dir.normalize();
            let speed = 8.0;
            velocity.x = move_dir.x * speed;
            velocity.z = move_dir.z * speed;
        }
    }
}

/// Handles localized FPS player movement via keyboard WASD inputs.
pub fn player_movement_system(
    keys: Res<ButtonInput<KeyCode>>, 
    camera_mode: Res<State<CameraMode>>,
    mut query: Query<(Entity, &BevyTransform, &mut LinearVelocity, &mut GravityScale, &mut Kcc), With<PlayerBody>>,
    spatial_query: SpatialQuery, 
) {
    let Ok((entity, transform, mut lin_vel, mut gravity, mut kcc)) = query.get_single_mut() else { return; };

    let ray_start = transform.translation; 
    let hit = spatial_query.cast_ray(
        ray_start, 
        Dir3::NEG_Y, 
        1.2, 
        true, 
        SpatialQueryFilter::from_excluded_entities([entity])
    );
    
    kcc.is_grounded = false;
    if let Some(hit_data) = hit {
        if hit_data.time_of_impact <= 1.05 {
            kcc.is_grounded = true;
        }
    }

    let mut move_dir = Vec3::ZERO;
    if *camera_mode.get() == CameraMode::FPS {
        if keys.pressed(KeyCode::KeyW) { move_dir += *transform.forward(); }
        if keys.pressed(KeyCode::KeyS) { move_dir -= *transform.forward(); }
        if keys.pressed(KeyCode::KeyD) { move_dir += *transform.right(); }
        if keys.pressed(KeyCode::KeyA) { move_dir -= *transform.right(); }
    }

    move_dir.y = 0.0;
    if move_dir != Vec3::ZERO { move_dir = move_dir.normalize(); }

    let horizontal_speed = 6.0;
    
    if *camera_mode.get() == CameraMode::FPS {
        lin_vel.x = move_dir.x * horizontal_speed;
        lin_vel.z = move_dir.z * horizontal_speed;
    }

    if kcc.is_grounded {
        if *camera_mode.get() == CameraMode::FPS && keys.just_pressed(KeyCode::Space) { 
            lin_vel.y = 7.0; 
            gravity.0 = 2.5; 
        } else {
            if move_dir == Vec3::ZERO {
                lin_vel.y = 0.0;
                gravity.0 = 0.0; 
            } else {
                lin_vel.y = -1.0; 
                gravity.0 = 2.5;
            }
        }
    } else {
        gravity.0 = 2.5; 
    }
}