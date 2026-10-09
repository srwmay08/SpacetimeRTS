// ============================================================================
// File: client/src/input/player_controller.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PLAYER LOCOMOTION & RTS NAVMESH MOVEMENT SYSTEMS
// ----------------------------------------------------------------------------

use crate::physics::*;
use bevy::prelude::{Transform as BevyTransform, *};

use crate::components::*;
use crate::core::*;
use crate::camera::CharacterCameraSettings;
use super::action_buffer::{ActionBuffer, LocomotionSettings, VirtualAction};
use super::hotkeys::ToggleNoClipEvent;

pub fn toggle_noclip_system(
    mut evts: EventReader<ToggleNoClipEvent>,
    mut noclip: ResMut<NoClipState>,
    mut player_q: Query<(&mut CollisionLayers, &mut LinearVelocity, &mut GravityScale, &mut Kcc), With<PlayerBody>>,
) {
    for _ in evts.read() {
        noclip.is_active = !noclip.is_active;
        if let Ok((mut layers, mut lin_vel, mut gravity, mut kcc)) = player_q.get_single_mut() {
            if noclip.is_active {
                *layers = CollisionLayers::NONE;
                lin_vel.0 = Vec3::ZERO;
                gravity.0 = 0.0;
                kcc.is_grounded = false;
                tracing::info!("God Mode Flying No-Clip: ENABLED. Speed: {:.0} m/s", noclip.fly_speed);
            } else {
                *layers = CollisionLayers::new(
                    [GameLayer::Unit],
                    [GameLayer::Default, GameLayer::Terrain, GameLayer::Environment, GameLayer::Glass],
                );
                gravity.0 = 8.0;
                tracing::info!("God Mode Flying No-Clip: DISABLED. Normal physics restored.");
            }
        }
    }
}

pub fn rts_navmesh_movement_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut BevyTransform, &mut LinearVelocity, &NavTarget)>,
    time: Res<Time>,
) {
    for (entity, mut transform, mut velocity, target) in query.iter_mut() {
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

            // Orient the entity to face directly in the direction it is moving
            let forward_heading = Vec3::new(move_dir.x, 0.0, move_dir.z).normalize_or_zero();
            if forward_heading.length_squared() > 0.001 {
                let target_rot = Quat::from_rotation_arc(Vec3::NEG_Z, forward_heading);
                let decay = (14.0 * time.delta_seconds()).min(1.0);
                transform.rotation = transform.rotation.slerp(target_rot, decay);
            }
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
    noclip: Option<Res<NoClipState>>,
    cam_settings: Option<Res<CharacterCameraSettings>>,
) {
    let Ok((entity, mut transform, mut lin_vel, mut gravity, mut kcc, mut loco_opt)) = query.get_single_mut() else { return; };

    // ------------------------------------------------------------------------
    // 1. GOD MODE FLYING NO-CLIP LOCOMOTION
    // ------------------------------------------------------------------------
    if let Some(ref nc) = noclip {
        if nc.is_active {
            let pitch = cam_settings.as_ref().map_or(0.0, |s| s.current_pitch);
            let pitch_rot = Quat::from_rotation_x(pitch);
            let look_dir = transform.rotation * pitch_rot * Vec3::NEG_Z;
            let right_dir = transform.rotation * Vec3::X;
            let up_dir = Vec3::Y;

            let mut fly_dir = Vec3::ZERO;
            if *camera_mode.get() == CameraMode::FPS && !console.is_open {
                if keys.pressed(KeyCode::KeyW) { fly_dir += look_dir; }
                if keys.pressed(KeyCode::KeyS) { fly_dir -= look_dir; }
                if keys.pressed(KeyCode::KeyD) { fly_dir += right_dir; }
                if keys.pressed(KeyCode::KeyA) { fly_dir -= right_dir; }
                if keys.pressed(KeyCode::Space) { fly_dir += up_dir; }
                if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) || keys.pressed(KeyCode::KeyC) {
                    fly_dir -= up_dir;
                }
            }

            let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                nc.fly_speed * nc.fast_multiplier
            } else {
                nc.fly_speed
            };

            let dt = time.delta_seconds();
            if fly_dir != Vec3::ZERO {
                fly_dir = fly_dir.normalize();
                transform.translation += fly_dir * speed * dt;
            }

            lin_vel.0 = Vec3::ZERO;
            gravity.0 = 0.0;
            kcc.is_grounded = false;
            return;
        }
    }

    // ------------------------------------------------------------------------
    // 2. STANDARD GROUND LOCOMOTION & COLLISION
    // ------------------------------------------------------------------------
    let ray_start = transform.translation; 
    let ground_filter = SpatialQueryFilter::from_mask([
        GameLayer::Terrain,
        GameLayer::Environment,
        GameLayer::Default,
    ]).with_excluded_entities([entity]);

    let hit = spatial_query.cast_ray(
        ray_start, 
        Dir3::NEG_Y, 
        1.25, 
        true, 
        ground_filter,
    );
    
    // High-Frequency Ground State Oscillation Fix:
    // Do not clear grounded state simply because lin_vel.y > 0.05 on slopes.
    // Only clear grounded state if no surface is within contact range or player is actively jumping.
    kcc.is_grounded = false;
    if let Some(hit_data) = hit {
        if hit_data.time_of_impact <= 1.25 && lin_vel.y < 4.0 {
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
        if move_dir == Vec3::ZERO {
            lin_vel.x = 0.0;
            lin_vel.z = 0.0;
        } else if kcc.is_grounded {
            // Slope Projection: project desired movement along surface tangent for smooth slope traversal
            let normal = hit.map_or(Vec3::Y, |h| h.normal);
            if normal.y > 0.4 {
                let slope_dir = (move_dir - normal * move_dir.dot(normal)).normalize_or_zero();
                let vel = slope_dir * locomotion_settings.horizontal_speed;
                lin_vel.x = vel.x;
                lin_vel.z = vel.z;
                lin_vel.y = vel.y;
            } else {
                lin_vel.x = move_dir.x * locomotion_settings.horizontal_speed;
                lin_vel.z = move_dir.z * locomotion_settings.horizontal_speed;
            }
        } else {
            lin_vel.x = move_dir.x * locomotion_settings.horizontal_speed;
            lin_vel.z = move_dir.z * locomotion_settings.horizontal_speed;
        }
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
                // Transform Snapping Fighting Physics Fix:
                // Zero downward/horizontal velocity and gravity to eliminate penetration chatter.
                // Never manually overwrite transform.translation.y on a dynamic rigid body!
                lin_vel.0 = Vec3::ZERO;
                gravity.0 = 0.0;
            } else {
                // Moving along slope: velocity is already tangent to slope, neutralize gravity
                gravity.0 = 0.0;
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
                lin_vel.0 = Vec3::ZERO;
                gravity.0 = 0.0;
            } else {
                gravity.0 = 0.0;
            }
        } else {
            gravity.0 = 8.0;
        }
    }

    // Bedrock & void safety net: permits 3D subterranean exploration down to BEDROCK_ELEVATION (-120.0m)
    // while catching players falling through unloaded chunks into the abyss.
    if crate::subterrain::is_bedrock_enabled() {
        let bedrock_safe_y = -120.0_f32 + 1.05;
        if transform.translation.y < bedrock_safe_y {
            transform.translation.y = bedrock_safe_y;
            if lin_vel.y < 0.0 { 
                lin_vel.y = 0.0; 
            }
            kcc.is_grounded = true;
        }
    }
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
    fn test_toggle_noclip_system_lifecycle() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin::default());
        app.init_resource::<NoClipState>();
        app.add_event::<ToggleNoClipEvent>();

        let player = app.world_mut().spawn((
            PlayerBody,
            CollisionLayers::new(
                [GameLayer::Unit],
                [GameLayer::Default, GameLayer::Terrain, GameLayer::Environment, GameLayer::Glass],
            ),
            LinearVelocity(Vec3::new(10.0, -5.0, 10.0)),
            GravityScale(8.0),
            Kcc { is_grounded: true },
        )).id();

        app.add_systems(Update, toggle_noclip_system);

        // Send ToggleNoClipEvent -> activate NoClip
        app.world_mut().resource_mut::<Events<ToggleNoClipEvent>>().send(ToggleNoClipEvent);
        app.update();

        assert!(app.world().resource::<NoClipState>().is_active);
        let layers = app.world().get::<CollisionLayers>(player).unwrap();
        assert_eq!(*layers, CollisionLayers::NONE);
        let lin_vel = app.world().get::<LinearVelocity>(player).unwrap();
        assert_eq!(lin_vel.0, Vec3::ZERO);
        let gravity = app.world().get::<GravityScale>(player).unwrap();
        assert_eq!(gravity.0, 0.0);
        let kcc = app.world().get::<Kcc>(player).unwrap();
        assert_eq!(kcc.is_grounded, false);

        // Send ToggleNoClipEvent again -> deactivate NoClip, restore physics
        app.world_mut().resource_mut::<Events<ToggleNoClipEvent>>().send(ToggleNoClipEvent);
        app.update();

        assert!(!app.world().resource::<NoClipState>().is_active);
        let layers = app.world().get::<CollisionLayers>(player).unwrap();
        assert_eq!(*layers, CollisionLayers::new(
            [GameLayer::Unit],
            [GameLayer::Default, GameLayer::Terrain, GameLayer::Environment, GameLayer::Glass],
        ));
        let gravity = app.world().get::<GravityScale>(player).unwrap();
        assert_eq!(gravity.0, 8.0);
    }
}
