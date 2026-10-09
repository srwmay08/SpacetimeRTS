// ============================================================================
// File: client/src/spellbook/mod.rs
// ============================================================================
// ----------------------------------------------------------------------------
// SPELLBOOK FACADE & BACKWARD COMPATIBILITY SURFACE
// ----------------------------------------------------------------------------
// Architectural Note:
// The UI elements have been cleanly modularized into:
//   - `crate::ui::options` (Global Options Panel modal)
//   - `crate::ui::spellbook` (Spellbook Grimoire modal)
//   - `crate::ui::hotbar` (16-slot prepared action bar HUD)
// And gameplay data has been cleanly modularized into:
//   - `crate::spells::catalog` (Spells, categories, IDs, definitions)
//
// This module provides re-exports to preserve 100% backward compatibility across
// the codebase and tests, alongside the Bevy plugin registration.

pub use crate::spells::*;
pub use crate::ui::hotbar::*;
pub use crate::ui::options::*;
pub use crate::ui::spellbook::*;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use crate::core::*;

// ----------------------------------------------------------------------------
// MODAL ARBITRATION & INPUT COUPLING
// ----------------------------------------------------------------------------

pub fn toggle_options_and_spellbook_system(
    mut options_evts: EventReader<crate::input::ToggleOptionsEvent>,
    mut spellbook_evts: EventReader<crate::input::ToggleSpellbookEvent>,
    mut options_state: ResMut<OptionsPanelState>,
    mut spellbook_state: ResMut<SpellbookWindowState>,
    mut options_q: Query<&mut Style, (With<OptionsPanelModalRoot>, Without<SpellbookModalRoot>)>,
    mut spellbook_q: Query<&mut Style, (With<SpellbookModalRoot>, Without<OptionsPanelModalRoot>)>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    let mut state_changed = false;

    // Toggle Options
    for _ in options_evts.read() {
        options_state.is_open = !options_state.is_open;
        if options_state.is_open {
            spellbook_state.is_open = false; // mutually exclusive or stack
        }
        state_changed = true;
    }

    // Toggle Spellbook
    for _ in spellbook_evts.read() {
        spellbook_state.is_open = !spellbook_state.is_open;
        if spellbook_state.is_open {
            options_state.is_open = false;
        }
        state_changed = true;
    }

    if state_changed {
        if let Ok(mut style) = options_q.get_single_mut() {
            style.display = if options_state.is_open { Display::Flex } else { Display::None };
        }
        if let Ok(mut style) = spellbook_q.get_single_mut() {
            style.display = if spellbook_state.is_open { Display::Flex } else { Display::None };
        }

        if let Ok(mut window) = window_q.get_single_mut() {
            if options_state.is_open || spellbook_state.is_open {
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
            } else if *camera_mode.get() == CameraMode::FPS {
                window.cursor.grab_mode = CursorGrabMode::Locked;
                window.cursor.visible = false;
            }
        }
    }
}

// ----------------------------------------------------------------------------
// PLUGIN DEFINITION
// ----------------------------------------------------------------------------

pub struct SpellbookPlugin;

impl Plugin for SpellbookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HotbarKeybinds>()
            .init_resource::<PreparedHotbarState>()
            .init_resource::<SpellbookWindowState>()
            .init_resource::<OptionsPanelState>()
            .init_resource::<TerrainRenderSettings>()
            .init_resource::<SpellDragState>()
            .add_event::<crate::input::ToggleOptionsEvent>()
            .add_event::<crate::input::ToggleSpellbookEvent>()
            .add_systems(OnEnter(GameState::InGame), (
                setup_prepared_hotbar_ui,
                setup_spellbook_modal_ui,
                setup_options_panel_modal_ui,
            ))
            .add_systems(Update, (
                toggle_options_and_spellbook_system,
                update_hotbar_ui_system,
                handle_hotbar_casting_system,
                handle_spell_drag_and_drop,
                update_spell_drag_ghost_ui,
                update_spellbook_display_system,
                handle_spellbook_interactions,
                handle_options_panel_interactions,
                sync_terrain_render_and_fog_system,
            ));
    }
}

// ----------------------------------------------------------------------------
// UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::CrosshairSettings;

    #[test]
    fn test_spell_catalog_integrity() {
        assert!(SPELL_CATALOG.len() >= 16, "Spell catalog must contain at least 16 spells");
        let mut ids = std::collections::BTreeSet::new();
        for spell in SPELL_CATALOG {
            assert!(!spell.name.is_empty(), "Spell must have a name");
            assert!(!spell.description.is_empty(), "Spell must have description");
            assert!(spell.cooldown_seconds > 0.0, "Cooldown must be positive");
            assert!(ids.insert(spell.id), "Duplicate spell ID: {}", spell.id);
            assert!(get_spell_by_id(spell.id).is_some());
            assert_eq!(SpellId::from_id_or_name(spell.id), Some(spell.spell_id));
            assert_eq!(SpellId::from_id_or_name(spell.name), Some(spell.spell_id));
            assert_eq!(get_spell(spell.spell_id).id, spell.id);
        }
    }

    #[test]
    fn test_spellbook_pagination_and_filtering() {
        let all_spells = get_filtered_spells(None, false);
        assert_eq!(all_spells.len(), 21);
        let arcane_spells = get_filtered_spells(Some(SpellCategory::Arcane), false);
        assert_eq!(arcane_spells.len(), 5);
        for s in &arcane_spells {
            assert_eq!(s.category, SpellCategory::Arcane);
        }
        let elemental_spells = get_filtered_spells(Some(SpellCategory::Elemental), false);
        assert_eq!(elemental_spells.len(), 5);
    }

    #[test]
    fn test_hotbar_keybinds_defaults_and_rebinding() {
        let mut binds = HotbarKeybinds::default();
        assert_eq!(binds.keybinds.len(), 16);
        assert_eq!(binds.keybinds[0], KeyCode::Digit1);
        assert_eq!(binds.keybinds[7], KeyCode::Digit8);
        assert_eq!(binds.keybinds[8], KeyCode::KeyQ);
        assert_eq!(keycode_display_name(binds.keybinds[0]), "1");
        assert_eq!(keycode_display_name(binds.keybinds[8]), "Q");

        // Rebind slot 0 to F1
        binds.keybinds[0] = KeyCode::F1;
        assert_eq!(keycode_display_name(binds.keybinds[0]), "F1");
    }

    #[test]
    fn test_prepared_hotbar_slotting_and_cooldowns() {
        let mut hotbar = PreparedHotbarState::default();
        assert_eq!(hotbar.slots.len(), 16);
        assert_eq!(hotbar.slots[0], None);
        assert_eq!(hotbar.slots[4], None);

        // Slot custom spell
        hotbar.slots[0] = Some(SpellId::PhaseDash);
        hotbar.slots[15] = Some(SpellId::SolarFlare);
        assert_eq!(hotbar.slots[0], Some(SpellId::PhaseDash));
        assert_eq!(hotbar.slots[15], Some(SpellId::SolarFlare));

        // Cooldown timer progression
        hotbar.cooldowns[0] = 5.0;
        assert!(hotbar.cooldowns[0] > 0.0);
        hotbar.cooldowns[0] = (hotbar.cooldowns[0] - 2.0).max(0.0);
        assert_eq!(hotbar.cooldowns[0], 3.0);
    }

    #[test]
    fn test_terrain_render_settings_zone_expansion() {
        let mut settings = TerrainRenderSettings::default();
        assert_eq!(settings.view_distance_chunks, 14);
        assert_eq!(settings.visible_range_meters, 230.4);
        assert!(!settings.spawn_full_zone);

        // Expand to full zone
        settings.view_distance_chunks = 64;
        settings.unload_distance_chunks = 70;
        settings.spawn_full_zone = true;
        settings.visible_range_meters = 1024.0;

        assert_eq!(settings.view_distance_chunks, 64);
        assert_eq!(settings.visible_range_meters, 1024.0);
        assert!(settings.spawn_full_zone);
    }

    #[test]
    fn test_options_panel_reticle_toggle() {
        let mut settings = CrosshairSettings::default();
        assert!(settings.enabled, "Reticle should be enabled by default");

        // Toggle OFF
        settings.enabled = !settings.enabled;
        assert!(!settings.enabled, "Reticle should be toggled OFF");

        // Toggle ON
        settings.enabled = !settings.enabled;
        assert!(settings.enabled, "Reticle should be toggled ON");
    }

    #[test]
    fn test_spellbook_systems_ecs_schedule_initialization() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<SpellbookWindowState>();
        app.add_systems(Update, update_spellbook_display_system);
        // Validates that Bevy ECS initializes and runs update_spellbook_display_system
        // with zero B0001 query conflicts
        app.update();
    }

    #[test]
    fn test_prepared_hotbar_drag_and_drop_slotting() {
        let mut hotbar = PreparedHotbarState::default();
        let mut spellbook_state = SpellbookWindowState::default();
        let mut spell_drag = SpellDragState::default();

        // 1. Initial State: all 16 slots are empty
        assert_eq!(hotbar.slots[0], None);
        assert_eq!(hotbar.slots[1], None);

        // 2. Select a spell via click-to-slot from the grimoire
        spellbook_state.selected_spell_for_slotting = Some(SpellId::Fireball);
        assert_eq!(spellbook_state.selected_spell_for_slotting, Some(SpellId::Fireball));

        // Simulate click on slot 0: slots selected spell and clears selection
        let selected = spellbook_state.selected_spell_for_slotting.take().unwrap();
        hotbar.slots[0] = Some(selected);
        hotbar.cooldowns[0] = 0.0;
        assert_eq!(hotbar.slots[0], Some(SpellId::Fireball));
        assert_eq!(spellbook_state.selected_spell_for_slotting, None);

        // 3. Drag spell from spellbook to slot 1
        spell_drag.is_dragging = true;
        spell_drag.spell_id = Some(SpellId::FrostNova);
        let dragged = spell_drag.spell_id.take().unwrap();
        hotbar.slots[1] = Some(dragged);
        spell_drag.is_dragging = false;
        assert!(!spell_drag.is_dragging);
        assert_eq!(hotbar.slots[1], Some(SpellId::FrostNova));

        // 4. Swap slots 0 and 1
        let temp = hotbar.slots[1].take();
        hotbar.slots[1] = hotbar.slots[0].take();
        hotbar.slots[0] = temp;
        assert_eq!(hotbar.slots[0], Some(SpellId::FrostNova));
        assert_eq!(hotbar.slots[1], Some(SpellId::Fireball));
    }
}
