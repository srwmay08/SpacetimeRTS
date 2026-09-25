// ============================================================================
// File: prediction.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PREDICTION COMPONENTS & ECS TYPES (Bevy Engine / SpacetimeDB)
// ----------------------------------------------------------------------------
// Architectural Note: Implements client-side movement prediction and authoritative
// server reconciliation. Calculates unacknowledged inputs and intra-tick physics
// displacement, applying smooth correction without false snapping.

use bevy::prelude::*;
use std::collections::VecDeque;
use crate::network::SpacetimeConnection;
use crate::core::NetworkTickTimer;

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
        app.init_resource::<ClientTick>()
           .add_systems(Update, (
               buffer_and_send_movement,
               reconcile_server_state
           ).chain());
    }
}

// ----------------------------------------------------------------------------
// CLIENT-SIDE PREDICTION & SERVER RECONCILIATION
// ----------------------------------------------------------------------------

fn buffer_and_send_movement(
    mut tick: ResMut<ClientTick>,
    mut timer: ResMut<NetworkTickTimer>,
    time: Res<Time>,
    mut query: Query<(&Transform, &mut InputBuffer, &mut LocalMovementTracker), With<AuthoritativeState>>,
    conn: Res<SpacetimeConnection>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    for (transform, mut buffer, mut tracker) in query.iter_mut() {
        let delta = transform.translation - tracker.last_position;

        if delta.length_squared() > 0.000001 {
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

fn reconcile_server_state(
    mut query: Query<(&mut Transform, &mut InputBuffer, &AuthoritativeState, &mut LocalMovementTracker), Changed<AuthoritativeState>>,
) {
    // Architectural Note: 0.25m threshold (0.0625m^2) absorbs natural slope elevation
    // clamping while catching true authoritative desyncs.
    const TOLERANCE_SQ: f32 = 0.25 * 0.25;

    for (mut transform, mut buffer, auth_state, mut tracker) in query.iter_mut() {
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

        // 4. Divergence validation
        let divergence_sq = transform.translation.distance_squared(expected_live_pos);

        if divergence_sq > TOLERANCE_SQ {
            transform.translation = expected_live_pos;
            tracker.last_position = expected_live_pos;

            tracing::info!(
                "Reconciliation Rollback triggered! Corrected divergence of {:.4} units.",
                divergence_sq.sqrt()
            );
        }
    }
}