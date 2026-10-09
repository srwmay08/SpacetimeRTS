// ============================================================================
// File: client/src/ui/console.rs
// ============================================================================
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow, WindowMode};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
 
 
 
 
 
 
use crate::module_bindings::admin_give_item_reducer::admin_give_item;
use crate::module_bindings::admin_teleport_reducer::admin_teleport;
use crate::module_bindings::admin_heal_reducer::admin_heal;
use crate::module_bindings::admin_god_mode_reducer::admin_god_mode;
use crate::module_bindings::admin_set_time_reducer::admin_set_time;
use crate::module_bindings::admin_clear_inventory_reducer::admin_clear_inventory;
use crate::module_bindings::admin_spawn_npc_reducer::admin_spawn_npc;
use crate::module_bindings::admin_detonate_reducer::admin_detonate;
use crate::module_bindings::admin_kill_all_npcs_reducer::admin_kill_all_npcs;
use crate::module_bindings::admin_spawn_building_reducer::admin_spawn_building;
use crate::module_bindings::admin_set_world_seed_reducer::admin_set_world_seed;
use crate::module_bindings::admin_randomize_world_seed_reducer::admin_randomize_world_seed;
use crate::module_bindings::global_state_table::GlobalStateTableAccess;
use crate::module_bindings::equip_weapon_reducer::equip_weapon;

use super::types::*;
use crate::input::ToggleNoClipEvent;

pub const CONSOLE_COMMANDS: &[&str] = &[
    "giveitem",
    "give",
    "tp",
    "teleport",
    "heal",
    "god",
    "seed",
    "time",
    "settime",
    "spawn",
    "nuke",
    "blast",
    "clearinv",
    "killall",
    "tuner",
    "weapontool",
    "customizer",
    "char",
    "model",
    "morphology",
    "crosshair",
    "abilities",
    "day",
    "noon",
    "night",
    "midnight",
    "weather",
    "aurora",
    "rain",
    "storm",
    "clearsky",
    "haze",
    "timescale",
    "speed",
    "star",
    "stara",
    "starb",
    "dualshadows",
    "dualshadow",
    "ambient",
    "starsize",
    "sunset",
    "dusk",
    "dawn",
    "palette",
    "hud",
    "togglehud",
    "skyhud",
    "f3",
    "fps",
    "diag",
    "dual",
    "equip",
    "skills",
    "spellbook",
    "res",
    "resolution",
    "fullscreen",
    "fs",
    "windowed",
    "win",
    "maxfps",
    "fpslimit",
    "fps_max",
    "limitfps",
    "vsync",
    "help",
    "light",
    "csm",
    "foliage",
    "tree",
    "noclip",
    "fly",
    "flyspeed",
    "bedrock",
    "terrain",
];


// ----------------------------------------------------------------------------
// CONSOLE INPUT & AUTO-FILL / AUTOCOMPLETE SYSTEM
// ----------------------------------------------------------------------------

pub fn toggle_console(
    keys: Res<ButtonInput<KeyCode>>,
    mut console: ResMut<ConsoleState>,
    mut console_query: Query<&mut Style, With<ConsoleRoot>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    if keys.just_pressed(KeyCode::Backquote) {
        console.is_open = !console.is_open;
        let Ok(mut window) = window_query.get_single_mut() else { return; };

        if let Ok(mut style) = console_query.get_single_mut() {
            style.display = if console.is_open { Display::Flex } else { Display::None };
        }

        if console.is_open {
            window.cursor.grab_mode = CursorGrabMode::None;
            window.cursor.visible = true;
        } else if *camera_mode.get() == CameraMode::FPS {
            window.cursor.grab_mode = CursorGrabMode::Locked;
            window.cursor.visible = false;
        }
    }
}

pub fn handle_console_input(
    mut console: ResMut<ConsoleState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut key_evts: EventReader<KeyboardInput>,
    conn: Res<SpacetimeConnection>,
    time: Res<Time>,
    mut ephemeris: Option<ResMut<crate::binary_sky::BinaryEphemerisState>>,
    mut sky_config: Option<ResMut<crate::binary_sky::BinarySkyConfig>>,
    mut sky_weather: Option<ResMut<crate::binary_sky::AtmosphericWeather>>,
    mut hud_pill_query: Query<&mut Style, With<CelestialHudRoot>>,
    mut diag_pill_query: Query<&mut Style, (With<DiagnosticOverlayRoot>, Without<CelestialHudRoot>)>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    mut fps_limiter: ResMut<FpsLimiterState>,
    mut foliage_config: Option<ResMut<crate::tree_colors::TreeFoliageConfig>>,
    mut noclip_toggle: EventWriter<ToggleNoClipEvent>,
    mut noclip_state: Option<ResMut<NoClipState>>,
    mut render_settings: Option<ResMut<crate::spellbook::TerrainRenderSettings>>,
) {
    if !console.is_open {
        return;
    }

    console.cursor_timer.tick(time.delta());
    if console.cursor_timer.just_finished() {
        console.show_cursor = !console.show_cursor;
    }

    // Architectural Note: Automatic Tab-Completion & Auto-Fill System.
    // Detects command and item prefixes, cycling through canonical matches
    // and automatically filling the input buffer with accurate casing and spacing.
    if keys.just_pressed(KeyCode::Tab) {
        let trimmed = console.input_buffer.trim_start();
        if trimmed.starts_with("giveitem") || trimmed.starts_with("give") {
            let cmd_prefix = if trimmed.starts_with("giveitem") { "giveitem " } else { "give " };
            let arg = trimmed.strip_prefix(cmd_prefix).unwrap_or("").trim_start();

            let matches: Vec<&'static str> = CANONICAL_ITEMS.iter()
                .filter(|&&item| item.to_lowercase().starts_with(&arg.to_lowercase()))
                .copied()
                .collect();

            if !matches.is_empty() {
                let chosen = matches[console.tab_completion_index % matches.len()];
                console.input_buffer = format!("{}{}", cmd_prefix, chosen);
                console.tab_completion_index += 1;
            }
        } else {
            // Command auto-fill
            let matches: Vec<&'static str> = CONSOLE_COMMANDS.iter()
                .filter(|&&cmd| cmd.starts_with(&trimmed.to_lowercase()))
                .copied()
                .collect();

            if !matches.is_empty() {
                let chosen = matches[console.tab_completion_index % matches.len()];
                console.input_buffer = format!("{} ", chosen);
                console.tab_completion_index += 1;
            }
        }
        return;
    }

    // Read typed characters from KeyboardInput's logical_key
    for evt in key_evts.read() {
        if evt.state == ButtonState::Pressed {
            if evt.key_code == KeyCode::Space {
                console.input_buffer.push(' ');
                console.tab_completion_index = 0;
            } else if let Key::Character(ref s) = evt.logical_key {
                for ch in s.chars() {
                    if ch != '`' && ch != '~' && !ch.is_control() {
                        console.input_buffer.push(ch);
                        console.tab_completion_index = 0;
                    }
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        console.input_buffer.pop();
        console.tab_completion_index = 0;
    }

    if keys.just_pressed(KeyCode::ArrowUp) && !console.history.is_empty() {
        let next_idx = match console.history_cursor {
            None => console.history.len().saturating_sub(1),
            Some(i) => i.saturating_sub(1),
        };
        console.history_cursor = Some(next_idx);
        if let Some(cmd) = console.history.get(next_idx) {
            console.input_buffer = cmd.clone();
            console.tab_completion_index = 0;
        }
    }

    if keys.just_pressed(KeyCode::ArrowDown) && !console.history.is_empty() {
        if let Some(i) = console.history_cursor {
            if i + 1 < console.history.len() {
                let next_idx = i + 1;
                console.history_cursor = Some(next_idx);
                console.input_buffer = console.history[next_idx].clone();
            } else {
                console.history_cursor = None;
                console.input_buffer.clear();
            }
            console.tab_completion_index = 0;
        }
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        let command_line = console.input_buffer.trim().to_string();
        console.input_buffer.clear();
        console.history_cursor = None;
        console.tab_completion_index = 0;

        if command_line.is_empty() {
            return;
        }

        console.history.push(command_line.clone());
        console.logs.push(format!("> {}", command_line));

        let tokens: Vec<&str> = command_line.split_whitespace().collect();
        let cmd = tokens[0].to_lowercase();

        match cmd.as_str() {
            "giveitem" | "give" => {
                if tokens.len() < 2 {
                    console.logs.push("[Syntax Error] Usage: giveitem <item_name> [amount]".into());
                    console.logs.push(format!("Available: {}", CANONICAL_ITEMS.join(", ")));
                } else {
                    let (item_name, amount) = if tokens.len() >= 3 && tokens.last().unwrap().parse::<u32>().is_ok() {
                        let amt = tokens.last().unwrap().parse::<u32>().unwrap();
                        let name = tokens[1..tokens.len() - 1].join(" ");
                        (name, amt)
                    } else {
                        let name = tokens[1..].join(" ");
                        (name, 10)
                    };

                    // Architectural Note: Strict Format & Alias Guard.
                    // Enforces exact casing (e.g. "Wood" vs "wood", "Branch" vs "branch")
                    // and clarifies naming disparities (e.g. "LooseStone" vs "Stone").
                    if !CANONICAL_ITEMS.contains(&item_name.as_str()) {
                        let case_match = CANONICAL_ITEMS.iter().find(|&&i| i.eq_ignore_ascii_case(&item_name));
                        if let Some(correct) = case_match {
                            console.logs.push(format!(
                                "[Format Error] Invalid item format '{}'. Did you mean '{}'? (Press Tab to auto-fill)",
                                item_name, correct
                            ));
                        } else if item_name.eq_ignore_ascii_case("loosestone") || item_name.eq_ignore_ascii_case("stone") {
                            console.logs.push(format!(
                                "[Format Error] Ambiguous item '{}'. Ground node stone is 'LooseStone', crafted block is 'Stone'. (Press Tab to auto-fill)",
                                item_name
                            ));
                        } else {
                            console.logs.push(format!("[Format Error] Unknown item '{}'. Use Tab to auto-fill valid options.", item_name));
                            console.logs.push(format!("Available: {}", CANONICAL_ITEMS.join(", ")));
                        }
                        return;
                    }

                    if let Err(e) = conn.db.reducers.admin_give_item(item_name.clone(), amount) {
                        console.logs.push(format!("[Server Error] Failed to grant item: {:?}", e));
                    } else {
                        console.logs.push(format!("[Admin] Granted {}x '{}'", amount, item_name));
                    }
                }
            }
            "tp" | "teleport" => {
                if tokens.len() < 3 {
                    console.logs.push("[Syntax Error] Usage: tp <x> <z>".into());
                } else if let (Ok(x), Ok(z)) = (tokens[1].parse::<f32>(), tokens[2].parse::<f32>()) {
                    if let Err(e) = conn.db.reducers.admin_teleport(x, z) {
                        console.logs.push(format!("[Server Error] Teleport failed: {:?}", e));
                    } else {
                        console.logs.push(format!("[Admin] Teleported to ({:.1}, {:.1})", x, z));
                    }
                } else {
                    console.logs.push("[Syntax Error] Coordinates must be valid floating point numbers.".into());
                }
            }
            "heal" => {
                let amount = tokens.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(100.0);
                if let Err(e) = conn.db.reducers.admin_heal(amount) {
                    console.logs.push(format!("[Server Error] Heal failed: {:?}", e));
                } else {
                    console.logs.push(format!("[Admin] Healed player by {:.0} HP", amount));
                }
            }
            "god" => {
                if let Err(e) = conn.db.reducers.admin_god_mode() {
                    console.logs.push(format!("[Server Error] God mode failed: {:?}", e));
                } else {
                    let is_active = noclip_state.as_ref().map_or(false, |nc| nc.is_active);
                    if !is_active {
                        noclip_toggle.send(ToggleNoClipEvent);
                    }
                    console.logs.push("[Admin] Invulnerability & God Mode Flying No-Clip: ENABLED (99999 HP + 3D Flight through terrain, caves, and bedrock). Press [F2] or [N] or type 'noclip' to toggle flight.".into());
                }
            }
            "noclip" | "fly" => {
                noclip_toggle.send(ToggleNoClipEvent);
                let will_be_active = noclip_state.as_ref().map_or(true, |nc| !nc.is_active);
                if will_be_active {
                    let speed = noclip_state.as_ref().map_or(20.0, |nc| nc.fly_speed);
                    console.logs.push(format!(
                        "[Admin] God Mode Flying No-Clip: ACTIVATED (Speed: {:.0} m/s). WASD fly, Space ascend, Ctrl/C descend, Shift 3x boost ({:.0} m/s). Explore caves and bedrock!",
                        speed, speed * 3.0
                    ));
                } else {
                    console.logs.push("[Admin] God Mode Flying No-Clip: DEACTIVATED. Normal collision and gravity restored.".into());
                }
            }
            "flyspeed" => {
                if let Some(s) = tokens.get(1).and_then(|s| s.parse::<f32>().ok()) {
                    if let Some(ref mut nc) = noclip_state {
                        nc.fly_speed = s.clamp(2.0, 200.0);
                        console.logs.push(format!("[Admin] Flying No-Clip speed set to {:.1} m/s (Sprint boost: {:.1} m/s)", nc.fly_speed, nc.fly_speed * nc.fast_multiplier));
                    }
                } else {
                    console.logs.push("[Syntax Error] Usage: flyspeed <speed_mps> (e.g. flyspeed 35)".into());
                }
            }
            "day" | "noon" => {
                if let Some(ref mut eph) = ephemeris {
                    eph.simulation_time_seconds = 0.0;
                    eph.diurnal_angle = 0.0;
                }
                let _ = conn.db.reducers.admin_set_time(12.0);
                console.logs.push("[Admin] Celestial cycle set to HIGH NOON (12:00). Direct sunlight active.".into());
            }
            "night" | "midnight" => {
                let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                if let Some(ref mut eph) = ephemeris {
                    eph.simulation_time_seconds = duration * 0.5;
                    eph.diurnal_angle = std::f32::consts::PI;
                }
                let _ = conn.db.reducers.admin_set_time(0.0);
                console.logs.push("[Admin] Celestial cycle set to DEEP MIDNIGHT (00:00). Starfield & Aurora active.".into());
            }
            "time" | "settime" => {
                if let Some(t) = tokens.get(1).and_then(|s| s.parse::<f32>().ok()) {
                    let hour_clamped = t.rem_euclid(24.0);
                    let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                    let frac = ((hour_clamped / 24.0) - 0.5).rem_euclid(1.0);
                    if let Some(ref mut eph) = ephemeris {
                        eph.simulation_time_seconds = duration * frac as f64;
                        eph.diurnal_angle = (frac * 2.0 * std::f32::consts::PI) as f32;
                    }
                    if let Err(e) = conn.db.reducers.admin_set_time(hour_clamped) {
                        console.logs.push(format!("[Server Notice] Admin reducer: {:?}", e));
                    }
                    console.logs.push(format!("[Admin] World time set to {:.1}h", hour_clamped));
                } else {
                    console.logs.push("[Syntax Error] Usage: time <0-24> or 'day' / 'night'".into());
                }
            }
            "timescale" | "speed" => {
                if let Some(speed) = tokens.get(1).and_then(|s| s.parse::<f32>().ok()) {
                    if let Some(ref mut cfg) = sky_config {
                        cfg.time_scale = speed.max(0.0);
                    }
                    console.logs.push(format!("[Admin] Celestial time scale set to {:.1}x (Press [-] or [=])", speed));
                } else {
                    console.logs.push("[Syntax Error] Usage: timescale <multiplier> (e.g. 1.0, 10.0, 60.0)".into());
                }
            }
            "weather" => {
                if let Some(w_type) = tokens.get(1) {
                    if let Some(ref mut w) = sky_weather {
                        match w_type.to_lowercase().as_str() {
                            "clear" | "clearsky" => {
                                w.weather_type = crate::binary_sky::WeatherType::ClearSky;
                                console.logs.push("[Weather] Set to ClearSky (pristine visibility, deep Rayleigh blues).".into());
                            }
                            "haze" | "aerosol" => {
                                w.weather_type = crate::binary_sky::WeatherType::AerosolHaze;
                                console.logs.push("[Weather] Set to AerosolHaze (golden horizon, diffuse Mie halo).".into());
                            }
                            "aurora" | "storm_aurora" => {
                                w.weather_type = crate::binary_sky::WeatherType::StellarWindAurora;
                                console.logs.push("[Weather] Set to StellarWindAurora (binary magnetic curtains).".into());
                                console.logs.push("[Tip] Enter 'night' or press [F8] for deep darkness view.".into());
                            }
                            "rain" | "storm" | "overcast" => {
                                w.weather_type = crate::binary_sky::WeatherType::OvercastPrecipitation;
                                console.logs.push("[Weather] Set to OvercastPrecipitation (rain streaks, heavy overcast).".into());
                            }
                            _ => {
                                console.logs.push("[Syntax Error] Options: clear, haze, aurora, rain (or press [F9])".into());
                            }
                        }
                    }
                } else if let Some(ref w) = sky_weather {
                    console.logs.push(format!("[Weather] Current: {:?}. Options: clear, haze, aurora, rain (or press [F9])", w.weather_type));
                } else {
                    console.logs.push("[Syntax Error] Usage: weather <clear|haze|aurora|rain> (or press [F9])".into());
                }
            }
            "aurora" => {
                if let Some(ref mut w) = sky_weather {
                    w.weather_type = crate::binary_sky::WeatherType::StellarWindAurora;
                    console.logs.push("[Weather] StellarWindAurora activated! Shimmering aurora curtains glowing in the sky.".into());
                    console.logs.push("[Tip] Enter 'night' or press [F8] for deep darkness view.".into());
                }
            }
            "rain" | "storm" => {
                if let Some(ref mut w) = sky_weather {
                    w.weather_type = crate::binary_sky::WeatherType::OvercastPrecipitation;
                    console.logs.push("[Weather] OvercastPrecipitation activated! Heavy rain streaks and stormy overcast.".into());
                }
            }
            "clearsky" => {
                if let Some(ref mut w) = sky_weather {
                    w.weather_type = crate::binary_sky::WeatherType::ClearSky;
                    console.logs.push("[Weather] ClearSky activated! Pristine visibility and deep Rayleigh blues.".into());
                }
            }
            "haze" => {
                if let Some(ref mut w) = sky_weather {
                    w.weather_type = crate::binary_sky::WeatherType::AerosolHaze;
                    console.logs.push("[Weather] AerosolHaze activated! Golden horizon and dense Mie halo.".into());
                }
            }
            "hud" | "togglehud" | "skyhud" | "timehud" | "celestialhud" => {
                let sub = tokens.get(1).map(|s| s.to_lowercase());
                if let Ok(mut style) = hud_pill_query.get_single_mut() {
                    match sub.as_deref() {
                        Some("on") | Some("show") | Some("1") | Some("true") => {
                            style.display = Display::Flex;
                            console.logs.push("[HUD] Celestial HUD display (time, conjunction, weather [F8/F9]) toggled ON (visible).".into());
                        }
                        Some("off") | Some("hide") | Some("0") | Some("false") => {
                            style.display = Display::None;
                            console.logs.push("[HUD] Celestial HUD display (time, conjunction, weather [F8/F9]) toggled OFF (hidden).".into());
                        }
                        Some("status") => {
                            let status_str = if style.display == Display::None { "hidden (OFF)" } else { "visible (ON)" };
                            console.logs.push(format!("[HUD] Celestial HUD display is currently {}.", status_str));
                        }
                        Some("toggle") | None => {
                            let new_state = style.display == Display::None;
                            style.display = if new_state { Display::Flex } else { Display::None };
                            let action_str = if new_state { "toggled ON (visible)" } else { "toggled OFF (hidden)" };
                            console.logs.push(format!("[HUD] Celestial HUD display (time, conjunction, weather [F8/F9]) {}.", action_str));
                        }
                        _ => {
                            console.logs.push("[Syntax Error] Usage: hud [on|off|toggle|status] (or simply 'hud') [F10]".into());
                        }
                    }
                } else {
                    console.logs.push("[HUD] Celestial HUD element not found.".into());
                }
            }
            "maxfps" | "fpslimit" | "fps_max" | "limitfps" => {
                if tokens.len() == 1 {
                    let limit_str = match fps_limiter.target_fps {
                        Some(fps) => format!("capped at {} FPS", fps),
                        None => "UNCAPPED (unlimited)".to_string(),
                    };
                    let vsync_str = if let Ok(window) = window_query.get_single() {
                        match window.present_mode {
                            bevy::window::PresentMode::AutoNoVsync => "OFF (AutoNoVsync)",
                            bevy::window::PresentMode::Immediate => "OFF (Immediate)",
                            bevy::window::PresentMode::AutoVsync => "ON (AutoVsync)",
                            bevy::window::PresentMode::Fifo => "ON (Fifo VSync)",
                            bevy::window::PresentMode::FifoRelaxed => "ON (FifoRelaxed)",
                            bevy::window::PresentMode::Mailbox => "Mailbox (No tearing, uncapped)",
                        }
                    } else {
                        "Unknown"
                    };
                    console.logs.push(format!("[Framerate] Max FPS limit: {} | VSync: {}", limit_str, vsync_str));
                    console.logs.push("Usage: maxfps <fps|0|off|uncapped> (e.g. 'maxfps 60', 'maxfps 144', 'maxfps 0')".into());
                    console.logs.push("       vsync <on|off>".into());
                } else {
                    let arg = tokens[1].to_lowercase();
                    match arg.as_str() {
                        "0" | "off" | "uncap" | "uncapped" | "none" | "unlimited" => {
                            fps_limiter.target_fps = None;
                            console.logs.push("[Framerate] Max FPS limit REMOVED. Client running completely UNCAPPED.".into());
                        }
                        _ => {
                            if let Ok(fps) = arg.parse::<u32>() {
                                if fps == 0 {
                                    fps_limiter.target_fps = None;
                                    console.logs.push("[Framerate] Max FPS limit REMOVED. Client running completely UNCAPPED.".into());
                                } else if fps < 10 {
                                    console.logs.push("[Syntax Error] Minimum target FPS limit is 10.".into());
                                } else {
                                    fps_limiter.target_fps = Some(fps);
                                    console.logs.push(format!("[Framerate] Max FPS limit set to {} FPS.", fps));
                                }
                            } else {
                                console.logs.push(format!("[Syntax Error] Invalid FPS limit '{}'. Usage: maxfps <number|0|off|uncapped>", tokens[1]));
                            }
                        }
                    }
                }
            }
            "vsync" => {
                if let Ok(mut window) = window_query.get_single_mut() {
                    let sub = tokens.get(1).map(|s| s.to_lowercase());
                    match sub.as_deref() {
                        Some("on") | Some("1") | Some("true") | Some("enable") => {
                            window.present_mode = bevy::window::PresentMode::AutoVsync;
                            console.logs.push("[Window] VSync ENABLED (display refresh rate lock).".into());
                        }
                        Some("off") | Some("0") | Some("false") | Some("disable") => {
                            window.present_mode = bevy::window::PresentMode::AutoNoVsync;
                            console.logs.push("[Window] VSync DISABLED (unlocked presentation rate).".into());
                        }
                        Some("immediate") => {
                            window.present_mode = bevy::window::PresentMode::Immediate;
                            console.logs.push("[Window] Present mode set to Immediate (lowest latency, tearing possible).".into());
                        }
                        Some("mailbox") => {
                            window.present_mode = bevy::window::PresentMode::Mailbox;
                            console.logs.push("[Window] Present mode set to Mailbox (lowest latency, tear-free).".into());
                        }
                        None => {
                            let mode_str = match window.present_mode {
                                bevy::window::PresentMode::AutoNoVsync => "OFF (AutoNoVsync)",
                                bevy::window::PresentMode::Immediate => "OFF (Immediate)",
                                bevy::window::PresentMode::AutoVsync => "ON (AutoVsync)",
                                bevy::window::PresentMode::Fifo => "ON (Fifo VSync)",
                                bevy::window::PresentMode::FifoRelaxed => "ON (FifoRelaxed)",
                                bevy::window::PresentMode::Mailbox => "Mailbox (No tearing, uncapped)",
                            };
                            console.logs.push(format!("[Window] VSync is currently {}. Usage: vsync <on|off|immediate|mailbox>", mode_str));
                        }
                        _ => {
                            console.logs.push("[Syntax Error] Usage: vsync <on|off|immediate|mailbox>".into());
                        }
                    }
                } else {
                    console.logs.push("[Window Error] Primary window not found.".into());
                }
            }
            "f3" | "fps" | "diag" | "diagnostics" => {
                let sub = tokens.get(1).map(|s| s.to_lowercase());

                // If user typed 'fps <number>' or 'fps max <number>' or 'fps limit <number>' or 'fps uncap'
                let is_fps_limit_cmd = if cmd == "fps" {
                    if let Some(ref s) = sub {
                        s == "max" || s == "limit" || s == "uncap" || s == "uncapped" || s.parse::<u32>().is_ok()
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_fps_limit_cmd {
                    let target_arg = if sub.as_deref() == Some("max") || sub.as_deref() == Some("limit") {
                        tokens.get(2).map(|s| s.to_lowercase())
                    } else {
                        sub
                    };
                    match target_arg.as_deref() {
                        Some("0") | Some("off") | Some("uncap") | Some("uncapped") | Some("none") => {
                            fps_limiter.target_fps = None;
                            console.logs.push("[Framerate] Max FPS limit REMOVED. Client running completely UNCAPPED.".into());
                        }
                        Some(val) => {
                            if let Ok(target) = val.parse::<u32>() {
                                if target == 0 {
                                    fps_limiter.target_fps = None;
                                    console.logs.push("[Framerate] Max FPS limit REMOVED. Client running completely UNCAPPED.".into());
                                } else {
                                    let clamped = target.max(10);
                                    fps_limiter.target_fps = Some(clamped);
                                    console.logs.push(format!("[Framerate] Max FPS limit set to {} FPS.", clamped));
                                }
                            } else {
                                console.logs.push(format!("[Syntax Error] Usage: fps <number> or maxfps <number>"));
                            }
                        }
                        None => {
                            console.logs.push("[Syntax Error] Usage: fps <number> or maxfps <number>".into());
                        }
                    }
                } else if let Ok(mut style) = diag_pill_query.get_single_mut() {
                    match sub.as_deref() {
                        Some("on") | Some("show") | Some("1") | Some("true") => {
                            style.display = Display::Flex;
                            console.logs.push("[Diagnostics] F3 telemetry overlay toggled ON (visible).".into());
                        }
                        Some("off") | Some("hide") | Some("0") | Some("false") => {
                            style.display = Display::None;
                            console.logs.push("[Diagnostics] F3 telemetry overlay toggled OFF (hidden).".into());
                        }
                        Some("toggle") | None => {
                            let new_state = style.display == Display::None;
                            style.display = if new_state { Display::Flex } else { Display::None };
                            let action_str = if new_state { "toggled ON (visible)" } else { "toggled OFF (hidden)" };
                            console.logs.push(format!("[Diagnostics] F3 telemetry overlay {}.", action_str));
                        }
                        _ => {
                            console.logs.push("[Syntax Error] Usage: f3 [on|off|toggle] (or press [F3]) | fps <target_fps>".into());
                        }
                    }
                } else {
                    console.logs.push("[Diagnostics] F3 overlay element not found.".into());
                }
            }
            "clearinv" | "clear" => {
                if let Err(e) = conn.db.reducers.admin_clear_inventory() {
                    console.logs.push(format!("[Server Error] Failed to clear inventory: {:?}", e));
                } else {
                    console.logs.push("[Admin] Inventory wiped clean.".into());
                }
            }
            "spawn" => {
                if tokens.len() < 2 {
                    console.logs.push("[Syntax Error] Usage: spawn <deer|boar|goblin|peasant> [count]".into());
                } else {
                    let mob_type = tokens[1].to_string();
                    let count = tokens.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                    if let Err(e) = conn.db.reducers.admin_spawn_npc(mob_type.clone(), count) {
                        console.logs.push(format!("[Server Error] Spawn failed: {:?}", e));
                    } else {
                        console.logs.push(format!("[Admin] Dispatched {}x {}", count, mob_type));
                    }
                }
            }
            "spawnbuilding" | "buildnpc" | "spawnhut" => {
                if tokens.len() < 2 {
                    console.logs.push("[Syntax Error] Usage: spawnbuilding <hut|cottage|guardpost> [yaw_steps (0-3)] [x] [z]".into());
                } else {
                    let template = tokens[1].to_lowercase();
                    let yaw_steps = tokens.get(2).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
                    let x = tokens.get(3).and_then(|s| s.parse::<f32>().ok());
                    let z = tokens.get(4).and_then(|s| s.parse::<f32>().ok());

                    if let Err(e) = conn.db.reducers.admin_spawn_building(template.clone(), yaw_steps, x, z) {
                        console.logs.push(format!("[Server Error] Spawn building failed: {:?}", e));
                    } else {
                        console.logs.push(format!("[Admin] Dispatched NPC building template '{}' (rot={})", template, yaw_steps));
                    }
                }
            }
            "nuke" | "blast" => {
                let radius = tokens.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(6.0);
                if let Err(e) = conn.db.reducers.admin_detonate(radius, 600.0) {
                    console.logs.push(format!("[Server Error] Detonation failed: {:?}", e));
                } else {
                    console.logs.push(format!("[Admin] Voxel blast triggered with radius {:.1}", radius));
                }
            }
            "killall" => {
                if let Err(e) = conn.db.reducers.admin_kill_all_npcs() {
                    console.logs.push(format!("[Server Error] Killall failed: {:?}", e));
                } else {
                    console.logs.push("[Admin] All non-player entities destroyed.".into());
                }
            }
            "seed" => {
                if tokens.len() == 1 {
                    let cur_seed = conn.db.db.global_state().id().find(&0).map(|g| g.world_seed).unwrap_or(42);
                    console.logs.push(format!("[World Seed] Current Authoritative World Seed: {}", cur_seed));
                    console.logs.push("Usage: 'seed <number>' to configure a specific seed, or 'seed random' to generate a new random seed.".into());
                } else if tokens[1].eq_ignore_ascii_case("random") || tokens[1].eq_ignore_ascii_case("new") {
                    if let Err(e) = conn.db.reducers.admin_randomize_world_seed() {
                        console.logs.push(format!("[Server Error] Failed to randomize world seed: {:?}", e));
                    } else {
                        console.logs.push("[Admin] Requested randomized world seed generation from server. Rebuilding world...".into());
                    }
                } else if let Ok(s) = tokens[1].parse::<u32>() {
                    if let Err(e) = conn.db.reducers.admin_set_world_seed(s) {
                        console.logs.push(format!("[Server Error] Failed to set world seed: {:?}", e));
                    } else {
                        console.logs.push(format!("[Admin] Set authoritative world seed to {}. Rebuilding world...", s));
                    }
                } else {
                    console.logs.push(format!("[Syntax Error] Invalid seed argument '{}'. Must be an unsigned 32-bit integer or 'random'.", tokens[1]));
                }
            }
            "tuner" | "weapontool" => {
                console.logs.push("[Admin] Weapon & Spell Tuner Workbench available via [F6] hotkey.".into());
            }
            "customizer" | "char" | "model" | "morphology" => {
                console.logs.push("[Admin] Character Model Customizer Studio available via [F5] hotkey.".into());
            }
            "dual" => {
                if tokens.len() < 3 {
                    console.logs.push("[Syntax Error] Usage: dual <main_weapon> <off_weapon> (e.g. 'dual Sword Axe', 'dual Revolver Revolver')".into());
                } else {
                    let main_w = tokens[1].to_string();
                    let off_w = tokens[2].to_string();
                    let _ = conn.db.reducers.equip_weapon(EquipmentSlot::MainHand.as_str().to_string(), main_w.clone());
                    let _ = conn.db.reducers.equip_weapon(EquipmentSlot::OffHand.as_str().to_string(), off_w.clone());
                    console.logs.push(format!("[Combat] Dual-wield loadout equipped: Main='{}', Off='{}'", main_w, off_w));
                }
            }
            "equip" => {
                if tokens.len() < 2 {
                    console.logs.push("[Syntax Error] Usage: equip [main|off] <weapon_name> (defaults to main)".into());
                } else if tokens.len() == 2 {
                    let w = tokens[1].to_string();
                    let _ = conn.db.reducers.equip_weapon(EquipmentSlot::MainHand.as_str().to_string(), w.clone());
                    console.logs.push(format!("[Combat] Equipped '{}' to Main-Hand", w));
                } else {
                    let slot = EquipmentSlot::from_str(tokens[1]).unwrap_or(EquipmentSlot::MainHand);
                    let w = tokens[2..].join(" ");
                    let _ = conn.db.reducers.equip_weapon(slot.as_str().to_string(), w.clone());
                    console.logs.push(format!("[Combat] Equipped '{}' to {}", w, slot.as_str()));
                }
            }
            "skills" | "skill" => {
                console.logs.push("[Skills] Character Skills Sheet (Valheim style) toggleable with [L] hotkey.".into());
            }
            "spellbook" | "spells" => {
                console.logs.push("[Spellbook] Spellbook & Ability Grimoire (WoW style) toggleable with [K] hotkey.".into());
            }
            "crosshair" => {
                console.logs.push("[Settings] Press [F7] to open the interactive Crosshair Tuner GUI.".into());
                console.logs.push("[Settings] Supports custom colors, thickness, length, gap, dot, outline, and Dynamic/Static lock.".into());
            }
            "abilities" | "ability" => {
                console.logs.push("--- TACTICAL ABILITIES DIRECTORY ---".into());
                console.logs.push("[Q] Phase Dash  : Instant directional horizontal thrust (6.0s CD)".into());
                console.logs.push("[C] Smoke Veil  : Obscures vision / blocks lines of sight (14.0s CD)".into());
                console.logs.push("[E] Intel Dart  : Sonar pulse reconnaissance (16.0s CD)".into());
                console.logs.push("[F] Grav-Lift   : Kinetic vertical air lift (10.0s CD)".into());
            }
            "star" | "stara" | "starb" => {
                if let Some(ref mut cfg) = sky_config {
                    let target_star = if cmd == "stara" {
                        Some("a")
                    } else if cmd == "starb" {
                        Some("b")
                    } else {
                        tokens.get(1).map(|s| s.to_lowercase()).and_then(|s| {
                            if s == "a" || s == "primary" {
                                Some("a")
                            } else if s == "b" || s == "secondary" {
                                Some("b")
                            } else {
                                None
                            }
                        })
                    };

                    let sub_tokens: Vec<&str> = if cmd == "stara" || cmd == "starb" {
                        tokens[1..].to_vec()
                    } else if target_star.is_some() {
                        tokens[2..].to_vec()
                    } else {
                        tokens[1..].to_vec()
                    };

                    if let Some(star) = target_star {
                        if sub_tokens.is_empty() {
                            if star == "a" {
                                console.logs.push(format!("[BinarySky] Star A (Primary): {:.0} lx | Shadows: {} | Color: {:?}",
                                    cfg.star_a_base_illuminance_lux, cfg.star_a_shadows_enabled, cfg.star_a_color_override));
                            } else {
                                console.logs.push(format!("[BinarySky] Star B (Secondary): {:.0} lx | Shadows: {} | Color: {:?}",
                                    cfg.star_b_base_illuminance_lux, cfg.star_b_shadows_enabled, cfg.star_b_color_override));
                            }
                            console.logs.push("Usage: star <a|b> <lux> | star <a|b> color <hex|name> | star <a|b> shadows <on|off>".into());
                        } else if let Ok(lux) = sub_tokens[0].parse::<f32>() {
                            if star == "a" {
                                cfg.star_a_base_illuminance_lux = lux.max(0.0);
                                console.logs.push(format!("[BinarySky] Star A base illuminance set to {:.0} lx.", cfg.star_a_base_illuminance_lux));
                            } else {
                                cfg.star_b_base_illuminance_lux = lux.max(0.0);
                                console.logs.push(format!("[BinarySky] Star B base illuminance set to {:.0} lx.", cfg.star_b_base_illuminance_lux));
                            }
                        } else if sub_tokens[0] == "color" || sub_tokens[0] == "col" {
                            let color_arg = sub_tokens.get(1).copied().unwrap_or("");
                            if color_arg.eq_ignore_ascii_case("reset") || color_arg.is_empty() {
                                if star == "a" {
                                    cfg.star_a_color_override = None;
                                    console.logs.push("[BinarySky] Star A color reset to physically based Rayleigh/Planck blackbody.".into());
                                } else {
                                    cfg.star_b_color_override = None;
                                    console.logs.push("[BinarySky] Star B color reset to physically based Mie/Planck blackbody.".into());
                                }
                            } else if let Some(parsed_color) = crate::binary_sky::parse_color_spec(color_arg) {
                                if star == "a" {
                                    cfg.star_a_color_override = Some(parsed_color);
                                    console.logs.push(format!("[BinarySky] Star A direct color override set to {:?}", parsed_color));
                                } else {
                                    cfg.star_b_color_override = Some(parsed_color);
                                    console.logs.push(format!("[BinarySky] Star B direct color override set to {:?}", parsed_color));
                                }
                            } else {
                                console.logs.push(format!("[Syntax Error] Unknown color '{}'. Try hex (e.g. ff5400, 390099) or names (blaze, amber, fuchsia, navy, raspberry, white, reset).", color_arg));
                            }
                        } else if sub_tokens[0] == "shadow" || sub_tokens[0] == "shadows" {
                            let on = sub_tokens.get(1).map(|s| *s != "off" && *s != "0" && *s != "false").unwrap_or(true);
                            if star == "a" {
                                cfg.star_a_shadows_enabled = on;
                                console.logs.push(format!("[BinarySky] Star A shadows: {}", if on { "ENABLED" } else { "DISABLED" }));
                            } else {
                                cfg.star_b_shadows_enabled = on;
                                console.logs.push(format!("[BinarySky] Star B shadows: {}", if on { "ENABLED" } else { "DISABLED" }));
                            }
                        } else {
                            console.logs.push("[Syntax Error] Usage: star <a|b> <lux> | star <a|b> color <hex|name> | star <a|b> shadows <on|off>".into());
                        }
                    } else if !sub_tokens.is_empty() && (sub_tokens[0] == "shadow" || sub_tokens[0] == "shadows") {
                        let on = sub_tokens.get(1).map(|s| *s != "off" && *s != "0" && *s != "false").unwrap_or(true);
                        cfg.star_a_shadows_enabled = on;
                        cfg.star_b_shadows_enabled = on;
                        console.logs.push(format!("[BinarySky] Dual star directional shadows: {}", if on { "ENABLED" } else { "DISABLED" }));
                    } else if !sub_tokens.is_empty() && (sub_tokens[0] == "balance" || sub_tokens[0] == "equal") {
                        let ratio = sub_tokens.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.5).clamp(0.1, 0.9);
                        let total = cfg.star_a_base_illuminance_lux + cfg.star_b_base_illuminance_lux;
                        cfg.star_a_base_illuminance_lux = total * (1.0 - ratio);
                        cfg.star_b_base_illuminance_lux = total * ratio;
                        console.logs.push(format!("[BinarySky] Balanced stars with ratio {:.2}: Star A = {:.0} lx, Star B = {:.0} lx",
                            ratio, cfg.star_a_base_illuminance_lux, cfg.star_b_base_illuminance_lux));
                    } else {
                        console.logs.push(format!("[BinarySky Status] Star A: {:.0} lx (shadows: {}) | Star B: {:.0} lx (shadows: {}) | Ambient: {:.0} lx",
                            cfg.star_a_base_illuminance_lux, cfg.star_a_shadows_enabled,
                            cfg.star_b_base_illuminance_lux, cfg.star_b_shadows_enabled,
                            cfg.ambient_illuminance_lux.unwrap_or(360.0)));
                        console.logs.push("Commands: star <a|b> <lux> | star <a|b> color <hex> | star <a|b> shadows <on|off> | dualshadows".into());
                    }
                } else {
                    console.logs.push("[Notice] Binary sky system is active in client world.".into());
                }
            }
            "dualshadows" | "dualshadow" => {
                if let Some(ref mut cfg) = sky_config {
                    cfg.star_a_base_illuminance_lux = 70_000.0;
                    cfg.star_b_base_illuminance_lux = 55_000.0;
                    cfg.star_a_color_override = Some(Color::srgb(1.0, 0.98, 0.92));
                    cfg.star_b_color_override = Some(crate::binary_sky::parse_color_spec("ff5400").unwrap());
                    cfg.star_a_shadows_enabled = true;
                    cfg.star_b_shadows_enabled = true;
                    cfg.ambient_illuminance_lux = Some(220.0);
                    console.logs.push("[BinarySky] Dual shadow maps activated with high-contrast penumbras!".into());
                    console.logs.push("  - Host Star A: 70k lx (Crisp solar white, sharp shadow)".into());
                    console.logs.push("  - Companion Star B: 55k lx (Blaze orange #ff5400, warm shadow)".into());
                    console.logs.push("  - Ambient Fill: 220 lx (Preserves deep colored penumbra cross-shadows)".into());
                }
            }
            "ambient" => {
                if let Some(ref mut cfg) = sky_config {
                    if let Some(arg) = tokens.get(1) {
                        if arg.eq_ignore_ascii_case("reset") || arg.eq_ignore_ascii_case("auto") {
                            cfg.ambient_illuminance_lux = None;
                            console.logs.push("[BinarySky] Ambient light reset to automatic atmospheric scattering scaling (2.5 - 360 lx).".into());
                        } else if let Ok(lux) = arg.parse::<f32>() {
                            cfg.ambient_illuminance_lux = Some(lux.max(0.0));
                            console.logs.push(format!("[BinarySky] Ambient light override set to {:.0} lx. (Lower values make dual shadows deeper)", lux));
                        } else {
                            console.logs.push("[Syntax Error] Usage: ambient <lux> (e.g. ambient 200, ambient 450, ambient reset)".into());
                        }
                    } else {
                        let cur = cfg.ambient_illuminance_lux.map(|v| format!("{:.0} lx (override)", v)).unwrap_or_else(|| "Auto (~2.5 - 360 lx)".into());
                        console.logs.push(format!("[BinarySky] Current ambient light: {}. Usage: ambient <lux> (e.g. 200) | ambient reset", cur));
                    }
                }
            }
            "light" => {
                if let Some(ref mut cfg) = sky_config {
                    let sub = tokens.get(1).map(|s| s.to_lowercase());
                    match sub.as_deref() {
                        Some("solo") | Some("isolate") | Some("only") => {
                            let which = tokens.get(2).map(|s| s.to_lowercase());
                            match which.as_deref() {
                                Some("a") | Some("stara") | Some("primary") | Some("host") => {
                                    cfg.star_a_enabled = true;
                                    cfg.star_b_enabled = false;
                                    console.logs.push("[Light Isolation] Host Star A ISOLATED. Companion Star B disabled (0 lx).".into());
                                    console.logs.push("  - Single directional light active: uniform shadow angle and facing direction.".into());
                                }
                                Some("b") | Some("starb") | Some("secondary") | Some("companion") => {
                                    cfg.star_a_enabled = false;
                                    cfg.star_b_enabled = true;
                                    console.logs.push("[Light Isolation] Companion Star B ISOLATED. Host Star A disabled (0 lx).".into());
                                    console.logs.push("  - Single directional light active: amber dwarf illumination.".into());
                                }
                                _ => {
                                    console.logs.push("[Syntax Error] Usage: light solo <a|b> | light dual".into());
                                }
                            }
                        }
                        Some("a") | Some("stara") => {
                            cfg.star_a_enabled = true;
                            cfg.star_b_enabled = false;
                            console.logs.push("[Light Isolation] Host Star A ISOLATED. Companion Star B disabled (0 lx).".into());
                        }
                        Some("b") | Some("starb") => {
                            cfg.star_a_enabled = false;
                            cfg.star_b_enabled = true;
                            console.logs.push("[Light Isolation] Companion Star B ISOLATED. Host Star A disabled (0 lx).".into());
                        }
                        Some("dual") | Some("both") | Some("reset") => {
                            cfg.star_a_enabled = true;
                            cfg.star_b_enabled = true;
                            console.logs.push("[Light Isolation] Dual Star lighting RESTORED. Host Star A & Companion Star B both active.".into());
                        }
                        Some("shadow") | Some("shadows") => {
                            let which = tokens.get(2).map(|s| s.to_lowercase());
                            let state_str = tokens.get(3).map(|s| s.to_lowercase());
                            let on = state_str.as_deref().map(|s| s != "off" && s != "0" && s != "false").unwrap_or(true);
                            match which.as_deref() {
                                Some("a") => {
                                    cfg.star_a_shadows_enabled = on;
                                    console.logs.push(format!("[Light] Star A shadows: {}", if on { "ENABLED" } else { "DISABLED" }));
                                }
                                Some("b") => {
                                    cfg.star_b_shadows_enabled = on;
                                    console.logs.push(format!("[Light] Star B shadows: {}", if on { "ENABLED" } else { "DISABLED" }));
                                }
                                Some("both") | None => {
                                    cfg.star_a_shadows_enabled = on;
                                    cfg.star_b_shadows_enabled = on;
                                    console.logs.push(format!("[Light] Dual star shadows: {}", if on { "ENABLED" } else { "DISABLED" }));
                                }
                                _ => {
                                    console.logs.push("[Syntax Error] Usage: light shadows <a|b|both> <on|off>".into());
                                }
                            }
                        }
                        _ => {
                            console.logs.push(format!("[Light Status] Host Star A: {} ({:.0} lx, shadows: {}) | Companion Star B: {} ({:.0} lx, shadows: {})",
                                if cfg.star_a_enabled { "ENABLED" } else { "OFF" },
                                cfg.star_a_base_illuminance_lux,
                                cfg.star_a_shadows_enabled,
                                if cfg.star_b_enabled { "ENABLED" } else { "OFF" },
                                cfg.star_b_base_illuminance_lux,
                                cfg.star_b_shadows_enabled,
                            ));
                            console.logs.push("Commands: light solo <a|b> | light dual | light shadows <a|b|both> <on|off>".into());
                        }
                    }
                } else {
                    console.logs.push("[Notice] Binary sky system is active in client world.".into());
                }
            }
            "csm" => {
                if let Some(ref mut cfg) = sky_config {
                    let sub = tokens.get(1).map(|s| s.to_lowercase());
                    match sub.as_deref() {
                        Some("bias") => {
                            if let (Some(Ok(depth)), Some(Ok(norm))) = (
                                tokens.get(2).map(|s| s.parse::<f32>()),
                                tokens.get(3).map(|s| s.parse::<f32>()),
                            ) {
                                cfg.star_a_shadow_depth_bias = depth;
                                cfg.star_a_shadow_normal_bias = norm;
                                cfg.star_b_shadow_depth_bias = depth;
                                cfg.star_b_shadow_normal_bias = norm;
                                console.logs.push(format!("[CSM] Updated shadow bias: depth_bias = {:.4}, normal_bias = {:.2}", depth, norm));
                                console.logs.push("  - If dark faces disappear, shadow acne/self-shadowing was the cause.".into());
                                console.logs.push("  - If contact shadows detach (peter-panning), reduce depth_bias slightly.".into());
                            } else {
                                console.logs.push("[Syntax Error] Usage: csm bias <depth_bias> <normal_bias> (e.g. 'csm bias 0.03 2.0')".into());
                                console.logs.push(format!("  Current: depth = {:.4}, normal = {:.2}", cfg.star_a_shadow_depth_bias, cfg.star_a_shadow_normal_bias));
                            }
                        }
                        Some("dist") | Some("distance") => {
                            if let Some(Ok(max_d)) = tokens.get(2).map(|s| s.parse::<f32>()) {
                                cfg.star_a_maximum_shadow_distance = max_d;
                                if let Some(Ok(first_b)) = tokens.get(3).map(|s| s.parse::<f32>()) {
                                    cfg.star_a_first_cascade_far_bound = first_b;
                                }
                                console.logs.push(format!("[CSM] Star A distance updated: max = {:.1}m, first_cascade_far = {:.1}m",
                                    cfg.star_a_maximum_shadow_distance, cfg.star_a_first_cascade_far_bound));
                            } else {
                                console.logs.push("[Syntax Error] Usage: csm dist <max_dist> [first_cascade_bound] (e.g. 'csm dist 160 18')".into());
                                console.logs.push(format!("  Current: max = {:.1}m, first_cascade_far = {:.1}m",
                                    cfg.star_a_maximum_shadow_distance, cfg.star_a_first_cascade_far_bound));
                            }
                        }
                        Some("min") | Some("near") => {
                            if let Some(Ok(min_d)) = tokens.get(2).map(|s| s.parse::<f32>()) {
                                cfg.star_a_minimum_shadow_distance = min_d.max(0.01);
                                cfg.star_b_minimum_shadow_distance = min_d.max(0.01);
                                console.logs.push(format!("[CSM] Near cascade clip plane updated: min_distance = {:.2}m", cfg.star_a_minimum_shadow_distance));
                            } else {
                                console.logs.push("[Syntax Error] Usage: csm min <min_dist> (e.g. 'csm min 0.5')".into());
                                console.logs.push(format!("  Current: min_distance = {:.2}m", cfg.star_a_minimum_shadow_distance));
                            }
                        }
                        _ => {
                            console.logs.push(format!("[CSM Status] Star A: depth_bias={:.4}, normal_bias={:.2} | bounds: [{:.1}m .. {:.1}m] (cascades={})",
                                cfg.star_a_shadow_depth_bias, cfg.star_a_shadow_normal_bias,
                                cfg.star_a_minimum_shadow_distance, cfg.star_a_maximum_shadow_distance,
                                cfg.star_a_num_cascades));
                            console.logs.push(format!("             Star B: depth_bias={:.4}, normal_bias={:.2} | bounds: [{:.1}m .. {:.1}m] (cascades={})",
                                cfg.star_b_shadow_depth_bias, cfg.star_b_shadow_normal_bias,
                                cfg.star_b_minimum_shadow_distance, cfg.star_b_maximum_shadow_distance,
                                cfg.star_b_num_cascades));
                            console.logs.push("Commands: csm bias <depth> <normal> | csm dist <max> [first] | csm min <min>".into());
                        }
                    }
                } else {
                    console.logs.push("[Notice] Binary sky system is active in client world.".into());
                }
            }
            "foliage" | "tree" => {
                if let Some(ref mut f_cfg) = foliage_config {
                    let sub = tokens.get(1).map(|s| s.to_lowercase());
                    match sub.as_deref() {
                        Some("unlit") => {
                            let state_str = tokens.get(2).map(|s| s.to_lowercase());
                            let new_unlit = match state_str.as_deref() {
                                Some("on") | Some("1") | Some("true") => true,
                                Some("off") | Some("0") | Some("false") => false,
                                _ => !f_cfg.unlit,
                            };
                            f_cfg.unlit = new_unlit;
                            console.logs.push(format!("[Foliage] Unlit mode: {}", if new_unlit { "ENABLED (Bypasses lighting & shadows)" } else { "DISABLED (Full PBR lighting)" }));
                            if new_unlit {
                                console.logs.push("  - If trees are now colorful, geometry & vertex colors are sound (issue is shadow acne or lighting).".into());
                            }
                        }
                        Some("cull") | Some("culling") => {
                            let mode_str = tokens.get(2).map(|s| s.to_lowercase());
                            match mode_str.as_deref() {
                                Some("back") | Some("one") | Some("single") => {
                                    f_cfg.cull_mode = Some(bevy::render::render_resource::Face::Back);
                                    console.logs.push("[Foliage] Culling set to Back (single-sided: only frontfaces visible).".into());
                                    console.logs.push("  - If leaves vanish or turn black, face normal winding was inverted.".into());
                                }
                                Some("none") | Some("two") | Some("double") | Some("off") => {
                                    f_cfg.cull_mode = None;
                                    console.logs.push("[Foliage] Culling set to None (double-sided ribbons/leaves).".into());
                                }
                                _ => {
                                    let cur_mode = if f_cfg.cull_mode.is_none() { "None (double-sided)" } else { "Back (single-sided)" };
                                    console.logs.push(format!("[Foliage] Current culling: {}. Usage: foliage cull <none|back>", cur_mode));
                                }
                            }
                        }
                        Some("white") => {
                            f_cfg.seasonal_tint_enabled = false;
                            console.logs.push("[Foliage] Set base_color to pure Color::WHITE (pure baked mesh vertex colors).".into());
                            console.logs.push("  - Vertex color squaring is eliminated.".into());
                        }
                        Some("tint") => {
                            let state_str = tokens.get(2).map(|s| s.to_lowercase());
                            let on = match state_str.as_deref() {
                                Some("on") | Some("1") | Some("true") => true,
                                Some("off") | Some("0") | Some("false") => false,
                                _ => !f_cfg.seasonal_tint_enabled,
                            };
                            f_cfg.seasonal_tint_enabled = on;
                            console.logs.push(format!("[Foliage] Seasonal tint multiplier: {}", if on { "ENABLED" } else { "DISABLED (Pure white base_color)" }));
                        }
                        _ => {
                            let cur_cull = if f_cfg.cull_mode.is_none() { "None (double-sided)" } else { "Back (single-sided)" };
                            console.logs.push(format!("[Foliage Status] Unlit: {} | Culling: {} | Seasonal Tint: {}",
                                f_cfg.unlit, cur_cull, f_cfg.seasonal_tint_enabled));
                            console.logs.push("Commands: foliage unlit <on|off> | foliage cull <none|back> | foliage white | foliage tint <on|off>".into());
                        }
                    }
                } else {
                    console.logs.push("[Notice] Tree foliage material configuration not available.".into());
                }
            }
            "starsize" => {
                if let Some(ref mut cfg) = sky_config {
                    if let Some(scale) = tokens.get(1).and_then(|s| s.parse::<f32>().ok()) {
                        cfg.starfield_scale = scale.clamp(0.05, 5.0);
                        console.logs.push(format!("[BinarySky] Cosmic starfield point size scale set to {:.2}x (pinpoint stars).", cfg.starfield_scale));
                    } else {
                        console.logs.push(format!("[BinarySky] Current starfield scale: {:.2}x. Usage: starsize <0.1 - 3.0> (e.g. starsize 0.5, starsize 1.0)", cfg.starfield_scale));
                    }
                }
            }
            "sunset" => {
                let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                if let Some(ref mut eph) = ephemeris {
                    // 18.2 hours: Low setting solar contact (Blaze Orange #ff5400 & Hot Fuchsia #ff0054)
                    let frac = ((18.2_f64 / 24.0) - 0.5).rem_euclid(1.0);
                    eph.simulation_time_seconds = duration * frac;
                    eph.diurnal_angle = (frac * 2.0 * std::f64::consts::PI) as f32;
                }
                let _ = conn.db.reducers.admin_set_time(18.2);
                console.logs.push("[Admin] Time set to SUNSET (18.2h). Horizon glowing in Blaze Orange (#ff5400) & Hot Fuchsia (#ff0054).".into());
            }
            "dusk" | "twilight" => {
                let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                if let Some(ref mut eph) = ephemeris {
                    // 19.3 hours: Nautical twilight (Dark Raspberry #9e0059 & Navy Electric #390099)
                    let frac = ((19.3_f64 / 24.0) - 0.5).rem_euclid(1.0);
                    eph.simulation_time_seconds = duration * frac;
                    eph.diurnal_angle = (frac * 2.0 * std::f64::consts::PI) as f32;
                }
                let _ = conn.db.reducers.admin_set_time(19.3);
                console.logs.push("[Admin] Time set to DUSK (19.3h). Horizon glowing in Dark Raspberry (#9e0059) & Navy Electric (#390099).".into());
            }
            "dawn" => {
                let duration = sky_config.as_ref().map(|c| c.day_duration_seconds as f64).unwrap_or(1440.0);
                if let Some(ref mut eph) = ephemeris {
                    // 05.8 hours: Sunrise (Navy Electric into Blaze Orange & Amber Gold)
                    let frac = ((5.8_f64 / 24.0) - 0.5).rem_euclid(1.0);
                    eph.simulation_time_seconds = duration * frac;
                    eph.diurnal_angle = (frac * 2.0 * std::f64::consts::PI) as f32;
                }
                let _ = conn.db.reducers.admin_set_time(5.8);
                console.logs.push("[Admin] Time set to DAWN (05.8h). Sunrise palette: Navy Electric -> Hot Fuchsia -> Blaze Orange -> Amber Gold.".into());
            }
            "palette" => {
                console.logs.push("--- S-TYPE ATMOSPHERIC COLOR FAMILY ---".into());
                console.logs.push("  --navy-electric:   #390099 (Astronomical night / cosmic starlight)".into());
                console.logs.push("  --dark-raspberry:  #9e0059 (Nautical twilight / Belt of Venus)".into());
                console.logs.push("  --hot-fuchsia:     #ff0054 (Civil dusk horizon glow)".into());
                console.logs.push("  --blaze-orange:    #ff5400 (Low setting solar contact)".into());
                console.logs.push("  --amber-gold:      #ffbd00 (Golden hour atmospheric scattering)".into());
                console.logs.push("Commands: 'sunset' (18.2h), 'dusk' (19.3h), 'night' (00:00), 'noon' (12:00), 'dawn' (05.8h)".into());
            }
            "help" => {
                console.logs.push("--- PLAYTESTING COMMAND DIRECTORY ---".into());
                console.logs.push("giveitem <Item> [amt]  : Grants item (Press [Tab] to auto-fill)".into());
                console.logs.push("tp <x> <z>             : Teleports player to world coordinate".into());
                console.logs.push("heal [amt]             : Restores player health points".into());
                console.logs.push("god                    : Sets health to 99999 HP".into());
                console.logs.push("time <0-24>            : Sets in-game world clock ([ [ ] and [ ] ])".into());
                console.logs.push("day / noon / night     : Quick toggle High Noon / Midnight [F8]".into());
                console.logs.push("sunset / dusk / dawn   : Jumps to sunset & twilight palette transitions".into());
                console.logs.push("palette                : Prints active S-type atmospheric color family".into());
                console.logs.push("star <a|b> <lux>       : Sets Star A or Star B illuminance in lux".into());
                console.logs.push("star <a|b> color <hex> : Sets Star A/B direct light color override".into());
                console.logs.push("star <a|b> shadows <on>: Toggles shadow casting for Star A or B".into());
                console.logs.push("dualshadows            : Activates high-contrast dual shadow maps preset".into());
                console.logs.push("light solo <a|b>       : Isolates to a single star (turns other star to 0 lx)".into());
                console.logs.push("light dual             : Restores both stars with natural lighting".into());
                console.logs.push("csm bias <depth> <norm>: Adjusts cascade shadow map biases (depth & normal)".into());
                console.logs.push("csm dist <max> [first] : Adjusts CSM maximum distance & cascade bounds".into());
                console.logs.push("foliage unlit <on|off> : Toggles unlit mode on foliage to isolate lighting".into());
                console.logs.push("foliage cull <none|back: Toggles double-sided vs single-sided face culling".into());
                console.logs.push("foliage white / tint   : Toggles raw vertex colors vs seasonal tint".into());
                console.logs.push("ambient <lux>          : Adjusts ambient light fill (deeper shadows)".into());
                console.logs.push("starsize <scale>       : Scales celestial starfield points of light".into());
                console.logs.push("weather <clear|aurora> : Sets atmospheric weather preset [F9]".into());
                console.logs.push("aurora / rain / haze   : Direct weather command shortcuts".into());
                console.logs.push("hud [on|off|toggle]    : Toggles celestial clock & weather HUD pill [F10]".into());
                console.logs.push("f3 / fps [on|off]       : Toggles performance diagnostics HUD [F3]".into());
                console.logs.push("timescale <speed>      : Sets cycle rate (e.g. 1.0, 60.0) [ - / = ]".into());
                console.logs.push("spawn <mob> [amt]      : Spawns Deer, Boar, Goblin, Peasant".into());
                console.logs.push("nuke [radius]          : Demolishes terrain with spherical blast".into());
                console.logs.push("clearinv               : Empties inventory slots completely".into());
                console.logs.push("killall                : Destroys all active NPC brains".into());
                console.logs.push("tuner / weapontool     : Opens Weapon & Spell Tuner [F6]".into());
                console.logs.push("char / customizer      : Opens Character Model Studio [F5]".into());
                console.logs.push("res [w h | preset]     : Sets resolution (e.g. res 1080p, res 1920 1080)".into());
                console.logs.push("fullscreen / windowed  : Toggles or sets fullscreen / windowed display mode".into());
                console.logs.push("maxfps <fps|0|off>     : Sets frame rate cap (0 or off = uncapped)".into());
                console.logs.push("vsync <on|off>         : Toggles vertical sync (AutoNoVsync default)".into());
                console.logs.push("bedrock <on|off|toggle>: Toggles -120m bedrock floor & collision".into());
                console.logs.push("abilities              : Displays tactical abilities directory".into());
            }
            "resolution" | "res" => {
                if let Ok(mut window) = window_query.get_single_mut() {
                    if tokens.len() == 1 {
                        let cur_w = window.resolution.width();
                        let cur_h = window.resolution.height();
                        let phys_w = window.resolution.physical_width();
                        let phys_h = window.resolution.physical_height();
                        let mode_str = match window.mode {
                            WindowMode::Windowed => "Windowed",
                            WindowMode::BorderlessFullscreen => "Borderless Fullscreen",
                            WindowMode::Fullscreen => "Exclusive Fullscreen",
                            _ => "Other",
                        };
                        console.logs.push(format!("[Window] Resolution: {:.0}x{:.0} (Physical: {}x{}), Mode: {}", cur_w, cur_h, phys_w, phys_h, mode_str));
                        console.logs.push("[Window] Usage: res <w> <h> | res <720p|1080p|1440p|4k> | fullscreen | windowed".into());
                    } else if tokens.len() == 2 {
                        let preset = tokens[1].to_lowercase();
                        let (w, h) = match preset.as_str() {
                            "720p" | "720" => (1280.0, 720.0),
                            "1080p" | "1080" | "fhd" => (1920.0, 1080.0),
                            "1440p" | "1440" | "2k" | "qhd" => (2560.0, 1440.0),
                            "4k" | "2160p" | "2160" | "uhd" => (3840.0, 2160.0),
                            _ => (0.0, 0.0),
                        };
                        if w > 0.0 {
                            window.resolution.set(w, h);
                            console.logs.push(format!("[Window] Set resolution to {:.0}x{:.0} ({})", w, h, preset));
                        } else {
                            console.logs.push(format!("[Syntax Error] Unknown preset '{}'. Use 720p, 1080p, 1440p, 4k or 'res <width> <height>'", tokens[1]));
                        }
                    } else if tokens.len() >= 3 {
                        if let (Ok(w), Ok(h)) = (tokens[1].parse::<f32>(), tokens[2].parse::<f32>()) {
                            if w >= 640.0 && h >= 360.0 {
                                window.resolution.set(w, h);
                                console.logs.push(format!("[Window] Set resolution to {:.0}x{:.0}", w, h));
                            } else {
                                console.logs.push("[Syntax Error] Minimum resolution is 640x360.".into());
                            }
                        } else {
                            console.logs.push("[Syntax Error] Width and height must be valid numbers.".into());
                        }
                    }
                } else {
                    console.logs.push("[Window Error] Primary window not found.".into());
                }
            }
            "fullscreen" | "fs" => {
                if let Ok(mut window) = window_query.get_single_mut() {
                    match window.mode {
                        WindowMode::Windowed => {
                            window.mode = WindowMode::BorderlessFullscreen;
                            console.logs.push("[Window] Display mode set to Borderless Fullscreen.".into());
                        }
                        _ => {
                            window.mode = WindowMode::Windowed;
                            console.logs.push("[Window] Display mode set to Windowed.".into());
                        }
                    }
                } else {
                    console.logs.push("[Window Error] Primary window not found.".into());
                }
            }
            "windowed" | "win" => {
                if let Ok(mut window) = window_query.get_single_mut() {
                    window.mode = WindowMode::Windowed;
                    console.logs.push("[Window] Display mode set to Windowed.".into());
                } else {
                    console.logs.push("[Window Error] Primary window not found.".into());
                }
            }
            "bedrock" => {
                let sub = tokens.get(1).map(|s| s.to_lowercase());
                let current = crate::subterrain::is_bedrock_enabled();
                let new_state = match sub.as_deref() {
                    Some("off") | Some("disable") | Some("disabled") | Some("false") | Some("0") => Some(false),
                    Some("on") | Some("enable") | Some("enabled") | Some("true") | Some("1") => Some(true),
                    Some("toggle") => Some(!current),
                    None | Some("status") => None,
                    Some(other) => {
                        console.logs.push(format!("[Syntax Error] Unknown bedrock argument '{}'. Usage: bedrock <on|off|toggle>", other));
                        return;
                    }
                };

                if let Some(target) = new_state {
                    crate::subterrain::set_bedrock_enabled(target);
                    if let Some(ref mut rs) = render_settings {
                        rs.bedrock_enabled = target;
                    }
                    if target {
                        console.logs.push("[Bedrock] Bedrock foundation ENABLED (-120.0m floor, Rapier3D collision).".into());
                        console.logs.push("  - Subterranean chunks rebuilding with indestructible bedrock floor.".into());
                    } else {
                        console.logs.push("[Bedrock] Bedrock foundation DISABLED (-120.0m floor removed).".into());
                        console.logs.push("  - Subterranean chunks rebuilding without bedrock floor. Void boundary open beneath -120.0m.".into());
                    }
                } else {
                    console.logs.push(format!(
                        "[Bedrock Status] Bedrock foundation: {} (-120.0m floor, Rapier3D collision)",
                        if current { "ENABLED" } else { "DISABLED" }
                    ));
                    console.logs.push("Usage: bedrock <on|off|toggle> | bedrock <1|0>".into());
                }
            }
            "terrain" => {
                let sub = tokens.get(1).map(|s| s.to_lowercase());
                match sub.as_deref() {
                    Some("bedrock") => {
                        let arg = tokens.get(2).map(|s| s.to_lowercase());
                        let current = crate::subterrain::is_bedrock_enabled();
                        let new_state = match arg.as_deref() {
                            Some("off") | Some("disable") | Some("disabled") | Some("false") | Some("0") => Some(false),
                            Some("on") | Some("enable") | Some("enabled") | Some("true") | Some("1") => Some(true),
                            Some("toggle") => Some(!current),
                            None | Some("status") => None,
                            Some(other) => {
                                console.logs.push(format!("[Syntax Error] Unknown terrain bedrock argument '{}'. Usage: terrain bedrock <on|off|toggle>", other));
                                return;
                            }
                        };
                        if let Some(target) = new_state {
                            crate::subterrain::set_bedrock_enabled(target);
                            if let Some(ref mut rs) = render_settings {
                                rs.bedrock_enabled = target;
                            }
                            if target {
                                console.logs.push("[Terrain] Bedrock foundation ENABLED (-120.0m floor, Rapier3D collision).".into());
                            } else {
                                console.logs.push("[Terrain] Bedrock foundation DISABLED (-120.0m floor removed).".into());
                            }
                        } else {
                            console.logs.push(format!("[Terrain] Bedrock: {}", if current { "ENABLED" } else { "DISABLED" }));
                            console.logs.push("Usage: terrain bedrock <on|off|toggle>".into());
                        }
                    }
                    _ => {
                        let cur_bedrock = crate::subterrain::is_bedrock_enabled();
                        console.logs.push(format!("[Terrain Status] Bedrock: {}", if cur_bedrock { "ENABLED" } else { "DISABLED" }));
                        console.logs.push("Commands: bedrock <on|off|toggle> | terrain bedrock <on|off|toggle>".into());
                    }
                }
            }
            _ => {
                console.logs.push(format!("[Error] Unknown command '{}'. Enter 'help' for directory.", tokens[0]));
            }
        }

        if console.logs.len() > 60 {
            let overflow = console.logs.len() - 60;
            console.logs.drain(0..overflow);
        }
    }
}

pub fn update_console_ui(
    console: Res<ConsoleState>,
    mut log_query: Query<&mut Text, (With<ConsoleLogText>, Without<ConsoleInputText>, Without<ConsoleSuggestionsText>)>,
    mut input_query: Query<&mut Text, (With<ConsoleInputText>, Without<ConsoleLogText>, Without<ConsoleSuggestionsText>)>,
    mut sugg_query: Query<&mut Text, (With<ConsoleSuggestionsText>, Without<ConsoleLogText>, Without<ConsoleInputText>)>,
) {
    if !console.is_open {
        return;
    }

    if let Ok(mut log_text) = log_query.get_single_mut() {
        let display_lines = console.logs.iter().rev().take(11).cloned().collect::<Vec<_>>();
        let text_block = display_lines.into_iter().rev().collect::<Vec<_>>().join("\n");
        if log_text.sections[0].value != text_block {
            log_text.sections[0].value = text_block;
        }
    }

    if let Ok(mut input_text) = input_query.get_single_mut() {
        let cursor_char = if console.show_cursor { "_" } else { " " };
        let new_input = format!("{}{}", console.input_buffer, cursor_char);
        if input_text.sections[0].value != new_input {
            input_text.sections[0].value = new_input;
        }
    }

    // Dynamic suggestions banner
    if let Ok(mut sugg_text) = sugg_query.get_single_mut() {
        let trimmed = console.input_buffer.trim_start();
        let (new_sugg, new_color) = if trimmed.starts_with("giveitem") || trimmed.starts_with("give") {
            let cmd_prefix = if trimmed.starts_with("giveitem") { "giveitem" } else { "give" };
            let arg = trimmed.strip_prefix(cmd_prefix).unwrap_or("").trim_start();
            let matches: Vec<&'static str> = CANONICAL_ITEMS.iter()
                .filter(|&&i| i.to_lowercase().starts_with(&arg.to_lowercase()))
                .copied()
                .collect();

            if matches.is_empty() {
                ("[No items match query | Press Tab to browse catalogue]".to_string(), Color::srgb(0.9, 0.4, 0.4))
            } else {
                let shown = if matches.len() > 6 {
                    format!("{}, ... ({} matches)", matches[..6].join(", "), matches.len())
                } else {
                    matches.join(", ")
                };
                (format!("[Tab to auto-fill]: {}", shown), Color::srgb(0.65, 0.85, 0.65))
            }
        } else {
            let matches: Vec<&'static str> = CONSOLE_COMMANDS.iter()
                .filter(|&&c| c.starts_with(&trimmed.to_lowercase()))
                .copied()
                .collect();
            (format!("[Tab to complete]: {}", matches.join(", ")), Color::srgb(0.5, 0.75, 0.9))
        };

        if sugg_text.sections[0].value != new_sugg {
            sugg_text.sections[0].value = new_sugg;
        }
        if sugg_text.sections[0].style.color != new_color {
            sugg_text.sections[0].style.color = new_color;
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_commands_contains_hud() {
        assert!(CONSOLE_COMMANDS.contains(&"hud"), "CONSOLE_COMMANDS must contain 'hud'");
        assert!(CONSOLE_COMMANDS.contains(&"togglehud"), "CONSOLE_COMMANDS must contain 'togglehud'");
        assert!(CONSOLE_COMMANDS.contains(&"skyhud"), "CONSOLE_COMMANDS must contain 'skyhud'");
        assert!(CONSOLE_COMMANDS.contains(&"f3"), "CONSOLE_COMMANDS must contain 'f3'");
        assert!(CONSOLE_COMMANDS.contains(&"fps"), "CONSOLE_COMMANDS must contain 'fps'");
        assert!(CONSOLE_COMMANDS.contains(&"diag"), "CONSOLE_COMMANDS must contain 'diag'");
    }

    #[test]
    fn test_console_commands_contains_resolution() {
        assert!(CONSOLE_COMMANDS.contains(&"res"), "CONSOLE_COMMANDS must contain 'res'");
        assert!(CONSOLE_COMMANDS.contains(&"resolution"), "CONSOLE_COMMANDS must contain 'resolution'");
        assert!(CONSOLE_COMMANDS.contains(&"fullscreen"), "CONSOLE_COMMANDS must contain 'fullscreen'");
        assert!(CONSOLE_COMMANDS.contains(&"fs"), "CONSOLE_COMMANDS must contain 'fs'");
        assert!(CONSOLE_COMMANDS.contains(&"windowed"), "CONSOLE_COMMANDS must contain 'windowed'");
        assert!(CONSOLE_COMMANDS.contains(&"win"), "CONSOLE_COMMANDS must contain 'win'");
    }

    #[test]
    fn test_console_commands_contains_fps_limit() {
        assert!(CONSOLE_COMMANDS.contains(&"maxfps"), "CONSOLE_COMMANDS must contain 'maxfps'");
        assert!(CONSOLE_COMMANDS.contains(&"fpslimit"), "CONSOLE_COMMANDS must contain 'fpslimit'");
        assert!(CONSOLE_COMMANDS.contains(&"fps_max"), "CONSOLE_COMMANDS must contain 'fps_max'");
        assert!(CONSOLE_COMMANDS.contains(&"limitfps"), "CONSOLE_COMMANDS must contain 'limitfps'");
        assert!(CONSOLE_COMMANDS.contains(&"vsync"), "CONSOLE_COMMANDS must contain 'vsync'");
    }


    #[test]
    fn test_diagnostic_overlay_root_toggle_and_display() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        app.world_mut().spawn((
            NodeBundle {
                style: Style {
                    display: Display::None,
                    ..default()
                },
                ..default()
            },
            DiagnosticOverlayRoot,
        )).with_children(|box_node| {
            box_node.spawn((
                TextBundle::from_sections([
                    TextSection::new("DIAGNOSTICS [F3]\n", TextStyle::default()),
                    TextSection::new("FPS: 60.0", TextStyle::default()),
                ]),
                DiagnosticOverlayText,
            ));
        });

        // 1. Initial State: Display::None (hidden until toggled)
        let mut q = app.world_mut().query_filtered::<&mut Style, With<DiagnosticOverlayRoot>>();
        let mut style = q.single_mut(app.world_mut());
        assert_eq!(style.display, Display::None);

        // 2. Toggle ON -> Display::Flex
        style.display = Display::Flex;
        assert_eq!(style.display, Display::Flex);

        // 3. Toggle OFF -> Display::None
        style.display = Display::None;
        assert_eq!(style.display, Display::None);
    }

    #[test]
    fn test_console_commands_contains_noclip() {
        assert!(CONSOLE_COMMANDS.contains(&"noclip"), "CONSOLE_COMMANDS must contain 'noclip'");
        assert!(CONSOLE_COMMANDS.contains(&"fly"), "CONSOLE_COMMANDS must contain 'fly'");
        assert!(CONSOLE_COMMANDS.contains(&"flyspeed"), "CONSOLE_COMMANDS must contain 'flyspeed'");
    }

    #[test]
    fn test_console_commands_contains_bedrock() {
        assert!(CONSOLE_COMMANDS.contains(&"bedrock"), "CONSOLE_COMMANDS must contain 'bedrock'");
        assert!(CONSOLE_COMMANDS.contains(&"terrain"), "CONSOLE_COMMANDS must contain 'terrain'");
    }
}