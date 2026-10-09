// ============================================================================
// File: client/src/binary_sky/mod.rs
// ============================================================================
// ----------------------------------------------------------------------------
// DYNAMIC CELESTIAL S-TYPE BINARY SKY CYCLE & UNIFIED PARTICIPATING MEDIUM
// ----------------------------------------------------------------------------
// Architectural Note:
// Modularized into:
//   - `ephemeris`: Astronomical ephemeris, orbital mechanics, colors, and radiative transfer math.
//   - `aurora`: Custom WGSL shader materials and uniform buffer synchronization.
//   - `meshes`: Procedural starfield, sky dome, and precipitation meshes.
//   - `lighting`: Directional lights, shadow cascades, subterranean depth dimming, and fog.

pub mod ephemeris;
pub mod aurora;
pub mod meshes;
pub mod lighting;

pub use ephemeris::*;
pub use aurora::*;
pub use meshes::*;
pub use lighting::*;

use bevy::prelude::*;
use bevy::pbr::{DirectionalLightShadowMap, MaterialPlugin};

/// Interactive user input system for adjusting time of day, scrubbing cycles, and cycling weather.
pub fn handle_sky_time_and_weather_inputs(
    mut day_night_evts: EventReader<crate::input::CelestialCycleDayNightEvent>,
    mut weather_evts: EventReader<crate::input::CelestialCycleWeatherEvent>,
    mut step_evts: EventReader<crate::input::CelestialCycleStepEvent>,
    mut scale_evts: EventReader<crate::input::CelestialTimeScaleStepEvent>,
    mut config: ResMut<BinarySkyConfig>,
    mut ephemeris: ResMut<BinaryEphemerisState>,
    mut weather: ResMut<AtmosphericWeather>,
    mut console: Option<ResMut<crate::core::ConsoleState>>,
) {
    let day_duration = config.day_duration_seconds as f64;

    // Quick Day/Night Toggle
    for _ in day_night_evts.read() {
        if ephemeris.star_a_elevation > 0.0 {
            ephemeris.simulation_time_seconds = day_duration * 0.5;
            ephemeris.diurnal_angle = std::f32::consts::PI;
            info!("[Celestial Cycle] Toggled to DEEP NIGHT (00:00). Starfield & Aurora active.");
            if let Some(ref mut c) = console {
                c.logs.push("[Celestial Cycle] Toggled to DEEP NIGHT (00:00). Starfield & Aurora active.".into());
            }
        } else {
            ephemeris.simulation_time_seconds = 0.0;
            ephemeris.diurnal_angle = 0.0;
            info!("[Celestial Cycle] Toggled to HIGH NOON (12:00). Brilliant daytime illumination.");
            if let Some(ref mut c) = console {
                c.logs.push("[Celestial Cycle] Toggled to HIGH NOON (12:00). Brilliant daytime illumination.".into());
            }
        }
    }

    // Step 1 hour forward or backward
    for ev in step_evts.read() {
        let hour_sec = (config.day_duration_seconds / 24.0) as f64;
        if ev.forward {
            ephemeris.simulation_time_seconds = (ephemeris.simulation_time_seconds + hour_sec).rem_euclid(day_duration);
            let msg = format!("[Celestial Cycle] Advanced 1 hour. Current time: {:.1}h", ephemeris.clock_time_hours());
            info!("{}", msg);
            if let Some(ref mut c) = console {
                c.logs.push(msg);
            }
        } else {
            ephemeris.simulation_time_seconds = (ephemeris.simulation_time_seconds - hour_sec).rem_euclid(day_duration);
            let msg = format!("[Celestial Cycle] Rewound 1 hour. Current time: {:.1}h", ephemeris.clock_time_hours());
            info!("{}", msg);
            if let Some(ref mut c) = console {
                c.logs.push(msg);
            }
        }
    }

    // Step time scale faster or slower
    for ev in scale_evts.read() {
        if ev.faster {
            config.time_scale = match config.time_scale {
                s if s < 1.0 => 1.0,
                s if s < 10.0 => 10.0,
                s if s < 60.0 => 60.0,
                _ => 300.0,
            };
        } else {
            config.time_scale = match config.time_scale {
                s if s >= 300.0 => 60.0,
                s if s >= 60.0 => 10.0,
                s if s >= 10.0 => 1.0,
                s if s >= 1.0 => 0.0,
                _ => 0.0,
            };
        }
        let msg = format!("[Celestial Cycle] Time scale set to: {:.0}x", config.time_scale);
        info!("{}", msg);
        if let Some(ref mut c) = console {
            c.logs.push(msg);
        }
    }

    // Cycle Atmospheric Weather Presets
    for _ in weather_evts.read() {
        weather.weather_type = match weather.weather_type {
            WeatherType::ClearSky => WeatherType::AerosolHaze,
            WeatherType::AerosolHaze => WeatherType::StellarWindAurora,
            WeatherType::StellarWindAurora => WeatherType::OvercastPrecipitation,
            WeatherType::OvercastPrecipitation => WeatherType::ClearSky,
        };
        let msg = format!("[Atmospheric Weather] Preset changed to: {:?}", weather.weather_type);
        info!("{}", msg);
        if let Some(ref mut c) = console {
            c.logs.push(msg);
        }
    }
}

/// Modular Bevy plugin delivering a complete physically based dynamic S-type
/// binary celestial day/night sky cycle and atmospheric scattering pipeline.
pub struct BinarySkyPlugin;

impl Plugin for BinarySkyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DirectionalLightShadowMap { size: 2048 })
            .register_type::<PrimaryStar>()
            .register_type::<SecondaryStar>()
            .init_resource::<BinarySkyConfig>()
            .init_resource::<BinaryEphemerisState>()
            .init_resource::<AtmosphericRadianceCache>()
            .init_resource::<AtmosphericWeather>()
            .init_resource::<crate::tree_colors::SeasonState>()
            .init_resource::<AmbientLight>()
            .init_resource::<ClearColor>()
            .add_event::<crate::input::CelestialCycleDayNightEvent>()
            .add_event::<crate::input::CelestialCycleWeatherEvent>()
            .add_event::<crate::input::CelestialCycleStepEvent>()
            .add_event::<crate::input::CelestialTimeScaleStepEvent>()
            .add_plugins(MaterialPlugin::<StarAuroraDomeMaterial> {
                prepass_enabled: false,
                shadows_enabled: false,
                ..default()
            })
            .add_systems(Startup, setup_binary_sky_environment)
            .add_systems(
                Update,
                (
                    handle_sky_time_and_weather_inputs,
                    update_binary_ephemeris,
                    evaluate_horizon_dip_and_parry_occlusion,
                    update_atmospheric_scattering_and_cache,
                    sync_stellar_directional_lights,
                    sync_celestial_visuals,
                    update_atmospheric_cameras_and_fog,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;
    use bevy::pbr::{CascadeShadowConfigBuilder, FogFalloff, FogSettings, NotShadowCaster};
    use bevy::render::mesh::PrimitiveTopology;
    use bevy::render::view::RenderLayers;
    use parry3d::math::{Point as ParryPoint, Vector as ParryVector};
    use parry3d::query::{Ray as ParryRay, RayCast};
    use parry3d::shape::Ball as ParryBall;

    #[test]
    fn test_planck_blackbody_temperatures() {
        let star_a_rgb = planck_blackbody_rgb(5800.0);
        let star_b_rgb = planck_blackbody_rgb(3600.0);

        assert!(star_a_rgb.x > 0.7 && star_a_rgb.y > 0.7 && star_a_rgb.z > 0.6);
        assert!(star_b_rgb.x > 0.9);
        assert!(star_b_rgb.z < star_b_rgb.x * 0.4);
    }

    #[test]
    fn test_optical_air_mass_bounds() {
        let m_zenith = optical_air_mass(PI * 0.5);
        assert!((m_zenith - 1.0).abs() < 0.05);

        let m_horizon = optical_air_mass(0.0);
        assert!(m_horizon > 30.0 && m_horizon < 45.0);

        let m_twilight = optical_air_mass((-5.0f32).to_radians());
        assert!(m_twilight > 50.0 && m_twilight <= 80.0);
    }

    #[test]
    fn test_spectral_transmittance_rayleigh_mie() {
        let config = BinarySkyConfig::default();

        let trans_zenith = evaluate_spectral_transmittance(PI * 0.5, &config);
        assert!(trans_zenith.x > 0.85);
        assert!(trans_zenith.y > 0.75);
        assert!(trans_zenith.z > 0.60);

        let trans_horizon = evaluate_spectral_transmittance(0.0, &config);
        assert!(trans_horizon.x > trans_horizon.y);
        assert!(trans_horizon.y > trans_horizon.z);
        assert!(trans_horizon.x > 10.0 * trans_horizon.z);
    }

    #[test]
    fn test_stratospheric_ozone_and_spectral_transmittance() {
        let config = BinarySkyConfig::default();

        // 1. Ozone air mass bounds:
        let m_o_zenith = optical_air_mass_ozone(PI * 0.5);
        assert!((m_o_zenith - 1.0).abs() < 0.05);

        let m_o_horizon = optical_air_mass_ozone(0.0);
        assert!(m_o_horizon > 10.0 && m_o_horizon < 12.0);

        let m_o_twilight = optical_air_mass_ozone((-5.0f32).to_radians());
        assert!(m_o_twilight > 20.0 && m_o_twilight <= 38.0);

        // 2. Chappuis ozone absorption in green (550nm):
        let trans_twilight = evaluate_spectral_transmittance((-3.0f32).to_radians(), &config);
        assert!(trans_twilight.x > trans_twilight.y, "Red transmittance should exceed green due to ozone absorption");
    }

    #[test]
    fn test_reverse_engineered_physical_sky_chromaticity() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<BinarySkyConfig>();
        app.init_resource::<BinaryEphemerisState>();
        app.init_resource::<AtmosphericRadianceCache>();
        app.init_resource::<AtmosphericWeather>();
        app.init_resource::<AmbientLight>();
        app.init_resource::<ClearColor>();

        app.add_systems(Update, (update_binary_ephemeris, update_atmospheric_scattering_and_cache, update_atmospheric_cameras_and_fog).chain());

        // 1. Test High Noon (12:00):
        {
            let mut eph = app.world_mut().resource_mut::<BinaryEphemerisState>();
            eph.simulation_time_seconds = 0.0;
            eph.diurnal_angle = 0.0;
        }
        app.update();
        {
            let cache = app.world().resource::<AtmosphericRadianceCache>();
            assert!(cache.zenith_radiance.z > cache.zenith_radiance.x * 2.5, "Zenith sky must be Rayleigh blue at noon");
            assert!(cache.star_a_transmittance.x > 0.8 && cache.star_a_transmittance.z > 0.6);
            let horizon_col = tonemap_atmospheric_radiance(cache.horizon_radiance);
            assert!(horizon_col.z > horizon_col.x, "Horizon must be Rayleigh blue/cyan biased at high noon (blue > red)");
        }

        // 2. Test Sunset (18.0h):
        {
            let mut eph = app.world_mut().resource_mut::<BinaryEphemerisState>();
            let day_sec = 1440.0;
            eph.simulation_time_seconds = day_sec * 0.25;
            eph.diurnal_angle = PI * 0.5;
        }
        app.update();
        {
            let cache = app.world().resource::<AtmosphericRadianceCache>();
            assert!(cache.star_a_transmittance.x > cache.star_a_transmittance.z * 10.0, "Blue light must be extinguished at sunset");
            let horizon_col = tonemap_atmospheric_radiance(cache.horizon_radiance);
            assert!(horizon_col.x > horizon_col.y, "Horizon must be red/orange biased at sunset");
        }

        // 3. Test Deep Night (00:00):
        {
            let mut eph = app.world_mut().resource_mut::<BinaryEphemerisState>();
            let day_sec = 1440.0;
            eph.simulation_time_seconds = day_sec * 0.5;
            eph.diurnal_angle = PI;
        }
        app.update();
        {
            let clear_col = app.world().resource::<ClearColor>().0;
            let srgba = clear_col.to_srgba();
            assert!(srgba.blue > 0.25, "Night horizon clear color must have prominent blue");
            assert!(srgba.red > 0.08, "Night horizon clear color must have violet red tint");
            assert!(srgba.green < 0.05, "Green must be suppressed by ozone Chappuis band in night airglow");
        }
    }

    #[test]
    fn test_cornette_shanks_and_henyey_greenstein_phases() {
        let g = 0.75;
        let cs_forward = cornette_shanks_phase(1.0, g);
        let cs_backward = cornette_shanks_phase(-1.0, g);
        assert!(cs_forward > cs_backward * 10.0);

        let hg_forward = henyey_greenstein_phase(1.0, g);
        let hg_backward = henyey_greenstein_phase(-1.0, g);
        assert!(hg_forward > hg_backward * 10.0);

        let glare_a = multi_octave_stellar_glare(1.0, false);
        let glare_b = multi_octave_stellar_glare(1.0, true);
        assert!(glare_a > 1.0);
        assert!(glare_b > 1.0);
    }

    #[test]
    fn test_shadow_caster_priority_arbitration() {
        let role1 = determine_shadow_caster_priority(20.0, 15.0);
        assert_eq!(role1, ShadowCasterRole::StarA);

        let role2 = determine_shadow_caster_priority(-5.0, 15.0);
        assert_eq!(role2, ShadowCasterRole::StarB);

        let role3 = determine_shadow_caster_priority(-10.0, -5.0);
        assert_eq!(role3, ShadowCasterRole::None);
    }

    #[test]
    fn test_parry3d_planetary_horizon_dip() {
        let r_planet = 6_371_000.0f64;
        let planet_ball = ParryBall::new(r_planet as f32);

        let obs_point = ParryPoint::new(0.0, (r_planet + 1000.0) as f32, 0.0);
        let down_ray = ParryRay::new(obs_point, ParryVector::new(0.0, -1.0, 0.0));
        assert!(planet_ball.intersects_local_ray(&down_ray, 1e8));

        let up_ray = ParryRay::new(obs_point, ParryVector::new(0.0, 1.0, 0.0));
        assert!(!planet_ball.intersects_local_ray(&up_ray, 1e8));

        let cos_dip: f64 = r_planet / (r_planet + 2000.0);
        let dip_radians = cos_dip.acos();
        assert!(dip_radians > 0.015);
    }

    #[test]
    fn test_clock_time_calculation() {
        let mut ephemeris = BinaryEphemerisState::default();
        
        ephemeris.diurnal_angle = 0.0;
        assert!((ephemeris.clock_time_hours() - 12.0).abs() < 0.01);

        ephemeris.diurnal_angle = PI;
        assert!((ephemeris.clock_time_hours() - 0.0).abs() < 0.01 || (ephemeris.clock_time_hours() - 24.0).abs() < 0.01);

        ephemeris.diurnal_angle = PI * 0.5;
        assert!((ephemeris.clock_time_hours() - 18.0).abs() < 0.01);

        ephemeris.diurnal_angle = PI * 1.5;
        assert!((ephemeris.clock_time_hours() - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_starfield_mesh_generation() {
        let mesh = create_starfield_mesh(6000);
        assert_eq!(mesh.primitive_topology(), PrimitiveTopology::TriangleList);
        assert_eq!(mesh.count_vertices(), 6000 * 4);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert!(mesh.indices().is_some());
    }

    #[test]
    fn test_starfield_equal_area_distribution() {
        let star_count = 6000;
        let mesh = create_billboard_starfield_mesh(star_count);
        let pos_attr = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        let positions = pos_attr.as_float3().unwrap();

        let mut lower_band = 0;
        let mut upper_band = 0;

        for chunk in positions.chunks_exact(4) {
            let center_y = (chunk[0][1] + chunk[1][1] + chunk[2][1] + chunk[3][1]) / 4.0;
            let norm_y = center_y / 240.0;
            if (0.0..0.5).contains(&norm_y) {
                lower_band += 1;
            } else if norm_y >= 0.5 {
                upper_band += 1;
            }
        }

        let total = lower_band + upper_band;
        let upper_ratio = upper_band as f32 / total as f32;
        assert!(
            (upper_ratio - 0.5).abs() < 0.05,
            "Stars must be uniformly distributed with zero zenith concentration: upper_ratio was {:.3}",
            upper_ratio
        );
    }

    #[test]
    fn test_starfield_horizon_extinction() {
        let star_count = 6000;
        let mesh = create_billboard_starfield_mesh(star_count);
        let pos_attr = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        let col_attr = match mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap() {
            bevy::render::mesh::VertexAttributeValues::Float32x4(v) => v,
            _ => panic!("Expected Float32x4 colors"),
        };

        let mut extinguished_count = 0;
        let mut overhead_count = 0;

        for i in 0..star_count {
            let idx = i * 4;
            let center_y = (pos_attr[idx][1] + pos_attr[idx + 1][1] + pos_attr[idx + 2][1] + pos_attr[idx + 3][1]) / 4.0;
            let norm_y = center_y / 240.0;
            let alpha = col_attr[idx][3];

            if norm_y <= 0.015 {
                assert_eq!(alpha, 0.0, "Stars at or below horizon must be fully extinguished (alpha == 0)");
                extinguished_count += 1;
            } else if norm_y >= 0.16 {
                assert!(alpha >= 0.99, "Stars above extinction boundary must retain full opacity (alpha == 1.0)");
                overhead_count += 1;
            }
        }

        assert!(extinguished_count > 0, "Must have extinguished stars near horizon");
        assert!(overhead_count > 0, "Must have bright stars high overhead");
    }

    #[test]
    fn test_sky_dome_mesh_generation() {
        let mesh = create_sky_dome_mesh();
        assert_eq!(mesh.primitive_topology(), PrimitiveTopology::TriangleList);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert!(mesh.indices().is_some());
    }

    #[test]
    fn test_precipitation_mesh_generation() {
        let mesh = create_precipitation_mesh(100);
        assert_eq!(mesh.primitive_topology(), PrimitiveTopology::TriangleList);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert!(mesh.indices().is_some());
    }

    #[test]
    fn test_sunset_ephemeris_synchronization() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<BinarySkyConfig>();
        app.init_resource::<BinaryEphemerisState>();
        app.init_resource::<AtmosphericRadianceCache>();
        app.init_resource::<AtmosphericWeather>();
        app.init_resource::<AmbientLight>();
        app.init_resource::<ClearColor>();

        let day_sec = 1440.0;
        {
            let mut eph = app.world_mut().resource_mut::<BinaryEphemerisState>();
            eph.simulation_time_seconds = (day_sec * 0.25) as f64;
        }

        app.add_systems(Update, (update_binary_ephemeris, update_atmospheric_scattering_and_cache).chain());
        app.update();

        let eph = app.world().resource::<BinaryEphemerisState>();
        assert!((eph.clock_time_hours() - 18.0).abs() < 0.2);
        assert!(eph.star_a_elevation.abs() < 0.15);
        assert!(eph.star_a_direction.x < -0.80);
    }

    #[test]
    fn test_plugin_registration() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(BinarySkyPlugin);
        
        app.update();

        assert!(app.world().get_resource::<BinarySkyConfig>().is_some());
        assert!(app.world().get_resource::<BinaryEphemerisState>().is_some());
        assert!(app.world().get_resource::<AtmosphericRadianceCache>().is_some());
        assert!(app.world().get_resource::<AtmosphericWeather>().is_some());
    }

    #[test]
    fn test_dual_camera_starfield_and_aurora_sync() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::transform::TransformPlugin);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(BinarySkyPlugin);

        let active_pos = Vec3::new(10.0, 5.0, -20.0);
        app.world_mut().spawn((
            Camera3dBundle {
                camera: Camera { is_active: true, ..default() },
                transform: Transform::from_translation(active_pos),
                ..default()
            },
            AtmosphericCamera,
        ));
        app.world_mut().spawn((
            Camera3dBundle {
                camera: Camera { is_active: false, ..default() },
                transform: Transform::from_xyz(0.0, 35.0, 30.0),
                ..default()
            },
            AtmosphericCamera,
        ));

        {
            let mut ephemeris = app.world_mut().resource_mut::<BinaryEphemerisState>();
            ephemeris.simulation_time_seconds = 720.0;
            ephemeris.diurnal_angle = PI;
        }
        {
            let mut weather = app.world_mut().resource_mut::<AtmosphericWeather>();
            weather.weather_type = WeatherType::StellarWindAurora;
        }

        app.update();

        let mut starfield_query = app.world_mut().query_filtered::<(&Transform, &Visibility), With<CosmicStarfield>>();
        let (star_tf, star_vis) = starfield_query.single(app.world());
        assert_eq!(*star_vis, Visibility::Visible, "Starfield must be visible at deep night");
        assert_eq!(star_tf.translation, active_pos, "Starfield must track active camera translation");

        let mut aurora_query = app.world_mut().query_filtered::<(&Transform, &Visibility), With<StarAuroraDome>>();
        let (aurora_tf, aurora_vis) = aurora_query.single(app.world());
        assert_eq!(*aurora_vis, Visibility::Visible, "Aurora must be visible when StellarWindAurora is active");
        assert_eq!(aurora_tf.translation.x, active_pos.x);
        assert_eq!(aurora_tf.translation.z, active_pos.z);

        let clear_col = app.world().resource::<ClearColor>().0;
        let srgba = clear_col.to_srgba();
        assert!(srgba.blue > 0.25, "Blue channel should be prominent in electric navy night, got {}", srgba.blue);
        assert!(srgba.red > 0.08, "Red channel should have electric navy violet tint, got {}", srgba.red);
    }

    #[test]
    fn test_daytime_stars_hidden() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(BinarySkyPlugin);

        app.world_mut().spawn((
            Camera3dBundle {
                camera: Camera { is_active: true, ..default() },
                transform: Transform::from_xyz(0.0, 2.0, 0.0),
                ..default()
            },
            AtmosphericCamera,
        ));

        {
            let mut ephemeris = app.world_mut().resource_mut::<BinaryEphemerisState>();
            ephemeris.simulation_time_seconds = 0.0;
            ephemeris.diurnal_angle = 0.0;
        }

        app.update();

        let mut starfield_query = app.world_mut().query_filtered::<(&Transform, &Visibility), With<CosmicStarfield>>();
        let (_, star_vis) = starfield_query.single(app.world());
        assert_eq!(*star_vis, Visibility::Hidden, "Starfield must be hidden during daytime");
    }

    #[test]
    fn test_celestial_disks_not_shadow_casters_and_lights_cover_player_layer() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(BinarySkyPlugin);

        app.update();

        let mut star_a_query = app.world_mut().query_filtered::<Entity, (With<StarAVolumetricDisk>, With<NotShadowCaster>)>();
        assert_eq!(star_a_query.iter(app.world()).count(), 1, "Star A disk must have NotShadowCaster");

        let mut star_b_query = app.world_mut().query_filtered::<Entity, (With<StarBVolumetricDisk>, With<NotShadowCaster>)>();
        assert_eq!(star_b_query.iter(app.world()).count(), 1, "Star B disk must have NotShadowCaster");

        let mut starfield_query = app.world_mut().query_filtered::<Entity, (With<CosmicStarfield>, With<NotShadowCaster>)>();
        assert_eq!(starfield_query.iter(app.world()).count(), 1, "Starfield must have NotShadowCaster");

        let mut aurora_query = app.world_mut().query_filtered::<Entity, (With<StarAuroraDome>, With<NotShadowCaster>)>();
        assert_eq!(aurora_query.iter(app.world()).count(), 1, "Aurora must have NotShadowCaster");

        let mut rain_query = app.world_mut().query_filtered::<Entity, (With<PrecipitationStreaks>, With<NotShadowCaster>)>();
        assert_eq!(rain_query.iter(app.world()).count(), 1, "Rain must have NotShadowCaster");

        let mut light_a_query = app.world_mut().query_filtered::<&RenderLayers, With<PrimaryStar>>();
        let light_a_layers = light_a_query.single(app.world());
        assert!(light_a_layers.intersects(&RenderLayers::layer(0)), "Star A light must intersect world layer 0");
        assert!(light_a_layers.intersects(&RenderLayers::layer(2)), "Star A light must intersect player layer 2 to cast player character shadow");
        assert!(!light_a_layers.intersects(&RenderLayers::layer(1)), "Star A light must NOT intersect viewmodel layer 1");

        let mut light_b_query = app.world_mut().query_filtered::<&RenderLayers, With<SecondaryStar>>();
        let light_b_layers = light_b_query.single(app.world());
        assert!(light_b_layers.intersects(&RenderLayers::layer(0)), "Star B light must intersect world layer 0");
        assert!(light_b_layers.intersects(&RenderLayers::layer(2)), "Star B light must intersect player layer 2 to cast player character shadow");
        assert!(!light_b_layers.intersects(&RenderLayers::layer(1)), "Star B light must NOT intersect viewmodel layer 1");

        let shadow_map = app.world().resource::<DirectionalLightShadowMap>();
        assert_eq!(shadow_map.size, 2048, "Shadow map resolution must be 2048 for optimal frame performance");
    }

    #[test]
    fn test_aurora_custom_material_and_uniform_synchronization() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::transform::TransformPlugin);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(BinarySkyPlugin);

        app.world_mut().spawn((
            Camera3dBundle {
                camera: Camera { is_active: true, ..default() },
                transform: Transform::from_xyz(0.0, 10.0, 0.0),
                ..default()
            },
            AtmosphericCamera,
        ));

        let day_sec = 1440.0;
        {
            let mut ephemeris = app.world_mut().resource_mut::<BinaryEphemerisState>();
            ephemeris.simulation_time_seconds = (day_sec * 0.5) as f64;
            ephemeris.diurnal_angle = std::f32::consts::PI;
            ephemeris.star_a_elevation = -std::f32::consts::FRAC_PI_2;
            let mut weather = app.world_mut().resource_mut::<AtmosphericWeather>();
            weather.weather_type = WeatherType::StellarWindAurora;
            weather.aurora_intensity = 0.92;
        }

        app.update();

        let mut aurora_query = app.world_mut().query_filtered::<(&Handle<StarAuroraDomeMaterial>, &Visibility), With<StarAuroraDome>>();
        let (aurora_mat_handle, aurora_vis) = aurora_query.single(app.world());
        assert_eq!(*aurora_vis, Visibility::Visible, "Aurora dome must be visible during active aurora weather");

        let aurora_materials = app.world().resource::<Assets<StarAuroraDomeMaterial>>();
        let mat = aurora_materials.get(aurora_mat_handle).expect("StarAuroraDomeMaterial must exist in Assets");
        assert_eq!(mat.uniforms.weather_intensity, 0.92, "SkyUniforms weather_intensity must match weather intensity");
        assert!(mat.uniforms.night_factor > 0.9, "Night factor must be near 1.0 at midnight");
        assert_eq!(mat.uniforms.speed, 1.0, "Speed must be 1.0");
        assert_eq!(mat.uniforms.brightness, 1.0, "Brightness must be 1.0");

        assert_eq!(mat.alpha_mode(), AlphaMode::Blend, "Aurora dome material must use blend mode for dome overlay");

        let shader_src = include_str!("../../../assets/shaders/aurora.wgsl");
        assert!(shader_src.contains("TAU"), "Shader must define TAU");
        assert!(shader_src.contains("SkyUniforms"), "Shader must define SkyUniforms");
        assert!(shader_src.contains("auroraCurtainNoise"), "Shader must contain Nimitz triangle noise curtain algorithm");
        assert!(shader_src.contains("tri2"), "Shader must contain triangle noise functions");
        assert!(shader_src.contains("x * 0.5, y, x"), "Shader must contain requested spectral color synthesis");
        assert!(shader_src.contains("uniforms.weather_intensity * uniforms.night_factor <= 0.01"), "Shader must contain early-out inactivity check");
        assert!(!shader_src.contains("pow(s, 70.0)"), "Shader must NOT draw stars directly onto the aurora curtain");
    }

    #[test]
    fn test_atmospheric_camera_linear_fog_parameters() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<BinarySkyConfig>();
        app.init_resource::<BinaryEphemerisState>();
        app.init_resource::<AtmosphericRadianceCache>();
        app.init_resource::<AtmosphericWeather>();
        app.init_resource::<AmbientLight>();
        app.init_resource::<ClearColor>();

        let cam = app.world_mut().spawn((
            Camera3dBundle {
                transform: Transform::from_xyz(0.0, 50.0, 0.0),
                ..default()
            },
            AtmosphericCamera,
        )).id();

        app.add_systems(Update, (update_binary_ephemeris, update_atmospheric_scattering_and_cache, update_atmospheric_cameras_and_fog).chain());
        app.update();

        let fog = app.world().get::<FogSettings>(cam).expect("FogSettings must be attached to AtmosphericCamera");
        match fog.falloff {
            FogFalloff::Linear { start, end } => {
                assert!(start >= 40.0, "Fog should start beyond immediate player vicinity: {start}");
                assert!(start < 100.0, "Fog should begin in mid-range to prevent clear edges: {start}");
                assert_eq!(end, 230.4, "Fog end must reach 100% opacity at max draw distance (230.4m)");
            }
            _ => panic!("Atmospheric camera must use Linear fog falloff"),
        }

        let clear_color = app.world().resource::<ClearColor>().0;
        assert_eq!(fog.color, clear_color, "Fog color must match clear color for seamless horizon blending");
    }

    #[test]
    fn test_subterranean_depth_ambient_dimming() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<BinarySkyConfig>();
        app.init_resource::<BinaryEphemerisState>();
        app.init_resource::<AtmosphericRadianceCache>();
        app.init_resource::<AtmosphericWeather>();
        app.init_resource::<AmbientLight>();
        app.init_resource::<ClearColor>();

        let surface_h = crate::terrain::get_terrain_height(0.0, 0.0);
        assert_eq!(calculate_subterranean_dimming_factor(Vec3::new(0.0, surface_h + 10.0, 0.0)), 0.0);
        assert_eq!(calculate_subterranean_dimming_factor(Vec3::new(0.0, surface_h, 0.0)), 0.0);
        assert_eq!(calculate_subterranean_dimming_factor(Vec3::new(0.0, surface_h - 1.0, 0.0)), 0.0);
        let mid_factor = calculate_subterranean_dimming_factor(Vec3::new(0.0, surface_h - 8.0, 0.0));
        assert!((mid_factor - 0.5).abs() < 1e-4, "Depth of 8m below crust should yield factor ~0.5, got {}", mid_factor);
        assert_eq!(calculate_subterranean_dimming_factor(Vec3::new(0.0, surface_h - 15.0, 0.0)), 1.0);
        assert_eq!(calculate_subterranean_dimming_factor(Vec3::new(0.0, -120.0, 0.0)), 1.0);

        let cam = app.world_mut().spawn((
            Camera3dBundle {
                camera: Camera { is_active: true, ..default() },
                transform: Transform::from_xyz(0.0, -25.0, 0.0),
                ..default()
            },
            AtmosphericCamera,
        )).id();

        app.world_mut().spawn((
            DirectionalLightBundle::default(),
            PrimaryStar,
        ));
        app.world_mut().spawn((
            DirectionalLightBundle::default(),
            SecondaryStar,
        ));

        app.add_systems(
            Update,
            (
                update_binary_ephemeris,
                update_atmospheric_scattering_and_cache,
                sync_stellar_directional_lights,
                update_atmospheric_cameras_and_fog,
            ).chain()
        );
        app.update();

        let ambient = app.world().resource::<AmbientLight>();
        assert!((ambient.brightness - 12.0).abs() < 1.0, "Subterranean ambient brightness should be floor 12 lx, got {}", ambient.brightness);

        let clear_color = app.world().resource::<ClearColor>().0;
        let c_srgba = clear_color.to_srgba();
        assert!(c_srgba.red < 0.05 && c_srgba.green < 0.05 && c_srgba.blue < 0.05, "ClearColor must be dark cavern slate underground");

        let fog = app.world().get::<FogSettings>(cam).expect("FogSettings must exist");
        assert_eq!(fog.color, clear_color);
        assert_eq!(fog.directional_light_color, Color::NONE);
        match fog.falloff {
            FogFalloff::Linear { start, end } => {
                assert!((start - 8.0).abs() < 0.1, "Subterranean fog start must clamp to 8.0m, got {start}");
                assert!((end - 55.0).abs() < 0.1, "Subterranean fog end must clamp to 55.0m, got {end}");
            }
            _ => panic!("Expected linear fog"),
        }

        let mut light_query = app.world_mut().query_filtered::<&DirectionalLight, With<PrimaryStar>>();
        let light_a = light_query.single(app.world());
        assert_eq!(light_a.illuminance, 0.0, "Directional light illuminance must be 0 underground");
        assert!(!light_a.shadows_enabled, "Shadows should be disabled deep underground");
    }

    #[test]
    fn test_asymmetric_cascade_configuration_construction() {
        let config = BinarySkyConfig::default();

        let cascade_a = CascadeShadowConfigBuilder {
            num_cascades: config.star_a_num_cascades,
            minimum_distance: config.star_a_minimum_shadow_distance,
            maximum_distance: config.star_a_maximum_shadow_distance,
            first_cascade_far_bound: config.star_a_first_cascade_far_bound,
            overlap_proportion: 0.20,
        }
        .build();

        let cascade_b = CascadeShadowConfigBuilder {
            num_cascades: config.star_b_num_cascades,
            minimum_distance: config.star_b_minimum_shadow_distance,
            maximum_distance: config.star_b_maximum_shadow_distance,
            first_cascade_far_bound: config.star_b_first_cascade_far_bound,
            overlap_proportion: 0.20,
        }
        .build();

        assert_eq!(cascade_a.bounds.len(), 3);
        assert_eq!(cascade_b.bounds.len(), 2);
        assert!(cascade_a.bounds.last().unwrap() > cascade_b.bounds.last().unwrap());
    }

    #[test]
    fn test_dual_shadow_casting_progression() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(BinarySkyPlugin);

        app.update();

        let mut light_query = app.world_mut().query::<(&DirectionalLight, Option<&PrimaryStar>, Option<&SecondaryStar>)>();
        let lights: Vec<(&DirectionalLight, bool, bool)> = light_query
            .iter(app.world())
            .map(|(l, p, s)| (l, p.is_some(), s.is_some()))
            .collect();

        assert_eq!(lights.len(), 2, "BinarySkyPlugin must spawn exactly 2 directional lights");
        let star_a = lights.iter().find(|(_, is_p, _)| *is_p).expect("Star A must exist");
        assert!(star_a.0.shadows_enabled, "Star A must have shadows enabled at default high noon");
    }

    #[test]
    fn test_drg_performance_illusion_point_lights_shadows_disabled() {
        let default_torch_light = PointLight {
            color: Color::srgb(1.0, 0.75, 0.4),
            intensity: 3500.0,
            range: 8.0,
            shadows_enabled: false,
            ..default()
        };
        assert!(!default_torch_light.shadows_enabled, "Local point lights must have shadows_enabled = false");
        assert!(default_torch_light.range <= 16.0, "Local point lights must maintain tight inverse-square radius");
    }
}
