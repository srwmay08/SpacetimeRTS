// ============================================================================
// File: client/src/input/mod.rs
// ============================================================================
// ----------------------------------------------------------------------------
// UNIFIED INPUT & LOCOMOTION PIPELINE
// ----------------------------------------------------------------------------

use bevy::prelude::*;

pub mod action_buffer;
pub mod action_router;
pub mod combat_dispatcher;
pub mod hotkeys;
pub mod player_controller;
pub mod world_interaction;

pub use action_buffer::*;
pub use action_router::*;
pub use combat_dispatcher::*;
pub use hotkeys::*;
pub use player_controller::*;
pub use world_interaction::*;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<ActionEvent>()
            .add_event::<ToggleBuildModeEvent>()
            .add_event::<BuildCycleTemplateEvent>()
            .add_event::<BuildCyclePieceEvent>()
            .add_event::<BuildRotatePieceEvent>()
            .add_event::<ToggleSpellbookEvent>()
            .add_event::<ToggleOptionsEvent>()
            .add_event::<ToggleWorldMapEvent>()
            .add_event::<ToggleZoneEditorEvent>()
            .add_event::<EditorBrushResizeEvent>()
            .add_event::<ToggleWeaponTunerEvent>()
            .add_event::<ToggleDiagnosticOverlayEvent>()
            .add_event::<TogglePerspectiveEvent>()
            .add_event::<ToggleSkillsSheetEvent>()
            .add_event::<ToggleInventoryEvent>()
            .add_event::<ToggleCelestialHudEvent>()
            .add_event::<ToggleCrosshairMenuEvent>()
            .add_event::<ToggleWeaponHandEvent>()
            .add_event::<WeaponReloadEvent>()
            .add_event::<CelestialCycleStepEvent>()
            .add_event::<CelestialTimeScaleStepEvent>()
            .add_event::<CelestialCycleDayNightEvent>()
            .add_event::<CelestialCycleWeatherEvent>()
            .add_systems(
                Update,
                hotkey_dispatch_system.run_if(in_state(crate::core::GameState::InGame)),
            );
    }
}
