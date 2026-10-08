// ============================================================================
// File: client/src/ui/combat_feedback.rs
// ============================================================================
use bevy::prelude::*;

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
 
 
 
 
use crate::module_bindings::health_table::HealthTableAccess; 
 
use spacetimedb_sdk::Table;


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

            let l = Val::Px(min_x);
            let t = Val::Px(min_y);
            let w = Val::Px(max_x - min_x);
            let h = Val::Px(max_y - min_y);
            if style.left != l { style.left = l; }
            if style.top != t { style.top = t; }
            if style.width != w { style.width = w; }
            if style.height != h { style.height = h; }
            if *vis != Visibility::Inherited {
                *vis = Visibility::Inherited;
            }
            return;
        }
    }
    if *vis != Visibility::Hidden {
        *vis = Visibility::Hidden;
    }
}

pub fn visualize_selection(
    selected_query: Query<&Children, With<Selected>>,
    unselected_query: Query<&Children, (With<Selectable>, Without<Selected>)>,
    mut ring_query: Query<&mut Visibility, With<SelectionRing>>,
) {
    for children in selected_query.iter() {
        for &child in children.iter() {
            if let Ok(mut vis) = ring_query.get_mut(child) {
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
            }
        }
    }

    for children in unselected_query.iter() {
        for &child in children.iter() {
            if let Ok(mut vis) = ring_query.get_mut(child) {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}


use avian3d::prelude::{GravityScale, LinearVelocity};

pub fn tick_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut Particle,
        &mut Transform,
        Option<&mut LinearVelocity>,
        Option<&GravityScale>,
    )>,
) {
    let dt = time.delta_seconds();
    for (entity, mut particle, mut transform, mut vel_opt, grav_opt) in query.iter_mut() {
        if particle.timer.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        if let Some(ref mut vel) = vel_opt {
            if let Some(grav) = grav_opt {
                vel.0.y -= 9.81 * grav.0 * dt;
            }
            transform.translation += vel.0 * dt;
        }
    }
}

// ----------------------------------------------------------------------------
// RETICLE-ADJACENT FIGHTING HUD & CROSSHAIR SYSTEMS
// ----------------------------------------------------------------------------
// Architectural Note: Renders the reticle-adjacent fighting interface and handles
// unrestricted crosshair customization (dynamic spread expansion vs competitive
// static lock, custom thickness, length, gap, dot, outline, and high-contrast palettes).

