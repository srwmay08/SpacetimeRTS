// ============================================================================
// File: client/src/legacy_architectural_factions.rs
// ============================================================================
// ----------------------------------------------------------------------------
// ARCHIVED PROCEDURAL ARCHITECTURAL FACTIONS (HIGH ELF & DARK ELF)
// ----------------------------------------------------------------------------
// Historical Archive Note:
// Preserves early horizontal prototype meshes for High Elf (Pristine Bastion)
// and Dark Elf (Subterranean Spire) architectural styles.
//
// Deprecated in favor of a focused, high-polish vertical slice centered on
// the canonical Frontier Timber & Stone aesthetic. Retained here for future
// cultural expansion or modding reference without polluting active gameplay budgets.
// ----------------------------------------------------------------------------

#![allow(dead_code)]

use bevy::prelude::*;
use crate::building::{build_voxel_mesh, VoxelBox};

// ----------------------------------------------------------------------------
// 1. HIGH ELF (PRISTINE BASTION) PROCEDURAL MODELS
// ----------------------------------------------------------------------------

pub fn create_high_elf_wall_mesh() -> Mesh {
    let mint_marble = [0.68, 0.88, 0.78, 1.0];
    let mint_light = [0.78, 0.94, 0.84, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];
    let polished_wood = [0.45, 0.28, 0.16, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-1.75, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(1.75, -1.5, -0.24), max: Vec3::new(2.0, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-2.02, -1.5, -0.26), max: Vec3::new(-1.73, -1.35, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.73, -1.5, -0.26), max: Vec3::new(2.02, -1.35, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(-2.02, 1.35, -0.26), max: Vec3::new(-1.73, 1.50, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.73, 1.35, -0.26), max: Vec3::new(2.02, 1.50, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(-1.75, -1.5, -0.18), max: Vec3::new(1.75, 1.35, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(-1.80, 0.20, -0.21), max: Vec3::new(1.80, 0.30, 0.21), color: gold_filigree },
        VoxelBox { min: Vec3::new(-1.85, 1.35, -0.22), max: Vec3::new(1.85, 1.50, 0.22), color: lapis },
        VoxelBox { min: Vec3::new(-1.0, 1.45, -0.24), max: Vec3::new(1.0, 1.55, 0.24), color: gold_filigree },
        VoxelBox { min: Vec3::new(-0.35, -0.10, -0.20), max: Vec3::new(0.35, 0.60, 0.20), color: mint_light },
    ])
}

pub fn create_high_elf_wall_damaged_mesh() -> Mesh {
    let mint_marble = [0.65, 0.84, 0.75, 1.0];
    let mint_fissure = [0.55, 0.72, 0.64, 1.0];
    let gold_broken = [0.80, 0.68, 0.28, 1.0];
    let polished_wood = [0.42, 0.26, 0.15, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-1.75, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(1.75, -1.5, -0.24), max: Vec3::new(2.0, 1.15, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-1.75, -1.5, -0.18), max: Vec3::new(0.50, 1.35, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(0.50, -1.5, -0.18), max: Vec3::new(1.75, -0.20, 0.18), color: mint_fissure },
        VoxelBox { min: Vec3::new(0.70, 0.40, -0.16), max: Vec3::new(1.75, 1.30, 0.16), color: mint_marble },
        VoxelBox { min: Vec3::new(-1.80, 0.20, -0.21), max: Vec3::new(0.40, 0.30, 0.21), color: gold_broken },
        VoxelBox { min: Vec3::new(0.85, 0.20, -0.21), max: Vec3::new(1.80, 0.30, 0.21), color: gold_broken },
        VoxelBox { min: Vec3::new(-1.85, 1.35, -0.22), max: Vec3::new(0.60, 1.50, 0.22), color: lapis },
    ])
}

pub fn create_high_elf_window_mesh() -> Mesh {
    let mint_marble = [0.68, 0.88, 0.78, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];
    let polished_wood = [0.45, 0.28, 0.16, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];
    let glass_tint = [0.60, 0.88, 0.95, 0.75];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-0.95, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.24), max: Vec3::new(2.0, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.18), max: Vec3::new(0.95, -0.32, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(-1.0, -0.36, -0.22), max: Vec3::new(1.0, -0.24, 0.22), color: gold_filigree },
        VoxelBox { min: Vec3::new(-0.95, 0.85, -0.18), max: Vec3::new(0.95, 1.35, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.24), max: Vec3::new(2.0, 1.50, 0.24), color: lapis },
        VoxelBox { min: Vec3::new(-0.03, -0.24, -0.05), max: Vec3::new(0.03, 0.85, 0.05), color: gold_filigree },
        VoxelBox { min: Vec3::new(-0.95, 0.25, -0.05), max: Vec3::new(0.95, 0.31, 0.05), color: gold_filigree },
        VoxelBox { min: Vec3::new(-0.92, -0.22, -0.03), max: Vec3::new(0.92, 0.83, 0.03), color: glass_tint },
    ])
}

pub fn create_high_elf_window_damaged_mesh() -> Mesh {
    let mint_marble = [0.65, 0.84, 0.75, 1.0];
    let gold_broken = [0.80, 0.68, 0.28, 1.0];
    let polished_wood = [0.42, 0.26, 0.15, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-0.95, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.24), max: Vec3::new(2.0, 1.20, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.18), max: Vec3::new(0.30, -0.32, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(-0.95, 0.95, -0.18), max: Vec3::new(0.95, 1.35, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.24), max: Vec3::new(1.20, 1.50, 0.24), color: lapis },
        VoxelBox { min: Vec3::new(-0.95, 0.25, -0.05), max: Vec3::new(0.20, 0.31, 0.05), color: gold_broken },
    ])
}

pub fn create_high_elf_door_frame_mesh() -> Mesh {
    let polished_wood = [0.45, 0.28, 0.16, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-0.70, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.24), max: Vec3::new(2.0, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-0.76, -1.5, -0.26), max: Vec3::new(-0.68, 1.15, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(0.68, -1.5, -0.26), max: Vec3::new(0.76, 1.15, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.22), max: Vec3::new(0.70, 1.50, 0.22), color: lapis },
        VoxelBox { min: Vec3::new(-0.50, 1.30, -0.25), max: Vec3::new(0.50, 1.48, 0.25), color: gold_filigree },
    ])
}

pub fn create_high_elf_door_frame_damaged_mesh() -> Mesh {
    let polished_wood = [0.42, 0.26, 0.15, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];
    let gold_broken = [0.80, 0.68, 0.28, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-0.70, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.24), max: Vec3::new(2.0, 1.10, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.22), max: Vec3::new(0.30, 1.50, 0.22), color: lapis },
        VoxelBox { min: Vec3::new(-0.76, -1.5, -0.26), max: Vec3::new(-0.68, 0.80, 0.26), color: gold_broken },
    ])
}

pub fn create_high_elf_door_leaf_mesh() -> Mesh {
    let lapis = [0.14, 0.18, 0.36, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(0.04, -1.15, -0.06), max: Vec3::new(1.36, 1.15, 0.06), color: lapis },
        VoxelBox { min: Vec3::new(0.08, -1.10, -0.08), max: Vec3::new(1.32, -0.95, 0.08), color: gold_filigree },
        VoxelBox { min: Vec3::new(0.08, 0.95, -0.08), max: Vec3::new(1.32, 1.10, 0.08), color: gold_filigree },
        VoxelBox { min: Vec3::new(0.60, -0.30, -0.08), max: Vec3::new(0.80, 0.30, 0.08), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.15, -0.10, -0.12), max: Vec3::new(1.25, 0.10, 0.12), color: gold_filigree },
    ])
}

pub fn create_high_elf_roof_mesh() -> Mesh {
    let lapis = [0.12, 0.16, 0.32, 1.0];
    let lapis_light = [0.18, 0.24, 0.44, 1.0];
    let gold_ridge = [0.92, 0.78, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.1, -0.12, -2.1), max: Vec3::new(2.1, 0.12, 2.1), color: lapis },
        VoxelBox { min: Vec3::new(-1.8, 0.12, -1.8), max: Vec3::new(1.8, 0.25, 1.8), color: lapis_light },
        VoxelBox { min: Vec3::new(-2.15, 0.22, -0.15), max: Vec3::new(2.15, 0.35, 0.15), color: gold_ridge },
    ])
}

pub fn create_high_elf_foundation_mesh() -> Mesh {
    let mint_marble = [0.68, 0.88, 0.78, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -0.5, -2.0), max: Vec3::new(2.0, 0.5, 2.0), color: mint_marble },
        VoxelBox { min: Vec3::new(-2.05, 0.35, -2.05), max: Vec3::new(-1.70, 0.52, -1.70), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.70, 0.35, -2.05), max: Vec3::new(2.05, 0.52, -1.70), color: gold_filigree },
        VoxelBox { min: Vec3::new(-2.05, 0.35, 1.70), max: Vec3::new(-1.70, 0.52, 2.05), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.70, 0.35, 1.70), max: Vec3::new(2.05, 0.52, 2.05), color: gold_filigree },
    ])
}

// ----------------------------------------------------------------------------
// 2. DARK ELF (THE SUBTERRANEAN SPIRE) PROCEDURAL MODELS
// ----------------------------------------------------------------------------

pub fn create_dark_elf_wall_mesh() -> Mesh {
    let cavern_stone = [0.10, 0.08, 0.15, 1.0];
    let basalt_dark = [0.06, 0.05, 0.10, 1.0];
    let obsidian_purple = [0.22, 0.12, 0.28, 1.0];
    let neon_pink = [0.95, 0.15, 0.85, 1.0];
    let neon_cyan = [0.20, 0.80, 1.0, 1.0];
    let spiked_iron = [0.16, 0.15, 0.18, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-1.70, 1.35, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(1.70, -1.5, -0.26), max: Vec3::new(2.0, 1.35, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-1.95, 1.35, -0.22), max: Vec3::new(-1.75, 1.65, 0.22), color: spiked_iron },
        VoxelBox { min: Vec3::new(1.75, 1.35, -0.22), max: Vec3::new(1.95, 1.65, 0.22), color: spiked_iron },
        VoxelBox { min: Vec3::new(-1.70, -1.5, -0.20), max: Vec3::new(1.70, 1.35, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-1.75, -0.35, -0.22), max: Vec3::new(1.75, -0.22, 0.22), color: obsidian_purple },
        VoxelBox { min: Vec3::new(-1.75, 1.25, -0.22), max: Vec3::new(1.75, 1.38, 0.22), color: obsidian_purple },
        VoxelBox { min: Vec3::new(-1.60, 0.22, -0.22), max: Vec3::new(1.60, 0.28, 0.22), color: neon_pink },
        VoxelBox { min: Vec3::new(-0.80, 0.40, -0.22), max: Vec3::new(-0.65, 0.85, 0.22), color: neon_pink },
        VoxelBox { min: Vec3::new(0.65, 0.40, -0.22), max: Vec3::new(0.80, 0.85, 0.22), color: neon_pink },
        VoxelBox { min: Vec3::new(-0.25, 0.45, -0.23), max: Vec3::new(0.25, 0.75, 0.23), color: neon_cyan },
    ])
}

pub fn create_dark_elf_wall_damaged_mesh() -> Mesh {
    let cavern_stone = [0.08, 0.07, 0.12, 1.0];
    let basalt_dark = [0.05, 0.04, 0.08, 1.0];
    let neon_flicker = [0.55, 0.10, 0.50, 1.0];
    let spiked_iron = [0.14, 0.13, 0.16, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-1.70, 1.35, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(1.70, -1.5, -0.26), max: Vec3::new(2.0, 0.95, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-1.95, 1.35, -0.22), max: Vec3::new(-1.75, 1.65, 0.22), color: spiked_iron },
        VoxelBox { min: Vec3::new(-1.70, -1.5, -0.20), max: Vec3::new(0.40, 1.35, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.18), max: Vec3::new(1.70, -0.30, 0.18), color: cavern_stone },
        VoxelBox { min: Vec3::new(-1.60, 0.22, -0.22), max: Vec3::new(0.20, 0.28, 0.22), color: neon_flicker },
    ])
}

pub fn create_dark_elf_window_mesh() -> Mesh {
    let cavern_stone = [0.10, 0.08, 0.15, 1.0];
    let basalt_dark = [0.06, 0.05, 0.10, 1.0];
    let spiked_iron = [0.16, 0.15, 0.18, 1.0];
    let neon_pink = [0.95, 0.15, 0.85, 1.0];
    let glass_purple = [0.45, 0.15, 0.55, 0.70];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-0.95, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.26), max: Vec3::new(2.0, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.20), max: Vec3::new(0.95, -0.32, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-0.95, 0.85, -0.20), max: Vec3::new(0.95, 1.35, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.26), max: Vec3::new(2.0, 1.50, 0.26), color: spiked_iron },
        VoxelBox { min: Vec3::new(-0.04, -0.24, -0.06), max: Vec3::new(0.04, 0.85, 0.06), color: spiked_iron },
        VoxelBox { min: Vec3::new(-0.95, 0.24, -0.06), max: Vec3::new(0.95, 0.32, 0.06), color: spiked_iron },
        VoxelBox { min: Vec3::new(-0.85, -0.30, -0.22), max: Vec3::new(0.85, -0.24, 0.22), color: neon_pink },
        VoxelBox { min: Vec3::new(-0.92, -0.22, -0.03), max: Vec3::new(0.92, 0.83, 0.03), color: glass_purple },
    ])
}

pub fn create_dark_elf_window_damaged_mesh() -> Mesh {
    let cavern_stone = [0.08, 0.07, 0.12, 1.0];
    let basalt_dark = [0.05, 0.04, 0.08, 1.0];
    let spiked_iron = [0.14, 0.13, 0.16, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-0.95, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.26), max: Vec3::new(2.0, 1.15, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.20), max: Vec3::new(0.35, -0.32, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-0.95, 0.95, -0.20), max: Vec3::new(0.95, 1.35, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.26), max: Vec3::new(1.10, 1.50, 0.26), color: spiked_iron },
    ])
}

pub fn create_dark_elf_door_frame_mesh() -> Mesh {
    let basalt_dark = [0.06, 0.05, 0.10, 1.0];
    let spiked_iron = [0.16, 0.15, 0.18, 1.0];
    let neon_purple = [0.75, 0.12, 0.95, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-0.70, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.26), max: Vec3::new(2.0, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.24), max: Vec3::new(0.70, 1.50, 0.24), color: spiked_iron },
        VoxelBox { min: Vec3::new(-0.75, 1.15, -0.26), max: Vec3::new(0.75, 1.25, 0.26), color: neon_purple },
    ])
}

pub fn create_dark_elf_door_frame_damaged_mesh() -> Mesh {
    let basalt_dark = [0.05, 0.04, 0.08, 1.0];
    let spiked_iron = [0.14, 0.13, 0.16, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-0.70, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.26), max: Vec3::new(2.0, 1.0, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.24), max: Vec3::new(0.20, 1.50, 0.24), color: spiked_iron },
    ])
}

pub fn create_dark_elf_door_leaf_mesh() -> Mesh {
    let spiked_iron = [0.18, 0.17, 0.20, 1.0];
    let neon_pink = [0.95, 0.15, 0.85, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(0.04, -1.15, -0.07), max: Vec3::new(1.36, 1.15, 0.07), color: spiked_iron },
        VoxelBox { min: Vec3::new(0.55, -0.40, -0.09), max: Vec3::new(0.85, 0.40, 0.09), color: neon_pink },
        VoxelBox { min: Vec3::new(0.40, -0.10, -0.09), max: Vec3::new(1.00, 0.10, 0.09), color: neon_pink },
    ])
}

pub fn create_dark_elf_roof_mesh() -> Mesh {
    let basalt = [0.08, 0.06, 0.12, 1.0];
    let spiked_iron = [0.16, 0.15, 0.18, 1.0];
    let neon_purple = [0.75, 0.12, 0.95, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -0.10, -2.0), max: Vec3::new(2.0, 0.10, 2.0), color: basalt },
        VoxelBox { min: Vec3::new(-1.8, 0.10, -1.8), max: Vec3::new(1.8, 0.22, 1.8), color: basalt },
        VoxelBox { min: Vec3::new(-2.05, 0.10, -2.05), max: Vec3::new(-1.85, 0.45, -1.85), color: spiked_iron },
        VoxelBox { min: Vec3::new(1.85, 0.10, -2.05), max: Vec3::new(2.05, 0.45, -1.85), color: spiked_iron },
        VoxelBox { min: Vec3::new(-2.05, 0.10, 1.85), max: Vec3::new(-1.85, 0.45, 2.05), color: spiked_iron },
        VoxelBox { min: Vec3::new(1.85, 0.10, 1.85), max: Vec3::new(2.05, 0.45, 2.05), color: spiked_iron },
        VoxelBox { min: Vec3::new(-1.5, 0.22, -0.08), max: Vec3::new(1.5, 0.26, 0.08), color: neon_purple },
    ])
}

pub fn create_dark_elf_foundation_mesh() -> Mesh {
    let basalt = [0.08, 0.06, 0.12, 1.0];
    let neon_pink = [0.95, 0.15, 0.85, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -0.5, -2.0), max: Vec3::new(2.0, 0.5, 2.0), color: basalt },
        VoxelBox { min: Vec3::new(-2.02, 0.42, -2.02), max: Vec3::new(2.02, 0.50, -1.94), color: neon_pink },
        VoxelBox { min: Vec3::new(-2.02, 0.42, 1.94), max: Vec3::new(2.02, 0.50, 2.02), color: neon_pink },
    ])
}

// ----------------------------------------------------------------------------
// 3. ARCHIVED MULTI-RACE DESTRUCTION FX & RUIN PROFILES
// ----------------------------------------------------------------------------

#[allow(deprecated, dead_code)]
pub fn spawn_legacy_faction_destruction_fx(
    commands: &mut Commands,
    manifest: &crate::building::BuildingAssetManifest,
    _meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    origin: Vec3,
    faction: crate::components::BuildingFaction,
) {

    use crate::components::{BuildingFaction, VoxelGib, RuneLightDecay};


    let mut rng_seed = (origin.x.abs() * 1000.0 + origin.z.abs() * 100.0) as u64;
    let pool = &manifest.generic_rubble_pool;
    let faction_mat = manifest.get_material(false);

    match faction {
        BuildingFaction::HighElf => {
            let mat_lapis = materials.add(StandardMaterial {
                base_color: Color::srgb(0.16, 0.22, 0.45),
                perceptual_roughness: 0.3,
                metallic: 0.2,
                ..default()
            });
            let mat_sparkle = materials.add(StandardMaterial {
                base_color: Color::srgb(0.85, 1.0, 0.95),
                unlit: true,
                ..default()
            });

            for i in 0..22 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 40) as f32 / 10.0 + 1.2;

                let mat = if i % 2 == 0 { faction_mat.clone() } else { mat_lapis.clone() };
                let mesh = if i % 2 == 0 { pool.geometric_shard_a.clone() } else { pool.geometric_shard_b.clone() };
                let offset = Vec3::new(rx * 0.35, (i as f32 * 0.06).min(1.2), rz * 0.35);

                commands.spawn((
                    PbrBundle {
                        mesh,
                        material: mat,
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.4 + (i as f32 * 0.03), TimerMode::Once),
                        velocity: Vec3::new(rx * 6.5, ry * 1.5, rz * 6.5),
                        angular_velocity: Vec3::new(rx * 26.0, ry * 18.0, rz * 26.0),
                    },
                ));
            }

            for i in 0..8 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;

                commands.spawn((
                    PbrBundle {
                        mesh: pool.sparkle_shard.clone(),
                        material: mat_sparkle.clone(),
                        transform: Transform::from_translation(origin + Vec3::new(rx * 0.5, 1.0 + (i as f32 * 0.1), rz * 0.5)),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(0.9 + (i as f32 * 0.04), TimerMode::Once),
                        velocity: Vec3::new(rx * 3.5, 3.5, rz * 3.5),
                        angular_velocity: Vec3::new(rx * 12.0, 10.0, rz * 12.0),
                    },
                ));
            }
        }

        BuildingFaction::DarkElf => {
            let mat_neon_pink = materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.15, 0.85),
                unlit: true,
                ..default()
            });
            let mat_neon_cyan = materials.add(StandardMaterial {
                base_color: Color::srgb(0.15, 0.85, 0.95),
                unlit: true,
                ..default()
            });

            for i in 0..16 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 35) as f32 / 10.0 + 1.2;

                let mesh = if i % 2 == 0 { pool.stone_cube.clone() } else { pool.geometric_shard_a.clone() };
                let offset = Vec3::new(rx * 0.3, (i as f32 * 0.06).min(1.0), rz * 0.3);

                commands.spawn((
                    PbrBundle {
                        mesh,
                        material: faction_mat.clone(),
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.4 + (i as f32 * 0.03), TimerMode::Once),
                        velocity: Vec3::new(rx * 5.0, ry, rz * 5.0),
                        angular_velocity: Vec3::new(rx * 18.0, ry * 12.0, rz * 18.0),
                    },
                ));
            }

            for i in 0..14 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 45) as f32 / 10.0 + 1.8;

                let mat = if i % 2 == 0 { mat_neon_pink.clone() } else { mat_neon_cyan.clone() };
                let offset = Vec3::new(rx * 0.25, 0.5 + (i as f32 * 0.05), rz * 0.25);

                commands.spawn((
                    PbrBundle {
                        mesh: pool.sparkle_shard.clone(),
                        material: mat,
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.1 + (i as f32 * 0.04), TimerMode::Once),
                        velocity: Vec3::new(rx * 7.5, ry, rz * 7.5),
                        angular_velocity: Vec3::new(rx * 22.0, ry * 16.0, rz * 22.0),
                    },
                ));
            }

            commands.spawn((
                PointLightBundle {
                    point_light: PointLight {
                        color: Color::srgb(0.92, 0.20, 0.85),
                        intensity: 45000.0,
                        range: 16.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    transform: Transform::from_translation(origin + Vec3::Y * 1.5),
                    ..default()
                },
                RuneLightDecay {
                    timer: Timer::from_seconds(1.4, TimerMode::Once),
                    base_intensity: 45000.0,
                },
            ));
        }

        BuildingFaction::Human | BuildingFaction::Barbarian => {
            for i in 0..16 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 30) as f32 / 10.0 + 1.0;

                let mesh = if i % 2 == 0 { pool.stone_cube.clone() } else { pool.brick_block.clone() };
                let offset = Vec3::new(rx * 0.3, (i as f32 * 0.05).min(0.8), rz * 0.3);

                commands.spawn((
                    PbrBundle {
                        mesh,
                        material: faction_mat.clone(),
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.3 + (i as f32 * 0.03), TimerMode::Once),
                        velocity: Vec3::new(rx * 4.2, ry, rz * 4.2),
                        angular_velocity: Vec3::new(rx * 12.0, ry * 6.0, rz * 12.0),
                    },
                ));
            }
        }
    }
}

