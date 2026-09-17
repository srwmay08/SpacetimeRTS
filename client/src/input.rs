use bevy::prelude::{Transform as BevyTransform, *};
use bevy::ecs::system::SystemParam; 
use bevy::window::PrimaryWindow;
use avian3d::prelude::*;
use tracing::info;

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::prediction::ClientTick; 
use crate::building::BuildModeState; // Architectural Note: Required to mask primary inputs.

use crate::module_bindings::gather_loot_reducer::gather_loot; 
use crate::module_bindings::fire_weapon_reducer::fire_weapon; 
use crate::module_bindings::swing_tool_reducer::swing_tool; 

// ----------------------------------------------------------------------------
// EVENTS & ENUMS
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VirtualAction {
    Primary,
    Secondary,
    Interact,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionState {
    JustPressed,
    Pressed,
    JustReleased,
}

#[derive(Event, Debug)]
pub struct ActionEvent {
    pub action: VirtualAction,
    pub state: ActionState,
    pub cursor_pos: Option<Vec2>,
}

// ----------------------------------------------------------------------------
// SYSTEM PARAM BUNDLING 
// ----------------------------------------------------------------------------

#[derive(SystemParam)]
pub struct ActionContextQueries<'w, 's> {
    pub fps_camera: Query<'w, 's, &'static GlobalTransform, With<FpsCamera>>,
    pub player: Query<'w, 's, (Entity, &'static BevyTransform), With<PlayerBody>>,
    pub loot: Query<'w, 's, &'static GroundLootItem>,
    pub node: Query<'w, 's, &'static ResourceNodeItem>,
    pub structure: Query<'w, 's, &'static NetworkStructure>,
    pub rts_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<RtsCameraChild>>,
    pub selectable: Query<'w, 's, (Entity, &'static BevyTransform), With<Selectable>>,
    pub selected: Query<'w, 's, Entity, With<Selected>>,
}

// ----------------------------------------------------------------------------
// INPUT ROUTING
// ----------------------------------------------------------------------------

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

pub fn context_aware_action_dispatcher(
    mut commands: Commands,
    mut action_events: EventReader<ActionEvent>,
    camera_mode: Res<State<CameraMode>>,
    build_state: Res<BuildModeState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut swing_state: ResMut<SwingState>,
    queries: ActionContextQueries,
    spatial_query: SpatialQuery,
    mut selection_state: ResMut<SelectionState>,
    conn: Res<SpacetimeConnection>,
    tick: Res<ClientTick>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let multi_select = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    for event in action_events.read() {
        match camera_mode.get() {
            CameraMode::FPS => {
                match event.action {
                    VirtualAction::Primary if event.state == ActionState::JustPressed => {
                        // Architectural Note: Strictly isolates combat/gathering input from the build system
                        if build_state.is_active { continue; }

                        if !swing_state.is_swinging {
                            swing_state.is_swinging = true;
                            if let Ok(cam_transform) = queries.fps_camera.get_single() {
                                if let Ok((player_entity, _)) = queries.player.get_single() {
                                    let origin = cam_transform.translation();
                                    let dir = cam_transform.forward();
                                    
                                    let hit = spatial_query.cast_ray(
                                        origin,
                                        dir.into(),
                                        50.0, 
                                        true,
                                        SpatialQueryFilter::from_excluded_entities([player_entity]),
                                    );

                                    let is_tool_context = hit.map_or(false, |hit_data| {
                                        hit_data.time_of_impact < 6.0 && 
                                        (queries.node.contains(hit_data.entity) || queries.structure.contains(hit_data.entity))
                                    });

                                    if is_tool_context {
                                        info!("Context Target: Resource Node / Structure. Invoking swing_tool.");
                                        let _ = conn.db.reducers.swing_tool(
                                            origin.x, origin.y, origin.z, 
                                            dir.x, dir.y, dir.z
                                        );
                                    } else {
                                        info!("Context Target: Enemy / Terrain. Invoking fire_weapon with prediction.");
                                        
                                        let distance = hit.map_or(50.0, |h| h.time_of_impact);
                                        let mid_point = origin + dir * (distance / 2.0);
                                        let mut tracer_transform = BevyTransform::from_translation(mid_point)
                                            .looking_at(origin + dir * distance, Vec3::Y);
                                        tracer_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);
                                        
                                        commands.spawn((
                                            PbrBundle {
                                                mesh: meshes.add(Cylinder::new(0.02, distance)),
                                                material: materials.add(StandardMaterial {
                                                    base_color: Color::srgb(1.0, 0.9, 0.5),
                                                    unlit: true,
                                                    ..default()
                                                }),
                                                transform: tracer_transform,
                                                ..default()
                                            },
                                            Particle { timer: Timer::from_seconds(0.05, TimerMode::Once) },
                                        ));

                                        let _ = conn.db.reducers.fire_weapon(
                                            tick.0, 
                                            origin.x, origin.y, origin.z, 
                                            dir.x, dir.y, dir.z
                                        );
                                    }
                                }
                            }
                        }
                    }
                    VirtualAction::Interact if event.state == ActionState::JustPressed => {
                        if build_state.is_active { continue; }

                        if let Ok((player_entity, player_transform)) = queries.player.get_single() {
                            let intersections = spatial_query.shape_intersections(
                                &Collider::sphere(3.5), 
                                player_transform.translation, 
                                Quat::IDENTITY, 
                                SpatialQueryFilter::from_excluded_entities([player_entity]), 
                            );
                            
                            for entity in intersections {
                                if let Ok(loot) = queries.loot.get(entity) {
                                    let _ = conn.db.reducers.gather_loot(loot.loot_id);
                                    break;
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
                        if build_state.is_active { continue; }

                        let Some(cursor_pos) = event.cursor_pos else { continue; };
                        let Ok((camera, cam_transform)) = queries.rts_camera.get_single() else { continue; };
                        
                        match event.state {
                            ActionState::JustPressed => {
                                selection_state.is_dragging = true;
                                selection_state.start_pos = Some(cursor_pos);
                                selection_state.end_pos = Some(cursor_pos);

                                if !multi_select {
                                    for entity in queries.selected.iter() {
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
                                    if let Some(ray) = camera.viewport_to_world(cam_transform, cursor_pos) {
                                        if let Some(hit) = spatial_query.cast_ray(
                                            ray.origin,
                                            ray.direction,
                                            1000.0,
                                            true,
                                            SpatialQueryFilter::from_mask([GameLayer::Unit, GameLayer::Terrain, GameLayer::Environment]),
                                        ) {
                                            if queries.selectable.contains(hit.entity) {
                                                commands.entity(hit.entity).insert(Selected);
                                            } else {
                                                let hit_point = ray.origin + ray.direction * hit.time_of_impact;
                                                info!("RTS Target Acquired: Issuing NavMesh pathfinding move command to {}.", hit_point);
                                                
                                                for selected_entity in queries.selected.iter() {
                                                    commands.entity(selected_entity).insert(NavTarget(hit_point));
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    let min_x = start.x.min(cursor_pos.x);
                                    let max_x = start.x.max(cursor_pos.x);
                                    let min_y = start.y.min(cursor_pos.y);
                                    let max_y = start.y.max(cursor_pos.y);

                                    for (entity, transform) in queries.selectable.iter() {
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

pub fn player_movement_system(
    keys: Res<ButtonInput<KeyCode>>, 
    camera_mode: Res<State<CameraMode>>,
    mut query: Query<(Entity, &mut BevyTransform, &mut LinearVelocity, &mut GravityScale, &mut Kcc), With<PlayerBody>>,
    spatial_query: SpatialQuery, 
) {
    let Ok((entity, mut transform, mut lin_vel, mut gravity, mut kcc)) = query.get_single_mut() else { return; };

    let ray_start = transform.translation; 
    let hit = spatial_query.cast_ray(
        ray_start, 
        Dir3::NEG_Y, 
        1.15, 
        true, 
        SpatialQueryFilter::from_excluded_entities([entity])
    );
    
    kcc.is_grounded = false;
    if let Some(hit_data) = hit {
        if hit_data.time_of_impact <= 1.15 {
            kcc.is_grounded = true;
        }
    }

    let ground_y = crate::terrain::get_terrain_height(transform.translation.x, transform.translation.z);
    let player_half_height = 1.05; 
    
    if transform.translation.y <= ground_y + player_half_height {
        transform.translation.y = ground_y + player_half_height;
        if lin_vel.y < 0.0 {
            lin_vel.y = 0.0;
        }
        kcc.is_grounded = true;
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

    let horizontal_speed = 15.0; 
    
    if *camera_mode.get() == CameraMode::FPS {
        lin_vel.x = move_dir.x * horizontal_speed;
        lin_vel.z = move_dir.z * horizontal_speed;
    }

    gravity.0 = 8.0; 

    if kcc.is_grounded && *camera_mode.get() == CameraMode::FPS && keys.just_pressed(KeyCode::Space) { 
        lin_vel.y = 10.0; 
        kcc.is_grounded = false; 
    }
}