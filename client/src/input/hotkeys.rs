// ============================================================================
// File: client/src/input/hotkeys.rs
// ============================================================================
// ----------------------------------------------------------------------------
// CENTRALIZED HOTKEY & MODAL EVENT PIPELINE
// ----------------------------------------------------------------------------
// Architectural Note:
// Acts as the Sole Source of Truth for keyboard hotkeys and modal window toggling.
// Eliminates raw hardware polling in domain files (building, spellbook, zone editor, etc.)
// and guarantees that when the developer console or text inputs are open,
// ZERO rogue gameplay or modal events are dispatched into the simulation.

use bevy::prelude::*;
use bevy::ecs::system::SystemParam;
use crate::core::ConsoleState;
use crate::building::BuildModeState;
use crate::zone_editor::ZoneEditorState;

// ----------------------------------------------------------------------------
// SEMANTIC BEVY EVENTS
// ----------------------------------------------------------------------------

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleBuildModeEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildCycleTemplateEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildCyclePieceEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildRotatePieceEvent {
    pub clockwise: bool,
}

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleSpellbookEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleOptionsEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleWorldMapEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleZoneEditorEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorBrushResizeEvent {
    pub grow: bool,
}

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ToggleCharacterEditorEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleWeaponTunerEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleDiagnosticOverlayEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TogglePerspectiveEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleSkillsSheetEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleInventoryEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleCelestialHudEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleCrosshairMenuEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleWeaponHandEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponReloadEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq)]
pub struct CelestialCycleStepEvent {
    pub forward: bool,
}

#[derive(Event, Clone, Copy, Debug, PartialEq)]
pub struct CelestialTimeScaleStepEvent {
    pub faster: bool,
}

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CelestialCycleDayNightEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CelestialCycleWeatherEvent;

#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleNoClipEvent;

// ----------------------------------------------------------------------------
// SYSTEM PARAMETER BUNDLE (Groups all event writers within Bevy's 16-param limit)
// ----------------------------------------------------------------------------

#[derive(SystemParam)]
pub struct ModalHotkeyWriters<'w> {
    pub build_toggle: EventWriter<'w, ToggleBuildModeEvent>,
    pub build_tmpl: EventWriter<'w, BuildCycleTemplateEvent>,
    pub build_piece: EventWriter<'w, BuildCyclePieceEvent>,
    pub build_rot: EventWriter<'w, BuildRotatePieceEvent>,
    pub spellbook: EventWriter<'w, ToggleSpellbookEvent>,
    pub options: EventWriter<'w, ToggleOptionsEvent>,
    pub map: EventWriter<'w, ToggleWorldMapEvent>,
    pub zone_editor: EventWriter<'w, ToggleZoneEditorEvent>,
    pub brush_resize: EventWriter<'w, EditorBrushResizeEvent>,
    pub char_editor: EventWriter<'w, ToggleCharacterEditorEvent>,
    pub tuner: EventWriter<'w, ToggleWeaponTunerEvent>,
    pub diag: EventWriter<'w, ToggleDiagnosticOverlayEvent>,
    pub persp: EventWriter<'w, TogglePerspectiveEvent>,
    pub skills: EventWriter<'w, ToggleSkillsSheetEvent>,
    pub inv: EventWriter<'w, ToggleInventoryEvent>,
    pub cel_hud: EventWriter<'w, ToggleCelestialHudEvent>,
    pub crosshair: EventWriter<'w, ToggleCrosshairMenuEvent>,
    pub weapon_hand: EventWriter<'w, ToggleWeaponHandEvent>,
    pub reload: EventWriter<'w, WeaponReloadEvent>,
    pub cel_step: EventWriter<'w, CelestialCycleStepEvent>,
    pub cel_scale: EventWriter<'w, CelestialTimeScaleStepEvent>,
    pub cel_day_night: EventWriter<'w, CelestialCycleDayNightEvent>,
    pub cel_weather: EventWriter<'w, CelestialCycleWeatherEvent>,
    pub noclip: EventWriter<'w, ToggleNoClipEvent>,
}

// ----------------------------------------------------------------------------
// CENTRALIZED HOTKEY DISPATCH SYSTEM
// ----------------------------------------------------------------------------

pub fn hotkey_dispatch_system(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    build_state: Option<Res<BuildModeState>>,
    zone_editor_state: Option<Res<ZoneEditorState>>,
    mut writers: ModalHotkeyWriters,
) {
    // 🛡️ CRITICAL SOLE GATEKEEPER:
    // When the developer console is open and user is typing,
    // ZERO gameplay, modal, or hotkey events may fire!
    if console.is_open {
        return;
    }

    // Modal & Window Toggles
    if keys.just_pressed(KeyCode::KeyB) { writers.build_toggle.send(ToggleBuildModeEvent); }
    if keys.just_pressed(KeyCode::KeyO) { writers.options.send(ToggleOptionsEvent); }
    if keys.just_pressed(KeyCode::KeyK) { writers.spellbook.send(ToggleSpellbookEvent); }
    if keys.just_pressed(KeyCode::KeyM) { writers.map.send(ToggleWorldMapEvent); }
    if keys.just_pressed(KeyCode::KeyL) { writers.skills.send(ToggleSkillsSheetEvent); }
    if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::KeyI) { writers.inv.send(ToggleInventoryEvent); }
    if keys.just_pressed(KeyCode::KeyV) { writers.persp.send(TogglePerspectiveEvent); }
    if keys.just_pressed(KeyCode::KeyH) { writers.weapon_hand.send(ToggleWeaponHandEvent); }
    if keys.just_pressed(KeyCode::F2) || keys.just_pressed(KeyCode::KeyN) { writers.noclip.send(ToggleNoClipEvent); }

    // Contextual 'R' key: In build mode -> cycle piece; in combat/exploration -> reload
    if keys.just_pressed(KeyCode::KeyR) {
        let is_building = build_state.as_ref().map_or(false, |b| b.is_active);
        if is_building {
            writers.build_piece.send(BuildCyclePieceEvent);
        } else {
            writers.reload.send(WeaponReloadEvent);
        }
    }

    // In-Build Controls (only evaluated when Build Mode is active)
    let is_building = build_state.as_ref().map_or(false, |b| b.is_active);
    if is_building {
        if keys.just_pressed(KeyCode::KeyY) { writers.build_tmpl.send(BuildCycleTemplateEvent); }
        if keys.just_pressed(KeyCode::KeyQ) { writers.build_rot.send(BuildRotatePieceEvent { clockwise: false }); }
        if keys.just_pressed(KeyCode::KeyE) { writers.build_rot.send(BuildRotatePieceEvent { clockwise: true }); }
    }

    // Function keys (Dev tools, Overlays, Tuners)
    if keys.just_pressed(KeyCode::F3) { writers.diag.send(ToggleDiagnosticOverlayEvent); }
    if keys.just_pressed(KeyCode::F4) { writers.zone_editor.send(ToggleZoneEditorEvent); }
    if keys.just_pressed(KeyCode::F5) { writers.char_editor.send(ToggleCharacterEditorEvent); }
    if keys.just_pressed(KeyCode::F6) { writers.tuner.send(ToggleWeaponTunerEvent); }
    if keys.just_pressed(KeyCode::F7) { writers.crosshair.send(ToggleCrosshairMenuEvent); }
    if keys.just_pressed(KeyCode::F8) { writers.cel_day_night.send(CelestialCycleDayNightEvent); }
    if keys.just_pressed(KeyCode::F9) { writers.cel_weather.send(CelestialCycleWeatherEvent); }
    if keys.just_pressed(KeyCode::F10) { writers.cel_hud.send(ToggleCelestialHudEvent); }

    // Contextual Bracket Keys: Zone Editor active -> Brush Resize; otherwise -> Scrub Sky
    let is_editor_active = zone_editor_state.as_ref().map_or(false, |z| z.is_editor_active && !z.is_world_map_open);
    if is_editor_active {
        if keys.just_pressed(KeyCode::BracketLeft) { writers.brush_resize.send(EditorBrushResizeEvent { grow: false }); }
        if keys.just_pressed(KeyCode::BracketRight) { writers.brush_resize.send(EditorBrushResizeEvent { grow: true }); }
    } else {
        if keys.just_pressed(KeyCode::BracketLeft) { writers.cel_step.send(CelestialCycleStepEvent { forward: false }); }
        if keys.just_pressed(KeyCode::BracketRight) { writers.cel_step.send(CelestialCycleStepEvent { forward: true }); }
    }

    // Simulation Time Scale (+ / -)
    if keys.just_pressed(KeyCode::Minus) { writers.cel_scale.send(CelestialTimeScaleStepEvent { faster: false }); }
    if keys.just_pressed(KeyCode::Equal) { writers.cel_scale.send(CelestialTimeScaleStepEvent { faster: true }); }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::ButtonInput;
    use crate::input::InputPlugin;

    #[test]
    fn test_hotkeys_suppressed_when_console_open() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin::default());
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<crate::core::GameState>();
        app.insert_state(crate::core::GameState::InGame);
        app.add_plugins(InputPlugin);
        app.insert_resource(ConsoleState { is_open: true, ..default() });
        app.insert_resource(BuildModeState::default());
        app.insert_resource(ZoneEditorState::default());

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyB);
        keys.press(KeyCode::KeyM);
        keys.press(KeyCode::KeyO);
        keys.press(KeyCode::KeyK);
        keys.press(KeyCode::KeyP);
        keys.press(KeyCode::KeyI);
        keys.press(KeyCode::KeyR);
        app.insert_resource(keys);

        app.update();

        assert!(app.world().resource::<Events<ToggleBuildModeEvent>>().is_empty());
        assert!(app.world().resource::<Events<ToggleWorldMapEvent>>().is_empty());
        assert!(app.world().resource::<Events<ToggleOptionsEvent>>().is_empty());
        assert!(app.world().resource::<Events<ToggleSpellbookEvent>>().is_empty());
        assert!(app.world().resource::<Events<ToggleSkillsSheetEvent>>().is_empty());
        assert!(app.world().resource::<Events<ToggleInventoryEvent>>().is_empty());
        assert!(app.world().resource::<Events<WeaponReloadEvent>>().is_empty());
    }

    #[test]
    fn test_hotkeys_contextual_arbitration() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin::default());
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<crate::core::GameState>();
        app.insert_state(crate::core::GameState::InGame);
        app.add_plugins(InputPlugin);
        app.insert_resource(ConsoleState { is_open: false, ..default() });
        app.insert_resource(BuildModeState { is_active: false, ..default() });
        app.insert_resource(ZoneEditorState { is_editor_active: false, is_world_map_open: false, ..default() });

        // Normal mode: KeyR sends WeaponReloadEvent
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyR);
        app.insert_resource(keys);
        app.update();

        assert_eq!(app.world().resource::<Events<WeaponReloadEvent>>().len(), 1);
        assert!(app.world().resource::<Events<BuildCyclePieceEvent>>().is_empty());

        // Build mode active: KeyR sends BuildCyclePieceEvent instead of WeaponReloadEvent
        app.insert_resource(BuildModeState { is_active: true, ..default() });
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyR);
        app.insert_resource(keys);
        app.update();

        assert_eq!(app.world().resource::<Events<BuildCyclePieceEvent>>().len(), 1);

        // Zone editor active: BracketLeft sends EditorBrushResizeEvent
        app.insert_resource(ZoneEditorState { is_editor_active: true, is_world_map_open: false, ..default() });
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::BracketLeft);
        app.insert_resource(keys);
        app.update();

        assert_eq!(app.world().resource::<Events<EditorBrushResizeEvent>>().len(), 1);
        assert!(app.world().resource::<Events<CelestialCycleStepEvent>>().is_empty());
    }
}
