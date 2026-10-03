// ============================================================================
// File: client/src/tactical_abilities.rs
// ============================================================================
// ----------------------------------------------------------------------------
// TACTICAL ABILITIES: UTILITY-FIRST FPS GUNPLAY MULTIPLIERS
// ----------------------------------------------------------------------------
// Architectural Note: Implements ability design centered on:
// 1. Utility over Lethality: Tools that create space, gather information, or
//    manipulate lines of sight (smokes, intel darts) rather than press-to-kill nukes.
// 2. Explicit Telegraphing & Counter-Play: Audible sonar pings, smoke canister hisses,
//    and aerodynamic dash trails ensure enemies understand mechanics and can counter.
// 3. Movement Synergy: Phase Dash and Grav-Lift directly multiply FPS gunplay mobility,
//    forcing opponents to adjust aim rather than just absorbing damage.

use avian3d::prelude::*;
use bevy::prelude::{Transform as BevyTransform, *};
use tracing::info;

use spacetime_rts_logic::TacticalAbilityKind;
use crate::audio_feedback::play_sound;
use crate::components::*;
use crate::core::*;

/// Architectural Note: Handles input detection and execution for tactical abilities in FPS perspective.
pub fn tactical_ability_input_system(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    camera_mode: Res<State<CameraMode>>,
    mut ability_state: ResMut<TacticalAbilityState>,
    audio_handles: Res<CombatAudioHandles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut player_q: Query<(Entity, &mut BevyTransform, &mut LinearVelocity, &mut Kcc), With<PlayerBody>>,
    camera_q: Query<&GlobalTransform, With<FpsCamera>>,
    spatial_query: SpatialQuery,
) {
    if console.is_open || *camera_mode.get() != CameraMode::FPS {
        return;
    }

    let Ok((player_entity, p_transform, mut lin_vel, mut kcc)) = player_q.get_single_mut() else {
        return;
    };
    let Ok(cam_global) = camera_q.get_single() else {
        return;
    };

    let cam_forward = cam_global.forward();
    let cam_pos = cam_global.translation();

    // ------------------------------------------------------------------------
    // 1. PHASE DASH (Key Q or LShift) - Horizontal Momentum Burst
    // ------------------------------------------------------------------------
    if (keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::ShiftLeft))
        && ability_state.cooldowns.is_ready(TacticalAbilityKind::PhaseDash)
    {
        if ability_state.cooldowns.trigger(TacticalAbilityKind::PhaseDash).is_ok() {
            let mut dash_dir = Vec3::ZERO;
            if keys.pressed(KeyCode::KeyW) { dash_dir += *p_transform.forward(); }
            if keys.pressed(KeyCode::KeyS) { dash_dir -= *p_transform.forward(); }
            if keys.pressed(KeyCode::KeyD) { dash_dir += *p_transform.right(); }
            if keys.pressed(KeyCode::KeyA) { dash_dir -= *p_transform.right(); }
            dash_dir.y = 0.0;

            if dash_dir == Vec3::ZERO {
                dash_dir = *p_transform.forward();
                dash_dir.y = 0.0;
            }
            let normalized_dir = dash_dir.normalize_or_zero();

            ability_state.is_dashing = true;
            ability_state.dash_timer.reset();
            ability_state.dash_velocity = normalized_dir * 32.0;

            // Apply immediate burst velocity
            lin_vel.x = ability_state.dash_velocity.x;
            lin_vel.z = ability_state.dash_velocity.z;

            // Telegraph: Play aerodynamic dash sound
            play_sound(&mut commands, &audio_handles.dash_whoosh);

            // Telegraph: High-contrast cyan particle slipstream trail
            let trail_mesh = meshes.add(bevy::math::primitives::Sphere::new(0.18));
            let trail_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.0, 1.0, 1.0, 0.8),
                unlit: true,
                ..default()
            });

            for step in 1..=4 {
                let offset = p_transform.translation - normalized_dir * (step as f32 * 0.7);
                commands.spawn((
                    PbrBundle {
                        mesh: trail_mesh.clone(),
                        material: trail_mat.clone(),
                        transform: BevyTransform::from_translation(offset),
                        ..default()
                    },
                    Particle { timer: Timer::from_seconds(0.35, TimerMode::Once) },
                ));
            }
            info!("Executed Tactical Ability: Phase Dash");
        }
    }

    // ------------------------------------------------------------------------
    // 2. SMOKE VEIL (Key C) - Line of Sight Manipulation & Space Creation
    // ------------------------------------------------------------------------
    if keys.just_pressed(KeyCode::KeyC) && ability_state.cooldowns.is_ready(TacticalAbilityKind::SmokeVeil) {
        if ability_state.cooldowns.trigger(TacticalAbilityKind::SmokeVeil).is_ok() {
            play_sound(&mut commands, &audio_handles.smoke_hiss);

            // Raycast forward to find ground/wall impact
            let hit = spatial_query.cast_ray(
                cam_pos, cam_forward, 18.0, true,
                SpatialQueryFilter::from_excluded_entities([player_entity]),
            );
            let target_point = hit.map_or(cam_pos + cam_forward * 12.0, |h| cam_pos + cam_forward * h.time_of_impact);

            // Spawn central smoke anchor with 8-second lifetime
            commands.spawn((
                PbrBundle {
                    transform: BevyTransform::from_translation(target_point),
                    ..default()
                },
                SmokeCloudMarker {
                    timer: Timer::from_seconds(8.0, TimerMode::Once),
                    radius: 7.5,
                },
            ));

            // Spawn volumetric smoke cloud puffs
            let smoke_mesh = meshes.add(bevy::math::primitives::Sphere::new(1.8));
            let smoke_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.65, 0.68, 0.72, 0.75),
                perceptual_roughness: 0.95,
                ..default()
            });

            let offsets = [
                Vec3::new(0.0, 0.5, 0.0),
                Vec3::new(1.8, 1.0, 0.0),
                Vec3::new(-1.8, 1.0, 0.0),
                Vec3::new(0.0, 1.2, 1.8),
                Vec3::new(0.0, 1.2, -1.8),
                Vec3::new(1.3, 2.0, 1.3),
                Vec3::new(-1.3, 2.0, -1.3),
                Vec3::new(-1.3, 2.0, 1.3),
                Vec3::new(1.3, 2.0, -1.3),
            ];

            for off in offsets {
                commands.spawn((
                    PbrBundle {
                        mesh: smoke_mesh.clone(),
                        material: smoke_mat.clone(),
                        transform: BevyTransform::from_translation(target_point + off),
                        ..default()
                    },
                    Particle { timer: Timer::from_seconds(8.0, TimerMode::Once) },
                ));
            }
            info!("Executed Tactical Ability: Smoke Veil deployed at {:?}", target_point);
        }
    }

    // ------------------------------------------------------------------------
    // 3. INTEL DART (Key X) - Reconnaissance Sonar Beacon
    // ------------------------------------------------------------------------
    if keys.just_pressed(KeyCode::KeyX) && ability_state.cooldowns.is_ready(TacticalAbilityKind::IntelDart) {
        if ability_state.cooldowns.trigger(TacticalAbilityKind::IntelDart).is_ok() {
            let hit = spatial_query.cast_ray(
                cam_pos, cam_forward, 45.0, true,
                SpatialQueryFilter::from_excluded_entities([player_entity]),
            );
            let target_point = hit.map_or(cam_pos + cam_forward * 30.0, |h| cam_pos + cam_forward * h.time_of_impact);

            // Telegraph: Initial sonar ping upon deployment
            play_sound(&mut commands, &audio_handles.sonar_ping);

            // Anchor dart marker entity with 3 sonar pings (1.5s interval)
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.2)),
                    material: materials.add(StandardMaterial {
                        base_color: Color::srgb(1.0, 0.85, 0.1),
                        unlit: true,
                        ..default()
                    }),
                    transform: BevyTransform::from_translation(target_point),
                    ..default()
                },
                IntelDartMarker {
                    pings_left: 3,
                    ping_timer: Timer::from_seconds(1.5, TimerMode::Repeating),
                    radius: 15.0,
                },
                Particle { timer: Timer::from_seconds(5.0, TimerMode::Once) },
            ));

            // Immediate expanding sonar ring visual
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Torus::new(0.2, 0.05)),
                    material: materials.add(StandardMaterial {
                        base_color: Color::srgba(1.0, 0.85, 0.2, 0.8),
                        unlit: true,
                        ..default()
                    }),
                    transform: BevyTransform::from_translation(target_point),
                    ..default()
                },
                IntelSonarPulseVisual {
                    timer: Timer::from_seconds(0.8, TimerMode::Once),
                    max_radius: 15.0,
                },
            ));
            info!("Executed Tactical Ability: Intel Dart anchored at {:?}", target_point);
        }
    }

    // ------------------------------------------------------------------------
    // 4. GRAV-LIFT (Key F) - Vertical Kinetic Air Jet (Mobility Multiplier)
    // ------------------------------------------------------------------------
    if keys.just_pressed(KeyCode::KeyF) && ability_state.cooldowns.is_ready(TacticalAbilityKind::GravLift) {
        if ability_state.cooldowns.trigger(TacticalAbilityKind::GravLift).is_ok() {
            // Reset downward momentum and launch upward
            lin_vel.y = 15.5; // ~7.5m vertical apex
            kcc.is_grounded = false;

            play_sound(&mut commands, &audio_handles.dash_whoosh);

            // Telegraph: Upward swirling particle jet
            let lift_mesh = meshes.add(bevy::math::primitives::Cylinder::new(0.6, 0.08));
            let lift_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.2, 1.0, 0.4, 0.7),
                unlit: true,
                ..default()
            });

            for step in 0..3 {
                let off = Vec3::new(0.0, step as f32 * 0.4, 0.0);
                commands.spawn((
                    PbrBundle {
                        mesh: lift_mesh.clone(),
                        material: lift_mat.clone(),
                        transform: BevyTransform::from_translation(p_transform.translation + off),
                        ..default()
                    },
                    Particle { timer: Timer::from_seconds(0.4, TimerMode::Once) },
                ));
            }
            info!("Executed Tactical Ability: Grav-Lift upward boost");
        }
    }
}

/// Architectural Note: Ticks ability cooldowns, active dash momentum, and world ability markers.
pub fn update_tactical_abilities_system(
    mut commands: Commands,
    time: Res<Time>,
    mut ability_state: ResMut<TacticalAbilityState>,
    audio_handles: Res<CombatAudioHandles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut smoke_q: Query<(Entity, &mut SmokeCloudMarker)>,
    mut dart_q: Query<(Entity, &BevyTransform, &mut IntelDartMarker)>,
    mut pulse_q: Query<(Entity, &mut BevyTransform, &mut IntelSonarPulseVisual), Without<IntelDartMarker>>,
    mut player_q: Query<(&mut LinearVelocity, &Kcc), With<PlayerBody>>,
) {
    let dt = time.delta_seconds();

    // 1. Advance authoritative cooldown timers
    ability_state.cooldowns.tick(dt);

    // 2. Process active Phase Dash momentum
    if ability_state.is_dashing {
        ability_state.dash_timer.tick(time.delta());
        if ability_state.dash_timer.just_finished() {
            ability_state.is_dashing = false;
        } else if let Ok((mut lin_vel, _)) = player_q.get_single_mut() {
            lin_vel.x = ability_state.dash_velocity.x;
            lin_vel.z = ability_state.dash_velocity.z;
        }
    }

    // 3. Process Smoke Clouds
    for (entity, mut smoke) in smoke_q.iter_mut() {
        if smoke.timer.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn_recursive();
        }
    }

    // 4. Process Intel Darts (Periodic Sonar Pings)
    for (entity, dart_trans, mut dart) in dart_q.iter_mut() {
        if dart.ping_timer.tick(time.delta()).just_finished() && dart.pings_left > 0 {
            dart.pings_left -= 1;

            // Audible acoustic chime
            play_sound(&mut commands, &audio_handles.sonar_ping);

            // Spawn expanding sonar visual ring
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Torus::new(0.2, 0.06)),
                    material: materials.add(StandardMaterial {
                        base_color: Color::srgba(1.0, 0.85, 0.2, 0.75),
                        unlit: true,
                        ..default()
                    }),
                    transform: *dart_trans,
                    ..default()
                },
                IntelSonarPulseVisual {
                    timer: Timer::from_seconds(0.85, TimerMode::Once),
                    max_radius: dart.radius,
                },
            ));

            if dart.pings_left == 0 {
                commands.entity(entity).despawn_recursive();
            }
        }
    }

    // 4. Expand Sonar Pulse Visual Rings
    for (entity, mut ring_trans, mut pulse) in pulse_q.iter_mut() {
        pulse.timer.tick(time.delta());
        let frac = pulse.timer.fraction();
        let current_radius = frac * pulse.max_radius;
        ring_trans.scale = Vec3::new(current_radius, 1.0, current_radius);

        if pulse.timer.just_finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
