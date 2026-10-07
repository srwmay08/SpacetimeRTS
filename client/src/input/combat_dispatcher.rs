// ============================================================================
// File: client/src/input/combat_dispatcher.rs
// ============================================================================
// ----------------------------------------------------------------------------
// CONTEXT-AWARE COMBAT & ACTION DISPATCHER
// ----------------------------------------------------------------------------

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::{Transform as BevyTransform, *};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use tracing::{error, info};

use crate::building::BuildModeState;
use crate::components::*;
use crate::core::*;
use crate::network::SpacetimeConnection;
use crate::prediction::ClientTick;
use crate::weapons::*;
use crate::weapons::EquippedHandSide;
use spacetime_rts_logic::HandSide;

use crate::module_bindings::command_peasant_reducer::command_peasant;
use crate::module_bindings::contribute_construction_reducer::contribute_construction;
use crate::module_bindings::fire_bow_reducer::fire_bow;
use crate::module_bindings::fire_weapon_reducer::fire_weapon;
use crate::module_bindings::interact_node_reducer::interact_node;
use crate::module_bindings::repair_structure_reducer::repair_structure;
use crate::module_bindings::spawn_peasant_reducer::spawn_peasant;
use crate::module_bindings::swing_tool_reducer::swing_tool;
use crate::module_bindings::toggle_door_reducer::toggle_door;
use crate::module_bindings::player_table::PlayerTableAccess;
use crate::module_bindings::structure_table::StructureTableAccess;

use super::action_buffer::{ActionEvent, ActionState, VirtualAction};
use super::world_interaction::{resolve_node_id, resolve_structure_id};

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
    pub dummy: Query<'w, 's, &'static mut TrainingDummy>,
    pub goblin: Query<'w, 's, (Entity, &'static mut SparringGoblin, &'static mut LinearVelocity), Without<PlayerBody>>,
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
    pub flick_tracker: Res<'w, MouseFlickTracker>,
    pub audio_handles: Option<Res<'w, CombatAudioHandles>>,
}


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
    mut queries: ActionContextQueries,
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

                if trigger_offhand {
                    match event.state {
                        ActionState::JustPressed => {
                            // 1. Feint Mechanic (Mount & Blade / cRPG):
                            // Tapping off-hand (RMB) during melee windup feints/cancels the attack immediately into guard/idle
                            if swing_state.phase == MeleeAttackPhase::Windup {
                                swing_state.phase = MeleeAttackPhase::Idle;
                                swing_state.is_swinging = false;
                                swing_state.windup_timer.reset();
                                if weapons.weapon_state.offhand_weapon == WeaponType::WoodenShield
                                    || (weapons.weapon_state.current_weapon.is_melee() && weapons.weapon_state.offhand_weapon == WeaponType::None) {
                                    swing_state.is_blocking = true;
                                }
                                continue;
                            }

                            // 2. Shield / Weapon Guard Start
                            if weapons.weapon_state.offhand_weapon == WeaponType::WoodenShield
                                || (weapons.weapon_state.current_weapon.is_melee() && weapons.weapon_state.offhand_weapon == WeaponType::None) {
                                swing_state.is_blocking = true;
                            }

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
                            } else if !event.is_over_ui && !build_state.is_active && !swing_state.is_blocking {
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

                                            crate::weapons::spawn_directional_slash_trail(
                                                &mut commands,
                                                &mut weapons.meshes,
                                                &mut weapons.materials,
                                                origin,
                                                *cam_transform.forward(),
                                                *cam_transform.right(),
                                                *cam_transform.up(),
                                                MeleeSwingDirection::Left,
                                            );

                                            let hit = spatial_query.cast_ray(
                                                origin, cam_transform.forward(), 4.5, true,
                                                SpatialQueryFilter::from_excluded_entities([player_entity]),
                                            );

                                            let _ = conn.db.reducers.swing_tool(
                                                origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                            );

                                            if let Some(hit_data) = hit {
                                                let hit_pt = origin + dir * hit_data.time_of_impact;
                                                let target_entity = if queries.dummy.contains(hit_data.entity) || queries.goblin.contains(hit_data.entity) {
                                                    hit_data.entity
                                                } else if let Ok(parent) = queries.parent_q.get(hit_data.entity) {
                                                    parent.get()
                                                } else {
                                                    hit_data.entity
                                                };

                                                if let Ok(mut dummy) = queries.dummy.get_mut(target_entity) {
                                                    dummy.wobble_timer.reset();
                                                    dummy.wobble_angle = 0.22;
                                                    let hit_dir = dir.normalize();
                                                    let wobble_axis = hit_dir.cross(Vec3::Y).normalize_or_zero();
                                                    dummy.wobble_axis = if wobble_axis.length_squared() > 0.01 { wobble_axis } else { Vec3::X };
                                                    if let Some(ref audio) = weapons.audio_handles {
                                                        crate::audio_feedback::play_sound(&mut commands, &audio.flesh_impact);
                                                    }
                                                    crate::tuner::spawn_comic_damage_floater(&mut commands, hit_pt + Vec3::Y * 0.3, "JAB -16", false);
                                                } else if let Ok((_, mut goblin, mut goblin_vel)) = queries.goblin.get_mut(target_entity) {
                                                    if goblin.is_blocking {
                                                        if let Some(ref audio) = weapons.audio_handles {
                                                            crate::audio_feedback::play_sound(&mut commands, &audio.shield_block);
                                                        }
                                                        crate::tuner::spawn_comic_damage_floater(&mut commands, hit_pt + Vec3::Y * 0.3, "BLOCKED!", false);
                                                    } else {
                                                        goblin.health = (goblin.health - 16.0).max(10.0);
                                                        goblin.stagger_timer = Timer::from_seconds(0.4, TimerMode::Once);
                                                        goblin_vel.0 = dir * 2.8 + Vec3::Y * 1.0;
                                                        if let Some(ref audio) = weapons.audio_handles {
                                                            crate::audio_feedback::play_sound(&mut commands, &audio.flesh_impact);
                                                        }
                                                        crate::tuner::spawn_comic_damage_floater(&mut commands, hit_pt + Vec3::Y * 0.3, "PUNCH -16", false);
                                                    }
                                                } else {
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
                }
                ActionState::Pressed => {
                        if (weapons.weapon_state.offhand_weapon == WeaponType::WoodenShield
                            || (weapons.weapon_state.current_weapon.is_melee() && weapons.weapon_state.offhand_weapon == WeaponType::None))
                            && !event.is_over_ui && !build_state.is_active {
                            swing_state.is_blocking = true;
                        }
                    }
                    ActionState::JustReleased => {
                        if swing_state.is_blocking {
                            swing_state.is_blocking = false;
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
                                            weapons.weapon_state.bow_nock_timer.reset();

                                            // Draw charge scales arrow velocity: 22.0m/s (quick release) to 55.0m/s (full draw)
                                            let arrow_speed = 22.0 + (charge * 33.0);
                                            let arrow_mesh = weapons.meshes.add(crate::weapons::create_lowpoly_arrow_mesh());
                                            let arrow_mat = weapons.materials.add(StandardMaterial {
                                                base_color: Color::WHITE,
                                                perceptual_roughness: 0.65,
                                                metallic: 0.35,
                                                cull_mode: None,
                                                ..default()
                                            });

                                            let arrow_transform = BevyTransform::from_translation(origin + dir * 0.9)
                                                .looking_to(dir, Vec3::Y);

                                            commands.spawn((
                                                PbrBundle {
                                                    mesh: arrow_mesh,
                                                    material: arrow_mat,
                                                    transform: arrow_transform,
                                                    ..default()
                                                },
                                                RigidBody::Dynamic,
                                                LinearVelocity(dir * arrow_speed),
                                                GravityScale(0.55),
                                                Particle { timer: Timer::from_seconds(4.0, TimerMode::Once) },
                                                ArrowProjectile,
                                            ));

                                            // Physical release recoil kick
                                            weapons.weapon_state.recoil_offset += Vec3::new(0.012 * charge, 0.035 * charge, -0.065 * charge);
                                            weapons.weapon_state.recoil_rot *= Quat::from_rotation_x(-0.18 * charge) * Quat::from_rotation_z(-0.10 * charge);
                                            weapons.weapon_state.dynamic_bloom += 8.0 * charge;

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
                                let can_attack = swing_state.phase == MeleeAttackPhase::Idle
                                    || swing_state.phase == MeleeAttackPhase::Recovery
                                    || !swing_state.is_swinging;

                                if can_attack {
                                    if swing_state.is_blocking {
                                        swing_state.is_blocking = false;
                                    }

                                    let direction = weapons.flick_tracker.classify();
                                    swing_state.direction = direction;
                                    swing_state.phase = MeleeAttackPhase::Windup;
                                    swing_state.windup_timer.reset();
                                    swing_state.release_timer.reset();
                                    swing_state.recovery_timer.reset();
                                    swing_state.is_swinging = true;
                                    swing_state.timer.reset();

                                    let hit = spatial_query.cast_ray(
                                        origin, cam_transform.forward(), 50.0, true,
                                        SpatialQueryFilter::from_excluded_entities([player_entity]),
                                    );

                                    let is_melee = weapons.weapon_state.current_weapon.is_melee();

                                    if is_melee {
                                        crate::weapons::spawn_directional_slash_trail(
                                            &mut commands,
                                            &mut weapons.meshes,
                                            &mut weapons.materials,
                                            origin,
                                            *cam_transform.forward(),
                                            *cam_transform.right(),
                                            *cam_transform.up(),
                                            direction,
                                        );

                                        // Authoritative melee swing: bare fists (unarmed), tools, blades, bludgeons
                                        let _ = conn.db.reducers.swing_tool(
                                            origin.x, origin.y, origin.z, dir.x, dir.y, dir.z
                                        );

                                        // Dynamic bloom feedback on swing/punch
                                        weapons.weapon_state.dynamic_bloom = (weapons.weapon_state.dynamic_bloom + 2.5).min(20.0);

                                        // Immediate visual & sensory feedback if hitting a surface/entity within melee reach (4.5m)
                                        if let Some(hit_data) = hit.filter(|h| h.time_of_impact <= 4.5) {
                                            let hit_pt = origin + dir * hit_data.time_of_impact;
                                            let target_entity = if queries.dummy.contains(hit_data.entity) || queries.goblin.contains(hit_data.entity) {
                                                hit_data.entity
                                            } else if let Ok(parent) = queries.parent_q.get(hit_data.entity) {
                                                parent.get()
                                            } else {
                                                hit_data.entity
                                            };

                                            if let Ok(mut dummy) = queries.dummy.get_mut(target_entity) {
                                                dummy.wobble_timer.reset();
                                                dummy.wobble_angle = 0.38;
                                                let hit_dir = dir.normalize();
                                                let wobble_axis = hit_dir.cross(Vec3::Y).normalize_or_zero();
                                                dummy.wobble_axis = if wobble_axis.length_squared() > 0.01 { wobble_axis } else { Vec3::X };

                                                if let Some(ref audio) = weapons.audio_handles {
                                                    crate::audio_feedback::play_sound(&mut commands, &audio.sword_clang);
                                                }

                                                let (label, is_crit) = match swing_state.direction {
                                                    MeleeSwingDirection::Overhead => ("CLEAVE -52!", true),
                                                    MeleeSwingDirection::Thrust => ("THRUST -44", false),
                                                    MeleeSwingDirection::Left => ("SLASH -38", false),
                                                    MeleeSwingDirection::Right => ("SLASH -38", false),
                                                };
                                                crate::tuner::spawn_comic_damage_floater(&mut commands, hit_pt + Vec3::Y * 0.4, label, is_crit);

                                                // Burst of brilliant combat sparks
                                                let spark_mesh = weapons.meshes.add(bevy::math::primitives::Sphere::new(0.04));
                                                let spark_mat = weapons.materials.add(StandardMaterial {
                                                    base_color: Color::srgb(1.0, 0.85, 0.25),
                                                    emissive: Color::srgb(2.5, 1.8, 0.5).into(),
                                                    unlit: true,
                                                    ..default()
                                                });
                                                for i in 0..7 {
                                                    let angle = (i as f32) * (std::f32::consts::TAU / 7.0);
                                                    let spark_vel = Vec3::new(angle.cos() * 4.2, 2.5 + (i as f32 * 0.25), angle.sin() * 4.2);
                                                    commands.spawn((
                                                        PbrBundle {
                                                            mesh: spark_mesh.clone(),
                                                            material: spark_mat.clone(),
                                                            transform: BevyTransform::from_translation(hit_pt),
                                                            ..default()
                                                        },
                                                        RigidBody::Dynamic,
                                                        LinearVelocity(spark_vel),
                                                        Particle { timer: Timer::from_seconds(0.2, TimerMode::Once) },
                                                    ));
                                                }
                                            } else if let Ok((_, mut goblin, mut goblin_vel)) = queries.goblin.get_mut(target_entity) {
                                                if goblin.is_blocking {
                                                    if let Some(ref audio) = weapons.audio_handles {
                                                        crate::audio_feedback::play_sound(&mut commands, &audio.shield_block);
                                                    }
                                                    crate::tuner::spawn_comic_damage_floater(&mut commands, hit_pt + Vec3::Y * 0.3, "BLOCKED! CLANG", false);
                                                } else {
                                                    goblin.health = (goblin.health - 38.0).max(10.0);
                                                    goblin.stagger_timer = Timer::from_seconds(0.65, TimerMode::Once);
                                                    goblin_vel.0 = dir * 4.2 + Vec3::Y * 1.5;

                                                    if let Some(ref audio) = weapons.audio_handles {
                                                        crate::audio_feedback::play_sound(&mut commands, &audio.flesh_impact);
                                                    }

                                                    let (label, is_crit) = match swing_state.direction {
                                                        MeleeSwingDirection::Overhead => ("CLEAVE -58!", true),
                                                        MeleeSwingDirection::Thrust => ("THRUST -42", false),
                                                        MeleeSwingDirection::Left => ("SLASH -36", false),
                                                        MeleeSwingDirection::Right => ("SLASH -36", false),
                                                    };
                                                    crate::tuner::spawn_comic_damage_floater(&mut commands, hit_pt + Vec3::Y * 0.3, label, is_crit);
                                                }
                                            } else {
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

