// ============================================================================
// File: client/src/weapons/viewmodels.rs
// ============================================================================
// ----------------------------------------------------------------------------
// VIEWMODEL SPAWNING, ANIMATION, RECOIL & THIRD-PERSON SYNCHRONIZATION
// ----------------------------------------------------------------------------

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::render::view::RenderLayers;
use tracing::info;
use spacetime_rts_logic::HandSide;

use crate::components::*;
use crate::core::*;
use super::types::*;
use super::mesh_builder::spawn_weapon_voxels;
use super::melee::compute_directional_melee_transform;

pub fn toggle_weapon_hand_system(
    mut hand_evts: EventReader<crate::input::ToggleWeaponHandEvent>,
    mut hand_side: ResMut<EquippedHandSide>,
    mut weapon_root_q: Query<&mut BevyTransform, With<ViewModelWeaponRoot>>,
) {
    for _ in hand_evts.read() {
        hand_side.0 = match hand_side.0 {
            HandSide::Right => HandSide::Left,
            HandSide::Left => HandSide::Right,
        };
        info!("Toggled Primary Hand: {:?}", hand_side.0);
        for mut t in weapon_root_q.iter_mut() {
            t.translation.x = -t.translation.x;
        }
    }
}

pub fn spawn_or_update_view_model_weapon(
    mut commands: Commands,
    active_item: Res<ActiveEquippedItem>,
    active_offhand: Res<ActiveOffHandItem>,
    mut weapon_state: ResMut<WeaponState>,
    hand_side: Res<EquippedHandSide>,
    camera_query: Query<Entity, With<FpsCamera>>,
    player_query: Query<Entity, With<PlayerBody>>,
    existing_weapon_q: Query<Entity, With<ViewModelWeaponRoot>>,
    existing_tp_q: Query<Entity, With<ThirdPersonWeaponRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let desired_main = WeaponType::from_item_name(active_item.0.as_deref());
    let desired_off = WeaponType::from_item_name(active_offhand.0.as_deref());

    let need_respawn = desired_main != weapon_state.current_weapon
        || desired_off != weapon_state.offhand_weapon
        || weapon_state.last_hand != hand_side.0
        || existing_weapon_q.is_empty();

    if !need_respawn {
        return;
    }

    // Despawn old weapon models (both 1st-person viewmodels and 3rd-person body attachments)
    for entity in existing_weapon_q.iter() {
        commands.entity(entity).despawn_recursive();
    }
    for entity in existing_tp_q.iter() {
        commands.entity(entity).despawn_recursive();
    }

    weapon_state.current_weapon = desired_main;
    weapon_state.offhand_weapon = desired_off;
    weapon_state.last_hand = hand_side.0;

    let Ok(camera_entity) = camera_query.get_single() else {
        return;
    };

    let is_left = hand_side.0 == HandSide::Left;
    let should_spawn_offhand = (desired_main == WeaponType::None && desired_off == WeaponType::None)
        || (desired_off != WeaponType::None && (desired_main.is_one_handed() || desired_main == WeaponType::None));

    commands.entity(camera_entity).with_children(|parent| {
        // 1. Main Hand Viewmodel Root (or Right Fist if Unarmed)
        let main_pos = get_default_weapon_pos(desired_main, is_left);
        let main_rot = match desired_main {
            WeaponType::Longsword | WeaponType::Greatsword | WeaponType::Rapier => {
                Quat::from_rotation_x(0.48) * Quat::from_rotation_y(-0.25) * Quat::from_rotation_z(0.18)
            }
            _ => if is_left {
                Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06)
            } else {
                Quat::IDENTITY
            },
        };

        parent.spawn((
            SpatialBundle {
                transform: BevyTransform::from_translation(main_pos).with_rotation(main_rot),
                ..default()
            },
            ViewModelWeaponRoot { is_offhand: false },
            RenderLayers::layer(1),
        )).with_children(|builder| {
            spawn_weapon_voxels(builder, desired_main, &mut meshes, &mut materials, 1);
        });

        // 2. Off-Hand Viewmodel Root
        // Only spawn off-hand if:
        // - player is fully unarmed (both hands unarmed -> brawler stance with both fists)
        // - OR off-hand has an explicitly equipped weapon while main hand is 1-handed or None
        if should_spawn_offhand {
            let off_is_left = !is_left;
            let off_pos = get_default_weapon_pos(desired_off, off_is_left);
            let off_rot = match desired_off {
                WeaponType::WoodenShield => {
                    if off_is_left {
                        Quat::from_rotation_y(0.26) * Quat::from_rotation_x(0.16) * Quat::from_rotation_z(-0.08)
                    } else {
                        Quat::from_rotation_y(-0.26) * Quat::from_rotation_x(0.16) * Quat::from_rotation_z(0.08)
                    }
                }
                _ => if off_is_left {
                    Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06)
                } else {
                    Quat::from_rotation_y(-0.08) * Quat::from_rotation_z(-0.06)
                },
            };

            parent.spawn((
                SpatialBundle {
                    transform: BevyTransform::from_translation(off_pos).with_rotation(off_rot),
                    ..default()
                },
                ViewModelWeaponRoot { is_offhand: true },
                RenderLayers::layer(1),
            )).with_children(|builder| {
                spawn_weapon_voxels(builder, desired_off, &mut meshes, &mut materials, 1);
            });
        }
    });

    // 2. Spawn Third-Person Weapon Models on Player Body (RenderLayers::layer(2))
    if let Ok(player_entity) = player_query.get_single() {
        commands.entity(player_entity).with_children(|body| {
            // Main hand on player character (peasant right hand)
            let tp_main_pos = if is_left {
                Vec3::new(-0.28, -0.55, 0.08)
            } else {
                Vec3::new(0.28, -0.55, 0.08)
            };
            let tp_main_rot = match desired_main {
                WeaponType::Longsword | WeaponType::Greatsword | WeaponType::Rapier => {
                    Quat::from_rotation_x(-0.75)
                        * Quat::from_rotation_y(if is_left { -0.15 } else { 0.15 })
                        * Quat::from_rotation_z(if is_left { 0.25 } else { -0.25 })
                }
                _ => Quat::from_rotation_x(-0.4),
            };

            body.spawn((
                SpatialBundle {
                    transform: BevyTransform::from_translation(tp_main_pos).with_rotation(tp_main_rot),
                    ..default()
                },
                ThirdPersonWeaponRoot { is_offhand: false },
                RenderLayers::layer(2),
            )).with_children(|builder| {
                spawn_weapon_voxels(builder, desired_main, &mut meshes, &mut materials, 2);
            });

            // Off hand on player character (peasant left hand / shield)
            if should_spawn_offhand {
                let off_is_left = !is_left;
                let tp_off_pos = if off_is_left {
                    Vec3::new(-0.30, -0.52, 0.06)
                } else {
                    Vec3::new(0.30, -0.52, 0.06)
                };
                let tp_off_rot = match desired_off {
                    WeaponType::WoodenShield => {
                        if off_is_left {
                            Quat::from_rotation_y(1.57) * Quat::from_rotation_x(0.12)
                        } else {
                            Quat::from_rotation_y(-1.57) * Quat::from_rotation_x(0.12)
                        }
                    }
                    _ => Quat::IDENTITY,
                };

                body.spawn((
                    SpatialBundle {
                        transform: BevyTransform::from_translation(tp_off_pos).with_rotation(tp_off_rot),
                        ..default()
                    },
                    ThirdPersonWeaponRoot { is_offhand: true },
                    RenderLayers::layer(2),
                )).with_children(|builder| {
                    spawn_weapon_voxels(builder, desired_off, &mut meshes, &mut materials, 2);
                });
            }
        });
    }
}

pub fn animate_weapon_viewmodel(
    time: Res<Time>,
    mut weapon_state: ResMut<WeaponState>,
    mut swing_state: ResMut<SwingState>,
    hand_side: Res<EquippedHandSide>,
    mut root_q: Query<(&ViewModelWeaponRoot, &mut BevyTransform), (Without<ViewModelBowArrow>, Without<ViewModelPumpSlide>)>,
    mut arrow_q: Query<(&mut Visibility, &mut BevyTransform), (With<ViewModelBowArrow>, Without<ViewModelWeaponRoot>, Without<ViewModelPumpSlide>, Without<ViewModelCrossbowBolt>)>,
    mut bolt_q: Query<&mut Visibility, (With<ViewModelCrossbowBolt>, Without<ViewModelBowArrow>)>,
) {
    let dt = time.delta_seconds();
    weapon_state.sway_time += dt;

    // Melee swing state machine & timers ticking
    match swing_state.phase {
        MeleeAttackPhase::Idle => {
            if swing_state.is_swinging {
                swing_state.timer.tick(time.delta());
                if swing_state.timer.just_finished() {
                    swing_state.is_swinging = false;
                }
            }
        }
        MeleeAttackPhase::Windup => {
            swing_state.windup_timer.tick(time.delta());
            if swing_state.windup_timer.just_finished() {
                swing_state.phase = MeleeAttackPhase::Release;
                swing_state.release_timer.reset();
            }
        }
        MeleeAttackPhase::Release => {
            swing_state.release_timer.tick(time.delta());
            if swing_state.release_timer.just_finished() {
                swing_state.phase = MeleeAttackPhase::Recovery;
                swing_state.recovery_timer.reset();
            }
        }
        MeleeAttackPhase::Recovery => {
            swing_state.recovery_timer.tick(time.delta());
            if swing_state.recovery_timer.just_finished() {
                swing_state.phase = MeleeAttackPhase::Idle;
                swing_state.is_swinging = false;
            }
        }
    }

    if swing_state.offhand_is_swinging {
        swing_state.offhand_timer.tick(time.delta());
        if swing_state.offhand_timer.just_finished() {
            swing_state.offhand_is_swinging = false;
        }
    }

    // 1. Ticking Reload & Action Timers
    if !weapon_state.crossbow_loaded {
        weapon_state.crossbow_reload_timer.tick(time.delta());
        if weapon_state.crossbow_reload_timer.just_finished() {
            weapon_state.crossbow_loaded = true;
            weapon_state.crossbow_reload_timer.reset();
        }
    }

    if !weapon_state.hand_crossbow_loaded {
        weapon_state.hand_crossbow_reload_timer.tick(time.delta());
        if weapon_state.hand_crossbow_reload_timer.just_finished() {
            weapon_state.hand_crossbow_loaded = true;
            weapon_state.hand_crossbow_reload_timer.reset();
        }
    }

    // 2. Bolt/Arrow Visibility & Dynamic Arrow Pullback
    for mut vis in bolt_q.iter_mut() {
        let is_visible = match weapon_state.current_weapon {
            WeaponType::Crossbow => weapon_state.crossbow_loaded,
            WeaponType::HandCrossbow => weapon_state.hand_crossbow_loaded,
            _ => true,
        };
        *vis = if is_visible { Visibility::Inherited } else { Visibility::Hidden };
    }

    if !weapon_state.bow_nock_timer.finished() {
        weapon_state.bow_nock_timer.tick(time.delta());
    }

    for (mut vis, mut arrow_t) in arrow_q.iter_mut() {
        if weapon_state.current_weapon == WeaponType::Bow {
            // Hide nocked arrow immediately upon firing until nock timer finishes
            *vis = if weapon_state.bow_nock_timer.finished() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };

            // Dynamic arrow pullback along bow shelf:
            let draw_offset = weapon_state.bow_charge * 0.20;
            arrow_t.translation = Vec3::new(-0.01, 0.02, -0.12 + draw_offset);
        } else {
            *vis = Visibility::Inherited;
        }
    }

    // 3. Recoil Recovery (Exponential Decay) & Dynamic Bloom Decay
    weapon_state.recoil_offset = weapon_state.recoil_offset.lerp(Vec3::ZERO, (dt * 14.0).min(1.0));
    weapon_state.recoil_rot = weapon_state.recoil_rot.slerp(Quat::IDENTITY, (dt * 16.0).min(1.0));
    weapon_state.offhand_recoil_offset = weapon_state.offhand_recoil_offset.lerp(Vec3::ZERO, (dt * 14.0).min(1.0));
    weapon_state.offhand_recoil_rot = weapon_state.offhand_recoil_rot.slerp(Quat::IDENTITY, (dt * 16.0).min(1.0));
    weapon_state.dynamic_bloom = (weapon_state.dynamic_bloom - dt * 28.0).max(0.0);
    if weapon_state.current_weapon == WeaponType::Bow && weapon_state.bow_drawing {
        weapon_state.dynamic_bloom = (1.0 - weapon_state.bow_charge) * 12.0;
    }

    // 4. Transform Animation on ViewModelWeaponRoot
    for (root, mut root_t) in root_q.iter_mut() {
        if !root.is_offhand {
            let is_left = hand_side.0 == HandSide::Left;
            let default_pos = get_default_weapon_pos(weapon_state.current_weapon, is_left);

            // Idle Breathing Sway
            let sway_x = (weapon_state.sway_time * 1.5).sin() * 0.003;
            let sway_y = (weapon_state.sway_time * 3.0).cos() * 0.002;
            let mut current_offset = default_pos + Vec3::new(sway_x, sway_y, 0.0) + weapon_state.recoil_offset;

            // Idle ready guard posture for melee weapons
            let idle_base_rot = match weapon_state.current_weapon {
                WeaponType::Longsword | WeaponType::Greatsword | WeaponType::Rapier => {
                    Quat::from_rotation_x(0.48) * Quat::from_rotation_y(-0.25) * Quat::from_rotation_z(0.18)
                }
                WeaponType::Warhammer | WeaponType::Handaxe | WeaponType::Club => {
                    Quat::from_rotation_x(0.38) * Quat::from_rotation_y(-0.20)
                }
                WeaponType::Spear | WeaponType::Halberd => {
                    Quat::from_rotation_x(0.25) * Quat::from_rotation_y(-0.15)
                }
                _ => Quat::IDENTITY,
            };

            let mut current_rot = idle_base_rot * weapon_state.recoil_rot;
            if is_left {
                current_rot = Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06) * current_rot;
            }

            // Directional Melee Swing Animation (Mount & Blade / cRPG Style)
            if weapon_state.current_weapon.is_melee() && swing_state.phase != MeleeAttackPhase::Idle {
                let windup_t = swing_state.windup_timer.fraction();
                let release_t = swing_state.release_timer.fraction();
                let recovery_t = swing_state.recovery_timer.fraction();

                let (melee_offset, melee_rot) = compute_directional_melee_transform(
                    swing_state.direction,
                    swing_state.phase,
                    windup_t,
                    release_t,
                    recovery_t,
                );
                current_offset += melee_offset;
                current_rot = melee_rot * current_rot;
            } else if swing_state.is_swinging {
                // Main hand punch / fallback swing lunge animation
                let t = swing_state.timer.fraction();
                let punch_forward = (t * std::f32::consts::PI).sin() * 0.16;
                current_offset.z -= punch_forward;
                current_offset.y += punch_forward * 0.15;
                current_rot *= Quat::from_rotation_x(punch_forward * 0.7);
            }

            // Weapon Parry Guard (When blocking with melee weapon and no off-hand shield)
            if swing_state.is_blocking && weapon_state.offhand_weapon == WeaponType::None && weapon_state.current_weapon.is_melee() {
                let parry_pos = Vec3::new(-0.04, 0.04, -0.24);
                let parry_rot = Quat::from_rotation_z(-0.85) * Quat::from_rotation_x(0.35) * Quat::from_rotation_y(0.22);
                current_offset = current_offset.lerp(parry_pos, (dt * 18.0).min(1.0));
                current_rot = current_rot.slerp(parry_rot, (dt * 18.0).min(1.0));
            }

            // Bow ADS & Draw Stance
            if weapon_state.current_weapon == WeaponType::Bow && weapon_state.bow_drawing {
                let charge = weapon_state.bow_charge;
                current_offset += Vec3::new(-0.11 * charge, 0.06 * charge, 0.14 * charge);
                current_rot *= Quat::from_rotation_z(0.38 * charge) * Quat::from_rotation_x(0.14 * charge);
            }

            // Crossbow Cranking Stance
            if !weapon_state.crossbow_loaded && weapon_state.current_weapon == WeaponType::Crossbow {
                let t = weapon_state.crossbow_reload_timer.fraction();
                let dip = (t * std::f32::consts::PI).sin() * 0.06;
                current_offset.y -= dip;
                current_rot *= Quat::from_rotation_x(0.25 * dip);
            }

            root_t.translation = current_offset;
            root_t.rotation = current_rot;
        } else {
            let off_is_left = hand_side.0 != HandSide::Left;
            let default_pos = get_default_weapon_pos(weapon_state.offhand_weapon, off_is_left);

            // Counter Sway
            let sway_x = ((weapon_state.sway_time + 1.2) * 1.5).sin() * 0.003;
            let sway_y = ((weapon_state.sway_time + 1.2) * 3.0).cos() * 0.002;
            let mut current_offset = default_pos + Vec3::new(sway_x, sway_y, 0.0) + weapon_state.offhand_recoil_offset;

            let idle_offhand_rot = match weapon_state.offhand_weapon {
                WeaponType::WoodenShield => {
                    if off_is_left {
                        Quat::from_rotation_y(0.26) * Quat::from_rotation_x(0.16) * Quat::from_rotation_z(-0.08)
                    } else {
                        Quat::from_rotation_y(-0.26) * Quat::from_rotation_x(0.16) * Quat::from_rotation_z(0.08)
                    }
                }
                _ => Quat::IDENTITY,
            };

            let mut current_rot = idle_offhand_rot * weapon_state.offhand_recoil_rot;
            if off_is_left {
                current_rot = Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06) * current_rot;
            } else {
                current_rot = Quat::from_rotation_y(-0.08) * Quat::from_rotation_z(-0.06) * current_rot;
            }

            // Shield Block Guard
            if swing_state.is_blocking && weapon_state.offhand_weapon == WeaponType::WoodenShield {
                let block_pos = Vec3::new(-0.06, -0.09, -0.24);
                let block_rot = Quat::from_rotation_y(-0.14) * Quat::from_rotation_x(0.14) * Quat::from_rotation_z(0.05);
                current_offset = current_offset.lerp(block_pos, (dt * 18.0).min(1.0));
                current_rot = current_rot.slerp(block_rot, (dt * 18.0).min(1.0));
            } else if swing_state.offhand_is_swinging {
                // Off-hand punch / swing lunge animation
                let t = swing_state.offhand_timer.fraction();
                let punch_forward = (t * std::f32::consts::PI).sin() * 0.16;
                current_offset.z -= punch_forward;
                current_offset.y += punch_forward * 0.15;
                current_rot *= Quat::from_rotation_x(punch_forward * 0.7);
            }

            root_t.translation = current_offset;
            root_t.rotation = current_rot;
        }
    }
}

/// Animates 3rd-person held weapon models (swords and shields) attached to the character body.
pub fn animate_third_person_weapons(
    time: Res<Time>,
    weapon_state: Res<WeaponState>,
    swing_state: Res<SwingState>,
    hand_side: Res<EquippedHandSide>,
    mut tp_root_q: Query<(&ThirdPersonWeaponRoot, &mut BevyTransform)>,
) {
    let dt = time.delta_seconds();
    let is_left = hand_side.0 == HandSide::Left;
    for (tp_root, mut tp_t) in tp_root_q.iter_mut() {
        if !tp_root.is_offhand {
            // Main-hand weapon (sword/mace/axe)
            let base_pos = if is_left { Vec3::new(-0.28, -0.55, 0.08) } else { Vec3::new(0.28, -0.55, 0.08) };
            let base_rot = match weapon_state.current_weapon {
                WeaponType::Longsword | WeaponType::Greatsword | WeaponType::Rapier => {
                    Quat::from_rotation_x(-0.75)
                        * Quat::from_rotation_y(if is_left { -0.15 } else { 0.15 })
                        * Quat::from_rotation_z(if is_left { 0.25 } else { -0.25 })
                }
                _ => Quat::from_rotation_x(-0.4),
            };

            if swing_state.is_swinging {
                let t = swing_state.timer.fraction();
                let swing_arc = (t * std::f32::consts::PI).sin() * 1.5;
                tp_t.translation = base_pos + Vec3::new(0.0, 0.08 * swing_arc, 0.18 * swing_arc);
                tp_t.rotation = base_rot * Quat::from_rotation_x(swing_arc);
            } else {
                tp_t.translation = tp_t.translation.lerp(base_pos, (dt * 12.0).min(1.0));
                tp_t.rotation = tp_t.rotation.slerp(base_rot, (dt * 12.0).min(1.0));
            }
        } else {
            // Off-hand weapon (shield)
            let off_is_left = !is_left;
            let idle_pos = if off_is_left { Vec3::new(-0.30, -0.52, 0.06) } else { Vec3::new(0.30, -0.52, 0.06) };
            let idle_rot = match weapon_state.offhand_weapon {
                WeaponType::WoodenShield => {
                    if off_is_left {
                        Quat::from_rotation_y(1.57) * Quat::from_rotation_x(0.12)
                    } else {
                        Quat::from_rotation_y(-1.57) * Quat::from_rotation_x(0.12)
                    }
                }
                _ => Quat::IDENTITY,
            };

            if swing_state.is_blocking && weapon_state.offhand_weapon == WeaponType::WoodenShield {
                // Raise shield in front of character's chest in 3rd person
                let block_pos = Vec3::new(-0.08, -0.42, 0.26);
                let block_rot = Quat::from_rotation_y(0.08) * Quat::from_rotation_x(0.1);
                tp_t.translation = tp_t.translation.lerp(block_pos, (dt * 18.0).min(1.0));
                tp_t.rotation = tp_t.rotation.slerp(block_rot, (dt * 18.0).min(1.0));
            } else if swing_state.offhand_is_swinging {
                let t = swing_state.offhand_timer.fraction();
                let punch_arc = (t * std::f32::consts::PI).sin() * 0.22;
                let punch_pos = idle_pos + Vec3::new(0.0, 0.05, punch_arc);
                tp_t.translation = tp_t.translation.lerp(punch_pos, (dt * 20.0).min(1.0));
                tp_t.rotation = tp_t.rotation.slerp(idle_rot * Quat::from_rotation_x(punch_arc), (dt * 20.0).min(1.0));
            } else {
                tp_t.translation = tp_t.translation.lerp(idle_pos, (dt * 12.0).min(1.0));
                tp_t.rotation = tp_t.rotation.slerp(idle_rot, (dt * 12.0).min(1.0));
            }
        }
    }
}

/// Traverses all descendants of `ThirdPersonWeaponRoot` and ensures their `RenderLayers`
/// is set to Layer 2 so they render in 3rd person / RTS view and never interfere with
/// 1st person viewmodels on Layer 1.
pub fn sync_third_person_weapon_render_layers(
    tp_roots: Query<&Children, With<ThirdPersonWeaponRoot>>,
    children_q: Query<&Children>,
    mut layers_q: Query<&mut RenderLayers>,
    mut commands: Commands,
) {
    for children in tp_roots.iter() {
        let mut stack: Vec<Entity> = children.iter().copied().collect();
        while let Some(entity) = stack.pop() {
            if let Ok(mut layers) = layers_q.get_mut(entity) {
                if *layers != RenderLayers::layer(2) {
                    *layers = RenderLayers::layer(2);
                }
            } else {
                commands.entity(entity).insert(RenderLayers::layer(2));
            }
            if let Ok(sub_children) = children_q.get(entity) {
                stack.extend(sub_children.iter().copied());
            }
        }
    }
}

pub fn weapon_reload_input_system(
    mut reload_evts: EventReader<crate::input::WeaponReloadEvent>,
    mut weapon_state: ResMut<WeaponState>,
) {
    for _ in reload_evts.read() {
        match weapon_state.current_weapon {
            WeaponType::Crossbow => {
                if !weapon_state.crossbow_loaded {
                    weapon_state.crossbow_reload_timer.reset();
                }
            }
            WeaponType::HandCrossbow => {
                if !weapon_state.hand_crossbow_loaded {
                    weapon_state.hand_crossbow_reload_timer.reset();
                }
            }
            _ => {}
        }
    }
}

pub fn update_weapon_hud(
    weapon_state: Res<WeaponState>,
    hand_side: Res<EquippedHandSide>,
    camera_mode: Res<State<CameraMode>>,
    mut text_q: Query<(&mut Visibility, &mut Text), With<WeaponHudText>>,
) {
    let Ok((mut vis, mut text)) = text_q.get_single_mut() else { return; };
    if *camera_mode.get() != CameraMode::FPS {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    }

    if *vis != Visibility::Inherited {
        *vis = Visibility::Inherited;
    }

    // Only recompute and re-format strings when weapon state, equipped hand, or camera mode changes
    if !weapon_state.is_changed() && !hand_side.is_changed() && !camera_mode.is_changed() && !text.sections[0].value.is_empty() {
        return;
    }

    let hand_str = match hand_side.0 {
        HandSide::Right => "PRIMARY: RIGHT [H to Swap]",
        HandSide::Left => "PRIMARY: LEFT [H to Swap]",
    };

    let new_val = if weapon_state.current_weapon == WeaponType::None && weapon_state.offhand_weapon == WeaponType::None {
        format!("{}\nUNARMED [BARE FISTS]\n[LMB] Right Jab | [RMB] Left Jab", hand_str)
    } else {
        let main_desc = format!("MAIN: {} [LMB]", weapon_state.current_weapon.display_name());
        let off_desc = if weapon_state.current_weapon.is_one_handed() || weapon_state.current_weapon == WeaponType::None {
            if weapon_state.offhand_weapon == WeaponType::None {
                "OFF: Bare Fist [RMB]".to_string()
            } else {
                format!("OFF: {} [RMB]", weapon_state.offhand_weapon.display_name())
            }
        } else {
            "TWO-HANDED [LMB Attack | Hold]".to_string()
        };
        format!("{}\n{}\n{}", hand_str, main_desc, off_desc)
    };

    if text.sections[0].value != new_val {
        text.sections[0].value = new_val;
    }
}
