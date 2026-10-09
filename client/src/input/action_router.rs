// ============================================================================
// File: client/src/input/action_router.rs
// ============================================================================
// ----------------------------------------------------------------------------
// INPUT ROUTING & QUICK-SLOT HOTBAR SYSTEMS
// ----------------------------------------------------------------------------

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use tracing::info;

use crate::components::*;
use crate::core::*;
use crate::network::SpacetimeConnection;
use crate::weapons::EquippedHandSide;
use crate::module_bindings::player_table::PlayerTableAccess;
use crate::module_bindings::inventory_table::InventoryTableAccess;
use crate::module_bindings::equipment_loadout_table::EquipmentLoadoutTableAccess;
use crate::module_bindings::equip_weapon_reducer::equip_weapon;
use spacetime_rts_logic::{HandSide, TacticalAbilityKind};
use crate::weapons::WeaponType;

use super::action_buffer::{ActionBuffer, ActionEvent, ActionState, VirtualAction};

pub fn hotbar_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    mut active_slot: ResMut<ActiveItemSlot>,
    mut active_item: ResMut<ActiveEquippedItem>,
    mut active_offhand: ResMut<ActiveOffHandItem>,
    hand_side: Res<EquippedHandSide>,
    conn: Res<SpacetimeConnection>,
    sparring_mode: Res<SparringMode>,
) {
    if console.is_open {
        return;
    }

    let digit_keys = [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
        (KeyCode::Digit6, 5),
        (KeyCode::Digit7, 6),
        (KeyCode::Digit8, 7),
    ];

    let Some(identity) = &conn.identity else { return; };
    let Some(player) = conn.db.db.player().identity().find(identity) else { return; };
    let inv = conn.db.db.inventory().entity_id().find(&player.entity_id);

    let equip_to_offhand = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) || hand_side.0 == HandSide::Left;
    let alt_held = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);

    for (key, slot_idx) in digit_keys {
        if alt_held && keys.just_pressed(key) {
            active_slot.0 = slot_idx;
            info!("Inventory Quick-Slot {} Selected (Alt+{})", slot_idx + 1, slot_idx + 1);
            if let Some(ref inventory) = inv {
                if let Some(slot) = inventory.slots.get(slot_idx) {
                    if slot.count > 0 && !slot.item_type.is_empty() {
                        let target_slot = if equip_to_offhand {
                            crate::ui::types::EquipmentSlot::OffHand
                        } else {
                            crate::ui::types::EquipmentSlot::MainHand
                        };
                        info!("Equipping '{}' to {}", slot.item_type, target_slot.as_str());
                        let _ = conn.db.reducers.equip_weapon(target_slot.as_str().to_string(), slot.item_type.clone());
                    }
                }
            }
        }
    }

    // Read authoritative equipped weapon from SpacetimeDB EquipmentLoadout
    let loadout = conn.db.db.equipment_loadout().entity_id().find(&player.entity_id);
    if let Some(l) = loadout {
        if l.main_hand != "None" && !l.main_hand.is_empty() {
            active_item.0 = Some(l.main_hand.clone());
        } else if sparring_mode.0 && active_item.0.is_none() {
            active_item.0 = Some("Crude Bow".to_string());
        } else if !sparring_mode.0 {
            active_item.0 = None;
        }

        let is_two_handed = active_item.0.as_deref().map_or(false, |m| WeaponType::from_item_name(Some(m)).is_two_handed());

        if !is_two_handed && l.off_hand != "None" && !l.off_hand.is_empty() {
            active_offhand.0 = Some(l.off_hand.clone());
        } else {
            active_offhand.0 = None;
        }
    } else if sparring_mode.0 {
        if active_item.0.is_none() {
            active_item.0 = Some("Crude Bow".to_string());
        }
        active_offhand.0 = None;
    } else {
        active_item.0 = None;
        active_offhand.0 = None;
    }
}

pub fn input_router_system(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    camera_mode: Res<State<CameraMode>>,
    time: Res<Time>,
    mut action_buffer: ResMut<ActionBuffer>,
    drag_drop: Res<DragDropState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut action_events: EventWriter<ActionEvent>,
    interaction_query: Query<&Interaction>,
    node_query: Query<(&Node, &GlobalTransform, &Visibility, &Style)>,
    mut flick_tracker: ResMut<MouseFlickTracker>,
    mut mouse_motion: EventReader<MouseMotion>,
    char_editor: Option<Res<crate::character_customizer::CharacterEditorState>>,
) {
    if console.is_open {
        return;
    }

    // Directional mouse flick tracking for granular melee combat (Mount & Blade / cRPG style)
    let mut total_dx = 0.0;
    let mut total_dy = 0.0;
    for motion in mouse_motion.read() {
        total_dx += motion.delta.x;
        total_dy += motion.delta.y;
    }
    if total_dx.abs() > 0.01 || total_dy.abs() > 0.01 {
        flick_tracker.update(total_dx, total_dy);
    } else {
        flick_tracker.decay(0.90);
    }

    let current_time = time.elapsed_seconds_f64();
    action_buffer.prune(current_time);

    let cursor_pos = window_query.get_single().ok().and_then(|w| w.cursor_position());
    let cursor_locked = window_query.get_single().map_or(false, |w| w.cursor.grab_mode == CursorGrabMode::Locked);
    let is_editor_open = char_editor.as_ref().map_or(false, |e| e.is_open);

    let mut is_over_ui = if cursor_locked {
        false
    } else {
        interaction_query.iter().any(|i| *i != Interaction::None) || drag_drop.is_dragging || is_editor_open
    };
    
    if !cursor_locked && !is_over_ui {
        if let Some(pos) = cursor_pos {
            for (node, transform, vis, style) in node_query.iter() {
                if *vis != Visibility::Hidden && style.display != Display::None {
                    if style.width == Val::Percent(100.0) && style.height == Val::Percent(100.0) {
                        continue;
                    }
                    let rect = Rect::from_center_size(transform.translation().truncate(), node.size());
                    if rect.contains(pos) {
                        is_over_ui = true;
                        break;
                    }
                }
            }
        }
    }

    if mouse.just_pressed(MouseButton::Left) {
        action_events.send(ActionEvent { action: VirtualAction::Primary, state: ActionState::JustPressed, cursor_pos, is_over_ui });
    }
    if mouse.pressed(MouseButton::Left) {
        action_events.send(ActionEvent { action: VirtualAction::Primary, state: ActionState::Pressed, cursor_pos, is_over_ui });
    }
    if mouse.just_released(MouseButton::Left) {
        action_events.send(ActionEvent { action: VirtualAction::Primary, state: ActionState::JustReleased, cursor_pos, is_over_ui });
    }

    if mouse.just_pressed(MouseButton::Right) {
        action_events.send(ActionEvent { action: VirtualAction::Secondary, state: ActionState::JustPressed, cursor_pos, is_over_ui });
    }
    if mouse.pressed(MouseButton::Right) {
        action_events.send(ActionEvent { action: VirtualAction::Secondary, state: ActionState::Pressed, cursor_pos, is_over_ui });
    }
    if mouse.just_released(MouseButton::Right) {
        action_events.send(ActionEvent { action: VirtualAction::Secondary, state: ActionState::JustReleased, cursor_pos, is_over_ui });
    }
    
    if keys.just_pressed(KeyCode::KeyE) {
        action_events.send(ActionEvent { action: VirtualAction::Interact, state: ActionState::JustPressed, cursor_pos, is_over_ui });
    }

    // Tactical Abilities & Jump (Only active when in FPS perspective and not clicking UI)
    if *camera_mode.get() == CameraMode::FPS && !is_over_ui {
        // Jump intent
        if keys.just_pressed(KeyCode::Space) {
            action_events.send(ActionEvent { action: VirtualAction::Jump, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(VirtualAction::Jump, current_time, 0.12);
        }

        // Tactical abilities: Q (Dash), C (Smoke), X (Intel Dart), F (Grav-Lift)
        if keys.just_pressed(KeyCode::KeyQ) {
            let act = VirtualAction::UseAbility(TacticalAbilityKind::PhaseDash);
            action_events.send(ActionEvent { action: act, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(act, current_time, 0.12);
        }
        if keys.just_pressed(KeyCode::KeyC) {
            let act = VirtualAction::UseAbility(TacticalAbilityKind::SmokeVeil);
            action_events.send(ActionEvent { action: act, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(act, current_time, 0.12);
        }
        if keys.just_pressed(KeyCode::KeyX) {
            let act = VirtualAction::UseAbility(TacticalAbilityKind::IntelDart);
            action_events.send(ActionEvent { action: act, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(act, current_time, 0.12);
        }
        if keys.just_pressed(KeyCode::KeyF) {
            let act = VirtualAction::UseAbility(TacticalAbilityKind::GravLift);
            action_events.send(ActionEvent { action: act, state: ActionState::JustPressed, cursor_pos, is_over_ui: false });
            action_buffer.push(act, current_time, 0.12);
        }
    }
}
