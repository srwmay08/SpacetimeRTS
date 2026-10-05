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
    pub rank: &'static str,
    pub category: SpellCategory,
    pub icon: &'static str,
    pub description: &'static str,
    pub cooldown_seconds: f32,
    pub color: Color,
}

pub const SPELL_CATALOG: &[SpellDef] = &[
    SpellDef {
        id: "entangling_roots",
        name: "Entangling Roots",
        rank: "Rank 1",
        category: SpellCategory::Restoration,
        icon: "ROOT",
        description: "Roots the target in place, inflicting nature damage over time.",
        cooldown_seconds: 12.0,
        color: Color::srgb(0.4, 0.85, 0.3),
    },
    SpellDef {
        id: "moonfire",
        name: "Moonfire",
        rank: "Rank 2",
        category: SpellCategory::Arcane,
        icon: "MOON",
        description: "Strikes the target with lunar radiance, inflicting direct and ongoing arcane burn.",
        cooldown_seconds: 3.0,
        color: Color::srgb(0.6, 0.8, 1.0),
    },
    SpellDef {
        id: "teleport_moonglade",
        name: "Teleport: Moonglade",
        rank: "Rank 1",
        category: SpellCategory::Arcane,
        icon: "TELE",
        description: "Transports the caster across the mystic ley lines to Moonglade.",
        cooldown_seconds: 60.0,
        color: Color::srgb(0.5, 0.9, 0.8),
    },
    SpellDef {
        id: "thorns",
        name: "Thorns",
        rank: "Rank 1",
        category: SpellCategory::Restoration,
        icon: "THRN",
        description: "Sprouts protective briars, dealing return nature damage to attackers.",
        cooldown_seconds: 20.0,
        color: Color::srgb(0.35, 0.75, 0.3),
    },
    SpellDef {
        id: "wrath",
        name: "Wrath",
        rank: "Rank 2",
        category: SpellCategory::Elemental,
        icon: "WRTH",
        description: "Hurls a bolt of solar fury at the target, striking with searing solar force.",
        cooldown_seconds: 2.0,
        color: Color::srgb(0.95, 0.85, 0.25),
    },
    SpellDef {
        id: "phase_dash",
        name: "Phase Dash",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "DASH",
        description: "Surges 32m/s forward or strafe-direction with aerodynamic slipstream.",
        cooldown_seconds: 6.0,
        color: Color::srgb(0.0, 0.9, 0.9),
    },
    SpellDef {
        id: "smoke_veil",
        name: "Smoke Veil",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "SMOK",
        description: "Deploys a dense volumetric smoke cloud obscuring lines of sight for 8s.",
        cooldown_seconds: 18.0,
        color: Color::srgb(0.65, 0.70, 0.75),
    },
    SpellDef {
        id: "intel_dart",
        name: "Intel Dart",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "INTL",
        description: "Fires a high-velocity sonar ping dart revealing hidden entities.",
        cooldown_seconds: 22.0,
        color: Color::srgb(1.0, 0.85, 0.2),
    },
    SpellDef {
        id: "grav_lift",
        name: "Grav-Lift",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "LIFT",
        description: "Projects an anti-gravity vertical repulsion column propelling player upward.",
        cooldown_seconds: 15.0,
        color: Color::srgb(0.3, 0.95, 0.45),
    },
    SpellDef {
        id: "fireball",
        name: "Fireball",
        rank: "Rank 3",
        category: SpellCategory::Elemental,
        icon: "FIRE",
        description: "Launches a slow, heavy incendiary sphere detonating with 4.5m blast radius.",
        cooldown_seconds: 4.0,
        color: Color::srgb(1.0, 0.35, 0.1),
    },
    SpellDef {
        id: "magic_missile",
        name: "Magic Missile",
        rank: "Rank 2",
        category: SpellCategory::Arcane,
        icon: "MISS",
        description: "Discharges a rapid 55m/s zero-gravity arcane projectile.",
        cooldown_seconds: 1.5,
        color: Color::srgb(0.75, 0.3, 0.95),
    },
    SpellDef {
        id: "frost_nova",
        name: "Frost Nova",
        rank: "Rank 1",
        category: SpellCategory::Elemental,
        icon: "NOVA",
        description: "Radial freezing explosion chilling all nearby hostile creatures.",
        cooldown_seconds: 8.0,
        color: Color::srgb(0.4, 0.8, 1.0),
    },
    SpellDef {
        id: "minor_healing",
        name: "Rejuvenation",
        rank: "Rank 2",
        category: SpellCategory::Restoration,
        icon: "HEAL",
        description: "Restores vital health with a rejuvenating surge of golden radiance.",
        cooldown_seconds: 10.0,
        color: Color::srgb(0.3, 1.0, 0.5),
    },
    SpellDef {
        id: "blink",
        name: "Blink",
        rank: "Rank 1",
        category: SpellCategory::Arcane,
        icon: "BLNK",
        description: "Instantly teleports 12m forward across the physical plane.",
        cooldown_seconds: 12.0,
        color: Color::srgb(0.85, 0.4, 0.9),
    },
    SpellDef {
        id: "chain_lightning",
        name: "Chain Lightning",
        rank: "Rank 1",
        category: SpellCategory::Elemental,
        icon: "LGHT",
        description: "Discharges crackling high-voltage lightning arcing through targets.",
        cooldown_seconds: 9.0,
        color: Color::srgb(0.95, 0.9, 0.3),
    },
    SpellDef {
        id: "stoneskin",
        name: "Barkskin",
        rank: "Rank 1",
        category: SpellCategory::Restoration,
        icon: "BARK",
        description: "Hardens flesh into earthen armor, absorbing incoming impact damage.",
        cooldown_seconds: 25.0,
        color: Color::srgb(0.75, 0.6, 0.4),
    },
    SpellDef {
        id: "starfall",
        name: "Starfall",
        rank: "Rank 1",
        category: SpellCategory::Arcane,
        icon: "STAR",
        description: "Calls down celestial radiant sparks from the binary star system.",
        cooldown_seconds: 30.0,
        color: Color::srgb(0.95, 0.8, 1.0),
    },
    SpellDef {
        id: "spirit_familiar",
        name: "Bear Form",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "BEAR",
        description: "Transforms into a fierce beast, increasing health and melee durability.",
        cooldown_seconds: 40.0,
        color: Color::srgb(0.4, 0.9, 0.85),
    },
    SpellDef {
        id: "war_cry",
        name: "Demoralizing Roar",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "ROAR",
        description: "Thunderous acoustic roar delivering kinetic knockback to nearby foes.",
        cooldown_seconds: 20.0,
        color: Color::srgb(0.9, 0.3, 0.3),
    },
    SpellDef {
        id: "shadow_cloak",
        name: "Prowl",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "PROW",
        description: "Envelops the player in shadows, reducing visual aggro profile.",
        cooldown_seconds: 35.0,
        color: Color::srgb(0.45, 0.4, 0.55),
    },
    SpellDef {
        id: "solar_flare",
        name: "Sunfire",
        rank: "Rank 1",
        category: SpellCategory::Elemental,
        icon: "SUNF",
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
        Self {
            slots: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            cooldowns: [0.0; 16],
            max_cooldowns: [1.0; 16],
        }
    }
}

#[derive(Resource)]
pub struct SpellbookWindowState {
    pub is_open: bool,
    pub selected_spell_for_slotting: Option<String>,
    pub category_filter: Option<SpellCategory>,
    pub current_page: usize,
    pub rank_filter: bool,
    pub auto_uprank: bool,
}

impl Default for SpellbookWindowState {
    fn default() -> Self {
        Self {
            is_open: false,
            selected_spell_for_slotting: None,
            category_filter: None,
            current_page: 0,
            rank_filter: false,
            auto_uprank: true,
        }
    }
}

pub fn get_filtered_spells(category: Option<SpellCategory>, rank_filter: bool) -> Vec<&'static SpellDef> {
    SPELL_CATALOG.iter().filter(|s| {
        if let Some(cat) = category {
            if s.category != cat {
                return false;
            }
        }
        if rank_filter {
            s.rank.contains('1')
        } else {
            true
        }
    }).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OptionsTab {
    #[default]
    Graphics,
    Advanced,
    Keybinds,
    Network,
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
            active_tab: OptionsTab::Graphics,
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
            view_distance_chunks: 14, // 224m default (matches terrain LOW_POLY_RADIUS_CHUNKS: 14, 20% reduction)
            unload_distance_chunks: 15, // 240m default (matches terrain LOW_POLY_UNLOAD_RADIUS_CHUNKS: 15, immediately beyond 230.4m fog limit)
            spawn_full_zone: false,
            visible_range_meters: 230.4,
        }
    }
}

// ----------------------------------------------------------------------------
// 3. UI MARKER COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Resource, Default, Debug)]
pub struct SpellDragState {
    pub is_dragging: bool,
    pub source_slot: Option<usize>,
    pub spell_id: Option<String>,
    pub current_pos: Vec2,
}

#[derive(Component)] pub struct SpellDragGhostUi;
#[derive(Component)] pub struct SpellDragGhostText;

#[derive(Component)] pub struct PreparedHotbarRoot;
#[derive(Component)] pub struct HotbarSlotButton(pub usize);
#[derive(Component)] pub struct HotbarSlotKeyText(pub usize);
#[derive(Component)] pub struct HotbarSlotNameText(pub usize);
#[derive(Component)] pub struct HotbarSlotCooldownText(pub usize);
#[derive(Component)] pub struct HotbarOpenSpellbookButton;

#[derive(Component)] pub struct SpellbookModalRoot;
#[derive(Component)] pub struct SpellbookCloseBtn;
#[derive(Component)] pub struct SpellbookTabButton(pub Option<SpellCategory>);
#[derive(Component)] pub struct SpellbookSlotCard(pub usize);
#[derive(Component)] pub struct SpellbookSlotIconBox(pub usize);
#[derive(Component)] pub struct SpellbookSlotIconText(pub usize);
#[derive(Component)] pub struct SpellbookSlotNameText(pub usize);
#[derive(Component)] pub struct SpellbookSlotRankText(pub usize);
#[derive(Component)] pub struct SpellbookSlotCooldownText(pub usize);
#[derive(Component)] pub struct SpellbookPrevPageBtn;
#[derive(Component)] pub struct SpellbookNextPageBtn;
#[derive(Component)] pub struct SpellbookPageIndicatorText;
#[derive(Component)] pub struct SpellbookRankFilterToggle;
#[derive(Component)] pub struct SpellbookAutoUpRankToggle;
#[derive(Component)] pub struct SpellbookRankFilterText;
#[derive(Component)] pub struct SpellbookAutoUpRankText;
#[derive(Component)] pub struct SpellbookStatusText;
#[derive(Component)] pub struct SpellbookCardSelectButton(pub String);

#[derive(Component)] pub struct OptionsPanelModalRoot;
#[derive(Component)] pub struct OptionsTabButton(pub OptionsTab);
#[derive(Component)] pub struct OptionsTabButtonText(pub OptionsTab);
#[derive(Component)] pub struct OptionsTabContent(pub OptionsTab);
#[derive(Component)] pub struct OptionsReticleToggleBtn;
#[derive(Component)] pub struct OptionsReticleStatusText;
#[derive(Component)] pub struct OptionsRangePresetButton(pub i32);
#[derive(Component)] pub struct OptionsSpawnWholeZoneBtn;
#[derive(Component)] pub struct OptionsPerfMetricsText;
#[derive(Component)] pub struct OptionsKeybindRebindBtn(pub usize);
#[derive(Component)] pub struct OptionsKeybindRebindText(pub usize);
#[derive(Component)] pub struct OptionsKeybindResetBtn;
#[derive(Component)] pub struct OptionsCloseBtn;
#[derive(Component)] pub struct OptionsOkayBtn;
#[derive(Component)] pub struct OptionsCancelBtn;
#[derive(Component)] pub struct OptionsRecommendedBtn;
#[derive(Component)] pub struct OptionsClassicBtn;
#[derive(Component)] pub struct OptionsApplyBtn;

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
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                width: Val::Auto,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            z_index: ZIndex::Global(200),
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
                                    width: Val::Px(50.0),
                                    height: Val::Px(50.0),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::SpaceBetween,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::all(Val::Px(3.0)),
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.35, 0.35, 0.35)),
                                background_color: BackgroundColor(Color::srgba(0.08, 0.08, 0.10, 0.92)),
                                ..default()
                            },
                            HotbarSlotButton(slot_idx),
                        )).with_children(|slot| {
                            // Top bar: Hotkey badge & cooldown
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
                                        TextStyle { font_size: 10.0, color: Color::srgb(1.0, 0.85, 0.3), ..default() }
                                    ),
                                    HotbarSlotKeyText(slot_idx),
                                ));
                                top.spawn((
                                    TextBundle::from_section(
                                        "",
                                        TextStyle { font_size: 10.0, color: Color::srgb(0.9, 0.3, 0.3), ..default() }
                                    ),
                                    HotbarSlotCooldownText(slot_idx),
                                ));
                            });

                            // Center spell name / badge
                            slot.spawn((
                                TextBundle::from_section(
                                    icon_text,
                                    TextStyle { font_size: 12.0, color: icon_color, ..default() }
                                ),
                                HotbarSlotNameText(slot_idx),
                            ));
                        });
                    }
                });
            }
        });
    });

    // Floating Drag Ghost UI for Dragged Spells
    commands.spawn((
        NodeBundle {
            focus_policy: bevy::ui::FocusPolicy::Pass,
            style: Style {
                position_type: PositionType::Absolute,
                width: Val::Px(50.0),
                height: Val::Px(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.5)),
                display: Display::None,
                ..default()
            },
            border_color: Color::srgba(1.0, 0.85, 0.3, 0.95).into(),
            background_color: Color::srgba(0.12, 0.10, 0.15, 0.92).into(),
            z_index: ZIndex::Global(999),
            ..default()
        },
        SpellDragGhostUi,
    )).with_children(|ghost| {
        ghost.spawn((
            TextBundle::from_section(
                "",
                TextStyle { font_size: 11.0, color: Color::srgb(1.0, 0.9, 0.5), ..default() }
            ),
            SpellDragGhostText,
        ));
    });
}

pub fn setup_spellbook_modal_ui(
    mut commands: Commands,
) {
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-410.0)),
                width: Val::Auto,
                height: Val::Auto,
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
        // Horizontal container: Open Book + Right-side vertical tabs
        overlay.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            ..default()
        }).with_children(|wrapper| {
            // Main Open Parchment Book Frame
            wrapper.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(780.0),
                    height: Val::Px(570.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    border: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                border_color: BorderColor(Color::srgb(0.24, 0.16, 0.10)),
                background_color: BackgroundColor(Color::srgba(0.88, 0.81, 0.68, 0.98)),
                ..default()
            }).with_children(|book| {
                // Header: Title + Checkboxes + Red Close Button
                book.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                    ..default()
                }).with_children(|header| {
                    header.spawn(TextBundle::from_section(
                        "📖 Spellbook & Abilities",
                        TextStyle { font_size: 17.0, color: Color::srgb(0.24, 0.15, 0.08), ..default() }
                    ));

                    header.spawn(NodeBundle {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(10.0),
                            ..default()
                        },
                        ..default()
                    }).with_children(|ctrls| {
                        // Rank Filter Checkbox Toggle
                        ctrls.spawn((
                            ButtonBundle {
                                style: Style {
                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.40, 0.30, 0.20)),
                                background_color: BackgroundColor(Color::srgb(0.80, 0.73, 0.60)),
                                ..default()
                            },
                            SpellbookRankFilterToggle,
                        )).with_children(|b| {
                            b.spawn((
                                TextBundle::from_section(
                                    "[ ] Rank Filter",
                                    TextStyle { font_size: 11.0, color: Color::srgb(0.22, 0.14, 0.08), ..default() }
                                ),
                                SpellbookRankFilterText,
                            ));
                        });

                        // Auto UpRank Checkbox Toggle
                        ctrls.spawn((
                            ButtonBundle {
                                style: Style {
                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.40, 0.30, 0.20)),
                                background_color: BackgroundColor(Color::srgb(0.80, 0.73, 0.60)),
                                ..default()
                            },
                            SpellbookAutoUpRankToggle,
                        )).with_children(|b| {
                            b.spawn((
                                TextBundle::from_section(
                                    "[X] Auto UpRank",
                                    TextStyle { font_size: 11.0, color: Color::srgb(0.22, 0.14, 0.08), ..default() }
                                ),
                                SpellbookAutoUpRankText,
                            ));
                        });

                        // Red [X] Close Button
                        ctrls.spawn((
                            ButtonBundle {
                                style: Style {
                                    width: Val::Px(24.0),
                                    height: Val::Px(24.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.45, 0.10, 0.10)),
                                background_color: BackgroundColor(Color::srgb(0.72, 0.15, 0.15)),
                                ..default()
                            },
                            SpellbookCloseBtn,
                        )).with_children(|b| {
                            b.spawn(TextBundle::from_section(
                                "X",
                                TextStyle { font_size: 12.0, color: Color::WHITE, ..default() }
                            ));
                        });
                    });
                });

                // Two-Column Pages Body: 6 slots on Left page, 6 slots on Right page (12 total per page)
                book.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(14.0),
                        ..default()
                    },
                    ..default()
                }).with_children(|pages_row| {
                    // Left Page (Slots 0..6)
                    pages_row.spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(50.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            padding: UiRect::right(Val::Px(8.0)),
                            border: UiRect::right(Val::Px(1.5)),
                            ..default()
                        },
                        border_color: BorderColor(Color::srgb(0.74, 0.66, 0.52)),
                        ..default()
                    }).with_children(|left_col| {
                        for slot_idx in 0..6 {
                            spawn_spellbook_slot(left_col, slot_idx);
                        }
                    });

                    // Right Page (Slots 6..12)
                    pages_row.spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(50.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            padding: UiRect::left(Val::Px(8.0)),
                            ..default()
                        },
                        ..default()
                    }).with_children(|right_col| {
                        for slot_idx in 6..12 {
                            spawn_spellbook_slot(right_col, slot_idx);
                        }
                    });
                });

                // Bottom Bar: Hint Status + Pagination (< Page X of Y >)
                book.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        margin: UiRect::top(Val::Px(8.0)),
                        padding: UiRect::top(Val::Px(6.0)),
                        border: UiRect::top(Val::Px(1.5)),
                        ..default()
                    },
                    border_color: BorderColor(Color::srgb(0.74, 0.66, 0.52)),
                    ..default()
                }).with_children(|bottom| {
                    // Left: status / prompt text
                    bottom.spawn((
                        TextBundle::from_section(
                            "Click any spell to select, then click Hotbar Slot (1-16) to prepare | [K to Close]",
                            TextStyle { font_size: 10.5, color: Color::srgb(0.32, 0.22, 0.14), ..default() }
                        ),
                        SpellbookStatusText,
                    ));

                    // Right: Pagination Controls
                    bottom.spawn(NodeBundle {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        },
                        ..default()
                    }).with_children(|paging| {
                        paging.spawn((
                            ButtonBundle {
                                style: Style {
                                    padding: UiRect::axes(Val::Px(9.0), Val::Px(3.0)),
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.40, 0.28, 0.18)),
                                background_color: BackgroundColor(Color::srgb(0.78, 0.70, 0.58)),
                                ..default()
                            },
                            SpellbookPrevPageBtn,
                        )).with_children(|b| {
                            b.spawn(TextBundle::from_section(
                                "<",
                                TextStyle { font_size: 12.0, color: Color::srgb(0.22, 0.14, 0.08), ..default() }
                            ));
                        });

                        paging.spawn((
                            TextBundle::from_section(
                                "Page 1 of 2",
                                TextStyle { font_size: 11.5, color: Color::srgb(0.22, 0.14, 0.08), ..default() }
                            ),
                            SpellbookPageIndicatorText,
                        ));

                        paging.spawn((
                            ButtonBundle {
                                style: Style {
                                    padding: UiRect::axes(Val::Px(9.0), Val::Px(3.0)),
                                    border: UiRect::all(Val::Px(1.5)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.40, 0.28, 0.18)),
                                background_color: BackgroundColor(Color::srgb(0.78, 0.70, 0.58)),
                                ..default()
                            },
                            SpellbookNextPageBtn,
                        )).with_children(|b| {
                            b.spawn(TextBundle::from_section(
                                ">",
                                TextStyle { font_size: 12.0, color: Color::srgb(0.22, 0.14, 0.08), ..default() }
                            ));
                        });
                    });
                });
            });

            // Right-side school / category tabs (attached to right book frame)
            wrapper.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(82.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    margin: UiRect::top(Val::Px(36.0)),
                    ..default()
                },
                ..default()
            }).with_children(|tabs_col| {
                let tabs_def = [
                    (None, "ALL", 21),
                    (Some(SpellCategory::Arcane), "ARCANE", 5),
                    (Some(SpellCategory::Elemental), "ELEMENT", 5),
                    (Some(SpellCategory::Restoration), "RESTORE", 5),
                    (Some(SpellCategory::Tactical), "TACTICS", 6),
                ];

                for (cat, label, count) in tabs_def {
                    tabs_col.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(82.0),
                                height: Val::Px(44.0),
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(3.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.40, 0.30, 0.20)),
                            background_color: BackgroundColor(Color::srgb(0.24, 0.17, 0.12)),
                            ..default()
                        },
                        SpellbookTabButton(cat),
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            label,
                            TextStyle { font_size: 10.5, color: Color::srgb(0.92, 0.85, 0.70), ..default() }
                        ));
                        b.spawn(TextBundle::from_section(
                            format!("({})", count),
                            TextStyle { font_size: 9.0, color: Color::srgb(0.72, 0.65, 0.50), ..default() }
                        ));
                    });
                }
            });
        });
    });
}

fn spawn_spellbook_slot(builder: &mut ChildBuilder, slot_idx: usize) {
    builder.spawn((
        ButtonBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Px(60.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.5)),
                column_gap: Val::Px(8.0),
                ..default()
            },
            border_color: BorderColor(Color::srgb(0.70, 0.62, 0.48)),
            background_color: BackgroundColor(Color::srgba(0.84, 0.77, 0.64, 0.40)),
            ..default()
        },
        SpellbookSlotCard(slot_idx),
    )).with_children(|card| {
        // Square Icon Box
        card.spawn((
            NodeBundle {
                style: Style {
                    width: Val::Px(44.0),
                    height: Val::Px(44.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                border_color: BorderColor(Color::srgb(0.32, 0.22, 0.14)),
                background_color: BackgroundColor(Color::srgb(0.18, 0.14, 0.10)),
                ..default()
            },
            SpellbookSlotIconBox(slot_idx),
        )).with_children(|ibox| {
            ibox.spawn((
                TextBundle::from_section(
                    "---",
                    TextStyle { font_size: 11.0, color: Color::srgb(0.5, 0.5, 0.5), ..default() }
                ),
                SpellbookSlotIconText(slot_idx),
            ));
        });

        // Details Column
        card.spawn(NodeBundle {
            style: Style {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceEvenly,
                ..default()
            },
            ..default()
        }).with_children(|info| {
            info.spawn((
                TextBundle::from_section(
                    "(Empty Slot)",
                    TextStyle { font_size: 12.0, color: Color::srgb(0.20, 0.12, 0.06), ..default() }
                ),
                SpellbookSlotNameText(slot_idx),
            ));

            info.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                ..default()
            }).with_children(|sub| {
                sub.spawn((
                    TextBundle::from_section(
                        "",
                        TextStyle { font_size: 10.0, color: Color::srgb(0.48, 0.38, 0.26), ..default() }
                    ),
                    SpellbookSlotRankText(slot_idx),
                ));

                sub.spawn((
                    TextBundle::from_section(
                        "",
                        TextStyle { font_size: 9.5, color: Color::srgb(0.55, 0.40, 0.25), ..default() }
                    ),
                    SpellbookSlotCooldownText(slot_idx),
                ));
            });
        });
    });
}

fn spawn_framed_section<F>(parent: &mut ChildBuilder, title: &'static str, build_content: F)
where
    F: FnOnce(&mut ChildBuilder),
{
    parent.spawn(NodeBundle {
        style: Style {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            margin: UiRect::bottom(Val::Px(4.0)),
            ..default()
        },
        border_color: BorderColor(Color::srgb(0.20, 0.22, 0.25)),
        background_color: BackgroundColor(Color::srgba(0.06, 0.07, 0.09, 0.7)),
        ..default()
    }).with_children(|sec| {
        sec.spawn(TextBundle::from_section(
            title,
            TextStyle { font_size: 12.0, color: Color::srgb(1.0, 0.85, 0.35), ..default() }
        ).with_style(Style { margin: UiRect::bottom(Val::Px(6.0)), ..default() }));

        build_content(sec);
    });
}

pub fn setup_options_panel_modal_ui(
    mut commands: Commands,
    keybinds: Res<HotbarKeybinds>,
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
        // Main WoW Dialog Window Frame
        overlay.spawn(NodeBundle {
            style: Style {
                width: Val::Px(840.0),
                height: Val::Px(580.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            border_color: BorderColor(Color::srgb(0.35, 0.38, 0.42)),
            background_color: BackgroundColor(Color::srgba(0.04, 0.05, 0.07, 0.98)),
            ..default()
        }).with_children(|window| {
            // 1. Top Header Plate ("System") + Close [X] Button
            window.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                ..default()
            }).with_children(|head| {
                head.spawn(NodeBundle {
                    style: Style { width: Val::Px(24.0), height: Val::Px(24.0), ..default() },
                    ..default()
                });

                // WoW Centered Title Badge
                head.spawn(NodeBundle {
                    style: Style {
                        padding: UiRect::axes(Val::Px(28.0), Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    border_color: BorderColor(Color::srgb(0.55, 0.46, 0.24)),
                    background_color: BackgroundColor(Color::srgb(0.12, 0.11, 0.10)),
                    ..default()
                }).with_children(|b| {
                    b.spawn(TextBundle::from_section(
                        "System",
                        TextStyle { font_size: 15.0, color: Color::srgb(1.0, 0.85, 0.30), ..default() }
                    ));
                });

                // Close Button [X]
                head.spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        border_color: BorderColor(Color::srgb(0.5, 0.2, 0.2)),
                        background_color: BackgroundColor(Color::srgb(0.35, 0.12, 0.12)),
                        ..default()
                    },
                    OptionsCloseBtn,
                )).with_children(|b| {
                    b.spawn(TextBundle::from_section("X", TextStyle { font_size: 12.0, color: Color::WHITE, ..default() }));
                });
            });

            // 2. Middle Area: Two-Column Layout (Sidebar + Content Panel)
            window.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                ..default()
            }).with_children(|columns| {
                // Left Column: Navigation Sidebar (160px)
                columns.spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(160.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        padding: UiRect::all(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    border_color: BorderColor(Color::srgb(0.24, 0.26, 0.30)),
                    background_color: BackgroundColor(Color::srgba(0.03, 0.04, 0.05, 0.9)),
                    ..default()
                }).with_children(|sidebar| {
                    let nav_tabs = [
                        (OptionsTab::Graphics, "Graphics"),
                        (OptionsTab::Advanced, "Advanced"),
                        (OptionsTab::Keybinds, "Keybindings"),
                        (OptionsTab::Network, "Network"),
                    ];
                    for (tab, label) in nav_tabs {
                        let is_active = tab == OptionsTab::Graphics;
                        let bg_col = if is_active { Color::srgba(0.85, 0.70, 0.15, 0.45) } else { Color::NONE };
                        let border_col = if is_active { Color::srgb(1.0, 0.85, 0.25) } else { Color::NONE };
                        let text_col = if is_active { Color::srgb(1.0, 0.90, 0.35) } else { Color::srgb(0.80, 0.80, 0.80) };

                        sidebar.spawn((
                            ButtonBundle {
                                style: Style {
                                    width: Val::Percent(100.0),
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    justify_content: JustifyContent::FlexStart,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                border_color: BorderColor(border_col),
                                background_color: BackgroundColor(bg_col),
                                ..default()
                            },
                            OptionsTabButton(tab),
                        )).with_children(|b| {
                            b.spawn((
                                TextBundle::from_section(
                                    label,
                                    TextStyle { font_size: 13.0, color: text_col, ..default() }
                                ),
                                OptionsTabButtonText(tab),
                            ));
                        });
                    }
                });

                // Right Column: Main Content Area
                columns.spawn(NodeBundle {
                    style: Style {
                        flex_grow: 1.0,
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::all(Val::Px(12.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    border_color: BorderColor(Color::srgb(0.24, 0.26, 0.30)),
                    background_color: BackgroundColor(Color::srgba(0.03, 0.04, 0.05, 0.9)),
                    ..default()
                }).with_children(|content_box| {
                    // ---- TAB 1: GRAPHICS ----
                    content_box.spawn((
                        NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                display: Display::Flex,
                                ..default()
                            },
                            ..default()
                        },
                        OptionsTabContent(OptionsTab::Graphics),
                    )).with_children(|tab| {
                        // Display Box
                        spawn_framed_section(tab, "Display", |sec| {
                            sec.spawn(NodeBundle {
                                style: Style {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::SpaceBetween,
                                    align_items: AlignItems::Center,
                                    margin: UiRect::bottom(Val::Px(4.0)),
                                    ..default()
                                },
                                ..default()
                            }).with_children(|row| {
                                row.spawn(TextBundle::from_section(
                                    "Center Reticle / Crosshair:",
                                    TextStyle { font_size: 12.0, color: Color::srgb(0.85, 0.85, 0.85), ..default() }
                                ));

                                row.spawn((
                                    ButtonBundle {
                                        style: Style {
                                            padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)),
                                            border: UiRect::all(Val::Px(1.0)),
                                            ..default()
                                        },
                                        border_color: BorderColor(Color::srgb(0.2, 0.7, 0.3)),
                                        background_color: BackgroundColor(Color::srgb(0.1, 0.35, 0.15)),
                                        ..default()
                                    },
                                    OptionsReticleToggleBtn,
                                )).with_children(|b| {
                                    b.spawn((
                                        TextBundle::from_section("RETICLE: ON", TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }),
                                        OptionsReticleStatusText,
                                    ));
                                });
                            });
                        });

                        // Environment & Visible Range Box
                        spawn_framed_section(tab, "Environment & Visible Range", |sec| {
                            sec.spawn(TextBundle::from_section(
                                "Chunk View Distance:",
                                TextStyle { font_size: 11.5, color: Color::srgb(0.8, 0.8, 0.8), ..default() }
                            ).with_style(Style { margin: UiRect::bottom(Val::Px(4.0)), ..default() }));

                            sec.spawn(NodeBundle {
                                style: Style {
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(5.0),
                                    margin: UiRect::bottom(Val::Px(8.0)),
                                    ..default()
                                },
                                ..default()
                            }).with_children(|presets_row| {
                                let presets = [
                                    (10, "160m (10c)"),
                                    (16, "256m (16c)"),
                                    (24, "384m (24c)"),
                                    (32, "512m (32c)"),
                                    (48, "768m (48c)"),
                                    (64, "1024m (64c)"),
                                ];
                                for (chunks, lbl) in presets {
                                    presets_row.spawn((
                                        ButtonBundle {
                                            style: Style {
                                                padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                                                border: UiRect::all(Val::Px(1.0)),
                                                ..default()
                                            },
                                            border_color: BorderColor(Color::srgb(0.3, 0.4, 0.5)),
                                            background_color: BackgroundColor(Color::srgb(0.12, 0.18, 0.25)),
                                            ..default()
                                        },
                                        OptionsRangePresetButton(chunks),
                                    )).with_children(|b| {
                                        b.spawn(TextBundle::from_section(lbl, TextStyle { font_size: 10.0, color: Color::WHITE, ..default() }));
                                    });
                                }
                            });

                            // Spawn entire zone button
                            sec.spawn((
                                ButtonBundle {
                                    style: Style {
                                        width: Val::Percent(100.0),
                                        padding: UiRect::all(Val::Px(6.0)),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(Val::Px(1.5)),
                                        ..default()
                                    },
                                    border_color: BorderColor(Color::srgb(0.8, 0.65, 0.2)),
                                    background_color: BackgroundColor(Color::srgb(0.30, 0.22, 0.08)),
                                    ..default()
                                },
                                OptionsSpawnWholeZoneBtn,
                            )).with_children(|b| {
                                b.spawn(TextBundle::from_section(
                                    "⚡ SPAWN ENTIRE ZONE (LOAD FULL 1KM RADIUS & BENCHMARK FPS)",
                                    TextStyle { font_size: 11.0, color: Color::srgb(1.0, 0.85, 0.3), ..default() }
                                ));
                            });
                        });

                        // Performance Box
                        spawn_framed_section(tab, "Display Performance", |sec| {
                            sec.spawn((
                                TextBundle::from_section(
                                    "Visible Range: 160m (10 chunks) | Camera Far: 1000m | FPS: 60",
                                    TextStyle { font_size: 11.0, color: Color::srgb(0.65, 0.9, 0.65), ..default() }
                                ),
                                OptionsPerfMetricsText,
                            ));
                        });
                    });

                    // ---- TAB 2: ADVANCED ----
                    content_box.spawn((
                        NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                display: Display::None,
                                ..default()
                            },
                            ..default()
                        },
                        OptionsTabContent(OptionsTab::Advanced),
                    )).with_children(|tab| {
                        spawn_framed_section(tab, "Advanced Graphics & Terrain Caching", |sec| {
                            sec.spawn(TextBundle::from_section(
                                "• Chunk Caching Buffer: Auto-allocated (View Distance + 3 chunks)\n\
                                 • Atmospheric Fog Falloff: Linear depth attenuation synchronized with horizon\n\
                                 • Camera Projection Far Plane: Scaled dynamically up to 2048m\n\
                                 • Dynamic Weapon Bloom: Real-time recoil spread recovery\n\
                                 • Voxel Render Layer: Layer 1 Viewmodel Culling Isolation",
                                TextStyle { font_size: 11.5, color: Color::srgb(0.8, 0.85, 0.9), ..default() }
                            ));
                        });

                        spawn_framed_section(tab, "In-Game Diagnostics & Tools", |sec| {
                            sec.spawn(TextBundle::from_section(
                                "• Press [F3] to toggle live FPS and engine memory diagnostics overlay\n\
                                 • Press [F8 / F9] to advance or reverse binary astronomical solar time\n\
                                 • Press [F10] to toggle celestial ephemeris & atmospheric weather pill\n\
                                 • Press [Enter] to open in-game developer admin & cheat console",
                                TextStyle { font_size: 11.5, color: Color::srgb(0.85, 0.85, 0.75), ..default() }
                            ));
                        });
                    });

                    // ---- TAB 3: KEYBINDS ----
                    content_box.spawn((
                        NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                display: Display::None,
                                ..default()
                            },
                            ..default()
                        },
                        OptionsTabContent(OptionsTab::Keybinds),
                    )).with_children(|tab| {
                        spawn_framed_section(tab, "16-Key Prepared Hotbar Keybindings (Click to Rebind)", |sec| {
                            sec.spawn(NodeBundle {
                                style: Style {
                                    flex_direction: FlexDirection::Row,
                                    flex_wrap: FlexWrap::Wrap,
                                    column_gap: Val::Px(6.0),
                                    row_gap: Val::Px(5.0),
                                    margin: UiRect::bottom(Val::Px(8.0)),
                                    ..default()
                                },
                                ..default()
                            }).with_children(|rebind_grid| {
                                for slot in 0..16 {
                                    let key_label = keycode_display_name(keybinds.keybinds[slot]);
                                    rebind_grid.spawn((
                                        ButtonBundle {
                                            style: Style {
                                                width: Val::Px(68.0),
                                                height: Val::Px(28.0),
                                                justify_content: JustifyContent::Center,
                                                align_items: AlignItems::Center,
                                                border: UiRect::all(Val::Px(1.0)),
                                                ..default()
                                            },
                                            border_color: BorderColor(Color::srgb(0.4, 0.4, 0.4)),
                                            background_color: BackgroundColor(Color::srgb(0.16, 0.17, 0.20)),
                                            ..default()
                                        },
                                        OptionsKeybindRebindBtn(slot),
                                    )).with_children(|b| {
                                        b.spawn((
                                            TextBundle::from_section(
                                                format!("S{}: [{}]", slot + 1, key_label),
                                                TextStyle { font_size: 10.0, color: Color::WHITE, ..default() }
                                            ),
                                            OptionsKeybindRebindText(slot),
                                        ));
                                    });
                                }
                            });

                            sec.spawn((
                                ButtonBundle {
                                    style: Style {
                                        align_self: AlignSelf::FlexStart,
                                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                        border: UiRect::all(Val::Px(1.0)),
                                        ..default()
                                    },
                                    border_color: BorderColor(Color::srgb(0.5, 0.3, 0.3)),
                                    background_color: BackgroundColor(Color::srgb(0.25, 0.14, 0.14)),
                                    ..default()
                                },
                                OptionsKeybindResetBtn,
                            )).with_children(|b| {
                                b.spawn(TextBundle::from_section("Reset Default Keybindings", TextStyle { font_size: 10.5, color: Color::WHITE, ..default() }));
                            });
                        });
                    });

                    // ---- TAB 4: NETWORK ----
                    content_box.spawn((
                        NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                display: Display::None,
                                ..default()
                            },
                            ..default()
                        },
                        OptionsTabContent(OptionsTab::Network),
                    )).with_children(|tab| {
                        spawn_framed_section(tab, "SpacetimeDB Networking & Multiplayer Sync", |sec| {
                            sec.spawn(TextBundle::from_section(
                                "• SpacetimeDB Server Engine: Authoritative WebAssembly Module\n\
                                 • Realtime Replication: High-Frequency 60Hz Physics & Transform Loop\n\
                                 • Deterministic Tick Sync: Active\n\
                                 • Low-Frequency Background Loop: 1.0s Economy & Resource Node Regeneration\n\
                                 • High-Frequency Loop: 16ms Combat Event & Active Projectile Trajectory\n\
                                 • Optimize Network for Speed: Enabled",
                                TextStyle { font_size: 11.5, color: Color::srgb(0.75, 0.85, 0.95), ..default() }
                            ));
                        });
                    });

                    // Bottom right Apply button inside content area
                    content_box.spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::FlexEnd,
                            ..default()
                        },
                        ..default()
                    }).with_children(|row| {
                        row.spawn((
                            ButtonBundle {
                                style: Style {
                                    padding: UiRect::axes(Val::Px(24.0), Val::Px(5.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                border_color: BorderColor(Color::srgb(0.4, 0.42, 0.45)),
                                background_color: BackgroundColor(Color::srgb(0.18, 0.20, 0.22)),
                                ..default()
                            },
                            OptionsApplyBtn,
                        )).with_children(|b| {
                            b.spawn(TextBundle::from_section(
                                "Apply",
                                TextStyle { font_size: 11.5, color: Color::srgb(0.8, 0.8, 0.8), ..default() }
                            ));
                        });
                    });
                });
            });

            // 3. Bottom Action Bar: [Recommended] [Classic] ... [Okay] [Cancel]
            window.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
                ..default()
            }).with_children(|bottom| {
                // Left presets buttons: [Recommended] [Classic]
                bottom.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    ..default()
                }).with_children(|left_btns| {
                    left_btns.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(18.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.68, 0.22, 0.22)),
                            background_color: BackgroundColor(Color::srgb(0.42, 0.12, 0.12)),
                            ..default()
                        },
                        OptionsRecommendedBtn,
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            "Recommended",
                            TextStyle { font_size: 12.0, color: Color::srgb(1.0, 0.9, 0.7), ..default() }
                        ));
                    });

                    left_btns.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(18.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.68, 0.22, 0.22)),
                            background_color: BackgroundColor(Color::srgb(0.42, 0.12, 0.12)),
                            ..default()
                        },
                        OptionsClassicBtn,
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            "Classic",
                            TextStyle { font_size: 12.0, color: Color::srgb(1.0, 0.9, 0.7), ..default() }
                        ));
                    });
                });

                // Right confirm buttons: [Okay] [Cancel]
                bottom.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    ..default()
                }).with_children(|right_btns| {
                    right_btns.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(24.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.68, 0.22, 0.22)),
                            background_color: BackgroundColor(Color::srgb(0.42, 0.12, 0.12)),
                            ..default()
                        },
                        OptionsOkayBtn,
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            "Okay",
                            TextStyle { font_size: 12.0, color: Color::srgb(1.0, 0.9, 0.7), ..default() }
                        ));
                    });

                    right_btns.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(24.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.68, 0.22, 0.22)),
                            background_color: BackgroundColor(Color::srgb(0.42, 0.12, 0.12)),
                            ..default()
                        },
                        OptionsCancelBtn,
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            "Cancel",
                            TextStyle { font_size: 12.0, color: Color::srgb(1.0, 0.9, 0.7), ..default() }
                        ));
                    });
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
    spell_drag: Res<SpellDragState>,
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
    if !spell_drag.is_dragging {
        for (interaction, slot_btn) in slot_click_q.iter() {
            if *interaction == Interaction::Pressed {
                let slot = slot_btn.0;
                if let Some(selected_spell) = spellbook_state.selected_spell_for_slotting.take() {
                    hotbar.slots[slot] = Some(selected_spell);
                    hotbar.cooldowns[slot] = 0.0;
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

pub fn update_spellbook_display_system(
    spellbook_state: Res<SpellbookWindowState>,
    mut text_q: Query<&mut Text>,
    page_indicator_entities: Query<Entity, With<SpellbookPageIndicatorText>>,
    rank_filter_entities: Query<Entity, With<SpellbookRankFilterText>>,
    auto_uprank_entities: Query<Entity, With<SpellbookAutoUpRankText>>,
    icon_text_entities: Query<(Entity, &SpellbookSlotIconText)>,
    name_text_entities: Query<(Entity, &SpellbookSlotNameText)>,
    rank_text_entities: Query<(Entity, &SpellbookSlotRankText)>,
    cd_text_entities: Query<(Entity, &SpellbookSlotCooldownText)>,
    mut color_q: Query<(&mut BackgroundColor, &mut BorderColor)>,
    card_entities: Query<(Entity, &SpellbookSlotCard)>,
    icon_box_entities: Query<(Entity, &SpellbookSlotIconBox)>,
    tab_button_entities: Query<(Entity, &SpellbookTabButton)>,
) {
    if !spellbook_state.is_open {
        return;
    }

    let filtered = get_filtered_spells(spellbook_state.category_filter, spellbook_state.rank_filter);
    let total_pages = ((filtered.len() + 11) / 12).max(1);
    let current_page = spellbook_state.current_page.min(total_pages - 1);

    // 1. Page Indicator Text
    if let Ok(entity) = page_indicator_entities.get_single() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            text.sections[0].value = format!("Page {} of {}", current_page + 1, total_pages);
        }
    }

    // 2. Rank Filter Checkbox Text
    if let Ok(entity) = rank_filter_entities.get_single() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            text.sections[0].value = if spellbook_state.rank_filter {
                "[X] Rank Filter".to_string()
            } else {
                "[ ] Rank Filter".to_string()
            };
        }
    }

    // 3. Auto UpRank Checkbox Text
    if let Ok(entity) = auto_uprank_entities.get_single() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            text.sections[0].value = if spellbook_state.auto_uprank {
                "[X] Auto UpRank".to_string()
            } else {
                "[ ] Auto UpRank".to_string()
            };
        }
    }

    // 4. Tab Buttons Styling
    for (entity, tab_marker) in tab_button_entities.iter() {
        if let Ok((mut bg, mut border)) = color_q.get_mut(entity) {
            if tab_marker.0 == spellbook_state.category_filter {
                *bg = Color::srgb(0.48, 0.35, 0.22).into();
                *border = Color::srgb(0.92, 0.78, 0.42).into();
            } else {
                *bg = Color::srgb(0.24, 0.17, 0.12).into();
                *border = Color::srgb(0.40, 0.30, 0.20).into();
            }
        }
    }

    // 5. Slot Cards Styling
    for (entity, slot_card) in card_entities.iter() {
        if let Ok((mut bg, mut border)) = color_q.get_mut(entity) {
            let slot_idx = slot_card.0;
            let spell_idx = current_page * 12 + slot_idx;
            if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                let is_selected = spellbook_state.selected_spell_for_slotting.as_deref() == Some(def.id);
                if is_selected {
                    *bg = Color::srgba(0.95, 0.88, 0.65, 0.85).into();
                    *border = Color::srgb(0.85, 0.60, 0.15).into();
                } else {
                    *bg = Color::srgba(0.84, 0.77, 0.64, 0.45).into();
                    *border = Color::srgb(0.70, 0.62, 0.48).into();
                }
            } else {
                *bg = Color::srgba(0.82, 0.75, 0.62, 0.15).into();
                *border = Color::srgb(0.76, 0.70, 0.60).into();
            }
        }
    }

    // 6. Slot Icon Boxes Styling
    for (entity, icon_box) in icon_box_entities.iter() {
        if let Ok((mut bg, mut border)) = color_q.get_mut(entity) {
            let slot_idx = icon_box.0;
            let spell_idx = current_page * 12 + slot_idx;
            if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                *bg = Color::srgb(0.14, 0.11, 0.08).into();
                *border = def.color.into();
            } else {
                *bg = Color::srgb(0.25, 0.20, 0.15).into();
                *border = Color::srgb(0.42, 0.35, 0.28).into();
            }
        }
    }

    // 7. Icon Text
    for (entity, icon_marker) in icon_text_entities.iter() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let slot_idx = icon_marker.0;
            let spell_idx = current_page * 12 + slot_idx;
            if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                text.sections[0].value = def.icon.to_string();
                text.sections[0].style.color = def.color;
            } else {
                text.sections[0].value = "".to_string();
            }
        }
    }

    // 8. Name Text
    for (entity, name_marker) in name_text_entities.iter() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let slot_idx = name_marker.0;
            let spell_idx = current_page * 12 + slot_idx;
            if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                text.sections[0].value = def.name.to_string();
                text.sections[0].style.color = Color::srgb(0.18, 0.10, 0.05);
            } else {
                text.sections[0].value = "(Empty Slot)".to_string();
                text.sections[0].style.color = Color::srgb(0.55, 0.48, 0.40);
            }
        }
    }

    // 9. Rank Text
    for (entity, rank_marker) in rank_text_entities.iter() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let slot_idx = rank_marker.0;
            let spell_idx = current_page * 12 + slot_idx;
            if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                text.sections[0].value = def.rank.to_string();
                text.sections[0].style.color = Color::srgb(0.48, 0.38, 0.26);
            } else {
                text.sections[0].value = "".to_string();
            }
        }
    }

    // 10. Cooldown Text
    for (entity, cd_marker) in cd_text_entities.iter() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let slot_idx = cd_marker.0;
            let spell_idx = current_page * 12 + slot_idx;
            if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                text.sections[0].value = format!("{:.1}s CD", def.cooldown_seconds);
                text.sections[0].style.color = Color::srgb(0.55, 0.40, 0.25);
            } else {
                text.sections[0].value = "".to_string();
            }
        }
    }
}

pub fn handle_spellbook_interactions(
    mut spellbook_state: ResMut<SpellbookWindowState>,
    mut status_text_q: Query<&mut Text, With<SpellbookStatusText>>,
    mut spellbook_q: Query<&mut Style, With<SpellbookModalRoot>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
    tab_clicks: Query<(&Interaction, &SpellbookTabButton), Changed<Interaction>>,
    close_clicks: Query<&Interaction, (Changed<Interaction>, With<SpellbookCloseBtn>)>,
    prev_clicks: Query<&Interaction, (Changed<Interaction>, With<SpellbookPrevPageBtn>)>,
    next_clicks: Query<&Interaction, (Changed<Interaction>, With<SpellbookNextPageBtn>)>,
    rank_toggle_clicks: Query<&Interaction, (Changed<Interaction>, With<SpellbookRankFilterToggle>)>,
    auto_uprank_clicks: Query<&Interaction, (Changed<Interaction>, With<SpellbookAutoUpRankToggle>)>,
    slot_clicks: Query<(&Interaction, &SpellbookSlotCard), Changed<Interaction>>,
) {
    if !spellbook_state.is_open {
        return;
    }

    // Close button
    for interaction in close_clicks.iter() {
        if *interaction == Interaction::Pressed {
            spellbook_state.is_open = false;
            if let Ok(mut style) = spellbook_q.get_single_mut() {
                style.display = Display::None;
            }
            if let Ok(mut window) = window_q.get_single_mut() {
                if *camera_mode.get() == CameraMode::FPS {
                    window.cursor.grab_mode = CursorGrabMode::Locked;
                    window.cursor.visible = false;
                }
            }
            return;
        }
    }

    // Category Tabs
    for (interaction, tab_btn) in tab_clicks.iter() {
        if *interaction == Interaction::Pressed {
            spellbook_state.category_filter = tab_btn.0;
            spellbook_state.current_page = 0;
        }
    }

    let filtered = get_filtered_spells(spellbook_state.category_filter, spellbook_state.rank_filter);
    let total_pages = ((filtered.len() + 11) / 12).max(1);

    // Prev Page Button
    for interaction in prev_clicks.iter() {
        if *interaction == Interaction::Pressed {
            if spellbook_state.current_page > 0 {
                spellbook_state.current_page -= 1;
            }
        }
    }

    // Next Page Button
    for interaction in next_clicks.iter() {
        if *interaction == Interaction::Pressed {
            if spellbook_state.current_page + 1 < total_pages {
                spellbook_state.current_page += 1;
            }
        }
    }

    // Rank Filter Toggle
    for interaction in rank_toggle_clicks.iter() {
        if *interaction == Interaction::Pressed {
            spellbook_state.rank_filter = !spellbook_state.rank_filter;
            spellbook_state.current_page = 0;
        }
    }

    // Auto UpRank Toggle
    for interaction in auto_uprank_clicks.iter() {
        if *interaction == Interaction::Pressed {
            spellbook_state.auto_uprank = !spellbook_state.auto_uprank;
        }
    }

    // Spell Slot Selection
    for (interaction, slot_btn) in slot_clicks.iter() {
        if *interaction == Interaction::Pressed {
            let spell_idx = spellbook_state.current_page * 12 + slot_btn.0;
            if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                spellbook_state.selected_spell_for_slotting = Some(def.id.to_string());
                if let Ok(mut text) = status_text_q.get_single_mut() {
                    text.sections[0].value = format!(
                        "SELECTED: '{}' ({}) -> Click Hotbar Slot (1-16) to prepare!",
                        def.name, def.rank
                    );
                    text.sections[0].style.color = Color::srgb(0.65, 0.22, 0.08);
                }
            }
        }
    }
}

fn ui_node_screen_rect(transform: &GlobalTransform, node: &Node, _window: &Window) -> Rect {
    node.logical_rect(transform)
}

pub fn handle_spell_drag_and_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    card_query: Query<(&SpellbookSlotCard, &GlobalTransform, &Node, Option<&Interaction>)>,
    hotbar_slot_q: Query<(&HotbarSlotButton, &GlobalTransform, &Node, Option<&Interaction>)>,
    mut spell_drag: ResMut<SpellDragState>,
    mut hotbar: ResMut<PreparedHotbarState>,
    mut spellbook_state: ResMut<SpellbookWindowState>,
) {
    let Ok(window) = window_query.get_single() else { return; };
    let Some(cursor_pos) = window.cursor_position() else { return; };

    let filtered = get_filtered_spells(spellbook_state.category_filter, spellbook_state.rank_filter);

    // Right-Click on Hotbar Slot: Clear / Unprepare slot!
    if mouse.just_pressed(MouseButton::Right) {
        for (slot_btn, transform, node, interaction) in hotbar_slot_q.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            let is_hit = rect.inflate(4.0).contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
            if is_hit {
                if hotbar.slots[slot_btn.0].is_some() {
                    info!("Unprepared spell from Hotbar Slot {}", slot_btn.0 + 1);
                    hotbar.slots[slot_btn.0] = None;
                    hotbar.cooldowns[slot_btn.0] = 0.0;
                }
                return;
            }
        }
    }

    // Left-Click: Begin Drag from Spellbook Card OR Hotbar Slot
    // (Click-to-slot of a selected grimoire spell is owned by handle_hotbar_casting_system.)
    if mouse.just_pressed(MouseButton::Left) {
        // Check spellbook cards if spellbook is open
        if spellbook_state.is_open {
            for (card, transform, node, interaction) in card_query.iter() {
                let rect = ui_node_screen_rect(transform, node, window);
                let is_hit = rect.inflate(4.0).contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
                if is_hit {
                    let spell_idx = spellbook_state.current_page * 12 + card.0;
                    if spell_idx < filtered.len() {
                        let def = filtered[spell_idx];
                        spell_drag.is_dragging = true;
                        spell_drag.source_slot = None;
                        spell_drag.spell_id = Some(def.id.to_string());
                        spell_drag.current_pos = cursor_pos;
                        spellbook_state.selected_spell_for_slotting = Some(def.id.to_string());
                        return;
                    }
                }
            }
        }

        // Check hotbar slots to drag/reorder prepared spells
        for (slot_btn, transform, node, interaction) in hotbar_slot_q.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            let is_hit = rect.inflate(4.0).contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
            if is_hit {
                if let Some(spell_id) = &hotbar.slots[slot_btn.0] {
                    spell_drag.is_dragging = true;
                    spell_drag.source_slot = Some(slot_btn.0);
                    spell_drag.spell_id = Some(spell_id.clone());
                    spell_drag.current_pos = cursor_pos;
                    return;
                }
            }
        }
    }

    if mouse.pressed(MouseButton::Left) && spell_drag.is_dragging {
        spell_drag.current_pos = cursor_pos;
    }

    if mouse.just_released(MouseButton::Left) && spell_drag.is_dragging {
        let mut target_slot = None;
        for (slot_btn, transform, node, interaction) in hotbar_slot_q.iter() {
            let rect = ui_node_screen_rect(transform, node, window);
            let is_hit = rect.inflate(4.0).contains(cursor_pos) || interaction.map_or(false, |i| *i != Interaction::None);
            if is_hit {
                target_slot = Some(slot_btn.0);
                break;
            }
        }

        if let Some(target) = target_slot {
            if let Some(source) = spell_drag.source_slot {
                if source != target {
                    // Swap hotbar slots
                    let temp = hotbar.slots[target].take();
                    hotbar.slots[target] = spell_drag.spell_id.take();
                    hotbar.slots[source] = temp;
                    let temp_cd = hotbar.cooldowns[target];
                    hotbar.cooldowns[target] = hotbar.cooldowns[source];
                    hotbar.cooldowns[source] = temp_cd;
                    info!("Swapped Hotbar Slot {} with Slot {}", source + 1, target + 1);
                }
            } else if let Some(spell_id) = spell_drag.spell_id.take() {
                // Dragged from spellbook into hotbar slot
                info!("Prepared spell '{}' into Hotbar Slot {}", spell_id, target + 1);
                hotbar.slots[target] = Some(spell_id);
                hotbar.cooldowns[target] = 0.0;
                spellbook_state.selected_spell_for_slotting = None;
            }
        } else if let Some(source) = spell_drag.source_slot {
            // Dragged hotbar spell off into empty space: clear the slot
            info!("Dragged spell off hotbar: Cleared Hotbar Slot {}", source + 1);
            hotbar.slots[source] = None;
            hotbar.cooldowns[source] = 0.0;
        }

        spell_drag.is_dragging = false;
        spell_drag.source_slot = None;
        spell_drag.spell_id = None;
    }
}

pub fn update_spell_drag_ghost_ui(
    spell_drag: Res<SpellDragState>,
    mut ghost_query: Query<&mut Style, With<SpellDragGhostUi>>,
    mut text_query: Query<&mut Text, With<SpellDragGhostText>>,
) {
    let Ok(mut style) = ghost_query.get_single_mut() else { return; };
    let Ok(mut text) = text_query.get_single_mut() else { return; };

    if spell_drag.is_dragging {
        style.display = Display::Flex;
        style.left = Val::Px(spell_drag.current_pos.x - 25.0);
        style.top = Val::Px(spell_drag.current_pos.y - 25.0);
        let label = if let Some(spell_id) = &spell_drag.spell_id {
            if let Some(def) = get_spell_by_id(spell_id) {
                def.name
            } else {
                spell_id.as_str()
            }
        } else {
            "Spell"
        };
        text.sections[0].value = label.to_string();
    } else {
        style.display = Display::None;
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
        Query<(&OptionsTabContent, &mut Style), (Without<OptionsPanelModalRoot>, Without<PrimaryWindow>)>,
        Query<(&OptionsTabButton, &mut BackgroundColor, &mut BorderColor)>,
        Query<(&OptionsTabButtonText, &mut Text), (Without<OptionsReticleStatusText>, Without<OptionsPerfMetricsText>, Without<OptionsKeybindRebindText>)>,
        Query<(&OptionsKeybindRebindText, &mut Text), (Without<OptionsReticleStatusText>, Without<OptionsPerfMetricsText>, Without<OptionsTabButtonText>)>,
    ),
    buttons: (
        Query<&Interaction, (Changed<Interaction>, With<OptionsReticleToggleBtn>)>,
        Query<(&Interaction, &OptionsRangePresetButton), Changed<Interaction>>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsSpawnWholeZoneBtn>)>,
        Query<(&Interaction, &OptionsKeybindRebindBtn), Changed<Interaction>>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsKeybindResetBtn>)>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsCloseBtn>)>,
    ),
    wow_buttons: (
        Query<(&Interaction, &OptionsTabButton), Changed<Interaction>>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsOkayBtn>)>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsCancelBtn>)>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsRecommendedBtn>)>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsClassicBtn>)>,
        Query<&Interaction, (Changed<Interaction>, With<OptionsApplyBtn>)>,
    ),
) {
    let (mut options_q, mut window_q, mut reticle_status_q, mut perf_text_q, mut tab_content_q, mut tab_btn_style_q, mut tab_btn_text_q, mut rebind_text_q) = ui_queries;
    let (reticle_btn_q, range_btn_q, spawn_zone_q, rebind_btn_q, reset_btn_q, close_btn_q) = buttons;
    let (tab_clicks_q, okay_btn_q, cancel_btn_q, recommended_btn_q, classic_btn_q, apply_btn_q) = wow_buttons;
    if !options_state.is_open {
        return;
    }

    // 1. Tab Switching
    for (interaction, tab_btn) in tab_clicks_q.iter() {
        if *interaction == Interaction::Pressed {
            options_state.active_tab = tab_btn.0;
            for (content, mut style) in tab_content_q.iter_mut() {
                style.display = if content.0 == options_state.active_tab {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            for (btn, mut bg, mut bc) in tab_btn_style_q.iter_mut() {
                if btn.0 == options_state.active_tab {
                    *bg = Color::srgba(0.85, 0.70, 0.15, 0.45).into();
                    *bc = Color::srgb(1.0, 0.85, 0.25).into();
                } else {
                    *bg = Color::NONE.into();
                    *bc = Color::NONE.into();
                }
            }
            for (text_marker, mut text) in tab_btn_text_q.iter_mut() {
                if text_marker.0 == options_state.active_tab {
                    text.sections[0].style.color = Color::srgb(1.0, 0.90, 0.35);
                } else {
                    text.sections[0].style.color = Color::srgb(0.80, 0.80, 0.80);
                }
            }
        }
    }

    // 2. Close / Okay / Cancel buttons
    let mut should_close = false;
    for interaction in close_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            should_close = true;
        }
    }
    for interaction in okay_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            should_close = true;
        }
    }
    for interaction in cancel_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            should_close = true;
        }
    }
    if should_close {
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

    // 3. Recommended & Classic presets buttons
    for interaction in recommended_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            render_settings.view_distance_chunks = 16;
            render_settings.unload_distance_chunks = 17;
            render_settings.spawn_full_zone = false;
            render_settings.visible_range_meters = 256.0;
            crosshair_settings.enabled = true;
            if let Ok(mut text) = reticle_status_q.get_single_mut() {
                text.sections[0].value = "RETICLE: ON".into();
                text.sections[0].style.color = Color::srgb(0.2, 1.0, 0.4);
            }
            info!("Applied Recommended Graphics profile: 256m (16 chunks), Reticle ON.");
        }
    }

    for interaction in classic_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            render_settings.view_distance_chunks = 10;
            render_settings.unload_distance_chunks = 11;
            render_settings.spawn_full_zone = false;
            render_settings.visible_range_meters = 160.0;
            crosshair_settings.enabled = true;
            if let Ok(mut text) = reticle_status_q.get_single_mut() {
                text.sections[0].value = "RETICLE: ON".into();
                text.sections[0].style.color = Color::srgb(0.2, 1.0, 0.4);
            }
            info!("Applied Classic Graphics profile: 160m (10 chunks), Reticle ON.");
        }
    }

    for interaction in apply_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            info!("Settings applied.");
        }
    }

    // 4. Reticle ON/OFF Toggle
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

    // 5. Visible range preset buttons
    for (interaction, preset_btn) in range_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            render_settings.view_distance_chunks = preset_btn.0;
            render_settings.unload_distance_chunks = preset_btn.0 + 1;
            render_settings.spawn_full_zone = false;
            render_settings.visible_range_meters = preset_btn.0 as f32 * 16.0;
            info!("Visible Range adjusted to {} chunks ({}m)", preset_btn.0, render_settings.visible_range_meters);
        }
    }

    // 6. Spawn entire zone button
    for interaction in spawn_zone_q.iter() {
        if *interaction == Interaction::Pressed {
            render_settings.view_distance_chunks = 64;
            render_settings.unload_distance_chunks = 70;
            render_settings.spawn_full_zone = true;
            render_settings.visible_range_meters = 1024.0;
            info!("⚡ Spawn Entire Zone requested: Full 1024m radius active!");
        }
    }

    // 7. Hotbar keybind rebinding
    for (interaction, rebind_btn) in rebind_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            options_state.rebinding_slot = Some(rebind_btn.0);
            for (marker, mut text) in rebind_text_q.iter_mut() {
                if marker.0 == rebind_btn.0 {
                    text.sections[0].value = format!("S{}: [...]", marker.0 + 1);
                    text.sections[0].style.color = Color::srgb(1.0, 0.85, 0.2);
                }
            }
            info!("Listening for new keybind for Hotbar Slot {}", rebind_btn.0 + 1);
        }
    }

    if let Some(slot) = options_state.rebinding_slot {
        for key in keys.get_just_pressed() {
            if *key != KeyCode::Escape && *key != KeyCode::KeyO {
                keybinds.keybinds[slot] = *key;
                info!("Rebound Hotbar Slot {} to {:?}", slot + 1, key);
                for (marker, mut text) in rebind_text_q.iter_mut() {
                    if marker.0 == slot {
                        text.sections[0].value = format!("S{}: [{}]", slot + 1, keycode_display_name(*key));
                        text.sections[0].style.color = Color::WHITE;
                    }
                }
                options_state.rebinding_slot = None;
                break;
            }
        }
    }

    for interaction in reset_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            *keybinds = HotbarKeybinds::default();
            for (marker, mut text) in rebind_text_q.iter_mut() {
                if marker.0 < 16 {
                    let k = keybinds.keybinds[marker.0];
                    text.sections[0].value = format!("S{}: [{}]", marker.0 + 1, keycode_display_name(k));
                    text.sections[0].style.color = Color::WHITE;
                }
            }
            info!("Reset all 16 hotbar keybinds to defaults.");
        }
    }

    // 8. Update live performance metrics text
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

    let end_dist = render_settings.visible_range_meters;
    let start_dist = (end_dist * 0.20).max(40.0);

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
            .init_resource::<SpellDragState>()
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
// 7. UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spell_catalog_integrity() {
        assert!(SPELL_CATALOG.len() >= 16, "Spell catalog must contain at least 16 spells");
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
        assert_eq!(hotbar.slots[0].as_deref(), None);
        assert_eq!(hotbar.slots[4].as_deref(), None);

        // Slot custom spell
        hotbar.slots[0] = Some("phase_dash".into());
        hotbar.slots[15] = Some("solar_flare".into());
        assert_eq!(hotbar.slots[0].as_deref(), Some("phase_dash"));
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
        spellbook_state.selected_spell_for_slotting = Some("fireball".to_string());
        assert_eq!(spellbook_state.selected_spell_for_slotting.as_deref(), Some("fireball"));

        // Simulate click on slot 0: slots selected spell and clears selection
        let selected = spellbook_state.selected_spell_for_slotting.take().unwrap();
        hotbar.slots[0] = Some(selected);
        hotbar.cooldowns[0] = 0.0;
        assert_eq!(hotbar.slots[0].as_deref(), Some("fireball"));
        assert_eq!(spellbook_state.selected_spell_for_slotting, None);

        // 3. Drag spell from spellbook to slot 1
        spell_drag.is_dragging = true;
        spell_drag.spell_id = Some("frost_nova".to_string());
        let dragged = spell_drag.spell_id.take().unwrap();
        hotbar.slots[1] = Some(dragged);
        spell_drag.is_dragging = false;
        assert_eq!(hotbar.slots[1].as_deref(), Some("frost_nova"));

        // 4. Swap slots 0 and 1
        let temp = hotbar.slots[1].take();
        hotbar.slots[1] = hotbar.slots[0].take();
        hotbar.slots[0] = temp;
        assert_eq!(hotbar.slots[0].as_deref(), Some("frost_nova"));
        assert_eq!(hotbar.slots[1].as_deref(), Some("fireball"));
    }
}
