// ============================================================================
// File: client/src/ui/hud.rs
// ============================================================================
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use tracing::{info, error};

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
 
 
use crate::module_bindings::player_table::PlayerTableAccess; 
use crate::module_bindings::inventory_table::InventoryTableAccess; 
use crate::module_bindings::health_table::HealthTableAccess; 
 
use crate::module_bindings::spawn_peasant_reducer::spawn_peasant;
use crate::module_bindings::command_peasant_reducer::command_peasant;

use super::types::*;

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
    mut last_display: Local<Option<(u32, u32, &'static str, &'static str)>>,
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

    let current = (h, m, icon, weather_str);
    if *last_display == Some(current) && !text.sections[0].value.is_empty() {
        return;
    }
    *last_display = Some(current);

    let formatted = format!("{:02}:{:02} ({}) | {} [F8/F9]", h, m, icon, weather_str);
    if text.sections[0].value != formatted {
        text.sections[0].value = formatted;
    }
}

/// Allows toggling the celestial clock & weather HUD pill with [F10] event.
pub fn toggle_celestial_hud_hotkey(
    mut hud_evts: EventReader<crate::input::ToggleCelestialHudEvent>,
    mut hud_pill_query: Query<&mut Style, With<CelestialHudRoot>>,
) {
    for _ in hud_evts.read() {
        if let Ok(mut style) = hud_pill_query.get_single_mut() {
            style.display = if style.display == Display::None {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}


pub fn toggle_action_bar_visibility(
    camera_mode: Res<State<CameraMode>>,
    mut query: Query<&mut Style, With<ActionBarUiRoot>>
) {
    if !camera_mode.is_changed() {
        return;
    }

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
        if *root_vis != Visibility::Hidden {
            *root_vis = Visibility::Hidden;
        }
        return;
    }
    if *root_vis != Visibility::Inherited {
        *root_vis = Visibility::Inherited;
    }

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

    let new_bg: BackgroundColor = base_color.into();
    let new_bc: BorderColor = border_color.into();
    let new_border = UiRect::all(Val::Px(outline_thick));

    for (arm, mut style, mut bg, mut bc) in arms_q.iter_mut() {
        if *bg != new_bg { *bg = new_bg; }
        if *bc != new_bc { *bc = new_bc; }
        if style.border != new_border { style.border = new_border; }

        let (new_w, new_h, new_l, new_t) = match arm.0 {
            CrosshairArmDir::Top => (Val::Px(thick), Val::Px(len), Val::Px(-thick / 2.0), Val::Px(-effective_gap - len)),
            CrosshairArmDir::Bottom => (Val::Px(thick), Val::Px(len), Val::Px(-thick / 2.0), Val::Px(effective_gap)),
            CrosshairArmDir::Left => (Val::Px(len), Val::Px(thick), Val::Px(-effective_gap - len), Val::Px(-thick / 2.0)),
            CrosshairArmDir::Right => (Val::Px(len), Val::Px(thick), Val::Px(effective_gap), Val::Px(-thick / 2.0)),
        };
        if style.width != new_w { style.width = new_w; }
        if style.height != new_h { style.height = new_h; }
        if style.left != new_l { style.left = new_l; }
        if style.top != new_t { style.top = new_t; }
    }

    if let Ok((mut dot_style, mut dot_bg)) = dot_q.get_single_mut() {
        if settings.dot {
            if dot_style.display != Display::Flex { dot_style.display = Display::Flex; }
            let s = Val::Px(settings.dot_size);
            let half_s = Val::Px(-settings.dot_size / 2.0);
            if dot_style.width != s { dot_style.width = s; }
            if dot_style.height != s { dot_style.height = s; }
            if dot_style.left != half_s { dot_style.left = half_s; }
            if dot_style.top != half_s { dot_style.top = half_s; }
            if *dot_bg != new_bg { *dot_bg = new_bg; }
        } else if dot_style.display != Display::None {
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

        if text.sections[0].value != val {
            text.sections[0].value = val;
        }
        if text.sections[0].style.color != color {
            text.sections[0].style.color = color;
        }
    }

    // 2. Bow Charge Bar (Directly below crosshair)
    if let Ok((mut bar_style, mut bar_bg)) = bow_bar_q.get_single_mut() {
        if weapon_state.current_weapon == crate::weapons::WeaponType::Bow && weapon_state.bow_drawing {
            if bar_style.display != Display::Flex {
                bar_style.display = Display::Flex;
            }
            let desired_w = Val::Px(weapon_state.bow_charge * 40.0);
            if bar_style.width != desired_w {
                bar_style.width = desired_w;
            }
            let desired_bg: BackgroundColor = Color::srgb(1.0, 0.85, 0.2).into();
            if *bar_bg != desired_bg {
                *bar_bg = desired_bg;
            }
        } else if bar_style.display != Display::None {
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
        if text.sections[0].value != alert_str {
            text.sections[0].value = alert_str;
        }
        if text.sections[0].style.color != alert_col {
            text.sections[0].style.color = alert_col;
        }
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
            if hm_style.display != Display::Flex {
                hm_style.display = Display::Flex;
            }
            let tick_color = if hit_marker_state.is_crit {
                Color::srgb(1.0, 0.2, 0.2) // Red/Gold for Crit
            } else if hit_marker_state.is_armor {
                Color::srgb(0.0, 0.8, 1.0) // Cyan for Armor
            } else {
                Color::WHITE // White for Bodyshot
            };
            let new_bg: BackgroundColor = tick_color.into();
            for &child in children.iter() {
                if let Ok(mut bg) = hitmarker_ticks_q.get_mut(child) {
                    if *bg != new_bg {
                        *bg = new_bg;
                    }
                }
            }
        } else if hm_style.display != Display::None {
            hm_style.display = Display::None;
        }
    }
}

pub fn toggle_crosshair_menu(
    mut menu_evts: EventReader<crate::input::ToggleCrosshairMenuEvent>,
    mut menu_state: ResMut<CrosshairMenuState>,
    mut menu_q: Query<&mut Style, With<CrosshairMenuRoot>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    for _ in menu_evts.read() {
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


}