// ============================================================================
// File: client/examples/binary_stars.rs
// ============================================================================
// ----------------------------------------------------------------------------
// S-TYPE BINARY STAR SYSTEM WITH DUAL SHADOW-CASTING DIRECTIONAL LIGHTS
// & ASYMMETRIC CASCADED SHADOW MAPPING (CSM) DEMONSTRATION
// ----------------------------------------------------------------------------
// Run with:
//   cargo run -p client --example binary_stars
//
// Key Demonstrations:
// 1. Dual Colored Shadows:
//    - Star A casts warm solar white light (100,000 lx). Its shadows are illuminated
//      by Star B's amber rays, appearing with a distinct AMBER tint.
//    - Star B casts amber dwarf light (35,000 lx). Its shadows are illuminated by
//      Star A's warm white rays, appearing as bright daylight shadows.
// 2. Double Penumbras & Intersecting Umbras:
//    - Where an occluder is lit by both stars at different angles, two separate
//      shadows extend in different directions.
//    - In the intersection where both stars are blocked, a deep double-core umbra forms.
// 3. Asymmetric CSM Performance Profile:
//    - Primary Star: 3 cascades, 160m distance, 15m contact bound.
//    - Secondary Star: 2 cascades, 75m distance, 12m contact bound (33% draw call saving).
//    - Global 4096 x 4096 shadow atlas ensures no texel starvation across all 5 cascades.
// ----------------------------------------------------------------------------

use std::f32::consts::PI;
use bevy::prelude::*;
use bevy::pbr::NotShadowCaster;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use client::binary_stars::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SpacetimeRTS - S-Type Binary Star System (Dual CSM & Colored Shadows)".into(),
                resolution: (1280.0_f32, 720.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        // Insert the modular BinaryStarSystemPlugin
        .add_plugins(BinaryStarSystemPlugin)
        .add_systems(Startup, setup_demonstration_scene)
        .add_systems(
            Update,
            (
                handle_demonstration_inputs,
                update_visual_stellar_disks,
                update_orbit_camera,
                update_diagnostics_ui,
            ),
        )
        .run();
}

// ============================================================================
// 1. SCENE SETUP: GROUND, OCCLUDERS, CAMERA, & UI
// ============================================================================

/// Marker for the central camera rig
#[derive(Component)]
struct OrbitCamera {
    pub radius: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub target: Vec3,
}

/// Visual marker representing Host Star A's disk in the sky
#[derive(Component)]
struct VisualStarADisk;

/// Visual marker representing Companion Star B's disk in the sky
#[derive(Component)]
struct VisualStarBDisk;

/// Marker for rotating floating occluder geometry
#[derive(Component)]
struct RotatingOccluder;

/// Marker for diagnostics text UI
#[derive(Component)]
struct DiagnosticsText;

fn setup_demonstration_scene(
    mut commands: Commands,
    mut config: ResMut<BinaryOrbitConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Configure faster demo day progression (60s day cycle for dynamic visual scrubbing)
    config.day_duration_seconds = 60.0;
    config.time_scale = 1.0;

    // ------------------------------------------------------------------------
    // 1. Ambient Lighting (Low floor so shadows are prominently visible)
    // ------------------------------------------------------------------------
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.08, 0.12, 0.22),
        brightness: 12.0,
    });

    // ------------------------------------------------------------------------
    // 2. High-Contrast Ground Plane
    // ------------------------------------------------------------------------
    // Matte surface with high diffuse reflectivity to highlight colored shadows
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Plane3d::default().mesh().size(160.0, 160.0)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.85, 0.85, 0.88),
                perceptual_roughness: 0.85,
                reflectance: 0.2,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        Name::new("Demonstration Ground Plane"),
    ));

    // ------------------------------------------------------------------------
    // 3. Central Occluder Monolith (Casts Long Dramatic Dual Shadows)
    // ------------------------------------------------------------------------
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(1.4, 9.0, 1.4)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.25, 0.28, 0.32),
                perceptual_roughness: 0.4,
                metallic: 0.1,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 4.5, 0.0),
            ..default()
        },
        Name::new("Central Monolith"),
    ));

    // ------------------------------------------------------------------------
    // 4. Ring of Standing Pillars (Stonehenge Pattern Exhibiting Intersecting Fans)
    // ------------------------------------------------------------------------
    let pillar_count = 8;
    let ring_radius = 12.0;
    for i in 0..pillar_count {
        let angle = (i as f32 / pillar_count as f32) * 2.0 * PI;
        let x = angle.cos() * ring_radius;
        let z = angle.sin() * ring_radius;

        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cylinder::new(0.45, 4.5)),
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.40, 0.42, 0.46),
                    perceptual_roughness: 0.6,
                    ..default()
                }),
                transform: Transform::from_xyz(x, 2.25, z),
                ..default()
            },
            Name::new(format!("Standing Pillar {}", i + 1)),
        ));
    }

    // ------------------------------------------------------------------------
    // 5. Floating Occluder Geometries (Layered Shadows & Double Penumbras)
    // ------------------------------------------------------------------------
    // Floating Torus around Central Monolith
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Torus::new(2.2, 0.35)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.85, 0.70, 0.20),
                metallic: 0.7,
                perceptual_roughness: 0.3,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 6.5, 0.0),
            ..default()
        },
        RotatingOccluder,
        Name::new("Floating Torus"),
    ));

    // Floating Cubes
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(2.2, 2.2, 2.2)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.20, 0.60, 0.85),
                perceptual_roughness: 0.3,
                ..default()
            }),
            transform: Transform::from_xyz(6.0, 3.5, 4.0),
            ..default()
        },
        RotatingOccluder,
        Name::new("Floating Blue Cube"),
    ));

    // Floating Sphere
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(1.6)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.85, 0.25, 0.30),
                perceptual_roughness: 0.25,
                ..default()
            }),
            transform: Transform::from_xyz(-6.0, 4.0, -3.5),
            ..default()
        },
        Name::new("Floating Crimson Sphere"),
    ));

    // ------------------------------------------------------------------------
    // 6. Visual Celestial Marker Disks in the Sky
    // ------------------------------------------------------------------------
    // Star A Visual Orb (Warm White Solar Sphere)
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(4.5)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.98, 0.90),
                emissive: LinearRgba::new(4.0, 3.8, 3.2, 1.0),
                unlit: true,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 100.0, -100.0),
            ..default()
        },
        VisualStarADisk,
        NotShadowCaster,
        Name::new("Visual Star A Solar Disk"),
    ));

    // Star B Visual Orb (Amber Dwarf Sphere)
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(3.2)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.58, 0.24),
                emissive: LinearRgba::new(3.5, 1.8, 0.6, 1.0),
                unlit: true,
                ..default()
            }),
            transform: Transform::from_xyz(50.0, 60.0, -80.0),
            ..default()
        },
        VisualStarBDisk,
        NotShadowCaster,
        Name::new("Visual Star B Amber Disk"),
    ));

    // ------------------------------------------------------------------------
    // 7. Orbiting 3D Camera
    // ------------------------------------------------------------------------
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 14.0, 26.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
            ..default()
        },
        OrbitCamera {
            radius: 28.0,
            pitch: 0.45,
            yaw: 0.0,
            target: Vec3::new(0.0, 3.0, 0.0),
        },
        Name::new("Demonstration Orbit Camera"),
    ));

    // ------------------------------------------------------------------------
    // 8. On-Screen Real-Time Diagnostics HUD
    // ------------------------------------------------------------------------
    commands.spawn((
        TextBundle::from_section(
            "Initializing S-Type Binary CSM Simulation...",
            TextStyle {
                font_size: 15.0,
                color: Color::srgb(0.95, 0.95, 0.95),
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .with_background_color(Color::srgba(0.05, 0.07, 0.12, 0.88)),
        DiagnosticsText,
        Name::new("Diagnostics Overlay HUD"),
    ));
}

// ============================================================================
// 2. INTERACTIVE USER INPUTS & TIME PROGRESSION SCRUBBING
// ============================================================================

fn handle_demonstration_inputs(
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<BinaryOrbitConfig>,
) {
    let day_sec = config.day_duration_seconds as f64;

    // [Space]: Toggle pause / resume
    if keys.just_pressed(KeyCode::Space) {
        if config.time_scale > 0.0 {
            config.time_scale = 0.0;
        } else {
            config.time_scale = 1.0;
        }
    }

    // [F8]: Quick jump between Noon, Sunset, and Midnight
    if keys.just_pressed(KeyCode::F8) {
        let clock = config.clock_time_hours();
        if clock > 6.0 && clock < 17.0 {
            // Jump to Sunset (18:00)
            config.simulation_time_seconds = day_sec * 0.25;
        } else if clock >= 17.0 && clock < 22.0 {
            // Jump to Deep Night (00:00)
            config.simulation_time_seconds = day_sec * 0.50;
        } else {
            // Jump to High Noon (12:00)
            config.simulation_time_seconds = 0.0;
        }
    }

    // [ [ ]: Step 1 hour backward
    if keys.just_pressed(KeyCode::BracketLeft) {
        let hour_sec = day_sec / 24.0;
        config.simulation_time_seconds = (config.simulation_time_seconds - hour_sec).rem_euclid(day_sec * config.binary_orbital_period_days as f64);
    }

    // [ ] ]: Step 1 hour forward
    if keys.just_pressed(KeyCode::BracketRight) {
        let hour_sec = day_sec / 24.0;
        config.simulation_time_seconds = (config.simulation_time_seconds + hour_sec).rem_euclid(day_sec * config.binary_orbital_period_days as f64);
    }

    // [ - ]: Slow down time
    if keys.just_pressed(KeyCode::Minus) {
        config.time_scale = match config.time_scale {
            s if s >= 60.0 => 20.0,
            s if s >= 20.0 => 5.0,
            s if s >= 5.0 => 1.0,
            _ => 0.0,
        };
    }

    // [ = ]: Accelerate time
    if keys.just_pressed(KeyCode::Equal) {
        config.time_scale = match config.time_scale {
            s if s < 1.0 => 1.0,
            s if s < 5.0 => 5.0,
            s if s < 20.0 => 20.0,
            _ => 60.0,
        };
    }

    // [1]: Toggle Star A shadows
    if keys.just_pressed(KeyCode::Digit1) {
        config.star_a_shadows_enabled = !config.star_a_shadows_enabled;
    }

    // [2]: Toggle Star B shadows
    if keys.just_pressed(KeyCode::Digit2) {
        config.star_b_shadows_enabled = !config.star_b_shadows_enabled;
    }
}

// ============================================================================
// 3. VISUAL DISK TRACKING & OCCLUDER ROTATION
// ============================================================================

fn update_visual_stellar_disks(
    time: Res<Time>,
    star_a_query: Query<&StellarOrbitalData, (With<PrimaryStar>, Without<SecondaryStar>)>,
    star_b_query: Query<&StellarOrbitalData, (With<SecondaryStar>, Without<PrimaryStar>)>,
    mut disk_a_query: Query<(&mut Transform, &mut Visibility), (With<VisualStarADisk>, Without<VisualStarBDisk>)>,
    mut disk_b_query: Query<(&mut Transform, &mut Visibility), (With<VisualStarBDisk>, Without<VisualStarADisk>)>,
    mut rotating_query: Query<&mut Transform, (With<RotatingOccluder>, Without<VisualStarADisk>, Without<VisualStarBDisk>)>,
) {
    let distance = 95.0;

    if let Ok(data_a) = star_a_query.get_single() {
        if let Ok((mut tf_a, mut vis_a)) = disk_a_query.get_single_mut() {
            if data_a.is_above_horizon {
                *vis_a = Visibility::Visible;
                tf_a.translation = data_a.direction * distance;
            } else {
                *vis_a = Visibility::Hidden;
            }
        }
    }

    if let Ok(data_b) = star_b_query.get_single() {
        if let Ok((mut tf_b, mut vis_b)) = disk_b_query.get_single_mut() {
            if data_b.is_above_horizon {
                *vis_b = Visibility::Visible;
                tf_b.translation = data_b.direction * distance;
            } else {
                *vis_b = Visibility::Hidden;
            }
        }
    }

    // Rotate floating geometry slowly to showcase moving shadow penumbras
    let dt = time.delta_seconds();
    for mut tf in &mut rotating_query {
        tf.rotate_y(0.25 * dt);
    }
}

// ============================================================================
// 4. ORBIT CAMERA CONTROLLER (PAN, TILT, ZOOM)
// ============================================================================

fn update_orbit_camera(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut mouse_wheel: EventReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<(&mut Transform, &mut OrbitCamera)>,
) {
    let Ok((mut transform, mut orbit)) = camera_query.get_single_mut() else { return; };

    // Mouse drag to orbit
    if mouse_buttons.pressed(MouseButton::Right) || mouse_buttons.pressed(MouseButton::Left) {
        for ev in mouse_motion.read() {
            orbit.yaw -= ev.delta.x * 0.005;
            orbit.pitch = (orbit.pitch + ev.delta.y * 0.005).clamp(0.05, PI * 0.48);
        }
    } else {
        mouse_motion.clear();
    }

    // Mouse wheel to zoom
    for ev in mouse_wheel.read() {
        orbit.radius = (orbit.radius - ev.y * 2.0).clamp(6.0, 70.0);
    }

    // Keyboard pan target
    let mut pan_dir = Vec3::ZERO;
    let forward = Vec3::new(orbit.yaw.sin(), 0.0, orbit.yaw.cos()).normalize();
    let right = Vec3::new(forward.z, 0.0, -forward.x);

    if keys.pressed(KeyCode::KeyW) { pan_dir -= forward; }
    if keys.pressed(KeyCode::KeyS) { pan_dir += forward; }
    if keys.pressed(KeyCode::KeyA) { pan_dir -= right; }
    if keys.pressed(KeyCode::KeyD) { pan_dir += right; }

    orbit.target += pan_dir * 12.0 * time.delta_seconds();

    // Recompute camera position
    let cos_p = orbit.pitch.cos();
    let sin_p = orbit.pitch.sin();
    let cos_y = orbit.yaw.cos();
    let sin_y = orbit.yaw.sin();

    let offset = Vec3::new(orbit.radius * cos_p * sin_y, orbit.radius * sin_p, orbit.radius * cos_p * cos_y);
    transform.translation = orbit.target + offset;
    transform.look_at(orbit.target, Vec3::Y);
}

// ============================================================================
// 5. REAL-TIME DIAGNOSTICS HUD UPDATE
// ============================================================================

fn update_diagnostics_ui(
    config: Res<BinaryOrbitConfig>,
    star_a_query: Query<&StellarOrbitalData, (With<PrimaryStar>, Without<SecondaryStar>)>,
    star_b_query: Query<&StellarOrbitalData, (With<SecondaryStar>, Without<PrimaryStar>)>,
    mut text_query: Query<&mut Text, With<DiagnosticsText>>,
) {
    let Ok(mut text) = text_query.get_single_mut() else { return; };

    let data_a = star_a_query.get_single().ok();
    let data_b = star_b_query.get_single().ok();

    let clock = config.clock_time_hours();
    let hours = clock.floor() as u32;
    let minutes = ((clock - clock.floor()) * 60.0) as u32;

    let a_elev = data_a.map(|d| d.elevation_radians.to_degrees()).unwrap_or(0.0);
    let a_lux = data_a.map(|d| d.current_illuminance_lux).unwrap_or(0.0);
    let a_shadow = data_a.map(|d| d.is_casting_shadows).unwrap_or(false);

    let b_elev = data_b.map(|d| d.elevation_radians.to_degrees()).unwrap_or(0.0);
    let b_lux = data_b.map(|d| d.current_illuminance_lux).unwrap_or(0.0);
    let b_shadow = data_b.map(|d| d.is_casting_shadows).unwrap_or(false);

    let illumination_state = if a_shadow && b_shadow {
        "DUAL SHADOWS ACTIVE (White + Amber Double Penumbra)"
    } else if a_shadow {
        "Host Star A Shadow Active (Primary Daylight)"
    } else if b_shadow {
        "Companion Star B Shadow Active (Amber Night Silhouette)"
    } else {
        "Deep Night (Directional Shadows Inactive)"
    };

    let summary = format!(
        "=== S-Type Binary Star System (Dual CSM Demonstration) ===\n\
        Time of Day: {:02}:{:02} (Day Duration: {:.0}s | Time Scale: {:.0}x)\n\
        State: {}\n\n\
        [Primary Host Star A - High Fidelity Tier (3 Cascades | 160m)]\n\
        Elevation: {:+5.1}° | Illuminance: {:6.0} lx | Shadows: {}\n\n\
        [Companion Star B - Asymmetric Tier (2 Cascades | 75m -33% Passes)]\n\
        Elevation: {:+5.1}° | Illuminance: {:6.0} lx | Shadows: {}\n\n\
        Controls:\n\
        [Space] Pause/Resume | [F8] Jump Noon/Sunset/Night\n\
        [ [ ] / [ ] ] Step -1h / +1h | [-] / [=] Speed Down / Up\n\
        [1] Toggle Star A Shadows   | [2] Toggle Star B Shadows\n\
        Mouse Drag: Orbit Camera     | Scroll: Zoom",
        hours, minutes, config.day_duration_seconds, config.time_scale,
        illumination_state,
        a_elev, a_lux, if a_shadow { "ACTIVE" } else { "OFF" },
        b_elev, b_lux, if b_shadow { "ACTIVE" } else { "OFF" },
    );

    text.sections[0].value = summary;
}
