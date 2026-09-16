use bevy::prelude::*;

// Note: Assuming `module_bindings` is exposed at the crate root.
use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;

// ----------------------------------------------------------------------------
// INVENTORY UI SETUP & MANAGEMENT
// ----------------------------------------------------------------------------

/// Constructs the DOM-like hierarchy for the inventory interface.
pub fn setup_ui(mut commands: Commands) {
    commands.spawn((NodeBundle {
        style: Style {
            width: Val::Percent(100.0), 
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center, 
            align_items: AlignItems::FlexEnd, 
            padding: UiRect::all(Val::Px(20.0)), 
            ..default()
        },
        ..default()
    }, InventoryUiRoot)).with_children(|parent| {
        parent.spawn(NodeBundle {
            style: Style {
                display: Display::Flex, 
                flex_direction: FlexDirection::Row, 
                column_gap: Val::Px(15.0),
                padding: UiRect::all(Val::Px(10.0)), 
                border: UiRect::all(Val::Px(4.0)), 
                ..default()
            },
            background_color: Color::srgba(0.1, 0.1, 0.1, 0.9).into(), 
            border_color: Color::srgb(0.3, 0.3, 0.3).into(),
            ..default()
        }).with_children(|hotbar| {
            
            hotbar.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(70.0), height: Val::Px(70.0), flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(5.0)), ..default()
                },
                background_color: Color::srgb(0.4, 0.2, 0.1).into(), ..default()
            }).with_children(|slot| {
                slot.spawn(TextBundle::from_section("Wood", TextStyle { font_size: 14.0, color: Color::WHITE, ..default() }));
                slot.spawn((TextBundle::from_section("0", TextStyle { font_size: 24.0, color: Color::WHITE, ..default() }), WoodText));
            });

            hotbar.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(70.0), height: Val::Px(70.0), flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(5.0)), ..default()
                },
                background_color: Color::srgb(0.5, 0.5, 0.5).into(), ..default()
            }).with_children(|slot| {
                slot.spawn(TextBundle::from_section("Ore", TextStyle { font_size: 14.0, color: Color::WHITE, ..default() }));
                slot.spawn((TextBundle::from_section("0", TextStyle { font_size: 24.0, color: Color::WHITE, ..default() }), OreText));
            });

            hotbar.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(70.0), height: Val::Px(70.0), flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(5.0)), ..default()
                },
                background_color: Color::srgb(0.8, 0.2, 0.2).into(), ..default()
            }).with_children(|slot| {
                slot.spawn(TextBundle::from_section("Food", TextStyle { font_size: 14.0, color: Color::WHITE, ..default() }));
                slot.spawn((TextBundle::from_section("0", TextStyle { font_size: 24.0, color: Color::WHITE, ..default() }), FoodText));
            });
        });
    });

    // Invisible marquee selection box setup
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
}

/// Polls hardware input directly to toggle the inventory visibility.
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

/// Reads authoritative resource data from SpacetimeDB to update text elements safely.
pub fn update_inventory_ui(
    conn: Res<SpacetimeConnection>,
    mut wood_q: Query<&mut Text, (With<WoodText>, Without<OreText>, Without<FoodText>)>,
    mut ore_q: Query<&mut Text, (With<OreText>, Without<WoodText>, Without<FoodText>)>,
    mut food_q: Query<&mut Text, (With<FoodText>, Without<WoodText>, Without<OreText>)>,
) {
    let Some(identity) = &conn.identity else { return; };
    
    if let Some(player) = conn.db.db.player().identity().find(identity) {
        if let Some(stockpile) = conn.db.db.resource_stockpile().entity_id().find(&player.entity_id) {
            if let Ok(mut text) = wood_q.get_single_mut() { text.sections[0].value = stockpile.wood.to_string(); }
            if let Ok(mut text) = ore_q.get_single_mut() { text.sections[0].value = stockpile.ore.to_string(); }
            if let Ok(mut text) = food_q.get_single_mut() { text.sections[0].value = stockpile.food.to_string(); }
        }
    }
}

// ----------------------------------------------------------------------------
// VISUAL EFFECTS & ANIMATIONS
// ----------------------------------------------------------------------------

/// Updates the dynamic size and position of the RTS selection box.
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

/// Toggles the green rendering ring underneath selected units.
pub fn visualize_selection(
    selected_query: Query<&Children, With<Selected>>,
    unselected_query: Query<&Children, (With<Selectable>, Without<Selected>)>,
    mut ring_query: Query<&mut Visibility, With<SelectionRing>>,
) {
    for children in selected_query.iter() {
        for &child in children.iter() {
            if let Ok(mut vis) = ring_query.get_mut(child) {
                *vis = Visibility::Inherited;
            }
        }
    }

    for children in unselected_query.iter() {
        for &child in children.iter() {
            if let Ok(mut vis) = ring_query.get_mut(child) {
                *vis = Visibility::Hidden;
            }
        }
    }
}

/// Drives the client-side predicted view model swing animation.
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

/// Garbage collects finished combat and interaction visual effects.
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