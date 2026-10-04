use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow, WindowMode};
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
use crate::module_bindings::equipment_loadout_table::EquipmentLoadoutTableAccess;
use crate::module_bindings::equip_weapon_reducer::equip_weapon;
use crate::module_bindings::unequip_weapon_reducer::unequip_weapon;
use crate::weapons::EquippedHandSide;
use spacetime_rts_logic::{HandSide, BagContainerDef, create_bag_container};

use spacetimedb_sdk::Table;

// Architectural Note: Canonical item catalogue used for autocomplete and format verification.
// Guarantees only properly capitalized and valid items can be auto-filled or submitted.
pub const CANONICAL_ITEMS: &[&str] = &[
    "1h Axe",
    "1h Black Jack",
    "1h Hammer",
    "1h Ranged Hand Crossbow",
    "1h Ranged Orb",
    "1h Ranged Revolver",
    "1h Ranged Wand",
    "1h Sword",
    "1h Tiger Claws",
    "2h Axe",
    "2h Hammer",
    "2h Ranged Long Bow",
    "2h Ranged Runestaff",
    "2h Ranged Shotgun",
    "2h Ranged Sniper Rifle",
    "2h Sword",
    "Bag of Sewn Evil-Eye",
    "Ballista",
    "Battering Ram",
    "Berry",
    "Black Jack",
    "Bouncy Bomb Ammo",
    "Bouncy Bomb Launcher",
    "Bow",
    "Branch",
    "Catapult",
    "Cestus",
    "Club",
    "Cooked Meat",
    "Crossbow",
    "Crossbow Bolt",
    "Crude Bow",
    "Dagger",
    "Dragonflight Haversack",
    "Elder Wood",
    "Flint",
    "Flint Arrow",
    "Flint Spear",
    "Frying Pan",
    "Greataxe",
    "Greatsword",
    "Halberd",
    "Hammer",
    "Hand Crossbow",
    "Handaxe",
    "Holy Mackerel",
    "Honey",
    "Iron Boots",
    "Iron Chestplate",
    "Iron Greaves",
    "Iron Helmet",
    "Iron Ingot",
    "Javelin",
    "Knuckle-Duster",
    "Leather Bag",
    "Leather Scraps",
    "Long Bow",
    "Longbow",
    "Longsword",
    "LooseStone",
    "Maul",
    "Orb",
    "Pickaxe",
    "Polearm Javelin - Thrown",
    "Polearm Spear",
    "Polearm Trident",
    "Rapier",
    "Resin",
    "Revolver",
    "Revolver Ammo",
    "Runestaff",
    "Shotgun",
    "Shotgun Shell",
    "Shralok Pack",
    "Small Pouch",
    "Sniper Ammo",
    "Sniper Rifle",
    "Spear",
    "Stone",
    "Stone Axe",
    "Tiger Claws",
    "Torch",
    "Traveler's Backpack",
    "Trebuchet",
    "Trident",
    "Unarmed",
    "Wand",
    "Warhammer",
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
    "tuner",
    "weapontool",
    "crosshair",
    "abilities",
    "day",
    "noon",
    "night",
    "midnight",
    "weather",
    "aurora",
    "rain",
    "storm",
    "clearsky",
    "haze",
    "timescale",
    "speed",
    "star",
    "stara",
    "starb",
    "dualshadows",
    "dualshadow",
    "ambient",
    "starsize",
    "sunset",
    "dusk",
    "dawn",
    "palette",
    "hud",
    "togglehud",
    "skyhud",
    "f3",
    "fps",
    "diag",
    "dual",
    "equip",
    "skills",
    "spellbook",
    "res",
    "resolution",
    "fullscreen",
    "fs",
    "windowed",
    "win",
    "maxfps",
    "fpslimit",
    "fps_max",
    "limitfps",
    "vsync",
    "help",
];

#[derive(Component)]
pub struct WorkbenchHeaderStatus;

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientEquippedBags {
    pub bags: [Option<BagContainerDef>; 4],
}

#[derive(Component)] pub struct PaperdollMainHandSlot;
#[derive(Component)] pub struct PaperdollOffHandSlot;
#[derive(Component)] pub struct PaperdollMainHandText;
#[derive(Component)] pub struct PaperdollOffHandText;
#[derive(Component)] pub struct PaperdollPrimaryHandText;
#[derive(Component)] pub struct PaperdollPrimaryHandButton;
#[derive(Component)] pub struct PaperdollUnequipMainButton;
#[derive(Component)] pub struct PaperdollUnequipOffButton;
#[derive(Component, Clone, Copy, Debug)] pub struct PaperdollBagSlotIndex(pub usize);
#[derive(Component, Clone, Copy, Debug)] pub struct PaperdollBagText(pub usize);
#[derive(Component, Clone, Copy, Debug)] pub struct PaperdollBagTooltip(pub usize);
#[derive(Component)] pub struct InventoryCapacityHeader;
#[derive(Component)] pub struct CelestialHudRoot;
#[derive(Component)] pub struct CelestialHudText;
#[derive(Component)] pub struct DiagnosticOverlayRoot;
#[derive(Component)] pub struct DiagnosticOverlayText;

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
                ("Revolver", "10 Stone, 8 Flint, 4 Leather [Workbench]"),
                ("Revolver Ammo", "4 Stone, 2 Flint, 1 Resin (x12) [Workbench]"),
                ("Shotgun", "15 Wood, 10 Stone, 6 Flint [Workbench]"),
                ("Shotgun Shell", "4 Wood, 3 Stone, 1 Resin (x8) [Workbench]"),
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

        // Reticle-Adjacent Ammo Gauge (Right of Crosshair)
        reticle.spawn((
            TextBundle::from_section(
                "",
                TextStyle {
                    font_size: 13.0,
                    color: Color::srgb(0.0, 1.0, 1.0),
                    ..default()
                }
            ).with_style(Style {
                position_type: PositionType::Absolute,
                left: Val::Px(30.0),
                top: Val::Px(-9.0),
                ..default()
            }),
            ReticleAmmoText,
        ));

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
    mut ephemeris: Option<ResMut<crate::binary_sky::BinaryEphemerisState>>,
    mut sky_config: Option<ResMut<crate::binary_sky::BinarySkyConfig>>,
    mut sky_weather: Option<ResMut<crate::binary_sky::AtmosphericWeather>>,
    mut hud_pill_query: Query<&mut Style, With<CelestialHudRoot>>,
    mut diag_pill_query: Query<&mut Style, (With<DiagnosticOverlayRoot>, Without<CelestialHudRoot>)>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    mut fps_limiter: ResMut<FpsLimiterState>,
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
            "day" | "noon" => {
                if let Some(ref mut eph) = ephemeris {
                    eph.simulation_time_seconds = 0.0;
                    eph.diurnal_angle = 0.0;
                }
                let _ = conn.db.reducers.admin_set_time(12.0);
                console.logs.push("[Admin] Celestial cycle set to HIGH NOON (12:00). Direct sunlight active.".into());
            }
            "night" | "midnight" => {
                let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                if let Some(ref mut eph) = ephemeris {
                    eph.simulation_time_seconds = duration * 0.5;
                    eph.diurnal_angle = std::f32::consts::PI;
                }
                let _ = conn.db.reducers.admin_set_time(0.0);
                console.logs.push("[Admin] Celestial cycle set to DEEP MIDNIGHT (00:00). Starfield & Aurora active.".into());
            }
            "time" | "settime" => {
                if let Some(t) = tokens.get(1).and_then(|s| s.parse::<f32>().ok()) {
                    let hour_clamped = t.rem_euclid(24.0);
                    let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                    let frac = ((hour_clamped / 24.0) - 0.5).rem_euclid(1.0);
                    if let Some(ref mut eph) = ephemeris {
                        eph.simulation_time_seconds = duration * frac as f64;
                        eph.diurnal_angle = (frac * 2.0 * std::f32::consts::PI) as f32;
                    }
                    if let Err(e) = conn.db.reducers.admin_set_time(hour_clamped) {
                        console.logs.push(format!("[Server Notice] Admin reducer: {:?}", e));
                    }
                    console.logs.push(format!("[Admin] World time set to {:.1}h", hour_clamped));
                } else {
                    console.logs.push("[Syntax Error] Usage: time <0-24> or 'day' / 'night'".into());
                }
            }
            "timescale" | "speed" => {
                if let Some(speed) = tokens.get(1).and_then(|s| s.parse::<f32>().ok()) {
                    if let Some(ref mut cfg) = sky_config {
                        cfg.time_scale = speed.max(0.0);
                    }
                    console.logs.push(format!("[Admin] Celestial time scale set to {:.1}x (Press [-] or [=])", speed));
                } else {
                    console.logs.push("[Syntax Error] Usage: timescale <multiplier> (e.g. 1.0, 10.0, 60.0)".into());
                }
            }
            "weather" => {
                if let Some(w_type) = tokens.get(1) {
                    if let Some(ref mut w) = sky_weather {
                        match w_type.to_lowercase().as_str() {
                            "clear" | "clearsky" => {
                                w.weather_type = crate::binary_sky::WeatherType::ClearSky;
                                console.logs.push("[Weather] Set to ClearSky (pristine visibility, deep Rayleigh blues).".into());
                            }
                            "haze" | "aerosol" => {
                                w.weather_type = crate::binary_sky::WeatherType::AerosolHaze;
                                console.logs.push("[Weather] Set to AerosolHaze (golden horizon, diffuse Mie halo).".into());
                            }
                            "aurora" | "storm_aurora" => {
                                w.weather_type = crate::binary_sky::WeatherType::StellarWindAurora;
                                console.logs.push("[Weather] Set to StellarWindAurora (binary magnetic curtains).".into());
                                console.logs.push("[Tip] Enter 'night' or press [F8] for deep darkness view.".into());
                            }
                            "rain" | "storm" | "overcast" => {
                                w.weather_type = crate::binary_sky::WeatherType::OvercastPrecipitation;
                                console.logs.push("[Weather] Set to OvercastPrecipitation (rain streaks, heavy overcast).".into());
                            }
                            _ => {
                                console.logs.push("[Syntax Error] Options: clear, haze, aurora, rain (or press [F9])".into());
                            }
                        }
                    }
                } else if let Some(ref w) = sky_weather {
                    console.logs.push(format!("[Weather] Current: {:?}. Options: clear, haze, aurora, rain (or press [F9])", w.weather_type));
                } else {
                    console.logs.push("[Syntax Error] Usage: weather <clear|haze|aurora|rain> (or press [F9])".into());
                }
            }
            "aurora" => {
                if let Some(ref mut w) = sky_weather {
                    w.weather_type = crate::binary_sky::WeatherType::StellarWindAurora;
                    console.logs.push("[Weather] StellarWindAurora activated! Shimmering aurora curtains glowing in the sky.".into());
                    console.logs.push("[Tip] Enter 'night' or press [F8] for deep darkness view.".into());
                }
            }
            "rain" | "storm" => {
                if let Some(ref mut w) = sky_weather {
                    w.weather_type = crate::binary_sky::WeatherType::OvercastPrecipitation;
                    console.logs.push("[Weather] OvercastPrecipitation activated! Heavy rain streaks and stormy overcast.".into());
                }
            }
            "clearsky" => {
                if let Some(ref mut w) = sky_weather {
                    w.weather_type = crate::binary_sky::WeatherType::ClearSky;
                    console.logs.push("[Weather] ClearSky activated! Pristine visibility and deep Rayleigh blues.".into());
                }
            }
            "haze" => {
                if let Some(ref mut w) = sky_weather {
                    w.weather_type = crate::binary_sky::WeatherType::AerosolHaze;
                    console.logs.push("[Weather] AerosolHaze activated! Golden horizon and dense Mie halo.".into());
                }
            }
            "hud" | "togglehud" | "skyhud" | "timehud" | "celestialhud" => {
                let sub = tokens.get(1).map(|s| s.to_lowercase());
                if let Ok(mut style) = hud_pill_query.get_single_mut() {
                    match sub.as_deref() {
                        Some("on") | Some("show") | Some("1") | Some("true") => {
                            style.display = Display::Flex;
                            console.logs.push("[HUD] Celestial HUD display (time, conjunction, weather [F8/F9]) toggled ON (visible).".into());
                        }
                        Some("off") | Some("hide") | Some("0") | Some("false") => {
                            style.display = Display::None;
                            console.logs.push("[HUD] Celestial HUD display (time, conjunction, weather [F8/F9]) toggled OFF (hidden).".into());
                        }
                        Some("status") => {
                            let status_str = if style.display == Display::None { "hidden (OFF)" } else { "visible (ON)" };
                            console.logs.push(format!("[HUD] Celestial HUD display is currently {}.", status_str));
                        }
                        Some("toggle") | None => {
                            let new_state = style.display == Display::None;
                            style.display = if new_state { Display::Flex } else { Display::None };
                            let action_str = if new_state { "toggled ON (visible)" } else { "toggled OFF (hidden)" };
                            console.logs.push(format!("[HUD] Celestial HUD display (time, conjunction, weather [F8/F9]) {}.", action_str));
                        }
                        _ => {
                            console.logs.push("[Syntax Error] Usage: hud [on|off|toggle|status] (or simply 'hud') [F10]".into());
                        }
                    }
                } else {
                    console.logs.push("[HUD] Celestial HUD element not found.".into());
                }
            }
            "maxfps" | "fpslimit" | "fps_max" | "limitfps" => {
                if tokens.len() == 1 {
                    let limit_str = match fps_limiter.target_fps {
                        Some(fps) => format!("capped at {} FPS", fps),
                        None => "UNCAPPED (unlimited)".to_string(),
                    };
                    let vsync_str = if let Ok(window) = window_query.get_single() {
                        match window.present_mode {
                            bevy::window::PresentMode::AutoNoVsync => "OFF (AutoNoVsync)",
                            bevy::window::PresentMode::Immediate => "OFF (Immediate)",
                            bevy::window::PresentMode::AutoVsync => "ON (AutoVsync)",
                            bevy::window::PresentMode::Fifo => "ON (Fifo VSync)",
                            bevy::window::PresentMode::FifoRelaxed => "ON (FifoRelaxed)",
                            bevy::window::PresentMode::Mailbox => "Mailbox (No tearing, uncapped)",
                        }
                    } else {
                        "Unknown"
                    };
                    console.logs.push(format!("[Framerate] Max FPS limit: {} | VSync: {}", limit_str, vsync_str));
                    console.logs.push("Usage: maxfps <fps|0|off|uncapped> (e.g. 'maxfps 60', 'maxfps 144', 'maxfps 0')".into());
                    console.logs.push("       vsync <on|off>".into());
                } else {
                    let arg = tokens[1].to_lowercase();
                    match arg.as_str() {
                        "0" | "off" | "uncap" | "uncapped" | "none" | "unlimited" => {
                            fps_limiter.target_fps = None;
                            console.logs.push("[Framerate] Max FPS limit REMOVED. Client running completely UNCAPPED.".into());
                        }
                        _ => {
                            if let Ok(fps) = arg.parse::<u32>() {
                                if fps == 0 {
                                    fps_limiter.target_fps = None;
                                    console.logs.push("[Framerate] Max FPS limit REMOVED. Client running completely UNCAPPED.".into());
                                } else if fps < 10 {
                                    console.logs.push("[Syntax Error] Minimum target FPS limit is 10.".into());
                                } else {
                                    fps_limiter.target_fps = Some(fps);
                                    console.logs.push(format!("[Framerate] Max FPS limit set to {} FPS.", fps));
                                }
                            } else {
                                console.logs.push(format!("[Syntax Error] Invalid FPS limit '{}'. Usage: maxfps <number|0|off|uncapped>", tokens[1]));
                            }
                        }
                    }
                }
            }
            "vsync" => {
                if let Ok(mut window) = window_query.get_single_mut() {
                    let sub = tokens.get(1).map(|s| s.to_lowercase());
                    match sub.as_deref() {
                        Some("on") | Some("1") | Some("true") | Some("enable") => {
                            window.present_mode = bevy::window::PresentMode::AutoVsync;
                            console.logs.push("[Window] VSync ENABLED (display refresh rate lock).".into());
                        }
                        Some("off") | Some("0") | Some("false") | Some("disable") => {
                            window.present_mode = bevy::window::PresentMode::AutoNoVsync;
                            console.logs.push("[Window] VSync DISABLED (unlocked presentation rate).".into());
                        }
                        Some("immediate") => {
                            window.present_mode = bevy::window::PresentMode::Immediate;
                            console.logs.push("[Window] Present mode set to Immediate (lowest latency, tearing possible).".into());
                        }
                        Some("mailbox") => {
                            window.present_mode = bevy::window::PresentMode::Mailbox;
                            console.logs.push("[Window] Present mode set to Mailbox (lowest latency, tear-free).".into());
                        }
                        None => {
                            let mode_str = match window.present_mode {
                                bevy::window::PresentMode::AutoNoVsync => "OFF (AutoNoVsync)",
                                bevy::window::PresentMode::Immediate => "OFF (Immediate)",
                                bevy::window::PresentMode::AutoVsync => "ON (AutoVsync)",
                                bevy::window::PresentMode::Fifo => "ON (Fifo VSync)",
                                bevy::window::PresentMode::FifoRelaxed => "ON (FifoRelaxed)",
                                bevy::window::PresentMode::Mailbox => "Mailbox (No tearing, uncapped)",
                            };
                            console.logs.push(format!("[Window] VSync is currently {}. Usage: vsync <on|off|immediate|mailbox>", mode_str));
                        }
                        _ => {
                            console.logs.push("[Syntax Error] Usage: vsync <on|off|immediate|mailbox>".into());
                        }
                    }
                } else {
                    console.logs.push("[Window Error] Primary window not found.".into());
                }
            }
            "f3" | "fps" | "diag" | "diagnostics" => {
                let sub = tokens.get(1).map(|s| s.to_lowercase());

                // If user typed 'fps <number>' or 'fps max <number>' or 'fps limit <number>' or 'fps uncap'
                let is_fps_limit_cmd = if cmd == "fps" {
                    if let Some(ref s) = sub {
                        s == "max" || s == "limit" || s == "uncap" || s == "uncapped" || s.parse::<u32>().is_ok()
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_fps_limit_cmd {
                    let target_arg = if sub.as_deref() == Some("max") || sub.as_deref() == Some("limit") {
                        tokens.get(2).map(|s| s.to_lowercase())
                    } else {
                        sub
                    };
                    match target_arg.as_deref() {
                        Some("0") | Some("off") | Some("uncap") | Some("uncapped") | Some("none") => {
                            fps_limiter.target_fps = None;
                            console.logs.push("[Framerate] Max FPS limit REMOVED. Client running completely UNCAPPED.".into());
                        }
                        Some(val) => {
                            if let Ok(target) = val.parse::<u32>() {
                                if target == 0 {
                                    fps_limiter.target_fps = None;
                                    console.logs.push("[Framerate] Max FPS limit REMOVED. Client running completely UNCAPPED.".into());
                                } else {
                                    let clamped = target.max(10);
                                    fps_limiter.target_fps = Some(clamped);
                                    console.logs.push(format!("[Framerate] Max FPS limit set to {} FPS.", clamped));
                                }
                            } else {
                                console.logs.push(format!("[Syntax Error] Usage: fps <number> or maxfps <number>"));
                            }
                        }
                        None => {
                            console.logs.push("[Syntax Error] Usage: fps <number> or maxfps <number>".into());
                        }
                    }
                } else if let Ok(mut style) = diag_pill_query.get_single_mut() {
                    match sub.as_deref() {
                        Some("on") | Some("show") | Some("1") | Some("true") => {
                            style.display = Display::Flex;
                            console.logs.push("[Diagnostics] F3 telemetry overlay toggled ON (visible).".into());
                        }
                        Some("off") | Some("hide") | Some("0") | Some("false") => {
                            style.display = Display::None;
                            console.logs.push("[Diagnostics] F3 telemetry overlay toggled OFF (hidden).".into());
                        }
                        Some("toggle") | None => {
                            let new_state = style.display == Display::None;
                            style.display = if new_state { Display::Flex } else { Display::None };
                            let action_str = if new_state { "toggled ON (visible)" } else { "toggled OFF (hidden)" };
                            console.logs.push(format!("[Diagnostics] F3 telemetry overlay {}.", action_str));
                        }
                        _ => {
                            console.logs.push("[Syntax Error] Usage: f3 [on|off|toggle] (or press [F3]) | fps <target_fps>".into());
                        }
                    }
                } else {
                    console.logs.push("[Diagnostics] F3 overlay element not found.".into());
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
            "tuner" | "weapontool" => {
                console.logs.push("[Admin] Weapon & Spell Tuner Workbench available via [F6] hotkey.".into());
            }
            "dual" => {
                if tokens.len() < 3 {
                    console.logs.push("[Syntax Error] Usage: dual <main_weapon> <off_weapon> (e.g. 'dual Sword Axe', 'dual Revolver Revolver')".into());
                } else {
                    let main_w = tokens[1].to_string();
                    let off_w = tokens[2].to_string();
                    let _ = conn.db.reducers.equip_weapon("MainHand".to_string(), main_w.clone());
                    let _ = conn.db.reducers.equip_weapon("OffHand".to_string(), off_w.clone());
                    console.logs.push(format!("[Combat] Dual-wield loadout equipped: Main='{}', Off='{}'", main_w, off_w));
                }
            }
            "equip" => {
                if tokens.len() < 2 {
                    console.logs.push("[Syntax Error] Usage: equip [main|off] <weapon_name> (defaults to main)".into());
                } else if tokens.len() == 2 {
                    let w = tokens[1].to_string();
                    let _ = conn.db.reducers.equip_weapon("MainHand".to_string(), w.clone());
                    console.logs.push(format!("[Combat] Equipped '{}' to Main-Hand", w));
                } else {
                    let slot = if tokens[1].eq_ignore_ascii_case("off") { "OffHand" } else { "MainHand" };
                    let w = tokens[2..].join(" ");
                    let _ = conn.db.reducers.equip_weapon(slot.to_string(), w.clone());
                    console.logs.push(format!("[Combat] Equipped '{}' to {}", w, slot));
                }
            }
            "skills" | "skill" => {
                console.logs.push("[Skills] Character Skills Sheet (Valheim style) toggleable with [L] hotkey.".into());
            }
            "spellbook" | "spells" => {
                console.logs.push("[Spellbook] Spellbook & Ability Grimoire (WoW style) toggleable with [K] hotkey.".into());
            }
            "crosshair" => {
                console.logs.push("[Settings] Press [F7] to open the interactive Crosshair Tuner GUI.".into());
                console.logs.push("[Settings] Supports custom colors, thickness, length, gap, dot, outline, and Dynamic/Static lock.".into());
            }
            "abilities" | "ability" => {
                console.logs.push("--- TACTICAL ABILITIES DIRECTORY ---".into());
                console.logs.push("[Q] Phase Dash  : Instant directional horizontal thrust (6.0s CD)".into());
                console.logs.push("[C] Smoke Veil  : Obscures vision / blocks lines of sight (14.0s CD)".into());
                console.logs.push("[E] Intel Dart  : Sonar pulse reconnaissance (16.0s CD)".into());
                console.logs.push("[F] Grav-Lift   : Kinetic vertical air lift (10.0s CD)".into());
            }
            "star" | "stara" | "starb" => {
                if let Some(ref mut cfg) = sky_config {
                    let target_star = if cmd == "stara" {
                        Some("a")
                    } else if cmd == "starb" {
                        Some("b")
                    } else {
                        tokens.get(1).map(|s| s.to_lowercase()).and_then(|s| {
                            if s == "a" || s == "primary" {
                                Some("a")
                            } else if s == "b" || s == "secondary" {
                                Some("b")
                            } else {
                                None
                            }
                        })
                    };

                    let sub_tokens: Vec<&str> = if cmd == "stara" || cmd == "starb" {
                        tokens[1..].to_vec()
                    } else if target_star.is_some() {
                        tokens[2..].to_vec()
                    } else {
                        tokens[1..].to_vec()
                    };

                    if let Some(star) = target_star {
                        if sub_tokens.is_empty() {
                            if star == "a" {
                                console.logs.push(format!("[BinarySky] Star A (Primary): {:.0} lx | Shadows: {} | Color: {:?}",
                                    cfg.star_a_base_illuminance_lux, cfg.star_a_shadows_enabled, cfg.star_a_color_override));
                            } else {
                                console.logs.push(format!("[BinarySky] Star B (Secondary): {:.0} lx | Shadows: {} | Color: {:?}",
                                    cfg.star_b_base_illuminance_lux, cfg.star_b_shadows_enabled, cfg.star_b_color_override));
                            }
                            console.logs.push("Usage: star <a|b> <lux> | star <a|b> color <hex|name> | star <a|b> shadows <on|off>".into());
                        } else if let Ok(lux) = sub_tokens[0].parse::<f32>() {
                            if star == "a" {
                                cfg.star_a_base_illuminance_lux = lux.max(0.0);
                                console.logs.push(format!("[BinarySky] Star A base illuminance set to {:.0} lx.", cfg.star_a_base_illuminance_lux));
                            } else {
                                cfg.star_b_base_illuminance_lux = lux.max(0.0);
                                console.logs.push(format!("[BinarySky] Star B base illuminance set to {:.0} lx.", cfg.star_b_base_illuminance_lux));
                            }
                        } else if sub_tokens[0] == "color" || sub_tokens[0] == "col" {
                            let color_arg = sub_tokens.get(1).copied().unwrap_or("");
                            if color_arg.eq_ignore_ascii_case("reset") || color_arg.is_empty() {
                                if star == "a" {
                                    cfg.star_a_color_override = None;
                                    console.logs.push("[BinarySky] Star A color reset to physically based Rayleigh/Planck blackbody.".into());
                                } else {
                                    cfg.star_b_color_override = None;
                                    console.logs.push("[BinarySky] Star B color reset to physically based Mie/Planck blackbody.".into());
                                }
                            } else if let Some(parsed_color) = crate::binary_sky::parse_color_spec(color_arg) {
                                if star == "a" {
                                    cfg.star_a_color_override = Some(parsed_color);
                                    console.logs.push(format!("[BinarySky] Star A direct color override set to {:?}", parsed_color));
                                } else {
                                    cfg.star_b_color_override = Some(parsed_color);
                                    console.logs.push(format!("[BinarySky] Star B direct color override set to {:?}", parsed_color));
                                }
                            } else {
                                console.logs.push(format!("[Syntax Error] Unknown color '{}'. Try hex (e.g. ff5400, 390099) or names (blaze, amber, fuchsia, navy, raspberry, white, reset).", color_arg));
                            }
                        } else if sub_tokens[0] == "shadow" || sub_tokens[0] == "shadows" {
                            let on = sub_tokens.get(1).map(|s| *s != "off" && *s != "0" && *s != "false").unwrap_or(true);
                            if star == "a" {
                                cfg.star_a_shadows_enabled = on;
                                console.logs.push(format!("[BinarySky] Star A shadows: {}", if on { "ENABLED" } else { "DISABLED" }));
                            } else {
                                cfg.star_b_shadows_enabled = on;
                                console.logs.push(format!("[BinarySky] Star B shadows: {}", if on { "ENABLED" } else { "DISABLED" }));
                            }
                        } else {
                            console.logs.push("[Syntax Error] Usage: star <a|b> <lux> | star <a|b> color <hex|name> | star <a|b> shadows <on|off>".into());
                        }
                    } else if !sub_tokens.is_empty() && (sub_tokens[0] == "shadow" || sub_tokens[0] == "shadows") {
                        let on = sub_tokens.get(1).map(|s| *s != "off" && *s != "0" && *s != "false").unwrap_or(true);
                        cfg.star_a_shadows_enabled = on;
                        cfg.star_b_shadows_enabled = on;
                        console.logs.push(format!("[BinarySky] Dual star directional shadows: {}", if on { "ENABLED" } else { "DISABLED" }));
                    } else if !sub_tokens.is_empty() && (sub_tokens[0] == "balance" || sub_tokens[0] == "equal") {
                        let ratio = sub_tokens.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.5).clamp(0.1, 0.9);
                        let total = cfg.star_a_base_illuminance_lux + cfg.star_b_base_illuminance_lux;
                        cfg.star_a_base_illuminance_lux = total * (1.0 - ratio);
                        cfg.star_b_base_illuminance_lux = total * ratio;
                        console.logs.push(format!("[BinarySky] Balanced stars with ratio {:.2}: Star A = {:.0} lx, Star B = {:.0} lx",
                            ratio, cfg.star_a_base_illuminance_lux, cfg.star_b_base_illuminance_lux));
                    } else {
                        console.logs.push(format!("[BinarySky Status] Star A: {:.0} lx (shadows: {}) | Star B: {:.0} lx (shadows: {}) | Ambient: {:.0} lx",
                            cfg.star_a_base_illuminance_lux, cfg.star_a_shadows_enabled,
                            cfg.star_b_base_illuminance_lux, cfg.star_b_shadows_enabled,
                            cfg.ambient_illuminance_lux.unwrap_or(360.0)));
                        console.logs.push("Commands: star <a|b> <lux> | star <a|b> color <hex> | star <a|b> shadows <on|off> | dualshadows".into());
                    }
                } else {
                    console.logs.push("[Notice] Binary sky system is active in client world.".into());
                }
            }
            "dualshadows" | "dualshadow" => {
                if let Some(ref mut cfg) = sky_config {
                    cfg.star_a_base_illuminance_lux = 70_000.0;
                    cfg.star_b_base_illuminance_lux = 55_000.0;
                    cfg.star_a_color_override = Some(Color::srgb(1.0, 0.98, 0.92));
                    cfg.star_b_color_override = Some(crate::binary_sky::parse_color_spec("ff5400").unwrap());
                    cfg.star_a_shadows_enabled = true;
                    cfg.star_b_shadows_enabled = true;
                    cfg.ambient_illuminance_lux = Some(220.0);
                    console.logs.push("[BinarySky] Dual shadow maps activated with high-contrast penumbras!".into());
                    console.logs.push("  - Host Star A: 70k lx (Crisp solar white, sharp shadow)".into());
                    console.logs.push("  - Companion Star B: 55k lx (Blaze orange #ff5400, warm shadow)".into());
                    console.logs.push("  - Ambient Fill: 220 lx (Preserves deep colored penumbra cross-shadows)".into());
                }
            }
            "ambient" => {
                if let Some(ref mut cfg) = sky_config {
                    if let Some(arg) = tokens.get(1) {
                        if arg.eq_ignore_ascii_case("reset") || arg.eq_ignore_ascii_case("auto") {
                            cfg.ambient_illuminance_lux = None;
                            console.logs.push("[BinarySky] Ambient light reset to automatic atmospheric scattering scaling (2.5 - 360 lx).".into());
                        } else if let Ok(lux) = arg.parse::<f32>() {
                            cfg.ambient_illuminance_lux = Some(lux.max(0.0));
                            console.logs.push(format!("[BinarySky] Ambient light override set to {:.0} lx. (Lower values make dual shadows deeper)", lux));
                        } else {
                            console.logs.push("[Syntax Error] Usage: ambient <lux> (e.g. ambient 200, ambient 450, ambient reset)".into());
                        }
                    } else {
                        let cur = cfg.ambient_illuminance_lux.map(|v| format!("{:.0} lx (override)", v)).unwrap_or_else(|| "Auto (~2.5 - 360 lx)".into());
                        console.logs.push(format!("[BinarySky] Current ambient light: {}. Usage: ambient <lux> (e.g. 200) | ambient reset", cur));
                    }
                }
            }
            "starsize" => {
                if let Some(ref mut cfg) = sky_config {
                    if let Some(scale) = tokens.get(1).and_then(|s| s.parse::<f32>().ok()) {
                        cfg.starfield_scale = scale.clamp(0.05, 5.0);
                        console.logs.push(format!("[BinarySky] Cosmic starfield point size scale set to {:.2}x (pinpoint stars).", cfg.starfield_scale));
                    } else {
                        console.logs.push(format!("[BinarySky] Current starfield scale: {:.2}x. Usage: starsize <0.1 - 3.0> (e.g. starsize 0.5, starsize 1.0)", cfg.starfield_scale));
                    }
                }
            }
            "sunset" => {
                let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                if let Some(ref mut eph) = ephemeris {
                    // 18.2 hours: Low setting solar contact (Blaze Orange #ff5400 & Hot Fuchsia #ff0054)
                    let frac = ((18.2_f64 / 24.0) - 0.5).rem_euclid(1.0);
                    eph.simulation_time_seconds = duration * frac;
                    eph.diurnal_angle = (frac * 2.0 * std::f64::consts::PI) as f32;
                }
                let _ = conn.db.reducers.admin_set_time(18.2);
                console.logs.push("[Admin] Time set to SUNSET (18.2h). Horizon glowing in Blaze Orange (#ff5400) & Hot Fuchsia (#ff0054).".into());
            }
            "dusk" | "twilight" => {
                let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                if let Some(ref mut eph) = ephemeris {
                    // 19.3 hours: Nautical twilight (Dark Raspberry #9e0059 & Navy Electric #390099)
                    let frac = ((19.3_f64 / 24.0) - 0.5).rem_euclid(1.0);
                    eph.simulation_time_seconds = duration * frac;
                    eph.diurnal_angle = (frac * 2.0 * std::f64::consts::PI) as f32;
                }
                let _ = conn.db.reducers.admin_set_time(19.3);
                console.logs.push("[Admin] Time set to DUSK (19.3h). Horizon glowing in Dark Raspberry (#9e0059) & Navy Electric (#390099).".into());
            }
            "dawn" => {
                let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                if let Some(ref mut eph) = ephemeris {
                    // 05.8 hours: Sunrise (Navy Electric into Blaze Orange & Amber Gold)
                    let frac = ((5.8_f64 / 24.0) - 0.5).rem_euclid(1.0);
                    eph.simulation_time_seconds = duration * frac;
                    eph.diurnal_angle = (frac * 2.0 * std::f64::consts::PI) as f32;
                }
                let _ = conn.db.reducers.admin_set_time(5.8);
                console.logs.push("[Admin] Time set to DAWN (05.8h). Sunrise palette: Navy Electric -> Hot Fuchsia -> Blaze Orange -> Amber Gold.".into());
            }
            "palette" => {
                console.logs.push("--- S-TYPE ATMOSPHERIC COLOR FAMILY ---".into());
                console.logs.push("  --navy-electric:   #390099 (Astronomical night / cosmic starlight)".into());
                console.logs.push("  --dark-raspberry:  #9e0059 (Nautical twilight / Belt of Venus)".into());
                console.logs.push("  --hot-fuchsia:     #ff0054 (Civil dusk horizon glow)".into());
                console.logs.push("  --blaze-orange:    #ff5400 (Low setting solar contact)".into());
                console.logs.push("  --amber-gold:      #ffbd00 (Golden hour atmospheric scattering)".into());
                console.logs.push("Commands: 'sunset' (18.2h), 'dusk' (19.3h), 'night' (00:00), 'noon' (12:00), 'dawn' (05.8h)".into());
            }
            "help" => {
                console.logs.push("--- PLAYTESTING COMMAND DIRECTORY ---".into());
                console.logs.push("giveitem <Item> [amt]  : Grants item (Press [Tab] to auto-fill)".into());
                console.logs.push("tp <x> <z>             : Teleports player to world coordinate".into());
                console.logs.push("heal [amt]             : Restores player health points".into());
                console.logs.push("god                    : Sets health to 99999 HP".into());
                console.logs.push("time <0-24>            : Sets in-game world clock ([ [ ] and [ ] ])".into());
                console.logs.push("day / noon / night     : Quick toggle High Noon / Midnight [F8]".into());
                console.logs.push("sunset / dusk / dawn   : Jumps to sunset & twilight palette transitions".into());
                console.logs.push("palette                : Prints active S-type atmospheric color family".into());
                console.logs.push("star <a|b> <lux>       : Sets Star A or Star B illuminance in lux".into());
                console.logs.push("star <a|b> color <hex> : Sets Star A/B direct light color override".into());
                console.logs.push("star <a|b> shadows <on>: Toggles shadow casting for Star A or B".into());
                console.logs.push("dualshadows            : Activates high-contrast dual shadow maps preset".into());
                console.logs.push("ambient <lux>          : Adjusts ambient light fill (deeper shadows)".into());
                console.logs.push("starsize <scale>       : Scales celestial starfield points of light".into());
                console.logs.push("weather <clear|aurora> : Sets atmospheric weather preset [F9]".into());
                console.logs.push("aurora / rain / haze   : Direct weather command shortcuts".into());
                console.logs.push("hud [on|off|toggle]    : Toggles celestial clock & weather HUD pill [F10]".into());
                console.logs.push("f3 / fps [on|off]       : Toggles performance diagnostics HUD [F3]".into());
                console.logs.push("timescale <speed>      : Sets cycle rate (e.g. 1.0, 60.0) [ - / = ]".into());
                console.logs.push("spawn <mob> [amt]      : Spawns Deer, Boar, Goblin, Peasant".into());
                console.logs.push("nuke [radius]          : Demolishes terrain with spherical blast".into());
                console.logs.push("clearinv               : Empties inventory slots completely".into());
                console.logs.push("killall                : Destroys all active NPC brains".into());
                console.logs.push("tuner / weapontool     : Opens Weapon & Spell Tuner [F6]".into());
                console.logs.push("res [w h | preset]     : Sets resolution (e.g. res 1080p, res 1920 1080)".into());
                console.logs.push("fullscreen / windowed  : Toggles or sets fullscreen / windowed display mode".into());
                console.logs.push("maxfps <fps|0|off>     : Sets frame rate cap (0 or off = uncapped)".into());
                console.logs.push("vsync <on|off>         : Toggles vertical sync (AutoNoVsync default)".into());
                console.logs.push("abilities              : Displays tactical abilities directory".into());
            }
            "resolution" | "res" => {
                if let Ok(mut window) = window_query.get_single_mut() {
                    if tokens.len() == 1 {
                        let cur_w = window.resolution.width();
                        let cur_h = window.resolution.height();
                        let phys_w = window.resolution.physical_width();
                        let phys_h = window.resolution.physical_height();
                        let mode_str = match window.mode {
                            WindowMode::Windowed => "Windowed",
                            WindowMode::BorderlessFullscreen => "Borderless Fullscreen",
                            WindowMode::Fullscreen => "Exclusive Fullscreen",
                            _ => "Other",
                        };
                        console.logs.push(format!("[Window] Resolution: {:.0}x{:.0} (Physical: {}x{}), Mode: {}", cur_w, cur_h, phys_w, phys_h, mode_str));
                        console.logs.push("[Window] Usage: res <w> <h> | res <720p|1080p|1440p|4k> | fullscreen | windowed".into());
                    } else if tokens.len() == 2 {
                        let preset = tokens[1].to_lowercase();
                        let (w, h) = match preset.as_str() {
                            "720p" | "720" => (1280.0, 720.0),
                            "1080p" | "1080" | "fhd" => (1920.0, 1080.0),
                            "1440p" | "1440" | "2k" | "qhd" => (2560.0, 1440.0),
                            "4k" | "2160p" | "2160" | "uhd" => (3840.0, 2160.0),
                            _ => (0.0, 0.0),
                        };
                        if w > 0.0 {
                            window.resolution.set(w, h);
                            console.logs.push(format!("[Window] Set resolution to {:.0}x{:.0} ({})", w, h, preset));
                        } else {
                            console.logs.push(format!("[Syntax Error] Unknown preset '{}'. Use 720p, 1080p, 1440p, 4k or 'res <width> <height>'", tokens[1]));
                        }
                    } else if tokens.len() >= 3 {
                        if let (Ok(w), Ok(h)) = (tokens[1].parse::<f32>(), tokens[2].parse::<f32>()) {
                            if w >= 640.0 && h >= 360.0 {
                                window.resolution.set(w, h);
                                console.logs.push(format!("[Window] Set resolution to {:.0}x{:.0}", w, h));
                            } else {
                                console.logs.push("[Syntax Error] Minimum resolution is 640x360.".into());
                            }
                        } else {
                            console.logs.push("[Syntax Error] Width and height must be valid numbers.".into());
                        }
                    }
                } else {
                    console.logs.push("[Window Error] Primary window not found.".into());
                }
            }
            "fullscreen" | "fs" => {
                if let Ok(mut window) = window_query.get_single_mut() {
                    match window.mode {
                        WindowMode::Windowed => {
                            window.mode = WindowMode::BorderlessFullscreen;
                            console.logs.push("[Window] Display mode set to Borderless Fullscreen.".into());
                        }
                        _ => {
                            window.mode = WindowMode::Windowed;
                            console.logs.push("[Window] Display mode set to Windowed.".into());
                        }
                    }
                } else {
                    console.logs.push("[Window Error] Primary window not found.".into());
                }
            }
            "windowed" | "win" => {
                if let Ok(mut window) = window_query.get_single_mut() {
                    window.mode = WindowMode::Windowed;
                    console.logs.push("[Window] Display mode set to Windowed.".into());
                } else {
                    console.logs.push("[Window Error] Primary window not found.".into());
                }
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

pub fn ui_node_screen_rect(transform: &GlobalTransform, node: &Node, window: &Window) -> Rect {
    let screen_center = Vec2::new(
        transform.translation().x + window.width() * 0.5,
        -transform.translation().y + window.height() * 0.5,
    );
    Rect::from_center_size(screen_center, node.size())
}

// ----------------------------------------------------------------------------
// INVENTORY DRAG-AND-DROP & WORLD DROP SYSTEMS
// ----------------------------------------------------------------------------

pub fn handle_inventory_drag_and_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    slot_query: Query<(&InventorySlotIndex, &GlobalTransform, &Node)>,
    main_hand_slot_q: Query<(&GlobalTransform, &Node), With<PaperdollMainHandSlot>>,
    off_hand_slot_q: Query<(&GlobalTransform, &Node), With<PaperdollOffHandSlot>>,
    bag_slot_query: Query<(&PaperdollBagSlotIndex, &GlobalTransform, &Node)>,
    inv_root_query: Query<(&GlobalTransform, &Node), With<InventoryUiRoot>>,
    mut drag_drop: ResMut<DragDropState>,
    conn: Res<SpacetimeConnection>,
    cached_player: Res<CachedPlayerEntity>,
    hand_side: Res<EquippedHandSide>,
    mut equipped_bags: ResMut<ClientEquippedBags>,
) {
    let Ok(window) = window_query.get_single() else { return; };
    let Some(cursor_pos) = window.cursor_position() else { return; };

    let Some(player_id) = cached_player.0 else { return; };
    let Some(inv) = conn.db.db.inventory().entity_id().find(&player_id) else { return; };

    // Right-Click to Quick-Equip Weapon or Bag
    if mouse.just_pressed(MouseButton::Right) {
        for (slot_idx, transform, node) in slot_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            if rect.contains(cursor_pos) {
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
                        } else {
                            let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                            let is_two_handed = crate::weapons::WeaponType::from_item_name(Some(&item_name)).is_two_handed();
                            let loadout = conn.db.db.equipment_loadout().entity_id().find(&player_id);
                            let main_equipped = loadout.as_ref().map(|l| l.main_hand.as_str()).unwrap_or("None");
                            let off_equipped = loadout.as_ref().map(|l| l.off_hand.as_str()).unwrap_or("None");

                            let target_hand = if is_two_handed {
                                "MainHand"
                            } else if shift_held || hand_side.0 == HandSide::Left {
                                "OffHand"
                            } else if main_equipped != "None" && !main_equipped.is_empty() && (off_equipped == "None" || off_equipped.is_empty()) {
                                "OffHand"
                            } else {
                                "MainHand"
                            };

                            info!("Quick-Equipping '{}' to {}", item_name, target_hand);
                            let _ = conn.db.reducers.equip_weapon(target_hand.to_string(), item_name);
                        }
                        return;
                    }
                }
            }
        }
    }

    if mouse.just_pressed(MouseButton::Left) {
        for (slot_idx, transform, node) in slot_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
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

        // 1. Check if released on another inventory bag slot
        let mut target_slot = None;
        for (slot_idx, transform, node) in slot_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            if rect.contains(cursor_pos) {
                target_slot = Some(slot_idx.0);
                break;
            }
        }

        // 2. Check if released over MainHand Paperdoll slot
        let mut dropped_on_main = false;
        for (transform, node) in main_hand_slot_q.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            if rect.contains(cursor_pos) {
                dropped_on_main = true;
                break;
            }
        }

        // 3. Check if released over OffHand Paperdoll slot
        let mut dropped_on_off = false;
        for (transform, node) in off_hand_slot_q.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            if rect.contains(cursor_pos) {
                dropped_on_off = true;
                break;
            }
        }

        // 4. Check if released over Paperdoll Bag slot
        let mut dropped_on_bag = None;
        for (bag_slot_idx, transform, node) in bag_slot_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            if rect.contains(cursor_pos) {
                dropped_on_bag = Some(bag_slot_idx.0);
                break;
            }
        }

        // 5. Check if released anywhere inside the Inventory window
        let mut inside_inventory_window = false;
        for (transform, node) in inv_root_query.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            if rect.contains(cursor_pos) {
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
            info!("Paperdoll: Dragged item '{}' into MainHand", drag_drop.item_type);
            if let Err(e) = conn.db.reducers.equip_weapon("MainHand".to_string(), drag_drop.item_type.clone()) {
                error!("Failed to equip item to MainHand: {:?}", e);
            }
        } else if dropped_on_off {
            info!("Paperdoll: Dragged item '{}' into OffHand", drag_drop.item_type);
            if let Err(e) = conn.db.reducers.equip_weapon("OffHand".to_string(), drag_drop.item_type.clone()) {
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
            info!("Paperdoll: Unequipping MainHand");
            let _ = conn.db.reducers.unequip_weapon("MainHand".to_string());
        }
    }

    for interaction in unequip_off_q.iter() {
        if *interaction == Interaction::Pressed {
            info!("Paperdoll: Unequipping OffHand");
            let _ = conn.db.reducers.unequip_weapon("OffHand".to_string());
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

pub fn update_celestial_hud_ui(
    ephemeris: Option<Res<crate::binary_sky::BinaryEphemerisState>>,
    weather: Option<Res<crate::binary_sky::AtmosphericWeather>>,
    root_q: Query<&Style, With<CelestialHudRoot>>,
    mut text_q: Query<&mut Text, With<CelestialHudText>>,
) {
    if let Ok(style) = root_q.get_single() {
        if style.display == Display::None {
            return;
        }
    }

    let (Some(eph), Some(wth)) = (ephemeris, weather) else { return; };
    let Ok(mut text) = text_q.get_single_mut() else { return; };

    let hours = eph.clock_time_hours();
    let h = hours.floor() as u32;
    let m = ((hours.fract()) * 60.0).floor() as u32;

    let icon = match eph.sky_state {
        crate::binary_sky::DynamicSkyState::DualDay => "Dual Day",
        crate::binary_sky::DynamicSkyState::StarAPrimaryDay => "Day",
        crate::binary_sky::DynamicSkyState::StarBSecondaryDay => "Dwarf Day",
        crate::binary_sky::DynamicSkyState::BinaryAlignment => "Conjunction",
        crate::binary_sky::DynamicSkyState::CivilTwilight | crate::binary_sky::DynamicSkyState::NauticalTwilight => "Twilight",
        crate::binary_sky::DynamicSkyState::TrueNight => "Night",
    };

    let weather_str = match wth.weather_type {
        crate::binary_sky::WeatherType::ClearSky => "Clear Sky",
        crate::binary_sky::WeatherType::AerosolHaze => "Aerosol Haze",
        crate::binary_sky::WeatherType::StellarWindAurora => "Aurora Active",
        crate::binary_sky::WeatherType::OvercastPrecipitation => "Overcast Rain",
    };

    let formatted = format!("{:02}:{:02} ({}) | {} [F8/F9]", h, m, icon, weather_str);
    if text.sections[0].value != formatted {
        text.sections[0].value = formatted;
    }
}

/// Allows toggling the celestial clock & weather HUD pill with [F10] when outside the console.
pub fn toggle_celestial_hud_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    mut hud_pill_query: Query<&mut Style, With<CelestialHudRoot>>,
    console: Res<ConsoleState>,
) {
    if !console.is_open && keys.just_pressed(KeyCode::F10) {
        if let Ok(mut style) = hud_pill_query.get_single_mut() {
            style.display = if style.display == Display::None {
                Display::Flex
            } else {
                Display::None
            };
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
    hand_side: Res<EquippedHandSide>,
    equipped_bags: Res<ClientEquippedBags>,
    mut header_q: Query<&mut Text, (With<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
    mut name_q: Query<(&mut Text, &InventorySlotName), Without<InventorySlotCount>>,
    mut count_q: Query<(&mut Text, &InventorySlotCount), Without<InventorySlotName>>,
    mut primary_hand_text_q: Query<&mut Text, (With<PaperdollPrimaryHandText>, Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
    mut main_hand_text_q: Query<&mut Text, (With<PaperdollMainHandText>, Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
    mut off_hand_text_q: Query<&mut Text, (With<PaperdollOffHandText>, Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
    mut bag_text_q: Query<(&mut Text, &PaperdollBagText), (Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagTooltip>)>,
    mut bag_tooltip_q: Query<(&mut Text, &PaperdollBagTooltip), (Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<InventoryCapacityHeader>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>)>,
    mut capacity_header_q: Query<&mut Text, (With<InventoryCapacityHeader>, Without<WorkbenchHeaderStatus>, Without<InventorySlotName>, Without<InventorySlotCount>, Without<PaperdollPrimaryHandText>, Without<PaperdollMainHandText>, Without<PaperdollOffHandText>, Without<PaperdollBagText>, Without<PaperdollBagTooltip>)>,
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

            // Sync Paperdoll Equipment Loadout
            let loadout = conn.db.db.equipment_loadout().entity_id().find(&player.entity_id);
            let main_weapon = loadout.as_ref().map(|l| l.main_hand.as_str()).unwrap_or("None");
            let off_weapon = loadout.as_ref().map(|l| l.off_hand.as_str()).unwrap_or("None");

            for mut text in main_hand_text_q.iter_mut() {
                text.sections[0].value = if main_weapon == "None" || main_weapon.is_empty() {
                    "MainHand: [Unarmed]".to_string()
                } else {
                    format!("MainHand: {}", main_weapon)
                };
            }

            for mut text in off_hand_text_q.iter_mut() {
                text.sections[0].value = if off_weapon == "None" || off_weapon.is_empty() {
                    "OffHand: [Empty]".to_string()
                } else {
                    format!("OffHand: {}", off_weapon)
                };
            }

            for mut text in primary_hand_text_q.iter_mut() {
                text.sections[0].value = match hand_side.0 {
                    HandSide::Right => "[⇄] PRIMARY HAND: RIGHT [H]".to_string(),
                    HandSide::Left => "[⇄] PRIMARY HAND: LEFT [H]".to_string(),
                };
            }

            // Sync Bags & Capacity
            for (mut text, b_idx) in bag_text_q.iter_mut() {
                if let Some(ref bag) = equipped_bags.bags[b_idx.0] {
                    text.sections[0].value = format!("Bag {}: {}", b_idx.0 + 1, bag.name);
                } else {
                    text.sections[0].value = format!("Bag {}: [Empty Bag Slot]", b_idx.0 + 1);
                }
            }

            for (mut text, b_idx) in bag_tooltip_q.iter_mut() {
                if let Some(ref bag) = equipped_bags.bags[b_idx.0] {
                    text.sections[0].value = format!("+{} Slots | Cap: {:?} | {}% WR", bag.capacity, bag.size_cap, bag.weight_reduction_pct);
                    text.sections[0].style.color = Color::srgb(0.4, 0.9, 0.5);
                } else {
                    text.sections[0].value = "Right-click bag in inventory to equip".to_string();
                    text.sections[0].style.color = Color::srgb(0.65, 0.70, 0.65);
                }
            }

            let extra_slots: usize = equipped_bags.bags.iter().filter_map(|b| b.as_ref()).map(|b| b.capacity).sum();
            let total_cap = 16 + extra_slots;
            let used_slots = inventory.slots.iter().filter(|s| s.count > 0 && !s.item_type.is_empty()).count();
            let free_slots = total_cap.saturating_sub(used_slots);

            for mut text in capacity_header_q.iter_mut() {
                text.sections[0].value = format!("BAG INVENTORY (Free: {} / {} Slots)", free_slots, total_cap);
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
    
    // AI_RULES.md Rule 2.1 #3: BTreeMap and BTreeSet guarantee deterministic ordering without randomized SipHash
    let health_map: std::collections::BTreeMap<u64, f32> = conn.db.db.health().iter()
        .map(|h| (h.entity_id, (h.current / h.max).clamp(0.0, 1.0)))
        .collect();

    let mut tracked_units = std::collections::BTreeSet::new();

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

// ----------------------------------------------------------------------------
// RETICLE-ADJACENT FIGHTING HUD & CROSSHAIR SYSTEMS
// ----------------------------------------------------------------------------
// Architectural Note: Renders the reticle-adjacent fighting interface and handles
// unrestricted crosshair customization (dynamic spread expansion vs competitive
// static lock, custom thickness, length, gap, dot, outline, and high-contrast palettes).

pub fn update_reticle_crosshair_ui(
    settings: Res<CrosshairSettings>,
    weapon_state: Res<crate::weapons::WeaponState>,
    camera_mode: Res<State<CameraMode>>,
    player_q: Query<&avian3d::prelude::LinearVelocity, With<PlayerBody>>,
    mut root_q: Query<&mut Visibility, With<ReticleHudRoot>>,
    mut arms_q: Query<(&ReticleCrosshairArm, &mut Style, &mut BackgroundColor, &mut BorderColor), Without<ReticleCrosshairDot>>,
    mut dot_q: Query<(&mut Style, &mut BackgroundColor), (With<ReticleCrosshairDot>, Without<ReticleCrosshairArm>)>,
) {
    let Ok(mut root_vis) = root_q.get_single_mut() else { return; };
    if *camera_mode.get() != CameraMode::FPS || !settings.enabled {
        *root_vis = Visibility::Hidden;
        return;
    }
    *root_vis = Visibility::Inherited;

    let base_color = settings.color_preset.to_color().with_alpha(settings.opacity);
    let border_color = if settings.outline {
        Color::BLACK.with_alpha(settings.opacity)
    } else {
        Color::NONE
    };

    let effective_gap = if settings.is_dynamic {
        let speed = player_q.get_single().map_or(0.0, |v| v.0.length());
        let vel_spread = (speed * 0.8).min(8.0);
        let bloom_spread = weapon_state.dynamic_bloom;
        settings.gap + vel_spread + bloom_spread
    } else {
        settings.gap
    };

    let thick = settings.thickness;
    let len = settings.length;
    let outline_thick = if settings.outline { settings.outline_thickness } else { 0.0 };

    for (arm, mut style, mut bg, mut bc) in arms_q.iter_mut() {
        *bg = base_color.into();
        *bc = border_color.into();
        style.border = UiRect::all(Val::Px(outline_thick));

        match arm.0 {
            CrosshairArmDir::Top => {
                style.width = Val::Px(thick);
                style.height = Val::Px(len);
                style.left = Val::Px(-thick / 2.0);
                style.top = Val::Px(-effective_gap - len);
            }
            CrosshairArmDir::Bottom => {
                style.width = Val::Px(thick);
                style.height = Val::Px(len);
                style.left = Val::Px(-thick / 2.0);
                style.top = Val::Px(effective_gap);
            }
            CrosshairArmDir::Left => {
                style.width = Val::Px(len);
                style.height = Val::Px(thick);
                style.left = Val::Px(-effective_gap - len);
                style.top = Val::Px(-thick / 2.0);
            }
            CrosshairArmDir::Right => {
                style.width = Val::Px(len);
                style.height = Val::Px(thick);
                style.left = Val::Px(effective_gap);
                style.top = Val::Px(-thick / 2.0);
            }
        }
    }

    if let Ok((mut dot_style, mut dot_bg)) = dot_q.get_single_mut() {
        if settings.dot {
            dot_style.display = Display::Flex;
            dot_style.width = Val::Px(settings.dot_size);
            dot_style.height = Val::Px(settings.dot_size);
            dot_style.left = Val::Px(-settings.dot_size / 2.0);
            dot_style.top = Val::Px(-settings.dot_size / 2.0);
            *dot_bg = base_color.into();
        } else {
            dot_style.display = Display::None;
        }
    }
}

pub fn update_reticle_adjacent_hud(
    weapon_state: Res<crate::weapons::WeaponState>,
    conn: Res<SpacetimeConnection>,
    camera_mode: Res<State<CameraMode>>,
    mut ammo_text_q: Query<&mut Text, (With<ReticleAmmoText>, Without<ReticleCriticalHealthAlert>)>,
    mut bow_bar_q: Query<(&mut Style, &mut BackgroundColor), With<ReticleBowChargeBar>>,
    mut crit_alert_q: Query<&mut Text, (With<ReticleCriticalHealthAlert>, Without<ReticleAmmoText>)>,
) {
    if *camera_mode.get() != CameraMode::FPS { return; }

    // 1. Reticle-Adjacent Ammo Gauge (Right of Crosshair)
    if let Ok(mut text) = ammo_text_q.get_single_mut() {
        let val = match weapon_state.current_weapon {
            crate::weapons::WeaponType::Revolver => {
                if weapon_state.revolver_is_reloading {
                    "[ RELOADING ]".to_string()
                } else {
                    format!("[ {} / {} ]", weapon_state.revolver_ammo, weapon_state.revolver_max_ammo)
                }
            }
            crate::weapons::WeaponType::Shotgun => {
                if weapon_state.shotgun_is_reloading {
                    "[ RELOADING ]".to_string()
                } else if weapon_state.shotgun_is_pumping {
                    "[ PUMPING ]".to_string()
                } else {
                    format!("[ {} / {} ]", weapon_state.shotgun_ammo, weapon_state.shotgun_max_ammo)
                }
            }
            crate::weapons::WeaponType::Crossbow => {
                if weapon_state.crossbow_loaded {
                    "[ BOLT READY ]".to_string()
                } else {
                    format!("[ CRANK {:.1}s ]", weapon_state.crossbow_reload_timer.remaining_secs())
                }
            }
            crate::weapons::WeaponType::HandCrossbow => {
                if weapon_state.hand_crossbow_loaded {
                    "[ READY ]".to_string()
                } else {
                    "[ RELOADING ]".to_string()
                }
            }
            crate::weapons::WeaponType::Bow => {
                if weapon_state.bow_drawing {
                    format!("[ DRAW: {}% ]", (weapon_state.bow_charge * 100.0) as u32)
                } else {
                    "[ READY ]".to_string()
                }
            }
            crate::weapons::WeaponType::None => "".to_string(),
            _ => "[ READY ]".to_string(),
        };

        let color = match weapon_state.current_weapon {
            crate::weapons::WeaponType::Revolver if weapon_state.revolver_ammo == 0 => Color::srgb(1.0, 0.2, 0.2),
            crate::weapons::WeaponType::Revolver if weapon_state.revolver_ammo <= 2 => Color::srgb(1.0, 0.7, 0.1),
            crate::weapons::WeaponType::Shotgun if weapon_state.shotgun_ammo == 0 => Color::srgb(1.0, 0.2, 0.2),
            crate::weapons::WeaponType::Shotgun if weapon_state.shotgun_ammo == 1 => Color::srgb(1.0, 0.7, 0.1),
            _ => Color::srgb(0.0, 1.0, 1.0),
        };

        text.sections[0].value = val;
        text.sections[0].style.color = color;
    }

    // 2. Bow Charge Bar (Directly below crosshair)
    if let Ok((mut bar_style, mut bar_bg)) = bow_bar_q.get_single_mut() {
        if weapon_state.current_weapon == crate::weapons::WeaponType::Bow && weapon_state.bow_drawing {
            bar_style.display = Display::Flex;
            bar_style.width = Val::Px(weapon_state.bow_charge * 40.0);
            *bar_bg = Color::srgb(1.0, 0.85, 0.2).into();
        } else {
            bar_style.display = Display::None;
        }
    }

    // 3. Reticle-Adjacent Critical Health Warning (Decluttered Periphery)
    if let Ok(mut text) = crit_alert_q.get_single_mut() {
        let mut alert_str = String::new();
        let mut alert_col = Color::srgb(1.0, 0.2, 0.2);

        if let Some(identity) = &conn.identity {
            if let Some(player) = conn.db.db.player().identity().find(identity) {
                if let Some(hp) = conn.db.db.health().entity_id().find(&player.entity_id) {
                    let pct = hp.current / hp.max.max(1.0);
                    if pct < 0.35 {
                        alert_str = format!("CRITICAL: {:.0} HP", hp.current);
                        alert_col = Color::srgb(1.0, 0.15, 0.15);
                    } else if pct <= 0.50 {
                        alert_str = format!("{:.0} HP", hp.current);
                        alert_col = Color::srgb(1.0, 0.75, 0.1);
                    }
                }
            }
        }
        text.sections[0].value = alert_str;
        text.sections[0].style.color = alert_col;
    }
}

pub fn update_reticle_abilities_and_hitmarker(
    time: Res<Time>,
    mut hit_marker_state: ResMut<HitMarkerState>,
    camera_mode: Res<State<CameraMode>>,
    mut hitmarker_q: Query<(&mut Style, &Children), With<ReticleHitMarker>>,
    mut hitmarker_ticks_q: Query<&mut BackgroundColor, With<ReticleHitMarkerTick>>,
) {
    if *camera_mode.get() != CameraMode::FPS { return; }

    // Reticle Hitmarker Ticks (Auditory/Visual Balance)
    hit_marker_state.timer.tick(time.delta());
    if let Ok((mut hm_style, children)) = hitmarker_q.get_single_mut() {
        if !hit_marker_state.timer.finished() {
            hm_style.display = Display::Flex;
            let tick_color = if hit_marker_state.is_crit {
                Color::srgb(1.0, 0.2, 0.2) // Red/Gold for Crit
            } else if hit_marker_state.is_armor {
                Color::srgb(0.0, 0.8, 1.0) // Cyan for Armor
            } else {
                Color::WHITE // White for Bodyshot
            };
            for &child in children.iter() {
                if let Ok(mut bg) = hitmarker_ticks_q.get_mut(child) {
                    *bg = tick_color.into();
                }
            }
        } else {
            hm_style.display = Display::None;
        }
    }
}

pub fn toggle_crosshair_menu(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    mut menu_state: ResMut<CrosshairMenuState>,
    mut menu_q: Query<&mut Style, With<CrosshairMenuRoot>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    if console.is_open { return; }

    if keys.just_pressed(KeyCode::F7) {
        menu_state.is_open = !menu_state.is_open;
        if let Ok(mut style) = menu_q.get_single_mut() {
            style.display = if menu_state.is_open { Display::Flex } else { Display::None };
        }
        if let Ok(mut win) = window_q.get_single_mut() {
            if menu_state.is_open {
                win.cursor.grab_mode = CursorGrabMode::None;
                win.cursor.visible = true;
            } else {
                win.cursor.grab_mode = CursorGrabMode::Locked;
                win.cursor.visible = false;
            }
        }
    }
}

pub fn handle_crosshair_menu_interactions(
    mut settings: ResMut<CrosshairSettings>,
    mut button_q: Query<(&Interaction, &CrosshairMenuButton), (Changed<Interaction>, With<Button>)>,
    mut text_q: Query<&mut Text, With<CrosshairMenuText>>,
) {
    let mut changed = false;

    for (interaction, btn) in button_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            changed = true;
            match btn.0.as_str() {
                "Color" => settings.color_preset = settings.color_preset.next(),
                "GapDec" => settings.gap = (settings.gap - 1.0).max(0.0),
                "GapInc" => settings.gap = (settings.gap + 1.0).min(30.0),
                "LenDec" => settings.length = (settings.length - 1.0).max(2.0),
                "LenInc" => settings.length = (settings.length + 1.0).min(30.0),
                "ThickDec" => settings.thickness = (settings.thickness - 0.5).max(1.0),
                "ThickInc" => settings.thickness = (settings.thickness + 0.5).min(8.0),
                "ToggleDot" => settings.dot = !settings.dot,
                "ToggleOutline" => settings.outline = !settings.outline,
                "ToggleDynamic" => settings.is_dynamic = !settings.is_dynamic,
                _ => {}
            }
        }
    }

    if changed || text_q.iter().next().map_or(false, |t| t.sections[0].value.starts_with("Loading")) {
        if let Ok(mut text) = text_q.get_single_mut() {
            text.sections[0].value = format!(
                "Color: {}\nGap: {:.0}px | Length: {:.0}px | Thick: {:.1}px\nDot: {} | Outline: {}\nMode: {}",
                settings.color_preset.name(),
                settings.gap,
                settings.length,
                settings.thickness,
                if settings.dot { "ON" } else { "OFF" },
                if settings.outline { "ON" } else { "OFF" },
                if settings.is_dynamic { "DYNAMIC (Spread Reactive)" } else { "STATIC (Competitive Lock)" },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_celestial_hud_root_toggle_and_display() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        // Spawn HUD pill root and text matching setup_ui
        app.world_mut().spawn((
            NodeBundle {
                style: Style {
                    display: Display::Flex,
                    ..default()
                },
                ..default()
            },
            CelestialHudRoot,
        )).with_children(|pill| {
            pill.spawn((
                TextBundle::from_section(
                    "12:00 (High Noon) | Clear Sky [F8/F9]",
                    TextStyle::default(),
                ),
                CelestialHudText,
            ));
        });

        // 1. Initial State: Display::Flex (visible)
        let mut q = app.world_mut().query_filtered::<&mut Style, With<CelestialHudRoot>>();
        let mut style = q.single_mut(app.world_mut());
        assert_eq!(style.display, Display::Flex);

        // 2. Toggle OFF -> Display::None
        style.display = Display::None;
        assert_eq!(style.display, Display::None);

        // 3. Toggle ON -> Display::Flex
        style.display = Display::Flex;
        assert_eq!(style.display, Display::Flex);
    }

    #[test]
    fn test_console_commands_contains_hud() {
        assert!(CONSOLE_COMMANDS.contains(&"hud"), "CONSOLE_COMMANDS must contain 'hud'");
        assert!(CONSOLE_COMMANDS.contains(&"togglehud"), "CONSOLE_COMMANDS must contain 'togglehud'");
        assert!(CONSOLE_COMMANDS.contains(&"skyhud"), "CONSOLE_COMMANDS must contain 'skyhud'");
        assert!(CONSOLE_COMMANDS.contains(&"f3"), "CONSOLE_COMMANDS must contain 'f3'");
        assert!(CONSOLE_COMMANDS.contains(&"fps"), "CONSOLE_COMMANDS must contain 'fps'");
        assert!(CONSOLE_COMMANDS.contains(&"diag"), "CONSOLE_COMMANDS must contain 'diag'");
    }

    #[test]
    fn test_console_commands_contains_resolution() {
        assert!(CONSOLE_COMMANDS.contains(&"res"), "CONSOLE_COMMANDS must contain 'res'");
        assert!(CONSOLE_COMMANDS.contains(&"resolution"), "CONSOLE_COMMANDS must contain 'resolution'");
        assert!(CONSOLE_COMMANDS.contains(&"fullscreen"), "CONSOLE_COMMANDS must contain 'fullscreen'");
        assert!(CONSOLE_COMMANDS.contains(&"fs"), "CONSOLE_COMMANDS must contain 'fs'");
        assert!(CONSOLE_COMMANDS.contains(&"windowed"), "CONSOLE_COMMANDS must contain 'windowed'");
        assert!(CONSOLE_COMMANDS.contains(&"win"), "CONSOLE_COMMANDS must contain 'win'");
    }

    #[test]
    fn test_console_commands_contains_fps_limit() {
        assert!(CONSOLE_COMMANDS.contains(&"maxfps"), "CONSOLE_COMMANDS must contain 'maxfps'");
        assert!(CONSOLE_COMMANDS.contains(&"fpslimit"), "CONSOLE_COMMANDS must contain 'fpslimit'");
        assert!(CONSOLE_COMMANDS.contains(&"fps_max"), "CONSOLE_COMMANDS must contain 'fps_max'");
        assert!(CONSOLE_COMMANDS.contains(&"limitfps"), "CONSOLE_COMMANDS must contain 'limitfps'");
        assert!(CONSOLE_COMMANDS.contains(&"vsync"), "CONSOLE_COMMANDS must contain 'vsync'");
    }

    #[test]
    fn test_diagnostic_overlay_root_toggle_and_display() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        app.world_mut().spawn((
            NodeBundle {
                style: Style {
                    display: Display::None,
                    ..default()
                },
                ..default()
            },
            DiagnosticOverlayRoot,
        )).with_children(|box_node| {
            box_node.spawn((
                TextBundle::from_sections([
                    TextSection::new("DIAGNOSTICS [F3]\n", TextStyle::default()),
                    TextSection::new("FPS: 60.0", TextStyle::default()),
                ]),
                DiagnosticOverlayText,
            ));
        });

        // 1. Initial State: Display::None (hidden until toggled)
        let mut q = app.world_mut().query_filtered::<&mut Style, With<DiagnosticOverlayRoot>>();
        let mut style = q.single_mut(app.world_mut());
        assert_eq!(style.display, Display::None);

        // 2. Toggle ON -> Display::Flex
        style.display = Display::Flex;
        assert_eq!(style.display, Display::Flex);

        // 3. Toggle OFF -> Display::None
        style.display = Display::None;
        assert_eq!(style.display, Display::None);
    }
}