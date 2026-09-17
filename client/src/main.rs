mod module_bindings;
mod core;
mod components;
mod network;
mod input;
mod camera;
mod terrain;
mod ui;
mod prediction;
mod building; 

use avian3d::prelude::*;
use bevy::prelude::*;
use tracing::warn;

use crate::core::*;
use crate::network::*;
use crate::input::*;
use crate::camera::*;
use crate::terrain::*;
use crate::ui::*;
use crate::building::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
        .add_plugins(prediction::PredictionPlugin) 
        .insert_resource(Msaa::Off)
        
        .init_state::<GameState>()
        .init_state::<CameraMode>()
        .add_event::<ActionEvent>() 
        
        .insert_resource(EventTracker::default()) 
        .insert_resource(GeneratedChunks::default())
        .insert_resource(SelectionState::default())
        .insert_resource(BuildModeState::default())
        .insert_resource(NetworkTickTimer(Timer::from_seconds(0.05, TimerMode::Repeating)))
        .insert_resource(SwingState { is_swinging: false, timer: Timer::from_seconds(0.3, TimerMode::Once) })
        .insert_resource(AmbientLight { color: Color::srgb(1.0, 0.95, 0.9), brightness: 400.0 })
        .insert_resource(TelemetryTracker { last_frame_time: 0.0, frame_drop_threshold: 0.033 })
        
        .add_systems(OnEnter(GameState::Connecting), init_network_connection)
        .add_systems(Update, wait_for_connection.run_if(in_state(GameState::Connecting)))

        .add_systems(OnEnter(GameState::InGame), (spawn_initial_world, setup_ui))
        .add_systems(OnEnter(CameraMode::FPS), enable_fps_perspective)
        .add_systems(OnEnter(CameraMode::RTS), enable_rts_perspective)

        .add_systems(Update, (
            track_telemetry_metrics,
            toggle_perspective,
            input_router_system, 
            rts_navmesh_movement_system, 
            player_movement_system,
        ).run_if(in_state(GameState::InGame)))

        // Architectural Note: Registered complex and high-parameter systems individually 
        // to bypass Bevy's internal system configuration tuple size limitations.
        .add_systems(Update, context_aware_action_dispatcher.run_if(in_state(GameState::InGame)))
        .add_systems(Update, update_infinite_terrain_chunks.run_if(in_state(GameState::InGame)))

        .add_systems(Update, (
            send_movement_input, 
            sync_transforms, 
            reconcile_local_transform, 
            sync_logical_components,
            sync_resource_nodes, 
            sync_ground_loot,
            sync_structures, 
        ).run_if(in_state(GameState::InGame)))

        .add_systems(Update, (
            toggle_build_mode, 
            update_build_hologram, 
            animate_view_model, 
            update_inventory_ui, 
            toggle_inventory_ui,
            process_combat_events, 
            tick_particles,
            visualize_selection 
        ).run_if(in_state(GameState::InGame)))    

        .add_systems(Update, fps_look.run_if(in_state(CameraMode::FPS).and_then(in_state(GameState::InGame))))
        .add_systems(Update, (
            rts_camera_controller,
            update_marquee_ui,
            update_floating_health_bars
        ).run_if(in_state(CameraMode::RTS).and_then(in_state(GameState::InGame))))
        
        .run();
}

fn track_telemetry_metrics(time: Res<Time>, mut telemetry: ResMut<TelemetryTracker>) {
    let delta = time.delta_seconds_f64();
    if delta > telemetry.frame_drop_threshold {
        warn!("ECS Frame Drop Detected! Frame took {:.2}ms", delta * 1000.0);
    }
    telemetry.last_frame_time = time.elapsed_seconds_f64();
}