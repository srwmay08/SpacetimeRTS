// ============================================================================
// File: client/src/binary_stars.rs
// ============================================================================
// ----------------------------------------------------------------------------
// S-TYPE BINARY STAR SYSTEM & ASYMMETRIC CASCADED SHADOW MAPPING (CSM) PIPELINE
// ----------------------------------------------------------------------------
// Architected for Bevy Engine (v0.14+).
//
// 1. ORBITAL MECHANICS (S-Type Circumstellar Architecture):
//    - Primary Star (Host Star A): The dominant solar host orbited by the player's planet.
//      Governs standard planetary diurnal axial rotation (uniform, consistent day/night cycle).
//    - Secondary Star (Companion Star B): An amber dwarf orbiting Star A at a wide orbital radius
//      (e.g., 28 AU) along an inclined mutual plane. Traverses the planetary sky at its own slower,
//      inclined orbital cadence, yielding realistic S-type orbital geometry:
//        * Conjunctions / eclipses when the two stars align.
//        * Dual-sun days with overlapping colored shadows and double penumbras.
//        * Opposite-horizon rises (Star B rising as Star A sets), casting warm amber starlight.
//        * Variable dusk/dawn twilight overlaps.
//    - Real-time orbital progression system advancing planetary diurnal rotation and binary orbit.
//
// 2. SHADOW MAP & CASCADED SHADOW MAPPING (CSM) ARCHITECTURE:
//    - Global Shadow Atlas Memory: Configures `DirectionalLightShadowMap { size: 4096 }`
//      to prevent cascade texel starvation across multiple active lights.
//    - Asymmetric Cascade Optimization (`CascadeShadowConfigBuilder`):
//      * Primary Star (Star A): Higher fidelity tier:
//        3 cascades, maximum_distance 160.0m, first_cascade_far_bound 15.0m for sharp near-field contact.
//      * Secondary Star (Star B): Asymmetric performance tier:
//        2 cascades, maximum_distance 75.0m, first_cascade_far_bound 12.0m.
//        Cuts secondary light rasterization draw calls by 33% relative to 3 cascades while
//        preserving crisp dual colored shadow silhouettes and double penumbras in the player's vicinity!
//    - Simultaneous dual shadow casting: When both stars are above the horizon, both lights cast shadows
//      simultaneously, producing distinct double penumbras and dual colored shadows (warm white vs amber).
//
// 3. COMPONENTS & RESOURCE ARCHITECTURE:
//    - Marker components: `PrimaryStar` and `SecondaryStar`.
//    - Data component: `StellarOrbitalData`.
//    - Config resource: `BinaryOrbitConfig` (controlling day length, binary period, inclination,
//      separation, colors, lux, shadow toggles, and cascade configurations).
//
// CONFIGURATION CALLOUTS:
// - Lines ~75-155:  `BinaryOrbitConfig` struct and default orbital parameters.
// - Lines ~160-210: `StellarOrbitalData` component and helper methods.
// - Lines ~220-305: `setup_binary_star_system` (Asymmetric `CascadeShadowConfigBuilder` setups).
// - Lines ~315-420: `update_binary_star_orbital_progression` (S-type orbital vector math).
// - Lines ~425-485: `sync_binary_star_shadows` (Dual shadow-casting synchronization).
// ============================================================================

use std::f32::consts::PI;
use bevy::prelude::*;
use bevy::pbr::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};

// ============================================================================
// 1. CONFIGURATION RESOURCE & DATA STRUCTURES
// ============================================================================

/// Global orbital mechanics, illumination, and cascaded shadow mapping configuration
/// for the S-type circumstellar binary star system.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct BinaryOrbitConfig {
    // ------------------------------------------------------------------------
    // Section 1.1: Planetary Diurnal Rotation (Star A Host Star Cycle)
    // ------------------------------------------------------------------------
    /// Planet diurnal rotation period in real-time seconds (default: 1440.0s = 24.0 min, or 60.0s in demo).
    /// Line Callout: Modify this to change the overall length of a full day/night cycle.
    pub day_duration_seconds: f32,

    /// Planetary axial tilt / obliquity in radians (default: 12.5° = 0.218 rad).
    /// Produces seasonal solar declination oscillation and solar altitude changes.
    pub planet_axial_tilt_radians: f32,

    /// Observer latitude on the planet in radians (default: 34.0° North = 0.593 rad).
    pub observer_latitude_radians: f32,

    /// Observer longitude on the planet in radians (default: 0.0 rad = Prime Meridian).
    pub observer_longitude_radians: f32,

    // ------------------------------------------------------------------------
    // Section 1.2: S-Type Binary Companion (Star B Mutual Orbit)
    // ------------------------------------------------------------------------
    /// Binary mutual orbital period expressed in planetary days (default: 16.0 days).
    /// In an S-type system, the secondary star orbits at wide separation with a much
    /// lower angular velocity than the planet's daily rotation.
    /// Line Callout: Tuning this changes how frequently the companion star traverses the sky.
    pub binary_orbital_period_days: f32,

    /// Inclination of Star B's mutual orbit relative to the planet's ecliptic plane (default: 24.0° = 0.419 rad).
    /// Creates distinct ground-track trajectories and opposite-sky rises.
    pub binary_inclination_radians: f32,

    /// Mutual orbital semi-major axis in Astronomical Units (AU) (default: 28.0 AU).
    pub binary_separation_au: f32,

    // ------------------------------------------------------------------------
    // Section 1.3: Time Progression & Scrubbing
    // ------------------------------------------------------------------------
    /// Global time scaling multiplier (0.0 = paused, 1.0 = real-time, 10.0 = fast, 60.0 = timelapse).
    pub time_scale: f32,

    /// Accumulated simulation time in seconds from epoch (0.0 = High Noon at Prime Meridian).
    pub simulation_time_seconds: f64,

    // ------------------------------------------------------------------------
    // Section 1.4: Primary Star (Star A) Lighting & Shadow Fidelity Profile
    // ------------------------------------------------------------------------
    /// Chromaticity color of Host Star A (default: warm solar white Color::srgb(1.0, 0.96, 0.88)).
    pub star_a_color: Color,

    /// Baseline high-noon illuminance in lux (default: 100,000 lx).
    pub star_a_base_illuminance_lux: f32,

    /// Master runtime toggle for Star A directional shadows (default: true).
    pub star_a_shadows_enabled: bool,

    /// Number of CSM cascades for Star A (default: 3).
    pub star_a_num_cascades: usize,

    /// Maximum shadow distance for Star A in meters (default: 160.0m).
    pub star_a_maximum_shadow_distance: f32,

    /// First cascade far bound for Star A in meters (default: 15.0m for near-field contact shadows).
    pub star_a_first_cascade_far_bound: f32,

    // ------------------------------------------------------------------------
    // Section 1.5: Secondary Star (Star B) Lighting & Asymmetric CSM Profile
    // ------------------------------------------------------------------------
    /// Chromaticity color of Companion Star B (default: amber dwarf Color::srgb(1.0, 0.58, 0.24)).
    pub star_b_color: Color,

    /// Baseline noon illuminance in lux (default: 35,000 lx).
    pub star_b_base_illuminance_lux: f32,

    /// Master runtime toggle for Star B directional shadows (default: true).
    pub star_b_shadows_enabled: bool,

    /// Number of CSM cascades for Star B (default: 2 - asymmetric optimization).
    /// Line Callout: Setting this to 2 saves ~33% of shadow rasterization passes on Star B.
    pub star_b_num_cascades: usize,

    /// Maximum shadow distance for Star B in meters (default: 75.0m - tightened for performance).
    pub star_b_maximum_shadow_distance: f32,

    /// First cascade far bound for Star B in meters (default: 12.0m).
    pub star_b_first_cascade_far_bound: f32,
}

impl Default for BinaryOrbitConfig {
    fn default() -> Self {
        Self {
            day_duration_seconds: 1440.0,
            planet_axial_tilt_radians: 12.5f32.to_radians(),
            observer_latitude_radians: 34.0f32.to_radians(),
            observer_longitude_radians: 0.0,

            binary_orbital_period_days: 16.0,
            binary_inclination_radians: 24.0f32.to_radians(),
            binary_separation_au: 28.0,

            time_scale: 1.0,
            simulation_time_seconds: 0.0, // Initial state: 12:00 High Noon

            star_a_color: Color::srgb(1.0, 0.96, 0.88),
            star_a_base_illuminance_lux: 100_000.0,
            star_a_shadows_enabled: true,
            star_a_num_cascades: 3,
            star_a_maximum_shadow_distance: 160.0,
            star_a_first_cascade_far_bound: 15.0,

            star_b_color: Color::srgb(1.0, 0.58, 0.24),
            star_b_base_illuminance_lux: 35_000.0,
            star_b_shadows_enabled: true,
            star_b_num_cascades: 2,
            star_b_maximum_shadow_distance: 75.0,
            star_b_first_cascade_far_bound: 12.0,
        }
    }
}

impl BinaryOrbitConfig {
    /// Helper: Converts current diurnal angle into local planetary clock time in hours (0.0 to 24.0).
    #[inline]
    pub fn clock_time_hours(&self) -> f32 {
        let day_sec = self.day_duration_seconds as f64;
        let elapsed = self.simulation_time_seconds.rem_euclid(day_sec);
        // Epoch 0.0 corresponds to High Noon (12:00)
        let fractional_day = elapsed / day_sec;
        let hours = (fractional_day * 24.0 + 12.0).rem_euclid(24.0);
        hours as f32
    }
}

// ============================================================================
// 2. COMPONENTS
// ============================================================================

/// Marker component for the primary host star DirectionalLight entity (Star A).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[reflect(Component)]
pub struct PrimaryStar;

/// Marker component for the companion dwarf star DirectionalLight entity (Star B).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[reflect(Component)]
pub struct SecondaryStar;

/// Dedicated component tracking dynamic topocentric orbital vectors, elevation, azimuth,
/// and surface illuminance for any celestial stellar body in the S-type system.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct StellarOrbitalData {
    /// Normalized topocentric direction vector pointing FROM the observer TOWARDS the star
    /// in Bevy world space (+Y = Up, +X = East, -Z = North).
    pub direction: Vec3,

    /// Topocentric elevation angle in radians (-PI/2 to +PI/2). Positive values indicate
    /// the star is above the local geometric horizon.
    pub elevation_radians: f32,

    /// Topocentric azimuth angle in radians (0 to 2*PI, North = 0, East = PI/2, South = PI, West = 3*PI/2).
    pub azimuth_radians: f32,

    /// Current apparent illuminance delivered to the horizontal surface in lux,
    /// attenuated by solar elevation angle and atmospheric air mass.
    pub current_illuminance_lux: f32,

    /// True if the star is currently above the local planetary horizon (elevation > -0.05 rad).
    pub is_above_horizon: bool,

    /// True if this star is actively casting directional shadow cascades this frame.
    pub is_casting_shadows: bool,
}

impl Default for StellarOrbitalData {
    fn default() -> Self {
        Self {
            direction: Vec3::Y,
            elevation_radians: PI * 0.5,
            azimuth_radians: PI,
            current_illuminance_lux: 100_000.0,
            is_above_horizon: true,
            is_casting_shadows: true,
        }
    }
}

// ============================================================================
// 3. STARTUP SETUP: DUAL DIRECTIONAL LIGHTS & ASYMMETRIC CSM CONFIGS
// ============================================================================

/// Spawns the dual directional lights for Star A and Star B with independent,
/// asymmetric Cascaded Shadow Map (CSM) configurations.
pub fn setup_binary_star_system(
    mut commands: Commands,
    config: Res<BinaryOrbitConfig>,
) {
    info!(
        "Initializing S-Type Binary Star System (Primary Star A: {} cascades, Secondary Star B: {} cascades)...",
        config.star_a_num_cascades, config.star_b_num_cascades
    );

    // ------------------------------------------------------------------------
    // 1. Primary Host Star A: Higher Fidelity CSM Tier
    // ------------------------------------------------------------------------
    // Line Callout: Primary Star cascade bounds configuration.
    // 3 cascades covering up to 160m:
    // - Cascade 0 (0.1m - 15.0m): Ultra-sharp contact shadows for player hands, weapons, nearby structures.
    // - Cascade 1 (15.0m - 50.0m): Mid-range fidelity for trees, units, buildings.
    // - Cascade 2 (50.0m - 160.0m): Distant silhouettes across the landscape.
    let cascade_config_a = CascadeShadowConfigBuilder {
        num_cascades: config.star_a_num_cascades,
        minimum_distance: 0.1,
        maximum_distance: config.star_a_maximum_shadow_distance,
        first_cascade_far_bound: config.star_a_first_cascade_far_bound,
        overlap_proportion: 0.20,
    }
    .build();

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: config.star_a_color,
                illuminance: config.star_a_base_illuminance_lux,
                shadows_enabled: config.star_a_shadows_enabled,
                ..default()
            },
            transform: Transform::from_xyz(0.0, 100.0, -100.0).looking_at(Vec3::ZERO, Vec3::Y),
            cascade_shadow_config: cascade_config_a,
            ..default()
        },
        PrimaryStar,
        StellarOrbitalData {
            direction: Vec3::new(0.0, 0.7071, 0.7071),
            elevation_radians: PI * 0.25,
            azimuth_radians: PI,
            current_illuminance_lux: config.star_a_base_illuminance_lux,
            is_above_horizon: true,
            is_casting_shadows: config.star_a_shadows_enabled,
        },
        Name::new("Host Star A (Primary Directional Light)"),
    ));

    // ------------------------------------------------------------------------
    // 2. Secondary Companion Star B: Asymmetric Performance CSM Tier
    // ------------------------------------------------------------------------
    // Line Callout: Secondary Star cascade bounds configuration.
    // 2 cascades covering up to 75m:
    // - Cascade 0 (0.1m - 12.0m): Contact shadows for secondary silhouettes.
    // - Cascade 1 (12.0m - 75.0m): Immediate surroundings.
    // By restricting Star B to 2 cascades and 75m, we eliminate 1 entire shadow rasterization
    // pass per frame and restrict vertex throughput, while retaining dual colored shadows
    // and double penumbras everywhere the player is looking!
    let cascade_config_b = CascadeShadowConfigBuilder {
        num_cascades: config.star_b_num_cascades,
        minimum_distance: 0.1,
        maximum_distance: config.star_b_maximum_shadow_distance,
        first_cascade_far_bound: config.star_b_first_cascade_far_bound,
        overlap_proportion: 0.20,
    }
    .build();

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: config.star_b_color,
                illuminance: config.star_b_base_illuminance_lux,
                shadows_enabled: config.star_b_shadows_enabled,
                ..default()
            },
            transform: Transform::from_xyz(50.0, 60.0, -80.0).looking_at(Vec3::ZERO, Vec3::Y),
            cascade_shadow_config: cascade_config_b,
            ..default()
        },
        SecondaryStar,
        StellarOrbitalData {
            direction: Vec3::new(0.5, 0.6, 0.62).normalize(),
            elevation_radians: PI * 0.20,
            azimuth_radians: PI * 0.75,
            current_illuminance_lux: config.star_b_base_illuminance_lux,
            is_above_horizon: true,
            is_casting_shadows: config.star_b_shadows_enabled,
        },
        Name::new("Companion Star B (Secondary Directional Light)"),
    ));
}

// ============================================================================
// 4. ORBITAL PROGRESSION & TOPOCENTRIC VECTOR MATH SYSTEM
// ============================================================================

/// Advances the planetary diurnal rotation and binary mutual orbit, computing exact
/// topocentric direction vectors, elevation angles, and azimuths for both stars.
///
/// Geometric Vector Math Explanation:
/// 1. Planet Diurnal Rotation (Star A):
///    - Planetary rotation about its spin axis generates an apparent diurnal motion
///      $\theta_d(t) = (2\pi \cdot t / T_{\text{day}}) \pmod{2\pi}$.
///    - Obliquity $\epsilon$ tilts the equatorial plane relative to the ecliptic:
///      $\sin(\delta_A) = \sin(\epsilon) \sin(\theta_d)$
///      $\cos(\alpha_A) = \cos(\theta_d)$, $\sin(\alpha_A) = \cos(\epsilon) \sin(\theta_d)$
///    - Topocentric transformation at observer latitude $\phi$:
///      $\sin(\text{elev}_A) = \sin(\phi) \sin(\delta_A) + \cos(\phi) \cos(\delta_A) \cos(H_A)$
///      where $H_A = \theta_d - \alpha_A - \lambda_{\text{obs}}$ is the Local Hour Angle.
/// 2. S-Type Binary Orbit (Star B):
///    - Star B revolves around Star A at wide radius with angular velocity
///      $\omega_B = 2\pi / (T_{\text{day}} \cdot N_{\text{period}})$.
///    - Mutual orbital inclination $i_B$ rotates the orbital plane relative to the ecliptic:
///      $\sin(\beta_B) = \sin(i_B) \sin(\omega_B t)$
///      $\tan(\lambda_B) = \cos(i_B) \tan(\omega_B t)$
///    - In the planet's sky, Star B's position combines planetary rotation ($H_B$) and
///      orbital displacement ($\lambda_B, \beta_B$), creating variable day/night alignments.
pub fn update_binary_star_orbital_progression(
    time: Res<Time>,
    mut config: ResMut<BinaryOrbitConfig>,
    mut star_a_query: Query<(&mut Transform, &mut DirectionalLight, &mut StellarOrbitalData), (With<PrimaryStar>, Without<SecondaryStar>)>,
    mut star_b_query: Query<(&mut Transform, &mut DirectionalLight, &mut StellarOrbitalData), (With<SecondaryStar>, Without<PrimaryStar>)>,
) {
    // ------------------------------------------------------------------------
    // Step 1: Advance simulation time based on time scale
    // ------------------------------------------------------------------------
    let dt = time.delta_seconds_f64() * config.time_scale as f64;
    let total_binary_cycle_sec = (config.day_duration_seconds * config.binary_orbital_period_days) as f64;
    config.simulation_time_seconds = (config.simulation_time_seconds + dt).rem_euclid(total_binary_cycle_sec);

    let day_sec = config.day_duration_seconds as f64;
    let diurnal_angle = ((config.simulation_time_seconds / day_sec) * 2.0 * std::f64::consts::PI) as f32;
    let eps = config.planet_axial_tilt_radians;
    let phi = config.observer_latitude_radians;
    let lambda_obs = config.observer_longitude_radians;

    // ------------------------------------------------------------------------
    // Step 2: Primary Host Star A Topocentric Vector Calculation
    // ------------------------------------------------------------------------
    let lambda_a = diurnal_angle;
    let sin_dec_a = eps.sin() * lambda_a.sin();
    let dec_a = sin_dec_a.asin();
    let ra_a = (eps.cos() * lambda_a.sin()).atan2(lambda_a.cos());

    let lha_a = diurnal_angle - ra_a - lambda_obs;
    let sin_elev_a = phi.sin() * dec_a.sin() + phi.cos() * dec_a.cos() * lha_a.cos();
    let elev_a = sin_elev_a.clamp(-1.0, 1.0).asin();

    let cos_azim_a = (dec_a.sin() - phi.sin() * elev_a.sin()) / (phi.cos() * elev_a.cos().max(1e-4));
    let azim_a = if lha_a.sin() > 0.0 {
        2.0 * PI - cos_azim_a.clamp(-1.0, 1.0).acos()
    } else {
        cos_azim_a.clamp(-1.0, 1.0).acos()
    };

    let cos_el_a = elev_a.cos();
    // Bevy World Space: +Y = Up, +X = East, -Z = North
    let dir_a = Vec3::new(
        cos_el_a * azim_a.sin(),
        elev_a.sin(),
        -cos_el_a * azim_a.cos(),
    ).normalize();

    // ------------------------------------------------------------------------
    // Step 3: Secondary Companion Star B Topocentric Vector Calculation
    // ------------------------------------------------------------------------
    let binary_orbit_angle = ((config.simulation_time_seconds / total_binary_cycle_sec) * 2.0 * std::f64::consts::PI) as f32;
    let i_b = config.binary_inclination_radians;

    let sin_beta_b = i_b.sin() * binary_orbit_angle.sin();
    let beta_b = sin_beta_b.asin();
    let lambda_b = (i_b.cos() * binary_orbit_angle.sin()).atan2(binary_orbit_angle.cos());

    let sin_dec_b = eps.cos() * beta_b.sin() + eps.sin() * beta_b.cos() * lambda_b.sin();
    let dec_b = sin_dec_b.clamp(-1.0, 1.0).asin();
    let ra_b = (lambda_b.sin() * eps.cos() - beta_b.tan() * eps.sin()).atan2(lambda_b.cos());

    let lha_b = diurnal_angle - ra_b - lambda_obs;
    let sin_elev_b = phi.sin() * dec_b.sin() + phi.cos() * dec_b.cos() * lha_b.cos();
    let elev_b = sin_elev_b.clamp(-1.0, 1.0).asin();

    let cos_azim_b = (dec_b.sin() - phi.sin() * elev_b.sin()) / (phi.cos() * elev_b.cos().max(1e-4));
    let azim_b = if lha_b.sin() > 0.0 {
        2.0 * PI - cos_azim_b.clamp(-1.0, 1.0).acos()
    } else {
        cos_azim_b.clamp(-1.0, 1.0).acos()
    };

    let cos_el_b = elev_b.cos();
    let dir_b = Vec3::new(
        cos_el_b * azim_b.sin(),
        elev_b.sin(),
        -cos_el_b * azim_b.cos(),
    ).normalize();

    // ------------------------------------------------------------------------
    // Step 4: Apply Topocentric Transforms and Illuminance to Star A
    // ------------------------------------------------------------------------
    if let Ok((mut transform_a, mut light_a, mut data_a)) = star_a_query.get_single_mut() {
        data_a.direction = dir_a;
        data_a.elevation_radians = elev_a;
        data_a.azimuth_radians = azim_a;
        data_a.is_above_horizon = elev_a > -0.05;

        // Light travels in -direction (looking towards the planet from the star)
        if dir_a.length_squared() > 1e-4 {
            *transform_a = Transform::from_translation(Vec3::ZERO).looking_to(-dir_a, Vec3::Y);
        }

        // Solar altitude extinction curve
        let solar_zenith_attenuation = elev_a.sin().max(0.0).powf(0.80);
        let apparent_lux = if data_a.is_above_horizon {
            config.star_a_base_illuminance_lux * solar_zenith_attenuation
        } else {
            0.0
        };
        data_a.current_illuminance_lux = apparent_lux;
        light_a.illuminance = apparent_lux;
        light_a.color = config.star_a_color;
    }

    // ------------------------------------------------------------------------
    // Step 5: Apply Topocentric Transforms and Illuminance to Star B
    // ------------------------------------------------------------------------
    if let Ok((mut transform_b, mut light_b, mut data_b)) = star_b_query.get_single_mut() {
        data_b.direction = dir_b;
        data_b.elevation_radians = elev_b;
        data_b.azimuth_radians = azim_b;
        data_b.is_above_horizon = elev_b > -0.05;

        if dir_b.length_squared() > 1e-4 {
            *transform_b = Transform::from_translation(Vec3::ZERO).looking_to(-dir_b, Vec3::Y);
        }

        let dwarf_zenith_attenuation = elev_b.sin().max(0.0).powf(0.80);
        let apparent_lux = if data_b.is_above_horizon {
            config.star_b_base_illuminance_lux * dwarf_zenith_attenuation
        } else {
            0.0
        };
        data_b.current_illuminance_lux = apparent_lux;
        light_b.illuminance = apparent_lux;
        light_b.color = config.star_b_color;
    }
}

// ============================================================================
// 5. SHADOW SYNCHRONIZATION & DUAL PENUMBRA ARBITRATION
// ============================================================================

/// Synchronizes shadow casting status for both directional lights.
/// Enables simultaneous dual shadow casting whenever both stars reside above the horizon,
/// producing distinct colored shadows (warm white vs amber) and intersecting double penumbras.
pub fn sync_binary_star_shadows(
    config: Res<BinaryOrbitConfig>,
    mut star_a_query: Query<(&mut DirectionalLight, &mut StellarOrbitalData), (With<PrimaryStar>, Without<SecondaryStar>)>,
    mut star_b_query: Query<(&mut DirectionalLight, &mut StellarOrbitalData), (With<SecondaryStar>, Without<PrimaryStar>)>,
) {
    if let Ok((mut light_a, mut data_a)) = star_a_query.get_single_mut() {
        let enable_shadows = config.star_a_shadows_enabled && data_a.is_above_horizon;
        light_a.shadows_enabled = enable_shadows;
        data_a.is_casting_shadows = enable_shadows;
    }

    if let Ok((mut light_b, mut data_b)) = star_b_query.get_single_mut() {
        let enable_shadows = config.star_b_shadows_enabled && data_b.is_above_horizon;
        light_b.shadows_enabled = enable_shadows;
        data_b.is_casting_shadows = enable_shadows;
    }
}

// ============================================================================
// 6. MODULAR BEVY PLUGIN
// ============================================================================

/// Modular Bevy plugin implementing an S-type binary star system with dual shadow-casting
/// DirectionalLights, maintaining an even day/night planetary cycle while ensuring
/// performant, asymmetric cascaded shadow mapping (CSM).
pub struct BinaryStarSystemPlugin;

impl Plugin for BinaryStarSystemPlugin {
    fn build(&self, app: &mut App) {
        // Allocate high-capacity shadow atlas memory to avoid cascade starvation across dual lights
        app.insert_resource(DirectionalLightShadowMap { size: 4096 })
            .init_resource::<BinaryOrbitConfig>()
            .register_type::<BinaryOrbitConfig>()
            .register_type::<PrimaryStar>()
            .register_type::<SecondaryStar>()
            .register_type::<StellarOrbitalData>()
            .add_systems(Startup, setup_binary_star_system)
            .add_systems(
                Update,
                (
                    update_binary_star_orbital_progression,
                    sync_binary_star_shadows,
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
    fn test_binary_orbit_config_clock_time() {
        let mut config = BinaryOrbitConfig::default();
        config.day_duration_seconds = 100.0;

        // At epoch 0.0 -> High Noon (12:00)
        config.simulation_time_seconds = 0.0;
        assert!((config.clock_time_hours() - 12.0).abs() < 0.01);

        // At 25.0 seconds (25% of day = 6 hours later) -> Sunset (18:00)
        config.simulation_time_seconds = 25.0;
        assert!((config.clock_time_hours() - 18.0).abs() < 0.01);

        // At 50.0 seconds (50% of day = 12 hours later) -> Midnight (00:00)
        config.simulation_time_seconds = 50.0;
        assert!((config.clock_time_hours() - 0.0).abs() < 0.01 || (config.clock_time_hours() - 24.0).abs() < 0.01);

        // At 75.0 seconds (75% of day = 18 hours later) -> Sunrise (06:00)
        config.simulation_time_seconds = 75.0;
        assert!((config.clock_time_hours() - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_asymmetric_cascade_configuration_construction() {
        let config = BinaryOrbitConfig::default();

        // Primary star: 3 cascades, 160m
        let cascade_a = CascadeShadowConfigBuilder {
            num_cascades: config.star_a_num_cascades,
            minimum_distance: 0.1,
            maximum_distance: config.star_a_maximum_shadow_distance,
            first_cascade_far_bound: config.star_a_first_cascade_far_bound,
            overlap_proportion: 0.20,
        }
        .build();

        // Secondary star: 2 cascades, 75m
        let cascade_b = CascadeShadowConfigBuilder {
            num_cascades: config.star_b_num_cascades,
            minimum_distance: 0.1,
            maximum_distance: config.star_b_maximum_shadow_distance,
            first_cascade_far_bound: config.star_b_first_cascade_far_bound,
            overlap_proportion: 0.20,
        }
        .build();

        // Validate bounds
        assert_eq!(cascade_a.bounds.len(), 3);
        assert_eq!(cascade_b.bounds.len(), 2);
        assert!(cascade_a.bounds.last().unwrap() > cascade_b.bounds.last().unwrap());
    }

    #[test]
    fn test_dual_shadow_casting_progression() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::transform::TransformPlugin);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.add_plugins(BinaryStarSystemPlugin);

        // High noon: Star A should be at zenith/high elevation
        app.update();

        let mut data_query = app.world_mut().query::<(&StellarOrbitalData, &DirectionalLight)>();
        let stars: Vec<(&StellarOrbitalData, &DirectionalLight)> = data_query.iter(app.world()).collect();
        assert_eq!(stars.len(), 2);

        // At noon, Star A must have shadows enabled
        let star_a = stars.iter().find(|(d, _)| d.current_illuminance_lux > 50_000.0).unwrap();
        assert!(star_a.0.is_above_horizon);
        assert!(star_a.1.shadows_enabled);
    }
}
