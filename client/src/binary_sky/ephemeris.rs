// ============================================================================
// File: client/src/binary_sky/ephemeris.rs
// ============================================================================
// ----------------------------------------------------------------------------
// ASTRONOMICAL EPHEMERIS, ORBITAL MECHANICS & PARTICIPATING MEDIUM
// ----------------------------------------------------------------------------

use std::f32::consts::PI;
use bevy::prelude::*;
use parry3d::math::{Point as ParryPoint, Vector as ParryVector};
use parry3d::query::{Ray as ParryRay, RayCast};
use parry3d::shape::Ball as ParryBall;

// Canonical sunset & twilight color palette for S-type planetary atmosphere:
pub const COLOR_NAVY_ELECTRIC: Vec3 = Vec3::new(0.2235, 0.0, 0.6000);   // #390099
pub const COLOR_DARK_RASPBERRY: Vec3 = Vec3::new(0.6196, 0.0, 0.3490);  // #9e0059
pub const COLOR_HOT_FUCHSIA: Vec3 = Vec3::new(1.0, 0.0, 0.3294);       // #ff0054
pub const COLOR_BLAZE_ORANGE: Vec3 = Vec3::new(1.0, 0.3294, 0.0);      // #ff5400
pub const COLOR_AMBER_GOLD: Vec3 = Vec3::new(1.0, 0.7412, 0.0);        // #ffbd00

/// Parses a hex string (e.g. "ff5400", "#390099") or preset name into a Bevy `Color`.
pub fn parse_color_spec(input: &str) -> Option<Color> {
    let clean = input.trim().to_lowercase();
    let stripped = clean.trim_start_matches('#');
    if stripped.len() == 6 {
        let r = u8::from_str_radix(&stripped[0..2], 16).ok()?;
        let g = u8::from_str_radix(&stripped[2..4], 16).ok()?;
        let b = u8::from_str_radix(&stripped[4..6], 16).ok()?;
        return Some(Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0));
    }
    match clean.as_str() {
        "navy" | "electric" | "navyelectric" => Some(Color::srgb(0.2235, 0.0, 0.6000)),
        "raspberry" | "darkraspberry" => Some(Color::srgb(0.6196, 0.0, 0.3490)),
        "fuchsia" | "hotfuchsia" => Some(Color::srgb(1.0, 0.0, 0.3294)),
        "blaze" | "orange" | "blazeorange" => Some(Color::srgb(1.0, 0.3294, 0.0)),
        "amber" | "gold" | "ambergold" => Some(Color::srgb(1.0, 0.7412, 0.0)),
        "white" | "solar" => Some(Color::srgb(1.0, 0.98, 0.92)),
        "cyan" | "sky" => Some(Color::srgb(0.55, 0.85, 1.0)),
        _ => None,
    }
}

/// Evaluates the palette-based twilight transition color given dominant stellar elevation:
pub fn evaluate_sunset_palette_color(elevation_radians: f32) -> Vec3 {
    let deg = elevation_radians.to_degrees();
    if deg >= 10.0 {
        Vec3::new(0.52, 0.68, 0.88)
    } else if deg >= 4.5 {
        let t = (10.0 - deg) / 5.5;
        Vec3::new(0.52, 0.68, 0.88).lerp(COLOR_AMBER_GOLD, t)
    } else if deg >= 1.0 {
        let t = (4.5 - deg) / 3.5;
        COLOR_AMBER_GOLD.lerp(COLOR_BLAZE_ORANGE, t)
    } else if deg >= -2.5 {
        let t = (1.0 - deg) / 3.5;
        COLOR_BLAZE_ORANGE.lerp(COLOR_HOT_FUCHSIA, t)
    } else if deg >= -6.5 {
        let t = (-2.5 - deg) / 4.0;
        COLOR_HOT_FUCHSIA.lerp(COLOR_DARK_RASPBERRY, t)
    } else if deg >= -12.0 {
        let t = (-6.5 - deg) / 5.5;
        COLOR_DARK_RASPBERRY.lerp(COLOR_NAVY_ELECTRIC, t)
    } else {
        let t = ((-12.0 - deg) / 6.0).clamp(0.0, 1.0);
        let deep_navy = COLOR_NAVY_ELECTRIC * 0.40 + Vec3::new(0.02, 0.015, 0.06);
        COLOR_NAVY_ELECTRIC.lerp(deep_navy, t)
    }
}

/// Dynamic sky illumination condition based on stellar elevation thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DynamicSkyState {
    #[default]
    DualDay,
    StarAPrimaryDay,
    StarBSecondaryDay,
    TrueNight,
    BinaryAlignment,
    CivilTwilight,
    NauticalTwilight,
}

/// Identifies which stellar body holds priority for DirectionalLight shadow cascades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowCasterRole {
    #[default]
    StarA,
    StarB,
    None,
}

/// Active atmospheric weather condition modulating participating medium turbidity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeatherType {
    #[default]
    ClearSky,
    AerosolHaze,
    StellarWindAurora,
    OvercastPrecipitation,
}

/// Configuration parameters governing the S-type binary orbital mechanics,
/// planetary physical dimensions, and atmospheric participating medium coefficients.
#[derive(Resource, Debug, Clone)]
pub struct BinarySkyConfig {
    pub time_scale: f32,
    pub day_duration_seconds: f32,
    pub year_duration_days: f32,
    pub binary_period_years: f32,
    pub axial_tilt_radians: f32,
    pub binary_inclination_radians: f32,
    pub observer_latitude_radians: f32,
    pub observer_longitude_radians: f32,
    pub planet_radius_meters: f64,
    pub atmosphere_height_meters: f32,
    pub rayleigh_scale_height: f32,
    pub mie_scale_height: f32,
    pub ozone_scale_height: f32,
    pub rayleigh_scattering_coefficients: Vec3,
    pub mie_scattering_coefficient: f32,
    pub mie_absorption_coefficient: f32,
    pub ozone_absorption_coefficients: Vec3,
    pub ground_albedo: f32,
    pub star_a_temperature_kelvin: f32,
    pub star_a_base_illuminance_lux: f32,
    pub star_b_temperature_kelvin: f32,
    pub star_b_base_illuminance_lux: f32,
    pub star_a_color_override: Option<Color>,
    pub star_b_color_override: Option<Color>,
    pub star_a_shadows_enabled: bool,
    pub star_b_shadows_enabled: bool,
    pub ambient_illuminance_lux: Option<f32>,
    pub starfield_scale: f32,
    pub star_a_enabled: bool,
    pub star_b_enabled: bool,
    pub star_a_num_cascades: usize,
    pub star_a_maximum_shadow_distance: f32,
    pub star_a_first_cascade_far_bound: f32,
    pub star_a_shadow_depth_bias: f32,
    pub star_a_shadow_normal_bias: f32,
    pub star_a_minimum_shadow_distance: f32,
    pub star_b_num_cascades: usize,
    pub star_b_maximum_shadow_distance: f32,
    pub star_b_first_cascade_far_bound: f32,
    pub star_b_shadow_depth_bias: f32,
    pub star_b_shadow_normal_bias: f32,
    pub star_b_minimum_shadow_distance: f32,
}

impl Default for BinarySkyConfig {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            day_duration_seconds: 1440.0,
            year_duration_days: 365.0,
            binary_period_years: 18.5,
            axial_tilt_radians: 23.44f32.to_radians(),
            binary_inclination_radians: 14.25f32.to_radians(),
            observer_latitude_radians: 45.0f32.to_radians(),
            observer_longitude_radians: 0.0,
            planet_radius_meters: 6_371_000.0,
            atmosphere_height_meters: 100_000.0,
            rayleigh_scale_height: 8_400.0,
            mie_scale_height: 1_200.0,
            ozone_scale_height: 25_000.0,
            rayleigh_scattering_coefficients: Vec3::new(0.0058, 0.0135, 0.0331),
            mie_scattering_coefficient: 0.0040,
            mie_absorption_coefficient: 0.0044,
            ozone_absorption_coefficients: Vec3::new(0.00065, 0.00240, 0.000085),
            ground_albedo: 0.18,
            star_a_temperature_kelvin: 5800.0,
            star_a_base_illuminance_lux: 50_000.0,
            star_b_temperature_kelvin: 3900.0,
            star_b_base_illuminance_lux: 35_000.0,
            star_a_color_override: None,
            star_b_color_override: None,
            star_a_shadows_enabled: true,
            star_b_shadows_enabled: true,
            star_a_enabled: true,
            star_b_enabled: true,
            ambient_illuminance_lux: None,
            starfield_scale: 1.0,
            star_a_num_cascades: 3,
            star_a_maximum_shadow_distance: 160.0,
            star_a_first_cascade_far_bound: 15.0,
            star_a_shadow_depth_bias: 0.02,
            star_a_shadow_normal_bias: 1.8,
            star_a_minimum_shadow_distance: 0.5,
            star_b_num_cascades: 2,
            star_b_maximum_shadow_distance: 112.5,
            star_b_first_cascade_far_bound: 18.0,
            star_b_shadow_depth_bias: 0.02,
            star_b_shadow_normal_bias: 1.8,
            star_b_minimum_shadow_distance: 0.5,
        }
    }
}

/// Instantaneous astronomical ephemeris state tracking normalized topocentric
/// stellar direction vectors, elevations, azimuths, and dynamic sky classifications.
#[derive(Resource, Debug, Clone)]
pub struct BinaryEphemerisState {
    pub simulation_time_seconds: f64,
    pub day_progress: f32,
    pub year_progress: f32,
    pub binary_progress: f32,
    pub diurnal_angle: f32,
    pub primary_orbit_angle: f32,
    pub binary_orbit_angle: f32,
    pub star_a_direction: Vec3,
    pub star_a_elevation: f32,
    pub star_a_azimuth: f32,
    pub star_b_direction: Vec3,
    pub star_b_elevation: f32,
    pub star_b_azimuth: f32,
    pub angular_separation: f32,
    pub sky_state: DynamicSkyState,
}

impl BinaryEphemerisState {
    pub fn clock_time_hours(&self) -> f32 {
        let frac = (self.diurnal_angle / (2.0 * PI)).rem_euclid(1.0);
        ((frac + 0.5).rem_euclid(1.0)) * 24.0
    }
}

impl Default for BinaryEphemerisState {
    fn default() -> Self {
        Self {
            simulation_time_seconds: 0.0,
            day_progress: 0.0,
            year_progress: 0.0,
            binary_progress: 0.0,
            diurnal_angle: 0.0,
            primary_orbit_angle: 0.0,
            binary_orbit_angle: 0.0,
            star_a_direction: Vec3::new(0.0, std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2),
            star_a_elevation: PI * 0.25,
            star_a_azimuth: 0.0,
            star_b_direction: Vec3::new(0.5, 0.4, 0.5).normalize(),
            star_b_elevation: PI * 0.15,
            star_b_azimuth: PI * 0.5,
            angular_separation: PI * 0.3,
            sky_state: DynamicSkyState::DualDay,
        }
    }
}

/// Downsampled analytical sky radiance cache evaluated each frame.
#[derive(Resource, Debug, Clone)]
pub struct AtmosphericRadianceCache {
    pub zenith_radiance: Vec3,
    pub horizon_radiance: Vec3,
    pub ambient_color: Color,
    pub ambient_brightness_lux: f32,
    pub ground_bounce_color: Color,
    pub star_a_illuminance_lux: f32,
    pub star_a_transmittance: Vec3,
    pub star_a_color: Color,
    pub star_b_illuminance_lux: f32,
    pub star_b_transmittance: Vec3,
    pub star_b_color: Color,
    pub active_shadow_caster: ShadowCasterRole,
    pub horizon_dip_radians: f32,
    pub conjunction_amplification: f32,
}

impl Default for AtmosphericRadianceCache {
    fn default() -> Self {
        Self {
            zenith_radiance: Vec3::new(0.05, 0.12, 0.28),
            horizon_radiance: Vec3::new(0.40, 0.52, 0.68),
            ambient_color: Color::srgb(0.35, 0.45, 0.65),
            ambient_brightness_lux: 1_200.0,
            ground_bounce_color: Color::srgb(0.08, 0.07, 0.05),
            star_a_illuminance_lux: 95_000.0,
            star_a_transmittance: Vec3::new(0.92, 0.88, 0.74),
            star_a_color: Color::srgb(1.0, 0.96, 0.88),
            star_b_illuminance_lux: 21_000.0,
            star_b_transmittance: Vec3::new(0.85, 0.62, 0.32),
            star_b_color: Color::srgb(1.0, 0.65, 0.30),
            active_shadow_caster: ShadowCasterRole::StarA,
            horizon_dip_radians: 0.0,
            conjunction_amplification: 1.0,
        }
    }
}

/// Dynamic atmospheric weather state resource controlling environmental effects.
#[derive(Resource, Debug, Clone)]
pub struct AtmosphericWeather {
    pub weather_type: WeatherType,
    pub haze_density_multiplier: f32,
    pub aurora_intensity: f32,
    pub cloud_coverage: f32,
    pub precipitation_rate: f32,
}

impl Default for AtmosphericWeather {
    fn default() -> Self {
        Self {
            weather_type: WeatherType::ClearSky,
            haze_density_multiplier: 1.0,
            aurora_intensity: 0.8,
            cloud_coverage: 0.0,
            precipitation_rate: 0.0,
        }
    }
}

pub fn planck_blackbody_rgb(temp_kelvin: f32) -> Vec3 {
    let t = temp_kelvin.max(1000.0);
    let wavelengths = [6.80e-7f64, 5.50e-7f64, 4.40e-7f64];
    
    let h = 6.62607015e-34f64;
    let c = 2.99792458e8f64;
    let k = 1.380649e-23f64;

    let mut spectral_radiance = [0.0f64; 3];
    for (i, &lambda) in wavelengths.iter().enumerate() {
        let exponent = (h * c) / (lambda * k * t as f64);
        let exp_term = if exponent > 700.0 { 700.0f64.exp() } else { exponent.exp() };
        let numerator = 2.0 * h * c * c;
        let denominator = lambda.powi(5) * (exp_term - 1.0);
        spectral_radiance[i] = numerator / denominator;
    }

    let max_val = spectral_radiance[0].max(spectral_radiance[1]).max(spectral_radiance[2]).max(1e-12);
    Vec3::new(
        (spectral_radiance[0] / max_val) as f32,
        (spectral_radiance[1] / max_val) as f32,
        (spectral_radiance[2] / max_val) as f32,
    )
}

pub fn optical_air_mass(elevation_radians: f32) -> f32 {
    let deg = elevation_radians.to_degrees();
    if deg >= 0.0 {
        let sin_elev = elevation_radians.sin();
        let denom = sin_elev + 0.50572 * (deg + 6.07995).powf(-1.6364);
        (1.0 / denom.max(1e-4)).clamp(1.0, 38.2)
    } else {
        (38.2 + (-deg) * 7.0).min(80.0)
    }
}

pub fn optical_air_mass_ozone(elevation_radians: f32) -> f32 {
    let deg = elevation_radians.to_degrees();
    if deg >= 0.0 {
        let r_ratio = 6371.0 / (6371.0 + 25.0);
        let cos_elev = elevation_radians.cos();
        let denom = (1.0 - r_ratio * r_ratio * cos_elev * cos_elev).max(1e-4).sqrt();
        (1.0 / denom).clamp(1.0, 11.35)
    } else {
        (11.35 + (-deg) * 2.5).clamp(11.35, 38.0)
    }
}

pub fn evaluate_spectral_transmittance(
    elevation_radians: f32,
    config: &BinarySkyConfig,
) -> Vec3 {
    let air_mass_r = optical_air_mass(elevation_radians);
    let air_mass_o = optical_air_mass_ozone(elevation_radians);
    let h_r_km = config.rayleigh_scale_height * 0.001;
    let h_m_km = config.mie_scale_height * 0.001;
    let h_o_km = config.ozone_scale_height * 0.001;
    let beta_me = config.mie_scattering_coefficient + config.mie_absorption_coefficient;

    let tau_rayleigh = config.rayleigh_scattering_coefficients * h_r_km * air_mass_r;
    let tau_mie = Vec3::splat(beta_me * h_m_km * air_mass_r);
    let tau_ozone = config.ozone_absorption_coefficients * h_o_km * air_mass_o;
    let total_optical_depth = tau_rayleigh + tau_mie + tau_ozone;
    
    Vec3::new(
        (-total_optical_depth.x).exp(),
        (-total_optical_depth.y).exp(),
        (-total_optical_depth.z).exp(),
    )
}

pub fn tonemap_atmospheric_radiance(radiance: Vec3) -> Vec3 {
    let luma = radiance.x * 0.2126 + radiance.y * 0.7152 + radiance.z * 0.0722;
    let t_day = (luma / 0.12).clamp(0.0, 1.0);
    let s_day = t_day * t_day * (3.0 - 2.0 * t_day);

    let t_night = 1.0 - (luma / 0.035).clamp(0.0, 1.0);
    let s_night = t_night * t_night * (3.0 - 2.0 * t_night);

    let exp_day = 1.0 / (luma + 0.15);
    let exp_twilight = 1.0 / (luma + 0.04);
    let exp_night = (1.0 / (luma + 0.02)).clamp(15.0, 32.0);

    let exposure = exp_day * s_day + exp_twilight * (1.0 - s_day) * (1.0 - s_night) + exp_night * s_night;
    let exp = radiance * exposure;
    Vec3::new(
        (exp.x / (1.0 + exp.x * 0.8)).clamp(0.0, 1.0),
        (exp.y / (1.0 + exp.y * 0.8)).clamp(0.0, 1.0),
        (exp.z / (1.0 + exp.z * 0.8)).clamp(0.0, 1.0),
    )
}

pub fn cornette_shanks_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let cos2 = cos_theta * cos_theta;
    let normalizer = (3.0 / (8.0 * PI)) * ((1.0 - g2) / (2.0 + g2));
    let denom = (1.0 + g2 - 2.0 * g * cos_theta).max(1e-4).powf(1.5);
    normalizer * ((1.0 + cos2) / denom)
}

pub fn henyey_greenstein_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = (1.0 + g2 - 2.0 * g * cos_theta).max(1e-4).powf(1.5);
    (1.0 / (4.0 * PI)) * ((1.0 - g2) / denom)
}

pub fn multi_octave_stellar_glare(cos_theta: f32, is_secondary_star: bool) -> f32 {
    if !is_secondary_star {
        let spike = henyey_greenstein_phase(cos_theta, 0.992) * 0.60;
        let corona = cornette_shanks_phase(cos_theta, 0.75) * 0.30;
        let haze = cornette_shanks_phase(cos_theta, 0.25) * 0.10;
        spike + corona + haze
    } else {
        let spike = henyey_greenstein_phase(cos_theta, 0.970) * 0.45;
        let corona = cornette_shanks_phase(cos_theta, 0.68) * 0.40;
        let haze = cornette_shanks_phase(cos_theta, 0.20) * 0.15;
        spike + corona + haze
    }
}

#[inline]
pub fn determine_shadow_caster_priority(elev_a_deg: f32, elev_b_deg: f32) -> ShadowCasterRole {
    if elev_a_deg > -4.0 {
        ShadowCasterRole::StarA
    } else if elev_b_deg > 0.0 {
        ShadowCasterRole::StarB
    } else {
        ShadowCasterRole::None
    }
}

pub fn update_binary_ephemeris(
    time: Res<Time<Virtual>>,
    config: Res<BinarySkyConfig>,
    mut ephemeris: ResMut<BinaryEphemerisState>,
    season: Option<ResMut<crate::tree_colors::SeasonState>>,
) {
    let delta = time.delta_seconds_f64() * config.time_scale as f64;
    ephemeris.simulation_time_seconds += delta;

    let day_duration = config.day_duration_seconds as f64;
    let year_duration = day_duration * config.year_duration_days as f64;
    let binary_duration = year_duration * config.binary_period_years as f64;

    let t = ephemeris.simulation_time_seconds;
    ephemeris.day_progress = ((t % day_duration) / day_duration) as f32;
    ephemeris.year_progress = ((t % year_duration) / year_duration) as f32;
    ephemeris.binary_progress = ((t % binary_duration) / binary_duration) as f32;

    if let Some(mut season) = season {
        let year_frac = ephemeris.year_progress.rem_euclid(1.0);
        let (current_season, progress) = if year_frac < 0.25 {
            (crate::tree_colors::Season::Spring, year_frac / 0.25)
        } else if year_frac < 0.50 {
            (crate::tree_colors::Season::Summer, (year_frac - 0.25) / 0.25)
        } else if year_frac < 0.75 {
            (crate::tree_colors::Season::Autumn, (year_frac - 0.50) / 0.25)
        } else {
            (crate::tree_colors::Season::Winter, (year_frac - 0.75) / 0.25)
        };
        season.current = current_season;
        season.progress = progress;
    }

    ephemeris.diurnal_angle = (ephemeris.day_progress * 2.0 * PI) as f32;
    ephemeris.primary_orbit_angle = (ephemeris.year_progress * 2.0 * PI) as f32;
    ephemeris.binary_orbit_angle = (ephemeris.binary_progress * 2.0 * PI) as f32;

    let phi = config.observer_latitude_radians;
    let eps = config.axial_tilt_radians;

    // ------------------------------------------------------------------------
    // Host Star A Topocentric Position
    // ------------------------------------------------------------------------
    let lambda_a = ephemeris.primary_orbit_angle;
    let sin_dec_a = eps.sin() * lambda_a.sin();
    let dec_a = sin_dec_a.asin();
    let ra_a = (eps.cos() * lambda_a.sin()).atan2(lambda_a.cos());

    let lha_a = ephemeris.diurnal_angle - ra_a - config.observer_longitude_radians;
    let sin_elev_a = phi.sin() * dec_a.sin() + phi.cos() * dec_a.cos() * lha_a.cos();
    let elev_a = sin_elev_a.clamp(-1.0, 1.0).asin();

    let cos_azim_a = (dec_a.sin() - phi.sin() * elev_a.sin()) / (phi.cos() * elev_a.cos().max(1e-4));
    let azim_a = if lha_a.sin() > 0.0 {
        2.0 * PI - cos_azim_a.clamp(-1.0, 1.0).acos()
    } else {
        cos_azim_a.clamp(-1.0, 1.0).acos()
    };

    ephemeris.star_a_elevation = elev_a;
    ephemeris.star_a_azimuth = azim_a;

    let cos_el_a = elev_a.cos();
    ephemeris.star_a_direction = Vec3::new(
        cos_el_a * azim_a.sin(),
        elev_a.sin(),
        -cos_el_a * azim_a.cos(),
    ).normalize_or_zero();

    // ------------------------------------------------------------------------
    // Companion Star B Topocentric Position
    // ------------------------------------------------------------------------
    let i_b = config.binary_inclination_radians;
    let omega_b = ephemeris.binary_orbit_angle;

    let beta_b = i_b * omega_b.sin();
    let lambda_b = lambda_a + 0.275 + 0.07 * omega_b.cos();

    let sin_dec_b = eps.cos() * beta_b.sin() + eps.sin() * beta_b.cos() * lambda_b.sin();
    let dec_b = sin_dec_b.clamp(-1.0, 1.0).asin();
    let ra_b = (lambda_b.sin() * eps.cos() - beta_b.tan() * eps.sin()).atan2(lambda_b.cos());

    let lha_b = ephemeris.diurnal_angle - ra_b - config.observer_longitude_radians;
    let sin_elev_b = phi.sin() * dec_b.sin() + phi.cos() * dec_b.cos() * lha_b.cos();
    let elev_b = sin_elev_b.clamp(-1.0, 1.0).asin();

    let cos_azim_b = (dec_b.sin() - phi.sin() * elev_b.sin()) / (phi.cos() * elev_b.cos().max(1e-4));
    let azim_b = if lha_b.sin() > 0.0 {
        2.0 * PI - cos_azim_b.clamp(-1.0, 1.0).acos()
    } else {
        cos_azim_b.clamp(-1.0, 1.0).acos()
    };

    ephemeris.star_b_elevation = elev_b;
    ephemeris.star_b_azimuth = azim_b;

    let cos_el_b = elev_b.cos();
    ephemeris.star_b_direction = Vec3::new(
        cos_el_b * azim_b.sin(),
        elev_b.sin(),
        -cos_el_b * azim_b.cos(),
    ).normalize_or_zero();

    // ------------------------------------------------------------------------
    // Angular Separation & Discrete Sky State Classification
    // ------------------------------------------------------------------------
    let dot_prod = ephemeris.star_a_direction.dot(ephemeris.star_b_direction).clamp(-1.0, 1.0);
    ephemeris.angular_separation = dot_prod.acos();

    let el_a_deg = elev_a.to_degrees();
    let el_b_deg = elev_b.to_degrees();

    ephemeris.sky_state = if ephemeris.angular_separation < 7.5f32.to_radians() && (el_a_deg > -2.0 || el_b_deg > -2.0) {
        DynamicSkyState::BinaryAlignment
    } else if el_a_deg > 0.0 && el_b_deg > 0.0 {
        DynamicSkyState::DualDay
    } else if el_a_deg > 0.0 && el_b_deg <= 0.0 {
        DynamicSkyState::StarAPrimaryDay
    } else if el_b_deg > 0.0 && el_a_deg <= 0.0 {
        DynamicSkyState::StarBSecondaryDay
    } else if (el_a_deg > -6.0 && el_a_deg <= 0.0) || (el_b_deg > -6.0 && el_b_deg <= 0.0) {
        DynamicSkyState::CivilTwilight
    } else if (el_a_deg > -12.0 && el_a_deg <= -6.0) || (el_b_deg > -12.0 && el_b_deg <= -6.0) {
        DynamicSkyState::NauticalTwilight
    } else {
        DynamicSkyState::TrueNight
    };
}

pub fn evaluate_horizon_dip_and_parry_occlusion(
    config: Res<BinarySkyConfig>,
    camera_query: Query<(&GlobalTransform, &Camera), With<super::lighting::AtmosphericCamera>>,
    mut cache: ResMut<AtmosphericRadianceCache>,
) {
    let camera_alt = if let Some((cam_tf, _)) = camera_query.iter().find(|(_, cam)| cam.is_active) {
        (cam_tf.translation().y as f64).max(0.0)
    } else if let Some((cam_tf, _)) = camera_query.iter().next() {
        (cam_tf.translation().y as f64).max(0.0)
    } else {
        2.0
    };

    let r_planet = config.planet_radius_meters;
    let r_obs = r_planet + camera_alt;
    
    let cos_dip = (r_planet / r_obs).clamp(0.0, 1.0);
    let horizon_dip = cos_dip.acos() as f32;
    cache.horizon_dip_radians = horizon_dip;

    let planet_ball = ParryBall::new(r_planet as f32);
    let observer_point = ParryPoint::new(0.0, r_obs as f32, 0.0);
    let tangent_dir = ParryVector::new(1.0, 0.0, 0.0);
    let ray = ParryRay::new(observer_point, tangent_dir);
    let _intersects_planet = planet_ball.intersects_local_ray(&ray, 1e8);
}

pub fn update_atmospheric_scattering_and_cache(
    config: Res<BinarySkyConfig>,
    ephemeris: Res<BinaryEphemerisState>,
    weather: Res<AtmosphericWeather>,
    mut cache: ResMut<AtmosphericRadianceCache>,
) {
    let effective_elev_a = ephemeris.star_a_elevation + cache.horizon_dip_radians;
    let effective_elev_b = ephemeris.star_b_elevation + cache.horizon_dip_radians;

    // 1. Direct Stellar Transmittance & Illuminance
    cache.star_a_transmittance = evaluate_spectral_transmittance(effective_elev_a, &config);
    cache.star_b_transmittance = evaluate_spectral_transmittance(effective_elev_b, &config);

    let blackbody_a = planck_blackbody_rgb(config.star_a_temperature_kelvin);
    let blackbody_b = planck_blackbody_rgb(config.star_b_temperature_kelvin);

    let filtered_rgb_a = blackbody_a * cache.star_a_transmittance;
    let filtered_rgb_b = blackbody_b * cache.star_b_transmittance;

    cache.star_a_color = Color::srgb(filtered_rgb_a.x, filtered_rgb_a.y, filtered_rgb_a.z);
    cache.star_b_color = Color::srgb(filtered_rgb_b.x, filtered_rgb_b.y, filtered_rgb_b.z);

    let t_a = ((effective_elev_a.to_degrees() + 8.0) / 12.0).clamp(0.0, 1.0);
    let cutoff_a = t_a * t_a * (3.0 - 2.0 * t_a);
    let t_b = ((effective_elev_b.to_degrees() + 8.0) / 12.0).clamp(0.0, 1.0);
    let cutoff_b = t_b * t_b * (3.0 - 2.0 * t_b);

    let mean_trans_a = (cache.star_a_transmittance.x + cache.star_a_transmittance.y + cache.star_a_transmittance.z) / 3.0;
    let mean_trans_b = (cache.star_b_transmittance.x + cache.star_b_transmittance.y + cache.star_b_transmittance.z) / 3.0;

    let weather_dimming = match weather.weather_type {
        WeatherType::ClearSky => 1.0,
        WeatherType::AerosolHaze => 0.82,
        WeatherType::StellarWindAurora => 0.95,
        WeatherType::OvercastPrecipitation => 0.35,
    };

    cache.star_a_illuminance_lux = config.star_a_base_illuminance_lux * mean_trans_a * cutoff_a * weather_dimming;
    cache.star_b_illuminance_lux = config.star_b_base_illuminance_lux * mean_trans_b * cutoff_b * weather_dimming;

    // 2. Binary Conjunction Amplification
    if ephemeris.sky_state == DynamicSkyState::BinaryAlignment {
        let align_ratio = 1.0 - (ephemeris.angular_separation / 7.5f32.to_radians()).clamp(0.0, 1.0);
        cache.conjunction_amplification = 1.0 + align_ratio * 0.45;
    } else {
        cache.conjunction_amplification = 1.0;
    }

    // 3. Diffuse Skylight & Zenith / Horizon Radiance Integration
    let zenith_dir = Vec3::Y;
    let cos_theta_za = zenith_dir.dot(ephemeris.star_a_direction);
    let cos_theta_zb = zenith_dir.dot(ephemeris.star_b_direction);

    let phase_a = multi_octave_stellar_glare(cos_theta_za, false);
    let phase_b = multi_octave_stellar_glare(cos_theta_zb, true);

    let sky_scatter_a = config.rayleigh_scattering_coefficients * (cache.star_a_illuminance_lux * 0.000035) * phase_a;
    let sky_scatter_b = config.rayleigh_scattering_coefficients * blackbody_b * (cache.star_b_illuminance_lux * 0.000035) * phase_b;

    let starlight_airglow = Vec3::new(
        config.rayleigh_scattering_coefficients.x * 24.0,
        config.rayleigh_scattering_coefficients.y * 0.08,
        config.rayleigh_scattering_coefficients.z * 18.0,
    ) * 0.08;

    cache.zenith_radiance = (sky_scatter_a + sky_scatter_b + starlight_airglow * 0.5) * cache.conjunction_amplification;

    let total_daylight_lux = cache.star_a_illuminance_lux + cache.star_b_illuminance_lux;
    let day_ratio = (total_daylight_lux / 25_000.0).clamp(0.0, 1.0);
    let smooth_day = day_ratio * day_ratio * (3.0 - 2.0 * day_ratio);

    let horizon_rayleigh = Vec3::new(
        config.rayleigh_scattering_coefficients.x * 2.2,
        config.rayleigh_scattering_coefficients.y * 1.55,
        config.rayleigh_scattering_coefficients.z * 1.0,
    ) * (total_daylight_lux * 0.000038) * smooth_day;

    let elev_deg_a = effective_elev_a.to_degrees();
    let sunset_factor_a = ((20.0 - elev_deg_a) / 20.0).clamp(0.0, 1.0);
    let sunset_mie_weight_a = sunset_factor_a * sunset_factor_a * (3.0 - 2.0 * sunset_factor_a);

    let elev_deg_b = effective_elev_b.to_degrees();
    let sunset_factor_b = ((20.0 - elev_deg_b) / 20.0).clamp(0.0, 1.0);
    let sunset_mie_weight_b = sunset_factor_b * sunset_factor_b * (3.0 - 2.0 * sunset_factor_b);

    let horizon_mie_a = filtered_rgb_a * (cache.star_a_illuminance_lux * 0.000045) * sunset_mie_weight_a;
    let horizon_mie_b = filtered_rgb_b * (cache.star_b_illuminance_lux * 0.000045) * sunset_mie_weight_b;

    let twilight_a = {
        let elev_deg = effective_elev_a.to_degrees();
        let bell = (-((elev_deg + 3.5) / 5.5).powi(2)).exp();
        Vec3::new(
            0.85 * ((-elev_deg * 0.10).exp()).min(1.2),
            0.005,
            0.35 * ((elev_deg + 10.0) / 10.0).clamp(0.0, 1.0),
        ) * bell * (config.star_a_base_illuminance_lux / 50_000.0)
    };
    let twilight_b = {
        let elev_deg = effective_elev_b.to_degrees();
        let bell = (-((elev_deg + 3.5) / 5.5).powi(2)).exp();
        Vec3::new(
            0.65 * ((-elev_deg * 0.10).exp()).min(1.0),
            0.003,
            0.25 * ((elev_deg + 10.0) / 10.0).clamp(0.0, 1.0),
        ) * bell * (config.star_b_base_illuminance_lux / 35_000.0)
    };

    cache.horizon_radiance = (horizon_rayleigh + horizon_mie_a + horizon_mie_b + twilight_a + twilight_b + starlight_airglow * 1.5) * cache.conjunction_amplification;

    // 4. Scene Ambient Light & Ground Bounce Irradiance
    let ambient_rgb = cache.zenith_radiance * 0.60 + cache.horizon_radiance * 0.40;
    let max_c = ambient_rgb.x.max(ambient_rgb.y).max(ambient_rgb.z).max(1e-4);
    
    cache.ambient_color = Color::srgb(
        (ambient_rgb.x / max_c).clamp(0.0, 1.0),
        (ambient_rgb.y / max_c).clamp(0.0, 1.0),
        (ambient_rgb.z / max_c).clamp(0.0, 1.0),
    );

    let total_lux = config.ambient_illuminance_lux.unwrap_or_else(|| {
        ((cache.star_a_illuminance_lux + cache.star_b_illuminance_lux) * 0.0028 + 2.5).clamp(2.5, 360.0)
    });
    cache.ambient_brightness_lux = total_lux;

    let ground_irradiance = ambient_rgb * config.ground_albedo * 0.5;
    cache.ground_bounce_color = Color::srgb(
        ground_irradiance.x.clamp(0.0, 1.0),
        ground_irradiance.y.clamp(0.0, 1.0),
        ground_irradiance.z.clamp(0.0, 1.0),
    );

    // 5. Shadow-Casting Priority Management
    let elev_a_deg = ephemeris.star_a_elevation.to_degrees();
    let elev_b_deg = ephemeris.star_b_elevation.to_degrees();
    cache.active_shadow_caster = determine_shadow_caster_priority(elev_a_deg, elev_b_deg);
}
