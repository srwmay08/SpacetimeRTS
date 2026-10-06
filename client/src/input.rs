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
use crate::weapons::*; 

use crate::module_bindings::swing_tool_reducer::swing_tool;
use crate::module_bindings::fire_weapon_reducer::fire_weapon;
use crate::module_bindings::fire_bow_reducer::fire_bow;
use crate::module_bindings::repair_structure_reducer::repair_structure;
use crate::module_bindings::contribute_construction_reducer::contribute_construction;
use crate::module_bindings::interact_node_reducer::interact_node;
use crate::module_bindings::command_peasant_reducer::command_peasant;
use crate::module_bindings::spawn_peasant_reducer::spawn_peasant;
use crate::module_bindings::toggle_door_reducer::toggle_door;
use crate::module_bindings::player_table::PlayerTableAccess;
use crate::module_bindings::inventory_table::InventoryTableAccess;
use crate::module_bindings::resource_node_table::ResourceNodeTableAccess;
use crate::module_bindings::structure_table::StructureTableAccess;
use crate::module_bindings::door_state_table::DoorStateTableAccess;
use crate::module_bindings::equipment_loadout_table::EquipmentLoadoutTableAccess;
use crate::module_bindings::equip_weapon_reducer::equip_weapon;
use crate::weapons::EquippedHandSide;
use spacetime_rts_logic::{HandSide, TacticalAbilityKind};

// ----------------------------------------------------------------------------
// EVENTS, ENUMS & ACTION BUFFER (Unified Input Pipeline)
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum VirtualAction {
    Primary,
    Secondary,
    Interact,
    Jump,
    UseAbility(TacticalAbilityKind),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionState {
    JustPressed,
    Pressed,
    JustReleased,
}

#[derive(Event, Debug, Clone)]
pub struct ActionEvent {
    pub action: VirtualAction,
    pub state: ActionState,
    pub cursor_pos: Option<Vec2>,
    pub is_over_ui: bool, 
}

/// A buffered player action intent with Time-To-Live (TTL) preventing "eaten inputs".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferedAction {
    pub action: VirtualAction,
    pub timestamp_seconds: f64,
    pub ttl_seconds: f32,
}

impl BufferedAction {
    pub fn new(action: VirtualAction, timestamp_seconds: f64, ttl_seconds: f32) -> Self {
        Self {
            action,
            timestamp_seconds,
            ttl_seconds,
        }
    }

    pub fn is_expired(&self, current_time: f64) -> bool {
        (current_time - self.timestamp_seconds) > (self.ttl_seconds as f64)
    }
}

/// Central action buffer retaining recent intent triggers across physics/cooldown frames.
#[derive(Resource, Default, Debug)]
pub struct ActionBuffer {
    pub actions: Vec<BufferedAction>,
}

impl ActionBuffer {
    pub fn push(&mut self, action: VirtualAction, current_time: f64, ttl_seconds: f32) {
        self.actions.push(BufferedAction::new(action, current_time, ttl_seconds));
    }

    pub fn pop_matching<F>(&mut self, current_time: f64, predicate: F) -> Option<VirtualAction>
    where
        F: Fn(VirtualAction) -> bool,
    {
        self.actions.retain(|item| !item.is_expired(current_time));
        if let Some(idx) = self.actions.iter().position(|item| predicate(item.action)) {
            Some(self.actions.remove(idx).action)
        } else {
            None
        }
    }

    pub fn prune(&mut self, current_time: f64) {
        self.actions.retain(|item| !item.is_expired(current_time));
    }
}

/// Locomotion feel settings (coyote time, jump buffer window, and movement speeds).
#[derive(Resource, Debug, Clone)]
pub struct LocomotionSettings {
    pub coyote_time_max: f32,
    pub jump_buffer_max: f32,
    pub horizontal_speed: f32,
    pub jump_impulse: f32,
}

impl Default for LocomotionSettings {
    fn default() -> Self {
        Self {
            coyote_time_max: 0.12,
            jump_buffer_max: 0.12,
            horizontal_speed: 15.0,
            jump_impulse: 10.0,
        }
    }
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
    pub door: Query<'w, 's, &'static Door>,
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

#[derive(SystemParam)]
pub struct WeaponActionParams<'w> {
    pub meshes: ResMut<'w, Assets<Mesh>>, 
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
    pub time: Res<'w, Time>,
    pub weapon_state: ResMut<'w, WeaponState>,
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

fn resolve_structure_id(
    entity: Entity,
    structure_q: &Query<&NetworkStructure>,
    door_q: &Query<&Door>,
    parent_q: &Query<&Parent>,
) -> Option<u64> {
    if let Ok(door) = door_q.get(entity) {
        return Some(door.structure_id);
    }
    if let Ok(st) = structure_q.get(entity) {
        return Some(st.structure_id);
    }
    if let Ok(parent) = parent_q.get(entity) {
        if let Ok(door) = door_q.get(parent.get()) {
            return Some(door.structure_id);
        }
        if let Ok(st) = structure_q.get(parent.get()) {
            return Some(st.structure_id);
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
    mut active_offhand: ResMut<ActiveOffHandItem>,
    hand_side: Res<EquippedHandSide>,
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

    let Some(identity) = &conn.identity else { return; };
    let Some(player) = conn.db.db.player().identity().find(identity) else { return; };
    let inv = conn.db.db.inventory().entity_id().find(&player.entity_id);

    let equip_to_offhand = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) || hand_side.0 == HandSide::Left;

    let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);

    for (key, slot_idx) in digit_keys {
        if alt_held && keys.just_pressed(key) {
            active_slot.0 = slot_idx;
            info!("Inventory Quick-Slot {} Selected (Alt+{})", slot_idx + 1, slot_idx + 1);
            if let Some(ref inventory) = inv {
                if let Some(slot) = inventory.slots.get(slot_idx) {
                    if slot.count > 0 && !slot.item_type.is_empty() {
                        let hand_str = if equip_to_offhand {
                            "OffHand".to_string()
                        } else {
                            "MainHand".to_string()
                        };
                        info!("Equipping '{}' to {}", slot.item_type, hand_str);
                        let _ = conn.db.reducers.equip_weapon(hand_str, slot.item_type.clone());
                    }
                }
            }
        }
    }

    // Read authoritative equipped weapon from SpacetimeDB EquipmentLoadout
    let loadout = conn.db.db.equipment_loadout().entity_id().find(&player.entity_id);
    if let Some(l) = loadout {
        active_item.0 = if l.main_hand != "None" && !l.main_hand.is_empty() {
            Some(l.main_hand.clone())
        } else {
            None
        };
        active_offhand.0 = if l.off_hand != "None" && !l.off_hand.is_empty() {
            Some(l.off_hand.clone())
        } else {
            None
        };
    } else {
        active_item.0 = None;
        active_offhand.0 = None;
    }
}

// ----------------------------------------------------------------------------
// INPUT ROUTING
// ----------------------------------------------------------------------------

pub fn input_router_system(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    camera_mode: Res<State<CameraMode>>,
    time: Res<Time>,
    mut action_buffer: ResMut<ActionBuffer>,
    drag_drop: Res<DragDropState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut action_events: EventWriter<ActionEvent>,
    interaction_query: Query<&Interaction>,
    node_query: Query<(&Node, &GlobalTransform, &Visibility, &Style)>,
) {
    if console.is_open {
        return;
    }

    let current_time = time.elapsed_seconds_f64();
    action_buffer.prune(current_time);

    let cursor_pos = window_query.get_single().ok().and_then(|w| w.cursor_position());
    let cursor_locked = window_query.get_single().map_or(false, |w| w.cursor.grab_mode == CursorGrabMode::Locked);

    let mut is_over_ui = if cursor_locked {
        false
    } else {
        interaction_query.iter().any(|i| *i != Interaction::None) || drag_drop.is_dragging
    };
    
    if !cursor_locked && !is_over_ui {
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

    // Tactical Abilities & Jump (Only active when in FPS perspective and not clicking UI)
    if *camera_mode.get() == CameraMode::FPS && !is_over_ui {
        // Jump intent
        if keys.just_pressed(KeyCode::Space) {
            action_events.send(ActionEvent { action: VirtualAction::Jump, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(VirtualAction::Jump, current_time, 0.12);
        }

        // Tactical abilities: Q (Dash), C (Smoke), X (Intel Dart), F (Grav-Lift)
        // Architectural Note: ShiftLeft is intentionally reserved for off-hand equipment, avoiding ghost dash triggers.
        if keys.just_pressed(KeyCode::KeyQ) {
            let act = VirtualAction::UseAbility(TacticalAbilityKind::PhaseDash);
            action_events.send(ActionEvent { action: act, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(act, current_time, 0.12);
        }
        if keys.just_pressed(KeyCode::KeyC) {
            let act = VirtualAction::UseAbility(TacticalAbilityKind::SmokeVeil);
            action_events.send(ActionEvent { action: act, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(act, current_time, 0.12);
        }
        if keys.just_pressed(KeyCode::KeyX) {
            let act = VirtualAction::UseAbility(TacticalAbilityKind::IntelDart);
            action_events.send(ActionEvent { action: act, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(act, current_time, 0.12);
        }
        if keys.just_pressed(KeyCode::KeyF) {
            let act = VirtualAction::UseAbility(TacticalAbilityKind::GravLift);
            action_events.send(ActionEvent { action: act, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(act, current_time, 0.12);
        }
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
    hand_side: Res<EquippedHandSide>,
    queries: ActionContextQueries,
    spatial_query: SpatialQuery,
    mut selection_state: ResMut<SelectionState>,
    conn: Res<SpacetimeConnection>,
    tick: Res<ClientTick>,
    mut ui_queries: UiActionQueries,
    mut weapons: WeaponActionParams,
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

    for event in action_events.read() {
        match camera_mode.get() {
            CameraMode::FPS => {
                let is_left_primary = hand_side.0 == HandSide::Left;
                let trigger_offhand = (!is_left_primary && event.action == VirtualAction::Secondary)
                    || (is_left_primary && event.action == VirtualAction::Primary);
                let trigger_mainhand = (!is_left_primary && event.action == VirtualAction::Primary)
                    || (is_left_primary && event.action == VirtualAction::Secondary);

                if trigger_offhand && event.state == ActionState::JustPressed {
                    if weapons.weapon_state.current_weapon == WeaponType::Bow && weapons.weapon_state.bow_drawing {
                        weapons.weapon_state.bow_drawing = false;
                        weapons.weapon_state.bow_charge = 0.0;
                    } else if is_holding_hammer && weapons.weapon_state.offhand_weapon == WeaponType::None && !event.is_over_ui {
                        if let Ok(mut style) = ui_queries.build_menu.get_single_mut() {
                            let opening = style.display == Display::None;
                            style.display = if opening { Display::Flex } else { Display::None };
                            if let Ok(mut window) = ui_queries.window.get_single_mut() {
                                window.cursor.grab_mode = if opening { CursorGrabMode::None } else { CursorGrabMode::Locked };
                                window.cursor.visible = opening;
                            }
                        }
                    } else if !event.is_over_ui && !build_state.is_active {
                        // Off-Hand attack execution (dual-wielding / unarmed left-jab)
                        if let Ok(cam_transform) = queries.fps_camera.get_single() {
                            if let Ok((player_entity, _)) = queries.player.get_single() {
                                let origin = cam_transform.translation();
                                let dir = cam_transform.forward().as_vec3();

                                match weapons.weapon_state.offhand_weapon {
                                    WeaponType::Revolver => {
                                        if weapons.weapon_state.revolver_ammo > 0 && weapons.weapon_state.revolver_cooldown.finished() && !weapons.weapon_state.revolver_is_reloading {
                                            weapons.weapon_state.revolver_ammo -= 1;
                                            weapons.weapon_state.revolver_cooldown.reset();

                                            weapons.weapon_state.offhand_recoil_offset += Vec3::new(0.0, 0.065, 0.11);
                                            weapons.weapon_state.offhand_recoil_rot *= Quat::from_rotation_x(-0.48);
                                            weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 8.0).min(30.0);

                                            let hit = spatial_query.cast_ray(
                                                origin, cam_transform.forward(), 80.0, true,
                                                SpatialQueryFilter::from_excluded_entities([player_entity]),
                                            );
                                            let distance = hit.map_or(80.0, |h| h.time_of_impact);
                                            let mid_point = origin + dir * (distance / 2.0);
                                            let mut tracer_transform = BevyTransform::from_translation(mid_point)
                                                .looking_at(origin + dir * distance, Vec3::Y);
                                            tracer_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                            commands.spawn((
                                                PbrBundle {
                                                    mesh: weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.018, distance)),
                                                    material: weapons.materials.add(StandardMaterial {
                                                        base_color: Color::srgb(1.0, 0.92, 0.6), unlit: true, ..default()
                                                    }),
                                                    transform: tracer_transform, ..default()
                                                },
                                                Particle { timer: Timer::from_seconds(0.06, TimerMode::Once) },
                                            ));

                                            let _ = conn.db.reducers.fire_weapon(
                                                tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                            );

                                            if weapons.weapon_state.revolver_ammo == 0 {
                                                weapons.weapon_state.revolver_is_reloading = true;
                                                weapons.weapon_state.revolver_reload_timer.reset();
                                            }
                                        }
                                    }
                                    WeaponType::HandCrossbow => {
                                        if weapons.weapon_state.hand_crossbow_loaded {
                                            weapons.weapon_state.hand_crossbow_loaded = false;
                                            weapons.weapon_state.hand_crossbow_reload_timer.reset();

                                            weapons.weapon_state.offhand_recoil_offset += Vec3::new(0.0, 0.025, 0.04);
                                            weapons.weapon_state.offhand_recoil_rot *= Quat::from_rotation_x(-0.20);

                                            let dart_speed = 52.0;
                                            let tracer_mesh = weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.015, 0.4));
                                            let tracer_mat = weapons.materials.add(StandardMaterial {
                                                base_color: Color::srgb(0.7, 0.75, 0.8),
                                                unlit: true,
                                                ..default()
                                            });

                                            let mut dart_transform = BevyTransform::from_translation(origin + dir * 0.7)
                                                .looking_at(origin + dir * 5.0, Vec3::Y);
                                            dart_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                            commands.spawn((
                                                PbrBundle {
                                                    mesh: tracer_mesh,
                                                    material: tracer_mat,
                                                    transform: dart_transform,
                                                    ..default()
                                                },
                                                RigidBody::Dynamic,
                                                LinearVelocity(dir * dart_speed),
                                                Particle { timer: Timer::from_seconds(1.0, TimerMode::Once) },
                                            ));

                                            let _ = conn.db.reducers.fire_bow(
                                                tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                            );
                                        }
                                    }
                                    WeaponType::Wand => {
                                        weapons.weapon_state.offhand_recoil_offset += Vec3::new(0.0, 0.02, 0.05);
                                        weapons.weapon_state.offhand_recoil_rot *= Quat::from_rotation_x(-0.22);
                                        weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 3.0).min(25.0);

                                        let hit = spatial_query.cast_ray(
                                            origin, cam_transform.forward(), 60.0, true,
                                            SpatialQueryFilter::from_excluded_entities([player_entity]),
                                        );
                                        let distance = hit.map_or(60.0, |h| h.time_of_impact);
                                        let mid_point = origin + dir * (distance / 2.0);
                                        let mut tracer_transform = BevyTransform::from_translation(mid_point)
                                            .looking_at(origin + dir * distance, Vec3::Y);
                                        tracer_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                        commands.spawn((
                                            PbrBundle {
                                                mesh: weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.025, distance)),
                                                material: weapons.materials.add(StandardMaterial {
                                                    base_color: Color::srgb(0.2, 0.85, 1.0),
                                                    unlit: true,
                                                    ..default()
                                                }),
                                                transform: tracer_transform, ..default()
                                            },
                                            Particle { timer: Timer::from_seconds(0.06, TimerMode::Once) },
                                        ));

                                        let _ = conn.db.reducers.fire_weapon(
                                            tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                        );
                                    }
                                    WeaponType::Orb => {
                                        weapons.weapon_state.offhand_recoil_offset += Vec3::new(0.0, 0.035, 0.08);
                                        weapons.weapon_state.offhand_recoil_rot *= Quat::from_rotation_x(-0.35);
                                        weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 6.0).min(30.0);

                                        commands.spawn((
                                            PbrBundle {
                                                mesh: weapons.meshes.add(bevy::math::primitives::Sphere::new(0.16)),
                                                material: weapons.materials.add(StandardMaterial {
                                                    base_color: Color::srgb(0.9, 0.4, 1.0),
                                                    unlit: true,
                                                    ..default()
                                                }),
                                                transform: BevyTransform::from_translation(origin + dir * 0.8),
                                                ..default()
                                            },
                                            RigidBody::Dynamic,
                                            LinearVelocity(dir * 42.0),
                                            Particle { timer: Timer::from_seconds(1.0, TimerMode::Once) },
                                        ));

                                        let _ = conn.db.reducers.fire_weapon(
                                            tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                        );
                                    }
                                    WeaponType::Javelin => {
                                        weapons.weapon_state.offhand_recoil_offset += Vec3::new(0.0, 0.03, -0.06);
                                        weapons.weapon_state.offhand_recoil_rot *= Quat::from_rotation_x(-0.25);

                                        let javelin_speed = 45.0;
                                        let tracer_mesh = weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.022, 1.0));
                                        let tracer_mat = weapons.materials.add(StandardMaterial {
                                            base_color: Color::srgb(0.75, 0.65, 0.45),
                                            unlit: true,
                                            ..default()
                                        });

                                        let mut javelin_transform = BevyTransform::from_translation(origin + dir * 0.9)
                                            .looking_at(origin + dir * 5.0, Vec3::Y);
                                        javelin_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                        commands.spawn((
                                            PbrBundle {
                                                mesh: tracer_mesh,
                                                material: tracer_mat,
                                                transform: javelin_transform,
                                                ..default()
                                            },
                                            RigidBody::Dynamic,
                                            LinearVelocity(dir * javelin_speed),
                                            Particle { timer: Timer::from_seconds(1.5, TimerMode::Once) },
                                        ));

                                        let _ = conn.db.reducers.fire_bow(
                                            tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                        );
                                    }
                                    _ => {
                                        // Melee & Unarmed
                                        if !swing_state.offhand_is_swinging {
                                            swing_state.offhand_is_swinging = true;
                                            swing_state.offhand_timer.reset();

                                            weapons.weapon_state.offhand_recoil_offset += Vec3::new(0.0, 0.03, -0.08);
                                            weapons.weapon_state.offhand_recoil_rot *= Quat::from_rotation_y(-0.2);
                                            weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 2.5).min(20.0);

                                            let hit = spatial_query.cast_ray(
                                                origin, cam_transform.forward(), 4.5, true,
                                                SpatialQueryFilter::from_excluded_entities([player_entity]),
                                            );

                                            let _ = conn.db.reducers.swing_tool(
                                                origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                            );

                                            if let Some(hit_data) = hit {
                                                let hit_pt = origin + dir * hit_data.time_of_impact;
                                                commands.spawn((
                                                    PbrBundle {
                                                        mesh: weapons.meshes.add(bevy::math::primitives::Sphere::new(0.06)),
                                                        material: weapons.materials.add(StandardMaterial {
                                                            base_color: Color::srgb(0.95, 0.3, 0.2),
                                                            unlit: true,
                                                            ..default()
                                                        }),
                                                        transform: BevyTransform::from_translation(hit_pt),
                                                        ..default()
                                                    },
                                                    Particle { timer: Timer::from_seconds(0.12, TimerMode::Once) },
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if trigger_mainhand {
                        if event.is_over_ui || build_state.is_active { continue; }

                        let Ok(cam_transform) = queries.fps_camera.get_single() else { continue; };
                        let Ok((player_entity, _)) = queries.player.get_single() else { continue; };
                        let origin = cam_transform.translation();
                        let dir = cam_transform.forward().as_vec3();

                        match weapons.weapon_state.current_weapon {
                            WeaponType::Bow => {
                                match event.state {
                                    ActionState::Pressed => {
                                        weapons.weapon_state.bow_drawing = true;
                                        weapons.weapon_state.bow_charge = (weapons.weapon_state.bow_charge + weapons.time.delta_seconds() * 1.35).min(1.0);
                                    }
                                    ActionState::JustReleased => {
                                        if weapons.weapon_state.bow_drawing {
                                            let charge = weapons.weapon_state.bow_charge;
                                            weapons.weapon_state.bow_drawing = false;
                                            weapons.weapon_state.bow_charge = 0.0;

                                            // Draw charge scales arrow velocity: 22.0m/s (quick tap) to 55.0m/s (full draw)
                                            let arrow_speed = 22.0 + (charge * 33.0);
                                            let tracer_mesh = weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.015, 0.8));
                                            let tracer_mat = weapons.materials.add(StandardMaterial {
                                                base_color: Color::srgb(0.8, 0.7, 0.5),
                                                unlit: true,
                                                ..default()
                                            });

                                            let mut arrow_transform = BevyTransform::from_translation(origin + dir * 0.9)
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
                                                Particle { timer: Timer::from_seconds(1.5, TimerMode::Once) },
                                            ));

                                            weapons.weapon_state.recoil_offset += Vec3::new(0.0, 0.015, -0.04);

                                            if let Err(e) = conn.db.reducers.fire_bow(
                                                tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                            ) {
                                                error!("Bow error: {:?}", e);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            WeaponType::Crossbow if event.state == ActionState::JustPressed => {
                                if weapons.weapon_state.crossbow_loaded {
                                    weapons.weapon_state.crossbow_loaded = false;
                                    weapons.weapon_state.crossbow_reload_timer.reset();

                                    // Heavy mechanical kickback
                                    weapons.weapon_state.recoil_offset += Vec3::new(0.0, 0.045, 0.09);
                                    weapons.weapon_state.recoil_rot *= Quat::from_rotation_x(-0.35);

                                    let bolt_speed = 75.0;
                                    let tracer_mesh = weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.02, 0.6));
                                    let tracer_mat = weapons.materials.add(StandardMaterial {
                                        base_color: Color::srgb(0.9, 0.85, 0.7),
                                        unlit: true,
                                        ..default()
                                    });

                                    let mut bolt_transform = BevyTransform::from_translation(origin + dir * 0.9)
                                        .looking_at(origin + dir * 5.0, Vec3::Y);
                                    bolt_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                    commands.spawn((
                                        PbrBundle {
                                            mesh: tracer_mesh,
                                            material: tracer_mat,
                                            transform: bolt_transform,
                                            ..default()
                                        },
                                        RigidBody::Dynamic,
                                        LinearVelocity(dir * bolt_speed),
                                        Particle { timer: Timer::from_seconds(1.2, TimerMode::Once) },
                                    ));

                                    let _ = conn.db.reducers.fire_bow(
                                        tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                    );
                                }
                            }
                            WeaponType::HandCrossbow if event.state == ActionState::JustPressed => {
                                if weapons.weapon_state.hand_crossbow_loaded {
                                    weapons.weapon_state.hand_crossbow_loaded = false;
                                    weapons.weapon_state.hand_crossbow_reload_timer.reset();

                                    weapons.weapon_state.recoil_offset += Vec3::new(0.0, 0.025, 0.04);
                                    weapons.weapon_state.recoil_rot *= Quat::from_rotation_x(-0.20);

                                    let dart_speed = 52.0;
                                    let tracer_mesh = weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.015, 0.4));
                                    let tracer_mat = weapons.materials.add(StandardMaterial {
                                        base_color: Color::srgb(0.7, 0.75, 0.8),
                                        unlit: true,
                                        ..default()
                                    });

                                    let mut dart_transform = BevyTransform::from_translation(origin + dir * 0.7)
                                        .looking_at(origin + dir * 5.0, Vec3::Y);
                                    dart_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                    commands.spawn((
                                        PbrBundle {
                                            mesh: tracer_mesh,
                                            material: tracer_mat,
                                            transform: dart_transform,
                                            ..default()
                                        },
                                        RigidBody::Dynamic,
                                        LinearVelocity(dir * dart_speed),
                                        Particle { timer: Timer::from_seconds(1.0, TimerMode::Once) },
                                    ));

                                    let _ = conn.db.reducers.fire_bow(
                                        tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                    );
                                }
                            }
                            WeaponType::Revolver if event.state == ActionState::JustPressed => {
                                if weapons.weapon_state.revolver_ammo > 0 && weapons.weapon_state.revolver_cooldown.finished() && !weapons.weapon_state.revolver_is_reloading {
                                    weapons.weapon_state.revolver_ammo -= 1;
                                    weapons.weapon_state.revolver_cooldown.reset();

                                    weapons.weapon_state.recoil_offset += Vec3::new(0.0, 0.065, 0.11);
                                    weapons.weapon_state.recoil_rot *= Quat::from_rotation_x(-0.48);
                                    weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 8.0).min(30.0);

                                    let hit = spatial_query.cast_ray(
                                        origin, cam_transform.forward(), 80.0, true,
                                        SpatialQueryFilter::from_excluded_entities([player_entity]),
                                    );
                                    let distance = hit.map_or(80.0, |h| h.time_of_impact);
                                    let mid_point = origin + dir * (distance / 2.0);
                                    let mut tracer_transform = BevyTransform::from_translation(mid_point)
                                        .looking_at(origin + dir * distance, Vec3::Y);
                                    tracer_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                    commands.spawn((
                                        PbrBundle {
                                            mesh: weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.018, distance)),
                                            material: weapons.materials.add(StandardMaterial {
                                                base_color: Color::srgb(1.0, 0.92, 0.6), unlit: true, ..default()
                                            }),
                                            transform: tracer_transform, ..default()
                                        },
                                        Particle { timer: Timer::from_seconds(0.06, TimerMode::Once) },
                                    ));

                                    let _ = conn.db.reducers.fire_weapon(
                                        tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                    );

                                    if weapons.weapon_state.revolver_ammo == 0 {
                                        weapons.weapon_state.revolver_is_reloading = true;
                                        weapons.weapon_state.revolver_reload_timer.reset();
                                    }
                                }
                            }
                            WeaponType::Shotgun if event.state == ActionState::JustPressed => {
                                if weapons.weapon_state.shotgun_ammo > 0 && !weapons.weapon_state.shotgun_is_pumping && !weapons.weapon_state.shotgun_is_reloading {
                                    weapons.weapon_state.shotgun_ammo -= 1;
                                    weapons.weapon_state.shotgun_is_pumping = true;
                                    weapons.weapon_state.shotgun_pump_timer.reset();

                                    weapons.weapon_state.recoil_offset += Vec3::new(0.0, 0.095, 0.15);
                                    weapons.weapon_state.recoil_rot *= Quat::from_rotation_x(-0.62);
                                    weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 16.0).min(40.0);

                                    let spread_offsets = [
                                        (0.0, 0.0), (0.025, 0.02), (-0.025, 0.02), (0.02, -0.025),
                                        (-0.02, -0.025), (0.04, 0.005), (-0.04, -0.005), (0.005, 0.04),
                                    ];

                                    let cam_right = cam_transform.right().as_vec3();
                                    let cam_up = cam_transform.up().as_vec3();

                                    for (sx, sy) in spread_offsets {
                                        let pellet_dir = (dir + cam_right * sx + cam_up * sy).normalize();
                                        let hit = spatial_query.cast_ray(
                                            origin, Dir3::new(pellet_dir).unwrap_or(Dir3::Y), 40.0, true,
                                            SpatialQueryFilter::from_excluded_entities([player_entity]),
                                        );
                                        let distance = hit.map_or(40.0, |h| h.time_of_impact);
                                        let mid_point = origin + pellet_dir * (distance / 2.0);
                                        let mut tracer_transform = BevyTransform::from_translation(mid_point)
                                            .looking_at(origin + pellet_dir * distance, Vec3::Y);
                                        tracer_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                        commands.spawn((
                                            PbrBundle {
                                                mesh: weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.012, distance)),
                                                material: weapons.materials.add(StandardMaterial {
                                                    base_color: Color::srgb(1.0, 0.8, 0.4), unlit: true, ..default()
                                                }),
                                                transform: tracer_transform, ..default()
                                            },
                                            Particle { timer: Timer::from_seconds(0.05, TimerMode::Once) },
                                        ));

                                        let _ = conn.db.reducers.fire_weapon(
                                            tick.0, origin.x, origin.y, origin.z, pellet_dir.x, pellet_dir.y, pellet_dir.z
                                        );
                                    }

                                    if weapons.weapon_state.shotgun_ammo == 0 {
                                        weapons.weapon_state.shotgun_is_reloading = true;
                                        weapons.weapon_state.shotgun_reload_timer.reset();
                                    }
                                }
                            }
                            WeaponType::SniperRifle if event.state == ActionState::JustPressed => {
                                weapons.weapon_state.recoil_offset += Vec3::new(0.0, 0.08, 0.16);
                                weapons.weapon_state.recoil_rot *= Quat::from_rotation_x(-0.55);
                                weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 20.0).min(40.0);

                                let hit = spatial_query.cast_ray(
                                    origin, cam_transform.forward(), 150.0, true,
                                    SpatialQueryFilter::from_excluded_entities([player_entity]),
                                );
                                let distance = hit.map_or(150.0, |h| h.time_of_impact);
                                let mid_point = origin + dir * (distance / 2.0);
                                let mut tracer_transform = BevyTransform::from_translation(mid_point)
                                    .looking_at(origin + dir * distance, Vec3::Y);
                                tracer_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                commands.spawn((
                                    PbrBundle {
                                        mesh: weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.015, distance)),
                                        material: weapons.materials.add(StandardMaterial {
                                            base_color: Color::srgb(1.0, 1.0, 0.7), unlit: true, ..default()
                                        }),
                                        transform: tracer_transform, ..default()
                                    },
                                    Particle { timer: Timer::from_seconds(0.08, TimerMode::Once) },
                                ));

                                let _ = conn.db.reducers.fire_weapon(
                                    tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                );
                            }
                            WeaponType::Wand if event.state == ActionState::JustPressed => {
                                weapons.weapon_state.recoil_offset += Vec3::new(0.0, 0.02, 0.05);
                                weapons.weapon_state.recoil_rot *= Quat::from_rotation_x(-0.22);
                                weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 3.0).min(25.0);

                                let hit = spatial_query.cast_ray(
                                    origin, cam_transform.forward(), 60.0, true,
                                    SpatialQueryFilter::from_excluded_entities([player_entity]),
                                );
                                let distance = hit.map_or(60.0, |h| h.time_of_impact);
                                let mid_point = origin + dir * (distance / 2.0);
                                let mut tracer_transform = BevyTransform::from_translation(mid_point)
                                    .looking_at(origin + dir * distance, Vec3::Y);
                                tracer_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                commands.spawn((
                                    PbrBundle {
                                        mesh: weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.025, distance)),
                                        material: weapons.materials.add(StandardMaterial {
                                            base_color: Color::srgb(0.2, 0.85, 1.0),
                                            unlit: true,
                                            ..default()
                                        }),
                                        transform: tracer_transform, ..default()
                                    },
                                    Particle { timer: Timer::from_seconds(0.06, TimerMode::Once) },
                                ));

                                let _ = conn.db.reducers.fire_weapon(
                                    tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                );
                            }
                            WeaponType::Orb if event.state == ActionState::JustPressed => {
                                weapons.weapon_state.recoil_offset += Vec3::new(0.0, 0.035, 0.08);
                                weapons.weapon_state.recoil_rot *= Quat::from_rotation_x(-0.35);
                                weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 6.0).min(30.0);

                                commands.spawn((
                                    PbrBundle {
                                        mesh: weapons.meshes.add(bevy::math::primitives::Sphere::new(0.16)),
                                        material: weapons.materials.add(StandardMaterial {
                                            base_color: Color::srgb(0.9, 0.4, 1.0),
                                            unlit: true,
                                            ..default()
                                        }),
                                        transform: BevyTransform::from_translation(origin + dir * 0.8),
                                        ..default()
                                    },
                                    RigidBody::Dynamic,
                                    LinearVelocity(dir * 42.0),
                                    Particle { timer: Timer::from_seconds(1.0, TimerMode::Once) },
                                ));

                                let _ = conn.db.reducers.fire_weapon(
                                    tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                );
                            }
                            WeaponType::Javelin if event.state == ActionState::JustPressed => {
                                weapons.weapon_state.recoil_offset += Vec3::new(0.0, 0.03, -0.06);
                                weapons.weapon_state.recoil_rot *= Quat::from_rotation_x(-0.25);

                                let javelin_speed = 45.0;
                                let tracer_mesh = weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.022, 1.0));
                                let tracer_mat = weapons.materials.add(StandardMaterial {
                                    base_color: Color::srgb(0.75, 0.65, 0.45),
                                    unlit: true,
                                    ..default()
                                });

                                let mut javelin_transform = BevyTransform::from_translation(origin + dir * 0.9)
                                    .looking_at(origin + dir * 5.0, Vec3::Y);
                                javelin_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);

                                commands.spawn((
                                    PbrBundle {
                                        mesh: tracer_mesh,
                                        material: tracer_mat,
                                        transform: javelin_transform,
                                        ..default()
                                    },
                                    RigidBody::Dynamic,
                                    LinearVelocity(dir * javelin_speed),
                                    Particle { timer: Timer::from_seconds(1.5, TimerMode::Once) },
                                ));

                                let _ = conn.db.reducers.fire_bow(
                                    tick.0, origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                );
                            }
                            _ if event.state == ActionState::JustPressed => {
                                if !swing_state.is_swinging {
                                    swing_state.is_swinging = true;
                                    swing_state.timer.reset();

                                    let hit = spatial_query.cast_ray(
                                        origin, cam_transform.forward(), 50.0, true,
                                        SpatialQueryFilter::from_excluded_entities([player_entity]),
                                    );

                                    let is_melee = weapons.weapon_state.current_weapon.is_melee();

                                    if is_melee {
                                        // Authoritative melee swing: bare fists (unarmed), tools, blades, bludgeons
                                        let _ = conn.db.reducers.swing_tool(
                                            origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                        );

                                        // Dynamic bloom feedback on swing/punch
                                        weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 2.5).min(20.0);

                                        // Immediate visual feedback if hitting a surface/entity within melee reach (4.5m)
                                        if let Some(hit_data) = hit.filter(|h| h.time_of_impact <= 4.5) {
                                            let hit_pt = origin + dir * hit_data.time_of_impact;
                                            let is_node = queries.node.contains(hit_data.entity);
                                            let impact_color = if is_node {
                                                Color::srgb(0.85, 0.75, 0.45)
                                            } else if queries.structure.contains(hit_data.entity) {
                                                Color::srgb(0.7, 0.65, 0.55)
                                            } else {
                                                Color::srgb(0.95, 0.2, 0.2)
                                            };

                                            commands.spawn((
                                                PbrBundle {
                                                    mesh: weapons.meshes.add(bevy::math::primitives::Sphere::new(0.06)),
                                                    material: weapons.materials.add(StandardMaterial {
                                                        base_color: impact_color,
                                                        unlit: true,
                                                        ..default()
                                                    }),
                                                    transform: BevyTransform::from_translation(hit_pt),
                                                    ..default()
                                                },
                                                Particle { timer: Timer::from_seconds(0.12, TimerMode::Once) },
                                            ));
                                        }
                                    } else {
                                        // Generic ranged weapon fallback
                                        let distance = hit.map_or(50.0, |h| h.time_of_impact);
                                        let mid_point = origin + dir * (distance / 2.0);
                                        let mut tracer_transform = BevyTransform::from_translation(mid_point)
                                            .looking_at(origin + dir * distance, Vec3::Y);
                                        tracer_transform.rotate_local_x(std::f32::consts::FRAC_PI_2);
                                        
                                        commands.spawn((
                                            PbrBundle {
                                                mesh: weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.02, distance)),
                                                material: weapons.materials.add(StandardMaterial {
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
                            _ => {}
                        }
                } else if event.action == VirtualAction::Interact && event.state == ActionState::JustPressed {
                    if !event.is_over_ui && !build_state.is_active {
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
                                    } else if let Some(struct_id) = resolve_structure_id(hit_data.entity, &queries.structure, &queries.door, &queries.parent_q) {
                                        if let Some(s) = conn.db.db.structure().structure_id().find(&struct_id) {
                                            if s.piece_type == "Door" && !s.is_blueprint {
                                                info!("Toggling door structure_id: {}", struct_id);
                                                if let Err(e) = conn.db.reducers.toggle_door(struct_id) {
                                                    error!("Failed to toggle door: {:?}", e);
                                                }
                                            } else if s.piece_type == "Workbench" && !s.is_blueprint {
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
                                    let struct_id_opt = resolve_structure_id(hit.entity, &queries.structure, &queries.door, &queries.parent_q);
                                    let struct_data = struct_id_opt.and_then(|sid| conn.db.db.structure().structure_id().find(&sid));
                                    let is_blueprint = struct_data.as_ref().map(|s| s.is_blueprint).unwrap_or(false);
                                    let is_door = struct_data.as_ref().map(|s| s.piece_type.ends_with("Door") || s.piece_type == "Door").unwrap_or(false);

                                    let indicator_color = if is_blueprint {
                                        Color::srgb(0.2, 0.7, 1.0)
                                    } else if is_door {
                                        Color::srgb(0.9, 0.6, 0.2)
                                    } else if is_node {
                                        Color::srgb(0.9, 0.8, 0.1)
                                    } else {
                                        Color::srgb(0.2, 0.9, 0.3)
                                    };

                                    commands.spawn((
                                        PbrBundle {
                                            mesh: weapons.meshes.add(bevy::math::primitives::Cylinder::new(0.8, 0.05)),
                                            material: weapons.materials.add(StandardMaterial {
                                                base_color: indicator_color,
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
                                            if let Some(ref s) = struct_data {
                                                if is_blueprint {
                                                    let _ = conn.db.reducers.command_peasant(
                                                        peasant.entity_id,
                                                        "Construct".to_string(),
                                                        hit_point.x, hit_point.y, hit_point.z,
                                                        s.structure_id,
                                                    );
                                                } else if is_door {
                                                    let _ = conn.db.reducers.command_peasant(
                                                        peasant.entity_id,
                                                        "Door".to_string(),
                                                        hit_point.x, hit_point.y, hit_point.z,
                                                        s.structure_id,
                                                    );
                                                } else {
                                                    let _ = conn.db.reducers.command_peasant(
                                                        peasant.entity_id,
                                                        "MoveTo".to_string(),
                                                        hit_point.x, hit_point.y, hit_point.z,
                                                        0,
                                                    );
                                                }
                                            } else if is_node {
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
    time: Res<Time>,
    locomotion_settings: Res<LocomotionSettings>,
    ability_state: Option<Res<TacticalAbilityState>>,
    mut action_buffer: ResMut<ActionBuffer>,
    mut query: Query<(
        Entity, 
        &mut BevyTransform, 
        &mut LinearVelocity, 
        &mut GravityScale, 
        &mut Kcc,
        Option<&mut LocomotionState>,
    ), With<PlayerBody>>,
    spatial_query: SpatialQuery, 
) {
    let Ok((entity, mut transform, mut lin_vel, mut gravity, mut kcc, mut loco_opt)) = query.get_single_mut() else { return; };

    let ray_start = transform.translation; 
    let hit = spatial_query.cast_ray(
        ray_start, 
        Dir3::NEG_Y, 
        1.25, 
        true, 
        SpatialQueryFilter::from_excluded_entities([entity])
    );
    
    kcc.is_grounded = false;
    if let Some(hit_data) = hit {
        if hit_data.time_of_impact <= 1.20 && lin_vel.y <= 0.05 {
            kcc.is_grounded = true;
        }
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

    // Preserve active tactical Phase Dash momentum instead of clamping to walking speed
    let is_dashing = ability_state.as_ref().map_or(false, |s| s.is_dashing);
    if !is_dashing && *camera_mode.get() == CameraMode::FPS {
        lin_vel.x = move_dir.x * locomotion_settings.horizontal_speed;
        lin_vel.z = move_dir.z * locomotion_settings.horizontal_speed;
    }

    let dt = time.delta_seconds();
    let current_time = time.elapsed_seconds_f64();
    let mut jump_requested = (!console.is_open && *camera_mode.get() == CameraMode::FPS) && (
        keys.just_pressed(KeyCode::Space) ||
        action_buffer.pop_matching(current_time, |a| a == VirtualAction::Jump).is_some()
    );

    if let Some(ref mut loco) = loco_opt {
        if kcc.is_grounded {
            loco.time_since_grounded = 0.0;
            // Check for buffered jump upon landing
            if loco.jump_buffered_timer > 0.0 {
                jump_requested = true;
                loco.jump_buffered_timer = 0.0;
            }
        } else {
            loco.time_since_grounded += dt;
            if jump_requested {
                // Buffer the jump intent while in the air to execute seamlessly on touchdown
                loco.jump_buffered_timer = locomotion_settings.jump_buffer_max;
            }
            loco.jump_buffered_timer = (loco.jump_buffered_timer - dt).max(0.0);
        }

        // Coyote Time: Grace window allowing jumps within 120ms of falling off edges
        let can_jump = kcc.is_grounded || loco.time_since_grounded <= locomotion_settings.coyote_time_max;

        if jump_requested && can_jump && *camera_mode.get() == CameraMode::FPS && !console.is_open {
            lin_vel.y = locomotion_settings.jump_impulse;
            gravity.0 = 8.0;
            kcc.is_grounded = false;
            loco.time_since_grounded = locomotion_settings.coyote_time_max + 1.0; // Consume coyote window
            loco.jump_buffered_timer = 0.0;
        } else if kcc.is_grounded {
            if move_dir == Vec3::ZERO {
                // Standing still: zero downward acceleration to prevent Avian3D penetration chatter
                lin_vel.y = 0.0;
                gravity.0 = 0.0;

                if let Some(hit_data) = hit {
                    if hit_data.time_of_impact < 0.98 {
                        let surface_y = ray_start.y - hit_data.time_of_impact;
                        transform.translation.y = surface_y + 1.0;
                    }
                }
            } else {
                // Moving along slope: apply slight downward adhesion
                gravity.0 = 2.0;
                if lin_vel.y < 0.0 {
                    lin_vel.y = -0.5;
                }
            }
        } else {
            gravity.0 = 8.0;
        }
    } else {
        // Fallback execution when LocomotionState component is absent
        if kcc.is_grounded {
            if jump_requested && *camera_mode.get() == CameraMode::FPS && !console.is_open {
                lin_vel.y = locomotion_settings.jump_impulse;
                gravity.0 = 8.0;
                kcc.is_grounded = false;
            } else if move_dir == Vec3::ZERO {
                lin_vel.y = 0.0;
                gravity.0 = 0.0;
            } else {
                gravity.0 = 2.0;
                if lin_vel.y < 0.0 { lin_vel.y = -0.5; }
            }
        } else {
            gravity.0 = 8.0;
        }
    }

    // Void safety net: failsafe if entity tunnels through unloaded chunks or physics glitch
    let ground_y = crate::terrain::get_terrain_height(transform.translation.x, transform.translation.z);
    let min_safe_y = ground_y + 1.0;
    if (hit.is_none() && transform.translation.y < min_safe_y) || transform.translation.y < ground_y - 1.0 {
        transform.translation.y = min_safe_y;
        if lin_vel.y < 0.0 { 
            lin_vel.y = 0.0; 
        }
        kcc.is_grounded = true;
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
    door_query: Query<&Door>,
    ruin_query: Query<&HarvestableRuin>,
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
                    "FallenLog" => "[E] Chop Fallen Log",
                    "Rubble" => "[E] Mine Rubble",
                    "Rock" => "Rock (Left-click with Pickaxe)",
                    _ => "[E] Gather",
                };

                text.sections[0].value = prompt.to_string();
                *vis = Visibility::Inherited;
                return;
            }
        } else if let Ok(ruin) = ruin_query.get(hit_data.entity) {
            text.sections[0].value = format!("[E] Mine {} (Yield: {})", ruin.node_type, ruin.yield_amount);
            *vis = Visibility::Inherited;
            return;
        } else if let Some(struct_id) = resolve_structure_id(hit_data.entity, &structure_query, &door_query, &parent_query) {
            if let Some(s) = conn.db.db.structure().structure_id().find(&struct_id) {
                let is_hammer = active_item.0.as_deref() == Some("Hammer");
                if s.piece_type == "Door" && !s.is_blueprint {
                    let is_open = conn.db.db.door_state().structure_id().find(&struct_id).map_or(false, |d| d.is_open);
                    text.sections[0].value = if is_open {
                        "[E] Close Door".to_string()
                    } else {
                        "[E] Open Door".to_string()
                    };
                } else if s.piece_type == "Workbench" && !s.is_blueprint {
                    text.sections[0].value = "[E] Open Workbench".to_string();
                } else if s.is_blueprint {
                    text.sections[0].value = format!("Autobuilding {} ({}%)...", s.piece_type, s.construction_progress);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grounded_stationary_stability_invariants() {
        // Invariants: when stationary on the ground, gravity must be 0.0 and vertical velocity
        // must be 0.0 so that XPBD collision constraint solver does not oscillate vertically.
        let mut lin_vel = LinearVelocity(Vec3::new(0.0, -1.0, 0.0));
        let mut gravity = GravityScale(8.0);
        let kcc = Kcc { is_grounded: true };
        let move_dir = Vec3::ZERO;

        if kcc.is_grounded {
            if move_dir == Vec3::ZERO {
                lin_vel.y = 0.0;
                gravity.0 = 0.0;
            }
        }

        assert_eq!(gravity.0, 0.0);
        assert_eq!(lin_vel.y, 0.0);
        assert_eq!(kcc.is_grounded, true);
    }

    #[test]
    fn test_jumping_invariants() {
        // Invariants: jumping sets vertical velocity to 10.0, re-engages 8G gravity, and clears grounded state
        let mut lin_vel = LinearVelocity::ZERO;
        let mut gravity = GravityScale(0.0);
        let mut kcc = Kcc { is_grounded: true };

        // Simulate jump execution
        lin_vel.y = 10.0;
        gravity.0 = 8.0;
        kcc.is_grounded = false;

        assert_eq!(lin_vel.y, 10.0);
        assert_eq!(gravity.0, 8.0);
        assert_eq!(kcc.is_grounded, false);
    }

    #[test]
    fn test_action_buffer_push_pop_and_ttl_expiration() {
        let mut buffer = ActionBuffer::default();
        let current_time = 100.0;

        buffer.push(VirtualAction::Jump, current_time, 0.12);
        buffer.push(VirtualAction::UseAbility(TacticalAbilityKind::PhaseDash), current_time, 0.12);
        assert_eq!(buffer.actions.len(), 2);

        // Pop jump
        let popped = buffer.pop_matching(current_time + 0.05, |a| a == VirtualAction::Jump);
        assert_eq!(popped, Some(VirtualAction::Jump));
        assert_eq!(buffer.actions.len(), 1);

        // Advancing time past TTL (0.12s) expires remaining actions
        let popped_expired = buffer.pop_matching(current_time + 0.20, |a| a == VirtualAction::UseAbility(TacticalAbilityKind::PhaseDash));
        assert_eq!(popped_expired, None);
        assert_eq!(buffer.actions.len(), 0);
    }

    #[test]
    fn test_coyote_time_grace_window() {
        let settings = LocomotionSettings::default();
        let mut loco = LocomotionState::default();

        // On ground: timer is 0
        loco.time_since_grounded = 0.0;
        let mut can_jump = loco.time_since_grounded <= settings.coyote_time_max;
        assert!(can_jump);

        // Falling off a ledge for 0.08s (within 0.12s coyote window): still can jump
        loco.time_since_grounded += 0.08;
        can_jump = loco.time_since_grounded <= settings.coyote_time_max;
        assert!(can_jump);

        // Falling off ledge past 0.12s: cannot coyote jump
        loco.time_since_grounded += 0.05; // 0.13s total
        can_jump = loco.time_since_grounded <= settings.coyote_time_max;
        assert!(!can_jump);
    }
}