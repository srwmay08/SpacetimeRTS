// ============================================================================
// File: client/src/ui/hotbar.rs
// ============================================================================
// ----------------------------------------------------------------------------
// 16-KEY PREPARED HOTBAR HUD, KEYBINDINGS & DRAG-AND-DROP
// ----------------------------------------------------------------------------

use crate::physics::*;
use bevy::prelude::{Transform as BevyTransform, *};
use bevy::window::PrimaryWindow;
use tracing::info;

use crate::components::*;
use crate::core::*;
use crate::audio_feedback::play_sound;
use spacetime_rts_logic::TacticalAbilityKind;

use crate::spells::*;
use super::spellbook::{SpellbookModalRoot, SpellbookSlotCard, SpellbookWindowState};
use super::options::{OptionsPanelModalRoot, OptionsPanelState};

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
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        KeyCode::Tab => "Tab",
        KeyCode::Space => "Space",
        KeyCode::Backquote => "`",
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
    pub slots: [Option<SpellId>; 16],
    pub cooldowns: [f32; 16],
    pub max_cooldowns: [f32; 16],
}

impl Default for PreparedHotbarState {
    fn default() -> Self {
        Self {
            slots: [None; 16],
            cooldowns: [0.0; 16],
            max_cooldowns: [1.0; 16],
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct SpellDragState {
    pub is_dragging: bool,
    pub source_slot: Option<usize>,
    pub spell_id: Option<SpellId>,
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
                        let spell_def = hotbar.slots[slot_idx].map(|id| id.def());

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

    if keybinds.is_changed() || key_text_q.iter().next().map_or(false, |(_, t)| t.sections[0].value.is_empty()) {
        for (k_marker, mut text) in key_text_q.iter_mut() {
            let slot = k_marker.0;
            if slot < 16 {
                let key_str = keycode_display_name(keybinds.keybinds[slot]);
                let new_val = format!("[{}]", key_str);
                if text.sections[0].value != new_val {
                    text.sections[0].value = new_val;
                }
            }
        }
    }

    if hotbar.is_changed() || name_text_q.iter().next().map_or(false, |(_, t)| t.sections[0].value.is_empty()) {
        for (n_marker, mut text) in name_text_q.iter_mut() {
            let slot = n_marker.0;
            if slot < 16 {
                let (new_icon, new_color) = if let Some(spell_id) = hotbar.slots[slot] {
                    let def = spell_id.def();
                    (def.icon, def.color)
                } else {
                    ("---", Color::srgb(0.3, 0.3, 0.3))
                };
                if text.sections[0].value != new_icon {
                    text.sections[0].value = new_icon.to_string();
                }
                if text.sections[0].style.color != new_color {
                    text.sections[0].style.color = new_color;
                }
            }
        }
    }

    for (cd_marker, mut text) in cd_text_q.iter_mut() {
        let slot = cd_marker.0;
        if slot < 16 {
            let cd = hotbar.cooldowns[slot];
            let new_val = if cd > 0.0 {
                format!("{:.1}s", cd)
            } else {
                String::new()
            };
            if text.sections[0].value != new_val {
                text.sections[0].value = new_val;
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
    open_spellbook_btn_q: Query<&Interaction, (With<HotbarOpenSpellbookButton>, Changed<Interaction>)>,
    styles: (
        Query<&mut Style, (With<SpellbookModalRoot>, Without<OptionsPanelModalRoot>)>,
        Query<&mut Window, With<PrimaryWindow>>,
    ),
    mut player_q: Query<(Entity, &mut BevyTransform, &mut LinearVelocity, &mut Kcc), With<PlayerBody>>,
    camera_q: Query<&GlobalTransform, With<FpsCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tactical_state: Option<ResMut<TacticalAbilityState>>,
    audio_handles: Option<Res<CombatAudioHandles>>,
) {
    if options_state.is_open || console.is_open || spell_drag.is_dragging {
        return;
    }

    let (mut spellbook_q, mut window_q) = styles;

    // Hotbar Spellbook Opener Button click
    for interaction in open_spellbook_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            spellbook_state.is_open = !spellbook_state.is_open;
            if let Ok(mut style) = spellbook_q.get_single_mut() {
                style.display = if spellbook_state.is_open { Display::Flex } else { Display::None };
            }
            if let Ok(mut window) = window_q.get_single_mut() {
                window.cursor.grab_mode = if spellbook_state.is_open {
                    bevy::window::CursorGrabMode::None
                } else {
                    bevy::window::CursorGrabMode::Locked
                };
                window.cursor.visible = spellbook_state.is_open;
            }
        }
    }

    // Direct Click on Hotbar Slot
    for (interaction, slot_btn) in slot_click_q.iter() {
        if *interaction == Interaction::Pressed {
            let slot = slot_btn.0;
            if slot < 16 {
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
    let Some(spell_id) = hotbar.slots[slot] else { return; };
    let def = spell_id.def();

    let Ok((_p_entity, mut p_trans, mut lin_vel, _)) = player_q.get_single_mut() else { return; };
    let cam_forward = camera_q.get_single().map_or(p_trans.forward(), |c| c.forward());
    let cam_pos = camera_q.get_single().map_or(p_trans.translation + Vec3::new(0.0, 1.5, 0.0), |c| c.translation());

    match spell_id {
        SpellId::PhaseDash => {
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
        SpellId::SmokeVeil => {
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
        SpellId::GravLift => {
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
        SpellId::Fireball => {
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
                LinearVelocity(*cam_forward * 18.0),
                Particle { timer: Timer::from_seconds(3.0, TimerMode::Once) },
            ));
        }
        SpellId::MagicMissile => {
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
                LinearVelocity(*cam_forward * 55.0),
                Particle { timer: Timer::from_seconds(2.0, TimerMode::Once) },
            ));
        }
        SpellId::MinorHealing => {
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
        SpellId::Blink => {
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

fn ui_node_screen_rect(transform: &GlobalTransform, node: &Node, _window: &Window) -> Rect {
    node.logical_rect(transform)
}

pub fn handle_spell_drag_and_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    console: Res<crate::core::ConsoleState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    card_query: Query<(&SpellbookSlotCard, &GlobalTransform, &Node, Option<&Interaction>)>,
    hotbar_slot_q: Query<(&HotbarSlotButton, &GlobalTransform, &Node, Option<&Interaction>)>,
    mut spell_drag: ResMut<SpellDragState>,
    mut hotbar: ResMut<PreparedHotbarState>,
    mut spellbook_state: ResMut<SpellbookWindowState>,
) {
    if console.is_open {
        return;
    }

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
                        spell_drag.spell_id = Some(def.spell_id);
                        spell_drag.current_pos = cursor_pos;
                        spellbook_state.selected_spell_for_slotting = Some(def.spell_id);
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
                if let Some(spell_id) = hotbar.slots[slot_btn.0] {
                    spell_drag.is_dragging = true;
                    spell_drag.source_slot = Some(slot_btn.0);
                    spell_drag.spell_id = Some(spell_id);
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
                info!("Prepared spell '{}' into Hotbar Slot {}", spell_id.def().name, target + 1);
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
        if style.display != Display::Flex {
            style.display = Display::Flex;
        }
        let l = Val::Px(spell_drag.current_pos.x - 25.0);
        let t = Val::Px(spell_drag.current_pos.y - 25.0);
        if style.left != l { style.left = l; }
        if style.top != t { style.top = t; }
        let label = if let Some(spell_id) = spell_drag.spell_id {
            spell_id.def().name
        } else {
            "Spell"
        };
        if text.sections[0].value != label {
            text.sections[0].value = label.to_string();
        }
    } else if style.display != Display::None {
        style.display = Display::None;
    }
}
