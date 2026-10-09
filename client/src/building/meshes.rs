// ============================================================================
// File: client/src/building/meshes.rs
// ============================================================================
// Procedural low-poly meshes for walls, windows, doors, ramps, workstations, siege weapons, and rubble.

use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;

use super::types::{build_voxel_mesh, VoxelBox};

// ----------------------------------------------------------------------------
// PROCEDURAL MESH GENERATION (PRISTINE & DAMAGED VARIANTS)
// ----------------------------------------------------------------------------

pub fn create_ramp_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let positions = vec![
        [-2.0, -1.5,  2.0], [ 2.0, -1.5,  2.0], [ 2.0, -1.5, -2.0], [-2.0, -1.5, -2.0],
        [ 2.0, -1.5, -2.0], [-2.0, -1.5, -2.0], [-2.0,  1.5, -2.0], [ 2.0,  1.5, -2.0],
        [-2.0, -1.5,  2.0], [ 2.0, -1.5,  2.0], [ 2.0,  1.5, -2.0], [-2.0,  1.5, -2.0],
        [-2.0, -1.5,  2.0], [-2.0, -1.5, -2.0], [-2.0,  1.5, -2.0],
        [ 2.0, -1.5,  2.0], [ 2.0, -1.5, -2.0], [ 2.0,  1.5, -2.0],
    ];
    
    let normals = vec![
        [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0],
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0],
        [0.0, 0.8, 0.6], [0.0, 0.8, 0.6], [0.0, 0.8, 0.6], [0.0, 0.8, 0.6],
        [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0],
    ];
    
    let indices = vec![
        0, 2, 1,  0, 3, 2,
        4, 6, 5,  4, 7, 6,
        8, 9, 10,  8, 10, 11,
        12, 14, 13,
        15, 16, 17,
    ];

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

pub fn create_workbench_mesh() -> Mesh {
    let wood = [0.42, 0.28, 0.16, 1.0];
    let top_wood = [0.55, 0.38, 0.22, 1.0];
    let iron = [0.35, 0.35, 0.38, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.70, 0.0, -0.40), max: Vec3::new(-0.52, 0.75, -0.22), color: wood },
        VoxelBox { min: Vec3::new(0.52, 0.0, -0.40), max: Vec3::new(0.70, 0.75, -0.22), color: wood },
        VoxelBox { min: Vec3::new(-0.70, 0.0, 0.22), max: Vec3::new(-0.52, 0.75, 0.40), color: wood },
        VoxelBox { min: Vec3::new(0.52, 0.0, 0.22), max: Vec3::new(0.70, 0.75, 0.40), color: wood },
        VoxelBox { min: Vec3::new(-0.62, 0.15, -0.32), max: Vec3::new(0.62, 0.22, 0.32), color: wood },
        VoxelBox { min: Vec3::new(-0.80, 0.75, -0.50), max: Vec3::new(0.80, 0.95, 0.50), color: top_wood },
        VoxelBox { min: Vec3::new(-0.55, 0.95, -0.25), max: Vec3::new(-0.25, 1.18, 0.05), color: iron },
        VoxelBox { min: Vec3::new(0.35, 0.95, -0.35), max: Vec3::new(0.65, 1.10, -0.15), color: wood },
    ])
}

pub fn create_campfire_mesh() -> Mesh {
    let stone = [0.48, 0.48, 0.50, 1.0];
    let wood = [0.32, 0.18, 0.10, 1.0];
    let embers = [0.88, 0.32, 0.08, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.65, 0.0, -0.65), max: Vec3::new(0.65, 0.25, -0.42), color: stone },
        VoxelBox { min: Vec3::new(-0.65, 0.0, 0.42), max: Vec3::new(0.65, 0.25, 0.65), color: stone },
        VoxelBox { min: Vec3::new(-0.65, 0.0, -0.42), max: Vec3::new(-0.42, 0.25, 0.42), color: stone },
        VoxelBox { min: Vec3::new(0.42, 0.0, -0.42), max: Vec3::new(0.65, 0.25, 0.42), color: stone },
        VoxelBox { min: Vec3::new(-0.40, 0.0, -0.40), max: Vec3::new(0.40, 0.12, 0.40), color: embers },
        VoxelBox { min: Vec3::new(-0.45, 0.10, -0.12), max: Vec3::new(0.45, 0.24, 0.12), color: wood },
        VoxelBox { min: Vec3::new(-0.12, 0.20, -0.45), max: Vec3::new(0.12, 0.34, 0.45), color: wood },
    ])
}

#[allow(dead_code)]
pub fn create_catapult_mesh() -> Mesh {
    let wood_dark = [0.35, 0.22, 0.12, 1.0];
    let wood_light = [0.52, 0.35, 0.20, 1.0];
    let iron = [0.25, 0.26, 0.28, 1.0];
    let rope = [0.65, 0.52, 0.28, 1.0];
    let stone = [0.50, 0.48, 0.46, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.85, 0.20, -1.50), max: Vec3::new(-0.65, 0.45, 1.50), color: wood_dark },
        VoxelBox { min: Vec3::new(0.65, 0.20, -1.50), max: Vec3::new(0.85, 0.45, 1.50), color: wood_dark },
        VoxelBox { min: Vec3::new(-0.65, 0.22, -1.35), max: Vec3::new(0.65, 0.42, -1.15), color: wood_light },
        VoxelBox { min: Vec3::new(-0.65, 0.22, -0.30), max: Vec3::new(0.65, 0.42, -0.10), color: wood_light },
        VoxelBox { min: Vec3::new(-0.65, 0.22, 1.15), max: Vec3::new(0.65, 0.42, 1.35), color: wood_light },
        VoxelBox { min: Vec3::new(-1.05, 0.0, -1.25), max: Vec3::new(-0.88, 0.65, -0.85), color: iron },
        VoxelBox { min: Vec3::new(0.88, 0.0, -1.25), max: Vec3::new(1.05, 0.65, -0.85), color: iron },
        VoxelBox { min: Vec3::new(-1.05, 0.0, 0.85), max: Vec3::new(-0.88, 0.65, 1.25), color: iron },
        VoxelBox { min: Vec3::new(0.88, 0.0, 0.85), max: Vec3::new(1.05, 0.65, 1.25), color: iron },
        VoxelBox { min: Vec3::new(-1.12, 0.25, -1.10), max: Vec3::new(1.12, 0.40, -1.00), color: wood_dark },
        VoxelBox { min: Vec3::new(-1.12, 0.25, 1.00), max: Vec3::new(1.12, 0.40, 1.10), color: wood_dark },
        VoxelBox { min: Vec3::new(-0.82, 0.45, -0.35), max: Vec3::new(-0.65, 1.85, -0.15), color: wood_dark },
        VoxelBox { min: Vec3::new(0.65, 0.45, -0.35), max: Vec3::new(0.82, 1.85, -0.15), color: wood_dark },
        VoxelBox { min: Vec3::new(-0.80, 0.45, -0.95), max: Vec3::new(-0.68, 1.35, -0.35), color: wood_light },
        VoxelBox { min: Vec3::new(0.68, 0.45, -0.95), max: Vec3::new(0.80, 1.35, -0.35), color: wood_light },
        VoxelBox { min: Vec3::new(-0.65, 0.50, -0.32), max: Vec3::new(0.65, 0.80, -0.18), color: rope },
        VoxelBox { min: Vec3::new(-0.95, 0.52, -0.30), max: Vec3::new(-0.82, 0.78, -0.20), color: iron },
        VoxelBox { min: Vec3::new(0.82, 0.52, -0.30), max: Vec3::new(0.95, 0.78, -0.20), color: iron },
        VoxelBox { min: Vec3::new(-0.12, 0.55, -0.30), max: Vec3::new(0.12, 1.10, 1.35), color: wood_light },
        VoxelBox { min: Vec3::new(-0.25, 0.95, 1.20), max: Vec3::new(0.25, 1.25, 1.55), color: wood_dark },
        VoxelBox { min: Vec3::new(-0.18, 1.18, 1.25), max: Vec3::new(0.18, 1.50, 1.50), color: stone },
        VoxelBox { min: Vec3::new(-0.65, 1.60, -0.32), max: Vec3::new(0.65, 1.82, -0.18), color: iron },
    ])
}

#[allow(dead_code)]
pub fn create_trebuchet_mesh() -> Mesh {
    let timber = [0.38, 0.24, 0.14, 1.0];
    let timber_light = [0.55, 0.38, 0.22, 1.0];
    let iron = [0.26, 0.27, 0.28, 1.0];
    let stone = [0.48, 0.46, 0.45, 1.0];
    let rope = [0.70, 0.58, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-1.30, 0.0, -1.80), max: Vec3::new(-1.05, 0.28, 1.80), color: timber },
        VoxelBox { min: Vec3::new(1.05, 0.0, -1.80), max: Vec3::new(1.30, 0.28, 1.80), color: timber },
        VoxelBox { min: Vec3::new(-1.05, 0.0, -1.60), max: Vec3::new(1.05, 0.25, -1.35), color: timber },
        VoxelBox { min: Vec3::new(-1.05, 0.0, 1.35), max: Vec3::new(1.05, 0.25, 1.60), color: timber },
        VoxelBox { min: Vec3::new(-1.25, 0.25, -0.15), max: Vec3::new(-1.05, 3.20, 0.15), color: timber },
        VoxelBox { min: Vec3::new(-1.22, 0.25, -1.40), max: Vec3::new(-1.08, 3.10, -0.10), color: timber_light },
        VoxelBox { min: Vec3::new(-1.22, 0.25, 0.10), max: Vec3::new(-1.08, 3.10, 1.40), color: timber_light },
        VoxelBox { min: Vec3::new(1.05, 0.25, -0.15), max: Vec3::new(1.25, 3.20, 0.15), color: timber },
        VoxelBox { min: Vec3::new(1.08, 0.25, -1.40), max: Vec3::new(1.22, 3.10, -0.10), color: timber_light },
        VoxelBox { min: Vec3::new(1.08, 0.25, 0.10), max: Vec3::new(1.22, 3.10, 1.40), color: timber_light },
        VoxelBox { min: Vec3::new(-1.35, 3.00, -0.12), max: Vec3::new(1.35, 3.22, 0.12), color: iron },
        VoxelBox { min: Vec3::new(-0.16, 2.70, -1.20), max: Vec3::new(0.16, 3.35, 2.80), color: timber_light },
        VoxelBox { min: Vec3::new(-0.55, 1.60, -1.45), max: Vec3::new(0.55, 2.80, -0.95), color: timber },
        VoxelBox { min: Vec3::new(-0.48, 1.70, -1.38), max: Vec3::new(0.48, 2.75, -1.02), color: stone },
        VoxelBox { min: Vec3::new(-0.04, 0.60, 2.60), max: Vec3::new(0.04, 2.80, 2.68), color: rope },
        VoxelBox { min: Vec3::new(-0.25, 0.40, 2.50), max: Vec3::new(0.25, 0.85, 2.95), color: stone },
    ])
}

#[allow(dead_code)]
pub fn create_ballista_mesh() -> Mesh {
    let dark_wood = [0.36, 0.22, 0.12, 1.0];
    let light_wood = [0.54, 0.36, 0.20, 1.0];
    let iron = [0.28, 0.29, 0.30, 1.0];
    let bronze = [0.72, 0.58, 0.24, 1.0];
    let red_mat = [0.85, 0.15, 0.15, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.65, 0.0, -0.65), max: Vec3::new(-0.45, 0.85, -0.45), color: dark_wood },
        VoxelBox { min: Vec3::new(0.45, 0.0, -0.65), max: Vec3::new(0.65, 0.85, -0.45), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.12, 0.0, 0.55), max: Vec3::new(0.12, 0.85, 0.75), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.25, 0.80, -0.25), max: Vec3::new(0.25, 1.15, 0.25), color: iron },
        VoxelBox { min: Vec3::new(-0.15, 1.15, -0.15), max: Vec3::new(0.15, 1.35, 0.15), color: bronze },
        VoxelBox { min: Vec3::new(-0.16, 1.25, -1.40), max: Vec3::new(0.16, 1.45, 1.20), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.06, 1.45, -1.35), max: Vec3::new(0.06, 1.48, 1.15), color: iron },
        VoxelBox { min: Vec3::new(-0.95, 1.15, -1.45), max: Vec3::new(0.95, 1.55, -1.25), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.90, 1.00, -1.40), max: Vec3::new(-0.70, 1.70, -1.20), color: bronze },
        VoxelBox { min: Vec3::new(0.70, 1.00, -1.40), max: Vec3::new(0.90, 1.70, -1.20), color: bronze },
        VoxelBox { min: Vec3::new(-1.45, 1.28, -1.15), max: Vec3::new(-0.85, 1.42, -1.35), color: light_wood },
        VoxelBox { min: Vec3::new(0.85, 1.28, -1.15), max: Vec3::new(1.45, 1.42, -1.35), color: light_wood },
        VoxelBox { min: Vec3::new(-1.40, 1.32, -1.15), max: Vec3::new(1.40, 1.38, -0.20), color: iron },
        VoxelBox { min: Vec3::new(-0.35, 1.20, 0.95), max: Vec3::new(0.35, 1.45, 1.15), color: iron },
        VoxelBox { min: Vec3::new(-0.45, 1.15, 1.00), max: Vec3::new(-0.35, 1.65, 1.10), color: bronze },
        VoxelBox { min: Vec3::new(-0.03, 1.48, -1.25), max: Vec3::new(0.03, 1.54, 0.35), color: light_wood },
        VoxelBox { min: Vec3::new(-0.06, 1.47, -1.45), max: Vec3::new(0.06, 1.55, -1.25), color: iron },
        VoxelBox { min: Vec3::new(-0.08, 1.46, 0.15), max: Vec3::new(0.08, 1.56, 0.30), color: red_mat },
    ])
}

#[allow(dead_code)]
pub fn create_battering_ram_mesh() -> Mesh {
    let timber = [0.35, 0.22, 0.12, 1.0];
    let roof_shingle = [0.45, 0.28, 0.15, 1.0];
    let iron = [0.26, 0.27, 0.28, 1.0];
    let bronze = [0.75, 0.55, 0.22, 1.0];
    let log_wood = [0.48, 0.32, 0.18, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-1.15, 0.0, -1.45), max: Vec3::new(-0.95, 0.70, -0.95), color: iron },
        VoxelBox { min: Vec3::new(0.95, 0.0, -1.45), max: Vec3::new(1.15, 0.70, -0.95), color: iron },
        VoxelBox { min: Vec3::new(-1.15, 0.0, 0.95), max: Vec3::new(-0.95, 0.70, 1.45), color: iron },
        VoxelBox { min: Vec3::new(0.95, 0.0, 0.95), max: Vec3::new(1.15, 0.70, 1.45), color: iron },
        VoxelBox { min: Vec3::new(-0.95, 0.25, -1.70), max: Vec3::new(-0.75, 0.50, 1.70), color: timber },
        VoxelBox { min: Vec3::new(0.75, 0.25, -1.70), max: Vec3::new(0.95, 0.50, 1.70), color: timber },
        VoxelBox { min: Vec3::new(-0.92, 0.50, -1.55), max: Vec3::new(-0.75, 2.20, -1.35), color: timber },
        VoxelBox { min: Vec3::new(0.75, 0.50, -1.55), max: Vec3::new(0.92, 2.20, -1.35), color: timber },
        VoxelBox { min: Vec3::new(-0.92, 0.50, 1.35), max: Vec3::new(-0.75, 2.20, 1.55), color: timber },
        VoxelBox { min: Vec3::new(0.75, 0.50, 1.35), max: Vec3::new(0.92, 2.20, 1.55), color: timber },
        VoxelBox { min: Vec3::new(-0.85, 2.10, -1.75), max: Vec3::new(0.85, 2.25, 1.75), color: timber },
        VoxelBox { min: Vec3::new(-0.95, 2.10, -1.75), max: Vec3::new(0.0, 2.65, 1.75), color: roof_shingle },
        VoxelBox { min: Vec3::new(0.0, 2.10, -1.75), max: Vec3::new(0.95, 2.65, 1.75), color: roof_shingle },
        VoxelBox { min: Vec3::new(-0.06, 1.25, -0.90), max: Vec3::new(0.06, 2.15, -0.82), color: iron },
        VoxelBox { min: Vec3::new(-0.06, 1.25, 0.82), max: Vec3::new(0.06, 2.15, 0.90), color: iron },
        VoxelBox { min: Vec3::new(-0.22, 0.85, -1.95), max: Vec3::new(0.22, 1.30, 1.85), color: log_wood },
        VoxelBox { min: Vec3::new(-0.28, 0.80, -2.40), max: Vec3::new(0.28, 1.35, -1.95), color: bronze },
        VoxelBox { min: Vec3::new(-0.45, 1.00, -2.25), max: Vec3::new(-0.28, 1.45, -2.05), color: iron },
        VoxelBox { min: Vec3::new(0.28, 1.00, -2.25), max: Vec3::new(0.45, 1.45, -2.05), color: iron },
        VoxelBox { min: Vec3::new(-0.32, 1.00, -0.45), max: Vec3::new(0.32, 1.10, -0.35), color: iron },
        VoxelBox { min: Vec3::new(-0.32, 1.00, 0.35), max: Vec3::new(0.32, 1.10, 0.45), color: iron },
    ])
}

/// Pristine architectural wall model engineered for FPS crosshair alignment.
/// Embeds a distinctive high-contrast horizontal datum band / sightline trim at exactly y = +0.25m
/// (1.75m above floor level, corresponding to player eye-level in first-person perspective).
pub fn create_wall_mesh() -> Mesh {
    let timber_dark = [0.36, 0.24, 0.14, 1.0];
    let timber_light = [0.52, 0.38, 0.22, 1.0];
    let stone_body = [0.48, 0.46, 0.44, 1.0];
    let iron_trim = [0.25, 0.26, 0.28, 1.0];
    let datum_band = [0.78, 0.62, 0.28, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-1.75, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(1.75, -1.5, -0.22), max: Vec3::new(2.0, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.75, -1.5, -0.18), max: Vec3::new(1.75, -0.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-1.80, -0.35, -0.21), max: Vec3::new(1.80, -0.20, 0.21), color: timber_light },
        VoxelBox { min: Vec3::new(-1.75, -0.20, -0.18), max: Vec3::new(1.75, 0.20, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-1.80, 0.20, -0.21), max: Vec3::new(1.80, 0.30, 0.21), color: datum_band },
        VoxelBox { min: Vec3::new(-1.75, 0.30, -0.18), max: Vec3::new(1.75, 1.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(2.0, 1.50, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.85, 0.18, -0.22), max: Vec3::new(-1.70, 0.32, 0.22), color: iron_trim },
        VoxelBox { min: Vec3::new(1.70, 0.18, -0.22), max: Vec3::new(1.85, 0.32, 0.22), color: iron_trim },
    ])
}

/// Damaged wall model used for model swapping when structure health drops to <= 50%.
/// Displays exposed broken laths, missing masonry chunks, and jagged fractures.
pub fn create_damaged_wall_mesh() -> Mesh {
    let timber_dark = [0.32, 0.20, 0.10, 1.0];
    let timber_split = [0.45, 0.30, 0.16, 1.0];
    let stone_body = [0.42, 0.40, 0.38, 1.0];
    let stone_dark = [0.30, 0.28, 0.26, 1.0];
    let iron_trim = [0.20, 0.20, 0.22, 1.0];
    let datum_broken = [0.60, 0.48, 0.20, 1.0];

    build_voxel_mesh(&[
        // Left upright post (splintered top)
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-1.75, 1.10, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.95, 1.10, -0.12), max: Vec3::new(-1.80, 1.38, 0.12), color: timber_split },
        // Right upright post
        VoxelBox { min: Vec3::new(1.75, -1.5, -0.22), max: Vec3::new(2.0, 1.5, 0.22), color: timber_dark },
        // Lower masonry course with missing chunk on right
        VoxelBox { min: Vec3::new(-1.75, -1.5, -0.18), max: Vec3::new(0.60, -0.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(0.60, -1.5, -0.15), max: Vec3::new(1.75, -0.85, 0.15), color: stone_dark },
        // Broken tactical ledge
        VoxelBox { min: Vec3::new(-1.80, -0.35, -0.21), max: Vec3::new(0.40, -0.20, 0.21), color: timber_split },
        // Mid wall breach (hole in center-right)
        VoxelBox { min: Vec3::new(-1.75, -0.20, -0.18), max: Vec3::new(-0.25, 0.20, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(1.05, -0.20, -0.18), max: Vec3::new(1.75, 0.20, 0.18), color: stone_body },
        // Fractured datum band
        VoxelBox { min: Vec3::new(-1.80, 0.20, -0.21), max: Vec3::new(-0.30, 0.30, 0.21), color: datum_broken },
        VoxelBox { min: Vec3::new(1.10, 0.20, -0.21), max: Vec3::new(1.80, 0.30, 0.21), color: datum_broken },
        // Upper damaged masonry
        VoxelBox { min: Vec3::new(-1.75, 0.30, -0.18), max: Vec3::new(-0.10, 1.15, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(0.95, 0.30, -0.18), max: Vec3::new(1.75, 1.35, 0.18), color: stone_body },
        // Broken header beam
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(-0.80, 1.50, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.40, 1.35, -0.22), max: Vec3::new(2.0, 1.50, 0.22), color: timber_dark },
        // Chipped iron brace
        VoxelBox { min: Vec3::new(-1.85, 0.18, -0.22), max: Vec3::new(-1.70, 0.32, 0.22), color: iron_trim },
    ])
}

/// Window wall model: 4.0m wide x 3.0m high structural frame with a 1.9m wide x 1.15m high
/// central window opening at eye level (+0.25m datum). Embedded frosted glass pane allows
/// line-of-sight raycasts through Rapier CollisionGroups while blocking player collision.
pub fn create_window_mesh() -> Mesh {
    let timber_dark = [0.36, 0.24, 0.14, 1.0];
    let timber_light = [0.52, 0.38, 0.22, 1.0];
    let stone_body = [0.48, 0.46, 0.44, 1.0];
    let iron_trim = [0.25, 0.26, 0.28, 1.0];
    let datum_band = [0.78, 0.62, 0.28, 1.0];
    // Light translucent blue crystalline glass pane
    let glass_tint = [0.52, 0.78, 0.90, 0.70];

    build_voxel_mesh(&[
        // Left & Right Structural Posts (x = -2.0 to -0.95 and +0.95 to +2.0)
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-0.95, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.22), max: Vec3::new(2.0, 1.5, 0.22), color: timber_dark },
        // Lower Masonry Sill Course (y = -1.5 to -0.32)
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.18), max: Vec3::new(0.95, -0.32, 0.18), color: stone_body },
        // Tactical Crouch Ledge on Sill
        VoxelBox { min: Vec3::new(-1.0, -0.38, -0.24), max: Vec3::new(1.0, -0.24, 0.24), color: timber_light },
        // Upper Masonry Header Course (y = +0.85 to +1.35)
        VoxelBox { min: Vec3::new(-0.95, 0.85, -0.18), max: Vec3::new(0.95, 1.35, 0.18), color: stone_body },
        // Top Roof Header Beam (y = +1.35 to +1.50)
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(2.0, 1.50, 0.22), color: timber_dark },
        // Eye-Level Datum Line on side jambs
        VoxelBox { min: Vec3::new(-2.0, 0.20, -0.23), max: Vec3::new(-0.95, 0.30, 0.23), color: datum_band },
        VoxelBox { min: Vec3::new(0.95, 0.20, -0.23), max: Vec3::new(2.0, 0.30, 0.23), color: datum_band },
        // Iron Window Sill Brackets
        VoxelBox { min: Vec3::new(-0.98, -0.36, -0.22), max: Vec3::new(-0.90, -0.22, 0.22), color: iron_trim },
        VoxelBox { min: Vec3::new(0.90, -0.36, -0.22), max: Vec3::new(0.98, -0.22, 0.22), color: iron_trim },
        // Central Iron Window Mullions (vertical & horizontal crossbars)
        VoxelBox { min: Vec3::new(-0.04, -0.24, -0.06), max: Vec3::new(0.04, 0.85, 0.06), color: iron_trim },
        VoxelBox { min: Vec3::new(-0.95, 0.25, -0.06), max: Vec3::new(0.95, 0.33, 0.06), color: iron_trim },
        // Translucent Glass Pane (y = -0.24 to +0.85, x = -0.95 to +0.95)
        VoxelBox { min: Vec3::new(-0.92, -0.22, -0.03), max: Vec3::new(0.92, 0.83, 0.03), color: glass_tint },
    ])
}

/// Damaged window wall model with fractured masonry and shattered glass pane.
pub fn create_damaged_window_mesh() -> Mesh {
    let timber_dark = [0.32, 0.20, 0.10, 1.0];
    let stone_body = [0.42, 0.40, 0.38, 1.0];
    let iron_trim = [0.20, 0.20, 0.22, 1.0];
    let glass_tint = [0.52, 0.78, 0.90, 0.70];

    build_voxel_mesh(&[
        // Left & Right Structural Posts
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-0.95, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.22), max: Vec3::new(2.0, 1.25, 0.22), color: timber_dark },
        // Cracked Lower Sill
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.18), max: Vec3::new(0.35, -0.32, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(0.35, -1.5, -0.15), max: Vec3::new(0.95, -0.65, 0.15), color: stone_body },
        // Upper Header Course
        VoxelBox { min: Vec3::new(-0.95, 0.95, -0.18), max: Vec3::new(0.95, 1.35, 0.18), color: stone_body },
        // Top Roof Header Beam
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(1.10, 1.50, 0.22), color: timber_dark },
        // Bent Mullion
        VoxelBox { min: Vec3::new(-0.04, -0.24, -0.06), max: Vec3::new(0.04, 0.45, 0.06), color: iron_trim },
        // Shattered glass shards remaining in corners
        VoxelBox { min: Vec3::new(-0.92, -0.22, -0.03), max: Vec3::new(-0.45, 0.15, 0.03), color: glass_tint },
        VoxelBox { min: Vec3::new(0.55, 0.45, -0.03), max: Vec3::new(0.92, 0.83, 0.03), color: glass_tint },
    ])
}

/// Stationary doorframe model: 4.0m wide x 3.0m high wall with an open 1.4m wide x 2.4m high doorway.
pub fn create_door_frame_mesh() -> Mesh {
    let timber_dark = [0.36, 0.24, 0.14, 1.0];
    let timber_light = [0.52, 0.38, 0.22, 1.0];
    let stone_body = [0.48, 0.46, 0.44, 1.0];
    let iron_trim = [0.25, 0.26, 0.28, 1.0];
    let datum_band = [0.78, 0.62, 0.28, 1.0];

    build_voxel_mesh(&[
        // Left Structural Wing (x = -2.0 to -0.70)
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-1.65, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.65, -1.5, -0.18), max: Vec3::new(-0.70, 1.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-0.78, -1.5, -0.24), max: Vec3::new(-0.68, 1.00, 0.24), color: timber_dark },
        // Right Structural Wing (x = +0.70 to +2.0)
        VoxelBox { min: Vec3::new(1.65, -1.5, -0.22), max: Vec3::new(2.0, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.18), max: Vec3::new(1.65, 1.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(0.68, -1.5, -0.24), max: Vec3::new(0.78, 1.00, 0.24), color: timber_dark },
        // Door Threshold Plate on Ground
        VoxelBox { min: Vec3::new(-0.70, -1.5, -0.22), max: Vec3::new(0.70, -1.42, 0.22), color: iron_trim },
        // Top Lintel Header Beam (above door opening: y = 0.90 to 1.50)
        VoxelBox { min: Vec3::new(-0.78, 0.90, -0.24), max: Vec3::new(0.78, 1.05, 0.24), color: timber_light },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.18), max: Vec3::new(0.70, 1.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(2.0, 1.50, 0.22), color: timber_dark },
        // Eye-Level Datum Line on outer masonry wings
        VoxelBox { min: Vec3::new(-1.65, 0.20, -0.21), max: Vec3::new(-0.70, 0.30, 0.21), color: datum_band },
        VoxelBox { min: Vec3::new(0.70, 0.20, -0.21), max: Vec3::new(1.65, 0.30, 0.21), color: datum_band },
        // Heavy Cast-Iron Door Hinges on Left Post
        VoxelBox { min: Vec3::new(-0.76, -0.80, -0.10), max: Vec3::new(-0.66, -0.60, 0.10), color: iron_trim },
        VoxelBox { min: Vec3::new(-0.76, 0.40, -0.10), max: Vec3::new(-0.66, 0.60, 0.10), color: iron_trim },
    ])
}

/// Swinging door leaf model: 1.36m wide x 2.30m high reinforced timber door leaf.
/// Pivot axis is located at x = 0.0 relative to its hinge joint so rotating about Y swings it naturally.
pub fn create_door_leaf_mesh() -> Mesh {
    let timber = [0.42, 0.26, 0.14, 1.0];
    let timber_plank = [0.48, 0.32, 0.18, 1.0];
    let iron = [0.24, 0.25, 0.26, 1.0];

    // Pivot is at local x = 0.0; the door leaf extends from x = 0.0 to x = +1.36
    build_voxel_mesh(&[
        // Vertical Planks
        VoxelBox { min: Vec3::new(0.02, -1.15, -0.06), max: Vec3::new(0.44, 1.15, 0.06), color: timber },
        VoxelBox { min: Vec3::new(0.46, -1.15, -0.06), max: Vec3::new(0.90, 1.15, 0.06), color: timber_plank },
        VoxelBox { min: Vec3::new(0.92, -1.15, -0.06), max: Vec3::new(1.34, 1.15, 0.06), color: timber },
        // Horizontal Heavy Iron Bracing Straps
        VoxelBox { min: Vec3::new(0.0, -0.75, -0.08), max: Vec3::new(1.30, -0.65, 0.08), color: iron },
        VoxelBox { min: Vec3::new(0.0, 0.45, -0.08), max: Vec3::new(1.30, 0.55, 0.08), color: iron },
        // Diagonal Cross-Brace
        VoxelBox { min: Vec3::new(0.15, -0.65, -0.07), max: Vec3::new(1.15, 0.45, 0.07), color: timber_plank },
        // Iron Door Ring Handle & Latch (at x = 1.15m)
        VoxelBox { min: Vec3::new(1.10, -0.15, 0.06), max: Vec3::new(1.25, -0.05, 0.14), color: iron },
        VoxelBox { min: Vec3::new(1.10, -0.15, -0.14), max: Vec3::new(1.25, -0.05, -0.06), color: iron },
    ])
}

/// Damaged doorframe model for visual destruction state.
pub fn create_damaged_door_frame_mesh() -> Mesh {
    let timber_dark = [0.32, 0.20, 0.10, 1.0];
    let stone_body = [0.42, 0.40, 0.38, 1.0];
    let iron_trim = [0.20, 0.20, 0.22, 1.0];

    build_voxel_mesh(&[
        // Left Wing
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-1.65, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.65, -1.5, -0.18), max: Vec3::new(-0.70, 0.90, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-0.78, -1.5, -0.24), max: Vec3::new(-0.68, 1.00, 0.24), color: timber_dark },
        // Right Wing
        VoxelBox { min: Vec3::new(1.65, -1.5, -0.22), max: Vec3::new(2.0, 1.25, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.18), max: Vec3::new(1.65, 1.35, 0.18), color: stone_body },
        // Threshold
        VoxelBox { min: Vec3::new(-0.70, -1.5, -0.22), max: Vec3::new(0.70, -1.42, 0.22), color: iron_trim },
        // Cracked Top Header
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(0.20, 1.50, 0.22), color: timber_dark },
    ])
}

/// Procedural low-poly rubble pile (2.0m x 1.0m x 2.0m) used for permanent collapse ruins.
pub fn create_rubble_mesh() -> Mesh {
    let stone = [0.48, 0.46, 0.44, 1.0];
    let stone_dark = [0.36, 0.35, 0.34, 1.0];
    let timber = [0.38, 0.24, 0.14, 1.0];

    build_voxel_mesh(&[
        // Base masonry block mound
        VoxelBox { min: Vec3::new(-1.40, 0.0, -1.40), max: Vec3::new(1.40, 0.45, 1.40), color: stone_dark },
        VoxelBox { min: Vec3::new(-1.00, 0.35, -0.90), max: Vec3::new(0.90, 0.85, 0.95), color: stone },
        VoxelBox { min: Vec3::new(-0.60, 0.70, -0.50), max: Vec3::new(0.50, 1.15, 0.60), color: stone },
        // Scattered fallen timber beams
        VoxelBox { min: Vec3::new(-1.30, 0.25, -0.30), max: Vec3::new(1.20, 0.42, -0.12), color: timber },
        VoxelBox { min: Vec3::new(-0.25, 0.65, -1.10), max: Vec3::new(0.45, 0.82, 1.20), color: timber },
        // Broken stone caps
        VoxelBox { min: Vec3::new(-1.25, 0.10, 0.65), max: Vec3::new(-0.75, 0.55, 1.15), color: stone },
        VoxelBox { min: Vec3::new(0.65, 0.10, -1.25), max: Vec3::new(1.15, 0.52, -0.75), color: stone },
    ])
}
