// ============================================================================
// File: client/src/weapons/mesh_builder.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL LOW-POLY WEAPON MESH BUILDER
// ----------------------------------------------------------------------------

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::render::view::RenderLayers;
use crate::trees::LowPolyMeshBuilder;
use crate::components::*;
use crate::core::*;
use super::types::*;

pub const COL_WOOD_DARK: [f32; 4] = [0.35, 0.22, 0.12, 1.0];
pub const COL_WOOD_LIGHT: [f32; 4] = [0.55, 0.38, 0.22, 1.0];
pub const COL_IRON_DARK: [f32; 4] = [0.22, 0.23, 0.25, 1.0];
pub const COL_IRON_BRIGHT: [f32; 4] = [0.48, 0.50, 0.52, 1.0];
pub const COL_BRASS_GOLD: [f32; 4] = [0.78, 0.62, 0.22, 1.0];
pub const COL_STRING_WHITE: [f32; 4] = [0.92, 0.90, 0.82, 1.0];
pub const COL_RED_FLETCH: [f32; 4] = [0.85, 0.15, 0.15, 1.0];
pub const COL_RUNE_CYAN: [f32; 4] = [0.2, 0.85, 1.0, 1.0];
pub const COL_FIRE_ORANGE: [f32; 4] = [1.0, 0.45, 0.05, 1.0];
pub const COL_SKIN_TONE: [f32; 4] = [0.86, 0.68, 0.52, 1.0];
pub const COL_WRAP_CLOTH: [f32; 4] = [0.80, 0.78, 0.72, 1.0];

/// Builds a watertight low-poly nocked arrow mesh for first-person bows.
pub fn create_lowpoly_arrow_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    // Shaft
    builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.015, 0.015, 0.52), COL_WOOD_LIGHT);
    // Arrowhead
    builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.24), Vec3::new(0.0, 0.0, -0.30), 0.03, 0.002, 0.012, 0.002, COL_IRON_BRIGHT, true);
    // Fletching
    builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.22), Vec3::new(0.008, 0.04, 0.07), COL_RED_FLETCH);
    builder.build()
}

/// Builds a watertight low-poly bolt mesh for first-person crossbows.
pub fn create_lowpoly_bolt_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    // Shaft
    builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.018, 0.018, 0.30), COL_WOOD_LIGHT);
    // Piercing tip
    builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.14), Vec3::new(0.0, 0.0, -0.19), 0.025, 0.002, 0.025, 0.002, COL_IRON_BRIGHT, true);
    // Fletching
    builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.12), Vec3::new(0.005, 0.035, 0.05), COL_RED_FLETCH);
    builder.build()
}

/// Builds a watertight low-poly shotgun pump slide mesh.
#[allow(dead_code)]
pub fn create_lowpoly_pumpslide_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    builder.add_box_center_size(Vec3::ZERO, Vec3::new(0.054, 0.054, 0.14), COL_WOOD_LIGHT);
    builder.build()
}

/// Constructs a watertight, unified low-poly weapon mesh with per-vertex shading.
pub fn create_lowpoly_weapon_mesh(weapon: WeaponType) -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();

    match weapon {
        WeaponType::Bow => {
            // Central Handle Grip
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.04, 0.12, 0.04), COL_WOOD_DARK);
            // Upper Limb
            builder.add_box_center_size(Vec3::new(0.0, 0.12, -0.04), Vec3::new(0.035, 0.14, 0.035), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.24, -0.10), Vec3::new(0.03, 0.14, 0.03), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.33, -0.14), Vec3::new(0.025, 0.08, 0.025), COL_WOOD_LIGHT);
            // Lower Limb
            builder.add_box_center_size(Vec3::new(0.0, -0.12, -0.04), Vec3::new(0.035, 0.14, 0.035), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, -0.24, -0.10), Vec3::new(0.03, 0.14, 0.03), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.33, -0.14), Vec3::new(0.025, 0.08, 0.025), COL_WOOD_LIGHT);
            // Bowstring
            builder.add_box_center_size(Vec3::new(0.0, 0.17, -0.02), Vec3::new(0.005, 0.36, 0.005), COL_STRING_WHITE);
            builder.add_box_center_size(Vec3::new(0.0, -0.17, -0.02), Vec3::new(0.005, 0.36, 0.005), COL_STRING_WHITE);
        }
        WeaponType::Crossbow => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.065, 0.075, 0.52), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.04, 0.18), Vec3::new(0.055, 0.11, 0.16), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.09, 0.04), Vec3::new(0.045, 0.12, 0.06), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.03, -0.32), Vec3::new(0.52, 0.035, 0.035), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.01, -0.42), Vec3::new(0.12, 0.025, 0.08), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.042, -0.14), Vec3::new(0.03, 0.015, 0.38), COL_IRON_BRIGHT);
        }
        WeaponType::HandCrossbow => {
            builder.add_box_center_size(Vec3::new(0.0, -0.08, 0.05), Vec3::new(0.04, 0.11, 0.05), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.10), Vec3::new(0.045, 0.055, 0.32), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.025, -0.22), Vec3::new(0.32, 0.025, 0.025), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.04, -0.02), Vec3::new(0.02, 0.04, 0.03), COL_BRASS_GOLD);
        }
        WeaponType::Revolver | WeaponType::Shotgun | WeaponType::SniperRifle => {
            // Archived legacy firearm variants: simple lightweight grip
            builder.add_box_center_size(Vec3::new(0.0, -0.06, 0.05), Vec3::new(0.04, 0.10, 0.06), COL_WOOD_DARK);
        }
        WeaponType::BouncyBombLauncher => {
            builder.add_box_center_size(Vec3::new(0.0, -0.05, 0.16), Vec3::new(0.06, 0.11, 0.24), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.06, 0.02), Vec3::new(0.055, 0.12, 0.12), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.01, -0.12), Vec3::new(0.12, 0.12, 0.16), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.01, -0.12), Vec3::new(0.125, 0.125, 0.02), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.34), Vec3::new(0.075, 0.075, 0.32), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, -0.04, -0.32), Vec3::new(0.05, 0.07, 0.12), COL_WOOD_LIGHT);
        }
        WeaponType::Runestaff => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.15), Vec3::new(0.045, 0.045, 0.90), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.45), Vec3::new(0.055, 0.055, 0.04), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.20), Vec3::new(0.055, 0.055, 0.04), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.62), Vec3::new(0.08, 0.08, 0.08), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.72), Vec3::new(0.065, 0.065, 0.12), COL_RUNE_CYAN);
        }
        WeaponType::Halberd => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.20), Vec3::new(0.04, 0.04, 1.10), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.65), Vec3::new(0.055, 0.055, 0.24), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.11, 0.0, -0.72), Vec3::new(0.18, 0.02, 0.22), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.08, 0.0, -0.72), Vec3::new(0.12, 0.025, 0.06), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.75), Vec3::new(0.0, 0.0, -1.02), 0.035, 0.005, 0.020, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Longsword => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.035, 0.035, 0.16), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.18), Vec3::new(0.05, 0.05, 0.04), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.02), Vec3::new(0.24, 0.025, 0.035), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.04), Vec3::new(0.0, 0.0, -0.72), 0.06, 0.015, 0.018, 0.004, COL_IRON_BRIGHT, true);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.32), Vec3::new(0.012, 0.019, 0.50), COL_IRON_DARK);
        }
        WeaponType::Greatsword => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.15), Vec3::new(0.038, 0.038, 0.26), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.30), Vec3::new(0.06, 0.06, 0.05), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.36, 0.03, 0.04), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.09), Vec3::new(0.07, 0.02, 0.14), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.17), Vec3::new(0.16, 0.02, 0.03), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.18), Vec3::new(0.0, 0.0, -1.02), 0.08, 0.02, 0.022, 0.005, COL_IRON_BRIGHT, true);
        }
        WeaponType::Maul => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.15), Vec3::new(0.045, 0.045, 0.75), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.55), Vec3::new(0.14, 0.14, 0.22), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.67), Vec3::new(0.13, 0.13, 0.02), COL_IRON_BRIGHT);
        }
        WeaponType::Spear => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.15), Vec3::new(0.032, 0.032, 1.05), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.68), Vec3::new(0.045, 0.045, 0.08), COL_BRASS_GOLD);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.72), Vec3::new(0.0, 0.0, -0.98), 0.065, 0.008, 0.018, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Rapier => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.028, 0.028, 0.14), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.01), Vec3::new(0.14, 0.14, 0.10), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.04), Vec3::new(0.0, 0.0, -0.82), 0.025, 0.004, 0.025, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Warhammer => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.04, 0.04, 0.65), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.05, 0.0, -0.46), Vec3::new(0.09, 0.09, 0.12), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.06, 0.0, -0.46), Vec3::new(0.10, 0.03, 0.04), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.56), Vec3::new(0.025, 0.025, 0.12), COL_IRON_BRIGHT);
        }
        WeaponType::Club => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.05, 0.05, 0.35), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.28), Vec3::new(0.095, 0.095, 0.28), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.28), Vec3::new(0.11, 0.11, 0.04), COL_IRON_BRIGHT);
        }
        WeaponType::Dagger => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.06), Vec3::new(0.032, 0.032, 0.12), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.01), Vec3::new(0.10, 0.025, 0.025), COL_BRASS_GOLD);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.02), Vec3::new(0.0, 0.0, -0.32), 0.042, 0.008, 0.014, 0.003, COL_IRON_BRIGHT, true);
        }
        WeaponType::Handaxe => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.10), Vec3::new(0.036, 0.036, 0.48), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.32), Vec3::new(0.05, 0.05, 0.08), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.08, 0.0, -0.36), Vec3::new(0.14, 0.018, 0.16), COL_IRON_BRIGHT);
        }
        WeaponType::Cestus => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.02), Vec3::new(0.09, 0.09, 0.22), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.04, -0.14), Vec3::new(0.10, 0.04, 0.06), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.065, -0.14), Vec3::new(0.09, 0.02, 0.03), COL_IRON_BRIGHT);
        }
        WeaponType::KnuckleDuster => {
            builder.add_box_center_size(Vec3::new(0.0, -0.02, -0.02), Vec3::new(0.08, 0.03, 0.025), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.06), Vec3::new(0.11, 0.05, 0.04), COL_IRON_BRIGHT);
        }
        WeaponType::FryingPan => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.22), Vec3::new(0.24, 0.02, 0.24), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.34), Vec3::new(0.26, 0.05, 0.02), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.10), Vec3::new(0.26, 0.05, 0.02), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.12, 0.02, -0.22), Vec3::new(0.02, 0.05, 0.24), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.12, 0.02, -0.22), Vec3::new(0.02, 0.05, 0.24), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, 0.0), Vec3::new(0.035, 0.025, 0.18), COL_IRON_DARK);
        }
        WeaponType::HolyMackerel => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.16), Vec3::new(0.06, 0.12, 0.38), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, -0.05, -0.16), Vec3::new(0.055, 0.03, 0.36), COL_STRING_WHITE);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.02, 0.14, 0.10), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.05, 0.08, 0.12), COL_STRING_WHITE);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.30), Vec3::new(0.065, 0.025, 0.025), COL_BRASS_GOLD);
        }
        WeaponType::Hammer => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.06), Vec3::new(0.035, 0.035, 0.40), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.24), Vec3::new(0.07, 0.07, 0.13), COL_IRON_BRIGHT);
        }
        WeaponType::Pickaxe => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.10), Vec3::new(0.038, 0.038, 0.52), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.34), Vec3::new(0.28, 0.04, 0.06), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.34), Vec3::new(0.32, 0.025, 0.03), COL_IRON_BRIGHT);
        }
        WeaponType::Torch => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.08), Vec3::new(0.038, 0.038, 0.46), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.28), Vec3::new(0.065, 0.065, 0.12), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.04, -0.38), Vec3::new(0.09, 0.12, 0.09), COL_FIRE_ORANGE);
        }
        WeaponType::TigerClaws => {
            builder.add_box_center_size(Vec3::new(0.0, -0.01, -0.04), Vec3::new(0.10, 0.05, 0.12), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.03, 0.0), Vec3::new(0.08, 0.04, 0.08), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.16), Vec3::new(0.018, 0.03, 0.22), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.035, 0.015, -0.14), Vec3::new(0.018, 0.03, 0.19), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.035, 0.015, -0.14), Vec3::new(0.018, 0.03, 0.19), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.06), Vec3::new(0.09, 0.02, 0.03), COL_BRASS_GOLD);
        }
        WeaponType::BlackJack => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.06), Vec3::new(0.035, 0.035, 0.20), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.03, 0.18), Vec3::new(0.015, 0.04, 0.08), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.065, 0.065, 0.16), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.07, 0.07, 0.02), COL_BRASS_GOLD);
        }
        WeaponType::TwoHandAxe => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.04, 0.04, 0.95), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.15), Vec3::new(0.045, 0.045, 0.28), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.52), Vec3::new(0.06, 0.06, 0.14), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.14, 0.0, -0.52), Vec3::new(0.24, 0.02, 0.28), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.12, 0.0, -0.52), Vec3::new(0.20, 0.02, 0.24), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.58), Vec3::new(0.0, 0.0, -0.74), 0.025, 0.005, 0.025, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Trident => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.15), Vec3::new(0.036, 0.036, 1.15), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.66), Vec3::new(0.055, 0.055, 0.08), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.72), Vec3::new(0.26, 0.03, 0.03), COL_IRON_DARK);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.72), Vec3::new(0.0, 0.0, -1.04), 0.030, 0.005, 0.018, 0.004, COL_IRON_BRIGHT, true);
            builder.add_diamond_blade(Vec3::new(-0.11, 0.0, -0.72), Vec3::new(-0.11, 0.0, -1.00), 0.025, 0.005, 0.015, 0.004, COL_IRON_BRIGHT, true);
            builder.add_diamond_blade(Vec3::new(0.11, 0.0, -0.72), Vec3::new(0.11, 0.0, -1.00), 0.025, 0.005, 0.015, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Javelin => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.10), Vec3::new(0.028, 0.028, 1.05), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.035, 0.035, 0.18), COL_WOOD_DARK);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.60), Vec3::new(0.0, 0.0, -0.84), 0.055, 0.008, 0.018, 0.004, COL_IRON_BRIGHT, true);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.45), Vec3::new(0.032, 0.032, 0.08), COL_BRASS_GOLD);
        }
        WeaponType::Wand => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.06), Vec3::new(0.022, 0.022, 0.38), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.06), Vec3::new(0.032, 0.032, 0.12), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.24), Vec3::new(0.042, 0.042, 0.03), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.28), Vec3::new(0.036, 0.036, 0.08), COL_RUNE_CYAN);
        }
        WeaponType::Orb => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.18), Vec3::new(0.09, 0.09, 0.09), COL_RUNE_CYAN);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.18), Vec3::new(0.16, 0.015, 0.16), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.18), Vec3::new(0.015, 0.14, 0.14), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.08, 0.06, -0.18), Vec3::new(0.025, 0.025, 0.025), COL_FIRE_ORANGE);
        }
        WeaponType::WoodenShield => {
            // Compact combat buckler / targe: ~70% reduced screen obstruction
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.22, 0.24, 0.025), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.01), Vec3::new(0.24, 0.26, 0.015), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.025), Vec3::new(0.07, 0.07, 0.04), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.02), Vec3::new(0.09, 0.03, 0.02), COL_WOOD_DARK);
        }
        WeaponType::None => {
            // Authentic Clenched Brawler Fist (Unarmed Default for 1st person)
            builder.add_box_center_size(Vec3::new(0.0, -0.01, 0.07), Vec3::new(0.065, 0.065, 0.14), COL_SKIN_TONE);
            builder.add_box_center_size(Vec3::new(0.0, -0.01, 0.02), Vec3::new(0.072, 0.072, 0.06), COL_WRAP_CLOTH);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.04), Vec3::new(0.075, 0.065, 0.07), COL_SKIN_TONE);
            builder.add_box_center_size(Vec3::new(0.0, -0.018, -0.07), Vec3::new(0.072, 0.035, 0.038), COL_SKIN_TONE);
            builder.add_box_center_size(Vec3::new(0.035, 0.01, -0.045), Vec3::new(0.028, 0.045, 0.035), COL_SKIN_TONE);
        }
    }

    builder.build()
}

pub fn spawn_weapon_voxels(
    builder: &mut ChildBuilder,
    desired_weapon: WeaponType,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    render_layer: usize,
) {
    if desired_weapon == WeaponType::None && render_layer == 2 {
        // Third-person body model already has hands; no weapon attachment needed when unarmed.
        return;
    }

    let weapon_mesh = create_lowpoly_weapon_mesh(desired_weapon);
    let weapon_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.65,
        metallic: 0.35,
        cull_mode: None,
        ..default()
    });

    builder.spawn((
        PbrBundle {
            mesh: meshes.add(weapon_mesh),
            material: weapon_mat.clone(),
            ..default()
        },
        RenderLayers::layer(render_layer),
    ));

    // Dynamic animated accessories (only spawned on Viewmodel layer 1)
    if render_layer == 1 {
        match desired_weapon {
            WeaponType::Bow => {
                let arrow_mesh = create_lowpoly_arrow_mesh();
                builder.spawn((
                    PbrBundle {
                        mesh: meshes.add(arrow_mesh),
                        material: weapon_mat,
                        transform: BevyTransform::from_xyz(-0.01, 0.02, -0.12),
                        ..default()
                    },
                    ViewModelBowArrow,
                    RenderLayers::layer(1),
                ));
            }
            WeaponType::Crossbow => {
                let bolt_mesh = create_lowpoly_bolt_mesh();
                builder.spawn((
                    PbrBundle {
                        mesh: meshes.add(bolt_mesh),
                        material: weapon_mat,
                        transform: BevyTransform::from_xyz(0.0, 0.055, -0.16),
                        ..default()
                    },
                    ViewModelCrossbowBolt,
                    RenderLayers::layer(1),
                ));
            }
            WeaponType::HandCrossbow => {
                let bolt_mesh = create_lowpoly_bolt_mesh();
                builder.spawn((
                    PbrBundle {
                        mesh: meshes.add(bolt_mesh),
                        material: weapon_mat,
                        transform: BevyTransform::from_xyz(0.0, 0.035, -0.12),
                        ..default()
                    },
                    ViewModelCrossbowBolt,
                    RenderLayers::layer(1),
                ));
            }
            _ => {}
        }
    }
}

/// Spawns an immediate procedural blade slash trail / crescent arc in front of the camera.
pub fn spawn_directional_slash_trail(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    origin: Vec3,
    cam_forward: Vec3,
    cam_right: Vec3,
    cam_up: Vec3,
    direction: MeleeSwingDirection,
) {
    let mut builder = LowPolyMeshBuilder::new();
    let num_segments = 12;

    match direction {
        MeleeSwingDirection::Right => {
            // Arc sweeping from upper-left to lower-right
            for i in 0..num_segments {
                let t0 = i as f32 / num_segments as f32;
                let t1 = (i + 1) as f32 / num_segments as f32;
                let angle0 = -0.9 + t0 * 1.8;
                let angle1 = -0.9 + t1 * 1.8;

                let r_inner = 0.55;
                let r_outer0 = 1.05 + 0.15 * (1.0 - (t0 - 0.5).abs() * 2.0);
                let r_outer1 = 1.05 + 0.15 * (1.0 - (t1 - 0.5).abs() * 2.0);

                let v0 = cam_right * (angle0.sin() * r_inner) + cam_up * (angle0.cos() * r_inner);
                let v1 = cam_right * (angle0.sin() * r_outer0) + cam_up * (angle0.cos() * r_outer0);
                let v2 = cam_right * (angle1.sin() * r_outer1) + cam_up * (angle1.cos() * r_outer1);
                let v3 = cam_right * (angle1.sin() * r_inner) + cam_up * (angle1.cos() * r_inner);

                let color = [0.8, 0.95, 1.0, 1.0 - (t0 - 0.5).abs() * 0.5];
                builder.add_flat_quad(v0, v1, v2, v3, color);
            }
        }
        MeleeSwingDirection::Left => {
            // Arc sweeping from upper-right to lower-left
            for i in 0..num_segments {
                let t0 = i as f32 / num_segments as f32;
                let t1 = (i + 1) as f32 / num_segments as f32;
                let angle0 = 0.9 - t0 * 1.8;
                let angle1 = 0.9 - t1 * 1.8;

                let r_inner = 0.55;
                let r_outer0 = 1.05 + 0.15 * (1.0 - (t0 - 0.5).abs() * 2.0);
                let r_outer1 = 1.05 + 0.15 * (1.0 - (t1 - 0.5).abs() * 2.0);

                let v0 = cam_right * (angle0.sin() * r_inner) + cam_up * (angle0.cos() * r_inner);
                let v1 = cam_right * (angle0.sin() * r_outer0) + cam_up * (angle0.cos() * r_outer0);
                let v2 = cam_right * (angle1.sin() * r_outer1) + cam_up * (angle1.cos() * r_outer1);
                let v3 = cam_right * (angle1.sin() * r_inner) + cam_up * (angle1.cos() * r_inner);

                let color = [0.8, 0.95, 1.0, 1.0 - (t0 - 0.5).abs() * 0.5];
                builder.add_flat_quad(v0, v1, v2, v3, color);
            }
        }
        MeleeSwingDirection::Overhead => {
            // Vertical downward cleave arc
            for i in 0..num_segments {
                let t0 = i as f32 / num_segments as f32;
                let t1 = (i + 1) as f32 / num_segments as f32;
                let angle0 = 1.1 - t0 * 2.2;
                let angle1 = 1.1 - t1 * 2.2;

                let r_inner = 0.50;
                let r_outer = 1.15;

                let v0 = cam_forward * (angle0.cos() * r_inner) + cam_up * (angle0.sin() * r_inner);
                let v1 = cam_forward * (angle0.cos() * r_outer) + cam_up * (angle0.sin() * r_outer);
                let v2 = cam_forward * (angle1.cos() * r_outer) + cam_up * (angle1.sin() * r_outer);
                let v3 = cam_forward * (angle1.cos() * r_inner) + cam_up * (angle1.sin() * r_inner);

                let color = [1.0, 0.85, 0.35, 1.0];
                builder.add_flat_quad(v0, v1, v2, v3, color);
            }
        }
        MeleeSwingDirection::Thrust => {
            // Focused linear aerodynamic thrust cone
            for i in 0..num_segments {
                let angle0 = (i as f32) * (std::f32::consts::TAU / num_segments as f32);
                let angle1 = ((i + 1) as f32) * (std::f32::consts::TAU / num_segments as f32);

                let p_tip = cam_forward * 1.5;
                let p_base0 = cam_forward * 0.3 + cam_right * (angle0.cos() * 0.18) + cam_up * (angle0.sin() * 0.18);
                let p_base1 = cam_forward * 0.3 + cam_right * (angle1.cos() * 0.18) + cam_up * (angle1.sin() * 0.18);

                let color = [0.85, 0.95, 1.0, 0.8];
                builder.add_flat_triangle(p_base0, p_tip, p_base1, color);
            }
        }
    }

    let trail_mesh = meshes.add(builder.build());
    let trail_mat = materials.add(StandardMaterial {
        base_color: match direction {
            MeleeSwingDirection::Overhead => Color::srgb(1.0, 0.85, 0.35),
            MeleeSwingDirection::Thrust => Color::srgb(0.9, 0.96, 1.0),
            _ => Color::srgb(0.75, 0.92, 1.0),
        },
        emissive: match direction {
            MeleeSwingDirection::Overhead => LinearRgba::rgb(4.0, 2.5, 0.6),
            MeleeSwingDirection::Thrust => LinearRgba::rgb(3.0, 4.0, 5.0),
            _ => LinearRgba::rgb(2.5, 4.0, 5.5),
        },
        unlit: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    commands.spawn((
        PbrBundle {
            mesh: trail_mesh,
            material: trail_mat,
            transform: BevyTransform::from_translation(origin + cam_forward * 0.45),
            ..default()
        },
        Particle { timer: Timer::from_seconds(0.14, TimerMode::Once) },
    ));
}
