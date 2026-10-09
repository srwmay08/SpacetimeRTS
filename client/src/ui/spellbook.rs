// ============================================================================
// File: client/src/ui/spellbook.rs
// ============================================================================
// ----------------------------------------------------------------------------
// SPELLBOOK & ABILITIES GRIMOIRE MODAL UI & INTERACTION SYSTEMS
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use crate::core::*;
use crate::spells::*;

#[derive(Resource)]
pub struct SpellbookWindowState {
    pub is_open: bool,
    pub selected_spell_for_slotting: Option<SpellId>,
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

// ----------------------------------------------------------------------------
// SPELLBOOK UI MARKER COMPONENTS
// ----------------------------------------------------------------------------

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
    if !spellbook_state.is_open || !spellbook_state.is_changed() {
        return;
    }

    let filtered = get_filtered_spells(spellbook_state.category_filter, spellbook_state.rank_filter);
    let total_pages = ((filtered.len() + 11) / 12).max(1);
    let current_page = spellbook_state.current_page.min(total_pages - 1);

    // 1. Page Indicator Text
    if let Ok(entity) = page_indicator_entities.get_single() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let new_val = format!("Page {} of {}", current_page + 1, total_pages);
            if text.sections[0].value != new_val {
                text.sections[0].value = new_val;
            }
        }
    }

    // 2. Rank Filter Checkbox Text
    if let Ok(entity) = rank_filter_entities.get_single() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let new_val = if spellbook_state.rank_filter {
                "[X] Rank Filter"
            } else {
                "[ ] Rank Filter"
            };
            if text.sections[0].value != new_val {
                text.sections[0].value = new_val.to_string();
            }
        }
    }

    // 3. Auto UpRank Checkbox Text
    if let Ok(entity) = auto_uprank_entities.get_single() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let new_val = if spellbook_state.auto_uprank {
                "[X] Auto UpRank"
            } else {
                "[ ] Auto UpRank"
            };
            if text.sections[0].value != new_val {
                text.sections[0].value = new_val.to_string();
            }
        }
    }

    // 4. Tab Buttons Styling
    for (entity, tab_marker) in tab_button_entities.iter() {
        if let Ok((mut bg, mut border)) = color_q.get_mut(entity) {
            let (new_bg, new_border): (BackgroundColor, BorderColor) = if tab_marker.0 == spellbook_state.category_filter {
                (Color::srgb(0.48, 0.35, 0.22).into(), Color::srgb(0.92, 0.78, 0.42).into())
            } else {
                (Color::srgb(0.24, 0.17, 0.12).into(), Color::srgb(0.40, 0.30, 0.20).into())
            };
            if *bg != new_bg { *bg = new_bg; }
            if *border != new_border { *border = new_border; }
        }
    }

    // 5. Slot Cards Styling
    for (entity, slot_card) in card_entities.iter() {
        if let Ok((mut bg, mut border)) = color_q.get_mut(entity) {
            let slot_idx = slot_card.0;
            let spell_idx = current_page * 12 + slot_idx;
            let (new_bg, new_border): (BackgroundColor, BorderColor) = if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                let is_selected = spellbook_state.selected_spell_for_slotting == Some(def.spell_id);
                if is_selected {
                    (Color::srgba(0.95, 0.88, 0.65, 0.85).into(), Color::srgb(0.85, 0.60, 0.15).into())
                } else {
                    (Color::srgba(0.84, 0.77, 0.64, 0.45).into(), Color::srgb(0.70, 0.62, 0.48).into())
                }
            } else {
                (Color::srgba(0.82, 0.75, 0.62, 0.15).into(), Color::srgb(0.76, 0.70, 0.60).into())
            };
            if *bg != new_bg { *bg = new_bg; }
            if *border != new_border { *border = new_border; }
        }
    }

    // 6. Slot Icon Boxes Styling
    for (entity, icon_box) in icon_box_entities.iter() {
        if let Ok((mut bg, mut border)) = color_q.get_mut(entity) {
            let slot_idx = icon_box.0;
            let spell_idx = current_page * 12 + slot_idx;
            let (new_bg, new_border): (BackgroundColor, BorderColor) = if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                (Color::srgb(0.14, 0.11, 0.08).into(), def.color.into())
            } else {
                (Color::srgb(0.25, 0.20, 0.15).into(), Color::srgb(0.42, 0.35, 0.28).into())
            };
            if *bg != new_bg { *bg = new_bg; }
            if *border != new_border { *border = new_border; }
        }
    }

    // 7. Icon Text
    for (entity, icon_marker) in icon_text_entities.iter() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let slot_idx = icon_marker.0;
            let spell_idx = current_page * 12 + slot_idx;
            let (new_icon, new_color) = if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                (def.icon, def.color)
            } else {
                ("", Color::NONE)
            };
            if text.sections[0].value != new_icon {
                text.sections[0].value = new_icon.to_string();
            }
            if text.sections[0].style.color != new_color {
                text.sections[0].style.color = new_color;
            }
        }
    }

    // 8. Name Text
    for (entity, name_marker) in name_text_entities.iter() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let slot_idx = name_marker.0;
            let spell_idx = current_page * 12 + slot_idx;
            let (new_name, new_color) = if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                (def.name, Color::srgb(0.18, 0.10, 0.05))
            } else {
                ("(Empty Slot)", Color::srgb(0.55, 0.48, 0.40))
            };
            if text.sections[0].value != new_name {
                text.sections[0].value = new_name.to_string();
            }
            if text.sections[0].style.color != new_color {
                text.sections[0].style.color = new_color;
            }
        }
    }

    // 9. Rank Text
    for (entity, rank_marker) in rank_text_entities.iter() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let slot_idx = rank_marker.0;
            let spell_idx = current_page * 12 + slot_idx;
            let (new_rank, new_color) = if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                (def.rank, Color::srgb(0.48, 0.38, 0.26))
            } else {
                ("", Color::NONE)
            };
            if text.sections[0].value != new_rank {
                text.sections[0].value = new_rank.to_string();
            }
            if text.sections[0].style.color != new_color {
                text.sections[0].style.color = new_color;
            }
        }
    }

    // 10. Cooldown Text
    for (entity, cd_marker) in cd_text_entities.iter() {
        if let Ok(mut text) = text_q.get_mut(entity) {
            let slot_idx = cd_marker.0;
            let spell_idx = current_page * 12 + slot_idx;
            let (new_cd, new_color) = if spell_idx < filtered.len() {
                let def = filtered[spell_idx];
                (format!("{:.1}s CD", def.cooldown_seconds), Color::srgb(0.55, 0.40, 0.25))
            } else {
                (String::new(), Color::NONE)
            };
            if text.sections[0].value != new_cd {
                text.sections[0].value = new_cd;
            }
            if text.sections[0].style.color != new_color {
                text.sections[0].style.color = new_color;
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
                spellbook_state.selected_spell_for_slotting = Some(def.spell_id);
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
