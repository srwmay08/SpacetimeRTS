use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use tracing::{info, error};

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::building::{BuildModeState, ModularPieceType}; 
use crate::module_bindings::player_table::PlayerTableAccess; 
use crate::module_bindings::inventory_table::InventoryTableAccess; 
use crate::module_bindings::health_table::HealthTableAccess; 
use crate::module_bindings::structure_table::StructureTableAccess; 
use crate::module_bindings::craft_item_reducer::craft_item;
use crate::module_bindings::spawn_peasant_reducer::spawn_peasant;
use crate::module_bindings::command_peasant_reducer::command_peasant;
use crate::module_bindings::swap_inventory_slots_reducer::swap_inventory_slots;
use crate::module_bindings::drop_inventory_item_reducer::drop_inventory_item;
use crate::module_bindings::admin_give_item_reducer::admin_give_item;
use crate::module_bindings::admin_teleport_reducer::admin_teleport;
use crate::module_bindings::admin_heal_reducer::admin_heal;
use crate::module_bindings::admin_god_mode_reducer::admin_god_mode;
use crate::module_bindings::admin_set_time_reducer::admin_set_time;
use crate::module_bindings::admin_clear_inventory_reducer::admin_clear_inventory;
use crate::module_bindings::admin_spawn_npc_reducer::admin_spawn_npc;
use crate::module_bindings::admin_detonate_reducer::admin_detonate;
use crate::module_bindings::admin_kill_all_npcs_reducer::admin_kill_all_npcs;

use spacetimedb_sdk::Table;

// Architectural Note: Canonical item catalogue used for autocomplete and format verification.
// Guarantees only properly capitalized and valid items can be auto-filled or submitted.
pub const CANONICAL_ITEMS: &[&str] = &[
    "Berry",
    "Branch",
    "Club",
    "Cooked Meat",
    "Crude Bow",
    "Flint",
    "Flint Arrow",
    "Flint Spear",
    "Hammer",
    "Honey",
    "Leather Scraps",
    "LooseStone",
    "Pickaxe",
    "Resin",
    "Stone",
    "Stone Axe",
    "Torch",
    "Wood",
    "Wood Arrow",
    "Wooden Shield",
];

pub const CONSOLE_COMMANDS: &[&str] = &[
    "giveitem",
    "give",
    "tp",
    "teleport",
    "heal",
    "god",
    "time",
    "settime",
    "spawn",
    "nuke",
    "blast",
    "clearinv",
    "killall",
    "help",
];

#[derive(Component)]
pub struct WorkbenchHeaderStatus;

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
    // 1. PERSISTENT TOP-LEFT HOTBAR (Slots 0..7 / Row 1)
    // ------------------------------------------------------------------------
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(15.0),
                left: Val::Px(15.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                ..default()
            },
            z_index: ZIndex::Global(10),
            ..default()
        },
        HotbarRoot,
    )).with_children(|bar| {
        for slot_idx in 0..8 {
            bar.spawn((
                NodeBundle {
                    style: Style {
                        width: Val::Px(55.0),
                        height: Val::Px(55.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(3.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    border_color: Color::srgba(0.4, 0.4, 0.4, 0.8).into(),
                    background_color: Color::srgba(0.1, 0.1, 0.1, 0.85).into(),
                    ..default()
                },
                HotbarSlotUi(slot_idx),
                InventorySlotIndex(slot_idx),
            )).with_children(|slot| {
                slot.spawn(TextBundle::from_section(
                    (slot_idx + 1).to_string(),
                    TextStyle { font_size: 11.0, color: Color::srgb(0.7, 0.7, 0.3), ..default() }
                ).with_style(Style { align_self: AlignSelf::FlexStart, ..default() }));

                slot.spawn((
                    TextBundle::from_section(
                        "",
                        TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }
                    ),
                    HotbarSlotName(slot_idx),
                ));

                slot.spawn((
                    TextBundle::from_section(
                        "",
                        TextStyle { font_size: 12.0, color: Color::srgb(0.9, 0.9, 0.9), ..default() }
                    ).with_style(Style { align_self: AlignSelf::FlexEnd, ..default() }),
                    HotbarSlotCount(slot_idx),
                ));
            });
        }
    });

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
        root.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            ..default()
        }).with_children(|pack_col| {
            pack_col.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                },
                ..default()
            }).with_children(|row_ui| {
                for slot_idx in 8..16 {
                    row_ui.spawn((
                        NodeBundle {
                            style: Style {
                                width: Val::Px(55.0),
                                height: Val::Px(55.0),
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(3.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            border_color: Color::srgba(0.3, 0.3, 0.3, 0.8).into(),
                            background_color: Color::srgba(0.08, 0.08, 0.08, 0.9).into(),
                            ..default()
                        },
                        InventorySlotIndex(slot_idx),
                    )).with_children(|slot| {
                        slot.spawn((
                            TextBundle::from_section(
                                "",
                                TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }
                            ).with_style(Style { margin: UiRect::top(Val::Px(8.0)), ..default() }),
                            InventorySlotName(slot_idx),
                        ));
                        slot.spawn((
                            TextBundle::from_section(
                                "",
                                TextStyle { font_size: 12.0, color: Color::srgb(0.9, 0.9, 0.9), ..default() }
                            ).with_style(Style { align_self: AlignSelf::FlexEnd, ..default() }),
                            InventorySlotCount(slot_idx),
                        ));
                    });
                }
            });
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
                ("Flint Arrow", "8 Wood, 2 Flint [Workbench]"),
                ("Flint Spear", "5 Wood, 2 Flint [Workbench]"),
                ("Wooden Shield", "10 Wood, 2 Leather [Workbench]"),
            ];

            for (item_name, cost) in recipes {
                crafting.spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Px(36.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::FlexStart,
                            padding: UiRect::horizontal(Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        border_color: Color::srgb(0.3, 0.3, 0.3).into(),
                        background_color: Color::srgb(0.16, 0.16, 0.16).into(),
                        ..default()
                    },
                    CraftRecipeButton(item_name.to_string()),
                )).with_children(|btn| {
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
            "DEVELOPER CONSOLE [Press ` to toggle | Tab to Auto-Fill] | giveitem <Item> [amt] | tp <x> <z> | heal | god | time <0-24> | spawn <mob> [amt] | nuke [rad] | help",
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

// ----------------------------------------------------------------------------
// CONSOLE INPUT & AUTO-FILL / AUTOCOMPLETE SYSTEM
// ----------------------------------------------------------------------------

pub fn toggle_console(
    keys: Res<ButtonInput<KeyCode>>,
    mut console: ResMut<ConsoleState>,
    mut console_query: Query<&mut Style, With<ConsoleRoot>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    if keys.just_pressed(KeyCode::Backquote) {
        console.is_open = !console.is_open;
        let Ok(mut window) = window_query.get_single_mut() else { return; };

        if let Ok(mut style) = console_query.get_single_mut() {
            style.display = if console.is_open { Display::Flex } else { Display::None };
        }

        if console.is_open {
            window.cursor.grab_mode = CursorGrabMode::None;
            window.cursor.visible = true;
        } else if *camera_mode.get() == CameraMode::FPS {
            window.cursor.grab_mode = CursorGrabMode::Locked;
            window.cursor.visible = false;
        }
    }
}

pub fn handle_console_input(
    mut console: ResMut<ConsoleState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut key_evts: EventReader<KeyboardInput>,
    conn: Res<SpacetimeConnection>,
    time: Res<Time>,
) {
    if !console.is_open {
        return;
    }

    console.cursor_timer.tick(time.delta());
    if console.cursor_timer.just_finished() {
        console.show_cursor = !console.show_cursor;
    }

    // Architectural Note: Automatic Tab-Completion & Auto-Fill System.
    // Detects command and item prefixes, cycling through canonical matches
    // and automatically filling the input buffer with accurate casing and spacing.
    if keys.just_pressed(KeyCode::Tab) {
        let trimmed = console.input_buffer.trim_start();
        if trimmed.starts_with("giveitem") || trimmed.starts_with("give") {
            let cmd_prefix = if trimmed.starts_with("giveitem") { "giveitem " } else { "give " };
            let arg = trimmed.strip_prefix(cmd_prefix).unwrap_or("").trim_start();

            let matches: Vec<&'static str> = CANONICAL_ITEMS.iter()
                .filter(|&&item| item.to_lowercase().starts_with(&arg.to_lowercase()))
                .copied()
                .collect();

            if !matches.is_empty() {
                let chosen = matches[console.tab_completion_index % matches.len()];
                console.input_buffer = format!("{}{}", cmd_prefix, chosen);
                console.tab_completion_index += 1;
            }
        } else {
            // Command auto-fill
            let matches: Vec<&'static str> = CONSOLE_COMMANDS.iter()
                .filter(|&&cmd| cmd.starts_with(&trimmed.to_lowercase()))
                .copied()
                .collect();

            if !matches.is_empty() {
                let chosen = matches[console.tab_completion_index % matches.len()];
                console.input_buffer = format!("{} ", chosen);
                console.tab_completion_index += 1;
            }
        }
        return;
    }

    // Read typed characters from KeyboardInput's logical_key
    for evt in key_evts.read() {
        if evt.state == ButtonState::Pressed {
            if evt.key_code == KeyCode::Space {
                console.input_buffer.push(' ');
                console.tab_completion_index = 0;
            } else if let Key::Character(ref s) = evt.logical_key {
                for ch in s.chars() {
                    if ch != '`' && ch != '~' && !ch.is_control() {
                        console.input_buffer.push(ch);
                        console.tab_completion_index = 0;
                    }
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        console.input_buffer.pop();
        console.tab_completion_index = 0;
    }

    if keys.just_pressed(KeyCode::ArrowUp) && !console.history.is_empty() {
        let next_idx = match console.history_cursor {
            None => console.history.len().saturating_sub(1),
            Some(i) => i.saturating_sub(1),
        };
        console.history_cursor = Some(next_idx);
        if let Some(cmd) = console.history.get(next_idx) {
            console.input_buffer = cmd.clone();
            console.tab_completion_index = 0;
        }
    }

    if keys.just_pressed(KeyCode::ArrowDown) && !console.history.is_empty() {
        if let Some(i) = console.history_cursor {
            if i + 1 < console.history.len() {
                let next_idx = i + 1;
                console.history_cursor = Some(next_idx);
                console.input_buffer = console.history[next_idx].clone();
            } else {
                console.history_cursor = None;
                console.input_buffer.clear();
            }
            console.tab_completion_index = 0;
        }
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        let command_line = console.input_buffer.trim().to_string();
        console.input_buffer.clear();
        console.history_cursor = None;
        console.tab_completion_index = 0;

        if command_line.is_empty() {
            return;
        }

        console.history.push(command_line.clone());
        console.logs.push(format!("> {}", command_line));

        let tokens: Vec<&str> = command_line.split_whitespace().collect();
        let cmd = tokens[0].to_lowercase();

        match cmd.as_str() {
            "giveitem" | "give" => {
                if tokens.len() < 2 {
                    console.logs.push("[Syntax Error] Usage: giveitem <item_name> [amount]".into());
                    console.logs.push(format!("Available: {}", CANONICAL_ITEMS.join(", ")));
                } else {
                    let (item_name, amount) = if tokens.len() >= 3 && tokens.last().unwrap().parse::<u32>().is_ok() {
                        let amt = tokens.last().unwrap().parse::<u32>().unwrap();
                        let name = tokens[1..tokens.len() - 1].join(" ");
                        (name, amt)
                    } else {
                        let name = tokens[1..].join(" ");
                        (name, 10)
                    };

                    // Architectural Note: Strict Format & Alias Guard.
                    // Enforces exact casing (e.g. "Wood" vs "wood", "Branch" vs "branch")
                    // and clarifies naming disparities (e.g. "LooseStone" vs "Stone").
                    if !CANONICAL_ITEMS.contains(&item_name.as_str()) {
                        let case_match = CANONICAL_ITEMS.iter().find(|&&i| i.eq_ignore_ascii_case(&item_name));
                        if let Some(correct) = case_match {
                            console.logs.push(format!(
                                "[Format Error] Invalid item format '{}'. Did you mean '{}'? (Press Tab to auto-fill)",
                                item_name, correct
                            ));
                        } else if item_name.eq_ignore_ascii_case("loosestone") || item_name.eq_ignore_ascii_case("stone") {
                            console.logs.push(format!(
                                "[Format Error] Ambiguous item '{}'. Ground node stone is 'LooseStone', crafted block is 'Stone'. (Press Tab to auto-fill)",
                                item_name
                            ));
                        } else {
                            console.logs.push(format!("[Format Error] Unknown item '{}'. Use Tab to auto-fill valid options.", item_name));
                            console.logs.push(format!("Available: {}", CANONICAL_ITEMS.join(", ")));
                        }
                        return;
                    }

                    if let Err(e) = conn.db.reducers.admin_give_item(item_name.clone(), amount) {
                        console.logs.push(format!("[Server Error] Failed to grant item: {:?}", e));
                    } else {
                        console.logs.push(format!("[Admin] Granted {}x '{}'", amount, item_name));
                    }
                }
            }
            "tp" | "teleport" => {
                if tokens.len() < 3 {
                    console.logs.push("[Syntax Error] Usage: tp <x> <z>".into());
                } else if let (Ok(x), Ok(z)) = (tokens[1].parse::<f32>(), tokens[2].parse::<f32>()) {
                    if let Err(e) = conn.db.reducers.admin_teleport(x, z) {
                        console.logs.push(format!("[Server Error] Teleport failed: {:?}", e));
                    } else {
                        console.logs.push(format!("[Admin] Teleported to ({:.1}, {:.1})", x, z));
                    }
                } else {
                    console.logs.push("[Syntax Error] Coordinates must be valid floating point numbers.".into());
                }
            }
            "heal" => {
                let amount = tokens.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(100.0);
                if let Err(e) = conn.db.reducers.admin_heal(amount) {
                    console.logs.push(format!("[Server Error] Heal failed: {:?}", e));
                } else {
                    console.logs.push(format!("[Admin] Healed player by {:.0} HP", amount));
                }
            }
            "god" => {
                if let Err(e) = conn.db.reducers.admin_god_mode() {
                    console.logs.push(format!("[Server Error] God mode failed: {:?}", e));
                } else {
                    console.logs.push("[Admin] Invulnerability / God mode enabled (99999 HP).".into());
                }
            }
            "time" | "settime" => {
                if let Some(t) = tokens.get(1).and_then(|s| s.parse::<f32>().ok()) {
                    if let Err(e) = conn.db.reducers.admin_set_time(t) {
                        console.logs.push(format!("[Server Error] Set time failed: {:?}", e));
                    } else {
                        console.logs.push(format!("[Admin] World time set to {:.1}", t));
                    }
                } else {
                    console.logs.push("[Syntax Error] Usage: time <0-24>".into());
                }
            }
            "clearinv" | "clear" => {
                if let Err(e) = conn.db.reducers.admin_clear_inventory() {
                    console.logs.push(format!("[Server Error] Failed to clear inventory: {:?}", e));
                } else {
                    console.logs.push("[Admin] Inventory wiped clean.".into());
                }
            }
            "spawn" => {
                if tokens.len() < 2 {
                    console.logs.push("[Syntax Error] Usage: spawn <deer|boar|goblin|peasant> [count]".into());
                } else {
                    let mob_type = tokens[1].to_string();
                    let count = tokens.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                    if let Err(e) = conn.db.reducers.admin_spawn_npc(mob_type.clone(), count) {
                        console.logs.push(format!("[Server Error] Spawn failed: {:?}", e));
                    } else {
                        console.logs.push(format!("[Admin] Dispatched {}x {}", count, mob_type));
                    }
                }
            }
            "nuke" | "blast" => {
                let radius = tokens.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(6.0);
                if let Err(e) = conn.db.reducers.admin_detonate(radius, 600.0) {
                    console.logs.push(format!("[Server Error] Detonation failed: {:?}", e));
                } else {
                    console.logs.push(format!("[Admin] Voxel blast triggered with radius {:.1}", radius));
                }
            }
            "killall" => {
                if let Err(e) = conn.db.reducers.admin_kill_all_npcs() {
                    console.logs.push(format!("[Server Error] Killall failed: {:?}", e));
                } else {
                    console.logs.push("[Admin] All non-player entities destroyed.".into());
                }
            }
            "help" => {
                console.logs.push("--- PLAYTESTING COMMAND DIRECTORY ---".into());
                console.logs.push("giveitem <Item> [amt]  : Grants item (Press [Tab] to auto-fill)".into());
                console.logs.push("tp <x> <z>             : Teleports player to world coordinate".into());
                console.logs.push("heal [amt]             : Restores player health points".into());
                console.logs.push("god                    : Sets health to 99999 HP".into());
                console.logs.push("time <0-24>            : Sets diurnal world clock".into());
                console.logs.push("spawn <mob> [amt]      : Spawns Deer, Boar, Goblin, Peasant".into());
                console.logs.push("nuke [radius]          : Demolishes terrain with spherical blast".into());
                console.logs.push("clearinv               : Empties inventory slots completely".into());
                console.logs.push("killall                : Destroys all active NPC brains".into());
            }
            _ => {
                console.logs.push(format!("[Error] Unknown command '{}'. Enter 'help' for directory.", tokens[0]));
            }
        }

        if console.logs.len() > 60 {
            let overflow = console.logs.len() - 60;
            console.logs.drain(0..overflow);
        }
    }
}

pub fn update_console_ui(
    console: Res<ConsoleState>,
    mut log_query: Query<&mut Text, (With<ConsoleLogText>, Without<ConsoleInputText>, Without<ConsoleSuggestionsText>)>,
    mut input_query: Query<&mut Text, (With<ConsoleInputText>, Without<ConsoleLogText>, Without<ConsoleSuggestionsText>)>,
    mut sugg_query: Query<&mut Text, (With<ConsoleSuggestionsText>, Without<ConsoleLogText>, Without<ConsoleInputText>)>,
) {
    if !console.is_open {
        return;
    }

    if let Ok(mut log_text) = log_query.get_single_mut() {
        let display_lines = console.logs.iter().rev().take(11).cloned().collect::<Vec<_>>();
        let text_block = display_lines.into_iter().rev().collect::<Vec<_>>().join("\n");
        if log_text.sections[0].value != text_block {
            log_text.sections[0].value = text_block;
        }
    }

    if let Ok(mut input_text) = input_query.get_single_mut() {
        let cursor_char = if console.show_cursor { "_" } else { " " };
        input_text.sections[0].value = format!("{}{}", console.input_buffer, cursor_char);
    }

    // Dynamic suggestions banner
    if let Ok(mut sugg_text) = sugg_query.get_single_mut() {
        let trimmed = console.input_buffer.trim_start();
        if trimmed.starts_with("giveitem") || trimmed.starts_with("give") {
            let cmd_prefix = if trimmed.starts_with("giveitem") { "giveitem" } else { "give" };
            let arg = trimmed.strip_prefix(cmd_prefix).unwrap_or("").trim_start();
            let matches: Vec<&'static str> = CANONICAL_ITEMS.iter()
                .filter(|&&i| i.to_lowercase().starts_with(&arg.to_lowercase()))
                .copied()
                .collect();

            if matches.is_empty() {
                sugg_text.sections[0].value = "[No items match query | Press Tab to browse catalogue]".to_string();
                sugg_text.sections[0].style.color = Color::srgb(0.9, 0.4, 0.4);
            } else {
                let shown = if matches.len() > 6 {
                    format!("{}, ... ({} matches)", matches[..6].join(", "), matches.len())
                } else {
                    matches.join(", ")
                };
                sugg_text.sections[0].value = format!("[Tab to auto-fill]: {}", shown);
                sugg_text.sections[0].style.color = Color::srgb(0.65, 0.85, 0.65);
            }
        } else {
            let matches: Vec<&'static str> = CONSOLE_COMMANDS.iter()
                .filter(|&&c| c.starts_with(&trimmed.to_lowercase()))
                .copied()
                .collect();
            sugg_text.sections[0].value = format!("[Tab to complete]: {}", matches.join(", "));
            sugg_text.sections[0].style.color = Color::srgb(0.5, 0.75, 0.9);
        }
    }
}

// ----------------------------------------------------------------------------
// INVENTORY DRAG-AND-DROP & WORLD DROP SYSTEMS
// ----------------------------------------------------------------------------

pub fn handle_inventory_drag_and_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    slot_query: Query<(&InventorySlotIndex, &GlobalTransform, &Node)>,
    mut drag_drop: ResMut<DragDropState>,
    conn: Res<SpacetimeConnection>,
    cached_player: Res<CachedPlayerEntity>,
) {
    let Ok(window) = window_query.get_single() else { return; };
    let Some(cursor_pos) = window.cursor_position() else { return; };

    let Some(player_id) = cached_player.0 else { return; };
    let Some(inv) = conn.db.db.inventory().entity_id().find(&player_id) else { return; };

    if mouse.just_pressed(MouseButton::Left) {
        for (slot_idx, transform, node) in slot_query.iter() {
            let rect = Rect::from_center_size(transform.translation().truncate(), node.size());
            if rect.contains(cursor_pos) {
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

        let mut target_slot = None;
        for (slot_idx, transform, node) in slot_query.iter() {
            let rect = Rect::from_center_size(transform.translation().truncate(), node.size());
            if rect.contains(cursor_pos) {
                target_slot = Some(slot_idx.0);
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
        } else {
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

pub fn update_drag_ghost_ui(
    drag_drop: Res<DragDropState>,
    mut ghost_query: Query<&mut Style, With<DragGhostUi>>,
    mut text_query: Query<&mut Text, With<DragGhostText>>,
) {
    let Ok(mut style) = ghost_query.get_single_mut() else { return; };
    let Ok(mut text) = text_query.get_single_mut() else { return; };

    if drag_drop.is_dragging {
        style.display = Display::Flex;
        style.left = Val::Px(drag_drop.current_pos.x - 27.5);
        style.top = Val::Px(drag_drop.current_pos.y - 27.5);
        text.sections[0].value = format!("{}\n(x{})", drag_drop.item_type, drag_drop.count);
    } else {
        style.display = Display::None;
    }
}

// ----------------------------------------------------------------------------
// INVENTORY & WORKBENCH HUD SYSTEMS
// ----------------------------------------------------------------------------

pub fn handle_crafting_interaction(
    mut interaction_query: Query<(&Interaction, &CraftRecipeButton, &mut BackgroundColor), Changed<Interaction>>,
    conn: Res<SpacetimeConnection>,
) {
    for (interaction, btn, mut bg) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                info!("CLIENT UI: Crafting item '{}'", btn.0);
                if let Err(e) = conn.db.reducers.craft_item(btn.0.clone()) {
                    error!("Crafting failed: {:?}", e);
                }
            }
            Interaction::Hovered => {
                *bg = Color::srgb(0.28, 0.28, 0.28).into();
            }
            Interaction::None => {
                *bg = Color::srgb(0.16, 0.16, 0.16).into();
            }
        }
    }
}

pub fn update_hotbar_ui(
    conn: Res<SpacetimeConnection>,
    active_slot: Res<ActiveItemSlot>,
    mut cached_player: ResMut<CachedPlayerEntity>,
    mut slot_q: Query<(&HotbarSlotUi, &mut BorderColor, &mut BackgroundColor)>,
    mut name_q: Query<(&mut Text, &HotbarSlotName), Without<HotbarSlotCount>>,
    mut count_q: Query<(&mut Text, &HotbarSlotCount), Without<HotbarSlotName>>,
    mut last_active_slot: Local<Option<usize>>,
    mut last_inventory_hash: Local<u64>,
) {
    let Some(identity) = &conn.identity else { return; };
    
    let player_entity_id = match cached_player.0 {
        Some(id) => id,
        None => {
            let Some(player) = conn.db.db.player().identity().find(identity) else { return; };
            cached_player.0 = Some(player.entity_id);
            player.entity_id
        }
    };
    
    let Some(inventory) = conn.db.db.inventory().entity_id().find(&player_entity_id) else { return; };
    
    let inventory_hash = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        inventory.slots.len().hash(&mut hasher);
        for slot in &inventory.slots {
            slot.item_type.hash(&mut hasher);
            slot.count.hash(&mut hasher);
        }
        hasher.finish()
    };
    
    let active_changed = *last_active_slot != Some(active_slot.0);
    if active_changed {
        for (slot_ui, mut border, mut bg) in slot_q.iter_mut() {
            if slot_ui.0 == active_slot.0 {
                *border = Color::srgb(1.0, 0.85, 0.2).into();
                *bg = Color::srgba(0.25, 0.25, 0.15, 0.95).into();
            } else {
                *border = Color::srgba(0.4, 0.4, 0.4, 0.8).into();
                *bg = Color::srgba(0.1, 0.1, 0.1, 0.85).into();
            }
        }
        *last_active_slot = Some(active_slot.0);
    }
    
    let inventory_changed = *last_inventory_hash != inventory_hash;
    if inventory_changed {
        for (mut text, name) in name_q.iter_mut() {
            let new_value = inventory.slots.get(name.0)
                .filter(|s| s.count > 0)
                .map(|s| s.item_type.as_str())
                .unwrap_or("");
            if text.sections[0].value != new_value {
                text.sections[0].value = new_value.to_string();
            }
        }
        
        for (mut text, count) in count_q.iter_mut() {
            let new_value = inventory.slots.get(count.0)
                .filter(|s| s.count > 1)
                .map(|s| s.count.to_string())
                .unwrap_or_default();
            if text.sections[0].value != new_value {
                text.sections[0].value = new_value;
            }
        }
        *last_inventory_hash = inventory_hash;
    }
}

pub fn update_hud_health_bar(
    conn: Res<SpacetimeConnection>,
    mut cached_player: ResMut<CachedPlayerEntity>,
    mut fill_q: Query<&mut Style, With<HealthBarFill>>,
    mut text_q: Query<&mut Text, With<HealthBarText>>,
    mut last_health: Local<Option<(f32, f32)>>,
) {
    let Some(identity) = &conn.identity else { return; };
    
    let player_entity_id = match cached_player.0 {
        Some(id) => id,
        None => {
            let Some(player) = conn.db.db.player().identity().find(identity) else { return; };
            cached_player.0 = Some(player.entity_id);
            player.entity_id
        }
    };
    
    let Some(hp) = conn.db.db.health().entity_id().find(&player_entity_id) else { return; };
    
    let current_health = (hp.current, hp.max);
    if *last_health == Some(current_health) {
        return;
    }
    *last_health = Some(current_health);
    
    let ratio = (hp.current / hp.max).clamp(0.0, 1.0);

    if let Ok(mut style) = fill_q.get_single_mut() {
        style.width = Val::Percent(ratio * 100.0);
    }
    if let Ok(mut text) = text_q.get_single_mut() {
        let new_text = format!("{:.0} / {:.0}", hp.current, hp.max);
        if text.sections[0].value != new_text {
            text.sections[0].value = new_text;
        }
    }
}

pub fn handle_build_menu_selection(
    mut interaction_query: Query<(&Interaction, &BuildPieceButton, &mut BackgroundColor), Changed<Interaction>>,
    mut build_state: ResMut<BuildModeState>,
    mut menu_query: Query<&mut Style, With<BuildMenuRoot>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    for (interaction, btn, mut bg) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                build_state.selected_piece = btn.0;
                build_state.is_active = true;
                if let Ok(mut style) = menu_query.get_single_mut() {
                    style.display = Display::None;
                }
                if let Ok(mut window) = window_query.get_single_mut() {
                    if *camera_mode.get() == CameraMode::FPS {
                        window.cursor.grab_mode = CursorGrabMode::Locked;
                        window.cursor.visible = false;
                    }
                }
            }
            Interaction::Hovered => {
                *bg = Color::srgb(0.3, 0.3, 0.3).into();
            }
            Interaction::None => {
                *bg = Color::srgb(0.18, 0.18, 0.18).into();
            }
        }
    }
}

pub fn toggle_action_bar_visibility(
    camera_mode: Res<State<CameraMode>>,
    mut query: Query<&mut Style, With<ActionBarUiRoot>>
) {
    let desired_display = if *camera_mode.get() == CameraMode::RTS {
        Display::Flex
    } else {
        Display::None
    };

    for mut style in query.iter_mut() {
        if style.display != desired_display {
            style.display = desired_display;
        }
    }
}

pub fn action_bar_interaction(
    mut interaction_query: Query<(&Interaction, &ActionBarButton, &mut BackgroundColor), Changed<Interaction>>,
    conn: Res<SpacetimeConnection>,
    selected_peasants: Query<&PeasantUnit, With<Selected>>,
) {
    for (interaction, button, mut bg) in interaction_query.iter_mut() {
        if button.0.is_empty() { continue; } 
        
        match *interaction {
            Interaction::Pressed => {
                *bg = Color::srgb(0.3, 0.8, 0.3).into(); 
                
                info!("CLIENT UI: Dispatching Action '{}' to {} selected units.", button.0, selected_peasants.iter().count());

                if button.0 == "Spawn Worker" {
                    if let Err(e) = conn.db.reducers.spawn_peasant() {
                        error!("NETWORK ERROR: Failed to spawn peasant. Details: {:?}", e);
                    }
                } else if button.0 == "Stop" {
                    for peasant in selected_peasants.iter() {
                        if let Err(e) = conn.db.reducers.command_peasant(peasant.entity_id, "Idle".to_string(), 0.0, 0.0, 0.0, 0) {
                            error!("NETWORK ERROR: Failed to command peasant. Details: {:?}", e);
                        }
                    }
                } else if button.0 == "Chop Wood" {
                    for peasant in selected_peasants.iter() {
                        if let Err(e) = conn.db.reducers.command_peasant(peasant.entity_id, "AutoTree".to_string(), 0.0, 0.0, 0.0, 0) {
                            error!("NETWORK ERROR: Failed to command peasant. Details: {:?}", e);
                        }
                    }
                } else if button.0 == "Mine Stone" {
                    for peasant in selected_peasants.iter() {
                        if let Err(e) = conn.db.reducers.command_peasant(peasant.entity_id, "AutoRock".to_string(), 0.0, 0.0, 0.0, 0) {
                            error!("NETWORK ERROR: Failed to command peasant. Details: {:?}", e);
                        }
                    }
                } else if button.0 == "Forage Berries" {
                    for peasant in selected_peasants.iter() {
                        if let Err(e) = conn.db.reducers.command_peasant(peasant.entity_id, "AutoBush".to_string(), 0.0, 0.0, 0.0, 0) {
                            error!("NETWORK ERROR: Failed to command peasant. Details: {:?}", e);
                        }
                    }
                } else if button.0 == "Collect All" {
                    for peasant in selected_peasants.iter() {
                        if let Err(e) = conn.db.reducers.command_peasant(peasant.entity_id, "AutoAll".to_string(), 0.0, 0.0, 0.0, 0) {
                            error!("NETWORK ERROR: Failed to command peasant. Details: {:?}", e);
                        }
                    }
                }
            }
            Interaction::Hovered => {
                *bg = Color::srgb(0.2, 0.2, 0.2).into();
            }
            Interaction::None => {
                *bg = Color::srgb(0.15, 0.15, 0.15).into();
            }
        }
    }
}

pub fn update_build_ui(
    build_state: Res<BuildModeState>,
    mut ui_query: Query<(&mut Visibility, &mut Text), With<BuildUIText>>,
) {
    if build_state.is_changed() {
        for (mut vis, mut text) in ui_query.iter_mut() {
            if build_state.is_active {
                *vis = Visibility::Inherited;
                text.sections[0].value = format!(
                    "BUILD MODE: ACTIVE | Piece: {} (Cost: {} Wood)\n[R] Next Piece | [Q/E] Rotate | [Right-Click] Catalog | [B] Exit", 
                    build_state.selected_piece.name(),
                    build_state.selected_piece.wood_cost()
                );
            } else {
                *vis = Visibility::Hidden;
            }
        }
    }
}

pub fn toggle_inventory_ui(
    keys: Res<ButtonInput<KeyCode>>, 
    console: Res<ConsoleState>,
    mut query: Query<&mut Style, With<InventoryUiRoot>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    if console.is_open {
        return;
    }

    if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::KeyI) {
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
    mut header_q: Query<&mut Text, (With<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>)>,
    mut name_q: Query<(&mut Text, &InventorySlotName), Without<InventorySlotCount>>,
    mut count_q: Query<(&mut Text, &InventorySlotCount), Without<InventorySlotName>>,
) {
    let Some(identity) = &conn.identity else { return; };
    if let Some(player) = conn.db.db.player().identity().find(identity) {
        if let Some(inventory) = conn.db.db.inventory().entity_id().find(&player.entity_id) {
            
            for (mut text, name) in name_q.iter_mut() {
                if let Some(slot) = inventory.slots.get(name.0) {
                    text.sections[0].value = if slot.count > 0 { slot.item_type.clone() } else { "".to_string() };
                } else {
                    text.sections[0].value = "".to_string();
                }
            }
            
            for (mut text, count) in count_q.iter_mut() {
                if let Some(slot) = inventory.slots.get(count.0) {
                    text.sections[0].value = if slot.count > 1 { slot.count.to_string() } else { "".to_string() };
                } else {
                    text.sections[0].value = "".to_string();
                }
            }
        }
    }

    if let Ok(player_t) = player_query.get_single() {
        let near_workbench = conn.db.db.structure().iter().any(|s| {
            if s.piece_type == "Workbench" && !s.is_blueprint {
                let dist_sq = (s.x - player_t.translation.x).powi(2) + (s.z - player_t.translation.z).powi(2);
                dist_sq <= 400.0
            } else {
                false
            }
        });

        for mut header in header_q.iter_mut() {
            if near_workbench {
                header.sections[0].value = "CRAFTING RECIPES [WORKBENCH ACTIVE]".to_string();
                header.sections[0].style.color = Color::srgb(1.0, 0.85, 0.2);
            } else {
                header.sections[0].value = "CRAFTING RECIPES [FIELD CRAFTING]".to_string();
                header.sections[0].style.color = Color::srgb(0.7, 0.7, 0.7);
            }
        }
    }
}

pub fn update_floating_health_bars(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCameraChild>>,
    unit_query: Query<(&NetworkEntity, &GlobalTransform)>,
    mut bar_query: Query<(Entity, &HealthBarUI, &mut Style, &mut BackgroundColor, &mut Visibility)>,
    camera_mode: Res<State<CameraMode>>,
) {
    if *camera_mode.get() != CameraMode::RTS {
        for (_, _, _, _, mut vis) in bar_query.iter_mut() { 
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden; 
            }
        }
        return;
    }

    let Ok((camera, cam_transform)) = camera_query.get_single() else { return; };
    
    let health_map: std::collections::HashMap<u64, f32> = conn.db.db.health().iter()
        .map(|h| (h.entity_id, (h.current / h.max).clamp(0.0, 1.0)))
        .collect();

    let mut tracked_units = std::collections::HashSet::new();

    for (net_id, transform) in unit_query.iter() {
        if let Some(&hp_percent) = health_map.get(&net_id.0) {
            tracked_units.insert(net_id.0);

            if let Some(screen_pos) = camera.world_to_viewport(cam_transform, transform.translation() + Vec3::Y * 2.2) {
                let color = if hp_percent > 0.5 { 
                    Color::srgb(0.1, 0.8, 0.1) 
                } else if hp_percent > 0.2 { 
                    Color::srgb(0.8, 0.8, 0.1) 
                } else { 
                    Color::srgb(0.8, 0.1, 0.1) 
                };

                let mut found = false;
                for (_, bar_ui, mut style, mut bg, mut vis) in bar_query.iter_mut() {
                    if bar_ui.0 == net_id.0 {
                        style.left = Val::Px(screen_pos.x - 25.0);
                        style.top = Val::Px(screen_pos.y);
                        style.width = Val::Px(50.0 * hp_percent);
                        *bg = color.into();
                        *vis = Visibility::Inherited;
                        found = true;
                        break;
                    }
                }

                if !found {
                    commands.spawn((
                        NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                left: Val::Px(screen_pos.x - 25.0),
                                top: Val::Px(screen_pos.y),
                                width: Val::Px(50.0 * hp_percent),
                                height: Val::Px(5.0),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            background_color: color.into(),
                            border_color: Color::BLACK.into(),
                            ..default()
                        },
                        HealthBarUI(net_id.0),
                    ));
                }
            }
        }
    }

    for (entity, bar_ui, _, _, _) in bar_query.iter() {
        if !tracked_units.contains(&bar_ui.0) {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn update_marquee_ui(
    state: Res<SelectionState>,
    mut query: Query<(&mut Style, &mut Visibility), With<MarqueeUI>>
) {
    let Ok((mut style, mut vis)) = query.get_single_mut() else { return; };

    if state.is_dragging {
        if let (Some(start), Some(end)) = (state.start_pos, state.end_pos) {
            let min_x = start.x.min(end.x);
            let max_x = start.x.max(end.x);
            let min_y = start.y.min(end.y);
            let max_y = start.y.max(end.y);

            style.left = Val::Px(min_x);
            style.top = Val::Px(min_y);
            style.width = Val::Px(max_x - min_x);
            style.height = Val::Px(max_y - min_y);
            *vis = Visibility::Inherited;
            return;
        }
    }
    *vis = Visibility::Hidden;
}

pub fn visualize_selection(
    selected_query: Query<&Children, With<Selected>>,
    unselected_query: Query<&Children, (With<Selectable>, Without<Selected>)>,
    mut ring_query: Query<&mut Visibility, With<SelectionRing>>,
) {
    for children in selected_query.iter() {
        for &child in children.iter() {
            if let Ok(mut vis) = ring_query.get_mut(child) { *vis = Visibility::Inherited; }
        }
    }

    for children in unselected_query.iter() {
        for &child in children.iter() {
            if let Ok(mut vis) = ring_query.get_mut(child) { *vis = Visibility::Hidden; }
        }
    }
}

pub fn animate_view_model(
    time: Res<Time>,
    mut swing_state: ResMut<SwingState>,
    mut arm_query: Query<&mut Transform, With<ViewModelArm>>
) {
    if swing_state.is_swinging {
        swing_state.timer.tick(time.delta());
        let Ok(mut arm) = arm_query.get_single_mut() else { return; };

        let t = swing_state.timer.fraction();
        let angle = if t < 0.5 { 1.0 - (t * 2.0) } else { (t - 0.5) * 2.0 };
        arm.rotation = Quat::from_rotation_x(angle);

        if swing_state.timer.just_finished() {
            swing_state.is_swinging = false;
            swing_state.timer.reset();
        }
    }
}

pub fn tick_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Particle)>,
) {
    for (entity, mut particle) in query.iter_mut() {
        if particle.timer.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}