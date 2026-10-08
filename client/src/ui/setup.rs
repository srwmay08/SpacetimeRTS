// ============================================================================
// File: client/src/ui/setup.rs
// ============================================================================
use bevy::prelude::*;

use crate::components::*;
use crate::building::ModularPieceType; 
use crate::templates::BuildingTemplateType; 
 
 
 
 

use super::types::*;

pub fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2dBundle {
        camera: Camera {
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        ..default()
    });

    // ------------------------------------------------------------------------
    // 1. TOP-LEFT HOTBAR REMOVED (Consolidated into central bottom hotbar)
    // ------------------------------------------------------------------------

    // ------------------------------------------------------------------------
    // 2. BOTTOM-LEFT HUD HEALTH BAR
    // ------------------------------------------------------------------------
    commands.spawn(NodeBundle {
        style: Style {
            position_type: PositionType::Absolute,
            bottom: Val::Px(25.0),
            left: Val::Px(25.0),
            width: Val::Px(200.0),
            height: Val::Px(22.0),
            border: UiRect::all(Val::Px(2.0)),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        },
        border_color: Color::srgb(0.1, 0.1, 0.1).into(),
        background_color: Color::srgba(0.15, 0.05, 0.05, 0.85).into(),
        z_index: ZIndex::Global(10),
        ..default()
    }).with_children(|bar_frame| {
        bar_frame.spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                background_color: Color::srgb(0.8, 0.15, 0.15).into(),
                ..default()
            },
            HealthBarFill,
        ));

        bar_frame.spawn((
            TextBundle::from_section(
                "100 / 100",
                TextStyle { font_size: 13.0, color: Color::WHITE, ..default() }
            ).with_style(Style {
                position_type: PositionType::Absolute,
                left: Val::Px(65.0),
                ..default()
            }),
            HealthBarText,
        ));
    });

    // ------------------------------------------------------------------------
    // CELESTIAL CLOCK & ATMOSPHERIC WEATHER HUD PILL (Top-Right)
    // ------------------------------------------------------------------------
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(15.0),
                right: Val::Px(15.0),
                padding: UiRect::new(Val::Px(14.0), Val::Px(14.0), Val::Px(6.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.5)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            background_color: Color::srgba(0.08, 0.08, 0.12, 0.85).into(),
            border_color: Color::srgba(0.40, 0.50, 0.70, 0.60).into(),
            z_index: ZIndex::Global(10),
            ..default()
        },
        CelestialHudRoot,
    )).with_children(|pill| {
        pill.spawn((
            TextBundle::from_section(
                "12:00 (High Noon) | Clear Sky [F8/F9]",
                TextStyle {
                    font_size: 13.0,
                    color: Color::srgb(0.92, 0.95, 1.0),
                    ..default()
                },
            ),
            CelestialHudText,
        ));
    });

    // ------------------------------------------------------------------------
    // BEVY F3 TELEMETRY & SYSTEM DIAGNOSTICS OVERLAY (Top-Right, below Celestial Pill)
    // ------------------------------------------------------------------------
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(52.0),
                right: Val::Px(15.0),
                padding: UiRect::new(Val::Px(14.0), Val::Px(14.0), Val::Px(8.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.5)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(3.0),
                min_width: Val::Px(150.0),
                display: Display::None,
                ..default()
            },
            background_color: Color::srgba(0.08, 0.08, 0.12, 0.85).into(),
            border_color: Color::srgba(0.40, 0.50, 0.70, 0.60).into(),
            z_index: ZIndex::Global(15),
            ..default()
        },
        DiagnosticOverlayRoot,
    )).with_children(|box_node| {
        box_node.spawn((
            TextBundle::from_sections([
                TextSection::new(
                    "DIAGNOSTICS [F3]\n",
                    TextStyle {
                        font_size: 11.0,
                        color: Color::srgb(0.50, 0.75, 1.0),
                        ..default()
                    },
                ),
                TextSection::new(
                    "Collecting telemetry...",
                    TextStyle {
                        font_size: 13.0,
                        color: Color::srgb(0.92, 0.95, 1.0),
                        ..default()
                    },
                ),
            ]),
            DiagnosticOverlayText,
        ));
    });

    // ------------------------------------------------------------------------
    // 3. INVENTORY & PERSONAL / WORKBENCH CRAFTING EXPANSION
    // ------------------------------------------------------------------------
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(76.0),
                left: Val::Px(15.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                align_items: AlignItems::FlexStart,
                display: Display::None,
                ..default()
            },
            z_index: ZIndex::Global(50),
            ..default()
        }, 
        InventoryUiRoot
    )).with_children(|root| {
        // --------------------------------------------------------------------
        // COLUMN 1: WOW / EVERQUEST EQUIPMENT LOADOUT & PAPERDOLL
        // --------------------------------------------------------------------
        root.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                width: Val::Px(270.0),
                ..default()
            },
            background_color: Color::srgba(0.08, 0.08, 0.08, 0.95).into(),
            border_color: Color::srgb(0.65, 0.50, 0.20).into(),
            ..default()
        }).with_children(|paperdoll| {
            paperdoll.spawn(TextBundle::from_section(
                "EQUIPMENT & PAPERDOLL",
                TextStyle { font_size: 13.0, color: Color::srgb(0.95, 0.82, 0.3), ..default() }
            ));

            // Primary Hand Swap Toggle Button
            paperdoll.spawn((
                ButtonBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(28.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                    background_color: Color::srgba(0.18, 0.18, 0.22, 0.9).into(),
                    border_color: Color::srgb(0.5, 0.5, 0.7).into(),
                    ..default()
                },
                PaperdollPrimaryHandButton,
            )).with_children(|btn| {
                btn.spawn((
                    TextBundle::from_section(
                        "[⇄] PRIMARY HAND: RIGHT [H]",
                        TextStyle { font_size: 11.0, color: Color::srgb(0.8, 0.9, 1.0), ..default() }
                    ),
                    PaperdollPrimaryHandText,
                ));
            });

            // MainHand Slot
            paperdoll.spawn((
                NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        width: Val::Percent(100.0),
                        height: Val::Px(32.0),
                        padding: UiRect::horizontal(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    background_color: Color::srgba(0.12, 0.12, 0.14, 0.9).into(),
                    border_color: Color::srgb(0.4, 0.4, 0.45).into(),
                    ..default()
                },
                Interaction::default(),
                PaperdollMainHandSlot,
            )).with_children(|row| {
                row.spawn((
                    TextBundle::from_section(
                        "MainHand: [Unarmed]",
                        TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }
                    ),
                    PaperdollMainHandText,
                ));
                row.spawn((
                    ButtonBundle {
                        style: Style {
                            padding: UiRect::all(Val::Px(2.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        background_color: Color::srgba(0.25, 0.1, 0.1, 0.8).into(),
                        border_color: Color::srgb(0.6, 0.2, 0.2).into(),
                        ..default()
                    },
                    PaperdollUnequipMainButton,
                )).with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        "Unequip",
                        TextStyle { font_size: 10.0, color: Color::srgb(1.0, 0.6, 0.6), ..default() }
                    ));
                });
            });

            // OffHand Slot
            paperdoll.spawn((
                NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        width: Val::Percent(100.0),
                        height: Val::Px(32.0),
                        padding: UiRect::horizontal(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    background_color: Color::srgba(0.12, 0.12, 0.14, 0.9).into(),
                    border_color: Color::srgb(0.4, 0.4, 0.45).into(),
                    ..default()
                },
                Interaction::default(),
                PaperdollOffHandSlot,
            )).with_children(|row| {
                row.spawn((
                    TextBundle::from_section(
                        "OffHand: [Empty]",
                        TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }
                    ),
                    PaperdollOffHandText,
                ));
                row.spawn((
                    ButtonBundle {
                        style: Style {
                            padding: UiRect::all(Val::Px(2.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        background_color: Color::srgba(0.25, 0.1, 0.1, 0.8).into(),
                        border_color: Color::srgb(0.6, 0.2, 0.2).into(),
                        ..default()
                    },
                    PaperdollUnequipOffButton,
                )).with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        "Unequip",
                        TextStyle { font_size: 10.0, color: Color::srgb(1.0, 0.6, 0.6), ..default() }
                    ));
                });
            });

            // 4 Container Bag Slots (EQ / WoW Container Paradigm)
            paperdoll.spawn(TextBundle::from_section(
                "CONTAINER BAGS (EQ / WoW)",
                TextStyle { font_size: 12.0, color: Color::srgb(0.85, 0.75, 0.35), ..default() }
            ).with_style(Style { margin: UiRect::top(Val::Px(6.0)), ..default() }));

            for bag_idx in 0..4 {
                paperdoll.spawn((
                    ButtonBundle {
                        style: Style {
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::FlexStart,
                            width: Val::Percent(100.0),
                            height: Val::Px(34.0),
                            padding: UiRect::all(Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        background_color: Color::srgba(0.10, 0.11, 0.12, 0.9).into(),
                        border_color: Color::srgb(0.35, 0.40, 0.35).into(),
                        ..default()
                    },
                    PaperdollBagSlotIndex(bag_idx),
                )).with_children(|b_slot| {
                    b_slot.spawn((
                        TextBundle::from_section(
                            format!("Bag {}: [Empty Bag Slot]", bag_idx + 1),
                            TextStyle { font_size: 10.5, color: Color::WHITE, ..default() }
                        ),
                        PaperdollBagText(bag_idx),
                    ));
                    b_slot.spawn((
                        TextBundle::from_section(
                            "Right-click bag in inventory to equip",
                            TextStyle { font_size: 9.0, color: Color::srgb(0.65, 0.70, 0.65), ..default() }
                        ),
                        PaperdollBagTooltip(bag_idx),
                    ));
                });
            }
        });

        // --------------------------------------------------------------------
        // COLUMN 2: STATIC INVENTORY GRID (WOW / EQ BAG STORAGE)
        // --------------------------------------------------------------------
        root.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            background_color: Color::srgba(0.06, 0.06, 0.07, 0.95).into(),
            border_color: Color::srgb(0.35, 0.35, 0.40).into(),
            ..default()
        }).with_children(|pack_col| {
            pack_col.spawn((
                TextBundle::from_section(
                    "BAG INVENTORY (Free: 16 / 16 Slots)",
                    TextStyle { font_size: 13.0, color: Color::srgb(0.9, 0.85, 0.5), ..default() }
                ).with_style(Style { margin: UiRect::bottom(Val::Px(6.0)), ..default() }),
                InventoryCapacityHeader,
            ));

            // Grid of 4 rows of 8 slots = 32 slots total
            for row in 0..4 {
                pack_col.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(5.0),
                        ..default()
                    },
                    ..default()
                }).with_children(|row_ui| {
                    for col in 0..8 {
                        let slot_idx = row * 8 + col;
                        row_ui.spawn((
                            NodeBundle {
                                style: Style {
                                    width: Val::Px(50.0),
                                    height: Val::Px(50.0),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::SpaceBetween,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::all(Val::Px(2.0)),
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                border_color: Color::srgba(0.32, 0.32, 0.32, 0.8).into(),
                                background_color: Color::srgba(0.09, 0.09, 0.09, 0.92).into(),
                                ..default()
                            },
                            Interaction::default(),
                            InventorySlotIndex(slot_idx),
                        )).with_children(|slot| {
                            slot.spawn((
                                TextBundle::from_section(
                                    "",
                                    TextStyle { font_size: 10.0, color: Color::WHITE, ..default() }
                                ).with_style(Style { margin: UiRect::top(Val::Px(4.0)), ..default() }),
                                InventorySlotName(slot_idx),
                            ));
                            slot.spawn((
                                TextBundle::from_section(
                                    "",
                                    TextStyle { font_size: 11.0, color: Color::srgb(0.9, 0.9, 0.9), ..default() }
                                ).with_style(Style { align_self: AlignSelf::FlexEnd, ..default() }),
                                InventorySlotCount(slot_idx),
                            ));
                        });
                    }
                });
            }
        });

        root.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                width: Val::Px(300.0),
                ..default()
            },
            background_color: Color::srgba(0.08, 0.08, 0.08, 0.95).into(),
            border_color: Color::srgb(0.4, 0.3, 0.15).into(),
            ..default()
        }).with_children(|crafting| {
            crafting.spawn((
                TextBundle::from_section(
                    "CRAFTING CATALOG",
                    TextStyle { font_size: 14.0, color: Color::srgb(0.9, 0.8, 0.3), ..default() }
                ),
                WorkbenchHeaderStatus,
            ));

            let recipes = [
                ("Hammer", "1 Branch, 1 Stone"),
                ("Stone Axe", "1 Branch, 1 Flint"),
                ("Pickaxe", "2 Branch, 2 Flint"),
                ("Club", "2 Branch"),
                ("Torch", "1 Branch, 1 Resin"),
                ("Wood Arrow", "8 Wood (x20)"),
                ("Crude Bow", "10 Wood, 4 Leather [Workbench]"),
                ("Bow", "12 Wood, 4 Leather [Workbench]"),
                ("Crossbow", "15 Wood, 5 Flint, 4 Leather [Workbench]"),
                ("Hand Crossbow", "8 Wood, 3 Flint, 2 Leather [Workbench]"),
                ("Crossbow Bolt", "6 Wood, 3 Flint (x15) [Workbench]"),
                ("Flint Arrow", "8 Wood, 2 Flint [Workbench]"),
                ("Flint Spear", "5 Wood, 2 Flint [Workbench]"),
                ("Wooden Shield", "10 Wood, 2 Leather [Workbench]"),
            ];

            for (item_name, cost) in recipes {
                let is_field_craft = item_name == "Torch" || item_name == "Club" || item_name == "Hammer";
                let mut btn_entity = crafting.spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Px(36.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::FlexStart,
                            padding: UiRect::horizontal(Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            display: if is_field_craft { Display::Flex } else { Display::None },
                            ..default()
                        },
                        border_color: Color::srgb(0.3, 0.3, 0.3).into(),
                        background_color: Color::srgb(0.16, 0.16, 0.16).into(),
                        ..default()
                    },
                    CraftRecipeButton(item_name.to_string()),
                ));
                if !is_field_craft {
                    btn_entity.insert(RequiresWorkbenchRecipe);
                }
                btn_entity.with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        item_name,
                        TextStyle { font_size: 12.0, color: Color::WHITE, ..default() }
                    ));
                    btn.spawn(TextBundle::from_section(
                        cost,
                        TextStyle { font_size: 10.0, color: Color::srgb(0.65, 0.65, 0.4), ..default() }
                    ));
                });
            }
        });
    });

    // ------------------------------------------------------------------------
    // 4. FLOATING DRAG-AND-DROP GHOST ELEMENT
    // ------------------------------------------------------------------------
    commands.spawn((
        NodeBundle {
            focus_policy: bevy::ui::FocusPolicy::Pass,
            style: Style {
                position_type: PositionType::Absolute,
                width: Val::Px(55.0),
                height: Val::Px(55.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            border_color: Color::srgba(1.0, 0.85, 0.2, 0.95).into(),
            background_color: Color::srgba(0.18, 0.18, 0.22, 0.85).into(),
            z_index: ZIndex::Global(999),
            ..default()
        },
        DragGhostUi,
    )).with_children(|ghost| {
        ghost.spawn((
            TextBundle::from_section(
                "",
                TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }
            ),
            DragGhostText,
        ));
    });

    // ------------------------------------------------------------------------
    // 5. DARK TRANSPARENT DEVELOPER ADMIN CONSOLE OVERLAY WITH AUTOCOMPLETE
    // ------------------------------------------------------------------------
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(350.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::bottom(Val::Px(3.0)),
                display: Display::None,
                ..default()
            },
            border_color: Color::srgba(0.2, 0.65, 0.95, 0.95).into(),
            background_color: Color::srgba(0.04, 0.04, 0.07, 0.90).into(),
            z_index: ZIndex::Global(200),
            ..default()
        },
        ConsoleRoot,
    )).with_children(|console_root| {
        console_root.spawn(TextBundle::from_section(
            "DEVELOPER CONSOLE [Press ` to toggle | Tab to Auto-Fill] | giveitem <Item> | star <a|b> <lux> | dualshadows | sunset | help",
            TextStyle { font_size: 11.0, color: Color::srgb(0.3, 0.8, 0.95), ..default() }
        ));

        console_root.spawn((
            TextBundle::from_section(
                "",
                TextStyle { font_size: 12.0, color: Color::srgb(0.85, 0.85, 0.85), ..default() }
            ).with_style(Style {
                height: Val::Px(225.0),
                margin: UiRect::vertical(Val::Px(4.0)),
                overflow: Overflow::clip_y(),
                ..default()
            }),
            ConsoleLogText,
        ));

        // Real-time suggestions & auto-fill bar
        console_root.spawn((
            TextBundle::from_section(
                "[Tab to auto-fill]: Type 'giveitem ' to inspect available items...",
                TextStyle { font_size: 11.0, color: Color::srgb(0.65, 0.85, 0.65), ..default() }
            ).with_style(Style {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            }),
            ConsoleSuggestionsText,
        ));

        console_root.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            background_color: Color::srgba(0.02, 0.02, 0.04, 0.95).into(),
            border_color: Color::srgb(0.3, 0.5, 0.7).into(),
            ..default()
        }).with_children(|input_bar| {
            input_bar.spawn(TextBundle::from_section(
                "> ",
                TextStyle { font_size: 15.0, color: Color::srgb(1.0, 0.85, 0.2), ..default() }
            ));
            input_bar.spawn((
                TextBundle::from_section(
                    "",
                    TextStyle { font_size: 14.0, color: Color::WHITE, ..default() }
                ),
                ConsoleInputText,
            ));
        });
    });

    // ------------------------------------------------------------------------
    // 6. VISUAL BUILDING MENU (Hammer Right-Click)
    // ------------------------------------------------------------------------
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            z_index: ZIndex::Global(60),
            ..default()
        },
        BuildMenuRoot,
    )).with_children(|overlay| {
        overlay.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(12.0),
                border: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            border_color: Color::srgb(0.4, 0.3, 0.15).into(),
            background_color: Color::srgba(0.08, 0.08, 0.08, 0.95).into(),
            ..default()
        }).with_children(|menu_box| {
            menu_box.spawn(TextBundle::from_section(
                "BUILDING CATALOG [HAMMER BLUEPRINTS]",
                TextStyle { font_size: 18.0, color: Color::srgb(0.9, 0.8, 0.4), ..default() }
            ));

            menu_box.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(10.0),
                    max_width: Val::Px(450.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ..default()
            }).with_children(|grid| {
                let pieces = [
                    (ModularPieceType::Foundation, "Foundation", "20 Wood"),
                    (ModularPieceType::Workbench, "Workbench", "8 Wood"),
                    (ModularPieceType::Campfire, "Campfire", "4 Wood, 4 Stone"),
                    (ModularPieceType::Wall, "Wall", "8 Wood"),
                    (ModularPieceType::Window, "Window", "8 Wood"),
                    (ModularPieceType::Door, "Door", "12 Wood"),
                    (ModularPieceType::Floor, "Floor", "12 Wood"),
                    (ModularPieceType::Roof, "Roof", "12 Wood"),
                    (ModularPieceType::Ramp, "Ramp", "16 Wood"),
                ];

                for (piece_type, name, cost) in pieces {
                    grid.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(95.0),
                                height: Val::Px(80.0),
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            border_color: Color::srgb(0.3, 0.3, 0.3).into(),
                            background_color: Color::srgb(0.18, 0.18, 0.18).into(),
                            ..default()
                        },
                        BuildPieceButton(piece_type),
                    )).with_children(|btn| {
                        btn.spawn(TextBundle::from_section(
                            name,
                            TextStyle { font_size: 13.0, color: Color::WHITE, ..default() }
                        ));
                        btn.spawn(TextBundle::from_section(
                            cost,
                            TextStyle { font_size: 10.0, color: Color::srgb(0.7, 0.7, 0.4), ..default() }
                        ));
                    });
                }
            });

            menu_box.spawn(TextBundle::from_section(
                "MULTI-STORY TEMPLATES [BLUEPRINT STAMP]",
                TextStyle { font_size: 16.0, color: Color::srgb(0.9, 0.8, 0.4), ..default() }
            ));

            menu_box.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(10.0),
                    max_width: Val::Px(450.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ..default()
            }).with_children(|tmpl_grid| {
                let templates = [
                    (BuildingTemplateType::Watchtower, "Watchtower", "120 Wood"),
                    (BuildingTemplateType::Palisade, "Palisade Gate", "90 Wood"),
                    (BuildingTemplateType::Cottage, "Cottage", "70 Wood"),
                    (BuildingTemplateType::Settlement, "Settlement", "260 Wood"),
                ];

                for (tmpl_type, name, cost) in templates {
                    tmpl_grid.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(95.0),
                                height: Val::Px(80.0),
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            border_color: Color::srgb(0.4, 0.35, 0.2).into(),
                            background_color: Color::srgb(0.22, 0.18, 0.14).into(),
                            ..default()
                        },
                        BuildTemplateButton(tmpl_type),
                    )).with_children(|btn| {
                        btn.spawn(TextBundle::from_section(
                            name,
                            TextStyle { font_size: 12.0, color: Color::srgb(1.0, 0.95, 0.8), ..default() }
                        ));
                        btn.spawn(TextBundle::from_section(
                            cost,
                            TextStyle { font_size: 10.0, color: Color::srgb(0.9, 0.75, 0.4), ..default() }
                        ));
                    });
                }
            });
        });
    });

    commands.spawn((
        TextBundle::from_section(
            "",
            TextStyle { font_size: 16.0, color: Color::srgb(1.0, 1.0, 1.0), ..default() }
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Percent(55.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-60.0)),
            ..default()
        }),
        InteractionPromptText,
    ));

    commands.spawn((NodeBundle {
        style: Style {
            position_type: PositionType::Absolute,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        background_color: Color::srgba(0.2, 0.8, 0.2, 0.2).into(),
        border_color: Color::srgb(0.2, 0.8, 0.2).into(),
        visibility: Visibility::Hidden,
        ..default()
    }, MarqueeUI));

    commands.spawn((
        TextBundle::from_section(
            "",
            TextStyle { font_size: 18.0, color: Color::srgb(0.1, 0.8, 1.0), ..default() }
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(138.0),
            left: Val::Px(15.0),
            ..default()
        }),
        BuildUIText,
    ));

    commands.spawn((
        TextBundle::from_section(
            "",
            TextStyle { font_size: 15.0, color: Color::srgb(1.0, 0.88, 0.4), ..default() }
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            bottom: Val::Px(115.0),
            right: Val::Px(30.0),
            ..default()
        }),
        crate::weapons::WeaponHudText,
    ));

    // ------------------------------------------------------------------------
    // 6. RETICLE-ADJACENT FIGHTING HUD & DYNAMIC CROSSHAIR ROOT
    // ------------------------------------------------------------------------
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            z_index: ZIndex::Global(110),
            ..default()
        },
        ReticleHudRoot,
    )).with_children(|reticle| {
        // Center Dot
        reticle.spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Px(2.0),
                    height: Val::Px(2.0),
                    left: Val::Px(-1.0),
                    top: Val::Px(-1.0),
                    display: Display::None,
                    ..default()
                },
                background_color: Color::srgb(0.0, 1.0, 1.0).into(),
                ..default()
            },
            ReticleCrosshairDot,
        ));

        // 4 Crosshair Arms (Top, Bottom, Left, Right)
        let arm_configs = [
            (CrosshairArmDir::Top, Val::Px(-1.0), Val::Px(-12.0), Val::Px(2.0), Val::Px(8.0)),
            (CrosshairArmDir::Bottom, Val::Px(-1.0), Val::Px(4.0), Val::Px(2.0), Val::Px(8.0)),
            (CrosshairArmDir::Left, Val::Px(-12.0), Val::Px(-1.0), Val::Px(8.0), Val::Px(2.0)),
            (CrosshairArmDir::Right, Val::Px(4.0), Val::Px(-1.0), Val::Px(8.0), Val::Px(2.0)),
        ];

        for (dir, left, top, width, height) in arm_configs {
            reticle.spawn((
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        left,
                        top,
                        width,
                        height,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    border_color: Color::BLACK.into(),
                    background_color: Color::srgb(0.0, 1.0, 1.0).into(),
                    ..default()
                },
                ReticleCrosshairArm(dir),
            ));
        }

        // Minimalist Reticle Hitmarker (4-tick X-flasher)
        reticle.spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Px(16.0),
                    height: Val::Px(16.0),
                    left: Val::Px(-8.0),
                    top: Val::Px(-8.0),
                    display: Display::None,
                    ..default()
                },
                ..default()
            },
            ReticleHitMarker,
        )).with_children(|hm| {
            let ticks = [
                (Val::Px(0.0), Val::Px(0.0)),
                (Val::Px(12.0), Val::Px(0.0)),
                (Val::Px(0.0), Val::Px(12.0)),
                (Val::Px(12.0), Val::Px(12.0)),
            ];
            for (l, t) in ticks {
                hm.spawn((
                    NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            left: l,
                            top: t,
                            width: Val::Px(4.0),
                            height: Val::Px(4.0),
                            ..default()
                        },
                        background_color: Color::WHITE.into(),
                        ..default()
                    },
                    ReticleHitMarkerTick,
                ));
            }
        });


        // Bow Draw / Charge Bar (Below Crosshair)
        reticle.spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-20.0),
                    top: Val::Px(22.0),
                    width: Val::Px(40.0),
                    height: Val::Px(3.0),
                    display: Display::None,
                    ..default()
                },
                background_color: Color::srgb(0.1, 0.1, 0.1).into(),
                ..default()
            },
            ReticleBowChargeBar,
        ));

        // Reticle-Adjacent Low-Profile Critical Health Alert (Directly below crosshair)
        reticle.spawn((
            TextBundle::from_section(
                "",
                TextStyle {
                    font_size: 11.0,
                    color: Color::srgb(1.0, 0.2, 0.2),
                    ..default()
                }
            ).with_style(Style {
                position_type: PositionType::Absolute,
                left: Val::Px(-50.0),
                top: Val::Px(16.0),
                width: Val::Px(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            }),
            ReticleCriticalHealthAlert,
        ));
    });

    // ------------------------------------------------------------------------
    // 7. INTERACTIVE CROSSHAIR CUSTOMIZER GUI [F7]
    // ------------------------------------------------------------------------
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                top: Val::Px(60.0),
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                row_gap: Val::Px(6.0),
                ..default()
            },
            background_color: Color::srgba(0.05, 0.08, 0.12, 0.95).into(),
            border_color: Color::srgb(0.0, 0.8, 1.0).into(),
            z_index: ZIndex::Global(150),
            ..default()
        },
        CrosshairMenuRoot,
    )).with_children(|menu| {
        menu.spawn(TextBundle::from_section(
            "CROSSHAIR TUNER [F7]",
            TextStyle { font_size: 16.0, color: Color::srgb(0.0, 1.0, 1.0), ..default() }
        ));

        menu.spawn((
            TextBundle::from_section(
                "Loading settings...",
                TextStyle { font_size: 11.0, color: Color::srgb(0.85, 0.9, 0.95), ..default() }
            ),
            CrosshairMenuText,
        ));

        let actions = [
            ("Cycle Color", "Color"),
            ("Gap -", "GapDec"),
            ("Gap +", "GapInc"),
            ("Length -", "LenDec"),
            ("Length +", "LenInc"),
            ("Thickness -", "ThickDec"),
            ("Thickness +", "ThickInc"),
            ("Toggle Center Dot", "ToggleDot"),
            ("Toggle Outline", "ToggleOutline"),
            ("Toggle Dynamic / Static", "ToggleDynamic"),
        ];

        for (label, action_id) in actions {
            menu.spawn((
                ButtonBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        height: Val::Px(24.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    border_color: Color::srgb(0.15, 0.45, 0.6).into(),
                    background_color: Color::srgb(0.08, 0.18, 0.28).into(),
                    ..default()
                },
                CrosshairMenuButton(action_id.to_string()),
            )).with_children(|btn| {
                btn.spawn(TextBundle::from_section(
                    label,
                    TextStyle { font_size: 11.0, color: Color::srgb(0.9, 0.95, 1.0), ..default() }
                ));
            });
        }
    });

    commands.spawn((NodeBundle {
        style: Style {
            width: Val::Percent(100.0), 
            height: Val::Px(105.0),
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.0),
            left: Val::Px(0.0),
            justify_content: JustifyContent::Center, 
            align_items: AlignItems::Center,
            display: Display::None, 
            ..default()
        },
        z_index: ZIndex::Global(50), 
        ..default()
    }, ActionBarUiRoot)).with_children(|root| {
        root.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row, 
                column_gap: Val::Px(5.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            background_color: Color::srgba(0.05, 0.05, 0.05, 0.9).into(), 
            border_color: Color::srgb(0.4, 0.3, 0.1).into(), 
            ..default()
        }).with_children(|bar| {
            let mut slots: Vec<Option<(&str, &str)>> = vec![
                Some(("Spawn Worker", "20 Wood")),
                Some(("Chop Wood", "Auto-Harvest")),
                Some(("Mine Stone", "Auto-Harvest")),
                Some(("Forage Berries", "Auto-Harvest")),
                Some(("Collect All", "Auto-Harvest")),
                Some(("Stop", "Halt All")),
            ];
            while slots.len() < 10 { slots.push(None); } 
            
            for action_opt in slots {
                let (action, desc) = action_opt.unwrap_or(("", ""));
                bar.spawn((
                    ButtonBundle {
                        style: Style { 
                            width: Val::Px(80.0), 
                            height: Val::Px(80.0), 
                            flex_direction: FlexDirection::Column, 
                            justify_content: JustifyContent::Center, 
                            align_items: AlignItems::Center, 
                            border: UiRect::all(Val::Px(1.0)), 
                            ..default() 
                        },
                        border_color: Color::srgb(0.2, 0.2, 0.2).into(),
                        background_color: Color::srgb(0.15, 0.15, 0.15).into(),
                        ..default()
                    },
                    ActionBarButton(action.to_string())
                )).with_children(|btn| {
                    btn.spawn(TextBundle::from_section(action.to_string(), TextStyle { font_size: 14.0, color: Color::srgb(0.8, 0.8, 0.8), ..default() }));
                    btn.spawn(TextBundle::from_section(desc.to_string(), TextStyle { font_size: 10.0, color: Color::srgb(0.5, 0.5, 0.5), ..default() }));
                });
            }
        });
    });
}

