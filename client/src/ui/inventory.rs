// ============================================================================
// File: client/src/ui/inventory.rs
// ============================================================================
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use tracing::{info, error};

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
 
 
use crate::module_bindings::player_table::PlayerTableAccess; 
use crate::module_bindings::inventory_table::InventoryTableAccess; 
 
use crate::module_bindings::structure_table::StructureTableAccess; 
use crate::module_bindings::swap_inventory_slots_reducer::swap_inventory_slots;
use crate::module_bindings::drop_inventory_item_reducer::drop_inventory_item;
use crate::module_bindings::equipment_loadout_table::EquipmentLoadoutTableAccess;
use crate::module_bindings::equip_weapon_reducer::equip_weapon;
use crate::module_bindings::unequip_weapon_reducer::unequip_weapon;
use crate::weapons::EquippedHandSide;
use spacetime_rts_logic::{HandSide, create_bag_container};
use spacetimedb_sdk::Table;

use super::types::*;

pub fn ui_node_screen_rect(transform: &GlobalTransform, node: &Node, _window: &Window) -> Rect {
    node.logical_rect(transform)
}

// ----------------------------------------------------------------------------
// INVENTORY DRAG-AND-DROP & WORLD DROP SYSTEMS
// ----------------------------------------------------------------------------

pub fn handle_inventory_drag_and_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    slot_query: Query<(&InventorySlotIndex, &GlobalTransform, &Node, Option<&Interaction>)>,
    main_hand_slot_q: Query<(&GlobalTransform, &Node, Option<&Interaction>), With<PaperdollMainHandSlot>>,
    off_hand_slot_q: Query<(&GlobalTransform, &Node, Option<&Interaction>), With<PaperdollOffHandSlot>>,
    bag_slot_query: Query<(&PaperdollBagSlotIndex, &GlobalTransform, &Node, Option<&Interaction>)>,
    inv_root_query: Query<(&GlobalTransform, &Node), With<InventoryUiRoot>>,
    mut drag_drop: ResMut<DragDropState>,
    conn: Res<SpacetimeConnection>,
    cached_player: Res<CachedPlayerEntity>,
    hand_side: Res<EquippedHandSide>,
    mut equipped_bags: ResMut<ClientEquippedBags>,
) {
    if console.is_open {
        return;
    }

    let Ok(window) = window_query.get_single() else { return; };
    let Some(cursor_pos) = window.cursor_position() else { return; };

    let Some(player_id) = cached_player.0 else { return; };
    let Some(inv) = conn.db.db.inventory().entity_id().find(&player_id) else { return; };

    // Right-Click to Quick-Equip Weapon or Bag
    if mouse.just_pressed(MouseButton::Right) {
        for (slot_idx, transform, node, interaction) in slot_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            let is_hit = rect.contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
            if is_hit {
                if let Some(slot) = inv.slots.get(slot_idx.0) {
                    if slot.count > 0 && !slot.item_type.is_empty() {
                        let item_name = slot.item_type.clone();
                        if let Some(bag_def) = create_bag_container(&item_name) {
                            for i in 0..4 {
                                if equipped_bags.bags[i].is_none() {
                                    info!("Equipped Bag '{}' into Paperdoll Bag Slot {}", item_name, i + 1);
                                    equipped_bags.bags[i] = Some(bag_def);
                                    break;
                                }
                            }
                            let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                            let is_two_handed = ItemKind::from_name(&item_name).map_or_else(
                                || crate::weapons::WeaponType::from_item_name(Some(&item_name)).is_two_handed(),
                                |k| k.is_two_handed(),
                            );
                            let loadout = conn.db.db.equipment_loadout().entity_id().find(&player_id);
                            let main_equipped = loadout.as_ref().map(|l| l.main_hand.as_str()).unwrap_or("None");
                            let off_equipped = loadout.as_ref().map(|l| l.off_hand.as_str()).unwrap_or("None");

                            let target_slot = if is_two_handed {
                                EquipmentSlot::MainHand
                            } else if shift_held || hand_side.0 == HandSide::Left {
                                EquipmentSlot::OffHand
                            } else if main_equipped != "None" && !main_equipped.is_empty() && (off_equipped == "None" || off_equipped.is_empty()) {
                                EquipmentSlot::OffHand
                            } else {
                                EquipmentSlot::MainHand
                            };

                            info!("Quick-Equipping '{}' to {}", item_name, target_slot.as_str());
                            let _ = conn.db.reducers.equip_weapon(target_slot.as_str().to_string(), item_name);
                        }
                        return;
                    }
                }
            }
        }
    }

    if mouse.just_pressed(MouseButton::Left) {
        for (slot_idx, transform, node, interaction) in slot_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            let is_hit = rect.contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
            if is_hit {
                if let Some(slot) = inv.slots.get(slot_idx.0) {
                    if slot.count > 0 && !slot.item_type.is_empty() {
                        drag_drop.is_dragging = true;
                        drag_drop.source_slot = Some(slot_idx.0);
                        drag_drop.item_type = slot.item_type.clone();
                        drag_drop.count = slot.count;
                        drag_drop.current_pos = cursor_pos;
                        break;
                    }
                }
            }
        }
    }

    if mouse.pressed(MouseButton::Left) && drag_drop.is_dragging {
        drag_drop.current_pos = cursor_pos;
    }

    if mouse.just_released(MouseButton::Left) && drag_drop.is_dragging {
        let source = drag_drop.source_slot.unwrap();

        // 1. Check if released on another inventory bag slot
        let mut target_slot = None;
        for (slot_idx, transform, node, interaction) in slot_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            let is_hit = rect.contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
            if is_hit {
                target_slot = Some(slot_idx.0);
                break;
            }
        }

        // 2. Check if released over MainHand Paperdoll slot
        let mut dropped_on_main = false;
        for (transform, node, interaction) in main_hand_slot_q.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            let is_hit = rect.contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
            if is_hit {
                dropped_on_main = true;
                break;
            }
        }

        // 3. Check if released over OffHand Paperdoll slot
        let mut dropped_on_off = false;
        for (transform, node, interaction) in off_hand_slot_q.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            let is_hit = rect.contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
            if is_hit {
                dropped_on_off = true;
                break;
            }
        }

        // 4. Check if released over Paperdoll Bag slot
        let mut dropped_on_bag = None;
        for (bag_slot_idx, transform, node, interaction) in bag_slot_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            let is_hit = rect.contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
            if is_hit {
                dropped_on_bag = Some(bag_slot_idx.0);
                break;
            }
        }

        // 5. Check if released anywhere inside the Inventory window
        let mut inside_inventory_window = false;
        for (transform, node) in inv_root_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            if rect.inflate(10.0).contains(cursor_pos) {
                inside_inventory_window = true;
                break;
            }
        }

        if let Some(target) = target_slot {
            if target != source {
                info!("Swapping inventory slot {} with slot {}", source, target);
                if let Err(e) = conn.db.reducers.swap_inventory_slots(source as u32, target as u32) {
                    error!("Failed to swap slots: {:?}", e);
                }
            }
        } else if dropped_on_main {
            info!("Paperdoll: Dragged item '{}' into {}", drag_drop.item_type, EquipmentSlot::MainHand.as_str());
            if let Err(e) = conn.db.reducers.equip_weapon(EquipmentSlot::MainHand.as_str().to_string(), drag_drop.item_type.clone()) {
                error!("Failed to equip item to MainHand: {:?}", e);
            }
        } else if dropped_on_off {
            info!("Paperdoll: Dragged item '{}' into {}", drag_drop.item_type, EquipmentSlot::OffHand.as_str());
            if let Err(e) = conn.db.reducers.equip_weapon(EquipmentSlot::OffHand.as_str().to_string(), drag_drop.item_type.clone()) {
                error!("Failed to equip item to OffHand: {:?}", e);
            }
        } else if let Some(bag_idx) = dropped_on_bag {
            if let Some(bag_def) = create_bag_container(&drag_drop.item_type) {
                info!("Paperdoll: Dragged Bag '{}' into Bag Slot {}", drag_drop.item_type, bag_idx + 1);
                equipped_bags.bags[bag_idx] = Some(bag_def);
            } else {
                info!("Item '{}' is not a container bag.", drag_drop.item_type);
            }
        } else if inside_inventory_window {
            // Drag was released on an empty part of the inventory window / margins / header / crafting panel.
            // DO NOT drop into the world! Cancel drag safely.
            info!("Drag released inside inventory window - cancelled without world drop.");
        } else {
            // Released outside the inventory window in the 3D game world: drop item into world
            info!("Dropping item '{}' ({}x) into world", drag_drop.item_type, drag_drop.count);
            if let Err(e) = conn.db.reducers.drop_inventory_item(source as u32, drag_drop.count) {
                error!("Failed to drop item into world: {:?}", e);
            }
        }

        drag_drop.is_dragging = false;
        drag_drop.source_slot = None;
        drag_drop.item_type.clear();
        drag_drop.count = 0;
    }
}

pub fn handle_paperdoll_interactions(
    conn: Res<SpacetimeConnection>,
    mut hand_side: ResMut<EquippedHandSide>,
    mut equipped_bags: ResMut<ClientEquippedBags>,
    primary_hand_btn_q: Query<&Interaction, (With<PaperdollPrimaryHandButton>, Changed<Interaction>)>,
    unequip_main_q: Query<&Interaction, (With<PaperdollUnequipMainButton>, Changed<Interaction>)>,
    unequip_off_q: Query<&Interaction, (With<PaperdollUnequipOffButton>, Changed<Interaction>)>,
    bag_slots_q: Query<(&PaperdollBagSlotIndex, &Interaction), Changed<Interaction>>,
) {
    for interaction in primary_hand_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            hand_side.0 = match hand_side.0 {
                HandSide::Right => HandSide::Left,
                HandSide::Left => HandSide::Right,
            };
            info!("Paperdoll: Swapped Primary Hand to {:?}", hand_side.0);
        }
    }

    for interaction in unequip_main_q.iter() {
        if *interaction == Interaction::Pressed {
            info!("Paperdoll: Unequipping {}", EquipmentSlot::MainHand.as_str());
            let _ = conn.db.reducers.unequip_weapon(EquipmentSlot::MainHand.as_str().to_string());
        }
    }

    for interaction in unequip_off_q.iter() {
        if *interaction == Interaction::Pressed {
            info!("Paperdoll: Unequipping {}", EquipmentSlot::OffHand.as_str());
            let _ = conn.db.reducers.unequip_weapon(EquipmentSlot::OffHand.as_str().to_string());
        }
    }

    for (bag_slot, interaction) in bag_slots_q.iter() {
        if *interaction == Interaction::Pressed {
            if let Some(removed) = equipped_bags.bags[bag_slot.0].take() {
                info!("Paperdoll: Unequipped Bag '{}' from Slot {}", removed.name, bag_slot.0 + 1);
            }
        }
    }
}

pub fn update_drag_ghost_ui(
    drag_drop: Res<DragDropState>,
    mut ghost_query: Query<&mut Style, With<DragGhostUi>>,
    mut text_query: Query<&mut Text, With<DragGhostText>>,
) {
    let Ok(mut style) = ghost_query.get_single_mut() else { return; };
    let Ok(mut text) = text_query.get_single_mut() else { return; };

    if drag_drop.is_dragging {
        if style.display != Display::Flex {
            style.display = Display::Flex;
        }
        let l = Val::Px(drag_drop.current_pos.x - 27.5);
        let t = Val::Px(drag_drop.current_pos.y - 27.5);
        if style.left != l { style.left = l; }
        if style.top != t { style.top = t; }
        let new_text = format!("{}\n(x{})", drag_drop.item_type, drag_drop.count);
        if text.sections[0].value != new_text {
            text.sections[0].value = new_text;
        }
    } else if style.display != Display::None {
        style.display = Display::None;
    }
}


pub fn toggle_inventory_ui(
    mut inv_evts: EventReader<crate::input::ToggleInventoryEvent>,
    mut query: Query<&mut Style, With<InventoryUiRoot>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    for _ in inv_evts.read() {
        let Ok(mut window) = window_query.get_single_mut() else { return; };
        
        for mut style in query.iter_mut() {
            if style.display == Display::None {
                style.display = Display::Flex;
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
            } else {
                style.display = Display::None;
                if *camera_mode.get() == CameraMode::FPS {
                    window.cursor.grab_mode = CursorGrabMode::Locked;
                    window.cursor.visible = false;
                }
            }
        }
    }
}

pub fn update_inventory_ui(
    conn: Res<SpacetimeConnection>,
    player_query: Query<&Transform, With<PlayerBody>>,
    hand_side: Res<EquippedHandSide>,
    equipped_bags: Res<ClientEquippedBags>,
    inventory_root_q: Query<&Style, With<InventoryUiRoot>>,
    mut header_q: Query<&mut Text, (With<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
    mut name_q: Query<(&mut Text, &InventorySlotName), Without<InventorySlotCount>>,
    mut count_q: Query<(&mut Text, &InventorySlotCount), Without<InventorySlotName>>,
    mut primary_hand_text_q: Query<&mut Text, (With<PaperdollPrimaryHandText>, Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
    mut main_hand_text_q: Query<&mut Text, (With<PaperdollMainHandText>, Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
    mut off_hand_text_q: Query<&mut Text, (With<PaperdollOffHandText>, Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
    mut bag_text_q: Query<(&mut Text, &PaperdollBagText), (Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagTooltip>)>,
    mut bag_tooltip_q: Query<(&mut Text, &PaperdollBagTooltip), (Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>)>,
    mut capacity_header_q: Query<&mut Text, (With<InventoryCapacityHeader>, Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
    mut workbench_recipe_styles: Query<&mut Style, With<RequiresWorkbenchRecipe>>,
) {
    // 1. Performance Guard: Zero overhead when the Inventory modal is closed
    if let Ok(root_style) = inventory_root_q.get_single() {
        if root_style.display == Display::None {
            return;
        }
    }

    let Some(identity) = &conn.identity else { return; };
    if let Some(player) = conn.db.db.player().identity().find(identity) {
        if let Some(inventory) = conn.db.db.inventory().entity_id().find(&player.entity_id) {
            
            for (mut text, name) in name_q.iter_mut() {
                let new_val = if let Some(slot) = inventory.slots.get(name.0) {
                    if slot.count > 0 { slot.item_type.as_str() } else { "" }
                } else {
                    ""
                };
                if text.sections[0].value != new_val {
                    text.sections[0].value = new_val.to_string();
                }
            }
            
            for (mut text, count) in count_q.iter_mut() {
                let new_val = if let Some(slot) = inventory.slots.get(count.0) {
                    if slot.count > 1 { slot.count.to_string() } else { String::new() }
                } else {
                    String::new()
                };
                if text.sections[0].value != new_val {
                    text.sections[0].value = new_val;
                }
            }

            // Sync Paperdoll Equipment Loadout
            let loadout = conn.db.db.equipment_loadout().entity_id().find(&player.entity_id);
            let main_weapon = loadout.as_ref().map(|l| l.main_hand.as_str()).unwrap_or("None");
            let off_weapon = loadout.as_ref().map(|l| l.off_hand.as_str()).unwrap_or("None");

            for mut text in main_hand_text_q.iter_mut() {
                let new_val = if main_weapon == "None" || main_weapon.is_empty() {
                    "MainHand: [Unarmed]"
                } else {
                    main_weapon
                };
                let formatted = if new_val == "MainHand: [Unarmed]" {
                    new_val.to_string()
                } else {
                    format!("MainHand: {}", new_val)
                };
                if text.sections[0].value != formatted {
                    text.sections[0].value = formatted;
                }
            }

            for mut text in off_hand_text_q.iter_mut() {
                let new_val = if off_weapon == "None" || off_weapon.is_empty() {
                    "OffHand: [Empty]"
                } else {
                    off_weapon
                };
                let formatted = if new_val == "OffHand: [Empty]" {
                    new_val.to_string()
                } else {
                    format!("OffHand: {}", new_val)
                };
                if text.sections[0].value != formatted {
                    text.sections[0].value = formatted;
                }
            }

            for mut text in primary_hand_text_q.iter_mut() {
                let new_val = match hand_side.0 {
                    HandSide::Right => "[⇄] PRIMARY HAND: RIGHT [H]",
                    HandSide::Left => "[⇄] PRIMARY HAND: LEFT [H]",
                };
                if text.sections[0].value != new_val {
                    text.sections[0].value = new_val.to_string();
                }
            }

            // Sync Bags & Capacity
            for (mut text, b_idx) in bag_text_q.iter_mut() {
                let new_val = if let Some(ref bag) = equipped_bags.bags[b_idx.0] {
                    format!("Bag {}: {}", b_idx.0 + 1, bag.name)
                } else {
                    format!("Bag {}: [Empty Bag Slot]", b_idx.0 + 1)
                };
                if text.sections[0].value != new_val {
                    text.sections[0].value = new_val;
                }
            }

            for (mut text, b_idx) in bag_tooltip_q.iter_mut() {
                let (new_val, new_col) = if let Some(ref bag) = equipped_bags.bags[b_idx.0] {
                    (
                        format!("+{} Slots | Cap: {:?} | {}% WR", bag.capacity, bag.size_cap, bag.weight_reduction_pct),
                        Color::srgb(0.4, 0.9, 0.5)
                    )
                } else {
                    (
                        "Right-click bag in inventory to equip".to_string(),
                        Color::srgb(0.65, 0.70, 0.65)
                    )
                };
                if text.sections[0].value != new_val {
                    text.sections[0].value = new_val;
                }
                if text.sections[0].style.color != new_col {
                    text.sections[0].style.color = new_col;
                }
            }

            let extra_slots: usize = equipped_bags.bags.iter().filter_map(|b| b.as_ref()).map(|b| b.capacity).sum();
            let total_cap = 16 + extra_slots;
            let used_slots = inventory.slots.iter().filter(|s| s.count > 0 && !s.item_type.is_empty()).count();
            let free_slots = total_cap.saturating_sub(used_slots);

            for mut text in capacity_header_q.iter_mut() {
                let new_val = format!("BAG INVENTORY (Free: {} / {} Slots)", free_slots, total_cap);
                if text.sections[0].value != new_val {
                    text.sections[0].value = new_val;
                }
            }
        }
    }

    let near_workbench = if let Ok(player_t) = player_query.get_single() {
        conn.db.db.structure().iter().any(|s| {
            if s.piece_type == "Workbench" && !s.is_blueprint {
                let dist_sq = (s.x - player_t.translation.x).powi(2) + (s.z - player_t.translation.z).powi(2);
                dist_sq <= 400.0
            } else {
                false
            }
        })
    } else {
        false
    };

    for mut header in header_q.iter_mut() {
        let (new_text, new_col) = if near_workbench {
            ("CRAFTING RECIPES [WORKBENCH ACTIVE]", Color::srgb(1.0, 0.85, 0.2))
        } else {
            ("FIELD CRAFTING [HAND CRAFTING]", Color::srgb(0.7, 0.7, 0.7))
        };
        if header.sections[0].value != new_text {
            header.sections[0].value = new_text.to_string();
        }
        if header.sections[0].style.color != new_col {
            header.sections[0].style.color = new_col;
        }
    }

    let desired_display = if near_workbench { Display::Flex } else { Display::None };
    for mut style in workbench_recipe_styles.iter_mut() {
        if style.display != desired_display {
            style.display = desired_display;
        }
    }
}

