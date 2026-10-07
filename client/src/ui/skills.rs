// ============================================================================
// File: client/src/skills_ui.rs
// ============================================================================
// ----------------------------------------------------------------------------
// VALHEIM-STYLE SKILLS SHEET UI
// ----------------------------------------------------------------------------
// Architectural Note:
// Renders the Norse/Viking character skills interface inspired by Valheim.
// Displays classless weapon proficiencies, physical attributes, and utility skills
// with progress bars, exact level numbers, and real-time SpacetimeDB synchronization.
// Toggleable with [L] key or through the UI.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::components::*;
use crate::core::*;
use crate::network::SpacetimeConnection;
use crate::module_bindings::weapon_skill_table::WeaponSkillTableAccess;

// ----------------------------------------------------------------------------
// RESOURCE & STATE
// ----------------------------------------------------------------------------

#[derive(Resource)]
pub struct SkillsSheetState {
    pub is_open: bool,
}

impl Default for SkillsSheetState {
    fn default() -> Self {
        Self { is_open: false }
    }
}

// ----------------------------------------------------------------------------
// ECS MARKER COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct SkillsSheetModalRoot;

#[derive(Component)]
pub struct SkillsSheetCloseBtn;

#[derive(Component)]
pub struct SkillRowFill(pub &'static str);

#[derive(Component)]
pub struct SkillRowLevelText(pub &'static str);

// ----------------------------------------------------------------------------
// SKILL DEFINITIONS (MATCHING VALHEIM REFERENCE)
// ----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SkillEntryDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub default_level: u32,
    pub default_progress: f32, // 0.0 to 1.0
}

pub const SKILL_ENTRIES: &[SkillEntryDef] = &[
    SkillEntryDef { id: "unarmed", name: "Unarmed", icon: "✊", default_level: 20, default_progress: 0.65 },
    SkillEntryDef { id: "swim", name: "Swim", icon: "〰", default_level: 6, default_progress: 0.25 },
    SkillEntryDef { id: "jump", name: "Jump", icon: "🥾", default_level: 28, default_progress: 0.80 },
    SkillEntryDef { id: "run", name: "Run", icon: "🏃", default_level: 36, default_progress: 0.90 },
    SkillEntryDef { id: "sneak", name: "Sneak", icon: "👁", default_level: 19, default_progress: 0.45 },
    SkillEntryDef { id: "clubs", name: "Clubs", icon: "🏏", default_level: 19, default_progress: 0.50 },
    SkillEntryDef { id: "blocking", name: "Blocking", icon: "🛡", default_level: 7, default_progress: 0.30 },
    SkillEntryDef { id: "axes", name: "Axes", icon: "🪓", default_level: 16, default_progress: 0.40 },
    SkillEntryDef { id: "woodcutting", name: "Wood cutting", icon: "🌲", default_level: 44, default_progress: 0.95 },
    SkillEntryDef { id: "spears", name: "Spears", icon: "🗡", default_level: 30, default_progress: 0.70 },
    SkillEntryDef { id: "bows", name: "Bows", icon: "🏹", default_level: 22, default_progress: 0.55 },
    SkillEntryDef { id: "knives", name: "Knives", icon: "🔪", default_level: 0, default_progress: 0.05 },
    SkillEntryDef { id: "pickaxes", name: "Pickaxes", icon: "⛏", default_level: 35, default_progress: 0.82 },
    SkillEntryDef { id: "swords", name: "Swords", icon: "⚔", default_level: 0, default_progress: 0.08 },
    SkillEntryDef { id: "firearms", name: "Firearms", icon: "🔫", default_level: 25, default_progress: 0.60 },
    SkillEntryDef { id: "polearms", name: "Polearms", icon: "🔱", default_level: 21, default_progress: 0.52 },
    SkillEntryDef { id: "magic", name: "Runestaff", icon: "🔮", default_level: 18, default_progress: 0.48 },
];

// ----------------------------------------------------------------------------
// SETUP UI SYSTEM
// ----------------------------------------------------------------------------

pub fn setup_skills_sheet_ui(mut commands: Commands) {
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
            z_index: ZIndex::Global(125),
            ..default()
        },
        SkillsSheetModalRoot,
    )).with_children(|overlay| {
        // Valheim Dark Stone / Slate Panel
        overlay.spawn(NodeBundle {
            style: Style {
                width: Val::Px(450.0),
                height: Val::Px(640.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(2.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            border_color: BorderColor(Color::srgb(0.32, 0.28, 0.22)), // Dark Norse bronze/stone
            background_color: BackgroundColor(Color::srgba(0.09, 0.10, 0.12, 0.97)),
            ..default()
        }).with_children(|panel| {
            // Norse Title: ‹SKILLS›
            panel.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
                ..default()
            }).with_children(|header| {
                header.spawn(TextBundle::from_section(
                    "‹SKILLS›",
                    TextStyle {
                        font_size: 20.0,
                        color: Color::srgb(0.96, 0.72, 0.24), // Amber Norse Gold
                        ..default()
                    },
                ));
            });

            // Scrollable / Structured Skill Rows Container
            panel.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    overflow: Overflow::clip_y(),
                    ..default()
                },
                ..default()
            }).with_children(|list| {
                for skill in SKILL_ENTRIES {
                    list.spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Px(26.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgba(0.14, 0.15, 0.18, 0.8)),
                        border_color: BorderColor(Color::srgb(0.20, 0.22, 0.25)),
                        ..default()
                    }).with_children(|row| {
                        // Left: Icon + Skill Name
                        row.spawn(NodeBundle {
                            style: Style {
                                width: Val::Px(140.0),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(6.0),
                                ..default()
                            },
                            ..default()
                        }).with_children(|label_box| {
                            label_box.spawn(TextBundle::from_section(
                                skill.icon,
                                TextStyle { font_size: 13.0, color: Color::srgb(0.9, 0.8, 0.6), ..default() },
                            ));
                            label_box.spawn(TextBundle::from_section(
                                skill.name,
                                TextStyle {
                                    font_size: 13.0,
                                    color: Color::srgb(0.96, 0.74, 0.26), // Valheim yellow text
                                    ..default()
                                },
                            ));
                        });

                        // Center: Horizontal Progress Bar
                        row.spawn(NodeBundle {
                            style: Style {
                                width: Val::Px(180.0),
                                height: Val::Px(12.0),
                                border: UiRect::all(Val::Px(1.0)),
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            background_color: BackgroundColor(Color::srgb(0.08, 0.08, 0.10)),
                            border_color: BorderColor(Color::srgb(0.24, 0.26, 0.30)),
                            ..default()
                        }).with_children(|bar_track| {
                            bar_track.spawn((
                                NodeBundle {
                                    style: Style {
                                        width: Val::Percent(skill.default_progress * 100.0),
                                        height: Val::Percent(100.0),
                                        ..default()
                                    },
                                    background_color: BackgroundColor(Color::srgb(0.95, 0.76, 0.10)), // Valheim solid yellow
                                    ..default()
                                },
                                SkillRowFill(skill.id),
                            ));
                        });

                        // Right: Level Number
                        row.spawn((
                            TextBundle::from_section(
                                skill.default_level.to_string(),
                                TextStyle {
                                    font_size: 13.0,
                                    color: Color::WHITE,
                                    ..default()
                                },
                            ).with_style(Style {
                                width: Val::Px(36.0),
                                justify_content: JustifyContent::FlexEnd,
                                ..default()
                            }).with_text_justify(JustifyText::Right),
                            SkillRowLevelText(skill.id),
                        ));
                    });
                }
            });

            // Bottom Close Button (Valheim Norse Button Style)
            panel.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
                ..default()
            }).with_children(|btn_row| {
                btn_row.spawn((
                    ButtonBundle {
                        style: Style {
                            padding: UiRect::axes(Val::Px(36.0), Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        border_color: BorderColor(Color::srgb(0.65, 0.40, 0.18)), // Amber/wood border
                        background_color: BackgroundColor(Color::srgb(0.22, 0.18, 0.14)), // Dark Norse wood
                        ..default()
                    },
                    SkillsSheetCloseBtn,
                )).with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        "Close",
                        TextStyle {
                            font_size: 14.0,
                            color: Color::srgb(0.95, 0.85, 0.65),
                            ..default()
                        },
                    ));
                });
            });
        });
    });
}

// ----------------------------------------------------------------------------
// TOGGLE & INTERACTION SYSTEMS
// ----------------------------------------------------------------------------

pub fn toggle_skills_sheet_system(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    mut state: ResMut<SkillsSheetState>,
    mut sheet_q: Query<&mut Style, With<SkillsSheetModalRoot>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    if console.is_open {
        return;
    }

    // Toggle with [L] key
    if keys.just_pressed(KeyCode::KeyL) {
        state.is_open = !state.is_open;

        if let Ok(mut style) = sheet_q.get_single_mut() {
            style.display = if state.is_open { Display::Flex } else { Display::None };
        }

        if let Ok(mut window) = window_q.get_single_mut() {
            if state.is_open {
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
            } else if *camera_mode.get() == CameraMode::FPS {
                window.cursor.grab_mode = CursorGrabMode::Locked;
                window.cursor.visible = false;
            }
        }
    }
}

pub fn handle_skills_sheet_close_button(
    mut state: ResMut<SkillsSheetState>,
    close_btn_q: Query<&Interaction, (Changed<Interaction>, With<SkillsSheetCloseBtn>)>,
    mut sheet_q: Query<&mut Style, With<SkillsSheetModalRoot>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    for interaction in close_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            state.is_open = false;

            if let Ok(mut style) = sheet_q.get_single_mut() {
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
}

// ----------------------------------------------------------------------------
// DATA SYNCHRONIZATION WITH SPACETIMEDB WEAPON_SKILL TABLE
// ----------------------------------------------------------------------------

pub fn sync_skills_sheet_data(
    state: Res<SkillsSheetState>,
    conn: Res<SpacetimeConnection>,
    cached_player: Res<CachedPlayerEntity>,
    mut fill_q: Query<(&SkillRowFill, &mut Style)>,
    mut text_q: Query<(&SkillRowLevelText, &mut Text)>,
) {
    if !state.is_open {
        return;
    }

    let player_id = cached_player.0.unwrap_or(0);
    let skill_data = conn.db.db.weapon_skill().entity_id().find(&player_id);

    for (fill_marker, mut style) in fill_q.iter_mut() {
        let (_level, progress) = get_skill_stats(fill_marker.0, skill_data.as_ref());
        style.width = Val::Percent((progress * 100.0).clamp(5.0, 100.0));
    }

    for (text_marker, mut text) in text_q.iter_mut() {
        let (level, _) = get_skill_stats(text_marker.0, skill_data.as_ref());
        text.sections[0].value = level.to_string();
    }
}

fn get_skill_stats(skill_id: &str, skill_table: Option<&crate::module_bindings::WeaponSkill>) -> (u32, f32) {
    if let Some(skill) = skill_table {
        match skill_id {
            "unarmed" => {
                let xp = skill.brawling_xp;
                let lvl = (xp / 100).clamp(1, 100);
                let prog = (xp % 100) as f32 / 100.0;
                (lvl, prog)
            }
            "swim" => {
                let lvl = (skill.generic_physical / 4).clamp(1, 100);
                let prog = (skill.generic_physical % 4) as f32 / 4.0;
                (lvl, prog)
            }
            "jump" => {
                let lvl = (skill.generic_physical / 3).clamp(1, 100);
                let prog = (skill.generic_physical % 3) as f32 / 3.0;
                (lvl, prog)
            }
            "run" => {
                let lvl = (skill.generic_physical / 2).clamp(1, 100);
                let prog = (skill.generic_physical % 2) as f32 / 2.0;
                (lvl, prog)
            }
            "clubs" => {
                let xp = skill.blunt_xp;
                let lvl = (xp / 100).clamp(1, 100);
                (lvl, (xp % 100) as f32 / 100.0)
            }
            "axes" => {
                let xp = skill.edged_xp;
                let lvl = (xp / 100).clamp(1, 100);
                (lvl, (xp % 100) as f32 / 100.0)
            }
            "spears" | "polearms" => {
                let xp = skill.polearm_xp;
                let lvl = (xp / 100).clamp(1, 100);
                (lvl, (xp % 100) as f32 / 100.0)
            }
            "bows" => {
                let xp = skill.missile_xp;
                let lvl = (xp / 100).clamp(1, 100);
                (lvl, (xp % 100) as f32 / 100.0)
            }
            "knives" => {
                let xp = skill.pointed_xp;
                let lvl = (xp / 100).clamp(0, 100);
                (lvl, (xp % 100) as f32 / 100.0)
            }
            "swords" => {
                let xp = skill.edged_xp;
                let lvl = (xp / 100).clamp(0, 100);
                (lvl, (xp % 100) as f32 / 100.0)
            }
            "firearms" => {
                let xp = skill.firearm_xp;
                let lvl = (xp / 100).clamp(1, 100);
                (lvl, (xp % 100) as f32 / 100.0)
            }
            "magic" => {
                let xp = skill.runestaff_xp;
                let lvl = (xp / 100).clamp(1, 100);
                (lvl, (xp % 100) as f32 / 100.0)
            }
            _ => (15, 0.5),
        }
    } else {
        // Fallback default progression for offline / testing
        if let Some(def) = SKILL_ENTRIES.iter().find(|s| s.id == skill_id) {
            (def.default_level, def.default_progress)
        } else {
            (10, 0.4)
        }
    }
}

// ----------------------------------------------------------------------------
// PLUGIN
// ----------------------------------------------------------------------------

pub struct SkillsSheetPlugin;

impl Plugin for SkillsSheetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillsSheetState>()
            .add_systems(Startup, setup_skills_sheet_ui)
            .add_systems(
                Update,
                (
                    toggle_skills_sheet_system,
                    handle_skills_sheet_close_button,
                    sync_skills_sheet_data,
                ),
            );
    }
}
