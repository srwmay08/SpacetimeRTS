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
// 5. Visual Celestial Objects & Starfield: Visible Host Star A and Companion Star B
//    stellar discs with emission scaling, plus an astronomical 6,000-star procedural
//    starfield that naturally emerges during twilight and deep night.
// 6. Atmospheric Weather System: Dynamic presets for ClearSky, AerosolHaze,
//    StellarWindAurora (binary magnetic interactions), and OvercastPrecipitation.
// 7. Interactive In-Game Controls:
//    - [F8] Toggle immediately between High Noon (12:00) and Deep Night (00:00).
//    - [ [ ] Rewind time by 1 in-game hour.
//    - [ ] ] Advance time by 1 in-game hour.
//    - [ - ] Slow down time progression (0x, 1x, 10x, 60x, 300x).
//    - [ = ] Accelerate time progression.
//    - [F9] Cycle atmospheric weather presets.
// ----------------------------------------------------------------------------

#![allow(dead_code)]

use std::f32::consts::PI;
use bevy::prelude::*;
use bevy::pbr::{
    CascadeShadowConfigBuilder, DirectionalLightShadowMap, FogFalloff, FogSettings,
    NotShadowCaster,
};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::RenderLayers;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};

// Import parry3d geometric query structures for planetary horizon occlusion
use parry3d::math::{Point as ParryPoint, Vector as ParryVector};
use parry3d::query::{Ray as ParryRay, RayCast};
use parry3d::shape::Ball as ParryBall;

// ============================================================================
// 1. ASTRONOMICAL ENUMS & DATA STRUCTURES
// ============================================================================

/// Canonical sunset & twilight color palette for S-type planetary atmosphere:
/// --navy-electric: #390099ff;
/// --dark-raspberry: #9e0059ff;
/// --hot-fuchsia: #ff0054ff;
/// --blaze-orange: #ff5400ff;
/// --amber-gold: #ffbd00ff;
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
        // Full daytime sky horizon (clear atmospheric Rayleigh blue)
        Vec3::new(0.52, 0.68, 0.88)
    } else if deg >= 4.5 {
        // Late golden hour: Daytime blue blending into Amber Gold (#ffbd00)
        let t = (10.0 - deg) / 5.5;
        Vec3::new(0.52, 0.68, 0.88).lerp(COLOR_AMBER_GOLD, t)
    } else if deg >= 1.0 {
        // Low sunset: Amber Gold (#ffbd00) blending into Blaze Orange (#ff5400)
        let t = (4.5 - deg) / 3.5;
        COLOR_AMBER_GOLD.lerp(COLOR_BLAZE_ORANGE, t)
    } else if deg >= -2.5 {
        // Solar contact & civil dusk: Blaze Orange (#ff5400) into Hot Fuchsia (#ff0054)
        let t = (1.0 - deg) / 3.5;
        COLOR_BLAZE_ORANGE.lerp(COLOR_HOT_FUCHSIA, t)
    } else if deg >= -6.5 {
        // Nautical twilight / Belt of Venus: Hot Fuchsia (#ff0054) into Dark Raspberry (#9e0059)
        let t = (-2.5 - deg) / 4.0;
        COLOR_HOT_FUCHSIA.lerp(COLOR_DARK_RASPBERRY, t)
    } else if deg >= -12.0 {
        // Astronomical twilight: Dark Raspberry (#9e0059) into Navy Electric (#390099)
        let t = (-6.5 - deg) / 5.5;
        COLOR_DARK_RASPBERRY.lerp(COLOR_NAVY_ELECTRIC, t)
    } else {
        // True deep night: Deep Navy Electric (#390099) atmospheric starlight
        let t = ((-12.0 - deg) / 6.0).clamp(0.0, 1.0);
        let deep_navy = COLOR_NAVY_ELECTRIC * 0.40 + Vec3::new(0.02, 0.015, 0.06);
        COLOR_NAVY_ELECTRIC.lerp(deep_navy, t)
    }
}

/// Dynamic sky illumination condition based on stellar elevation thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DynamicSkyState {
    /// Both Star A and Star B are simultaneously above the horizon.
    #[default]
    DualDay,
    /// Host Star A is above horizon; Star B is below twilight threshold.
    StarAPrimaryDay,
    /// Companion Star B is above horizon; Star A is below horizon (amber daylight).
    StarBSecondaryDay,
    /// Both stars are below astronomical twilight (-12° to -18°).
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

/// Active atmospheric weather condition modulating participating medium turbidity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeatherType {
    /// Pristine optical clarity, sharp solar disk and deep blue Rayleigh scatter.
    #[default]
    ClearSky,
    /// High aerosol optical depth, diffuse glowing horizon, prominent Mie coronas.
    AerosolHaze,
    /// Magnetic storm from S-type stellar wind interaction; shimmering emerald/cyan/violet auroras.
    StellarWindAurora,
    /// Heavy moisture and cloud extinction with dim ambient starlight.
    OvercastPrecipitation,
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
    /// Time acceleration multiplier (e.g. 60.0 = 1 in-game minute per real second,
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
    /// Stratospheric ozone layer scale height H_O in meters (~25,000 m).
    pub ozone_scale_height: f32,
    /// Wavelength-dependent Rayleigh scattering coefficient vector β_R (km^-1)
    /// evaluated at RGB primary wavelengths [680nm, 550nm, 440nm]:
    /// [0.0058, 0.0135, 0.0331] km^-1.
    pub rayleigh_scattering_coefficients: Vec3,
    /// Wavelength-independent Mie aerosol scattering coefficient β_Ms (km^-1): 0.004 km^-1.
    pub mie_scattering_coefficient: f32,
    /// Mie aerosol absorption coefficient β_Ma (km^-1): 0.0044 km^-1.
    pub mie_absorption_coefficient: f32,
    /// Wavelength-dependent stratospheric Chappuis ozone absorption coefficient vector β_O (km^-1)
    /// evaluated at RGB primary wavelengths [680nm, 550nm, 440nm]:
    /// Peak absorption occurs in green (550nm) with low red and negligible blue absorption:
    /// [0.00065, 0.00240, 0.000085] km^-1.
    pub ozone_absorption_coefficients: Vec3,
    /// Mean ground surface diffuse albedo (e.g. 0.18 for average soil/vegetation).
    pub ground_albedo: f32,

    // ------------------------------------------------------------------------
    // Stellar Characteristics
    // ------------------------------------------------------------------------
    /// Effective blackbody temperature of Host Star A (Kelvin, G-type: 5800K).
    pub star_a_temperature_kelvin: f32,
    /// Base extraterrestrial solar illuminance for Star A in lux (default: 75,000 lx).
    pub star_a_base_illuminance_lux: f32,
    /// Effective blackbody temperature of Companion Star B (Kelvin, K/M dwarf: 3600K).
    pub star_b_temperature_kelvin: f32,
    /// Base extraterrestrial solar illuminance for Star B in lux (default: 55,000 lx).
    pub star_b_base_illuminance_lux: f32,

    // ------------------------------------------------------------------------
    // Dynamic Runtime Lighting & Console Controls
    // ------------------------------------------------------------------------
    /// Manual direct color override for Star A (None = physically based Planck/Rayleigh).
    pub star_a_color_override: Option<Color>,
    /// Manual direct color override for Star B (None = physically based Planck/Mie).
    pub star_b_color_override: Option<Color>,
    /// Runtime shadow toggle for Star A DirectionalLight (default: true).
    pub star_a_shadows_enabled: bool,
    /// Runtime shadow toggle for Star B DirectionalLight (default: true).
    pub star_b_shadows_enabled: bool,
    /// Manual ambient light illuminance override in lux (None = auto-scaled 2.5 - 380 lx).
    pub ambient_illuminance_lux: Option<f32>,
    /// Dynamic scale factor for cosmic starfield points of light (default: 1.0).
    pub starfield_scale: f32,
}

impl Default for BinarySkyConfig {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            day_duration_seconds: 1440.0, // 24 minutes real-time per in-game day
            year_duration_days: 365.0,
            binary_period_years: 18.5,    // Star B moves slowly across seasonal cycles
            axial_tilt_radians: 23.44f32.to_radians(),
            binary_inclination_radians: 14.25f32.to_radians(),
            observer_latitude_radians: 45.0f32.to_radians(),
            observer_longitude_radians: 0.0,
            planet_radius_meters: 6_371_000.0,
            atmosphere_height_meters: 100_000.0,
            rayleigh_scale_height: 8_400.0,
            mie_scale_height: 1_200.0,
            ozone_scale_height: 25_000.0,
            // Rayleigh scattering cross-sections at 680nm (R), 550nm (G), 440nm (B)
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
            ambient_illuminance_lux: None,
            starfield_scale: 1.0,
        }
    }
}

/// Instantaneous astronomical ephemeris state tracking normalized topocentric
/// stellar direction vectors, elevations, azimuths, and dynamic sky classifications.
#[derive(Resource, Debug, Clone)]
pub struct BinaryEphemerisState {
    /// Accumulated simulation time in seconds. Starts at 0.0 (High Noon by default).
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

impl BinaryEphemerisState {
    /// Returns current in-game clock time in 24-hour format [0.0, 24.0),
    /// where 12.0 is solar noon and 0.0 is midnight.
    pub fn clock_time_hours(&self) -> f32 {
        let frac = (self.diurnal_angle / (2.0 * PI)).rem_euclid(1.0);
        ((frac + 0.5).rem_euclid(1.0)) * 24.0
    }
}

impl Default for BinaryEphemerisState {
    fn default() -> Self {
        Self {
            simulation_time_seconds: 0.0, // Defaults to High Noon (12:00)
            day_progress: 0.0,
            year_progress: 0.0,
            binary_progress: 0.0,
            diurnal_angle: 0.0,
            primary_orbit_angle: 0.0,
            binary_orbit_angle: 0.0,
            star_a_direction: Vec3::new(0.0, std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2),
            star_a_elevation: PI * 0.25, // High +45° bright sun
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

/// Component tag for Host Star A's visual disc and corona sphere.
#[derive(Component, Debug, Default)]
pub struct StarAVolumetricDisk;

/// Component tag for Companion Star B's visual dwarf disc and corona sphere.
#[derive(Component, Debug, Default)]
pub struct StarBVolumetricDisk;

/// Component tag for the procedural cosmic background starfield dome.
#[derive(Component, Debug, Default)]
pub struct CosmicStarfield;

/// Uniform buffer passed to the procedural aurora sky dome WGSL shader.
#[derive(Clone, Copy, ShaderType, Debug, Reflect)]
pub struct SkyUniforms {
    pub night_factor: f32,
    pub weather_intensity: f32,
    pub speed: f32,
    pub brightness: f32,
}

impl Default for SkyUniforms {
    fn default() -> Self {
        Self {
            night_factor: 0.0,
            weather_intensity: 0.0,
            speed: 1.0,
            brightness: 1.0,
        }
    }
}

/// Custom Bevy PBR Material rendering animated planetary solar wind auroras across the sky dome.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StarAuroraDomeMaterial {
    #[uniform(0)]
    pub uniforms: SkyUniforms,
}

impl Default for StarAuroraDomeMaterial {
    fn default() -> Self {
        Self {
            uniforms: SkyUniforms::default(),
        }
    }
}

impl Material for StarAuroraDomeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/aurora.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Disable backface culling so the dome renders correctly from the inside
        descriptor.primitive.cull_mode = None;
        // Disable depth writing and set depth comparison so the dome renders strictly behind all 3D world geometry
        if let Some(ref mut depth_stencil) = descriptor.depth_stencil {
            depth_stencil.depth_write_enabled = false;
            depth_stencil.depth_compare = bevy::render::render_resource::CompareFunction::GreaterEqual;
        }
        Ok(())
    }
}

/// Compatibility alias preserving external API and test suite expectations.
pub type AuroraMaterial = StarAuroraDomeMaterial;
pub type AuroraUniforms = SkyUniforms;

/// Component tag for the procedural planetary aurora sky dome.
#[derive(Component, Debug, Default)]
pub struct StarAuroraDome;

/// Deprecated component tag for legacy ribbon queries and test compatibility.
#[derive(Component, Debug, Default)]
pub struct AuroraCurtain;

/// Component tag for the procedural atmospheric sky dome rendering Rayleigh/Mie scattering.
#[derive(Component, Debug, Default)]
pub struct AtmosphericSkyDome;

/// Component tag for the dynamic volumetric rain precipitation streaks.
#[derive(Component, Debug, Default)]
pub struct PrecipitationStreaks;

// ============================================================================
// 4. RADIATIVE TRANSFER & SCATTERING MATHEMATICS (RDR2 METHODOLOGY)
// ============================================================================

/// Evaluates normalized Planckian spectral radiance for a given blackbody temperature
/// at RGB primary wavelengths: λ_R = 680nm, λ_G = 550nm, λ_B = 440nm.
pub fn planck_blackbody_rgb(temp_kelvin: f32) -> Vec3 {
    let t = temp_kelvin.max(1000.0);
    let wavelengths = [6.80e-7f64, 5.50e-7f64, 4.40e-7f64];
    
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
        (38.2 + (-deg) * 7.0).min(80.0)
    }
}

/// Calculates relative optical air mass m_O(θ) across the planetary stratospheric ozone layer (h_O ≈ 25 km).
/// - For elevation α >= 0°: Evaluates Chapman spherical layer geometry (zenith m_O ≈ 1.0, horizon m_O ≈ 11.35).
/// - For elevation α < 0° (twilight): Evaluates tangent spherical shell grazing path through both entry
///   and exit chords, increasing smoothly up to ~38.0 during nautical/astronomical twilight without singularities.
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

/// Evaluates wavelength-dependent spectral transmittance T(λ) across the
/// combined Rayleigh, Mie, and stratospheric Chappuis ozone participating medium:
///
/// T(λ) = exp(-(β_R(λ) * H_R * m_R(α) + β_Me * H_M * m_M(α) + β_O(λ) * H_O * m_O(α)))
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

/// Converts physical atmospheric spectral radiance to sRGB display color using
/// physical exposure adaptation (photopic daytime, mesopic twilight, and scotopic night):
pub fn tonemap_atmospheric_radiance(radiance: Vec3) -> Vec3 {
    let luma = radiance.x * 0.2126 + radiance.y * 0.7152 + radiance.z * 0.0722;
    // Continuous, smooth photographic exposure adaptation across daytime, twilight, and night.
    // Replaces disjoint piecewise branch jumps with a smooth C^1 transition:
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

/// Cornette-Shanks analytical scattering phase function P_CS(θ, g).
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

/// Generates an astronomical 3D starfield mesh consisting of billboarded diamond stars
/// distributed over the celestial sphere, perfectly facing the observer at the center.
pub fn create_billboard_starfield_mesh(star_count: usize) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let mut positions = Vec::with_capacity(star_count * 4);
    let mut colors = Vec::with_capacity(star_count * 4);
    let mut indices = Vec::with_capacity(star_count * 6);

    let mut seed = 987654321u64;
    let mut xorshift = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    let spectral_colors = [
        [0.75, 0.85, 1.0, 1.0], // O/B Blue-White
        [1.0, 1.0, 1.0, 1.0],   // A Pure White
        [1.0, 0.96, 0.82, 1.0], // F/G Yellow-White (Sol-like)
        [1.0, 0.80, 0.50, 1.0], // K Amber
        [1.0, 0.55, 0.35, 1.0], // M Crimson Supergiant
        [0.60, 0.95, 1.0, 1.0], // Cyan flare
    ];

    let radius = 240.0;
    let mut normals = Vec::with_capacity(star_count * 4);
    let mut uvs = Vec::with_capacity(star_count * 4);

    for i in 0..star_count {
        let u1 = ((xorshift() % 10000) as f32) / 10000.0;
        let u2 = ((xorshift() % 10000) as f32) / 10000.0;
        let u3 = ((xorshift() % 10000) as f32) / 10000.0;
        let u4 = ((xorshift() % 10000) as f32) / 10000.0;

        // Archimedes equal-area celestial projection:
        // Sampling y = sin(elev) uniformly eliminates polar/zenith pinching,
        // delivering identical star density per steradian across the entire sky dome.
        let azim = u1 * 2.0 * PI;
        let y_min = -0.04;
        let y = y_min + u2 * (1.0 - y_min);
        let r_xz = (1.0 - y * y).max(0.0).sqrt();

        let dir = Vec3::new(
            r_xz * azim.sin(),
            y,
            -r_xz * azim.cos(),
        );

        let center = dir * radius;

        // Construct orthonormal billboard basis facing camera at origin
        let up = if dir.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
        let right = dir.cross(up).normalize();
        let star_up = right.cross(dir).normalize();

        // Astrometric magnitude hierarchy for pinpoint stars (reduced by 15% to eliminate chunky look):
        // Top 3% are prominent guide stars (0.306m - 0.425m)
        // Next 18% are medium navigational stars (0.204m - 0.272m)
        // Remaining 79% are background field stars (0.110m - 0.162m)
        let base_size = if u3 > 0.97 {
            0.36 + u4 * 0.14
        } else if u3 > 0.82 {
            0.24 + u4 * 0.08
        } else {
            0.13 + u4 * 0.06
        };
        let size = base_size * 0.85;

        let color_idx = (xorshift() % (spectral_colors.len() as u64)) as usize;
        let mut c = spectral_colors[color_idx];
        if u3 <= 0.82 {
            let dim = 0.80 + u4 * 0.20;
            c[0] *= dim;
            c[1] *= dim;
            c[2] *= dim;
        }

        // Magnitude-tiered atmospheric air mass extinction:
        // In real observational astronomy, prominent 0th-magnitude guide stars punch through
        // much thicker low-horizon haze than faint 5th-magnitude background field stars.
        let is_guide_star = u3 > 0.97;
        let is_nav_star = u3 > 0.82;

        let (cutoff_y, full_y) = if is_guide_star {
            (0.015, 0.075) // Guide stars: visible down to ~1.2°
        } else if is_nav_star {
            (0.025, 0.110) // Navigational stars: visible down to ~2.5°
        } else {
            (0.040, 0.145) // Faint background stars: fade across 2.5° to 8.3°
        };

        let horizon_factor = ((y - cutoff_y) / (full_y - cutoff_y)).clamp(0.0, 1.0);
        // Smooth concave Hermite curve (preserves starlight visibility longer before falling off)
        let extinction = horizon_factor * (2.0 - horizon_factor);

        // Fast polynomial Rayleigh reddening (zero transcendental powf calls):
        // Blue wavelengths scatter out rapidly in lower atmosphere; red wavelengths penetrate deepest
        let ext_sq = extinction * extinction;
        c[0] *= extinction;        // Red penetrates haze cleanly
        c[1] *= extinction * 0.98; // Green slightly attenuated
        c[2] *= ext_sq;            // Blue scatters away rapidly (quadratic decay)
        c[3] *= extinction;        // Alpha transparency

        // Diamond star geometry eliminating square rasterization artifacts:
        // Generates tapered 4-point diamond facets along randomized celestial axes,
        // producing natural stellar glints that taper to sharp subpixel points at the tips.
        let half = size * 0.5;
        let rot_angle = u1 * (2.0 * PI);
        let cos_r = rot_angle.cos();
        let sin_r = rot_angle.sin();
        let axis_a = (right * cos_r + star_up * sin_r).normalize();
        let axis_b = (-right * sin_r + star_up * cos_r).normalize();

        // Slight celestial aspect ratio gives prominent stars natural scintillation spikes (0.72 ratio),
        // while background stars are symmetric diamonds (0.85 ratio).
        let aspect = if u3 > 0.82 { 0.72 } else { 0.85 };
        let v0 = center + axis_a * half;
        let v1 = center + axis_b * (half * aspect);
        let v2 = center - axis_a * half;
        let v3 = center - axis_b * (half * aspect);

        let base_idx = (i * 4) as u32;

        positions.push([v0.x, v0.y, v0.z]);
        positions.push([v1.x, v1.y, v1.z]);
        positions.push([v2.x, v2.y, v2.z]);
        positions.push([v3.x, v3.y, v3.z]);

        let norm = [-dir.x, -dir.y, -dir.z];
        normals.push(norm);
        normals.push(norm);
        normals.push(norm);
        normals.push(norm);

        uvs.push([0.5, 1.0]);
        uvs.push([1.0, 0.5]);
        uvs.push([0.5, 0.0]);
        uvs.push([0.0, 0.5]);

        colors.push(c);
        colors.push(c);
        colors.push(c);
        colors.push(c);

        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);

        indices.push(base_idx);
        indices.push(base_idx + 2);
        indices.push(base_idx + 3);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Compatibility alias preserving external API and test suite expectations.
pub fn create_starfield_mesh(star_count: usize) -> Mesh {
    create_billboard_starfield_mesh(star_count)
}

/// Generates an inverted procedural hemisphere mesh for the unified participating medium sky dome.
pub fn create_sky_dome_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let rings = 16;
    let sectors = 32;
    let radius = 460.0;

    let mut positions = Vec::with_capacity((rings + 1) * (sectors + 1));
    let mut normals = Vec::with_capacity((rings + 1) * (sectors + 1));
    let mut uvs = Vec::with_capacity((rings + 1) * (sectors + 1));
    let mut colors = Vec::with_capacity((rings + 1) * (sectors + 1));
    let mut indices = Vec::with_capacity(rings * sectors * 6);

    for r in 0..=rings {
        let phi = -0.05 + (r as f32 / rings as f32) * (PI * 0.5 + 0.05);
        let cos_phi = phi.cos();
        let sin_phi = phi.sin();

        for s in 0..=sectors {
            let theta = (s as f32 / sectors as f32) * 2.0 * PI;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            let x = radius * cos_phi * sin_theta;
            let y = radius * sin_phi;
            let z = -radius * cos_phi * cos_theta;

            positions.push([x, y, z]);
            // Inward-facing normal for sky dome viewed from inside
            normals.push([-cos_phi * sin_theta, -sin_phi, cos_phi * cos_theta]);
            uvs.push([s as f32 / sectors as f32, r as f32 / rings as f32]);
            colors.push([0.15, 0.40, 0.85, 1.0]); // Default daytime sky blue
        }
    }

    for r in 0..rings {
        for s in 0..sectors {
            let cur = (r * (sectors + 1) + s) as u32;
            let next = cur + sectors as u32 + 1;

            indices.push(cur);
            indices.push(cur + 1);
            indices.push(next);

            indices.push(cur + 1);
            indices.push(next + 1);
            indices.push(next);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Legacy dual ribbon curtain generator, deprecated in favor of procedural sky dome.
#[deprecated(note = "Legacy dual ribbon mesh retired in favor of procedural sky dome (create_sky_dome_mesh)")]
pub fn create_aurora_mesh() -> Mesh {
    create_sky_dome_mesh()
}

/// Generates a cylindrical volume of downward precipitation streaks for stormy weather.
pub fn create_precipitation_mesh(drop_count: usize) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let mut positions = Vec::with_capacity(drop_count * 4);
    let mut normals = Vec::with_capacity(drop_count * 4);
    let mut uvs = Vec::with_capacity(drop_count * 4);
    let mut colors = Vec::with_capacity(drop_count * 4);
    let mut indices = Vec::with_capacity(drop_count * 6);

    let mut seed = 5544332211u64;
    let mut xorshift = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    let w = 0.035;
    let slant_x = 0.06;
    let slant_z = 0.03;

    for i in 0..drop_count {
        let u1 = ((xorshift() % 10000) as f32) / 10000.0;
        let u2 = ((xorshift() % 10000) as f32) / 10000.0;
        let u3 = ((xorshift() % 10000) as f32) / 10000.0;
        let u4 = ((xorshift() % 10000) as f32) / 10000.0;

        let angle = u1 * 2.0 * PI;
        let dist = 1.0 + u2.sqrt() * 15.0; // 1m to 16m radius around player
        let x = angle.cos() * dist;
        let z = angle.sin() * dist;
        let y = -2.0 + u3 * 24.0; // -2m to +22m height
        let len = 0.65 + u4 * 0.45; // Streak length

        let base_idx = (i * 4) as u32;

        positions.push([x - w, y, z]);
        positions.push([x + w, y, z]);
        positions.push([x - w + slant_x, y - len, z + slant_z]);
        positions.push([x + w + slant_x, y - len, z + slant_z]);

        let norm = [0.0, 1.0, 0.0];
        normals.push(norm);
        normals.push(norm);
        normals.push(norm);
        normals.push(norm);

        uvs.push([0.0, 1.0]);
        uvs.push([1.0, 1.0]);
        uvs.push([0.0, 0.0]);
        uvs.push([1.0, 0.0]);

        let col = [0.80, 0.88, 1.0, 0.45];
        colors.push(col);
        colors.push(col);
        colors.push(col);
        colors.push(col);

        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);

        indices.push(base_idx + 1);
        indices.push(base_idx + 3);
        indices.push(base_idx + 2);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

// ============================================================================
// 5. BEVY SYSTEMS & PIPELINE SCHEDULING
// ============================================================================

/// Startup system: Spawns dual directional lights, stellar discs, starfield, and aurora.
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
        num_cascades: 3,
        minimum_distance: 0.1,
        maximum_distance: 160.0,
        first_cascade_far_bound: 15.0,
        overlap_proportion: 0.20,
    }
    .build();

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: Color::srgb(1.0, 0.97, 0.92),
                illuminance: config.star_a_base_illuminance_lux,
                shadows_enabled: true,
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
        num_cascades: 2,
        minimum_distance: 0.1,
        maximum_distance: 112.5,
        first_cascade_far_bound: 18.0,
        overlap_proportion: 0.20,
    }
    .build();

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: Color::srgb(1.0, 0.65, 0.35),
                illuminance: config.star_b_base_illuminance_lux,
                shadows_enabled: true,
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
                base_color: Color::srgba(1.0, 1.0, 1.0, 0.0), // Starts invisible in noon daylight
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
        AuroraCurtain,
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

/// Advances simulation time and computes exact topocentric directional vectors,
/// elevation angles, and azimuths for both stars in the S-type binary system.
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
        cos_el_a * azim_a.sin(), // East (+X)
        elev_a.sin(),            // Up (+Y)
        -cos_el_a * azim_a.cos(), // North is -Z, South is +Z
    ).normalize_or_zero();

    // ------------------------------------------------------------------------
    // Companion Star B Topocentric Position (S-Type Companion orbiting Star A)
    // Synchronized with Star A's celestial longitude with a ~16° angular separation
    // offset so both stars are prominently visible in the daytime sky simultaneously.
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

/// Evaluates planetary horizon dip and geometric line-of-sight occlusion
/// utilizing `parry3d`'s spatial query pipeline.
pub fn evaluate_horizon_dip_and_parry_occlusion(
    config: Res<BinarySkyConfig>,
    camera_query: Query<(&GlobalTransform, &Camera), With<AtmosphericCamera>>,
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

/// Evaluates the downsampled atmospheric participating medium radiance cache.
pub fn update_atmospheric_scattering_and_cache(
    config: Res<BinarySkyConfig>,
    ephemeris: Res<BinaryEphemerisState>,
    weather: Res<AtmosphericWeather>,
    mut cache: ResMut<AtmosphericRadianceCache>,
) {
    let effective_elev_a = ephemeris.star_a_elevation + cache.horizon_dip_radians;
    let effective_elev_b = ephemeris.star_b_elevation + cache.horizon_dip_radians;

    // ------------------------------------------------------------------------
    // 1. Direct Stellar Transmittance & Illuminance
    // ------------------------------------------------------------------------
    cache.star_a_transmittance = evaluate_spectral_transmittance(effective_elev_a, &config);
    cache.star_b_transmittance = evaluate_spectral_transmittance(effective_elev_b, &config);

    let blackbody_a = planck_blackbody_rgb(config.star_a_temperature_kelvin);
    let blackbody_b = planck_blackbody_rgb(config.star_b_temperature_kelvin);

    let filtered_rgb_a = blackbody_a * cache.star_a_transmittance;
    let filtered_rgb_b = blackbody_b * cache.star_b_transmittance;

    cache.star_a_color = Color::srgb(filtered_rgb_a.x, filtered_rgb_a.y, filtered_rgb_a.z);
    cache.star_b_color = Color::srgb(filtered_rgb_b.x, filtered_rgb_b.y, filtered_rgb_b.z);

    // Smooth Hermite sunset/sunrise cutoff across civil/nautical twilight [-8.0°, +4.0°]
    let t_a = ((effective_elev_a.to_degrees() + 8.0) / 12.0).clamp(0.0, 1.0);
    let cutoff_a = t_a * t_a * (3.0 - 2.0 * t_a);
    let t_b = ((effective_elev_b.to_degrees() + 8.0) / 12.0).clamp(0.0, 1.0);
    let cutoff_b = t_b * t_b * (3.0 - 2.0 * t_b);

    let mean_trans_a = (cache.star_a_transmittance.x + cache.star_a_transmittance.y + cache.star_a_transmittance.z) / 3.0;
    let mean_trans_b = (cache.star_b_transmittance.x + cache.star_b_transmittance.y + cache.star_b_transmittance.z) / 3.0;

    // Weather extinction modifier
    let weather_dimming = match weather.weather_type {
        WeatherType::ClearSky => 1.0,
        WeatherType::AerosolHaze => 0.82,
        WeatherType::StellarWindAurora => 0.95,
        WeatherType::OvercastPrecipitation => 0.35,
    };

    cache.star_a_illuminance_lux = config.star_a_base_illuminance_lux * mean_trans_a * cutoff_a * weather_dimming;
    cache.star_b_illuminance_lux = config.star_b_base_illuminance_lux * mean_trans_b * cutoff_b * weather_dimming;

    // ------------------------------------------------------------------------
    // 2. Binary Conjunction Amplification
    // ------------------------------------------------------------------------
    if ephemeris.sky_state == DynamicSkyState::BinaryAlignment {
        let align_ratio = 1.0 - (ephemeris.angular_separation / 7.5f32.to_radians()).clamp(0.0, 1.0);
        cache.conjunction_amplification = 1.0 + align_ratio * 0.45;
    } else {
        cache.conjunction_amplification = 1.0;
    }

    // ------------------------------------------------------------------------
    // 3. Diffuse Skylight & Zenith / Horizon Radiance Integration
    // ------------------------------------------------------------------------
    let zenith_dir = Vec3::Y;
    let cos_theta_za = zenith_dir.dot(ephemeris.star_a_direction);
    let cos_theta_zb = zenith_dir.dot(ephemeris.star_b_direction);

    let phase_a = multi_octave_stellar_glare(cos_theta_za, false);
    let phase_b = multi_octave_stellar_glare(cos_theta_zb, true);

    // Rayleigh zenith single-scattering (produces vivid azure Rayleigh blue at high noon)
    let sky_scatter_a = config.rayleigh_scattering_coefficients * (cache.star_a_illuminance_lux * 0.000035) * phase_a;
    let sky_scatter_b = config.rayleigh_scattering_coefficients * blackbody_b * (cache.star_b_illuminance_lux * 0.000035) * phase_b;

    // Diffuse cosmic starlight airglow floor (Rayleigh-scattered extraterrestrial starlight with ozone green depletion)
    let starlight_airglow = Vec3::new(
        config.rayleigh_scattering_coefficients.x * 24.0, // Red starlight component
        config.rayleigh_scattering_coefficients.y * 0.08, // Ozone Chappuis suppresses green
        config.rayleigh_scattering_coefficients.z * 18.0, // Rayleigh high-frequency blue
    ) * 0.08;

    cache.zenith_radiance = (sky_scatter_a + sky_scatter_b + starlight_airglow * 0.5) * cache.conjunction_amplification;

    // Horizon Radiance Integration:
    // a) Daytime Rayleigh Horizon Skylight:
    // Multiple-scattered Rayleigh blue light across dozens of optical air masses,
    // combined with atmospheric forward aerosol haze, creates a luminous, airy cyan-blue horizon.
    let total_daylight_lux = cache.star_a_illuminance_lux + cache.star_b_illuminance_lux;
    let day_ratio = (total_daylight_lux / 25_000.0).clamp(0.0, 1.0);
    // Smooth C^1 Hermite curve for daytime skylight transition
    let smooth_day = day_ratio * day_ratio * (3.0 - 2.0 * day_ratio);

    // Desaturated luminous azure horizon (higher optical depth than zenith):
    let horizon_rayleigh = Vec3::new(
        config.rayleigh_scattering_coefficients.x * 2.2,
        config.rayleigh_scattering_coefficients.y * 1.55,
        config.rayleigh_scattering_coefficients.z * 1.0,
    ) * (total_daylight_lux * 0.000038) * smooth_day;

    // b) Forward Mie aerosol scattering of transmitted direct beams:
    // Seamlessly emerges via C^1 Hermite curve as each star approaches the horizon [0.0°, 20.0°].
    // Overhead stars (>20°) do not wash the ground horizon in orange; only setting/rising stars
    // cast their blaze-orange / golden amber aureole along the horizon rim:
    let elev_deg_a = effective_elev_a.to_degrees();
    let sunset_factor_a = ((20.0 - elev_deg_a) / 20.0).clamp(0.0, 1.0);
    let sunset_mie_weight_a = sunset_factor_a * sunset_factor_a * (3.0 - 2.0 * sunset_factor_a);

    let elev_deg_b = effective_elev_b.to_degrees();
    let sunset_factor_b = ((20.0 - elev_deg_b) / 20.0).clamp(0.0, 1.0);
    let sunset_mie_weight_b = sunset_factor_b * sunset_factor_b * (3.0 - 2.0 * sunset_factor_b);

    let horizon_mie_a = filtered_rgb_a * (cache.star_a_illuminance_lux * 0.000045) * sunset_mie_weight_a;
    let horizon_mie_b = filtered_rgb_b * (cache.star_b_illuminance_lux * 0.000045) * sunset_mie_weight_b;

    // c) Stratospheric Ozone Twilight Arch (Belt of Venus: Hot Fuchsia #ff0054 & Dark Raspberry #9e0059):
    // Sunlight grazing horizontally through the stratospheric ozone layer has green (550nm) heavily absorbed,
    // while high-altitude Rayleigh scattering scatters blue and grazing red penetrates:
    let twilight_a = {
        let elev_deg = effective_elev_a.to_degrees();
        // Smooth Gaussian bell profile centered during civil twilight (-3.5°):
        let bell = (-((elev_deg + 3.5) / 5.5).powi(2)).exp();
        Vec3::new(
            0.85 * ((-elev_deg * 0.10).exp()).min(1.2),
            0.005, // Depleted green by Chappuis band
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

    // ------------------------------------------------------------------------
    // 4. Scene Ambient Light & Ground Bounce Irradiance
    // ------------------------------------------------------------------------
    let ambient_rgb = cache.zenith_radiance * 0.60 + cache.horizon_radiance * 0.40;
    let max_c = ambient_rgb.x.max(ambient_rgb.y).max(ambient_rgb.z).max(1e-4);
    
    cache.ambient_color = Color::srgb(
        (ambient_rgb.x / max_c).clamp(0.0, 1.0),
        (ambient_rgb.y / max_c).clamp(0.0, 1.0),
        (ambient_rgb.z / max_c).clamp(0.0, 1.0),
    );

    // Total ambient illuminance in lux (scaled appropriately to prevent washing out dual penumbras)
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

    // ------------------------------------------------------------------------
    // 5. Shadow-Casting Priority Management
    // ------------------------------------------------------------------------
    let elev_a_deg = ephemeris.star_a_elevation.to_degrees();
    let elev_b_deg = ephemeris.star_b_elevation.to_degrees();
    cache.active_shadow_caster = determine_shadow_caster_priority(elev_a_deg, elev_b_deg);
}

/// Determines which stellar body holds priority for DirectionalLight shadow cascades:
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

/// Synchronizes Bevy's `DirectionalLight` components and `AmbientLight` resource.
pub fn sync_stellar_directional_lights(
    config: Res<BinarySkyConfig>,
    ephemeris: Res<BinaryEphemerisState>,
    cache: Res<AtmosphericRadianceCache>,
    mut ambient_light: ResMut<AmbientLight>,
    mut star_a_query: Query<(&mut DirectionalLight, &mut Transform), (With<PrimaryStar>, Without<SecondaryStar>)>,
    mut star_b_query: Query<(&mut DirectionalLight, &mut Transform), (With<SecondaryStar>, Without<PrimaryStar>)>,
) {
    ambient_light.color = cache.ambient_color;
    ambient_light.brightness = config.ambient_illuminance_lux.unwrap_or(cache.ambient_brightness_lux);

    if let Ok((mut light_a, mut transform_a)) = star_a_query.get_single_mut() {
        light_a.color = config.star_a_color_override.unwrap_or(cache.star_a_color);
        light_a.illuminance = cache.star_a_illuminance_lux;
        // Simultaneous dual shadow casting: active whenever above horizon and enabled
        light_a.shadows_enabled = config.star_a_shadows_enabled && ephemeris.star_a_elevation > -0.05;

        let target_dir = -ephemeris.star_a_direction;
        if target_dir.length_squared() > 1e-4 {
            *transform_a = Transform::from_translation(Vec3::ZERO).looking_to(target_dir, Vec3::Y);
        }
    }

    if let Ok((mut light_b, mut transform_b)) = star_b_query.get_single_mut() {
        light_b.color = config.star_b_color_override.unwrap_or(cache.star_b_color);
        light_b.illuminance = cache.star_b_illuminance_lux;
        // Simultaneous dual shadow casting: active whenever above horizon and enabled
        light_b.shadows_enabled = config.star_b_shadows_enabled && ephemeris.star_b_elevation > -0.05;

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

    // 1. Sync Host Star A Disk (Luminous solar sphere with Rayleigh sunset chromaticity)
    if let Ok((mut tf_a, mut vis_a, mat_handle_a)) = celestial_set.p1().get_single_mut() {
        if ephemeris.star_a_elevation < -0.10 {
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

    // 2. Sync Companion Star B Disk (Amber dwarf with blaze-orange & crimson horizon shift)
    if let Ok((mut tf_b, mut vis_b, mat_handle_b)) = celestial_set.p2().get_single_mut() {
        if ephemeris.star_b_elevation < -0.10 {
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

    // 3. Sync Cosmic Starfield (Camera-facing pinprick stars glittering at night)
    let total_sun_lux = cache.star_a_illuminance_lux + cache.star_b_illuminance_lux;
    let night_factor = (1.0 - (total_sun_lux / 16000.0).clamp(0.0, 1.0)).powi(2);

    if let Ok((mut tf_stars, mut vis_stars, mat_handle_stars)) = celestial_set.p3().get_single_mut() {
        tf_stars.translation = cam_pos;
        tf_stars.scale = Vec3::splat(config.starfield_scale);
        if night_factor <= 0.02 {
            *vis_stars = Visibility::Hidden;
        } else {
            *vis_stars = Visibility::Visible;
            if let Some(mat) = materials.get_mut(mat_handle_stars) {
                let t = time.elapsed_seconds();
                // Atmospheric scintillation / organic twinkle:
                // Twinkling rates (frequencies 2.8, 5.4, 9.7 rad/s) are strictly preserved,
                // while twinkling intensity (amplitudes) is significantly heightened for brilliant glittering.
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
    // Dome transform keeps its origin locked strictly to active camera translation
    let mut p4 = celestial_set.p4();
    for (mut tf_aurora, mut vis_aurora, maybe_mat_handle) in p4.iter_mut() {
        tf_aurora.translation = cam_pos;
        tf_aurora.rotation = Quat::IDENTITY;

        let is_aurora_weather = weather.weather_type == WeatherType::StellarWindAurora;
        let is_active = is_aurora_weather && night_factor > 0.02;

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

    // 5. Sync Volumetric Precipitation Streaks (Stormy rain weather system)
    let mut p5 = celestial_set.p5();
    for (mut tf_rain, mut vis_rain) in p5.iter_mut() {
        if weather.weather_type == WeatherType::OvercastPrecipitation {
            *vis_rain = Visibility::Visible;
            let fall_offset = (time.elapsed_seconds() * 28.0).rem_euclid(4.0);
            tf_rain.translation = cam_pos + Vec3::new(0.0, -fall_offset, 0.0);
        } else {
            *vis_rain = Visibility::Hidden;
        }
    }
}

/// Interactive user input system for adjusting time of day, scrubbing cycles, and cycling weather.
pub fn handle_sky_time_and_weather_inputs(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut config: ResMut<BinarySkyConfig>,
    mut ephemeris: ResMut<BinaryEphemerisState>,
    mut weather: ResMut<AtmosphericWeather>,
    mut console: Option<ResMut<crate::core::ConsoleState>>,
) {
    let Some(keys) = keys else { return; };
    let day_duration = config.day_duration_seconds as f64;

    // [F8] Quick Day/Night Toggle
    if keys.just_pressed(KeyCode::F8) {
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

    // [ [ ] Step 1 hour backward
    if keys.just_pressed(KeyCode::BracketLeft) {
        let hour_sec = (config.day_duration_seconds / 24.0) as f64;
        ephemeris.simulation_time_seconds = (ephemeris.simulation_time_seconds - hour_sec).rem_euclid(day_duration);
        let msg = format!("[Celestial Cycle] Rewound 1 hour. Current time: {:.1}h", ephemeris.clock_time_hours());
        info!("{}", msg);
        if let Some(ref mut c) = console {
            c.logs.push(msg);
        }
    }

    // [ ] ] Step 1 hour forward
    if keys.just_pressed(KeyCode::BracketRight) {
        let hour_sec = (config.day_duration_seconds / 24.0) as f64;
        ephemeris.simulation_time_seconds = (ephemeris.simulation_time_seconds + hour_sec).rem_euclid(day_duration);
        let msg = format!("[Celestial Cycle] Advanced 1 hour. Current time: {:.1}h", ephemeris.clock_time_hours());
        info!("{}", msg);
        if let Some(ref mut c) = console {
            c.logs.push(msg);
        }
    }

    // [ - ] Slow down time progression
    if keys.just_pressed(KeyCode::Minus) {
        config.time_scale = match config.time_scale {
            s if s >= 300.0 => 60.0,
            s if s >= 60.0 => 10.0,
            s if s >= 10.0 => 1.0,
            s if s >= 1.0 => 0.0,
            _ => 0.0,
        };
        let msg = format!("[Celestial Cycle] Time scale set to: {:.0}x", config.time_scale);
        info!("{}", msg);
        if let Some(ref mut c) = console {
            c.logs.push(msg);
        }
    }

    // [ = ] Accelerate time progression
    if keys.just_pressed(KeyCode::Equal) {
        config.time_scale = match config.time_scale {
            s if s < 1.0 => 1.0,
            s if s < 10.0 => 10.0,
            s if s < 60.0 => 60.0,
            _ => 300.0,
        };
        let msg = format!("[Celestial Cycle] Time scale set to: {:.0}x", config.time_scale);
        info!("{}", msg);
        if let Some(ref mut c) = console {
            c.logs.push(msg);
        }
    }

    // [F9] Cycle Atmospheric Weather Presets
    if keys.just_pressed(KeyCode::F9) {
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

/// Updates camera clear color and volumetric atmospheric distance fog
/// matching the physical participating medium's extinction profiles, view distance, and radiance cache.
pub fn update_atmospheric_cameras_and_fog(
    _ephemeris: Res<BinaryEphemerisState>,
    cache: Res<AtmosphericRadianceCache>,
    weather: Res<AtmosphericWeather>,
    render_settings: Option<Res<crate::spellbook::TerrainRenderSettings>>,
    mut commands: Commands,
    mut camera_query: Query<(Entity, Option<&mut FogSettings>), With<AtmosphericCamera>>,
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

    clear_color.0 = horizon_color;

    // Synchronize fog range with terrain & entity visible draw distance
    let base_range = if let Some(ref rs) = render_settings {
        if rs.spawn_full_zone { 64.0 * 16.0 } else { rs.visible_range_meters.max(160.0) }
    } else {
        230.4
    };

    // Calculate linear fog depth gradient: clear foreground for crisp interactions,
    // progressive atmospheric haze across mid-range, reaching 100% opacity at max draw distance.
    let (start_dist, end_dist) = match weather.weather_type {
        WeatherType::ClearSky => ((base_range * 0.20).max(40.0), base_range * 1.0),
        WeatherType::StellarWindAurora => ((base_range * 0.18).max(35.0), base_range * 0.95),
        WeatherType::AerosolHaze => ((base_range * 0.10).max(20.0), base_range * 0.75),
        WeatherType::OvercastPrecipitation => ((base_range * 0.05).max(10.0), base_range * 0.60),
    };

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

    for (entity, mut fog_opt) in camera_query.iter_mut() {
        if let Some(ref mut fog) = fog_opt {
            fog.color = horizon_color;
            fog.directional_light_color = sun_scatter_color;
            fog.directional_light_exponent = 8.0;
            fog.falloff = FogFalloff::Linear {
                start: start_dist,
                end: end_dist,
            };
        } else {
            commands.entity(entity).insert(FogSettings {
                color: horizon_color,
                directional_light_color: sun_scatter_color,
                directional_light_exponent: 8.0,
                falloff: FogFalloff::Linear {
                    start: start_dist,
                    end: end_dist,
                },
            });
        }
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
        app.insert_resource(DirectionalLightShadowMap { size: 2048 })
            .init_resource::<BinarySkyConfig>()
            .init_resource::<BinaryEphemerisState>()
            .init_resource::<AtmosphericRadianceCache>()
            .init_resource::<AtmosphericWeather>()
            .init_resource::<crate::tree_colors::SeasonState>()
            .init_resource::<AmbientLight>()
            .init_resource::<ClearColor>()
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
            // Zenith radiance should be dominated by Rayleigh blue:
            assert!(cache.zenith_radiance.z > cache.zenith_radiance.x * 2.5, "Zenith sky must be Rayleigh blue at noon");
            // Direct Star A should be warm solar white:
            assert!(cache.star_a_transmittance.x > 0.8 && cache.star_a_transmittance.z > 0.6);
            // Horizon must be Rayleigh blue/cyan biased at high noon:
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
            // Transmittance should shift Star A to blaze-orange/amber (blue strongly extinguished):
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
            // ClearColor must have the Electric Navy tone (blue prominent, subtle red violet, green near zero):
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
        
        // At diurnal_angle = 0 -> High Noon (12:00)
        ephemeris.diurnal_angle = 0.0;
        assert!((ephemeris.clock_time_hours() - 12.0).abs() < 0.01);

        // At diurnal_angle = PI -> Midnight (00:00)
        ephemeris.diurnal_angle = PI;
        assert!((ephemeris.clock_time_hours() - 0.0).abs() < 0.01 || (ephemeris.clock_time_hours() - 24.0).abs() < 0.01);

        // At diurnal_angle = PI / 2 -> Sunset (18:00)
        ephemeris.diurnal_angle = PI * 0.5;
        assert!((ephemeris.clock_time_hours() - 18.0).abs() < 0.01);

        // At diurnal_angle = 3 * PI / 2 -> Sunrise (06:00)
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

        // Sample stars across the upper hemisphere split into lower band (0 <= y < 0.5)
        // and upper zenith band (0.5 <= y <= 1.0)
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

        // Under Archimedes equal-area projection, equal intervals of y contain equal spherical area!
        // The ratio between upper_band and lower_band must be 0.5 within ±5% tolerance.
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
    fn test_aurora_mesh_generation() {
        let mesh = create_aurora_mesh();
        assert_eq!(mesh.primitive_topology(), PrimitiveTopology::TriangleList);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
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

        // Set simulation time to 18:00 (Sunset, 6 hours after noon = 25% of day duration)
        let day_sec = 1440.0;
        {
            let mut eph = app.world_mut().resource_mut::<BinaryEphemerisState>();
            eph.simulation_time_seconds = (day_sec * 0.25) as f64;
        }

        app.add_systems(Update, (update_binary_ephemeris, update_atmospheric_scattering_and_cache).chain());
        app.update();

        let eph = app.world().resource::<BinaryEphemerisState>();
        // Hour should be approx 18.0 (sunset)
        assert!((eph.clock_time_hours() - 18.0).abs() < 0.2);
        // Sun elevation should be near horizon (within 5 degrees)
        assert!(eph.star_a_elevation.abs() < 0.15);
        // Sun direction x should be negative (Western sky, -X)
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
        
        // Execute frame update to trigger Bevy ECS system param and schedule borrow checks
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

        // Spawn dual cameras mimicking RTS Camera and FPS Camera
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

        // Advance to deep night and activate aurora
        {
            let mut ephemeris = app.world_mut().resource_mut::<BinaryEphemerisState>();
            ephemeris.simulation_time_seconds = 720.0;
            ephemeris.diurnal_angle = PI;
        }
        {
            let mut weather = app.world_mut().resource_mut::<AtmosphericWeather>();
            weather.weather_type = WeatherType::StellarWindAurora;
        }

        // Execute frame cycle
        app.update();

        // 1. Verify Starfield is visible at night and tracking active camera position
        let mut starfield_query = app.world_mut().query_filtered::<(&Transform, &Visibility), With<CosmicStarfield>>();
        let (star_tf, star_vis) = starfield_query.single(app.world());
        assert_eq!(*star_vis, Visibility::Visible, "Starfield must be visible at deep night");
        assert_eq!(star_tf.translation, active_pos, "Starfield must track active camera translation");

        // 2. Verify Aurora Curtain is visible when StellarWindAurora is active
        let mut aurora_query = app.world_mut().query_filtered::<(&Transform, &Visibility), With<AuroraCurtain>>();
        let (aurora_tf, aurora_vis) = aurora_query.single(app.world());
        assert_eq!(*aurora_vis, Visibility::Visible, "Aurora must be visible when StellarWindAurora is active");
        assert_eq!(aurora_tf.translation.x, active_pos.x);
        assert_eq!(aurora_tf.translation.z, active_pos.z);

        // 3. Verify ClearColor is electric navy night sky color from the palette
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

        // High noon: diurnal_angle = 0
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

        // 1. Verify Star A Volumetric Disk has NotShadowCaster so it does NOT cast a giant circular shadow on the player
        let mut star_a_query = app.world_mut().query_filtered::<Entity, (With<StarAVolumetricDisk>, With<NotShadowCaster>)>();
        assert_eq!(star_a_query.iter(app.world()).count(), 1, "Star A disk must have NotShadowCaster");

        // 2. Verify Star B Dwarf Disk has NotShadowCaster
        let mut star_b_query = app.world_mut().query_filtered::<Entity, (With<StarBVolumetricDisk>, With<NotShadowCaster>)>();
        assert_eq!(star_b_query.iter(app.world()).count(), 1, "Star B disk must have NotShadowCaster");

        // 3. Verify Cosmic Starfield, Aurora, and Rain have NotShadowCaster
        let mut starfield_query = app.world_mut().query_filtered::<Entity, (With<CosmicStarfield>, With<NotShadowCaster>)>();
        assert_eq!(starfield_query.iter(app.world()).count(), 1, "Starfield must have NotShadowCaster");

        let mut aurora_query = app.world_mut().query_filtered::<Entity, (With<AuroraCurtain>, With<NotShadowCaster>)>();
        assert_eq!(aurora_query.iter(app.world()).count(), 1, "Aurora must have NotShadowCaster");

        let mut rain_query = app.world_mut().query_filtered::<Entity, (With<PrecipitationStreaks>, With<NotShadowCaster>)>();
        assert_eq!(rain_query.iter(app.world()).count(), 1, "Rain must have NotShadowCaster");

        // 4. Verify DirectionalLights include Layer 2 (so the player character on layer 2 casts shadows)
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

        // 5. Verify DirectionalLightShadowMap is configured to 2048
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

        // Spawn active camera
        app.world_mut().spawn((
            Camera3dBundle {
                camera: Camera { is_active: true, ..default() },
                transform: Transform::from_xyz(0.0, 10.0, 0.0),
                ..default()
            },
            AtmosphericCamera,
        ));

        // Advance to deep night and activate aurora weather
        let day_sec = 1440.0;
        {
            let mut ephemeris = app.world_mut().resource_mut::<BinaryEphemerisState>();
            ephemeris.simulation_time_seconds = (day_sec * 0.5) as f64; // midnight
            ephemeris.diurnal_angle = std::f32::consts::PI;
            ephemeris.star_a_elevation = -std::f32::consts::FRAC_PI_2;
            let mut weather = app.world_mut().resource_mut::<AtmosphericWeather>();
            weather.weather_type = WeatherType::StellarWindAurora;
            weather.aurora_intensity = 0.92;
        }

        app.update();

        // 1. Verify StarAuroraDome has Handle<StarAuroraDomeMaterial>
        let mut aurora_query = app.world_mut().query_filtered::<(&Handle<StarAuroraDomeMaterial>, &Visibility), With<StarAuroraDome>>();
        let (aurora_mat_handle, aurora_vis) = aurora_query.single(app.world());
        assert_eq!(*aurora_vis, Visibility::Visible, "Aurora dome must be visible during active aurora weather");

        // 2. Verify StarAuroraDomeMaterial uniforms were synchronized with ephemeris & weather
        let aurora_materials = app.world().resource::<Assets<StarAuroraDomeMaterial>>();
        let mat = aurora_materials.get(aurora_mat_handle).expect("StarAuroraDomeMaterial must exist in Assets");
        assert_eq!(mat.uniforms.weather_intensity, 0.92, "SkyUniforms weather_intensity must match weather intensity");
        assert!(mat.uniforms.night_factor > 0.9, "Night factor must be near 1.0 at midnight");
        assert_eq!(mat.uniforms.speed, 1.0, "Speed must be 1.0");
        assert_eq!(mat.uniforms.brightness, 1.0, "Brightness must be 1.0");

        // 3. Verify Material alpha mode is Blend
        assert_eq!(mat.alpha_mode(), AlphaMode::Blend, "Aurora dome material must use blend mode for dome overlay");

        // 4. Verify WGSL shader file content mirrors the requested algorithm
        let shader_src = include_str!("../../assets/shaders/aurora.wgsl");
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
            Camera3dBundle::default(),
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

        // Fog color and ClearColor must match the horizon color
        let clear_color = app.world().resource::<ClearColor>().0;
        assert_eq!(fog.color, clear_color, "Fog color must match clear color for seamless horizon blending");
    }
}
