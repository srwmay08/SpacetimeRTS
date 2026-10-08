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
pub mod subterrain;
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
pub mod voxel_mesh;
pub mod prng;
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
use crate::creatures::{setup_sparring_yard, update_training_dummy_wobble, update_sparring_goblin_ai};

pub use crate::core::UpdateSet;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_sparring = args.iter().any(|arg| arg == "--sparring" || arg == "-s" || arg == "sparring")
        || std::env::var("SPARRING_MODE").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false)
        || std::env::var("SPARRING").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);

    if is_sparring {
        println!("🥋 Launching client in Sparring Sandbox Mode (--sparring)");
    } else {
        println!("🌲 Launching client in Main World (Run with '-- --sparring' to enter the Sparring Yard)");
    }

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
        .add_plugins(SkillsSheetPlugin)
        .add_plugins(grass::GrassPlugin)
        .add_plugins(input::InputPlugin)
        .insert_resource(Msaa::Off)
        
        // Architectural Note: Luminous Sky Clear Color.
        // Synchronized with FogSettings (0.75, 0.84, 0.92) to form an airy, cohesive horizon.
        .insert_resource(ClearColor(Color::srgb(0.75, 0.84, 0.92)))

        .init_state::<GameState>()
        .init_state::<CameraMode>()
        .add_event::<BuildingDestructionEvent>()
        .add_event::<SpawnBuildingEvent>()
        
        // Strict deterministic execution order for InGame update systems:
        // Input -> Network -> Logic -> Physics -> Animation -> Rendering
        .configure_sets(Update, (
            UpdateSet::Input,
            UpdateSet::Network,
            UpdateSet::Logic,
            UpdateSet::Physics,
            UpdateSet::Animation,
            UpdateSet::Rendering,
        ).chain().run_if(in_state(GameState::InGame)))
        
        .insert_resource(EventTracker::default()) 
        .insert_resource(GeneratedChunks::default())
        .insert_resource(SelectionState::default())
        .insert_resource(BuildModeState::default())
        .insert_resource(NetworkCullingState::default()) 
        .insert_resource(CameraTransitionState::default())
        .insert_resource(CharacterCameraSettings::default())
        .insert_resource(ConsoleState::default())
        .insert_resource(DragDropState::default())
        .insert_resource(ActiveItemSlot(0))
        .insert_resource(CachedPlayerEntity::default())
        .insert_resource(SparringMode(is_sparring))
        .insert_resource(if is_sparring {
            ActiveEquippedItem(Some("Crude Bow".to_string()))
        } else {
            ActiveEquippedItem(None)
        })
        .insert_resource(ActiveOffHandItem(None))
        .insert_resource(NetworkTickTimer(Timer::from_seconds(0.05, TimerMode::Repeating)))
        .insert_resource(SwingState::default())
        .init_resource::<MouseFlickTracker>()
        .insert_resource(AmbientLight { color: Color::srgb(0.95, 0.98, 1.0), brightness: 550.0 })
        .insert_resource(TelemetryTracker { last_frame_time: 0.0, frame_drop_threshold: 0.1 })
        .init_resource::<WeaponState>()
        .init_resource::<WeaponTunerState>()
        .init_resource::<EquippedHandSide>()
        .init_resource::<ClientEquippedBags>()
        .init_resource::<CrosshairSettings>()
        .init_resource::<CrosshairMenuState>()
        .init_resource::<TacticalAbilityState>()
        .init_resource::<ActionBuffer>()
        .init_resource::<LocomotionSettings>()
        .init_resource::<HitMarkerState>()
        .init_resource::<SeasonState>()
        .init_resource::<TreeFoliageConfig>()
        .init_resource::<TreeMaterialHandles>()
        .init_resource::<FpsLimiterState>()
        .init_resource::<BuildingAssetManifest>()
        .init_resource::<NoClipState>()
        
        .add_systems(OnEnter(GameState::Connecting), init_network_connection)
        .add_systems(Update, wait_for_connection.run_if(in_state(GameState::Connecting)))

        .add_systems(OnEnter(GameState::InGame), (spawn_initial_world, setup_ui, setup_tuner_ui, setup_procedural_combat_audio, setup_sparring_yard))
        .add_systems(OnEnter(CameraMode::FPS), enable_fps_perspective)
        .add_systems(OnEnter(CameraMode::RTS), enable_rts_perspective)

        // ==========================================
        // 1. INPUT PHASE
        // Keyboard, mouse, UI toggles, and modal clicks
        // ==========================================
        .add_systems(Update, (
            toggle_console,
            handle_console_input,
            toggle_celestial_hud_hotkey,
            toggle_perspective,
            hotbar_input_system,
            input_router_system,
            toggle_tuner_ui,
            handle_tuner_interactions,
            toggle_weapon_hand_system,
            weapon_reload_input_system,
            tactical_ability_input_system,
            toggle_crosshair_menu,
            handle_crosshair_menu_interactions,
            toggle_noclip_system,
        ).in_set(UpdateSet::Input))

        .add_systems(Update, (
            handle_inventory_drag_and_drop,
            handle_paperdoll_interactions,
            handle_build_menu_selection,
            handle_crafting_interaction,
            toggle_build_mode,
            toggle_inventory_ui,
            action_bar_interaction,
            toggle_action_bar_visibility,
        ).in_set(UpdateSet::Input))

        // ==========================================
        // 2. NETWORK PHASE
        // SpacetimeDB table sync and subscription streams
        // ==========================================
        .add_systems(Update, (
            sync_transforms,
            crate::prediction::reconcile_server_state,
            update_spatial_subscriptions,
            sync_logical_components,
            sync_resource_nodes,
            sync_structures,
            sync_door_states,
            sync_fall_hazards,
            sync_active_projectiles,
            sync_third_person_weapon_render_layers,
        ).in_set(UpdateSet::Network))

        // ==========================================
        // 3. LOGIC PHASE
        // Simulation, combat, actions, and procedural terrain
        // ==========================================
        .add_systems(Update, (
            context_aware_action_dispatcher,
            process_combat_events,
            update_tactical_abilities_system,
            update_arrow_projectiles,
            interior_occlusion_culling_system,
            update_infinite_voxel_terrain,
            spawn_modular_building_system,
            handle_building_destruction,
            spawn_or_update_view_model_weapon,
        ).in_set(UpdateSet::Logic))

        .add_systems(Update, (
            update_sparring_goblin_ai,
            update_training_dummy_wobble,
            update_falling_trees,
            update_tree_colors,
            update_berry_visuals,
            update_fall_hazards,
            tick_voxel_gibs,
            tick_particles,
            update_rune_light_decay,
        ).in_set(UpdateSet::Logic))

        // ==========================================
        // 4. PHYSICS PHASE
        // Locomotion and navmesh movement
        // ==========================================
        .add_systems(Update, (
            player_movement_system,
            rts_navmesh_movement_system,
            crate::prediction::buffer_and_send_movement,
        ).in_set(UpdateSet::Physics))

        // ==========================================
        // 5. ANIMATION & CAMERA ORIENTATION PHASE
        // Camera look, viewmodel bob, and procedural animation
        // (Runs strictly after Physics to eliminate 1-frame camera jitter)
        // ==========================================
        .add_systems(Update, (
            update_camera_transition,
            animate_doors,
            animate_weapon_viewmodel,
            animate_third_person_weapons,
            update_building_destruction_animations,
            fps_look.run_if(in_state(CameraMode::FPS)),
            update_camera_fov.run_if(in_state(CameraMode::FPS)),
            rts_camera_controller.run_if(in_state(CameraMode::RTS)),
        ).in_set(UpdateSet::Animation))

        // ==========================================
        // 6. RENDERING & UI PHASE
        // Reticles, HUD, damage numbers, and diagnostics
        // ==========================================
        .add_systems(Update, (
            update_build_hologram,
            update_build_ui,
            update_comic_damage_floaters,
            update_building_destruction_visuals,
            visualize_selection,
            update_floating_health_bars,
            update_drag_ghost_ui,
            update_marquee_ui.run_if(in_state(CameraMode::RTS)),
        ).in_set(UpdateSet::Rendering))

        .add_systems(Update, (
            update_console_ui,
            update_tuner_ui_display,
            update_hotbar_ui,
            update_hud_health_bar,
            update_celestial_hud_ui,
            update_interaction_prompt,
            update_inventory_ui,
            update_weapon_hud,
            update_reticle_crosshair_ui,
            update_reticle_adjacent_hud,
            update_reticle_abilities_and_hitmarker,
            update_diagnostic_overlay,
            track_telemetry_metrics,
        ).in_set(UpdateSet::Rendering))

        .add_systems(Last, enforce_fps_limit)
        
        .run();
}

fn update_diagnostic_overlay(
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut diag_evts: EventReader<crate::input::ToggleDiagnosticOverlayEvent>,
    mut root_q: Query<&mut Style, With<DiagnosticOverlayRoot>>,
    mut text_q: Query<&mut Text, With<DiagnosticOverlayText>>,
    noclip: Option<Res<NoClipState>>,
) {
    for _ in diag_evts.read() {
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

    if let Some(ref nc) = noclip {
        if nc.is_active {
            output.push_str(&format!(
                "\n\n[FLY NOCLIP ACTIVE]\nSpeed: {:.0} m/s (Shift: {:.0} m/s)\nSpace: Up | Ctrl/C: Down",
                nc.fly_speed,
                nc.fly_speed * nc.fast_multiplier
            ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_registered_systems_query_disjointness() {
        let mut world = World::new();

        macro_rules! check_system {
            ($sys:expr) => {
                let mut system = IntoSystem::into_system($sys);
                system.initialize(&mut world);
            };
        }

        check_system!(toggle_console);
        check_system!(handle_console_input);
        check_system!(toggle_celestial_hud_hotkey);
        check_system!(toggle_perspective);
        check_system!(hotbar_input_system);
        check_system!(input_router_system);
        check_system!(toggle_tuner_ui);
        check_system!(handle_tuner_interactions);
        check_system!(toggle_weapon_hand_system);
        check_system!(weapon_reload_input_system);
        check_system!(tactical_ability_input_system);
        check_system!(toggle_crosshair_menu);
        check_system!(handle_crosshair_menu_interactions);
        check_system!(toggle_noclip_system);
        check_system!(handle_inventory_drag_and_drop);
        check_system!(handle_paperdoll_interactions);
        check_system!(handle_build_menu_selection);
        check_system!(handle_crafting_interaction);
        check_system!(toggle_build_mode);
        check_system!(toggle_inventory_ui);
        check_system!(action_bar_interaction);
        check_system!(toggle_action_bar_visibility);
        check_system!(sync_transforms);
        check_system!(crate::prediction::reconcile_server_state);
        check_system!(update_spatial_subscriptions);
        check_system!(sync_logical_components);
        check_system!(sync_resource_nodes);
        check_system!(sync_structures);
        check_system!(sync_door_states);
        check_system!(sync_fall_hazards);
        check_system!(sync_active_projectiles);
        check_system!(sync_third_person_weapon_render_layers);
        check_system!(context_aware_action_dispatcher);
        check_system!(process_combat_events);
        check_system!(update_tactical_abilities_system);
        check_system!(update_arrow_projectiles);
        check_system!(interior_occlusion_culling_system);
        check_system!(update_infinite_voxel_terrain);
        check_system!(spawn_modular_building_system);
        check_system!(handle_building_destruction);
        check_system!(spawn_or_update_view_model_weapon);
        check_system!(update_sparring_goblin_ai);
        check_system!(update_training_dummy_wobble);
        check_system!(update_falling_trees);
        check_system!(update_tree_colors);
        check_system!(update_berry_visuals);
        check_system!(update_fall_hazards);
        check_system!(tick_voxel_gibs);
        check_system!(tick_particles);
        check_system!(update_rune_light_decay);
        check_system!(player_movement_system);
        check_system!(rts_navmesh_movement_system);
        check_system!(crate::prediction::buffer_and_send_movement);
        check_system!(update_camera_transition);
        check_system!(animate_doors);
        check_system!(animate_weapon_viewmodel);
        check_system!(animate_third_person_weapons);
        check_system!(update_building_destruction_animations);
        check_system!(fps_look);
        check_system!(update_camera_fov);
        check_system!(rts_camera_controller);
        check_system!(update_build_hologram);
        check_system!(update_build_ui);
        check_system!(update_comic_damage_floaters);
        check_system!(update_building_destruction_visuals);
        check_system!(visualize_selection);
        check_system!(update_floating_health_bars);
        check_system!(update_drag_ghost_ui);
        check_system!(update_marquee_ui);
        check_system!(update_console_ui);
        check_system!(update_tuner_ui_display);
        check_system!(update_hotbar_ui);
        check_system!(update_hud_health_bar);
        check_system!(update_celestial_hud_ui);
        check_system!(update_interaction_prompt);
        check_system!(update_inventory_ui);
        check_system!(update_weapon_hud);
        check_system!(update_reticle_crosshair_ui);
        check_system!(update_reticle_adjacent_hud);
        check_system!(update_reticle_abilities_and_hitmarker);
        check_system!(update_diagnostic_overlay);
        check_system!(track_telemetry_metrics);
        check_system!(enforce_fps_limit);
    }
}