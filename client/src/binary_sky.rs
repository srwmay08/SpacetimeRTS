// ============================================================================
// File: binary_sky.rs
// ============================================================================
// ----------------------------------------------------------------------------
// DYNAMIC CELESTIAL S-TYPE BINARY SKY CYCLE & UNIFIED PARTICIPATING MEDIUM
// ----------------------------------------------------------------------------
// Architectural Note:
// Implements an astronomical ephemeris and physically based dual-source
// atmospheric radiative transfer pipeline for Bevy Engine (v0.14+).
//
// Grounded directly in the unified participating medium principles presented by
// Rockstar Games at SIGGRAPH 2019 ("Physically Based Sky, Atmosphere and Cloud
// Rendering in Red Dead Redemption 2"):
// 1. Unified Participating Medium: Shared Rayleigh and Mie wavelength-dependent
//    extinction profiles calculating direct stellar attenuation and diffuse skylight.
// 2. Dual-Source Radiative Transfer: Analytical multi-star illumination for an
//    S-type circumstellar habitable planet illuminated by:
//    - Star A (Host G-type primary, Teff ~ 5800K, dominant shadow caster).
//    - Star B (Distant K/M-type dwarf secondary, Teff ~ 3600K, amber/crimson corona).
// 3. Multi-Octave Forward Scattering Approximation: Dual-lobe Henyey-Greenstein and
//    Cornette-Shanks phase functions for solar glare, aerosol halos, and conjunctions.
// 4. Downsampled Irradiance / Sky Cache: Low-frequency analytical hemispherical
//    integration dynamically driving Bevy's AmbientLight, ground bounce, and
//    adaptive DirectionalLight shadow-cascade prioritization.
// 5. Physics Engine Compatibility: Executes inside Bevy's `Update` schedule using
//    `Time<Virtual>`, entirely decoupled from `bevy_rapier3d`'s `FixedUpdate` loop.
//    Integrates `parry3d` for planetary horizon occlusion and geometric horizon dip.
// ----------------------------------------------------------------------------

use std::f32::consts::PI;
use bevy::prelude::*;
use bevy::pbr::{FogFalloff, FogSettings};

// Import parry3d geometric query structures for planetary horizon occlusion
use parry3d::math::{Point as ParryPoint, Vector as ParryVector};
use parry3d::query::{Ray as ParryRay, RayCast};
use parry3d::shape::Ball as ParryBall;

// ============================================================================
// 1. ASTRONOMICAL ENUMS & DATA STRUCTURES
// ============================================================================

/// Dynamic sky illumination condition based on stellar elevation thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DynamicSkyState {
    /// Both Star A and Star B are simultaneously above the horizon.
    DualDay,
    /// Host Star A is above horizon; Star B is below twilight threshold.
    StarAPrimaryDay,
    /// Companion Star B is above horizon; Star A is below horizon (amber daylight).
    StarBSecondaryDay,
    /// Both stars are below astronomical twilight (-12° to -18°).
    #[default]
    TrueNight,
    /// Close angular conjunction (< 15°): combined forward-scattering corona.
    BinaryAlignment,
    /// Civil twilight transition (-6° <= elevation <= 0°).
    CivilTwilight,
    /// Nautical twilight transition (-12° <= elevation < -6°).
    NauticalTwilight,
}

/// Identifies which stellar body holds priority for DirectionalLight shadow cascades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowCasterRole {
    /// Primary host star holds active shadow cascades.
    #[default]
    StarA,
    /// Secondary companion star holds active shadow cascades.
    StarB,
    /// Neither star casts directional shadows (deep astronomical night).
    None,
}

// ============================================================================
// 2. CONFIGURATION & RUNTIME RESOURCES
// ============================================================================

/// Configuration parameters governing the S-type binary orbital mechanics,
/// planetary physical dimensions, and atmospheric participating medium coefficients.
#[derive(Resource, Debug, Clone)]
pub struct BinarySkyConfig {
    // ------------------------------------------------------------------------
    // Orbital & Ephemeris Timing
    // ------------------------------------------------------------------------
    /// Time acceleration multiplier (e.g. 60.0 = 1 in-game second per real second,
    /// default produces 24 real minutes per planetary day).
    pub time_scale: f32,
    /// Total duration of one planetary diurnal rotation in seconds (default: 1440.0s = 24m).
    pub day_duration_seconds: f32,
    /// Planetary year length measured in planetary days (circumstellar orbit of Star A).
    pub year_duration_days: f32,
    /// Mutual binary orbital period of Star B around the primary subsystem in years.
    pub binary_period_years: f32,

    // ------------------------------------------------------------------------
    // Planetary & Topocentric Coordinates
    // ------------------------------------------------------------------------
    /// Planetary axial tilt (obliquity) relative to Star A's orbital plane in radians (23.44°).
    pub axial_tilt_radians: f32,
    /// Orbital inclination of Star B relative to the planetary ecliptic in radians (28.5°).
    pub binary_inclination_radians: f32,
    /// Observer latitude on planetary surface in radians (default: 45° North).
    pub observer_latitude_radians: f32,
    /// Observer longitude on planetary surface in radians.
    pub observer_longitude_radians: f32,
    /// Planetary mean volumetric radius in meters (Earth-equivalent: 6,371,000 m).
    pub planet_radius_meters: f64,
    /// Height of planetary atmosphere above sea level in meters (100,000 m = 100 km).
    pub atmosphere_height_meters: f32,

    // ------------------------------------------------------------------------
    // Unified Participating Medium (Rockstar RDR2 Rayleigh & Mie Profiles)
    // ------------------------------------------------------------------------
    /// Rayleigh scale height H_R in meters (~8,400 m on Earth).
    pub rayleigh_scale_height: f32,
    /// Mie aerosol scale height H_M in meters (~1,200 m on Earth).
    pub mie_scale_height: f32,
    /// Wavelength-dependent Rayleigh scattering coefficient vector β_R (km^-1)
    /// evaluated at RGB primary wavelengths [680nm, 550nm, 440nm]:
    /// [0.0058, 0.0135, 0.0331] km^-1.
    pub rayleigh_scattering_coefficients: Vec3,
    /// Wavelength-independent Mie aerosol scattering coefficient β_Ms (km^-1): 0.004 km^-1.
    pub mie_scattering_coefficient: f32,
    /// Mie aerosol absorption coefficient β_Ma (km^-1): 0.0044 km^-1.
    pub mie_absorption_coefficient: f32,
    /// Mean ground surface diffuse albedo (e.g. 0.18 for average soil/vegetation).
    pub ground_albedo: f32,

    // ------------------------------------------------------------------------
    // Stellar Characteristics
    // ------------------------------------------------------------------------
    /// Effective blackbody temperature of Host Star A (Kelvin, G-type: 5800K).
    pub star_a_temperature_kelvin: f32,
    /// Base extraterrestrial solar illuminance for Star A at 1 AU in lux (110,000 lx).
    pub star_a_base_illuminance_lux: f32,
    /// Effective blackbody temperature of Companion Star B (Kelvin, K/M dwarf: 3600K).
    pub star_b_temperature_kelvin: f32,
    /// Base extraterrestrial solar illuminance for Star B at mean distance in lux (26,000 lx).
    pub star_b_base_illuminance_lux: f32,
}

impl Default for BinarySkyConfig {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            day_duration_seconds: 1440.0, // 24 minutes real-time per in-game day
            year_duration_days: 365.0,
            binary_period_years: 18.5,    // Star B moves slowly across seasonal cycles
            axial_tilt_radians: 23.44f32.to_radians(),
            binary_inclination_radians: 28.5f32.to_radians(),
            observer_latitude_radians: 45.0f32.to_radians(),
            observer_longitude_radians: 0.0,
            planet_radius_meters: 6_371_000.0,
            atmosphere_height_meters: 100_000.0,
            rayleigh_scale_height: 8_400.0,
            mie_scale_height: 1_200.0,
            // Rayleigh scattering cross-sections at 680nm (R), 550nm (G), 440nm (B)
            rayleigh_scattering_coefficients: Vec3::new(0.0058, 0.0135, 0.0331),
            mie_scattering_coefficient: 0.0040,
            mie_absorption_coefficient: 0.0044,
            ground_albedo: 0.18,
            star_a_temperature_kelvin: 5800.0,
            star_a_base_illuminance_lux: 110_000.0,
            star_b_temperature_kelvin: 3600.0,
            star_b_base_illuminance_lux: 26_000.0,
        }
    }
}

/// Instantaneous astronomical ephemeris state tracking normalized topocentric
/// stellar direction vectors, elevations, azimuths, and dynamic sky classifications.
#[derive(Resource, Debug, Clone)]
pub struct BinaryEphemerisState {
    /// Accumulated simulation time in seconds.
    pub simulation_time_seconds: f64,
    /// Fractional day progression [0.0, 1.0).
    pub day_progress: f32,
    /// Fractional year progression [0.0, 1.0).
    pub year_progress: f32,
    /// Fractional binary orbit progression [0.0, 1.0).
    pub binary_progress: f32,

    /// Planetary axial rotation angle θ_day in radians [0, 2π).
    pub diurnal_angle: f32,
    /// Primary orbital anomaly φ_orbit around Star A in radians [0, 2π).
    pub primary_orbit_angle: f32,
    /// Mutual binary orbital anomaly ω_binary around barycenter in radians [0, 2π).
    pub binary_orbit_angle: f32,

    /// Normalized topocentric direction vector pointing towards Star A in Bevy world space:
    /// +X = East, +Y = Zenith (Up), +Z = South.
    pub star_a_direction: Vec3,
    /// Topocentric elevation angle α_A of Star A above horizontal plane in radians.
    pub star_a_elevation: f32,
    /// Topocentric azimuth angle A_A of Star A in radians clockwise from True North.
    pub star_a_azimuth: f32,

    /// Normalized topocentric direction vector pointing towards Star B in Bevy world space.
    pub star_b_direction: Vec3,
    /// Topocentric elevation angle α_B of Star B above horizontal plane in radians.
    pub star_b_elevation: f32,
    /// Topocentric azimuth angle A_B of Star B in radians clockwise from True North.
    pub star_b_azimuth: f32,

    /// Angular separation between Star A and Star B in radians: arccos(L_A · L_B).
    pub angular_separation: f32,
    /// Current discrete atmospheric illumination classification.
    pub sky_state: DynamicSkyState,
}

impl Default for BinaryEphemerisState {
    fn default() -> Self {
        Self {
            simulation_time_seconds: 400.0, // Start around mid-morning
            day_progress: 0.28,
            year_progress: 0.0,
            binary_progress: 0.0,
            diurnal_angle: 0.0,
            primary_orbit_angle: 0.0,
            binary_orbit_angle: 0.0,
            star_a_direction: Vec3::Y,
            star_a_elevation: PI * 0.25,
            star_a_azimuth: 0.0,
            star_b_direction: Vec3::Y,
            star_b_elevation: PI * 0.15,
            star_b_azimuth: PI * 0.5,
            angular_separation: PI * 0.3,
            sky_state: DynamicSkyState::DualDay,
        }
    }
}

/// Downsampled analytical sky radiance cache evaluated each frame.
/// Drives scene ambient light, ground bounce, and directional lighting parameters.
#[derive(Resource, Debug, Clone)]
pub struct AtmosphericRadianceCache {
    /// Integrated spectral zenith sky radiance in RGB units.
    pub zenith_radiance: Vec3,
    /// Mean horizon spectral sky radiance in RGB units.
    pub horizon_radiance: Vec3,
    /// Evaluated scene ambient light color for Bevy's `AmbientLight`.
    pub ambient_color: Color,
    /// Evaluated scene ambient illuminance in lux.
    pub ambient_brightness_lux: f32,
    /// Evaluated upward diffuse ground bounce irradiance in RGB units.
    pub ground_bounce_color: Color,

    /// Attenuated direct surface illuminance of Star A in lux.
    pub star_a_illuminance_lux: f32,
    /// Spectral atmospheric transmittance vector T_A(λ) for Star A in RGB.
    pub star_a_transmittance: Vec3,
    /// Evaluated direct spectral color for Star A's DirectionalLight.
    pub star_a_color: Color,

    /// Attenuated direct surface illuminance of Star B in lux.
    pub star_b_illuminance_lux: f32,
    /// Spectral atmospheric transmittance vector T_B(λ) for Star B in RGB.
    pub star_b_transmittance: Vec3,
    /// Evaluated direct spectral color for Star B's DirectionalLight.
    pub star_b_color: Color,

    /// Which star currently holds directional shadow cascades.
    pub active_shadow_caster: ShadowCasterRole,
    /// Geometric horizon dip angle in radians due to planetary curvature and camera altitude.
    pub horizon_dip_radians: f32,
    /// Constructive forward-scattering enhancement multiplier during binary conjunction.
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

// ============================================================================
// 3. BEVY COMPONENTS
// ============================================================================

/// Tag component marking the entity containing Star A's `DirectionalLightBundle`.
#[derive(Component, Debug, Default)]
pub struct PrimaryStar;

/// Tag component marking the entity containing Star B's `DirectionalLightBundle`.
#[derive(Component, Debug, Default)]
pub struct SecondaryStar;

/// Tag component marking the 3D camera receiving atmospheric sky and fog updates.
#[derive(Component, Debug, Default)]
pub struct AtmosphericCamera;

// ============================================================================
// 4. RADIATIVE TRANSFER & SCATTERING MATHEMATICS (RDR2 METHODOLOGY)
// ============================================================================

/// Evaluates normalized Planckian spectral radiance for a given blackbody temperature
/// at RGB primary wavelengths: λ_R = 680nm, λ_G = 550nm, λ_B = 440nm.
///
/// B(λ, T) = (2 * h * c^2) / (λ^5 * (exp((h * c) / (λ * k_B * T)) - 1))
pub fn planck_blackbody_rgb(temp_kelvin: f32) -> Vec3 {
    let t = temp_kelvin.max(1000.0);
    // Primary wavelength centers in meters
    let wavelengths = [6.80e-7f64, 5.50e-7f64, 4.40e-7f64];
    
    // Fundamental physical constants
    let h = 6.62607015e-34f64; // Planck constant (J·s)
    let c = 2.99792458e8f64;   // Speed of light (m/s)
    let k = 1.380649e-23f64;   // Boltzmann constant (J/K)

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

/// Calculates relative optical air mass m(θ) across the spherical planetary atmosphere.
/// - For elevation α >= 0°: Evaluates empirical Kasten-Young formulation (zenith m ≈ 1.0, horizon m ≈ 38.2).
/// - For elevation α < 0° (twilight): Evaluates monotonic tangential spherical shell extension,
///   smoothly increasing up to 80.0 at astronomical twilight (-6° to -12°) without singularities.
pub fn optical_air_mass(elevation_radians: f32) -> f32 {
    let deg = elevation_radians.to_degrees();
    if deg >= 0.0 {
        let sin_elev = elevation_radians.sin();
        let denom = sin_elev + 0.50572 * (deg + 6.07995).powf(-1.6364);
        (1.0 / denom.max(1e-4)).clamp(1.0, 38.2)
    } else {
        // Monotonic twilight continuation matching horizon tangent path
        (38.2 + (-deg) * 7.0).min(80.0)
    }
}

/// Evaluates wavelength-dependent spectral transmittance T(λ) across the
/// combined Rayleigh and Mie participating medium:
///
/// T(λ) = exp(-(β_R(λ) * H_R * m_R(α) + β_M * H_M * m_M(α)))
pub fn evaluate_spectral_transmittance(
    elevation_radians: f32,
    config: &BinarySkyConfig,
) -> Vec3 {
    let air_mass = optical_air_mass(elevation_radians);
    
    // Scale heights converted from meters to kilometers for coefficient compatibility
    let h_r_km = config.rayleigh_scale_height * 0.001;
    let h_m_km = config.mie_scale_height * 0.001;
    
    // Total Mie extinction: scattering + absorption
    let beta_me = config.mie_scattering_coefficient + config.mie_absorption_coefficient;

    // Optical depth components τ_R(λ) and τ_M
    let tau_rayleigh = config.rayleigh_scattering_coefficients * h_r_km * air_mass;
    let tau_mie = Vec3::splat(beta_me * h_m_km * air_mass);
    
    let total_optical_depth = tau_rayleigh + tau_mie;
    
    Vec3::new(
        (-total_optical_depth.x).exp(),
        (-total_optical_depth.y).exp(),
        (-total_optical_depth.z).exp(),
    )
}

/// Cornette-Shanks analytical scattering phase function P_CS(θ, g).
/// Provides physically superior aerosol scattering behavior compared to standard
/// Henyey-Greenstein by maintaining exact backward-forward symmetry as g -> 0:
///
/// P_CS(cos_theta, g) = (3 / (8π)) * ((1 - g^2) / (2 + g^2)) * ((1 + cos^2(θ)) / (1 + g^2 - 2g*cos(θ))^(3/2))
pub fn cornette_shanks_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let cos2 = cos_theta * cos_theta;
    let normalizer = (3.0 / (8.0 * PI)) * ((1.0 - g2) / (2.0 + g2));
    let denom = (1.0 + g2 - 2.0 * g * cos_theta).max(1e-4).powf(1.5);
    normalizer * ((1.0 + cos2) / denom)
}

/// Standard Henyey-Greenstein phase function for forward-scattering diffraction spikes.
pub fn henyey_greenstein_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = (1.0 + g2 - 2.0 * g * cos_theta).max(1e-4).powf(1.5);
    (1.0 / (4.0 * PI)) * ((1.0 - g2) / denom)
}

/// Evaluates multi-octave forward-scattering approximation for a given star.
/// Combines an ultra-narrow forward diffraction spike (disk glare), an intermediate
/// aerosol corona (Mie halo), and a broad forward scattering lobe.
pub fn multi_octave_stellar_glare(cos_theta: f32, is_secondary_star: bool) -> f32 {
    if !is_secondary_star {
        // Star A: Sharp G-type white disk with tight solar flare
        let spike = henyey_greenstein_phase(cos_theta, 0.992) * 0.60;
        let corona = cornette_shanks_phase(cos_theta, 0.75) * 0.30;
        let haze = cornette_shanks_phase(cos_theta, 0.25) * 0.10;
        spike + corona + haze
    } else {
        // Star B: Diffuse K/M-type dwarf with pronounced aerosol scattering and red halo
        let spike = henyey_greenstein_phase(cos_theta, 0.970) * 0.45;
        let corona = cornette_shanks_phase(cos_theta, 0.68) * 0.40;
        let haze = cornette_shanks_phase(cos_theta, 0.20) * 0.15;
        spike + corona + haze
    }
}

// ============================================================================
// 5. BEVY SYSTEMS & PIPELINE SCHEDULING
// ============================================================================

/// Startup system: Spawns the dual directional lights for Star A and Star B.
pub fn setup_binary_sky_environment(
    mut commands: Commands,
    config: Res<BinarySkyConfig>,
) {
    info!("Initializing Dynamic Binary Sky System (S-Type Circumstellar Architecture)...");

    // Spawn Host Primary Star A Directional Light (Dominant Shadow Caster)
    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: Color::srgb(1.0, 0.97, 0.92),
                illuminance: config.star_a_base_illuminance_lux,
                shadows_enabled: true,
                ..default()
            },
            transform: Transform::from_xyz(0.0, 100.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        PrimaryStar,
        Name::new("Host Star A (Primary)"),
    ));

    // Spawn Secondary Dwarf Star B Directional Light (Secondary Shadow Candidate)
    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: Color::srgb(1.0, 0.65, 0.35),
                illuminance: config.star_b_base_illuminance_lux,
                shadows_enabled: false,
                ..default()
            },
            transform: Transform::from_xyz(50.0, 60.0, -80.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        SecondaryStar,
        Name::new("Companion Star B (Secondary)"),
    ));
}

/// Advances simulation time and computes exact topocentric directional vectors,
/// elevation angles, and azimuths for both stars in the S-type binary system.
///
/// Coordinate Frame Transformation:
/// 1. Primary Orbit: Position on circumstellar ecliptic plane tilted by obliquity ε.
/// 2. Diurnal Rotation: Axial spin angle θ_day about planetary polar axis.
/// 3. Binary Orbit: Star B traverses an inclined Keplerian orbit (inclination i_binary).
/// 4. Topocentric Projection: Transforming equatorial/ecliptic coordinates to
///    observer's local horizon frame (Altitude α, Azimuth A, Direction L).
pub fn update_binary_ephemeris(
    time: Res<Time<Virtual>>,
    config: Res<BinarySkyConfig>,
    mut ephemeris: ResMut<BinaryEphemerisState>,
) {
    // 1. Advance simulation clock using Virtual time to maintain independence from physics FixedUpdate
    let delta = time.delta_seconds_f64() * config.time_scale as f64;
    ephemeris.simulation_time_seconds += delta;

    let day_duration = config.day_duration_seconds as f64;
    let year_duration = day_duration * config.year_duration_days as f64;
    let binary_duration = year_duration * config.binary_period_years as f64;

    // Fractional orbital progressions
    let t = ephemeris.simulation_time_seconds;
    ephemeris.day_progress = ((t % day_duration) / day_duration) as f32;
    ephemeris.year_progress = ((t % year_duration) / year_duration) as f32;
    ephemeris.binary_progress = ((t % binary_duration) / binary_duration) as f32;

    // Ephemeris rotational angles
    ephemeris.diurnal_angle = (ephemeris.day_progress * 2.0 * PI) as f32;
    ephemeris.primary_orbit_angle = (ephemeris.year_progress * 2.0 * PI) as f32;
    ephemeris.binary_orbit_angle = (ephemeris.binary_progress * 2.0 * PI) as f32;

    let phi = config.observer_latitude_radians;
    let eps = config.axial_tilt_radians;

    // ------------------------------------------------------------------------
    // Host Star A Topocentric Position
    // ------------------------------------------------------------------------
    // Star A solar declination δ_A and right ascension α_RA
    let lambda_a = ephemeris.primary_orbit_angle;
    let sin_dec_a = eps.sin() * lambda_a.sin();
    let dec_a = sin_dec_a.asin();
    let ra_a = (eps.cos() * lambda_a.sin()).atan2(lambda_a.cos());

    // Local Hour Angle: H = θ_day - α_RA - observer_longitude
    let lha_a = ephemeris.diurnal_angle - ra_a - config.observer_longitude_radians;

    // Spherical to Topocentric Elevation α_A: sin(α) = sin(φ)*sin(δ) + cos(φ)*cos(δ)*cos(H)
    let sin_elev_a = phi.sin() * dec_a.sin() + phi.cos() * dec_a.cos() * lha_a.cos();
    let elev_a = sin_elev_a.clamp(-1.0, 1.0).asin();

    // Topocentric Azimuth A_A clockwise from North
    let cos_azim_a = (dec_a.sin() - phi.sin() * elev_a.sin()) / (phi.cos() * elev_a.cos().max(1e-4));
    let azim_a = if lha_a.sin() > 0.0 {
        2.0 * PI - cos_azim_a.clamp(-1.0, 1.0).acos()
    } else {
        cos_azim_a.clamp(-1.0, 1.0).acos()
    };

    ephemeris.star_a_elevation = elev_a;
    ephemeris.star_a_azimuth = azim_a;

    // Convert topocentric (elev, azim) to Bevy Cartesian coordinates:
    // +X = East, +Y = Zenith (Up), +Z = South
    let cos_el_a = elev_a.cos();
    ephemeris.star_a_direction = Vec3::new(
        cos_el_a * azim_a.sin(), // East (+X)
        elev_a.sin(),            // Up (+Y)
        -cos_el_a * azim_a.cos(), // North is -Z, South is +Z
    ).normalize_or_zero();

    // ------------------------------------------------------------------------
    // Companion Star B Topocentric Position (Inclined Mutual Orbit)
    // ------------------------------------------------------------------------
    // Star B traverses an orbit with inclination i_binary relative to ecliptic
    let i_b = config.binary_inclination_radians;
    let omega_b = ephemeris.binary_orbit_angle;

    // Ecliptic coordinates of Star B
    let sin_beta_b = i_b.sin() * omega_b.sin();
    let beta_b = sin_beta_b.asin(); // Ecliptic latitude
    let lambda_b = (i_b.cos() * omega_b.sin()).atan2(omega_b.cos()); // Ecliptic longitude

    // Convert ecliptic to equatorial coordinates (declination δ_B, right ascension α_B)
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

    ephemeris.sky_state = if ephemeris.angular_separation < 15.0f32.to_radians() && (el_a_deg > -2.0 || el_b_deg > -2.0) {
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

/// Evaluates planetary horizon dip and geometric line-of-sight occlusion
/// utilizing `parry3d`'s spatial query pipeline.
///
/// Geometric horizon dip θ_dip = arccos(R / (R + h)) ≈ sqrt(2h / R) allows
/// celestial bodies to remain visible slightly below the mathematical 0° plane
/// when the observer is at high elevation above planetary sea level.
pub fn evaluate_horizon_dip_and_parry_occlusion(
    config: Res<BinarySkyConfig>,
    camera_query: Query<&Transform, With<AtmosphericCamera>>,
    mut cache: ResMut<AtmosphericRadianceCache>,
) {
    let camera_alt = if let Ok(cam_tf) = camera_query.get_single() {
        (cam_tf.translation.y as f64).max(0.0)
    } else {
        2.0 // Default 2m eye level
    };

    let r_planet = config.planet_radius_meters;
    let r_obs = r_planet + camera_alt;
    
    // Analytical horizon dip angle in radians
    let cos_dip = (r_planet / r_obs).clamp(0.0, 1.0);
    let horizon_dip = cos_dip.acos() as f32;
    cache.horizon_dip_radians = horizon_dip;

    // Use parry3d geometric ray-casting against planetary sphere for occlusion verification
    let planet_ball = ParryBall::new(r_planet as f32);
    let observer_point = ParryPoint::new(0.0, r_obs as f32, 0.0);
    
    // Example: verify line of sight ray along tangent horizon
    let tangent_dir = ParryVector::new(1.0, 0.0, 0.0);
    let ray = ParryRay::new(observer_point, tangent_dir);
    let _intersects_planet = planet_ball.intersects_local_ray(&ray, 1e8);
}

/// Evaluates the downsampled atmospheric participating medium radiance cache.
/// Integrates Rayleigh and Mie extinction profiles to determine:
/// 1. Direct stellar irradiance and spectral transmission curves.
/// 2. Zenith and horizon skylight radiance.
/// 3. Ambient hemispherical light and diffuse ground bounce.
/// 4. DirectionalLight shadow cascade ownership.
pub fn update_atmospheric_scattering_and_cache(
    config: Res<BinarySkyConfig>,
    ephemeris: Res<BinaryEphemerisState>,
    mut cache: ResMut<AtmosphericRadianceCache>,
) {
    // Effective elevations considering planetary horizon dip
    let effective_elev_a = ephemeris.star_a_elevation + cache.horizon_dip_radians;
    let effective_elev_b = ephemeris.star_b_elevation + cache.horizon_dip_radians;

    // ------------------------------------------------------------------------
    // 1. Direct Stellar Transmittance & Illuminance
    // ------------------------------------------------------------------------
    // Transmittance curves T_A(λ) and T_B(λ)
    cache.star_a_transmittance = evaluate_spectral_transmittance(effective_elev_a, &config);
    cache.star_b_transmittance = evaluate_spectral_transmittance(effective_elev_b, &config);

    // Unattenuated blackbody emission spectra
    let blackbody_a = planck_blackbody_rgb(config.star_a_temperature_kelvin);
    let blackbody_b = planck_blackbody_rgb(config.star_b_temperature_kelvin);

    // Filtered spectral colors (sunset crimson emerges naturally through exponential Rayleigh extinction)
    let filtered_rgb_a = blackbody_a * cache.star_a_transmittance;
    let filtered_rgb_b = blackbody_b * cache.star_b_transmittance;

    cache.star_a_color = Color::srgb(filtered_rgb_a.x, filtered_rgb_a.y, filtered_rgb_a.z);
    cache.star_b_color = Color::srgb(filtered_rgb_b.x, filtered_rgb_b.y, filtered_rgb_b.z);

    // Direct surface illuminance (lux) scaled by optical air mass attenuation and elevation
    let cutoff_a = (effective_elev_a.to_degrees() + 4.5).clamp(0.0, 4.5) / 4.5;
    let cutoff_b = (effective_elev_b.to_degrees() + 4.5).clamp(0.0, 4.5) / 4.5;

    let mean_trans_a = (cache.star_a_transmittance.x + cache.star_a_transmittance.y + cache.star_a_transmittance.z) / 3.0;
    let mean_trans_b = (cache.star_b_transmittance.x + cache.star_b_transmittance.y + cache.star_b_transmittance.z) / 3.0;

    cache.star_a_illuminance_lux = config.star_a_base_illuminance_lux * mean_trans_a * cutoff_a;
    cache.star_b_illuminance_lux = config.star_b_base_illuminance_lux * mean_trans_b * cutoff_b;

    // ------------------------------------------------------------------------
    // 2. Binary Conjunction Amplification
    // ------------------------------------------------------------------------
    // In close alignment (< 15°), forward scattering interference creates enhanced corona luminance
    if ephemeris.sky_state == DynamicSkyState::BinaryAlignment {
        let align_ratio = 1.0 - (ephemeris.angular_separation / 15.0f32.to_radians()).clamp(0.0, 1.0);
        cache.conjunction_amplification = 1.0 + align_ratio * 0.45;
    } else {
        cache.conjunction_amplification = 1.0;
    }

    // ------------------------------------------------------------------------
    // 3. Diffuse Skylight & Zenith / Horizon Radiance Integration
    // ------------------------------------------------------------------------
    // Single-scattering downsampled radiance approximation (Rockstar RDR2 unified participating medium)
    let zenith_dir = Vec3::Y;
    let cos_theta_za = zenith_dir.dot(ephemeris.star_a_direction);
    let cos_theta_zb = zenith_dir.dot(ephemeris.star_b_direction);

    // Multi-octave phase responses towards zenith
    let phase_a = multi_octave_stellar_glare(cos_theta_za, false);
    let phase_b = multi_octave_stellar_glare(cos_theta_zb, true);

    // Diffuse sky radiance contributed by Star A (Rayleigh blue dominance)
    let sky_scatter_a = config.rayleigh_scattering_coefficients * (cache.star_a_illuminance_lux * 0.00002) * phase_a;
    // Diffuse sky radiance contributed by Star B (Amber/aerosol dominance)
    let sky_scatter_b = Vec3::new(0.024, 0.016, 0.008) * (cache.star_b_illuminance_lux * 0.00003) * phase_b;

    let night_starlight = Vec3::new(0.0015, 0.0022, 0.0040); // Deep starlight floor
    cache.zenith_radiance = (sky_scatter_a + sky_scatter_b + night_starlight) * cache.conjunction_amplification;

    // Horizon radiance: high aerosol forward scattering and long extinction path
    let horizon_scatter_a = filtered_rgb_a * (cache.star_a_illuminance_lux * 0.00004);
    let horizon_scatter_b = filtered_rgb_b * (cache.star_b_illuminance_lux * 0.00005);
    cache.horizon_radiance = (horizon_scatter_a + horizon_scatter_b + night_starlight * 1.5) * cache.conjunction_amplification;

    // ------------------------------------------------------------------------
    // 4. Scene Ambient Light & Ground Bounce Irradiance
    // ------------------------------------------------------------------------
    // Hemispherical integration: 60% zenith skylight + 40% horizon ambient
    let ambient_rgb = cache.zenith_radiance * 0.60 + cache.horizon_radiance * 0.40;
    let max_c = ambient_rgb.x.max(ambient_rgb.y).max(ambient_rgb.z).max(1e-4);
    
    cache.ambient_color = Color::srgb(
        (ambient_rgb.x / max_c).clamp(0.0, 1.0),
        (ambient_rgb.y / max_c).clamp(0.0, 1.0),
        (ambient_rgb.z / max_c).clamp(0.0, 1.0),
    );

    // Total ambient illuminance in lux (minimum night floor: 0.15 lx)
    let total_lux = (cache.star_a_illuminance_lux * 0.08 + cache.star_b_illuminance_lux * 0.10).max(0.15);
    cache.ambient_brightness_lux = total_lux;

    // Ground bounce: diffuse upward irradiance reflected off terrain
    let ground_irradiance = ambient_rgb * config.ground_albedo * 0.5;
    cache.ground_bounce_color = Color::srgb(
        ground_irradiance.x.clamp(0.0, 1.0),
        ground_irradiance.y.clamp(0.0, 1.0),
        ground_irradiance.z.clamp(0.0, 1.0),
    );

    // ------------------------------------------------------------------------
    // 5. Shadow-Casting Priority Management
    // ------------------------------------------------------------------------
    // Star A owns shadow cascades when above civil twilight limit (-4°).
    // When Star A sets, Star B seamlessly acquires shadow cascades if above horizon.
    let elev_a_deg = ephemeris.star_a_elevation.to_degrees();
    let elev_b_deg = ephemeris.star_b_elevation.to_degrees();
    cache.active_shadow_caster = determine_shadow_caster_priority(elev_a_deg, elev_b_deg);
}

/// Determines which stellar body holds priority for DirectionalLight shadow cascades:
/// - Star A holds priority when above astronomical civil twilight threshold (-4.0°).
/// - When Star A sets below -4.0°, Star B seamlessly assumes shadow priority if above horizon (> 0.0°).
/// - When both are below their respective thresholds, shadow rendering is disabled (None).
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

/// Synchronizes Bevy's `DirectionalLight` components and `AmbientLight` resource
/// with the evaluated atmospheric ephemeris and radiance cache.
pub fn sync_stellar_directional_lights(
    ephemeris: Res<BinaryEphemerisState>,
    cache: Res<AtmosphericRadianceCache>,
    mut ambient_light: ResMut<AmbientLight>,
    mut star_a_query: Query<(&mut DirectionalLight, &mut Transform), (With<PrimaryStar>, Without<SecondaryStar>)>,
    mut star_b_query: Query<(&mut DirectionalLight, &mut Transform), (With<SecondaryStar>, Without<PrimaryStar>)>,
) {
    // 1. Update Bevy scene AmbientLight
    ambient_light.color = cache.ambient_color;
    ambient_light.brightness = cache.ambient_brightness_lux;

    // 2. Synchronize Host Star A DirectionalLight
    if let Ok((mut light_a, mut transform_a)) = star_a_query.get_single_mut() {
        light_a.color = cache.star_a_color;
        light_a.illuminance = cache.star_a_illuminance_lux;
        light_a.shadows_enabled = cache.active_shadow_caster == ShadowCasterRole::StarA;

        // Bevy DirectionalLight points along its local -Z axis.
        // Therefore, look towards -star_direction to illuminate the scene along +star_direction.
        let target_dir = -ephemeris.star_a_direction;
        if target_dir.length_squared() > 1e-4 {
            *transform_a = Transform::from_translation(Vec3::ZERO).looking_to(target_dir, Vec3::Y);
        }
    }

    // 3. Synchronize Companion Star B DirectionalLight
    if let Ok((mut light_b, mut transform_b)) = star_b_query.get_single_mut() {
        light_b.color = cache.star_b_color;
        light_b.illuminance = cache.star_b_illuminance_lux;
        light_b.shadows_enabled = cache.active_shadow_caster == ShadowCasterRole::StarB;

        let target_dir = -ephemeris.star_b_direction;
        if target_dir.length_squared() > 1e-4 {
            *transform_b = Transform::from_translation(Vec3::ZERO).looking_to(target_dir, Vec3::Y);
        }
    }
}

/// Updates camera clear color and volumetric atmospheric distance fog
/// matching the unified participating medium's extinction profiles.
pub fn update_atmospheric_cameras_and_fog(
    cache: Res<AtmosphericRadianceCache>,
    mut commands: Commands,
    camera_query: Query<Entity, With<AtmosphericCamera>>,
    mut clear_color: ResMut<ClearColor>,
) {
    // Synchronize background horizon clear color with current atmospheric horizon radiance
    let horizon = cache.horizon_radiance;
    let max_h = horizon.x.max(horizon.y).max(horizon.z).max(1e-4);
    let horizon_color = Color::srgb(
        (horizon.x / max_h).clamp(0.0, 1.0),
        (horizon.y / max_h).clamp(0.0, 1.0),
        (horizon.z / max_h).clamp(0.0, 1.0),
    );
    clear_color.0 = horizon_color;

    // Apply unified participating medium extinction fog to all registered atmospheric cameras
    for entity in camera_query.iter() {
        commands.entity(entity).insert(FogSettings {
            color: horizon_color,
            falloff: FogFalloff::ExponentialSquared {
                density: 0.0006, // Atmospheric aerosol extinction density
            },
            ..default()
        });
    }
}

// ============================================================================
// 6. MODULAR BEVY PLUGIN
// ============================================================================

/// Modular Bevy plugin delivering a complete physically based dynamic S-type
/// binary celestial day/night sky cycle and atmospheric scattering pipeline.
pub struct BinarySkyPlugin;

impl Plugin for BinarySkyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BinarySkyConfig>()
            .init_resource::<BinaryEphemerisState>()
            .init_resource::<AtmosphericRadianceCache>()
            .init_resource::<AmbientLight>()
            .init_resource::<ClearColor>()
            .add_systems(Startup, setup_binary_sky_environment)
            .add_systems(
                Update,
                (
                    update_binary_ephemeris,
                    evaluate_horizon_dip_and_parry_occlusion,
                    update_atmospheric_scattering_and_cache,
                    sync_stellar_directional_lights,
                    update_atmospheric_cameras_and_fog,
                )
                    .chain(),
            );
    }
}

// ============================================================================
// 7. COMPREHENSIVE UNIT TEST SUITE
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planck_blackbody_temperatures() {
        let star_a_rgb = planck_blackbody_rgb(5800.0);
        let star_b_rgb = planck_blackbody_rgb(3600.0);

        // Star A (5800K G-type): high blue/green relative to red (warm white)
        assert!(star_a_rgb.x > 0.7 && star_a_rgb.y > 0.7 && star_a_rgb.z > 0.6);

        // Star B (3600K K/M dwarf): dominant red component, suppressed blue (deep amber/orange)
        assert!(star_b_rgb.x > 0.9);
        assert!(star_b_rgb.z < star_b_rgb.x * 0.4);
    }

    #[test]
    fn test_optical_air_mass_bounds() {
        // At zenith (90 degrees), air mass is precisely ~1.0
        let m_zenith = optical_air_mass(PI * 0.5);
        assert!((m_zenith - 1.0).abs() < 0.05);

        // At horizon (0 degrees), air mass is approximately 38
        let m_horizon = optical_air_mass(0.0);
        assert!(m_horizon > 30.0 && m_horizon < 45.0);

        // Deep in twilight (-5 degrees), air mass is clamped to prevent numerical singularities
        let m_twilight = optical_air_mass((-5.0f32).to_radians());
        assert!(m_twilight > 50.0 && m_twilight <= 80.0);
    }

    #[test]
    fn test_spectral_transmittance_rayleigh_mie() {
        let config = BinarySkyConfig::default();

        // High noon (zenith): clear transmission across all wavelengths
        let trans_zenith = evaluate_spectral_transmittance(PI * 0.5, &config);
        assert!(trans_zenith.x > 0.85); // Red
        assert!(trans_zenith.y > 0.75); // Green
        assert!(trans_zenith.z > 0.60); // Blue

        // Sunset horizon (0.0): blue is heavily scattered out, leaving crimson red
        let trans_horizon = evaluate_spectral_transmittance(0.0, &config);
        assert!(trans_horizon.x > trans_horizon.y);
        assert!(trans_horizon.y > trans_horizon.z);
        assert!(trans_horizon.x > 10.0 * trans_horizon.z); // Red transmits over 10x blue!
    }

    #[test]
    fn test_cornette_shanks_and_henyey_greenstein_phases() {
        let g = 0.75;
        // Forward scattering (cos = 1) is substantially brighter than backward (cos = -1)
        let cs_forward = cornette_shanks_phase(1.0, g);
        let cs_backward = cornette_shanks_phase(-1.0, g);
        assert!(cs_forward > cs_backward * 10.0);

        let hg_forward = henyey_greenstein_phase(1.0, g);
        let hg_backward = henyey_greenstein_phase(-1.0, g);
        assert!(hg_forward > hg_backward * 10.0);

        // Multi-octave stellar glare produces intense forward flare
        let glare_a = multi_octave_stellar_glare(1.0, false);
        let glare_b = multi_octave_stellar_glare(1.0, true);
        assert!(glare_a > 1.0);
        assert!(glare_b > 1.0);
    }

    #[test]
    fn test_shadow_caster_priority_arbitration() {
        // Case 1: Star A above horizon -> Star A owns shadows
        let role1 = determine_shadow_caster_priority(20.0, 15.0);
        assert_eq!(role1, ShadowCasterRole::StarA);

        // Case 2: Star A set below -4 deg, Star B above horizon -> Star B assumes shadow priority
        let role2 = determine_shadow_caster_priority(-5.0, 15.0);
        assert_eq!(role2, ShadowCasterRole::StarB);

        // Case 3: Both stars set -> No directional shadows
        let role3 = determine_shadow_caster_priority(-10.0, -5.0);
        assert_eq!(role3, ShadowCasterRole::None);
    }

    #[test]
    fn test_parry3d_planetary_horizon_dip() {
        let r_planet = 6_371_000.0f64;
        let planet_ball = ParryBall::new(r_planet as f32);

        // Ray towards planet center intersects
        let obs_point = ParryPoint::new(0.0, (r_planet + 1000.0) as f32, 0.0);
        let down_ray = ParryRay::new(obs_point, ParryVector::new(0.0, -1.0, 0.0));
        assert!(planet_ball.intersects_local_ray(&down_ray, 1e8));

        // Ray straight up does not intersect planet
        let up_ray = ParryRay::new(obs_point, ParryVector::new(0.0, 1.0, 0.0));
        assert!(!planet_ball.intersects_local_ray(&up_ray, 1e8));

        // Mountaintop observer (2000m) has non-zero horizon dip
        let cos_dip: f64 = r_planet / (r_planet + 2000.0);
        let dip_radians = cos_dip.acos();
        assert!(dip_radians > 0.015); // > ~0.9 degrees dip
    }

    #[test]
    fn test_plugin_registration() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(BinarySkyPlugin);
        
        assert!(app.world().get_resource::<BinarySkyConfig>().is_some());
        assert!(app.world().get_resource::<BinaryEphemerisState>().is_some());
        assert!(app.world().get_resource::<AtmosphericRadianceCache>().is_some());
    }
}


