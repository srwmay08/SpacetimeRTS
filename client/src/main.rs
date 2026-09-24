// ----------------------------------------------------------------------------
// APP CONFIGURATION & SYSTEM SCHEDULING
// ----------------------------------------------------------------------------
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

use crate::core::*;
use crate::components::*;
use crate::network::*;
use crate::input::*;
use crate::camera::*;
use crate::terrain::*;
use crate::ui::*;
use crate::building::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PhysicsPlugins::default(),
            bevy::diagnostic::FrameTimeDiagnosticsPlugin,
            bevy::diagnostic::EntityCountDiagnosticsPlugin,
            bevy::diagnostic::SystemInformationDiagnosticsPlugin,
        ))
        .add_plugins(prediction::PredictionPlugin) 
        .insert_resource(Msaa::Off)
        
        .init_state::<GameState>()
        .init_state::<CameraMode>()
        .add_event::<ActionEvent>() 
        
        .insert_resource(EventTracker::default()) 
        .insert_resource(GeneratedChunks::default())
        .insert_resource(SelectionState::default())
        .insert_resource(BuildModeState::default())
        .insert_resource(NetworkCullingState::default()) 
        .insert_resource(CameraTransitionState::default())
        .insert_resource(ConsoleState::default())
        .insert_resource(DragDropState::default())
        .insert_resource(ActiveItemSlot(0))
        .insert_resource(CachedPlayerEntity::default())
        .insert_resource(ActiveEquippedItem(None))
        .insert_resource(NetworkTickTimer(Timer::from_seconds(0.05, TimerMode::Repeating)))
        .insert_resource(SwingState { is_swinging: false, timer: Timer::from_seconds(0.3, TimerMode::Once) })
        .insert_resource(AmbientLight { color: Color::srgb(1.0, 0.95, 0.9), brightness: 400.0 })
        .insert_resource(TelemetryTracker { last_frame_time: 0.0, frame_drop_threshold: 0.1 })
        
        .add_systems(OnEnter(GameState::Connecting), init_network_connection)
        .add_systems(Update, wait_for_connection.run_if(in_state(GameState::Connecting)))

        .add_systems(OnEnter(GameState::InGame), (spawn_initial_world, setup_ui))
        .add_systems(OnEnter(CameraMode::FPS), enable_fps_perspective)
        .add_systems(OnEnter(CameraMode::RTS), enable_rts_perspective)

        .add_systems(Update, (
            track_telemetry_metrics,
            toggle_console,
            handle_console_input,
            update_console_ui,
            toggle_perspective,
            update_camera_transition,
            hotbar_input_system,
            input_router_system, 
            rts_navmesh_movement_system, 
            player_movement_system,
        ).run_if(in_state(GameState::InGame)))

        .add_systems(Update, (
            handle_inventory_drag_and_drop,
            update_drag_ghost_ui,
            context_aware_action_dispatcher,
            handle_build_menu_selection,
            handle_crafting_interaction,
            interior_occlusion_culling_system,
            update_infinite_voxel_terrain,
        ).run_if(in_state(GameState::InGame)))

        .add_systems(Update, (
            sync_transforms, 
            update_spatial_subscriptions, 
            sync_logical_components,
            sync_resource_nodes, 
            update_falling_trees,
            update_berry_visuals, 
            sync_structures, 
            tick_voxel_gibs,
        ).run_if(in_state(GameState::InGame)))

        .add_systems(Update, (
            toggle_build_mode, 
            update_build_hologram, 
            update_build_ui, 
            update_hotbar_ui,
            update_hud_health_bar,
            update_interaction_prompt,
            animate_view_model, 
            update_inventory_ui, 
            toggle_inventory_ui,
            process_combat_events, 
            tick_particles,
            visualize_selection,
            action_bar_interaction,       
            toggle_action_bar_visibility,
            update_floating_health_bars,
        ).run_if(in_state(GameState::InGame)))    

        .add_systems(Update, fps_look.run_if(in_state(CameraMode::FPS).and_then(in_state(GameState::InGame))))
        .add_systems(Update, (
            rts_camera_controller,
            update_marquee_ui,
        ).run_if(in_state(CameraMode::RTS).and_then(in_state(GameState::InGame))))
        
        .add_systems(Update, update_diagnostic_overlay.run_if(in_state(GameState::InGame)))
        
        .run();
}

#[derive(Component)]
struct DiagnosticOverlay;

fn update_diagnostic_overlay(
    mut commands: Commands,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut existing: Query<(Entity, &mut Text), With<DiagnosticOverlay>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if keyboard.just_pressed(KeyCode::F3) {
        if let Some((entity, _)) = existing.iter().next() {
            commands.entity(entity).despawn();
            return;
        } else {
            commands.spawn((
                TextBundle::from_section(
                    "Loading diagnostics...",
                    TextStyle { font_size: 14.0, color: Color::srgb(0.0, 1.0, 0.0), ..default() }
                ).with_style(Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    right: Val::Px(10.0),
                    ..default()
                }),
                DiagnosticOverlay,
            ));
            return;
        }
    }
    
    for (_, mut text) in existing.iter_mut() {
        let mut output = String::new();
        
        if let Some(fps) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.value() {
                output.push_str(&format!("FPS: {:.1}\n", value));
            }
        }
        if let Some(frame_time) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME) {
            if let Some(value) = frame_time.value() {
                output.push_str(&format!("Frame: {:.2}ms\n", value));
            }
        }
        if let Some(entities) = diagnostics.get(&bevy::diagnostic::EntityCountDiagnosticsPlugin::ENTITY_COUNT) {
            if let Some(value) = entities.value() {
                output.push_str(&format!("Entities: {:.0}\n", value));
            }
        }
        
        text.sections[0].value = output;
    }
}

fn track_telemetry_metrics(time: Res<Time>, mut telemetry: ResMut<TelemetryTracker>) {
    telemetry.last_frame_time = time.elapsed_seconds_f64();
}