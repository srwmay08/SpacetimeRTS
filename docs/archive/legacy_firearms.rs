// ============================================================================
// File: docs/archive/legacy_firearms.rs
// ============================================================================
// ----------------------------------------------------------------------------
// ARCHIVED LEGACY FIREARMS & MODERN BALLISTICS SYSTEM
// ----------------------------------------------------------------------------
// Architectural Note:
// Preserves the historical implementation of modern firearms (Revolver, Shotgun,
// Sniper Rifle) and reticle-adjacent ammo/ready indicators.
//
// These components were removed from active fantasy RTS gameplay to maintain
// thematic genre coherence (pure high-fantasy blades, bludgeons, bows, crossbows,
// polearms, and arcane implements) and to prepare for clean Bevy Taffy UI layouts.
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use crate::trees::LowPolyMeshBuilder;

// ----------------------------------------------------------------------------
// 1. ARCHIVED RETICLE-ADJACENT AMMO & READY GAUGE
// ----------------------------------------------------------------------------
// Rendered right of the crosshair with dynamic status:
// - "[ 6 / 6 ]" for revolver ammo count
// - "[ RELOADING ]" / "[ PUMPING ]" for pump-action and cylinder reload states
// - "[ READY ]" indicator for idle/loaded states
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct LegacyReticleAmmoText;

pub fn legacy_update_reticle_adjacent_status(
    weapon_state: &LegacyWeaponState,
    text: &mut Text,
) {
    let val = match weapon_state.current_weapon {
        LegacyWeaponType::Revolver => {
            if weapon_state.revolver_is_reloading {
                "[ RELOADING ]".to_string()
            } else {
                format!("[ {} / {} ]", weapon_state.revolver_ammo, weapon_state.revolver_max_ammo)
            }
        }
        LegacyWeaponType::Shotgun => {
            if weapon_state.shotgun_is_reloading {
                "[ RELOADING ]".to_string()
            } else if weapon_state.shotgun_is_pumping {
                "[ PUMPING ]".to_string()
            } else {
                format!("[ {} / {} ]", weapon_state.shotgun_ammo, weapon_state.shotgun_max_ammo)
            }
        }
        LegacyWeaponType::Crossbow => {
            if weapon_state.crossbow_loaded {
                "[ BOLT READY ]".to_string()
            } else {
                format!("[ CRANK {:.1}s ]", weapon_state.crossbow_reload_timer.remaining_secs())
            }
        }
        LegacyWeaponType::Bow => {
            if weapon_state.bow_drawing {
                format!("[ DRAW: {}% ]", (weapon_state.bow_charge * 100.0) as u32)
            } else {
                "[ READY ]".to_string()
            }
        }
        LegacyWeaponType::None => "".to_string(),
        _ => "[ READY ]".to_string(),
    };

    let color = match weapon_state.current_weapon {
        LegacyWeaponType::Revolver if weapon_state.revolver_ammo == 0 => Color::srgb(1.0, 0.2, 0.2),
        LegacyWeaponType::Revolver if weapon_state.revolver_ammo <= 2 => Color::srgb(1.0, 0.7, 0.1),
        LegacyWeaponType::Shotgun if weapon_state.shotgun_ammo == 0 => Color::srgb(1.0, 0.2, 0.2),
        LegacyWeaponType::Shotgun if weapon_state.shotgun_ammo == 1 => Color::srgb(1.0, 0.7, 0.1),
        _ => Color::srgb(0.0, 1.0, 1.0),
    };

    text.sections[0].value = val;
    text.sections[0].style.color = color;
}

// ----------------------------------------------------------------------------
// 2. ARCHIVED FIREARM WEAPON STATE & TIMERS
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyWeaponType {
    None,
    Revolver,
    Shotgun,
    SniperRifle,
    Bow,
    Crossbow,
}

pub struct LegacyWeaponState {
    pub current_weapon: LegacyWeaponType,
    // Revolver Mechanics
    pub revolver_ammo: u32,
    pub revolver_max_ammo: u32,
    pub revolver_cooldown: Timer,
    pub revolver_reload_timer: Timer,
    pub revolver_is_reloading: bool,
    // Shotgun Mechanics
    pub shotgun_ammo: u32,
    pub shotgun_max_ammo: u32,
    pub shotgun_pump_timer: Timer,
    pub shotgun_is_pumping: bool,
    pub shotgun_reload_timer: Timer,
    pub shotgun_is_reloading: bool,
    // Bow/Crossbow
    pub bow_charge: f32,
    pub bow_drawing: bool,
    pub crossbow_loaded: bool,
    pub crossbow_reload_timer: Timer,
    // Dynamic Bloom & Recoil
    pub dynamic_bloom: f32,
    pub recoil_offset: Vec3,
    pub recoil_rot: Quat,
}

impl Default for LegacyWeaponState {
    fn default() -> Self {
        Self {
            current_weapon: LegacyWeaponType::None,
            revolver_ammo: 6,
            revolver_max_ammo: 6,
            revolver_cooldown: Timer::from_seconds(0.28, TimerMode::Once),
            revolver_reload_timer: Timer::from_seconds(2.1, TimerMode::Once),
            revolver_is_reloading: false,
            shotgun_ammo: 4,
            shotgun_max_ammo: 4,
            shotgun_pump_timer: Timer::from_seconds(0.65, TimerMode::Once),
            shotgun_is_pumping: false,
            shotgun_reload_timer: Timer::from_seconds(1.8, TimerMode::Once),
            shotgun_is_reloading: false,
            bow_charge: 0.0,
            bow_drawing: false,
            crossbow_loaded: true,
            crossbow_reload_timer: Timer::from_seconds(1.4, TimerMode::Once),
            dynamic_bloom: 0.0,
            recoil_offset: Vec3::ZERO,
            recoil_rot: Quat::IDENTITY,
        }
    }
}

// ----------------------------------------------------------------------------
// 3. ARCHIVED PROCEDURAL LOW-POLY FIREARM MESH GENERATORS
// ----------------------------------------------------------------------------

/// Procedural low-poly 6-shot cylinder revolver mesh.
pub fn create_legacy_revolver_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    let gunmetal = [0.25, 0.26, 0.28, 1.0];
    let steel = [0.45, 0.47, 0.50, 1.0];
    let wood = [0.38, 0.22, 0.12, 1.0];

    // Contoured grip handle
    builder.add_tapered_box(
        Vec2::new(-0.025, -0.015), Vec2::new(0.025, 0.015), -0.16,
        Vec2::new(-0.02, -0.012), Vec2::new(0.02, 0.012), -0.04,
        wood,
    );

    // Frame & trigger guard
    builder.add_faceted_box(Vec3::new(-0.028, -0.05, -0.05), Vec3::new(0.028, 0.04, 0.06), gunmetal);
    builder.add_faceted_box(Vec3::new(-0.008, -0.07, 0.0), Vec3::new(0.008, -0.045, 0.05), steel);

    // Fluted 6-chamber cylinder
    builder.add_tapered_prism(
        Vec3::new(0.0, 0.0, 0.01), Vec3::new(0.0, 0.0, 0.09),
        0.038, 0.038, 6, steel, true, true,
    );

    // Solid barrel and front blade sight
    builder.add_tapered_prism(
        Vec3::new(0.0, 0.02, 0.09), Vec3::new(0.0, 0.02, 0.32),
        0.022, 0.019, 6, gunmetal, true, true,
    );
    builder.add_faceted_box(Vec3::new(-0.004, 0.038, 0.29), Vec3::new(0.004, 0.048, 0.31), steel);

    builder.build()
}

/// Procedural low-poly pump-action shotgun mesh with underbarrel tubular magazine.
pub fn create_legacy_shotgun_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    let dark_metal = [0.20, 0.21, 0.23, 1.0];
    let walnut = [0.32, 0.18, 0.09, 1.0];
    let steel = [0.42, 0.44, 0.46, 1.0];

    // Solid walnut shoulder stock
    builder.add_tapered_box(
        Vec2::new(-0.035, -0.08), Vec2::new(0.035, 0.02), -0.42,
        Vec2::new(-0.025, -0.04), Vec2::new(0.025, 0.01), -0.15,
        walnut,
    );

    // Heavy rectangular receiver
    builder.add_faceted_box(Vec3::new(-0.032, -0.05, -0.15), Vec3::new(0.032, 0.04, 0.12), dark_metal);

    // Primary barrel (12-gauge smoothbore)
    builder.add_tapered_prism(
        Vec3::new(0.0, 0.015, 0.12), Vec3::new(0.0, 0.015, 0.62),
        0.024, 0.022, 8, steel, true, true,
    );

    // Underbarrel magazine tube
    builder.add_tapered_prism(
        Vec3::new(0.0, -0.022, 0.12), Vec3::new(0.0, -0.022, 0.54),
        0.018, 0.018, 6, dark_metal, true, true,
    );

    builder.build()
}

/// Procedural low-poly pump slide for animated shotgun viewmodels.
pub fn create_legacy_pumpslide_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    let ribbed_wood = [0.28, 0.15, 0.08, 1.0];

    builder.add_tapered_prism(
        Vec3::new(0.0, -0.022, 0.20), Vec3::new(0.0, -0.022, 0.35),
        0.032, 0.032, 8, ribbed_wood, true, true,
    );

    builder.build()
}

/// Procedural low-poly bolt-action sniper rifle with high-magnification scope and bipod.
pub fn create_legacy_sniper_rifle_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    let composite = [0.15, 0.16, 0.17, 1.0];
    let matte_black = [0.10, 0.11, 0.12, 1.0];
    let optic_glass = [0.12, 0.45, 0.60, 1.0];

    // Ergonomic thumbhole composite stock
    builder.add_tapered_box(
        Vec2::new(-0.03, -0.08), Vec2::new(0.03, 0.02), -0.50,
        Vec2::new(-0.025, -0.04), Vec2::new(0.025, 0.01), -0.18,
        composite,
    );

    // Reinforced receiver and bolt handle
    builder.add_faceted_box(Vec3::new(-0.03, -0.04, -0.18), Vec3::new(0.03, 0.035, 0.15), matte_black);

    // Free-floating precision match barrel
    builder.add_tapered_prism(
        Vec3::new(0.0, 0.01, 0.15), Vec3::new(0.0, 0.01, 0.88),
        0.020, 0.016, 8, matte_black, true, true,
    );

    // High-magnification optical scope
    builder.add_tapered_prism(
        Vec3::new(0.0, 0.065, -0.06), Vec3::new(0.0, 0.065, 0.18),
        0.028, 0.032, 8, matte_black, true, true,
    );
    builder.add_faceted_box(Vec3::new(-0.02, 0.045, 0.175), Vec3::new(0.02, 0.085, 0.18), optic_glass);

    builder.build()
}

// ----------------------------------------------------------------------------
// 4. ARCHIVED FIREARM COMBAT DISPATCH & BALLISTICS
// ----------------------------------------------------------------------------
// Historical implementation of hitscan raycasts, 8-pellet buckshot spread cone,
// recoil offsets, and tracer cylinders.
// ----------------------------------------------------------------------------

/// Historical main-hand firearm firing dispatcher.
pub fn legacy_dispatch_firearm_fire(
    weapon: LegacyWeaponType,
    weapon_state: &mut LegacyWeaponState,
    origin: Vec3,
    dir: Vec3,
    cam_right: Vec3,
    cam_up: Vec3,
) -> Vec<(Vec3, Vec3, f32)> { // (tracer_start, tracer_end, thickness)
    let mut tracers = Vec::new();

    match weapon {
        LegacyWeaponType::Revolver => {
            if weapon_state.revolver_ammo > 0 && weapon_state.revolver_cooldown.finished() && !weapon_state.revolver_is_reloading {
                weapon_state.revolver_ammo -= 1;
                weapon_state.revolver_cooldown.reset();

                weapon_state.recoil_offset += Vec3::new(0.0, 0.065, 0.11);
                weapon_state.recoil_rot *= Quat::from_rotation_x(-0.48);
                weapon_state.dynamic_bloom = (weapon_state.dynamic_bloom + 8.0).min(30.0);

                let distance = 80.0;
                let target = origin + dir * distance;
                tracers.push((origin, target, 0.018));

                if weapon_state.revolver_ammo == 0 {
                    weapon_state.revolver_is_reloading = true;
                    weapon_state.revolver_reload_timer.reset();
                }
            }
        }
        LegacyWeaponType::Shotgun => {
            if weapon_state.shotgun_ammo > 0 && !weapon_state.shotgun_is_pumping && !weapon_state.shotgun_is_reloading {
                weapon_state.shotgun_ammo -= 1;
                weapon_state.shotgun_is_pumping = true;
                weapon_state.shotgun_pump_timer.reset();

                weapon_state.recoil_offset += Vec3::new(0.0, 0.095, 0.15);
                weapon_state.recoil_rot *= Quat::from_rotation_x(-0.62);
                weapon_state.dynamic_bloom = (weapon_state.dynamic_bloom + 16.0).min(40.0);

                let spread_offsets = [
                    (0.0, 0.0), (0.025, 0.02), (-0.025, 0.02), (0.02, -0.025),
                    (-0.02, -0.025), (0.04, 0.005), (-0.04, -0.005), (0.005, 0.04),
                ];

                for (sx, sy) in spread_offsets {
                    let pellet_dir = (dir + cam_right * sx + cam_up * sy).normalize();
                    let distance = 40.0;
                    let target = origin + pellet_dir * distance;
                    tracers.push((origin, target, 0.012));
                }

                if weapon_state.shotgun_ammo == 0 {
                    weapon_state.shotgun_is_reloading = true;
                    weapon_state.shotgun_reload_timer.reset();
                }
            }
        }
        LegacyWeaponType::SniperRifle => {
            weapon_state.recoil_offset += Vec3::new(0.0, 0.08, 0.16);
            weapon_state.recoil_rot *= Quat::from_rotation_x(-0.55);
            weapon_state.dynamic_bloom = (weapon_state.dynamic_bloom + 20.0).min(40.0);

            let distance = 150.0;
            let target = origin + dir * distance;
            tracers.push((origin, target, 0.015));
        }
        _ => {}
    }

    tracers
}

// ----------------------------------------------------------------------------
// 5. ARCHIVED FIREARM ANIMATION & RELOAD SYSTEM
// ----------------------------------------------------------------------------

pub fn legacy_animate_firearm_viewmodels(
    dt: f32,
    weapon_state: &mut LegacyWeaponState,
    pump_slide_z: &mut f32,
) {
    if weapon_state.revolver_is_reloading {
        weapon_state.revolver_reload_timer.tick(std::time::Duration::from_secs_f32(dt));
        if weapon_state.revolver_reload_timer.just_finished() {
            weapon_state.revolver_ammo = weapon_state.revolver_max_ammo;
            weapon_state.revolver_is_reloading = false;
            weapon_state.revolver_reload_timer.reset();
        }
    }
    weapon_state.revolver_cooldown.tick(std::time::Duration::from_secs_f32(dt));

    if weapon_state.shotgun_is_reloading {
        weapon_state.shotgun_reload_timer.tick(std::time::Duration::from_secs_f32(dt));
        if weapon_state.shotgun_reload_timer.just_finished() {
            weapon_state.shotgun_ammo = weapon_state.shotgun_max_ammo;
            weapon_state.shotgun_is_reloading = false;
            weapon_state.shotgun_reload_timer.reset();
        }
    }

    if weapon_state.shotgun_is_pumping {
        weapon_state.shotgun_pump_timer.tick(std::time::Duration::from_secs_f32(dt));
        let t = weapon_state.shotgun_pump_timer.fraction();
        *pump_slide_z = if t < 0.5 {
            -0.26 + (t * 2.0) * 0.08
        } else {
            -0.18 - ((t - 0.5) * 2.0) * 0.08
        };

        if weapon_state.shotgun_pump_timer.just_finished() {
            weapon_state.shotgun_is_pumping = false;
            weapon_state.shotgun_pump_timer.reset();
        }
    }
}

