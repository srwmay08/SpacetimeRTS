use bevy::prelude::*;
use std::collections::VecDeque;

/// Tracks the client's current simulation tick.
#[derive(Resource, Default)]
pub struct ClientTick(pub u64);

/// Represents a single unit of movement input that has been predicted locally
/// but not yet confirmed by the SpacetimeDB backend.
#[derive(Clone, Debug)]
pub struct BufferedInput {
    pub tick_id: u64,
    pub delta: Vec3,
}

/// Placed on the local player's entity to queue recent movements.
#[derive(Component, Default)]
pub struct InputBuffer {
    pub queue: VecDeque<BufferedInput>,
}

/// Placed on the local player's entity to store the latest verified
/// state received from SpacetimeDB.
#[derive(Component, Default)]
pub struct AuthoritativeState {
    pub position: Vec3,
    pub last_processed_tick: u64,
}

pub struct PredictionPlugin;

impl Plugin for PredictionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientTick>()
           .add_systems(Update, (
               apply_local_input,
               reconcile_server_state
           ).chain());
    }
}

/// Reads local player input, instantly moves the Bevy transform for zero perceived lag,
/// buffers the action, and transmits the network RPC.
fn apply_local_input(
    mut tick: ResMut<ClientTick>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut InputBuffer), With<AuthoritativeState>>,
) {
    // Determine movement intent based on WASD
    let mut intent = Vec3::ZERO;
    if keyboard_input.pressed(KeyCode::KeyW) { intent.z -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyS) { intent.z += 1.0; }
    if keyboard_input.pressed(KeyCode::KeyA) { intent.x -= 1.0; }
    if keyboard_input.pressed(KeyCode::KeyD) { intent.x += 1.0; }

    if intent == Vec3::ZERO {
        return; 
    }

    // Normalize and scale by player speed and delta time
    let speed = 4.0;
    let delta = intent.normalize_or_zero() * speed * time.delta_seconds();

    // Increment local simulation tick
    tick.0 += 1;

    for (mut transform, mut buffer) in query.iter_mut() {
        // 1. Predict instantly on the client UI (zero perceived latency)
        transform.translation += delta;

        // 2. Push to local input buffer for future reconciliation
        buffer.queue.push_back(BufferedInput {
            tick_id: tick.0,
            delta,
        });

        // 3. Fire the RPC to SpacetimeDB
        // Note: In production, this binds to the auto-generated SDK module.
        // spacetimedb_bindings::process_movement(tick.0, delta.x, delta.y, delta.z);
    }
}

/// Evaluates authoritative state updates from SpacetimeDB against the local buffer.
/// If the client drifted beyond the tolerance radius, forces a rollback and replays inputs.
fn reconcile_server_state(
    mut query: Query<(&mut Transform, &mut InputBuffer, &AuthoritativeState), Changed<AuthoritativeState>>,
) {
    // Maximum acceptable positional drift (squared for cheaper CPU checks)
    const TOLERANCE_SQ: f32 = 0.05 * 0.05;

    for (mut transform, mut buffer, auth_state) in query.iter_mut() {
        // 1. Cull Acknowledged Inputs
        // Discard any inputs in our buffer that the server has already computed.
        buffer.queue.retain(|input| input.tick_id > auth_state.last_processed_tick);

        // 2. Re-simulate unacknowledged inputs on top of the true server position
        let mut predicted_pos = auth_state.position;
        for unacked_input in &buffer.queue {
            predicted_pos += unacked_input.delta;
        }

        // 3. Divergence Check
        // If the client's current perceived position differs significantly from 
        // what it *should* be based on authoritative state + pending inputs, we rollback.
        let divergence_sq = transform.translation.distance_squared(predicted_pos);
        
        if divergence_sq > TOLERANCE_SQ {
            // Rollback and instantly snap to the correct predicted position.
            // Because we re-added the pending inputs to the server's state, this 
            // completely negates latency jitter while enforcing server authority.
            transform.translation = predicted_pos;
            info!(
                "Reconciliation Rollback triggered! Corrected divergence of {} units.", 
                divergence_sq.sqrt()
            );
        }
    }
}