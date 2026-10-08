// ============================================================================
// File: prediction.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PREDICTION COMPONENTS & ECS TYPES (Bevy Engine / SpacetimeDB)
// ----------------------------------------------------------------------------
// Architectural Note: Implements client-side movement prediction and authoritative
// server reconciliation. Calculates unacknowledged inputs and intra-tick physics
// displacement, applying smooth correction without false snapping or physics desyncs.

use bevy::prelude::*;
use avian3d::prelude::{Position as PhysicsPosition, LinearVelocity};
use std::collections::VecDeque;
use crate::network::SpacetimeConnection;
use crate::core::{NetworkTickTimer, NoClipState};
use crate::components::RtsCameraRig;
use crate::camera::{CharacterCameraSettings, CameraTransitionState};

use crate::module_bindings::process_movement_reducer::process_movement;

#[derive(Resource, Default)]
pub struct ClientTick(pub u64);

#[derive(Clone, Debug)]
pub struct BufferedInput {
    pub tick_id: u64,
    pub delta: Vec3,
}

#[derive(Component, Default)]
pub struct InputBuffer {
    pub queue: VecDeque<BufferedInput>,
}

#[derive(Component, Default)]
pub struct AuthoritativeState {
    pub position: Vec3,
    pub last_processed_tick: u64,
}

#[derive(Component, Default)]
pub struct LocalMovementTracker {
    pub last_position: Vec3,
}

pub struct PredictionPlugin;

impl Plugin for PredictionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientTick>();
    }
}

// ----------------------------------------------------------------------------
// CLIENT-SIDE PREDICTION & SERVER RECONCILIATION
// ----------------------------------------------------------------------------

pub fn buffer_and_send_movement(
    mut tick: ResMut<ClientTick>,
    mut timer: ResMut<NetworkTickTimer>,
    time: Res<Time>,
    mut query: Query<(&Transform, &mut InputBuffer, &mut LocalMovementTracker, &AuthoritativeState), With<AuthoritativeState>>,
    conn: Res<SpacetimeConnection>,
    noclip: Option<Res<NoClipState>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    if noclip.as_ref().map_or(false, |nc| nc.is_active) {
        for (transform, mut buffer, mut tracker, _) in query.iter_mut() {
            tracker.last_position = transform.translation;
            buffer.queue.clear();
        }
        return;
    }

    for (transform, mut buffer, mut tracker, auth_state) in query.iter_mut() {
        let delta = transform.translation - tracker.last_position;

        if delta.length_squared() > 0.000001 {
            // Safety guard: Ensure client tick is always strictly ahead of the server's last acknowledged tick
            if tick.0 < auth_state.last_processed_tick {
                tick.0 = auth_state.last_processed_tick;
            }
            tick.0 += 1;

            buffer.queue.push_back(BufferedInput {
                tick_id: tick.0,
                delta,
            });

            let _ = conn.db.reducers.process_movement(tick.0, delta.x, delta.y, delta.z);

            tracker.last_position = transform.translation;
        }
    }
}

pub fn reconcile_server_state(
    mut query: Query<(
        &mut Transform,
        Option<&mut PhysicsPosition>,
        Option<&mut LinearVelocity>,
        &mut InputBuffer,
        &AuthoritativeState,
        &mut LocalMovementTracker,
    ), Changed<AuthoritativeState>>,
    mut cam_settings: Option<ResMut<CharacterCameraSettings>>,
    mut transition_state: Option<ResMut<CameraTransitionState>>,
    mut rts_rig_query: Query<&mut Transform, (With<RtsCameraRig>, Without<AuthoritativeState>)>,
    noclip: Option<Res<NoClipState>>,
) {
    if noclip.as_ref().map_or(false, |nc| nc.is_active) {
        for (transform, _, _, mut buffer, _, mut tracker) in query.iter_mut() {
            buffer.queue.clear();
            tracker.last_position = transform.translation;
        }
        return;
    }
    // Architectural Note: 0.35m threshold (0.1225m^2) absorbs natural slope elevation
    // clamping and network packet timing jitter without false rollback loops.
    const TOLERANCE_SQ: f32 = 0.35 * 0.35;
    // Hard teleport / death respawn threshold: 12.0m (144.0m^2).
    // Prevents false positives during high-speed sprints or intra-tick prediction buffers.
    const TELEPORT_THRESHOLD_SQ: f32 = 12.0 * 12.0;

    for (mut transform, maybe_physics_pos, maybe_lin_vel, mut buffer, auth_state, mut tracker) in query.iter_mut() {
        // 1. Discard acknowledged inputs
        buffer.queue.retain(|input| input.tick_id > auth_state.last_processed_tick);

        // 2. Replay pending inputs on top of verified server position
        let mut predicted_pos = auth_state.position;
        for unacked_input in &buffer.queue {
            predicted_pos += unacked_input.delta;
        }

        // 3. Compensate for intra-tick unbuffered displacement
        let intra_tick_delta = transform.translation - tracker.last_position;
        let expected_live_pos = predicted_pos + intra_tick_delta;

        // 4. Divergence validation against predicted expected live position
        let divergence_sq = transform.translation.distance_squared(expected_live_pos);
        let direct_dist_sq = transform.translation.distance_squared(auth_state.position);

        // True hard teleport / death respawn: divergence exceeds 12m from expected live position
        // AND the raw server distance exceeds 12m.
        if divergence_sq > TELEPORT_THRESHOLD_SQ && direct_dist_sq > TELEPORT_THRESHOLD_SQ {
            // Hard teleport / death respawn detected: Flush all pre-death movement deltas,
            // zero residual physics momentum, and align player body & camera horizontally.
            buffer.queue.clear();
            transform.translation = auth_state.position;
            transform.rotation = Quat::IDENTITY;

            if let Some(mut phys_pos) = maybe_physics_pos {
                phys_pos.0 = auth_state.position;
            }
            if let Some(mut lin_vel) = maybe_lin_vel {
                lin_vel.0 = Vec3::ZERO;
            }
            tracker.last_position = auth_state.position;

            // Level camera pitch, yaw, and reset free-look offsets
            if let Some(ref mut settings) = cam_settings {
                settings.current_pitch = 0.0;
                settings.free_look_yaw = 0.0;
                settings.free_look_pitch = 0.0;
                settings.current_distance = settings.target_distance;
            }

            // Halt any active camera transition from old death coordinates
            if let Some(ref mut trans) = transition_state {
                trans.is_transitioning = false;
            }

            // Snap RTS camera rig to the respawn coordinates so tactical view is centered
            for mut rig_t in rts_rig_query.iter_mut() {
                rig_t.translation = auth_state.position;
            }

            tracing::info!(
                "Respawn / Hard Teleport Reconciliation: Snapped to {:?}, leveled camera pitch, reset yaw, and cleared prediction buffer.",
                auth_state.position
            );
            continue;
        }

        if divergence_sq > TOLERANCE_SQ {
            // Smoothly reconcile minor discrepancies (< 1.0m) to eliminate visual stuttering / popping,
            // while snapping moderate-to-large discrepancies immediately.
            let correction = if divergence_sq > 1.0 {
                expected_live_pos
            } else {
                transform.translation.lerp(expected_live_pos, 0.45)
            };

            transform.translation = correction;
            if let Some(mut phys_pos) = maybe_physics_pos {
                // Synchronize Avian3D PhysicsPosition so physics engine does not revert transform
                phys_pos.0 = correction;
            }
            tracker.last_position = correction;

            tracing::debug!(
                "Reconciliation Rollback triggered! Corrected divergence of {:.4} units.",
                divergence_sq.sqrt()
            );
        }
    }
}

// ----------------------------------------------------------------------------
// UNIT TESTS: PREDICTION & RECONCILIATION
// ----------------------------------------------------------------------------

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_respawn_hard_teleport_reconciles_view_angle_and_rig() {
        let mut app = App::new();
        app.add_systems(Update, reconcile_server_state);

        app.insert_resource(CharacterCameraSettings {
            current_pitch: 0.75,
            free_look_yaw: 0.42,
            free_look_pitch: -0.31,
            target_distance: 3.5,
            current_distance: 3.5,
            ..default()
        });

        // Spawn RTS camera rig at old death location
        let rig_id = app.world_mut().spawn((
            RtsCameraRig,
            Transform::from_xyz(85.0, 12.0, 95.0),
        )).id();

        // Spawn player with pending inputs at old death location
        let mut buffer = InputBuffer::default();
        buffer.queue.push_back(BufferedInput {
            tick_id: 10,
            delta: Vec3::new(1.0, 0.0, 1.0),
        });

        let player_id = app.world_mut().spawn((
            Transform::from_xyz(85.0, 12.0, 95.0).with_rotation(Quat::from_rotation_y(1.57)),
            buffer,
            AuthoritativeState {
                position: Vec3::new(0.0, 2.0, 0.0), // Respawn at origin
                last_processed_tick: 11,
            },
            LocalMovementTracker {
                last_position: Vec3::new(85.0, 12.0, 95.0),
            },
        )).id();

        // Run reconciliation system
        app.update();

        // Verify player transform, rotation, buffer, and camera state
        let player_transform = app.world().get::<Transform>(player_id).unwrap();
        assert_eq!(player_transform.translation, Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(player_transform.rotation, Quat::IDENTITY);

        let input_buffer = app.world().get::<InputBuffer>(player_id).unwrap();
        assert!(input_buffer.queue.is_empty(), "Prediction queue must be cleared on hard respawn");

        let cam_settings = app.world().resource::<CharacterCameraSettings>();
        assert_eq!(cam_settings.current_pitch, 0.0);
        assert_eq!(cam_settings.free_look_yaw, 0.0);
        assert_eq!(cam_settings.free_look_pitch, 0.0);

        let rig_transform = app.world().get::<Transform>(rig_id).unwrap();
        assert_eq!(rig_transform.translation, Vec3::new(0.0, 2.0, 0.0), "RTS camera rig must snap to respawn coordinates");
    }
}