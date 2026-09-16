mod module_bindings;
mod core;
mod components;
mod network;
mod input;
mod camera;
mod terrain;
mod ui;

use avian3d::prelude::*;
use bevy::prelude::*;
use tracing::warn;

use crate::core::*;
use crate::network::*;
use crate::input::*;
use crate::camera::*;
use crate::terrain::*;
use crate::ui::*;

fn main() {
    App::new()
        // Core Engine Setup
        .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
        .insert_resource(Msaa::Off)
        
        // Custom State & Event Registrations
        .init_state::<GameState>()
        .init_state::<CameraMode>()
        .add_event::<ActionEvent>() 
        
        // Initial Resource Injection
        .insert_resource(EventTracker::default()) 
        .insert_resource(GeneratedChunks::default())
        .insert_resource(SelectionState::default())
        .insert_resource(NetworkTickTimer(Timer::from_seconds(0.05, TimerMode::Repeating)))
        .insert_resource(SwingState { is_swinging: false, timer: Timer::from_seconds(0.3, TimerMode::Once) })
        .insert_resource(AmbientLight { color: Color::srgb(1.0, 0.95, 0.9), brightness: 400.0 })
        .insert_resource(TelemetryTracker { last_frame_time: 0.0, frame_drop_threshold: 0.033 })
        
        // Bootstrapping Phase
        .add_systems(OnEnter(GameState::Connecting), init_network_connection)
        .add_systems(Update, wait_for_connection.run_if(in_state(GameState::Connecting)))

        // World Generation & Initialization Phase
        .add_systems(OnEnter(GameState::InGame), (spawn_initial_world, setup_ui))
        .add_systems(OnEnter(CameraMode::FPS), enable_fps_perspective)
        .add_systems(OnEnter(CameraMode::RTS), enable_rts_perspective)

        // Primary Gameplay Loop
        .add_systems(Update, (
            track_telemetry_metrics,
            toggle_perspective,
            input_router_system, 
            context_aware_action_dispatcher, 
            rts_navmesh_movement_system, 
            player_movement_system,
            update_infinite_terrain_chunks,
            send_movement_input, 
            sync_transforms, 
            reconcile_local_transform, 
            sync_logical_components,
            sync_resource_nodes, 
            sync_ground_loot, 
            animate_view_model, 
            update_inventory_ui, 
            toggle_inventory_ui,
            process_combat_events, 
            tick_particles,
            visualize_selection 
        ).run_if(in_state(GameState::InGame)))    

        // Camera-Specific Logic Overrides
        .add_systems(Update, fps_look.run_if(in_state(CameraMode::FPS).and_then(in_state(GameState::InGame))))
        .add_systems(Update, (
            rts_camera_controller,
            update_marquee_ui
        ).run_if(in_state(CameraMode::RTS).and_then(in_state(GameState::InGame))))
        
        .run();
}

/// Core loop diagnostic tool for detecting ECS frame drops.
/// Architectural Note: Kept in main as it monitors the health of the entire application.
fn track_telemetry_metrics(time: Res<Time>, mut telemetry: ResMut<TelemetryTracker>) {
    let delta = time.delta_seconds_f64();
    if delta > telemetry.frame_drop_threshold {
        warn!("ECS Frame Drop Detected! Frame took {:.2}ms", delta * 1000.0);
    }
    telemetry.last_frame_time = time.elapsed_seconds_f64();
}