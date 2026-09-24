use bevy::prelude::{Transform as BevyTransform, *};
use bevy::ecs::system::SystemParam; 
use bevy::window::{CursorGrabMode, PrimaryWindow};
use avian3d::prelude::*;
use tracing::{info, error};

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::prediction::ClientTick; 
use crate::building::BuildModeState; 

use crate::module_bindings::swing_tool_reducer::swing_tool;
use crate::module_bindings::fire_weapon_reducer::fire_weapon;
use crate::module_bindings::fire_bow_reducer::fire_bow;
use crate::module_bindings::repair_structure_reducer::repair_structure;
use crate::module_bindings::contribute_construction_reducer::contribute_construction;
use crate::module_bindings::interact_node_reducer::interact_node;
use crate::module_bindings::command_peasant_reducer::command_peasant;
use crate::module_bindings::spawn_peasant_reducer::spawn_peasant;
use crate::module_bindings::player_table::PlayerTableAccess;
use crate::module_bindings::inventory_table::InventoryTableAccess;
use crate::module_bindings::resource_node_table::ResourceNodeTableAccess;
use crate::module_bindings::structure_table::StructureTableAccess;

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
    pub is_over_ui: bool, 
}

// ----------------------------------------------------------------------------
// SYSTEM PARAM BUNDLING 
// ----------------------------------------------------------------------------

#[derive(SystemParam)]
pub struct ActionContextQueries<'w, 's> {
    pub fps_camera: Query<'w, 's, &'static GlobalTransform, With<FpsCamera>>,
    pub player: Query<'w, 's, (Entity, &'static BevyTransform), With<PlayerBody>>,
    pub node: Query<'w, 's, &'static ResourceNodeItem>,
    pub parent_q: Query<'w, 's, &'static Parent>,
    pub structure: Query<'w, 's, &'static NetworkStructure>,
    pub peasant: Query<'w, 's, &'static PeasantUnit>, 
    pub rts_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<RtsCameraChild>>,
    pub selectable: Query<'w, 's, (Entity, &'static BevyTransform), With<Selectable>>,
    pub selected: Query<'w, 's, Entity, With<Selected>>,
}

#[derive(SystemParam)]
pub struct UiActionQueries<'w, 's> {
    pub build_menu: Query<'w, 's, &'static mut Style, With<BuildMenuRoot>>,
    pub inventory: Query<'w, 's, &'static mut Style, (With<InventoryUiRoot>, Without<BuildMenuRoot>)>,
    pub window: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
}

fn resolve_node_id(entity: Entity, node_q: &Query<&ResourceNodeItem>, parent_q: &Query<&Parent>) -> Option<u64> {
    if let Ok(node) = node_q.get(entity) {
        return Some(node.node_id);
    }
    if let Ok(parent) = parent_q.get(entity) {
        if let Ok(node) = node_q.get(parent.get()) {
            return Some(node.node_id);
        }
    }
    None
}

// ----------------------------------------------------------------------------
// HOTBAR INPUT SYSTEM
// ----------------------------------------------------------------------------

pub fn hotbar_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    mut active_slot: ResMut<ActiveItemSlot>,
    mut active_item: ResMut<ActiveEquippedItem>,
    conn: Res<SpacetimeConnection>,
) {
    if console.is_open {
        return;
    }

    let digit_keys = [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
        (KeyCode::Digit6, 5),
        (KeyCode::Digit7, 6),
        (KeyCode::Digit8, 7),
    ];

    for (key, slot_idx) in digit_keys {
        if keys.just_pressed(key) {
            active_slot.0 = slot_idx;
            info!("Hotbar Slot {} Selected", slot_idx + 1);
        }
    }

    let Some(identity) = &conn.identity else { return; };
    let Some(player) = conn.db.db.player().identity().find(identity) else { return; };
    let Some(inventory) = conn.db.db.inventory().entity_id().find(&player.entity_id) else { return; };

    active_item.0 = inventory.slots.get(active_slot.0)
        .filter(|s| s.count > 0 && !s.item_type.is_empty())
        .map(|s| s.item_type.clone());
}

// ----------------------------------------------------------------------------
// INPUT ROUTING
// ----------------------------------------------------------------------------

pub fn input_router_system(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    drag_drop: Res<DragDropState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut action_events: EventWriter<ActionEvent>,
    interaction_query: Query<&Interaction>,
    node_query: Query<(&Node, &GlobalTransform, &Visibility, &Style)>,
) {
    if console.is_open {
        return;
    }

    let cursor_pos = window_query.get_single().ok().and_then(|w| w.cursor_position());
    
    let mut is_over_ui = interaction_query.iter().any(|i| *i != Interaction::None) || drag_drop.is_dragging;
    
    if !is_over_ui {
        if let Some(pos) = cursor_pos {
            for (node, transform, vis, style) in node_query.iter() {
                if *vis != Visibility::Hidden && style.display != Display::None {
                    if style.width == Val::Percent(100.0) && style.height == Val::Percent(100.0) {
                        continue;
                    }
                    let rect = Rect::from_center_size(transform.translation().truncate(), node.size());
                    if rect.contains(pos) {
                        is_over_ui = true;
                        break;
                    }
                }
            }
        }
    }

    if mouse.just_pressed(MouseButton::Left) {
        action_events.send(ActionEvent { action: VirtualAction::Primary, state: ActionState::JustPressed, cursor_pos, is_over_ui });
    }
    if mouse.pressed(MouseButton::Left) {
        action_events.send(ActionEvent { action: VirtualAction::Primary, state: ActionState::Pressed, cursor_pos, is_over_ui });
    }
    if mouse.just_released(MouseButton::Left) {
        action_events.send(ActionEvent { action: VirtualAction::Primary, state: ActionState::JustReleased, cursor_pos, is_over_ui });
    }

    if mouse.just_pressed(MouseButton::Right) {
        action_events.send(ActionEvent { action: VirtualAction::Secondary, state: ActionState::JustPressed, cursor_pos, is_over_ui });
    }
    
    if keys.just_pressed(KeyCode::KeyE) {
        action_events.send(ActionEvent { action: VirtualAction::Interact, state: ActionState::JustPressed, cursor_pos, is_over_ui });
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
    console: Res<ConsoleState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut swing_state: ResMut<SwingState>,
    active_item: Res<ActiveEquippedItem>,
    queries: ActionContextQueries,
    spatial_query: SpatialQuery,
    mut selection_state: ResMut<SelectionState>,
    conn: Res<SpacetimeConnection>,
    tick: Res<ClientTick>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ui_queries: UiActionQueries,
) {
    if console.is_open {
        return;
    }

    let multi_select = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let my_player_entity = queries.player.get_single().map(|(e, _)| e).unwrap_or(Entity::PLACEHOLDER);

    if keys.just_pressed(KeyCode::KeyP) {
        let _ = conn.db.reducers.spawn_peasant();
    }

    let is_holding_hammer = active_item.0.as_deref() == Some("Hammer");
    let is_holding_bow = active_item.0.as_deref() == Some("Crude Bow");

    for event in action_events.read() {
        match camera_mode.get() {
            CameraMode::FPS => {
                match event.action {
                    VirtualAction::Secondary if event.state == ActionState::JustPressed => {
                        if is_holding_hammer && !event.is_over_ui {
                            if let Ok(mut style) = ui_queries.build_menu.get_single_mut() {
                                let opening = style.display == Display::None;
                                style.display = if opening { Display::Flex } else { Display::None };
                                if let Ok(mut window) = ui_queries.window.get_single_mut() {
                                    window.cursor.grab_mode = if opening { CursorGrabMode::None } else { CursorGrabMode::Locked };
                                    window.cursor.visible = opening;
                                }
                            }
                        }
                    }
                    VirtualAction::Primary if event.state == ActionState::JustPressed => {
                        if event.is_over_ui || build_state.is_active { continue; }

                        if !swing_state.is_swinging {
                            swing_state.is_swinging = true;
                            if let Ok(cam_transform) = queries.fps_camera.get_single() {
                                if let Ok((player_entity, _)) = queries.player.get_single() {
                                    let origin = cam_transform.translation();
                                    let dir = cam_transform.forward();

                                    if is_holding_bow {
                                        let arrow_speed = 45.0;
                                        let tracer_mesh = meshes.add(bevy::math::primitives::Cylinder::new(0.015, 0.8));
                                        let tracer_mat = materials.add(StandardMaterial {
                                            base_color: Color::srgb(0.8, 0.7, 0.5),
                                            unlit: true,
                                            ..default()
                                        });

                                        let mut arrow_transform = BevyTransform::from_translation(origin + dir * 1.0)
                                            .looking_at(origin + dir * 5.0, Vec3::Y);
                                        arrow_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                        commands.spawn((
                                            PbrBundle {
                                                mesh: tracer_mesh,
                                                material: tracer_mat,
                                                transform: arrow_transform,
                                                ..default()
                                            },
                                            RigidBody::Dynamic,
                                            LinearVelocity(dir * arrow_speed),
                                            Particle { timer: Timer::from_seconds(1.2, TimerMode::Once) },
                                        ));

                                        if let Err(e) = conn.db.reducers.fire_bow(
                                            tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                        ) {
                                            error!("Bow error: {:?}", e);
                                        }
                                        continue;
                                    }

                                    let hit = spatial_query.cast_ray(
                                        origin, dir.into(), 50.0, true,
                                        SpatialQueryFilter::from_excluded_entities([player_entity]),
                                    );

                                    let is_tool_context = hit.map_or(false, |hit_data| {
                                        hit_data.time_of_impact < 6.0 && 
                                        (queries.node.contains(hit_data.entity) || 
                                         queries.structure.contains(hit_data.entity) ||
                                         queries.selectable.contains(hit_data.entity))
                                    });

                                    if is_tool_context {
                                        let _ = conn.db.reducers.swing_tool(
                                            origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                        );
                                    } else {
                                        let distance = hit.map_or(50.0, |h| h.time_of_impact);
                                        let mid_point = origin + dir * (distance / 2.0);
                                        let mut tracer_transform = BevyTransform::from_translation(mid_point)
                                            .looking_at(origin + dir * distance, Vec3::Y);
                                        tracer_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);
                                        
                                        commands.spawn((
                                            PbrBundle {
                                                mesh: meshes.add(bevy::math::primitives::Cylinder::new(0.02, distance)),
                                                material: materials.add(StandardMaterial {
                                                    base_color: Color::srgb(1.0, 0.9, 0.5), unlit: true, ..default()
                                                }),
                                                transform: tracer_transform, ..default()
                                            },
                                            Particle { timer: Timer::from_seconds(0.05, TimerMode::Once) },
                                        ));

                                        let _ = conn.db.reducers.fire_weapon(
                                            tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                        );
                                    }
                                }
                            }
                        }
                    }
                    VirtualAction::Interact if event.state == ActionState::JustPressed => {
                        if event.is_over_ui || build_state.is_active { continue; }

                        if let Ok(cam_transform) = queries.fps_camera.get_single() {
                            if let Ok((player_entity, _)) = queries.player.get_single() {
                                let origin = cam_transform.translation();
                                let dir = cam_transform.forward();
                                
                                let hit = spatial_query.cast_ray(
                                    origin, dir.into(), 7.0, true,
                                    SpatialQueryFilter::from_mask([GameLayer::Environment, GameLayer::Default])
                                        .with_excluded_entities([player_entity]),
                                );
                                
                                if let Some(hit_data) = hit {
                                    if let Some(node_id) = resolve_node_id(hit_data.entity, &queries.node, &queries.parent_q) {
                                        info!("Dispatching interact_node for node_id: {}", node_id);
                                        if let Err(e) = conn.db.reducers.interact_node(node_id) {
                                            error!("Failed to pick up resource: {:?}", e);
                                        }
                                    } else if let Ok(net_structure) = queries.structure.get(hit_data.entity) {
                                        if let Some(s) = conn.db.db.structure().structure_id().find(&net_structure.structure_id) {
                                            if s.piece_type == "Workbench" && !s.is_blueprint {
                                                if let Ok(mut style) = ui_queries.inventory.get_single_mut() {
                                                    style.display = Display::Flex;
                                                    if let Ok(mut window) = ui_queries.window.get_single_mut() {
                                                        window.cursor.grab_mode = CursorGrabMode::None;
                                                        window.cursor.visible = true;
                                                    }
                                                }
                                            } else if is_holding_hammer {
                                                if s.is_blueprint {
                                                    let _ = conn.db.reducers.contribute_construction(s.structure_id);
                                                } else if s.current_health < s.max_health {
                                                    let _ = conn.db.reducers.repair_structure(s.structure_id);
                                                }
                                            }
                                        }
                                    }
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
                                if event.is_over_ui { continue; } 

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
                                selection_state.start_pos = None;
                                selection_state.end_pos = None;

                                if event.is_over_ui { continue; } 

                                let dist = start.distance(cursor_pos);
                                if dist < 5.0 {
                                    if let Some(ray) = camera.viewport_to_world(cam_transform, cursor_pos) {
                                        if let Some(hit) = spatial_query.cast_ray(
                                            ray.origin, ray.direction, 1000.0, true,
                                            SpatialQueryFilter::from_mask([GameLayer::Unit, GameLayer::Terrain, GameLayer::Environment]),
                                        ) {
                                            if queries.selectable.contains(hit.entity) {
                                                commands.entity(hit.entity).insert(Selected);
                                            } else {
                                                let hit_point = ray.origin + ray.direction * hit.time_of_impact;
                                                for selected_entity in queries.selected.iter() {
                                                    if queries.peasant.get(selected_entity).is_err() {
                                                        commands.entity(selected_entity).insert(NavTarget(hit_point));
                                                    }
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
                            }
                        }
                    }
                    VirtualAction::Secondary => {
                        if event.state == ActionState::JustPressed {
                            if event.is_over_ui { continue; }

                            let Some(cursor_pos) = event.cursor_pos else { continue; };
                            let Ok((camera, cam_transform)) = queries.rts_camera.get_single() else { continue; };
                            
                            if let Some(ray) = camera.viewport_to_world(cam_transform, cursor_pos) {
                                if let Some(hit) = spatial_query.cast_ray(
                                    ray.origin, ray.direction, 1000.0, true,
                                    SpatialQueryFilter::from_mask([GameLayer::Unit, GameLayer::Terrain, GameLayer::Environment]),
                                ) {
                                    let hit_point = ray.origin + ray.direction * hit.time_of_impact;
                                    let is_node = queries.node.contains(hit.entity);
                                    let is_player = hit.entity == my_player_entity;

                                    commands.spawn((
                                        PbrBundle {
                                            mesh: meshes.add(bevy::math::primitives::Cylinder::new(0.8, 0.05)),
                                            material: materials.add(StandardMaterial {
                                                base_color: if is_node { Color::srgb(0.9, 0.8, 0.1) } else { Color::srgb(0.2, 0.9, 0.3) },
                                                unlit: true,
                                                ..default()
                                            }),
                                            transform: BevyTransform::from_xyz(hit_point.x, hit_point.y + 0.05, hit_point.z),
                                            ..default()
                                        },
                                        Particle { timer: Timer::from_seconds(0.8, TimerMode::Once) },
                                    ));

                                    for selected_entity in queries.selected.iter() {
                                        if let Ok(peasant) = queries.peasant.get(selected_entity) {
                                            if is_node {
                                                let node = queries.node.get(hit.entity).unwrap();
                                                let _ = conn.db.reducers.command_peasant(peasant.entity_id, "Harvest".to_string(), hit_point.x, hit_point.y, hit_point.z, node.node_id);
                                            } else if is_player {
                                                let player_id = conn.identity.as_ref().and_then(|id| conn.db.db.player().identity().find(id)).map(|p| p.entity_id).unwrap_or(0);
                                                let _ = conn.db.reducers.command_peasant(peasant.entity_id, "Return".to_string(), hit_point.x, hit_point.y, hit_point.z, player_id);
                                            } else {
                                                let _ = conn.db.reducers.command_peasant(peasant.entity_id, "MoveTo".to_string(), hit_point.x, hit_point.y, hit_point.z, 0);
                                            }
                                        }
                                    }
                                }
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
    console: Res<ConsoleState>,
    mut query: Query<(Entity, &mut BevyTransform, &mut LinearVelocity, &mut GravityScale, &mut Kcc), With<PlayerBody>>,
    spatial_query: SpatialQuery, 
) {
    let Ok((entity, mut transform, mut lin_vel, mut gravity, mut kcc)) = query.get_single_mut() else { return; };

    let ray_start = transform.translation; 
    let hit = spatial_query.cast_ray(
        ray_start, Dir3::NEG_Y, 1.15, true, 
        SpatialQueryFilter::from_excluded_entities([entity])
    );
    
    kcc.is_grounded = false;
    if let Some(hit_data) = hit {
        if hit_data.time_of_impact <= 1.15 { kcc.is_grounded = true; }
    }

    let ground_y = crate::terrain::get_terrain_height(transform.translation.x, transform.translation.z);
    let player_half_height = 1.05; 
    
    if transform.translation.y <= ground_y + player_half_height {
        transform.translation.y = ground_y + player_half_height;
        if lin_vel.y < 0.0 { lin_vel.y = 0.0; }
        kcc.is_grounded = true;
    }

    let mut move_dir = Vec3::ZERO;
    if *camera_mode.get() == CameraMode::FPS && !console.is_open {
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
    if kcc.is_grounded && *camera_mode.get() == CameraMode::FPS && !console.is_open && keys.just_pressed(KeyCode::Space) { 
        lin_vel.y = 10.0; 
        kcc.is_grounded = false; 
    }
}

// ----------------------------------------------------------------------------
// INTERACTION PROMPT SYSTEM
// ----------------------------------------------------------------------------

pub fn update_interaction_prompt(
    conn: Res<SpacetimeConnection>,
    active_item: Res<ActiveEquippedItem>,
    camera_query: Query<&GlobalTransform, With<FpsCamera>>,
    player_query: Query<Entity, With<PlayerBody>>,
    spatial_query: SpatialQuery,
    node_query: Query<&ResourceNodeItem>,
    parent_query: Query<&Parent>,
    structure_query: Query<&NetworkStructure>,
    mut prompt_query: Query<(&mut Text, &mut Visibility), With<InteractionPromptText>>,
) {
    let Ok(cam_transform) = camera_query.get_single() else { return; };
    let Ok(player_entity) = player_query.get_single() else { return; };
    let Ok((mut text, mut vis)) = prompt_query.get_single_mut() else { return; };

    let origin = cam_transform.translation();
    let dir = cam_transform.forward();

    let hit = spatial_query.cast_ray(
        origin, dir.into(), 7.0, true,
        SpatialQueryFilter::from_mask([GameLayer::Environment, GameLayer::Default])
            .with_excluded_entities([player_entity]),
    );

    if let Some(hit_data) = hit {
        if let Some(node_id) = resolve_node_id(hit_data.entity, &node_query, &parent_query) {
            if let Some(node) = conn.db.db.resource_node().node_id().find(&node_id) {
                let prompt = match node.node_type.as_str() {
                    "Bush" => {
                        if node.health > 0 { "[E] Pick Berries" } else { "Berries Depleted" }
                    }
                    "Branch" => "[E] Pick up Branch",
                    "Flint" => "[E] Pick up Flint",
                    "LooseStone" => "[E] Pick up Stone",
                    "Tree" => "Tree (Left-click with Stone Axe)",
                    "Rock" => "Rock (Left-click with Pickaxe)",
                    _ => "[E] Gather",
                };

                text.sections[0].value = prompt.to_string();
                *vis = Visibility::Inherited;
                return;
            }
        } else if let Ok(net_structure) = structure_query.get(hit_data.entity) {
            if let Some(s) = conn.db.db.structure().structure_id().find(&net_structure.structure_id) {
                let is_hammer = active_item.0.as_deref() == Some("Hammer");
                if s.piece_type == "Workbench" && !s.is_blueprint {
                    text.sections[0].value = "[E] Open Workbench".to_string();
                } else if s.is_blueprint {
                    text.sections[0].value = if is_hammer { 
                        format!("[E] Build {} ({}%)", s.piece_type, s.construction_progress) 
                    } else { 
                        "Equip Hammer to Build".to_string() 
                    };
                } else if s.current_health < s.max_health {
                    text.sections[0].value = if is_hammer {
                        format!("[E] Repair Structure ({:.0}/{:.0} HP)", s.current_health, s.max_health)
                    } else {
                        format!("Damaged ({:.0}/{:.0} HP) - Equip Hammer", s.current_health, s.max_health)
                    };
                } else {
                    text.sections[0].value = format!("{} ({:.0}/{:.0} HP)", s.piece_type, s.current_health, s.max_health);
                }
                *vis = Visibility::Inherited;
                return;
            }
        }
    }

    text.sections[0].value = "".to_string();
    *vis = Visibility::Hidden;
}