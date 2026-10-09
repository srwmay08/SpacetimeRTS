// ============================================================================
// File: client/src/binary_sky/lighting.rs
// ============================================================================
// ----------------------------------------------------------------------------
// BEVY ECS LIGHTING, SHADOW CASCADES, AMBIENT & FOG SYNCHRONIZATION
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use bevy::pbr::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, FogFalloff, FogSettings,
    NotShadowCaster,
};
use bevy::render::view::RenderLayers;

use super::ephemeris::*;
use super::aurora::*;
use super::meshes::*;

/// Tag component marking the entity containing Star A's `DirectionalLightBundle`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[reflect(Component)]
pub struct PrimaryStar;

/// Tag component marking the entity containing Star B's `DirectionalLightBundle`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[reflect(Component)]
pub struct SecondaryStar;

/// Tag component marking the 3D camera receiving atmospheric sky and fog updates.
#[derive(Component, Debug, Default)]
pub struct AtmosphericCamera;

/// Component tag for Host Star A's visual disc and corona sphere.
#[derive(Component, Debug, Default)]
pub struct StarAVolumetricDisk;

/// Component tag for Companion Star B's visual dwarf disc and corona sphere.
#[derive(Component, Debug, Default)]
pub struct StarBVolumetricDisk;

/// Component tag for the procedural cosmic background starfield dome.
#[derive(Component, Debug, Default)]
pub struct CosmicStarfield;

pub fn setup_binary_sky_environment(
    mut commands: Commands,
    config: Res<BinarySkyConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut aurora_materials: ResMut<Assets<StarAuroraDomeMaterial>>,
) {
    info!("Initializing Dynamic Binary Sky System (S-Type Circumstellar Architecture)...");

    // Spawn Host Primary Star A Directional Light (Dominant Shadow Caster - Higher Fidelity CSM)
    let cascade_config_a = CascadeShadowConfigBuilder {
        num_cascades: config.star_a_num_cascades,
        minimum_distance: config.star_a_minimum_shadow_distance,
        maximum_distance: config.star_a_maximum_shadow_distance,
        first_cascade_far_bound: config.star_a_first_cascade_far_bound,
        overlap_proportion: 0.20,
    }
    .build();

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: Color::srgb(1.0, 0.97, 0.92),
                illuminance: config.star_a_base_illuminance_lux,
                shadows_enabled: true,
                shadow_depth_bias: config.star_a_shadow_depth_bias,
                shadow_normal_bias: config.star_a_shadow_normal_bias,
                ..default()
            },
            transform: Transform::from_xyz(0.0, 100.0, -100.0).looking_at(Vec3::ZERO, Vec3::Y),
            cascade_shadow_config: cascade_config_a,
            ..default()
        },
        PrimaryStar,
        RenderLayers::from_layers(&[0, 2]),
        Name::new("Host Star A (Primary)"),
    ));

    // Spawn Secondary Dwarf Star B Directional Light (Asymmetric CSM Tier: 2 cascades, 112.5m)
    let cascade_config_b = CascadeShadowConfigBuilder {
        num_cascades: config.star_b_num_cascades,
        minimum_distance: config.star_b_minimum_shadow_distance,
        maximum_distance: config.star_b_maximum_shadow_distance,
        first_cascade_far_bound: config.star_b_first_cascade_far_bound,
        overlap_proportion: 0.20,
    }
    .build();

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: Color::srgb(1.0, 0.65, 0.35),
                illuminance: config.star_b_base_illuminance_lux,
                shadows_enabled: true,
                shadow_depth_bias: config.star_b_shadow_depth_bias,
                shadow_normal_bias: config.star_b_shadow_normal_bias,
                ..default()
            },
            transform: Transform::from_xyz(50.0, 60.0, -80.0).looking_at(Vec3::ZERO, Vec3::Y),
            cascade_shadow_config: cascade_config_b,
            ..default()
        },
        SecondaryStar,
        RenderLayers::from_layers(&[0, 2]),
        Name::new("Companion Star B (Secondary)"),
    ));

    // Spawn Host Star A Visual Volumetric Disk
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(18.0)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.98, 0.90),
                emissive: LinearRgba::new(4.0, 3.8, 3.2, 1.0),
                unlit: true,
                fog_enabled: false,
                cull_mode: None,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 300.0, -300.0),
            ..default()
        },
        StarAVolumetricDisk,
        NotShadowCaster,
        RenderLayers::from_layers(&[0, 1, 2]),
        Name::new("Host Star A Volumetric Disk"),
    ));

    // Spawn Companion Star B Visual Dwarf Disk
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(12.0)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.65, 0.30),
                emissive: LinearRgba::new(3.5, 2.0, 0.8, 1.0),
                unlit: true,
                fog_enabled: false,
                cull_mode: None,
                ..default()
            }),
            transform: Transform::from_xyz(100.0, 200.0, -250.0),
            ..default()
        },
        StarBVolumetricDisk,
        NotShadowCaster,
        RenderLayers::from_layers(&[0, 1, 2]),
        Name::new("Companion Star B Dwarf Disk"),
    ));

    // Spawn Cosmic Background Starfield Dome (6,000 diamond stars with astrometric magnitudes)
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(create_billboard_starfield_mesh(6000)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 1.0, 1.0, 0.0),
                unlit: true,
                fog_enabled: false,
                cull_mode: None,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        CosmicStarfield,
        NotShadowCaster,
        RenderLayers::from_layers(&[0, 1, 2]),
        Name::new("Cosmic Starfield Dome"),
    ));

    // Spawn Procedural Planetary Aurora Sky Dome
    commands.spawn((
        MaterialMeshBundle {
            mesh: meshes.add(create_sky_dome_mesh()),
            material: aurora_materials.add(StarAuroraDomeMaterial {
                uniforms: SkyUniforms {
                    night_factor: 0.0,
                    weather_intensity: 0.0,
                    speed: 1.0,
                    brightness: 1.0,
                },
            }),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            visibility: Visibility::Hidden,
            ..default()
        },
        StarAuroraDome,
        AtmosphericSkyDome,
        NotShadowCaster,
        RenderLayers::from_layers(&[0, 1, 2]),
        Name::new("Planetary Aurora Sky Dome"),
    ));

    // Spawn Volumetric Precipitation Streaks (Stormy Rain Weather System)
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(create_precipitation_mesh(450)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgba(0.85, 0.92, 1.0, 0.55),
                unlit: true,
                fog_enabled: false,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            visibility: Visibility::Hidden,
            ..default()
        },
        PrecipitationStreaks,
        NotShadowCaster,
        RenderLayers::from_layers(&[0, 1, 2]),
        Name::new("Volumetric Rain Streaks"),
    ));
}

/// Computes the progressive dimming factor for subterranean depths:
/// 0.0 at surface height or above, smoothly scaling to 1.0 as the camera
/// descends 15 meters below the surface terrain crust into deep caverns.
pub fn calculate_subterranean_dimming_factor(cam_pos: Vec3) -> f32 {
    let surface_h = crate::terrain::get_terrain_height(cam_pos.x, cam_pos.z);
    let depth = surface_h - cam_pos.y;
    ((depth - 1.0) / 14.0).clamp(0.0, 1.0)
}

/// Synchronizes Bevy's `DirectionalLight` components and `AmbientLight` resource.
pub fn sync_stellar_directional_lights(
    config: Res<BinarySkyConfig>,
    ephemeris: Res<BinaryEphemerisState>,
    cache: Res<AtmosphericRadianceCache>,
    mut ambient_light: ResMut<AmbientLight>,
    mut star_a_query: Query<(&mut DirectionalLight, &mut Transform, &mut CascadeShadowConfig), (With<PrimaryStar>, Without<SecondaryStar>)>,
    mut star_b_query: Query<(&mut DirectionalLight, &mut Transform, &mut CascadeShadowConfig), (With<SecondaryStar>, Without<PrimaryStar>)>,
    camera_query: Query<(Option<&GlobalTransform>, Option<&Transform>, Option<&Camera>), (With<AtmosphericCamera>, Without<PrimaryStar>, Without<SecondaryStar>)>,
) {
    let resolve_pos = |gt: Option<&GlobalTransform>, lt: Option<&Transform>| -> Option<Vec3> {
        match (gt, lt) {
            (Some(g), Some(l)) if g.translation() == Vec3::ZERO && l.translation != Vec3::ZERO => Some(l.translation),
            (Some(g), _) => Some(g.translation()),
            (None, Some(l)) => Some(l.translation),
            (None, None) => None,
        }
    };

    let cam_pos = camera_query
        .iter()
        .find(|(_, _, cam)| cam.map_or(false, |c| c.is_active))
        .and_then(|(gt, lt, _)| resolve_pos(gt, lt))
        .or_else(|| {
            camera_query
                .iter()
                .find_map(|(gt, lt, _)| resolve_pos(gt, lt))
        });

    let underground_factor = cam_pos
        .map(calculate_subterranean_dimming_factor)
        .unwrap_or(0.0);

    let surface_ambient_brightness = config.ambient_illuminance_lux.unwrap_or(cache.ambient_brightness_lux);
    const CAVERN_AMBIENT_FLOOR_LUX: f32 = 12.0;
    ambient_light.brightness = surface_ambient_brightness * (1.0 - underground_factor) + CAVERN_AMBIENT_FLOOR_LUX * underground_factor;

    let surface_ambient_color = cache.ambient_color;
    let cavern_ambient_color = Color::srgb(0.03, 0.04, 0.06);
    let s_surf = surface_ambient_color.to_srgba();
    let s_cav = cavern_ambient_color.to_srgba();
    ambient_light.color = Color::srgb(
        s_surf.red * (1.0 - underground_factor) + s_cav.red * underground_factor,
        s_surf.green * (1.0 - underground_factor) + s_cav.green * underground_factor,
        s_surf.blue * (1.0 - underground_factor) + s_cav.blue * underground_factor,
    );

    let light_attenuation = (1.0 - underground_factor).powi(2);

    if let Ok((mut light_a, mut transform_a, mut cascade_a)) = star_a_query.get_single_mut() {
        light_a.color = config.star_a_color_override.unwrap_or(cache.star_a_color);
        let base_lux = if config.star_a_enabled { cache.star_a_illuminance_lux } else { 0.0 };
        light_a.illuminance = base_lux * light_attenuation;
        light_a.shadows_enabled = config.star_a_enabled
            && config.star_a_shadows_enabled
            && ephemeris.star_a_elevation > -0.05
            && underground_factor < 0.99;
        light_a.shadow_depth_bias = config.star_a_shadow_depth_bias;
        light_a.shadow_normal_bias = config.star_a_shadow_normal_bias;

        if cascade_a.minimum_distance != config.star_a_minimum_shadow_distance
            || cascade_a.bounds.last().copied() != Some(config.star_a_maximum_shadow_distance)
            || cascade_a.bounds.first().copied() != Some(config.star_a_first_cascade_far_bound)
        {
            *cascade_a = CascadeShadowConfigBuilder {
                num_cascades: config.star_a_num_cascades,
                minimum_distance: config.star_a_minimum_shadow_distance,
                maximum_distance: config.star_a_maximum_shadow_distance,
                first_cascade_far_bound: config.star_a_first_cascade_far_bound,
                overlap_proportion: 0.20,
            }
            .build();
        }

        let target_dir = -ephemeris.star_a_direction;
        if target_dir.length_squared() > 1e-4 {
            *transform_a = Transform::from_translation(Vec3::ZERO).looking_to(target_dir, Vec3::Y);
        }
    }

    if let Ok((mut light_b, mut transform_b, mut cascade_b)) = star_b_query.get_single_mut() {
        light_b.color = config.star_b_color_override.unwrap_or(cache.star_b_color);
        let base_lux = if config.star_b_enabled { cache.star_b_illuminance_lux } else { 0.0 };
        light_b.illuminance = base_lux * light_attenuation;
        light_b.shadows_enabled = config.star_b_enabled
            && config.star_b_shadows_enabled
            && ephemeris.star_b_elevation > -0.05
            && underground_factor < 0.99;
        light_b.shadow_depth_bias = config.star_b_shadow_depth_bias;
        light_b.shadow_normal_bias = config.star_b_shadow_normal_bias;

        if cascade_b.minimum_distance != config.star_b_minimum_shadow_distance
            || cascade_b.bounds.last().copied() != Some(config.star_b_maximum_shadow_distance)
            || cascade_b.bounds.first().copied() != Some(config.star_b_first_cascade_far_bound)
        {
            *cascade_b = CascadeShadowConfigBuilder {
                num_cascades: config.star_b_num_cascades,
                minimum_distance: config.star_b_minimum_shadow_distance,
                maximum_distance: config.star_b_maximum_shadow_distance,
                first_cascade_far_bound: config.star_b_first_cascade_far_bound,
                overlap_proportion: 0.20,
            }
            .build();
        }

        let target_dir = -ephemeris.star_b_direction;
        if target_dir.length_squared() > 1e-4 {
            *transform_b = Transform::from_translation(Vec3::ZERO).looking_to(target_dir, Vec3::Y);
        }
    }
}

/// Synchronizes visual celestial disc meshes, background cosmic starfield, procedural aurora sky dome,
/// and precipitation streaks relative to the active camera.
pub fn sync_celestial_visuals(
    time: Res<Time>,
    config: Res<BinarySkyConfig>,
    ephemeris: Res<BinaryEphemerisState>,
    cache: Res<AtmosphericRadianceCache>,
    weather: Res<AtmosphericWeather>,
    mut celestial_set: ParamSet<(
        Query<(&GlobalTransform, &Camera), With<Camera3d>>,
        Query<(&mut Transform, &mut Visibility, &Handle<StandardMaterial>), With<StarAVolumetricDisk>>,
        Query<(&mut Transform, &mut Visibility, &Handle<StandardMaterial>), With<StarBVolumetricDisk>>,
        Query<(&mut Transform, &mut Visibility, &Handle<StandardMaterial>), With<CosmicStarfield>>,
        Query<(&mut Transform, &mut Visibility, Option<&Handle<StarAuroraDomeMaterial>>), With<StarAuroraDome>>,
        Query<(&mut Transform, &mut Visibility), With<PrecipitationStreaks>>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut aurora_materials: Option<ResMut<Assets<StarAuroraDomeMaterial>>>,
) {
    let cam_pos = if let Some((cam_tf, _)) = celestial_set.p0().iter().find(|(_, cam)| cam.is_active) {
        cam_tf.translation()
    } else if let Some((cam_tf, _)) = celestial_set.p0().iter().next() {
        cam_tf.translation()
    } else {
        return;
    };

    let is_deep_underground = calculate_subterranean_dimming_factor(cam_pos) >= 0.99;

    // 1. Sync Host Star A Disk
    if let Ok((mut tf_a, mut vis_a, mat_handle_a)) = celestial_set.p1().get_single_mut() {
        if ephemeris.star_a_elevation < -0.10 || is_deep_underground {
            *vis_a = Visibility::Hidden;
        } else {
            *vis_a = Visibility::Visible;
            tf_a.translation = cam_pos + ephemeris.star_a_direction * 410.0;
            tf_a.look_at(cam_pos, Vec3::Y);
            if let Some(mat) = materials.get_mut(mat_handle_a) {
                let chrom = config.star_a_color_override.unwrap_or_else(|| {
                    let trans = cache.star_a_transmittance;
                    let blackbody = planck_blackbody_rgb(config.star_a_temperature_kelvin);
                    let filtered = blackbody * trans;
                    let max_c = filtered.x.max(filtered.y).max(filtered.z).max(1e-4);
                    let norm = filtered / max_c;
                    Color::srgb(norm.x.clamp(0.0, 1.0), norm.y.clamp(0.0, 1.0), norm.z.clamp(0.0, 1.0))
                });
                mat.base_color = chrom;
                let srgba = chrom.to_srgba();
                mat.emissive = LinearRgba::new(srgba.red * 2.5, srgba.green * 2.5, srgba.blue * 2.5, 1.0);
            }
        }
    }

    // 2. Sync Companion Star B Disk
    if let Ok((mut tf_b, mut vis_b, mat_handle_b)) = celestial_set.p2().get_single_mut() {
        if ephemeris.star_b_elevation < -0.10 || is_deep_underground {
            *vis_b = Visibility::Hidden;
        } else {
            *vis_b = Visibility::Visible;
            tf_b.translation = cam_pos + ephemeris.star_b_direction * 410.0;
            tf_b.look_at(cam_pos, Vec3::Y);
            if let Some(mat) = materials.get_mut(mat_handle_b) {
                let chrom = config.star_b_color_override.unwrap_or_else(|| {
                    let trans = cache.star_b_transmittance;
                    let blackbody = planck_blackbody_rgb(config.star_b_temperature_kelvin);
                    let filtered = blackbody * trans;
                    let max_c = filtered.x.max(filtered.y).max(filtered.z).max(1e-4);
                    let norm = filtered / max_c;
                    Color::srgb(norm.x.clamp(0.0, 1.0), norm.y.clamp(0.0, 1.0), norm.z.clamp(0.0, 1.0))
                });
                mat.base_color = chrom;
                let srgba = chrom.to_srgba();
                mat.emissive = LinearRgba::new(srgba.red * 2.5, srgba.green * 2.5, srgba.blue * 2.5, 1.0);
            }
        }
    }

    // 3. Sync Cosmic Starfield
    let total_sun_lux = cache.star_a_illuminance_lux + cache.star_b_illuminance_lux;
    let night_factor = (1.0 - (total_sun_lux / 16000.0).clamp(0.0, 1.0)).powi(2);

    if let Ok((mut tf_stars, mut vis_stars, mat_handle_stars)) = celestial_set.p3().get_single_mut() {
        tf_stars.translation = cam_pos;
        tf_stars.scale = Vec3::splat(config.starfield_scale);
        if night_factor <= 0.02 || is_deep_underground {
            *vis_stars = Visibility::Hidden;
        } else {
            *vis_stars = Visibility::Visible;
            if let Some(mat) = materials.get_mut(mat_handle_stars) {
                let t = time.elapsed_seconds();
                let twinkle = 1.0
                    + (t * 2.8).sin() * 0.28
                    + (t * 5.4 + 1.2).sin() * 0.18
                    + (t * 9.7 + 2.5).cos() * 0.12;
                let star_luminance = night_factor * 7.5 * twinkle.max(0.15);
                let alpha_twinkle = (night_factor * (0.75 + twinkle * 0.25)).clamp(0.0, 1.0);
                mat.base_color = Color::srgba(1.0, 1.0, 1.0, alpha_twinkle);
                mat.emissive = LinearRgba::new(star_luminance, star_luminance, star_luminance, 1.0);
            }
        }
    }

    // 4. Sync Procedural Aurora Sky Dome
    let mut p4 = celestial_set.p4();
    for (mut tf_aurora, mut vis_aurora, maybe_mat_handle) in p4.iter_mut() {
        tf_aurora.translation = cam_pos;
        tf_aurora.rotation = Quat::IDENTITY;

        let is_aurora_weather = weather.weather_type == WeatherType::StellarWindAurora;
        let is_active = is_aurora_weather && night_factor > 0.02 && !is_deep_underground;

        if is_active {
            *vis_aurora = Visibility::Visible;
        } else {
            *vis_aurora = Visibility::Hidden;
        }

        if let (Some(mat_handle), Some(ref mut mats)) = (maybe_mat_handle, aurora_materials.as_mut()) {
            if let Some(mat) = mats.get_mut(mat_handle) {
                mat.uniforms.night_factor = night_factor;
                mat.uniforms.weather_intensity = if is_aurora_weather { weather.aurora_intensity } else { 0.0 };
                mat.uniforms.speed = 1.0;
                mat.uniforms.brightness = 1.0;
            }
        }
    }

    // 5. Sync Volumetric Precipitation Streaks
    let mut p5 = celestial_set.p5();
    for (mut tf_rain, mut vis_rain) in p5.iter_mut() {
        if weather.weather_type == WeatherType::OvercastPrecipitation && !is_deep_underground {
            *vis_rain = Visibility::Visible;
            let fall_offset = (time.elapsed_seconds() * 28.0).rem_euclid(4.0);
            tf_rain.translation = cam_pos + Vec3::new(0.0, -fall_offset, 0.0);
        } else {
            *vis_rain = Visibility::Hidden;
        }
    }
}

/// Updates camera clear color and volumetric atmospheric distance fog.
pub fn update_atmospheric_cameras_and_fog(
    _ephemeris: Res<BinaryEphemerisState>,
    cache: Res<AtmosphericRadianceCache>,
    weather: Res<AtmosphericWeather>,
    render_settings: Option<Res<crate::spellbook::TerrainRenderSettings>>,
    mut commands: Commands,
    mut camera_query: Query<(Entity, Option<&GlobalTransform>, Option<&Transform>, Option<&Camera>, Option<&mut FogSettings>), With<AtmosphericCamera>>,
    mut clear_color: ResMut<ClearColor>,
) {
    let horizon_rgb = tonemap_atmospheric_radiance(cache.horizon_radiance);
    let mut horizon_color = Color::srgb(
        horizon_rgb.x.clamp(0.0, 1.0),
        horizon_rgb.y.clamp(0.0, 1.0),
        horizon_rgb.z.clamp(0.0, 1.0),
    );

    if weather.weather_type == WeatherType::OvercastPrecipitation {
        let srgba = horizon_color.to_srgba();
        horizon_color = Color::srgb(
            srgba.red * 0.4 + 0.25 * 0.6,
            srgba.green * 0.4 + 0.28 * 0.6,
            srgba.blue * 0.4 + 0.32 * 0.6,
        );
    }

    let resolve_pos = |gt: Option<&GlobalTransform>, lt: Option<&Transform>| -> Option<Vec3> {
        match (gt, lt) {
            (Some(g), Some(l)) if g.translation() == Vec3::ZERO && l.translation != Vec3::ZERO => Some(l.translation),
            (Some(g), _) => Some(g.translation()),
            (None, Some(l)) => Some(l.translation),
            (None, None) => None,
        }
    };

    let active_cam_pos = camera_query
        .iter()
        .find(|(_, _, _, cam, _)| cam.map_or(false, |c| c.is_active))
        .and_then(|(_, gt, lt, _, _)| resolve_pos(gt, lt))
        .or_else(|| {
            camera_query
                .iter()
                .find_map(|(_, gt, lt, _, _)| resolve_pos(gt, lt))
        });

    let underground_factor = active_cam_pos
        .map(calculate_subterranean_dimming_factor)
        .unwrap_or(0.0);

    let cavern_sky_color = Color::srgb(0.015, 0.015, 0.022);
    let h_srgba = horizon_color.to_srgba();
    let c_srgba = cavern_sky_color.to_srgba();
    let effective_horizon_color = Color::srgb(
        h_srgba.red * (1.0 - underground_factor) + c_srgba.red * underground_factor,
        h_srgba.green * (1.0 - underground_factor) + c_srgba.green * underground_factor,
        h_srgba.blue * (1.0 - underground_factor) + c_srgba.blue * underground_factor,
    );

    clear_color.0 = effective_horizon_color;

    let base_range = if let Some(ref rs) = render_settings {
        if rs.spawn_full_zone { 64.0 * 16.0 } else { rs.visible_range_meters.max(160.0) }
    } else {
        230.4
    };

    let (start_dist, end_dist) = match weather.weather_type {
        WeatherType::ClearSky => ((base_range * 0.20).max(40.0), base_range * 1.0),
        WeatherType::StellarWindAurora => ((base_range * 0.18).max(35.0), base_range * 0.95),
        WeatherType::AerosolHaze => ((base_range * 0.10).max(20.0), base_range * 0.75),
        WeatherType::OvercastPrecipitation => ((base_range * 0.05).max(10.0), base_range * 0.60),
    };

    let effective_start = start_dist * (1.0 - underground_factor) + 8.0 * underground_factor;
    let effective_end = end_dist * (1.0 - underground_factor) + 55.0 * underground_factor;

    let weight_a = (cache.star_a_illuminance_lux / 60.0).clamp(0.0, 1.0);
    let weight_b = (cache.star_b_illuminance_lux / 60.0).clamp(0.0, 1.0);
    let sun_scatter_color = if weight_a > 0.001 || weight_b > 0.001 {
        let col_a = cache.star_a_color.to_srgba();
        let col_b = cache.star_b_color.to_srgba();
        let total_w = (weight_a + weight_b).max(1e-4);
        let r = (col_a.red * weight_a + col_b.red * weight_b) / total_w;
        let g = (col_a.green * weight_a + col_b.green * weight_b) / total_w;
        let b = (col_a.blue * weight_a + col_b.blue * weight_b) / total_w;
        let alpha = weight_a.max(weight_b);
        Color::srgba(r * alpha, g * alpha, b * alpha, alpha)
    } else {
        Color::NONE
    };

    let final_sun_scatter = if underground_factor >= 0.99 {
        Color::NONE
    } else {
        let sc = sun_scatter_color.to_srgba();
        let att = 1.0 - underground_factor;
        Color::srgba(sc.red * att, sc.green * att, sc.blue * att, sc.alpha * att)
    };

    for (entity, _, _, _, mut fog_opt) in camera_query.iter_mut() {
        if let Some(ref mut fog) = fog_opt {
            fog.color = effective_horizon_color;
            fog.directional_light_color = final_sun_scatter;
            fog.directional_light_exponent = 8.0;
            fog.falloff = FogFalloff::Linear {
                start: effective_start,
                end: effective_end,
            };
        } else {
            commands.entity(entity).insert(FogSettings {
                color: effective_horizon_color,
                directional_light_color: final_sun_scatter,
                directional_light_exponent: 8.0,
                falloff: FogFalloff::Linear {
                    start: effective_start,
                    end: effective_end,
                },
            });
        }
    }
}
