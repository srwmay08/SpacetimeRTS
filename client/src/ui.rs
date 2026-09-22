use bevy::prelude::*;
use tracing::{info, error};

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::building::BuildModeState; 
use crate::module_bindings::player_table::PlayerTableAccess; 
use crate::module_bindings::inventory_table::InventoryTableAccess; 
use crate::module_bindings::health_table::HealthTableAccess; 
use crate::module_bindings::spawn_peasant_reducer::spawn_peasant;
use crate::module_bindings::command_peasant_reducer::command_peasant;
use spacetimedb_sdk::Table;

// ----------------------------------------------------------------------------
// INVENTORY UI SETUP & MANAGEMENT
// ----------------------------------------------------------------------------

pub fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2dBundle {
        camera: Camera {
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        ..default()
    });

    // Architectural Note: Interaction prompt text displayed in the center of the screen 
    // when looking at pickable ground items or nodes.
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
            width: Val::Percent(100.0), 
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center, 
            align_items: AlignItems::Center, 
            ..default()
        },
        visibility: Visibility::Hidden,
        ..default()
    }, InventoryUiRoot)).with_children(|screen| {
        screen.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column, 
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(20.0)), 
                border: UiRect::all(Val::Px(4.0)), 
                ..default()
            },
            background_color: Color::srgba(0.1, 0.1, 0.1, 0.95).into(), 
            border_color: Color::srgb(0.3, 0.3, 0.3).into(),
            ..default()
        }).with_children(|backpack| {
            for row in 0..2 {
                backpack.spawn(NodeBundle {
                    style: Style { flex_direction: FlexDirection::Row, column_gap: Val::Px(10.0), ..default() },
                    ..default()
                }).with_children(|row_ui| {
                    for col in 0..8 {
                        let index = row * 8 + col;
                        row_ui.spawn(NodeBundle {
                            style: Style {
                                width: Val::Px(70.0), height: Val::Px(70.0), flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(5.0)), border: UiRect::all(Val::Px(2.0)), ..default()
                            },
                            border_color: Color::srgb(0.2, 0.2, 0.2).into(),
                            background_color: Color::srgb(0.15, 0.15, 0.15).into(), ..default()
                        }).with_children(|slot| {
                            slot.spawn((TextBundle::from_section("", TextStyle { font_size: 14.0, color: Color::WHITE, ..default() }), InventorySlotName(index)));
                            slot.spawn((TextBundle::from_section("", TextStyle { font_size: 24.0, color: Color::WHITE, ..default() }), InventorySlotCount(index)));
                        });
                    }
                });
            }
        });
    });

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
            TextStyle { font_size: 20.0, color: Color::srgb(0.1, 0.8, 1.0), ..default() }
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-200.0)),
            ..default()
        }),
        BuildUIText,
    ));

    commands.spawn((NodeBundle {
        style: Style {
            width: Val::Percent(100.0), height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            justify_content: JustifyContent::Center, align_items: AlignItems::FlexEnd,
            padding: UiRect::bottom(Val::Px(0.0)),
            display: Display::None, 
            ..default()
        },
        z_index: ZIndex::Global(50), 
        ..default()
    }, ActionBarUiRoot)).with_children(|root| {
        root.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row, column_gap: Val::Px(5.0),
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
                            width: Val::Px(80.0), height: Val::Px(80.0), flex_direction: FlexDirection::Column, 
                            justify_content: JustifyContent::Center, align_items: AlignItems::Center, border: UiRect::all(Val::Px(1.0)), ..default() 
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
                        error!("NETWORK ERROR: Failed to spawn peasant. Are you disconnected? Details: {:?}", e);
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

// ----------------------------------------------------------------------------
// INVENTORY & BUILD UI
// ----------------------------------------------------------------------------

pub fn update_build_ui(
    build_state: Res<BuildModeState>,
    mut ui_query: Query<(&mut Visibility, &mut Text), With<BuildUIText>>,
) {
    if build_state.is_changed() {
        for (mut vis, mut text) in ui_query.iter_mut() {
            if build_state.is_active {
                *vis = Visibility::Inherited;
                text.sections[0].value = format!(
                    "BUILD MODE: ACTIVE\n[R] to cycle: {} (Cost: {} Wood)", 
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
    mut query: Query<&mut Visibility, With<InventoryUiRoot>>
) {
    if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::KeyI) {
        for mut visibility in query.iter_mut() {
            *visibility = match *visibility {
                Visibility::Visible | Visibility::Inherited => Visibility::Hidden,
                Visibility::Hidden => Visibility::Inherited,
            };
        }
    }
}

pub fn update_inventory_ui(
    conn: Res<SpacetimeConnection>,
    mut name_q: Query<(&mut Text, &InventorySlotName)>,
    mut count_q: Query<(&mut Text, &InventorySlotCount, &mut Visibility), Without<InventorySlotName>>,
) {
    let Some(identity) = &conn.identity else { return; };
    if let Some(player) = conn.db.db.player().identity().find(identity) {
        if let Some(inventory) = conn.db.db.inventory().entity_id().find(&player.entity_id) {
            
            for (mut text, name) in name_q.iter_mut() {
                if let Some(slot) = inventory.slots.get(name.0) {
                    text.sections[0].value = slot.item_type.clone();
                } else {
                    text.sections[0].value = "".to_string();
                }
            }
            
            for (mut text, count, mut vis) in count_q.iter_mut() {
                if let Some(slot) = inventory.slots.get(count.0) {
                    text.sections[0].value = slot.count.to_string();
                    *vis = Visibility::Inherited;
                } else {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}

pub fn update_floating_health_bars(
    conn: Res<SpacetimeConnection>,
    camera_query: Query<(&Camera, &GlobalTransform), With<RtsCameraChild>>,
    unit_query: Query<(&NetworkEntity, &GlobalTransform)>,
    mut bar_query: Query<(&mut Style, &mut BackgroundColor, &mut Visibility), With<HealthBarUI>>,
    camera_mode: Res<State<CameraMode>>,
) {
    if *camera_mode.get() != CameraMode::RTS {
        for (_, _, mut vis) in bar_query.iter_mut() { *vis = Visibility::Hidden; }
        return;
    }

    let Ok((camera, cam_transform)) = camera_query.get_single() else { return; };
    
    let health_map: std::collections::HashMap<u64, f32> = conn.db.db.health().iter()
        .map(|h| (h.entity_id, h.current / h.max))
        .collect();

    for (net_id, transform) in unit_query.iter() {
        if let Some(&hp_percent) = health_map.get(&net_id.0) {
            if let Some(_screen_pos) = camera.world_to_viewport(cam_transform, transform.translation() + Vec3::Y * 2.0) {
                let _color = if hp_percent > 0.5 { Color::srgb(0.1, 0.8, 0.1) } 
                            else if hp_percent > 0.2 { Color::srgb(0.8, 0.8, 0.1) } 
                            else { Color::srgb(0.8, 0.1, 0.1) };
            }
        }
    }
}

// ----------------------------------------------------------------------------
// VISUAL EFFECTS & ANIMATIONS
// ----------------------------------------------------------------------------

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