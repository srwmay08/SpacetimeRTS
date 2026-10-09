// ============================================================================
// File: client/src/ui/options.rs
// ============================================================================
// ----------------------------------------------------------------------------
// CONFIGURABLE OPTIONS PANEL MODAL & TERRAIN RENDER SETTINGS
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use crate::components::CrosshairSettings;
use crate::core::*;
use super::hotbar::{HotbarKeybinds, keycode_display_name};

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
    pub bedrock_enabled: bool,
}

impl Default for TerrainRenderSettings {
    fn default() -> Self {
        Self {
            view_distance_chunks: 14, // 224m default (matches terrain LOW_POLY_RADIUS_CHUNKS: 14, 20% reduction)
            unload_distance_chunks: 15, // 240m default (matches terrain LOW_POLY_UNLOAD_RADIUS_CHUNKS: 15, immediately beyond 230.4m fog limit)
            spawn_full_zone: false,
            visible_range_meters: 230.4,
            bedrock_enabled: true,
        }
    }
}

// ----------------------------------------------------------------------------
// OPTIONS UI MARKER COMPONENTS
// ----------------------------------------------------------------------------

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
            z_index: ZIndex::Global(150),
            ..default()
        },
        OptionsPanelModalRoot,
    )).with_children(|backdrop| {
        // Modal Outer Dialog Box (EverQuest Titanium Slate Theme)
        backdrop.spawn(NodeBundle {
            style: Style {
                width: Val::Px(720.0),
                height: Val::Px(520.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            border_color: BorderColor(Color::srgb(0.35, 0.38, 0.42)),
            background_color: BackgroundColor(Color::srgba(0.10, 0.11, 0.14, 0.98)),
            ..default()
        }).with_children(|dialog| {
            // Window Titlebar
            dialog.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(32.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                    border: UiRect::bottom(Val::Px(1.5)),
                    ..default()
                },
                border_color: BorderColor(Color::srgb(0.25, 0.28, 0.32)),
                background_color: BackgroundColor(Color::srgb(0.14, 0.16, 0.20)),
                ..default()
            }).with_children(|titlebar| {
                titlebar.spawn(TextBundle::from_section(
                    "⚙ Options",
                    TextStyle { font_size: 14.0, color: Color::srgb(1.0, 0.85, 0.35), ..default() }
                ));

                // Titlebar Close (X)
                titlebar.spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Px(20.0),
                            height: Val::Px(20.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        border_color: BorderColor(Color::srgb(0.5, 0.2, 0.2)),
                        background_color: BackgroundColor(Color::srgb(0.3, 0.1, 0.1)),
                        ..default()
                    },
                    OptionsCloseBtn,
                )).with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        "✕",
                        TextStyle { font_size: 11.0, color: Color::WHITE, ..default() }
                    ));
                });
            });

            // Tabs Header Bar
            dialog.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(34.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::FlexEnd,
                    padding: UiRect::left(Val::Px(12.0)),
                    column_gap: Val::Px(4.0),
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                border_color: BorderColor(Color::srgb(0.25, 0.28, 0.32)),
                background_color: BackgroundColor(Color::srgb(0.08, 0.09, 0.12)),
                ..default()
            }).with_children(|tabs_bar| {
                let tabs = [
                    (OptionsTab::Graphics, "Display"),
                    (OptionsTab::Advanced, "General"),
                    (OptionsTab::Keybinds, "Keys"),
                    (OptionsTab::Network, "Chat"),
                ];

                for (tab_type, tab_label) in tabs {
                    let is_active = tab_type == OptionsTab::Graphics;
                    let bg = if is_active { Color::srgb(0.18, 0.20, 0.25) } else { Color::srgb(0.12, 0.13, 0.16) };
                    let border = if is_active { Color::srgb(0.85, 0.70, 0.25) } else { Color::srgb(0.28, 0.30, 0.35) };
                    let text_color = if is_active { Color::srgb(1.0, 0.85, 0.35) } else { Color::srgb(0.65, 0.65, 0.70) };

                    tabs_bar.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: BorderColor(border),
                            background_color: BackgroundColor(bg),
                            ..default()
                        },
                        OptionsTabButton(tab_type),
                    )).with_children(|btn| {
                        btn.spawn((
                            TextBundle::from_section(
                                tab_label,
                                TextStyle { font_size: 11.5, color: text_color, ..default() }
                            ),
                            OptionsTabButtonText(tab_type),
                        ));
                    });
                }
            });

            // Tab Content Frame Body
            dialog.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(410.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(12.0)),
                    overflow: Overflow::clip_y(),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgba(0.07, 0.08, 0.10, 0.95)),
                ..default()
            }).with_children(|body| {
                // ---- TAB 1: GRAPHICS ----
                body.spawn((
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
                )).with_children(|gfx| {
                    // Frame 1: Reticle Configuration Section
                    spawn_framed_section(gfx, "Reticle Configuration", |sec| {
                        sec.spawn(NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            ..default()
                        }).with_children(|row| {
                            row.spawn(TextBundle::from_section(
                                "First-Person Combat Reticle / Crosshair:",
                                TextStyle { font_size: 11.0, color: Color::srgb(0.8, 0.8, 0.8), ..default() }
                            ));

                            row.spawn(NodeBundle {
                                style: Style {
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                },
                                ..default()
                            }).with_children(|ctrl| {
                                ctrl.spawn((
                                    ButtonBundle {
                                        style: Style {
                                            padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                                            border: UiRect::all(Val::Px(1.0)),
                                            ..default()
                                        },
                                        border_color: BorderColor(Color::srgb(0.4, 0.45, 0.5)),
                                        background_color: BackgroundColor(Color::srgb(0.18, 0.22, 0.26)),
                                        ..default()
                                    },
                                    OptionsReticleToggleBtn,
                                )).with_children(|b| {
                                    b.spawn((
                                        TextBundle::from_section(
                                            "TOGGLE [ON]",
                                            TextStyle { font_size: 10.5, color: Color::srgb(0.3, 0.9, 0.4), ..default() }
                                        ),
                                        OptionsReticleStatusText,
                                    ));
                                });
                            });
                        });
                    });

                    // Frame 2: Visible Range & Chunk Streaming Section
                    spawn_framed_section(gfx, "Terrain Draw Distance & Chunks", |sec| {
                        sec.spawn(NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                ..default()
                            },
                            ..default()
                        }).with_children(|chunk_sec| {
                            chunk_sec.spawn(TextBundle::from_section(
                                "View Radius Presets (Zone Terrain Chunks):",
                                TextStyle { font_size: 11.0, color: Color::srgb(0.8, 0.8, 0.8), ..default() }
                            ));

                            chunk_sec.spawn(NodeBundle {
                                style: Style {
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(6.0),
                                    ..default()
                                },
                                ..default()
                            }).with_children(|presets| {
                                let dists = [
                                    (14, "224m (14 Chunks - Default)"),
                                    (24, "384m (24 Chunks)"),
                                    (32, "512m (32 Chunks)"),
                                    (48, "768m (48 Chunks)"),
                                ];

                                for (radius, label) in dists {
                                    presets.spawn((
                                        ButtonBundle {
                                            style: Style {
                                                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                                                border: UiRect::all(Val::Px(1.0)),
                                                ..default()
                                            },
                                            border_color: BorderColor(Color::srgb(0.3, 0.35, 0.4)),
                                            background_color: BackgroundColor(Color::srgb(0.14, 0.16, 0.20)),
                                            ..default()
                                        },
                                        OptionsRangePresetButton(radius),
                                    )).with_children(|btn| {
                                        btn.spawn(TextBundle::from_section(
                                            label,
                                            TextStyle { font_size: 10.0, color: Color::srgb(0.85, 0.85, 0.90), ..default() }
                                        ));
                                    });
                                }
                            });

                            // Spawn Whole Zone Stress Benchmark Trigger Button
                            chunk_sec.spawn(NodeBundle {
                                style: Style {
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    margin: UiRect::top(Val::Px(4.0)),
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                },
                                ..default()
                            }).with_children(|bench| {
                                bench.spawn((
                                    ButtonBundle {
                                        style: Style {
                                            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                            border: UiRect::all(Val::Px(1.0)),
                                            ..default()
                                        },
                                        border_color: BorderColor(Color::srgb(0.6, 0.25, 0.25)),
                                        background_color: BackgroundColor(Color::srgb(0.28, 0.12, 0.12)),
                                        ..default()
                                    },
                                    OptionsSpawnWholeZoneBtn,
                                )).with_children(|b| {
                                    b.spawn(TextBundle::from_section(
                                        "⚡ SPAWN WHOLE ZONE (64 Chunks / 1024m)",
                                        TextStyle { font_size: 10.5, color: Color::srgb(1.0, 0.6, 0.6), ..default() }
                                    ));
                                });

                                bench.spawn((
                                    TextBundle::from_section(
                                        "Zone Mode: Normal Streaming (14 Chunks)",
                                        TextStyle { font_size: 10.0, color: Color::srgb(0.7, 0.7, 0.7), ..default() }
                                    ),
                                    OptionsPerfMetricsText,
                                ));
                            });
                        });
                    });
                });

                // ---- TAB 2: ADVANCED ----
                body.spawn((
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
                )).with_children(|adv| {
                    spawn_framed_section(adv, "Display Settings", |sec| {
                        sec.spawn(TextBundle::from_section(
                            "• V-Sync: Triple Buffered (Metal / Vulkan)\n\
                             • Texture Quality: Native Uncompressed Voxels\n\
                             • Dynamic Shadow Cascades: Directional Star A/B + Point Lighting\n\
                             • Subterranean Depth Ambient Dimming: Active\n\
                             • Volumetric Night Sky & Aurora Borealis: Active",
                            TextStyle { font_size: 10.5, color: Color::srgb(0.75, 0.78, 0.82), ..default() }
                        ));
                    });
                });

                // ---- TAB 3: KEYBINDS ----
                body.spawn((
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
                )).with_children(|keys_tab| {
                    spawn_framed_section(keys_tab, "Prepared Hotbar Rebinding (16 Slots)", |sec| {
                        sec.spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(3.0),
                                ..default()
                            },
                            ..default()
                        }).with_children(|list| {
                            for row in 0..4 {
                                list.spawn(NodeBundle {
                                    style: Style {
                                        flex_direction: FlexDirection::Row,
                                        column_gap: Val::Px(8.0),
                                        ..default()
                                    },
                                    ..default()
                                }).with_children(|row_ui| {
                                    for col in 0..4 {
                                        let slot_idx = row * 4 + col;
                                        let key_name = keycode_display_name(keybinds.keybinds[slot_idx]);

                                        row_ui.spawn((
                                            ButtonBundle {
                                                style: Style {
                                                    width: Val::Px(160.0),
                                                    height: Val::Px(24.0),
                                                    flex_direction: FlexDirection::Row,
                                                    justify_content: JustifyContent::SpaceBetween,
                                                    align_items: AlignItems::Center,
                                                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                                                    border: UiRect::all(Val::Px(1.0)),
                                                    ..default()
                                                },
                                                border_color: BorderColor(Color::srgb(0.3, 0.35, 0.4)),
                                                background_color: BackgroundColor(Color::srgb(0.12, 0.14, 0.18)),
                                                ..default()
                                            },
                                            OptionsKeybindRebindBtn(slot_idx),
                                        )).with_children(|btn| {
                                            btn.spawn(TextBundle::from_section(
                                                format!("Slot {}:", slot_idx + 1),
                                                TextStyle { font_size: 9.5, color: Color::srgb(0.7, 0.7, 0.7), ..default() }
                                            ));

                                            btn.spawn((
                                                TextBundle::from_section(
                                                    format!("[{}]", key_name),
                                                    TextStyle { font_size: 10.0, color: Color::srgb(1.0, 0.85, 0.35), ..default() }
                                                ),
                                                OptionsKeybindRebindText(slot_idx),
                                            ));
                                        });
                                    }
                                });
                            }
                        });

                        sec.spawn(NodeBundle {
                            style: Style {
                                margin: UiRect::top(Val::Px(4.0)),
                                ..default()
                            },
                            ..default()
                        }).with_children(|btn_wrap| {
                            btn_wrap.spawn((
                                ButtonBundle {
                                    style: Style {
                                        padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                                        border: UiRect::all(Val::Px(1.0)),
                                        ..default()
                                    },
                                    border_color: BorderColor(Color::srgb(0.4, 0.45, 0.5)),
                                    background_color: BackgroundColor(Color::srgb(0.20, 0.22, 0.28)),
                                    ..default()
                                },
                                OptionsKeybindResetBtn,
                            )).with_children(|b| {
                                b.spawn(TextBundle::from_section(
                                    "Reset Keybinds to Default",
                                    TextStyle { font_size: 10.0, color: Color::srgb(0.85, 0.85, 0.85), ..default() }
                                ));
                            });
                        });
                    });
                });

                // ---- TAB 4: NETWORK ----
                body.spawn((
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
                )).with_children(|net| {
                    spawn_framed_section(net, "SpacetimeDB Network & Replication", |sec| {
                        sec.spawn(TextBundle::from_section(
                            "• SpacetimeDB Server Tick Rate: 60 Hz\n\
                             • Local Client Interpolation: 60 FPS\n\
                             • Terrain Persistence: Authoritative Server Tables (VoxelChunk)\n\
                             • Deterministic Movement Replay: Active\n\
                             • Network Round-Trip Latency: Synchronized",
                            TextStyle { font_size: 10.5, color: Color::srgb(0.75, 0.78, 0.82), ..default() }
                        ));
                    });
                });
            });

            // Modal Footer Control Buttons
            dialog.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(44.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                    border: UiRect::top(Val::Px(1.5)),
                    ..default()
                },
                border_color: BorderColor(Color::srgb(0.25, 0.28, 0.32)),
                background_color: BackgroundColor(Color::srgb(0.12, 0.14, 0.18)),
                ..default()
            }).with_children(|footer| {
                // Left presets
                footer.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },
                    ..default()
                }).with_children(|left| {
                    left.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.35, 0.40, 0.45)),
                            background_color: BackgroundColor(Color::srgb(0.18, 0.20, 0.24)),
                            ..default()
                        },
                        OptionsRecommendedBtn,
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            "Recommended",
                            TextStyle { font_size: 10.5, color: Color::srgb(0.85, 0.85, 0.85), ..default() }
                        ));
                    });

                    left.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.35, 0.40, 0.45)),
                            background_color: BackgroundColor(Color::srgb(0.18, 0.20, 0.24)),
                            ..default()
                        },
                        OptionsClassicBtn,
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            "Classic EQ",
                            TextStyle { font_size: 10.5, color: Color::srgb(0.85, 0.85, 0.85), ..default() }
                        ));
                    });
                });

                // Right action buttons
                footer.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },
                    ..default()
                }).with_children(|right| {
                    right.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.85, 0.70, 0.25)),
                            background_color: BackgroundColor(Color::srgb(0.24, 0.20, 0.12)),
                            ..default()
                        },
                        OptionsOkayBtn,
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            "OK",
                            TextStyle { font_size: 11.0, color: Color::srgb(1.0, 0.85, 0.35), ..default() }
                        ));
                    });

                    right.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.40, 0.45, 0.50)),
                            background_color: BackgroundColor(Color::srgb(0.18, 0.20, 0.24)),
                            ..default()
                        },
                        OptionsCancelBtn,
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            "Cancel",
                            TextStyle { font_size: 11.0, color: Color::srgb(0.85, 0.85, 0.85), ..default() }
                        ));
                    });

                    right.spawn((
                        ButtonBundle {
                            style: Style {
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            border_color: BorderColor(Color::srgb(0.40, 0.45, 0.50)),
                            background_color: BackgroundColor(Color::srgb(0.18, 0.20, 0.24)),
                            ..default()
                        },
                        OptionsApplyBtn,
                    )).with_children(|b| {
                        b.spawn(TextBundle::from_section(
                            "Apply",
                            TextStyle { font_size: 11.0, color: Color::srgb(0.85, 0.85, 0.85), ..default() }
                        ));
                    });
                });
            });
        });
    });
}

pub fn handle_options_panel_interactions(
    (keys, camera_mode, _diagnostics): (
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
        Query<(&Interaction, &OptionsTabButton), Changed<Interaction>>,
        Query<&Interaction, (With<OptionsReticleToggleBtn>, Changed<Interaction>)>,
        Query<(&Interaction, &OptionsRangePresetButton), Changed<Interaction>>,
        Query<&Interaction, (With<OptionsSpawnWholeZoneBtn>, Changed<Interaction>)>,
        Query<(&Interaction, &OptionsKeybindRebindBtn), Changed<Interaction>>,
        Query<&Interaction, (With<OptionsKeybindResetBtn>, Changed<Interaction>)>,
        Query<&Interaction, (Or<(With<OptionsCloseBtn>, With<OptionsOkayBtn>, With<OptionsCancelBtn>)>, Changed<Interaction>)>,
        Query<&Interaction, (With<OptionsRecommendedBtn>, Changed<Interaction>)>,
        Query<&Interaction, (With<OptionsClassicBtn>, Changed<Interaction>)>,
    ),
) {
    let (
        mut panel_q,
        mut window_q,
        mut reticle_status_q,
        mut metrics_q,
        mut tab_content_q,
        mut tab_btn_q,
        mut tab_text_q,
        mut rebind_text_q,
    ) = ui_queries;

    let (
        tab_clicks,
        reticle_clicks,
        range_clicks,
        spawn_zone_clicks,
        rebind_clicks,
        reset_keybind_clicks,
        close_clicks,
        recommended_clicks,
        classic_clicks,
    ) = buttons;

    if !options_state.is_open {
        return;
    }

    // 1. Keybind Capture: if user clicked a slot to rebind, listen for next keyboard stroke
    if let Some(slot) = options_state.rebinding_slot {
        for &key in keys.get_just_pressed() {
            if key == KeyCode::Escape {
                options_state.rebinding_slot = None;
            } else {
                keybinds.keybinds[slot] = key;
                options_state.rebinding_slot = None;
            }

            // Refresh text badges
            for (marker, mut text) in rebind_text_q.iter_mut() {
                if marker.0 == slot {
                    text.sections[0].value = format!("[{}]", keycode_display_name(keybinds.keybinds[slot]));
                    text.sections[0].style.color = Color::srgb(1.0, 0.85, 0.35);
                }
            }
            break;
        }
    }

    // 2. Tab Navigation clicks
    for (interaction, tab_btn) in tab_clicks.iter() {
        if *interaction == Interaction::Pressed {
            options_state.active_tab = tab_btn.0;

            // Switch tab content visibility
            for (content, mut style) in tab_content_q.iter_mut() {
                style.display = if content.0 == options_state.active_tab {
                    Display::Flex
                } else {
                    Display::None
                };
            }

            // Update tab button highlights
            for (btn, mut bg, mut border) in tab_btn_q.iter_mut() {
                let is_active = btn.0 == options_state.active_tab;
                *bg = if is_active { Color::srgb(0.18, 0.20, 0.25).into() } else { Color::srgb(0.12, 0.13, 0.16).into() };
                *border = if is_active { Color::srgb(0.85, 0.70, 0.25).into() } else { Color::srgb(0.28, 0.30, 0.35).into() };
            }

            for (btn_text, mut text) in tab_text_q.iter_mut() {
                let is_active = btn_text.0 == options_state.active_tab;
                text.sections[0].style.color = if is_active {
                    Color::srgb(1.0, 0.85, 0.35)
                } else {
                    Color::srgb(0.65, 0.65, 0.70)
                };
            }
        }
    }

    // 3. Reticle Toggle Click
    for interaction in reticle_clicks.iter() {
        if *interaction == Interaction::Pressed {
            crosshair_settings.enabled = !crosshair_settings.enabled;
            if let Ok(mut text) = reticle_status_q.get_single_mut() {
                if crosshair_settings.enabled {
                    text.sections[0].value = "TOGGLE [ON]".to_string();
                    text.sections[0].style.color = Color::srgb(0.3, 0.9, 0.4);
                } else {
                    text.sections[0].value = "TOGGLE [OFF]".to_string();
                    text.sections[0].style.color = Color::srgb(0.9, 0.3, 0.3);
                }
            }
        }
    }

    // 4. View Distance Range Presets
    for (interaction, preset_btn) in range_clicks.iter() {
        if *interaction == Interaction::Pressed {
            render_settings.spawn_full_zone = false;
            render_settings.view_distance_chunks = preset_btn.0;
            render_settings.unload_distance_chunks = preset_btn.0 + 1;
            render_settings.visible_range_meters = (preset_btn.0 as f32) * 16.0;

            if let Ok(mut text) = metrics_q.get_single_mut() {
                text.sections[0].value = format!(
                    "Zone Mode: Normal Streaming ({} Chunks / {:.0}m)",
                    preset_btn.0, render_settings.visible_range_meters
                );
            }
        }
    }

    // 5. Spawn Whole Zone Stress Benchmark Trigger
    for interaction in spawn_zone_clicks.iter() {
        if *interaction == Interaction::Pressed {
            render_settings.spawn_full_zone = true;
            render_settings.view_distance_chunks = 64;
            render_settings.unload_distance_chunks = 70;
            render_settings.visible_range_meters = 1024.0;

            if let Ok(mut text) = metrics_q.get_single_mut() {
                text.sections[0].value = "Zone Mode: FULL ZONE SPAWN (64 Chunks / 1024m)".to_string();
            }
        }
    }

    // 6. Keybinding Rebind Button Clicks
    for (interaction, rebind_btn) in rebind_clicks.iter() {
        if *interaction == Interaction::Pressed {
            options_state.rebinding_slot = Some(rebind_btn.0);

            for (marker, mut text) in rebind_text_q.iter_mut() {
                if marker.0 == rebind_btn.0 {
                    text.sections[0].value = "[PRESS KEY]".to_string();
                    text.sections[0].style.color = Color::srgb(1.0, 0.3, 0.3);
                }
            }
        }
    }

    // 7. Reset Keybinds Button
    for interaction in reset_keybind_clicks.iter() {
        if *interaction == Interaction::Pressed {
            *keybinds = HotbarKeybinds::default();
            for (marker, mut text) in rebind_text_q.iter_mut() {
                let slot = marker.0;
                if slot < 16 {
                    text.sections[0].value = format!("[{}]", keycode_display_name(keybinds.keybinds[slot]));
                    text.sections[0].style.color = Color::srgb(1.0, 0.85, 0.35);
                }
            }
        }
    }

    // 8. Recommended / Classic EQ Presets
    for interaction in recommended_clicks.iter() {
        if *interaction == Interaction::Pressed {
            crosshair_settings.enabled = true;
            render_settings.spawn_full_zone = false;
            render_settings.view_distance_chunks = 14;
            render_settings.unload_distance_chunks = 15;
            render_settings.visible_range_meters = 230.4;
        }
    }

    for interaction in classic_clicks.iter() {
        if *interaction == Interaction::Pressed {
            crosshair_settings.enabled = false;
            render_settings.spawn_full_zone = false;
            render_settings.view_distance_chunks = 14;
            render_settings.unload_distance_chunks = 15;
            render_settings.visible_range_meters = 230.4;
        }
    }

    // 9. Close Modal Buttons (X, OK, Cancel)
    for interaction in close_clicks.iter() {
        if *interaction == Interaction::Pressed {
            options_state.is_open = false;
            if let Ok(mut style) = panel_q.get_single_mut() {
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
