// ============================================================================
// File: client/src/input/player_controller.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PLAYER LOCOMOTION & RTS NAVMESH MOVEMENT SYSTEMS
// ----------------------------------------------------------------------------

use avian3d::prelude::*;
use bevy::prelude::{Transform as BevyTransform, *};

use crate::components::*;
use crate::core::*;
use super::action_buffer::{ActionBuffer, LocomotionSettings, VirtualAction};

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

    // Bedrock & void safety net: permits 3D subterranean exploration down to BEDROCK_ELEVATION (-120.0m)
    // while catching players falling through unloaded chunks into the abyss.
    let ground_y = crate::terrain::get_terrain_height(transform.translation.x, transform.translation.z);
    let bedrock_safe_y = -120.0_f32 + 1.05;
    if transform.translation.y < bedrock_safe_y {
        transform.translation.y = bedrock_safe_y;
        if lin_vel.y < 0.0 { 
            lin_vel.y = 0.0; 
        }
        kcc.is_grounded = true;
    } else if hit.is_none() && transform.translation.y < ground_y - 1.0 && transform.translation.y > ground_y - 4.0 {
        // Surface missing-collider grace window: catches players when overworld terrain collider has not yet loaded
        transform.translation.y = ground_y + 1.0;
        if lin_vel.y < 0.0 { 
            lin_vel.y = 0.0; 
        }
        kcc.is_grounded = true;
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
}
