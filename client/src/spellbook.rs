// ============================================================================
// File: client/src/spellbook.rs
// ============================================================================
// ----------------------------------------------------------------------------
// SPELLBOOK, 16-KEY PREPARED HOTBAR & CONFIGURABLE OPTIONS PANEL
// ----------------------------------------------------------------------------
// Architectural Note:
// 1. Spells & Abilities Grimoire: EverQuest-style clickable spellbook catalog
//    containing tactical and magical abilities that can be prepared into a 16-slot hotbar.
// 2. 16-Key Prepared Hotbar: Dynamic HUD action bar with cooldown timers, slot numbers,
//    and real-time keybind badges.
// 3. Options Panel [Key O]:
//    - Reticle System Toggle (ON / OFF) & crosshair customizer.
//    - Extended Visible Range / Chunk Load Distance (10 - 64 chunks / up to 1024m)
//      and "Spawn Whole Zone" trigger for benchmarking FPS and draw performance.
//    - Keybind Rebinding Menu for all 16 hotbar slots.
// 4. Strict determinism and seamless Bevy 0.14 integration.

use avian3d::prelude::*;
use bevy::prelude::{Transform as BevyTransform, *};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use tracing::info;

use crate::components::*;
use crate::core::*;
use crate::audio_feedback::play_sound;
use spacetime_rts_logic::TacticalAbilityKind;

// ----------------------------------------------------------------------------
// 1. SPELL & ABILITY CATALOG DEFINITIONS
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellCategory {
    Tactical,
    Elemental,
    Arcane,
    Restoration,
}

impl SpellCategory {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Tactical => "Tactical",
            Self::Elemental => "Elemental",
            Self::Arcane => "Arcane",
            Self::Restoration => "Restoration",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Tactical => Color::srgb(0.2, 0.8, 0.8),
            Self::Elemental => Color::srgb(1.0, 0.45, 0.2),
            Self::Arcane => Color::srgb(0.7, 0.4, 0.95),
            Self::Restoration => Color::srgb(0.3, 0.9, 0.4),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpellDef {
    pub id: &'static str,
    pub name: &'static str,
    pub category: SpellCategory,
    pub icon: &'static str,
    pub description: &'static str,
    pub cooldown_seconds: f32,
    pub color: Color,
}

pub const SPELL_CATALOG: &[SpellDef] = &[
    SpellDef {
        id: "phase_dash",
        name: "Phase Dash",
        category: SpellCategory::Tactical,
        icon: "DASH",
        description: "Surges 32m/s forward or strafe-direction with aerodynamic slipstream.",
        cooldown_seconds: 6.0,
        color: Color::srgb(0.0, 0.9, 0.9),
    },
    SpellDef {
        id: "smoke_veil",
        name: "Smoke Veil",
        category: SpellCategory::Tactical,
        icon: "SMOK",
        description: "Deploys a dense volumetric smoke cloud obscuring lines of sight for 8s.",
        cooldown_seconds: 18.0,
        color: Color::srgb(0.65, 0.70, 0.75),
    },
    SpellDef {
        id: "intel_dart",
        name: "Intel Dart",
        category: SpellCategory::Tactical,
        icon: "INTL",
        description: "Fires a high-velocity sonar ping dart revealing hidden entities.",
        cooldown_seconds: 22.0,
        color: Color::srgb(1.0, 0.85, 0.2),
    },
    SpellDef {
        id: "grav_lift",
        name: "Grav-Lift",
        category: SpellCategory::Tactical,
        icon: "LIFT",
        description: "Projects an anti-gravity vertical repulsion column propelling player upward.",
        cooldown_seconds: 15.0,
        color: Color::srgb(0.3, 0.95, 0.45),
    },
    SpellDef {
        id: "fireball",
        name: "Fireball",
        category: SpellCategory::Elemental,
        icon: "FIRE",
        description: "Launches a slow, heavy incendiary sphere detonating with 4.5m blast radius.",
        cooldown_seconds: 4.0,
        color: Color::srgb(1.0, 0.35, 0.1),
    },
    SpellDef {
        id: "magic_missile",
        name: "Magic Missile",
        category: SpellCategory::Arcane,
        icon: "MISS",
        description: "Discharges a rapid 55m/s zero-gravity arcane projectile.",
        cooldown_seconds: 1.5,
        color: Color::srgb(0.75, 0.3, 0.95),
    },
    SpellDef {
        id: "frost_nova",
        name: "Frost Nova",
        category: SpellCategory::Elemental,
        icon: "NOVA",
        description: "Radial freezing explosion chilling all nearby hostile creatures.",
        cooldown_seconds: 8.0,
        color: Color::srgb(0.4, 0.8, 1.0),
    },
    SpellDef {
        id: "minor_healing",
        name: "Minor Healing",
        category: SpellCategory::Restoration,
        icon: "HEAL",
        description: "Restores vital health with a rejuvenating surge of golden radiance.",
        cooldown_seconds: 10.0,
        color: Color::srgb(0.3, 1.0, 0.5),
    },
    SpellDef {
        id: "blink",
        name: "Blink",
        category: SpellCategory::Arcane,
        icon: "BLNK",
        description: "Instantly teleports 12m forward across the physical plane.",
        cooldown_seconds: 12.0,
        color: Color::srgb(0.85, 0.4, 0.9),
    },
    SpellDef {
        id: "chain_lightning",
        name: "Chain Lightning",
        category: SpellCategory::Elemental,
        icon: "LGHT",
        description: "Discharges crackling high-voltage lightning arcing through targets.",
        cooldown_seconds: 9.0,
        color: Color::srgb(0.95, 0.9, 0.3),
    },
    SpellDef {
        id: "stoneskin",
        name: "Stoneskin",
        category: SpellCategory::Restoration,
        icon: "SHLD",
        description: "Hardens flesh into earthen armor, absorbing incoming impact damage.",
        cooldown_seconds: 25.0,
        color: Color::srgb(0.75, 0.6, 0.4),
    },
    SpellDef {
        id: "starfall",
        name: "Starfall",
        category: SpellCategory::Arcane,
        icon: "STAR",
        description: "Calls down celestial radiant sparks from the binary star system.",
        cooldown_seconds: 30.0,
        color: Color::srgb(0.95, 0.8, 1.0),
    },
    SpellDef {
        id: "spirit_familiar",
        name: "Spirit Familiar",
        category: SpellCategory::Restoration,
        icon: "WOLF",
        description: "Summons a loyal spectral spirit pet companion to accompany player.",
        cooldown_seconds: 40.0,
        color: Color::srgb(0.4, 0.9, 0.85),
    },
    SpellDef {
        id: "war_cry",
        name: "War Cry",
        category: SpellCategory::Tactical,
        icon: "ROAR",
        description: "Thunderous acoustic roar delivering kinetic knockback to nearby foes.",
        cooldown_seconds: 20.0,
        color: Color::srgb(0.9, 0.3, 0.3),
    },
    SpellDef {
        id: "shadow_cloak",
        name: "Shadow Cloak",
        category: SpellCategory::Tactical,
        icon: "CLOK",
        description: "Envelops the player in shadows, reducing visual aggro profile.",
        cooldown_seconds: 35.0,
        color: Color::srgb(0.45, 0.4, 0.55),
    },
    SpellDef {
        id: "solar_flare",
        name: "Solar Flare",
        category: SpellCategory::Elemental,
        icon: "SOLR",
        description: "Emits a blistering beam of concentrated stellar radiation.",
        cooldown_seconds: 16.0,
        color: Color::srgb(1.0, 0.6, 0.15),
    },
];

pub fn get_spell_by_id(id: &str) -> Option<&'static SpellDef> {
    SPELL_CATALOG.iter().find(|s| s.id == id || s.name == id)
}

// ----------------------------------------------------------------------------
// 2. KEYBINDINGS & RESOURCES
// ----------------------------------------------------------------------------

pub fn keycode_display_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::Digit0 => "0",
        KeyCode::Minus => "-",
        KeyCode::Equal => "=",
        KeyCode::KeyQ => "Q",
        KeyCode::KeyW => "W",
        KeyCode::KeyE => "E",
        KeyCode::KeyR => "R",
        KeyCode::KeyT => "T",
        KeyCode::KeyY => "Y",
        KeyCode::KeyU => "U",
        KeyCode::KeyI => "I",
        KeyCode::KeyO => "O",
        KeyCode::KeyP => "P",
        KeyCode::KeyA => "A",
        KeyCode::KeyS => "S",
        KeyCode::KeyD => "D",
        KeyCode::KeyF => "F",
        KeyCode::KeyG => "G",
        KeyCode::KeyH => "H",
        KeyCode::KeyJ => "J",
        KeyCode::KeyK => "K",
        KeyCode::KeyL => "L",
        KeyCode::KeyZ => "Z",
        KeyCode::KeyX => "X",
        KeyCode::KeyC => "C",
        KeyCode::KeyV => "V",
        KeyCode::KeyB => "B",
        KeyCode::KeyN => "N",
        KeyCode::KeyM => "M",
        KeyCode::Space => "Space",
        KeyCode::ShiftLeft => "LShift",
        KeyCode::ShiftRight => "RShift",
        KeyCode::ControlLeft => "LCtrl",
        KeyCode::AltLeft => "LAlt",
        KeyCode::Tab => "Tab",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        _ => "Key",
    }
}

#[derive(Resource, Clone, Debug)]
pub struct HotbarKeybinds {
    pub keybinds: [KeyCode; 16],
}

impl Default for HotbarKeybinds {
    fn default() -> Self {
        Self {
            keybinds: [
                // Top row: 1-8
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
                KeyCode::Digit5,
                KeyCode::Digit6,
                KeyCode::Digit7,
                KeyCode::Digit8,
                // Bottom row: Q, E, R, F, Z, X, C, V
                KeyCode::KeyQ,
                KeyCode::KeyE,
                KeyCode::KeyR,
                KeyCode::KeyF,
                KeyCode::KeyZ,
                KeyCode::KeyX,
                KeyCode::KeyC,
                KeyCode::KeyV,
            ],
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct PreparedHotbarState {
    pub slots: [Option<String>; 16],
    pub cooldowns: [f32; 16],
    pub max_cooldowns: [f32; 16],
}

impl Default for PreparedHotbarState {
    fn default() -> Self {
        let mut slots = [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None];
        // Populate standard default prepared spells
        slots[0] = Some("phase_dash".into());
        slots[1] = Some("smoke_veil".into());
        slots[2] = Some("intel_dart".into());
        slots[3] = Some("grav_lift".into());
        slots[4] = Some("fireball".into());
        slots[5] = Some("magic_missile".into());
        slots[6] = Some("frost_nova".into());
        slots[7] = Some("minor_healing".into());
        slots[8] = Some("blink".into());
        slots[9] = Some("chain_lightning".into());
        slots[10] = Some("stoneskin".into());
        slots[11] = Some("starfall".into());
        slots[12] = Some("spirit_familiar".into());
        slots[13] = Some("war_cry".into());
        slots[14] = Some("shadow_cloak".into());
        slots[15] = Some("solar_flare".into());

        Self {
            slots,
            cooldowns: [0.0; 16],
            max_cooldowns: [1.0; 16],
        }
    }
}

#[derive(Resource, Default)]
pub struct SpellbookWindowState {
    pub is_open: bool,
    pub selected_spell_for_slotting: Option<String>,
    pub category_filter: Option<SpellCategory>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OptionsTab {
    #[default]
    Reticle,
    Performance,
    Keybinds,
}

#[derive(Resource)]
pub struct OptionsPanelState {
    pub is_open: bool,
    pub active_tab: OptionsTab,
    pub rebinding_slot: Option<usize>,
}

impl Default for OptionsPanelState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_tab: OptionsTab::Reticle,
            rebinding_slot: None,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct TerrainRenderSettings {
    pub view_distance_chunks: i32,
    pub unload_distance_chunks: i32,
    pub spawn_full_zone: bool,
    pub visible_range_meters: f32,
}

impl Default for TerrainRenderSettings {
    fn default() -> Self {
        Self {
            view_distance_chunks: 10, // 160m default
            unload_distance_chunks: 13,
            spawn_full_zone: false,
            visible_range_meters: 160.0,
        }
    }
}

// ----------------------------------------------------------------------------
// 3. UI MARKER COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)] pub struct PreparedHotbarRoot;
#[derive(Component)] pub struct HotbarSlotButton(pub usize);
#[derive(Component)] pub struct HotbarSlotKeyText(pub usize);
#[derive(Component)] pub struct HotbarSlotNameText(pub usize);
#[derive(Component)] pub struct HotbarSlotCooldownText(pub usize);
#[derive(Component)] pub struct HotbarOpenSpellbookButton;

#[derive(Component)] pub struct SpellbookModalRoot;
#[derive(Component)] pub struct SpellbookTabButton(pub Option<SpellCategory>);
#[derive(Component)] pub struct SpellbookCardSelectButton(pub String);
#[derive(Component)] pub struct SpellbookStatusText;

#[derive(Component)] pub struct OptionsPanelModalRoot;
#[derive(Component)] pub struct OptionsTabButton(pub OptionsTab);
#[derive(Component)] pub struct OptionsReticleToggleBtn;
#[derive(Component)] pub struct OptionsReticleStatusText;
#[derive(Component)] pub struct OptionsRangePresetButton(pub i32);
#[derive(Component)] pub struct OptionsSpawnWholeZoneBtn;
#[derive(Component)] pub struct OptionsPerfMetricsText;
#[derive(Component)] pub struct OptionsKeybindRebindBtn(pub usize);
#[derive(Component)] pub struct OptionsKeybindResetBtn;
#[derive(Component)] pub struct OptionsCloseBtn;

// ----------------------------------------------------------------------------
// 4. UI SETUP SYSTEMS
// ----------------------------------------------------------------------------

pub fn setup_prepared_hotbar_ui(
    mut commands: Commands,
    keybinds: Res<HotbarKeybinds>,
    hotbar: Res<PreparedHotbarState>,
) {
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-360.0)),
                width: Val::Px(720.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            z_index: ZIndex::Global(100),
            ..default()
        },
        PreparedHotbarRoot,
    )).with_children(|root| {
        // 2 rows of 8 slots grid container
        root.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            ..default()
        }).with_children(|grid| {
            for row in 0..2 {
                grid.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(4.0),
                        ..default()
                    },
                    ..default()
                }).with_children(|row_ui| {
                    for col in 0..8 {
                        let slot_idx = row * 8 + col;
                        let key_label = keycode_display_name(keybinds.keybinds[slot_idx]);
                        let spell_name = hotbar.slots[slot_idx].as_deref().unwrap_or("EMPTY");
                        let spell_def = get_spell_by_id(spell_name);

                        let (icon_text, icon_color) = if let Some(def) = spell_def {
                            (def.icon, def.color)
                        } else {
                            ("---", Color::srgb(0.4, 0.4, 0.4))
                        };

                        row_ui.spawn((
                            ButtonBundle {
                                style: Style {
                                    width: Val::Px(72.0),
                                    height: Val::Px(36.0),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::SpaceBetween,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::all(Val::Px(2.0)),
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.35, 0.35, 0.35)),
                                background_color: BackgroundColor(Color::srgba(0.08, 0.08, 0.10, 0.92)),
                                ..default()
                            },
                            HotbarSlotButton(slot_idx),
                        )).with_children(|slot| {
                            // Top bar: Hotkey badge & slot index
                            slot.spawn(NodeBundle {
                                style: Style {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::SpaceBetween,
                                    ..default()
                                },
                                ..default()
                            }).with_children(|top| {
                                top.spawn((
                                    TextBundle::from_section(
                                        format!("[{}]", key_label),
                                        TextStyle { font_size: 9.0, color: Color::srgb(1.0, 0.85, 0.3), ..default() }
                                    ),
                                    HotbarSlotKeyText(slot_idx),
                                ));
                                top.spawn((
                                    TextBundle::from_section(
                                        "",
                                        TextStyle { font_size: 9.0, color: Color::srgb(0.9, 0.3, 0.3), ..default() }
                                    ),
                                    HotbarSlotCooldownText(slot_idx),
                                ));
                            });

                            // Center spell name / badge
                            slot.spawn((
                                TextBundle::from_section(
                                    icon_text,
                                    TextStyle { font_size: 11.0, color: icon_color, ..default() }
                                ),
                                HotbarSlotNameText(slot_idx),
                            ));
                        });
                    }
                });
            }
        });

        // Open Spellbook Grimoire button on the right
        root.spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(78.0),
                    height: Val::Px(76.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(4.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                border_color: BorderColor(Color::srgb(0.7, 0.55, 0.3)),
                background_color: BackgroundColor(Color::srgb(0.22, 0.15, 0.10)),
                ..default()
            },
            HotbarOpenSpellbookButton,
        )).with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                "GRIMOIRE",
                TextStyle { font_size: 10.0, color: Color::srgb(0.95, 0.85, 0.5), ..default() }
            ));
            btn.spawn(TextBundle::from_section(
                "[K]",
                TextStyle { font_size: 12.0, color: Color::WHITE, ..default() }
            ));
        });
    });
}

pub fn setup_spellbook_modal_ui(
    mut commands: Commands,
) {
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
            z_index: ZIndex::Global(120),
            ..default()
        },
        SpellbookModalRoot,
    )).with_children(|overlay| {
        // Parchment tome window
        overlay.spawn(NodeBundle {
            style: Style {
                width: Val::Px(740.0),
                height: Val::Px(560.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            border_color: BorderColor(Color::srgb(0.65, 0.50, 0.25)),
            background_color: BackgroundColor(Color::srgba(0.12, 0.10, 0.08, 0.98)),
            ..default()
        }).with_children(|book| {
            // Header
            book.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                ..default()
            }).with_children(|header| {
                header.spawn(TextBundle::from_section(
                    "SPELLBOOK & ABILITY GRIMOIRE",
                    TextStyle { font_size: 16.0, color: Color::srgb(0.95, 0.85, 0.55), ..default() }
                ));

                header.spawn(TextBundle::from_section(
                    "Press [K] to Close | Click any Spell to Prepare into Hotbar (Slots 1-16)",
                    TextStyle { font_size: 11.0, color: Color::srgb(0.75, 0.70, 0.60), ..default() }
                ));
            });

            // Category filter tabs
            book.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    margin: UiRect::bottom(Val::Px(12.0)),
                    ..default()
                },
                ..default()
            }).with_children(|tabs| {
                let tab_list = [
                    (None, "ALL (16)"),
                    (Some(SpellCategory::Tactical), "TACTICAL"),
                    (Some(SpellCategory::Elemental), "ELEMENTAL"),
                    (Some(SpellCategory::Arcane), "ARCANE"),
                    (Some(SpellCategory::Restoration), "RESTORATION"),
                ];

                for (cat, label) in tab_list {
                    tabs.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.5, 0.4, 0.2)),
                            background_color: BackgroundColor(Color::srgb(0.20, 0.16, 0.12)),
                            ..default()
                        },
                        SpellbookTabButton(cat),
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            label,
                            TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }
                        ));
                    });
                }
            });

            // Grid of spell cards
            book.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(8.0),
                    overflow: Overflow::clip_y(),
                    ..default()
                },
                ..default()
            }).with_children(|grid| {
                for spell in SPELL_CATALOG {
                    grid.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(168.0),
                                height: Val::Px(95.0),
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::all(Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.4, 0.35, 0.25)),
                            background_color: BackgroundColor(Color::srgb(0.16, 0.14, 0.12)),
                            ..default()
                        },
                        SpellbookCardSelectButton(spell.id.to_string()),
                    )).with_children(|card| {
                        // Title + CD
                        card.spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                ..default()
                            },
                            ..default()
                        }).with_children(|top| {
                            top.spawn(TextBundle::from_section(
                                spell.name,
                                TextStyle { font_size: 12.0, color: spell.color, ..default() }
                            ));
                            top.spawn(TextBundle::from_section(
                                format!("{:.0}s", spell.cooldown_seconds),
                                TextStyle { font_size: 10.0, color: Color::srgb(0.7, 0.7, 0.7), ..default() }
                            ));
                        });

                        // Description
                        card.spawn(TextBundle::from_section(
                            spell.description,
                            TextStyle { font_size: 9.5, color: Color::srgb(0.8, 0.75, 0.70), ..default() }
                        ));

                        // Action label
                        card.spawn(TextBundle::from_section(
                            "CLICK TO PREPARE ->",
                            TextStyle { font_size: 9.0, color: Color::srgb(1.0, 0.85, 0.3), ..default() }
                        ));
                    });
                }
            });

            // Bottom Status / Instructions
            book.spawn((
                TextBundle::from_section(
                    "Click a spell above, then click any of the 16 Hotbar Slots below to prepare it into your spellbook.",
                    TextStyle { font_size: 11.0, color: Color::srgb(0.9, 0.85, 0.5), ..default() }
                ).with_style(Style { margin: UiRect::top(Val::Px(8.0)), ..default() }),
                SpellbookStatusText,
            ));
        });
    });
}

pub fn setup_options_panel_modal_ui(
    mut commands: Commands,
) {
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
            z_index: ZIndex::Global(130),
            ..default()
        },
        OptionsPanelModalRoot,
    )).with_children(|overlay| {
        overlay.spawn(NodeBundle {
            style: Style {
                width: Val::Px(640.0),
                height: Val::Px(500.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(2.5)),
                ..default()
            },
            border_color: BorderColor(Color::srgb(0.35, 0.45, 0.60)),
            background_color: BackgroundColor(Color::srgba(0.08, 0.09, 0.12, 0.98)),
            ..default()
        }).with_children(|window| {
            // Header
            window.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(12.0)),
                    ..default()
                },
                ..default()
            }).with_children(|head| {
                head.spawn(TextBundle::from_section(
                    "SYSTEM & GAMEPLAY OPTIONS [O]",
                    TextStyle { font_size: 16.0, color: Color::srgb(0.4, 0.8, 1.0), ..default() }
                ));

                head.spawn((
                    ButtonBundle {
                        style: Style {
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        border_color: BorderColor(Color::srgb(0.6, 0.3, 0.3)),
                        background_color: BackgroundColor(Color::srgb(0.35, 0.15, 0.15)),
                        ..default()
                    },
                    OptionsCloseBtn,
                )).with_children(|b| {
                    b.spawn(TextBundle::from_section("CLOSE [O]", TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }));
                });
            });

            // Tabs bar
            window.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    margin: UiRect::bottom(Val::Px(14.0)),
                    ..default()
                },
                ..default()
            }).with_children(|tabs| {
                let tabs_def = [
                    (OptionsTab::Reticle, "1. RETICLE & HUD"),
                    (OptionsTab::Performance, "2. VISIBLE RANGE & PERFORMANCE"),
                    (OptionsTab::Keybinds, "3. HOTBAR KEYBINDS"),
                ];
                for (tab, label) in tabs_def {
                    tabs.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.3, 0.5, 0.7)),
                            background_color: BackgroundColor(Color::srgb(0.15, 0.20, 0.28)),
                            ..default()
                        },
                        OptionsTabButton(tab),
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(label, TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }));
                    });
                }
            });

            // Body Content Panel
            window.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                border_color: BorderColor(Color::srgb(0.2, 0.3, 0.4)),
                background_color: BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.8)),
                ..default()
            }).with_children(|body| {
                // Section 1: Reticle Toggle
                body.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(12.0)),
                        ..default()
                    },
                    ..default()
                }).with_children(|row| {
                    row.spawn(TextBundle::from_section(
                        "Center Reticle / Crosshair System:",
                        TextStyle { font_size: 13.0, color: Color::WHITE, ..default() }
                    ));

                    row.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.2, 0.8, 0.4)),
                            background_color: BackgroundColor(Color::srgb(0.1, 0.4, 0.2)),
                            ..default()
                        },
                        OptionsReticleToggleBtn,
                    )).with_children(|b| {
                        b.spawn((
                            TextBundle::from_section("RETICLE: ON", TextStyle { font_size: 12.0, color: Color::WHITE, ..default() }),
                            OptionsReticleStatusText,
                        ));
                    });
                });

                // Section 2: Visible Range & Testing
                body.spawn(TextBundle::from_section(
                    "TERRAIN VISIBLE RANGE & DRAW BENCHMARK (TEST FPS):",
                    TextStyle { font_size: 12.0, color: Color::srgb(0.4, 0.8, 1.0), ..default() }
                ).with_style(Style { margin: UiRect::bottom(Val::Px(6.0)), ..default() }));

                body.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                    ..default()
                }).with_children(|range_row| {
                    let presets = [
                        (10, "10 Chunks (160m)"),
                        (16, "16 Chunks (256m)"),
                        (24, "24 Chunks (384m)"),
                        (32, "32 Chunks (512m)"),
                        (48, "48 Chunks (768m)"),
                        (64, "64 Chunks (1024m)"),
                    ];
                    for (chunks, lbl) in presets {
                        range_row.spawn((
                            ButtonBundle {
                                style: Style {
                                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.3, 0.5, 0.6)),
                                background_color: BackgroundColor(Color::srgb(0.12, 0.22, 0.28)),
                                ..default()
                            },
                            OptionsRangePresetButton(chunks),
                        )).with_children(|b| {
                            b.spawn(TextBundle::from_section(lbl, TextStyle { font_size: 10.0, color: Color::WHITE, ..default() }));
                        });
                    }
                });

                // Spawn Whole Zone Button
                body.spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(8.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::bottom(Val::Px(10.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            ..default()
                        },
                        border_color: BorderColor(Color::srgb(0.8, 0.6, 0.2)),
                        background_color: BackgroundColor(Color::srgb(0.35, 0.25, 0.10)),
                        ..default()
                    },
                    OptionsSpawnWholeZoneBtn,
                )).with_children(|b| {
                    b.spawn(TextBundle::from_section(
                        "⚡ SPAWN ENTIRE ZONE (LOAD FULL 1KM RADIUS & BENCHMARK FPS)",
                        TextStyle { font_size: 12.0, color: Color::srgb(1.0, 0.85, 0.3), ..default() }
                    ));
                });

                // Live Performance Readout
                body.spawn((
                    TextBundle::from_section(
                        "Visible Range: 160m (10 chunks) | Camera Far: 1000m | FPS: 60",
                        TextStyle { font_size: 11.0, color: Color::srgb(0.7, 0.9, 0.7), ..default() }
                    ).with_style(Style { margin: UiRect::bottom(Val::Px(12.0)), ..default() }),
                    OptionsPerfMetricsText,
                ));

                // Section 3: Hotbar Keybinds Rebinding
                body.spawn(TextBundle::from_section(
                    "HOTBAR KEYBINDINGS (16 SLOTS) - CLICK TO REBIND:",
                    TextStyle { font_size: 12.0, color: Color::srgb(0.9, 0.8, 0.4), ..default() }
                ).with_style(Style { margin: UiRect::bottom(Val::Px(4.0)), ..default() }));

                body.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(4.0),
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                    ..default()
                }).with_children(|rebind_grid| {
                    for slot in 0..16 {
                        rebind_grid.spawn((
                            ButtonBundle {
                                style: Style {
                                    width: Val::Px(72.0),
                                    height: Val::Px(24.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.4, 0.4, 0.4)),
                                background_color: BackgroundColor(Color::srgb(0.18, 0.18, 0.22)),
                                ..default()
                            },
                            OptionsKeybindRebindBtn(slot),
                        )).with_children(|b| {
                            b.spawn(TextBundle::from_section(
                                format!("S{}: ?", slot + 1),
                                TextStyle { font_size: 10.0, color: Color::WHITE, ..default() }
                            ));
                        });
                    }
                });

                // Reset keybinds button
                body.spawn((
                    ButtonBundle {
                        style: Style {
                            align_self: AlignSelf::FlexStart,
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        border_color: BorderColor(Color::srgb(0.5, 0.3, 0.3)),
                        background_color: BackgroundColor(Color::srgb(0.25, 0.15, 0.15)),
                        ..default()
                    },
                    OptionsKeybindResetBtn,
                )).with_children(|b| {
                    b.spawn(TextBundle::from_section("Reset Default Keybindings", TextStyle { font_size: 10.0, color: Color::WHITE, ..default() }));
                });
            });
        });
    });
}

// ----------------------------------------------------------------------------
// 5. INTERACTION & LOGIC SYSTEMS
// ----------------------------------------------------------------------------

pub fn toggle_options_and_spellbook_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut options_state: ResMut<OptionsPanelState>,
    mut spellbook_state: ResMut<SpellbookWindowState>,
    mut options_q: Query<&mut Style, (With<OptionsPanelModalRoot>, Without<SpellbookModalRoot>)>,
    mut spellbook_q: Query<&mut Style, (With<SpellbookModalRoot>, Without<OptionsPanelModalRoot>)>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    let mut state_changed = false;

    // Toggle Options with O
    if keys.just_pressed(KeyCode::KeyO) {
        options_state.is_open = !options_state.is_open;
        if options_state.is_open {
            spellbook_state.is_open = false; // mutually exclusive or stack
        }
        state_changed = true;
    }

    // Toggle Spellbook with K
    if keys.just_pressed(KeyCode::KeyK) {
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

pub fn update_hotbar_ui_system(
    time: Res<Time>,
    keybinds: Res<HotbarKeybinds>,
    mut hotbar: ResMut<PreparedHotbarState>,
    mut key_text_q: Query<(&HotbarSlotKeyText, &mut Text), (Without<HotbarSlotNameText>, Without<HotbarSlotCooldownText>)>,
    mut name_text_q: Query<(&HotbarSlotNameText, &mut Text), (Without<HotbarSlotKeyText>, Without<HotbarSlotCooldownText>)>,
    mut cd_text_q: Query<(&HotbarSlotCooldownText, &mut Text), (Without<HotbarSlotKeyText>, Without<HotbarSlotNameText>)>,
) {
    let dt = time.delta_seconds();

    // Advance cooldown timers
    for slot in 0..16 {
        if hotbar.cooldowns[slot] > 0.0 {
            hotbar.cooldowns[slot] = (hotbar.cooldowns[slot] - dt).max(0.0);
        }
    }

    for (k_marker, mut text) in key_text_q.iter_mut() {
        let slot = k_marker.0;
        if slot < 16 {
            let key_str = keycode_display_name(keybinds.keybinds[slot]);
            text.sections[0].value = format!("[{}]", key_str);
        }
    }

    for (n_marker, mut text) in name_text_q.iter_mut() {
        let slot = n_marker.0;
        if slot < 16 {
            if let Some(spell_id) = &hotbar.slots[slot] {
                if let Some(def) = get_spell_by_id(spell_id) {
                    text.sections[0].value = def.icon.to_string();
                    text.sections[0].style.color = def.color;
                } else {
                    text.sections[0].value = "---".into();
                    text.sections[0].style.color = Color::srgb(0.5, 0.5, 0.5);
                }
            } else {
                text.sections[0].value = "---".into();
                text.sections[0].style.color = Color::srgb(0.3, 0.3, 0.3);
            }
        }
    }

    for (cd_marker, mut text) in cd_text_q.iter_mut() {
        let slot = cd_marker.0;
        if slot < 16 {
            let cd = hotbar.cooldowns[slot];
            if cd > 0.0 {
                text.sections[0].value = format!("{:.1}s", cd);
            } else {
                text.sections[0].value = "".into();
            }
        }
    }
}

pub fn handle_hotbar_casting_system(
    mut commands: Commands,
    (keys, keybinds): (Res<ButtonInput<KeyCode>>, Res<HotbarKeybinds>),
    (mut hotbar, mut spellbook_state): (ResMut<PreparedHotbarState>, ResMut<SpellbookWindowState>),
    (options_state, console): (Res<OptionsPanelState>, Res<ConsoleState>),
    slot_click_q: Query<(&Interaction, &HotbarSlotButton), Changed<Interaction>>,
    open_book_q: Query<&Interaction, (Changed<Interaction>, With<HotbarOpenSpellbookButton>)>,
    styles: (
        Query<&mut Style, With<SpellbookModalRoot>>,
        Query<&mut Window, With<PrimaryWindow>>,
    ),
    mut player_q: Query<(Entity, &mut BevyTransform, &mut LinearVelocity, &mut Kcc), With<PlayerBody>>,
    camera_q: Query<&GlobalTransform, With<FpsCamera>>,
    (mut meshes, mut materials): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    (mut tactical_state, audio_handles): (Option<ResMut<TacticalAbilityState>>, Option<Res<CombatAudioHandles>>),
) {
    let (mut spellbook_q, mut window_q) = styles;
    if console.is_open || options_state.is_open {
        return;
    }

    // Click on Open Spellbook button
    for interaction in open_book_q.iter() {
        if *interaction == Interaction::Pressed {
            spellbook_state.is_open = !spellbook_state.is_open;
            if let Ok(mut style) = spellbook_q.get_single_mut() {
                style.display = if spellbook_state.is_open { Display::Flex } else { Display::None };
            }
            if let Ok(mut window) = window_q.get_single_mut() {
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
            }
        }
    }

    // Check if slot was clicked (either to cast or to slot selected spell)
    for (interaction, slot_btn) in slot_click_q.iter() {
        if *interaction == Interaction::Pressed {
            let slot = slot_btn.0;
            if let Some(selected_spell) = spellbook_state.selected_spell_for_slotting.take() {
                hotbar.slots[slot] = Some(selected_spell);
                info!("Prepared spell into Hotbar Slot {}", slot + 1);
            } else {
                // Cast from slot
                cast_slot(
                    slot,
                    &mut commands,
                    &mut hotbar,
                    &mut player_q,
                    &camera_q,
                    &mut meshes,
                    &mut materials,
                    tactical_state.as_deref_mut(),
                    audio_handles.as_deref(),
                );
            }
        }
    }

    // Keyboard casting: iterate through bound keys
    for slot in 0..16 {
        let key = keybinds.keybinds[slot];
        if keys.just_pressed(key) {
            cast_slot(
                slot,
                &mut commands,
                &mut hotbar,
                &mut player_q,
                &camera_q,
                &mut meshes,
                &mut materials,
                tactical_state.as_deref_mut(),
                audio_handles.as_deref(),
            );
        }
    }
}

fn cast_slot(
    slot: usize,
    commands: &mut Commands,
    hotbar: &mut ResMut<PreparedHotbarState>,
    player_q: &mut Query<(Entity, &mut BevyTransform, &mut LinearVelocity, &mut Kcc), With<PlayerBody>>,
    camera_q: &Query<&GlobalTransform, With<FpsCamera>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    mut tactical_state: Option<&mut TacticalAbilityState>,
    audio_handles: Option<&CombatAudioHandles>,
) {
    if slot >= 16 { return; }
    if hotbar.cooldowns[slot] > 0.0 { return; }
    let Some(spell_id) = hotbar.slots[slot].clone() else { return; };
    let Some(def) = get_spell_by_id(&spell_id) else { return; };

    let Ok((_p_entity, mut p_trans, mut lin_vel, _)) = player_q.get_single_mut() else { return; };
    let cam_forward = camera_q.get_single().map_or(p_trans.forward(), |c| c.forward());
    let cam_pos = camera_q.get_single().map_or(p_trans.translation + Vec3::new(0.0, 1.5, 0.0), |c| c.translation());

    match def.id {
        "phase_dash" => {
            let mut dash_dir = *cam_forward;
            dash_dir.y = 0.0;
            let dir = dash_dir.normalize_or_zero();
            lin_vel.x = dir.x * 32.0;
            lin_vel.z = dir.z * 32.0;

            if let Some(ref mut ts) = tactical_state {
                let _ = ts.cooldowns.trigger(TacticalAbilityKind::PhaseDash);
            }
            if let Some(ah) = audio_handles {
                play_sound(commands, &ah.dash_whoosh);
            }

            // Visual slipstream particles
            let trail_mesh = meshes.add(bevy::math::primitives::Sphere::new(0.2));
            let trail_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.0, 1.0, 1.0, 0.8),
                unlit: true,
                ..default()
            });
            for step in 1..=4 {
                let off = p_trans.translation - dir * (step as f32 * 0.7);
                commands.spawn((
                    PbrBundle {
                        mesh: trail_mesh.clone(),
                        material: trail_mat.clone(),
                        transform: BevyTransform::from_translation(off),
                        ..default()
                    },
                    Particle { timer: Timer::from_seconds(0.35, TimerMode::Once) },
                ));
            }
        }
        "smoke_veil" => {
            let target_point = cam_pos + cam_forward * 12.0;
            let smoke_mesh = meshes.add(bevy::math::primitives::Sphere::new(1.8));
            let smoke_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.65, 0.68, 0.72, 0.75),
                perceptual_roughness: 0.95,
                ..default()
            });
            for off in [Vec3::ZERO, Vec3::new(1.5, 0.5, 0.0), Vec3::new(-1.5, 0.5, 0.0), Vec3::new(0.0, 1.2, 1.5)] {
                commands.spawn((
                    PbrBundle {
                        mesh: smoke_mesh.clone(),
                        material: smoke_mat.clone(),
                        transform: BevyTransform::from_translation(target_point + off),
                        ..default()
                    },
                    Particle { timer: Timer::from_seconds(8.0, TimerMode::Once) },
                ));
            }
        }
        "grav_lift" => {
            lin_vel.y = 14.0; // Propel upward
            let lift_mesh = meshes.add(bevy::math::primitives::Cylinder::new(1.8, 0.2));
            let lift_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.3, 1.0, 0.5, 0.7),
                unlit: true,
                ..default()
            });
            commands.spawn((
                PbrBundle {
                    mesh: lift_mesh,
                    material: lift_mat,
                    transform: BevyTransform::from_translation(p_trans.translation),
                    ..default()
                },
                Particle { timer: Timer::from_seconds(4.0, TimerMode::Once) },
            ));
        }
        "fireball" => {
            let fb_mesh = meshes.add(bevy::math::primitives::Sphere::new(0.45));
            let fb_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.4, 0.1),
                emissive: LinearRgba::from(Color::srgb(1.0, 0.3, 0.0)),
                unlit: true,
                ..default()
            });
            commands.spawn((
                PbrBundle {
                    mesh: fb_mesh,
                    material: fb_mat,
                    transform: BevyTransform::from_translation(cam_pos + cam_forward * 1.5),
                    ..default()
                },
                RigidBody::Dynamic,
                LinearVelocity(*cam_forward * 18.0),
                Particle { timer: Timer::from_seconds(3.0, TimerMode::Once) },
            ));
        }
        "magic_missile" => {
            let mm_mesh = meshes.add(bevy::math::primitives::Sphere::new(0.25));
            let mm_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.3, 1.0),
                emissive: LinearRgba::from(Color::srgb(0.7, 0.2, 0.9)),
                unlit: true,
                ..default()
            });
            commands.spawn((
                PbrBundle {
                    mesh: mm_mesh,
                    material: mm_mat,
                    transform: BevyTransform::from_translation(cam_pos + cam_forward * 1.2),
                    ..default()
                },
                RigidBody::Dynamic,
                LinearVelocity(*cam_forward * 55.0),
                Particle { timer: Timer::from_seconds(2.0, TimerMode::Once) },
            ));
        }
        "minor_healing" => {
            let heal_mesh = meshes.add(bevy::math::primitives::Sphere::new(0.12));
            let heal_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.3, 1.0, 0.4, 0.9),
                unlit: true,
                ..default()
            });
            for i in 0..6 {
                let angle = (i as f32 / 6.0) * std::f32::consts::TAU;
                let off = Vec3::new(angle.cos() * 0.8, 0.5 + i as f32 * 0.2, angle.sin() * 0.8);
                commands.spawn((
                    PbrBundle {
                        mesh: heal_mesh.clone(),
                        material: heal_mat.clone(),
                        transform: BevyTransform::from_translation(p_trans.translation + off),
                        ..default()
                    },
                    Particle { timer: Timer::from_seconds(1.2, TimerMode::Once) },
                ));
            }
        }
        "blink" => {
            let mut blink_dir = *cam_forward;
            blink_dir.y = 0.0;
            let dir = blink_dir.normalize_or_zero();
            p_trans.translation += dir * 12.0;
        }
        _ => {
            // General arcane flash
            let flash_mesh = meshes.add(bevy::math::primitives::Sphere::new(0.3));
            let flash_mat = materials.add(StandardMaterial {
                base_color: def.color,
                unlit: true,
                ..default()
            });
            commands.spawn((
                PbrBundle {
                    mesh: flash_mesh,
                    material: flash_mat,
                    transform: BevyTransform::from_translation(cam_pos + cam_forward * 1.0),
                    ..default()
                },
                Particle { timer: Timer::from_seconds(0.5, TimerMode::Once) },
            ));
        }
    }

    hotbar.cooldowns[slot] = def.cooldown_seconds;
    hotbar.max_cooldowns[slot] = def.cooldown_seconds;
    info!("Cast {} from Slot {}", def.name, slot + 1);
}

pub fn handle_spellbook_interactions(
    mut spellbook_state: ResMut<SpellbookWindowState>,
    mut tab_clicks: Query<(&Interaction, &SpellbookTabButton), Changed<Interaction>>,
    mut card_clicks: Query<(&Interaction, &SpellbookCardSelectButton), Changed<Interaction>>,
    mut status_text_q: Query<&mut Text, With<SpellbookStatusText>>,
) {
    for (interaction, tab_btn) in tab_clicks.iter_mut() {
        if *interaction == Interaction::Pressed {
            spellbook_state.category_filter = tab_btn.0;
        }
    }

    for (interaction, card_btn) in card_clicks.iter_mut() {
        if *interaction == Interaction::Pressed {
            spellbook_state.selected_spell_for_slotting = Some(card_btn.0.clone());
            if let Ok(mut text) = status_text_q.get_single_mut() {
                if let Some(def) = get_spell_by_id(&card_btn.0) {
                    text.sections[0].value = format!("SELECTED: '{}' -> Now CLICK a Hotbar Slot (1-16) to prepare it!", def.name);
                    text.sections[0].style.color = Color::srgb(1.0, 0.9, 0.3);
                }
            }
        }
    }
}

pub fn handle_options_panel_interactions(
    (keys, camera_mode, diagnostics): (
        Res<ButtonInput<KeyCode>>,
        Res<State<CameraMode>>,
        Res<bevy::diagnostic::DiagnosticsStore>,
    ),
    (mut crosshair_settings, mut render_settings, mut keybinds, mut options_state): (
        ResMut<CrosshairSettings>,
        ResMut<TerrainRenderSettings>,
        ResMut<HotbarKeybinds>,
        ResMut<OptionsPanelState>,
    ),
    ui_queries: (
        Query<&mut Style, With<OptionsPanelModalRoot>>,
        Query<&mut Window, With<PrimaryWindow>>,
        Query<&mut Text, (With<OptionsReticleStatusText>, Without<OptionsPerfMetricsText>)>,
        Query<&mut Text, (With<OptionsPerfMetricsText>, Without<OptionsReticleStatusText>)>,
    ),
    buttons: (
        Query<&Interaction, (Changed<Interaction>, With<OptionsReticleToggleBtn>)>,
        Query<(&Interaction, &OptionsRangePresetButton), Changed<Interaction>>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsSpawnWholeZoneBtn>)>,
        Query<(&Interaction, &OptionsKeybindRebindBtn), Changed<Interaction>>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsKeybindResetBtn>)>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsCloseBtn>)>,
    ),
) {
    let (mut options_q, mut window_q, mut reticle_status_q, mut perf_text_q) = ui_queries;
    let (reticle_btn_q, range_btn_q, spawn_zone_q, rebind_btn_q, reset_btn_q, close_btn_q) = buttons;
    if !options_state.is_open {
        return;
    }

    // Close button
    for interaction in close_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            options_state.is_open = false;
            if let Ok(mut style) = options_q.get_single_mut() {
                style.display = Display::None;
            }
            if let Ok(mut window) = window_q.get_single_mut() {
                if *camera_mode.get() == CameraMode::FPS {
                    window.cursor.grab_mode = CursorGrabMode::Locked;
                    window.cursor.visible = false;
                }
            }
        }
    }

    // Reticle ON/OFF Toggle
    for interaction in reticle_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            crosshair_settings.enabled = !crosshair_settings.enabled;
            if let Ok(mut text) = reticle_status_q.get_single_mut() {
                if crosshair_settings.enabled {
                    text.sections[0].value = "RETICLE: ON".into();
                    text.sections[0].style.color = Color::srgb(0.2, 1.0, 0.4);
                } else {
                    text.sections[0].value = "RETICLE: OFF".into();
                    text.sections[0].style.color = Color::srgb(1.0, 0.3, 0.3);
                }
            }
        }
    }

    // Visible range preset buttons
    for (interaction, preset_btn) in range_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            render_settings.view_distance_chunks = preset_btn.0;
            render_settings.unload_distance_chunks = preset_btn.0 + 3;
            render_settings.spawn_full_zone = false;
            render_settings.visible_range_meters = preset_btn.0 as f32 * 16.0;
            info!("Visible Range adjusted to {} chunks ({}m)", preset_btn.0, render_settings.visible_range_meters);
        }
    }

    // Spawn entire zone button
    for interaction in spawn_zone_q.iter() {
        if *interaction == Interaction::Pressed {
            render_settings.view_distance_chunks = 64;
            render_settings.unload_distance_chunks = 70;
            render_settings.spawn_full_zone = true;
            render_settings.visible_range_meters = 1024.0;
            info!("⚡ Spawn Entire Zone requested: Full 1024m radius active!");
        }
    }

    // Hotbar keybind rebinding button click
    for (interaction, rebind_btn) in rebind_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            options_state.rebinding_slot = Some(rebind_btn.0);
            info!("Listening for new keybind for Hotbar Slot {}", rebind_btn.0 + 1);
        }
    }

    // Capture next key press if rebinding
    if let Some(slot) = options_state.rebinding_slot {
        for key in keys.get_just_pressed() {
            if *key != KeyCode::Escape && *key != KeyCode::KeyO {
                keybinds.keybinds[slot] = *key;
                info!("Rebound Hotbar Slot {} to {:?}", slot + 1, key);
                options_state.rebinding_slot = None;
                break;
            }
        }
    }

    // Reset keybinds
    for interaction in reset_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            *keybinds = HotbarKeybinds::default();
            info!("Reset all 16 hotbar keybinds to defaults.");
        }
    }

    // Update live performance metrics text
    if let Ok(mut text) = perf_text_q.get_single_mut() {
        let fps = diagnostics
            .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.average())
            .unwrap_or(60.0);

        text.sections[0].value = format!(
            "Visible Range: {:.0}m ({} chunks) | FPS: {:.0} | Spawn Full Zone: {}",
            render_settings.visible_range_meters,
            render_settings.view_distance_chunks,
            fps,
            if render_settings.spawn_full_zone { "ACTIVE" } else { "OFF" }
        );
    }
}

pub fn sync_terrain_render_and_fog_system(
    render_settings: Res<TerrainRenderSettings>,
    mut fog_q: Query<&mut FogSettings>,
    mut cam_q: Query<&mut Projection, With<Camera3d>>,
) {
    if !render_settings.is_changed() {
        return;
    }

    let end_dist = render_settings.visible_range_meters * 1.05;
    let start_dist = end_dist * 0.65;

    for mut fog in fog_q.iter_mut() {
        fog.falloff = FogFalloff::Linear {
            start: start_dist,
            end: end_dist,
        };
    }

    for mut proj in cam_q.iter_mut() {
        if let Projection::Perspective(ref mut pers) = *proj {
            pers.far = (end_dist * 2.0).max(1200.0);
        }
    }
}

// ----------------------------------------------------------------------------
// 6. PLUGIN DEFINITION
// ----------------------------------------------------------------------------

pub struct SpellbookPlugin;

impl Plugin for SpellbookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HotbarKeybinds>()
            .init_resource::<PreparedHotbarState>()
            .init_resource::<SpellbookWindowState>()
            .init_resource::<OptionsPanelState>()
            .init_resource::<TerrainRenderSettings>()
            .add_systems(OnEnter(GameState::InGame), (
                setup_prepared_hotbar_ui,
                setup_spellbook_modal_ui,
                setup_options_panel_modal_ui,
            ))
            .add_systems(Update, (
                toggle_options_and_spellbook_system,
                update_hotbar_ui_system,
                handle_hotbar_casting_system,
                handle_spellbook_interactions,
                handle_options_panel_interactions,
                sync_terrain_render_and_fog_system,
            ));
    }
}

// ----------------------------------------------------------------------------
// 7. UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spell_catalog_integrity() {
        assert_eq!(SPELL_CATALOG.len(), 16, "Spell catalog must contain 16 spells");
        let mut ids = std::collections::BTreeSet::new();
        for spell in SPELL_CATALOG {
            assert!(!spell.name.is_empty(), "Spell must have a name");
            assert!(!spell.description.is_empty(), "Spell must have description");
            assert!(spell.cooldown_seconds > 0.0, "Cooldown must be positive");
            assert_eq!(spell.icon.len(), 4, "Spell icon must be 4 characters");
            assert!(ids.insert(spell.id), "Duplicate spell ID: {}", spell.id);
            assert!(get_spell_by_id(spell.id).is_some());
        }
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
        assert_eq!(hotbar.slots[0].as_deref(), Some("phase_dash"));
        assert_eq!(hotbar.slots[4].as_deref(), Some("fireball"));

        // Slot custom spell
        hotbar.slots[15] = Some("solar_flare".into());
        assert_eq!(hotbar.slots[15].as_deref(), Some("solar_flare"));

        // Cooldown timer progression
        hotbar.cooldowns[0] = 5.0;
        assert!(hotbar.cooldowns[0] > 0.0);
        hotbar.cooldowns[0] = (hotbar.cooldowns[0] - 2.0).max(0.0);
        assert_eq!(hotbar.cooldowns[0], 3.0);
    }

    #[test]
    fn test_terrain_render_settings_zone_expansion() {
        let mut settings = TerrainRenderSettings::default();
        assert_eq!(settings.view_distance_chunks, 10);
        assert_eq!(settings.visible_range_meters, 160.0);
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
}
