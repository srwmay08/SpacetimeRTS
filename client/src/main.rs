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
mod weapons;
mod tuner;
mod audio_feedback;
mod tactical_abilities;
pub mod binary_sky;
pub mod tree_colors;
pub mod zone_editor;
pub mod spellbook;
pub mod skills_ui;
pub mod voxel_mesh;
pub mod trees;
pub mod props;
pub mod creatures;
pub mod resource_nodes;
pub mod grass;
pub mod templates;

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
use crate::weapons::*;
use crate::tuner::*;
use crate::audio_feedback::*;
use crate::tactical_abilities::*;
use crate::tree_colors::*;
use crate::resource_nodes::*;

// P2 Fix: SystemSets for explicit ordering and predictable behavior
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum UpdateSet {
    /// Input handling (keyboard, mouse, gamepad)
    Input,
    /// Network synchronization (SpacetimeDB)
    Network,
    /// Game logic (AI, combat, building)
    Logic,
    /// Physics and movement
    Physics,
    /// Visual/UI updates
    Rendering,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "SpacetimeRTS".into(),
                    present_mode: bevy::window::PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            }),
            PhysicsPlugins::default(),
            bevy::diagnostic::FrameTimeDiagnosticsPlugin,
            bevy::diagnostic::EntityCountDiagnosticsPlugin,
            bevy::diagnostic::SystemInformationDiagnosticsPlugin,
        ))
        .add_plugins(prediction::PredictionPlugin) 
        .add_plugins(binary_sky::BinarySkyPlugin)
        .add_plugins(terrain::TerrainPlugin)
        .add_plugins(zone_editor::ZoneEditorPlugin)
        .add_plugins(spellbook::SpellbookPlugin)
        .add_plugins(skills_ui::SkillsSheetPlugin)
        .add_plugins(grass::GrassPlugin)
        .insert_resource(Msaa::Off)
        
        // Architectural Note: Luminous Sky Clear Color.
        // Synchronized with FogSettings (0.75, 0.84, 0.92) to form an airy, cohesive horizon.
        .insert_resource(ClearColor(Color::srgb(0.75, 0.84, 0.92)))

        .init_state::<GameState>()
        .init_state::<CameraMode>()
        .add_event::<ActionEvent>() 
        .add_event::<BuildingDestructionEvent>()
        .add_event::<SpawnBuildingEvent>()
        
        // P2 Fix: Configure SystemSets for explicit ordering
        .configure_sets(Update, (
            UpdateSet::Input,
            UpdateSet::Network,
            UpdateSet::Logic,
            UpdateSet::Physics,
            UpdateSet::Rendering,
        ).chain())
        
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
        .insert_resource(ActiveOffHandItem(None))
        .insert_resource(NetworkTickTimer(Timer::from_seconds(0.05, TimerMode::Repeating)))
        .insert_resource(SwingState::default())
        .insert_resource(AmbientLight { color: Color::srgb(0.95, 0.98, 1.0), brightness: 550.0 })
        .insert_resource(TelemetryTracker { last_frame_time: 0.0, frame_drop_threshold: 0.1 })
        .init_resource::<WeaponState>()
        .init_resource::<WeaponTunerState>()
        .init_resource::<EquippedHandSide>()
        .init_resource::<ClientEquippedBags>()
        .init_resource::<CrosshairSettings>()
        .init_resource::<CrosshairMenuState>()
        .init_resource::<TacticalAbilityState>()
        .init_resource::<HitMarkerState>()
        .init_resource::<SeasonState>()
        .init_resource::<TreeMaterialHandles>()
        .init_resource::<FpsLimiterState>()
        .init_resource::<BuildingAssetManifest>()
        
        .add_systems(OnEnter(GameState::Connecting), init_network_connection)
        .add_systems(Update, wait_for_connection.run_if(in_state(GameState::Connecting)))

        .add_systems(OnEnter(GameState::InGame), (spawn_initial_world, setup_ui, setup_tuner_ui, setup_procedural_combat_audio))
        .add_systems(OnEnter(CameraMode::FPS), enable_fps_perspective)
        .add_systems(OnEnter(CameraMode::RTS), enable_rts_perspective)

        .add_systems(Update, (
            track_telemetry_metrics,
            toggle_console,
            handle_console_input,
            toggle_celestial_hud_hotkey,
            update_console_ui,
            toggle_perspective,
            update_camera_transition,
            hotbar_input_system,
            input_router_system, 
            rts_navmesh_movement_system, 
            player_movement_system,
            toggle_tuner_ui,
            handle_tuner_interactions,
            update_tuner_ui_display,
            update_comic_damage_floaters,
            toggle_weapon_hand_system,
        ).run_if(in_state(GameState::InGame)))

        .add_systems(Update, (
            handle_inventory_drag_and_drop,
            handle_paperdoll_interactions,
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
            update_tree_colors,
            update_berry_visuals, 
            sync_structures, 
            spawn_modular_building_system,
            update_building_destruction_visuals,
            handle_building_destruction,
            update_building_destruction_animations,
            update_rune_light_decay,
            sync_door_states,
            animate_doors,
            sync_fall_hazards,
            update_fall_hazards,
            sync_active_projectiles,
            tick_voxel_gibs,
        ).run_if(in_state(GameState::InGame)))

        .add_systems(Update, (
            toggle_build_mode, 
            update_build_hologram, 
            update_build_ui, 
            update_hotbar_ui,
            update_hud_health_bar,
            update_celestial_hud_ui,
            update_interaction_prompt,
            update_inventory_ui, 
            toggle_inventory_ui,
            process_combat_events, 
            tick_particles,
            visualize_selection,
            action_bar_interaction,       
            toggle_action_bar_visibility,
            update_floating_health_bars,
        ).run_if(in_state(GameState::InGame)))

        .add_systems(Update, (
            spawn_or_update_view_model_weapon,
            animate_weapon_viewmodel,
            weapon_reload_input_system,
            update_weapon_hud,
            update_reticle_crosshair_ui,
            update_reticle_adjacent_hud,
            update_reticle_abilities_and_hitmarker,
            toggle_crosshair_menu,
            handle_crosshair_menu_interactions,
            tactical_ability_input_system,
            update_tactical_abilities_system,
        ).run_if(in_state(GameState::InGame)))    

        .add_systems(Update, fps_look.run_if(in_state(CameraMode::FPS).and_then(in_state(GameState::InGame))))
        .add_systems(Update, (
            rts_camera_controller,
            update_marquee_ui,
        ).run_if(in_state(CameraMode::RTS).and_then(in_state(GameState::InGame))))
        
        .add_systems(Update, update_diagnostic_overlay.run_if(in_state(GameState::InGame)))
        .add_systems(Last, enforce_fps_limit)
        
        .run();
}

fn update_diagnostic_overlay(
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut root_q: Query<&mut Style, With<DiagnosticOverlayRoot>>,
    mut text_q: Query<&mut Text, With<DiagnosticOverlayText>>,
) {
    if keyboard.just_pressed(KeyCode::F3) {
        if let Ok(mut style) = root_q.get_single_mut() {
            style.display = if style.display == Display::None {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    let Ok(style) = root_q.get_single() else { return; };
    if style.display == Display::None {
        return;
    }

    let Ok(mut text) = text_q.get_single_mut() else { return; };
    let mut output = String::new();

    if let Some(fps) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.value().or_else(|| fps.smoothed()) {
            output.push_str(&format!("FPS: {:.1}\n", value));
        }
    }
    if let Some(frame_time) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME) {
        if let Some(value) = frame_time.value().or_else(|| frame_time.smoothed()) {
            output.push_str(&format!("Frame: {:.2} ms\n", value));
        }
    }
    if let Some(entities) = diagnostics.get(&bevy::diagnostic::EntityCountDiagnosticsPlugin::ENTITY_COUNT) {
        if let Some(value) = entities.value() {
            output.push_str(&format!("Entities: {:.0}", value));
        }
    }

    if output.is_empty() {
        output = "Collecting telemetry...".to_string();
    }

    if text.sections.len() > 1 {
        if text.sections[1].value != output {
            text.sections[1].value = output;
        }
    } else if !text.sections.is_empty() && text.sections[0].value != output {
        text.sections[0].value = output;
    }
}

fn track_telemetry_metrics(time: Res<Time>, mut telemetry: ResMut<TelemetryTracker>) {
    telemetry.last_frame_time = time.elapsed_seconds_f64();
}