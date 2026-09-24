use bevy::prelude::{Transform as BevyTransform, *};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::RenderLayers;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::pbr::{
    FogFalloff, 
    FogSettings, 
    ScreenSpaceAmbientOcclusionQualityLevel, 
    ScreenSpaceAmbientOcclusionSettings,
};
use avian3d::prelude::*;
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

use spacetimedb_sdk::{DbContext, Table}; 

use crate::module_bindings::{self, *};
use crate::module_bindings::peasant_table::PeasantTableAccess; 
use crate::module_bindings::npc_brain_table::NpcBrainTableAccess; 
use crate::module_bindings::pet_component_table::PetComponentTableAccess; 
use crate::module_bindings::resource_node_table::ResourceNodeTableAccess;
use crate::core::*;
use crate::components::*;

const DB_NAME: &str = "hybrid-backend";

#[derive(Resource)] 
pub struct SpacetimeConnection {
    pub db: module_bindings::DbConnection, 
    pub identity: Option<spacetimedb_sdk::Identity>,
}

#[derive(Resource)] 
pub struct IdentityStore(pub Arc<Mutex<Option<spacetimedb_sdk::Identity>>>);

// ----------------------------------------------------------------------------
// PROCEDURAL COMPOSITE VOXEL MESH BUILDER
// ----------------------------------------------------------------------------

pub struct VoxelBox {
    pub min: Vec3,
    pub max: Vec3,
    pub color: [f32; 4],
}

pub fn build_voxel_mesh(boxes: &[VoxelBox]) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(boxes.len() * 24);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(boxes.len() * 24);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(boxes.len() * 24);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(boxes.len() * 24);
    let mut indices: Vec<u32> = Vec::with_capacity(boxes.len() * 36);

    for b in boxes {
        let min = b.min;
        let max = b.max;
        let c = b.color;

        // Top Face (+Y)
        let s = positions.len() as u32;
        positions.push([min.x, max.y, max.z]);
        positions.push([max.x, max.y, max.z]);
        positions.push([max.x, max.y, min.z]);
        positions.push([min.x, max.y, min.z]);
        for _ in 0..4 { normals.push([0.0, 1.0, 0.0]); colors.push(c); }
        uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // Bottom Face (-Y)
        let s = positions.len() as u32;
        positions.push([min.x, min.y, min.z]);
        positions.push([max.x, min.y, min.z]);
        positions.push([max.x, min.y, max.z]);
        positions.push([min.x, min.y, max.z]);
        for _ in 0..4 { normals.push([0.0, -1.0, 0.0]); colors.push(c); }
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // East Face (+X)
        let s = positions.len() as u32;
        positions.push([max.x, min.y, max.z]);
        positions.push([max.x, min.y, min.z]);
        positions.push([max.x, max.y, min.z]);
        positions.push([max.x, max.y, max.z]);
        for _ in 0..4 { normals.push([1.0, 0.0, 0.0]); colors.push(c); }
        uvs.extend_from_slice(&[[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // West Face (-X)
        let s = positions.len() as u32;
        positions.push([min.x, min.y, min.z]);
        positions.push([min.x, min.y, max.z]);
        positions.push([min.x, max.y, max.z]);
        positions.push([min.x, max.y, min.z]);
        for _ in 0..4 { normals.push([-1.0, 0.0, 0.0]); colors.push(c); }
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // South Face (+Z)
        let s = positions.len() as u32;
        positions.push([min.x, min.y, max.z]);
        positions.push([max.x, min.y, max.z]);
        positions.push([max.x, max.y, max.z]);
        positions.push([min.x, max.y, max.z]);
        for _ in 0..4 { normals.push([0.0, 0.0, 1.0]); colors.push(c); }
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // North Face (-Z)
        let s = positions.len() as u32;
        positions.push([max.x, min.y, min.z]);
        positions.push([min.x, min.y, min.z]);
        positions.push([min.x, max.y, min.z]);
        positions.push([max.x, max.y, min.z]);
        for _ in 0..4 { normals.push([0.0, 0.0, -1.0]); colors.push(c); }
        uvs.extend_from_slice(&[[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

// ----------------------------------------------------------------------------
// HIGH-FIDELITY 0.25m VOXEL RESOURCE MODELS
// ----------------------------------------------------------------------------

pub fn create_voxel_tree_mesh() -> Mesh {
    let bark_dark = [0.26, 0.16, 0.08, 1.0];
    let bark_mid = [0.36, 0.24, 0.14, 1.0];
    let needle_dark = [0.12, 0.38, 0.12, 1.0];
    let needle_mid = [0.18, 0.52, 0.18, 1.0];
    let needle_light = [0.26, 0.65, 0.26, 1.0];

    build_voxel_mesh(&[
        // Fluted Root Buttresses
        VoxelBox { min: Vec3::new(-0.625, 0.0, -0.25), max: Vec3::new(-0.25, 0.5, 0.25), color: bark_dark },
        VoxelBox { min: Vec3::new(0.25, 0.0, -0.25), max: Vec3::new(0.625, 0.5, 0.25), color: bark_dark },
        VoxelBox { min: Vec3::new(-0.25, 0.0, -0.625), max: Vec3::new(0.25, 0.5, -0.25), color: bark_dark },
        VoxelBox { min: Vec3::new(-0.25, 0.0, 0.25), max: Vec3::new(0.25, 0.5, 0.625), color: bark_dark },
        // Lower Trunk Core
        VoxelBox { min: Vec3::new(-0.30, 0.0, -0.30), max: Vec3::new(0.30, 2.5, 0.30), color: bark_mid },
        // Mid Trunk Core
        VoxelBox { min: Vec3::new(-0.25, 2.5, -0.25), max: Vec3::new(0.25, 5.0, 0.25), color: bark_mid },
        // Upper Trunk Core
        VoxelBox { min: Vec3::new(-0.15, 5.0, -0.15), max: Vec3::new(0.15, 7.25, 0.15), color: bark_dark },

        // Foliage Tier 1
        VoxelBox { min: Vec3::new(-1.875, 3.25, -1.375), max: Vec3::new(1.875, 4.0, 1.375), color: needle_dark },
        VoxelBox { min: Vec3::new(-1.375, 3.25, -1.875), max: Vec3::new(1.375, 4.0, 1.875), color: needle_dark },
        VoxelBox { min: Vec3::new(-1.625, 4.0, -1.625), max: Vec3::new(1.625, 4.75, 1.625), color: needle_mid },

        // Foliage Tier 2
        VoxelBox { min: Vec3::new(-1.50, 4.75, -1.0), max: Vec3::new(1.50, 5.375, 1.0), color: needle_dark },
        VoxelBox { min: Vec3::new(-1.0, 4.75, -1.50), max: Vec3::new(1.0, 5.375, 1.50), color: needle_dark },
        VoxelBox { min: Vec3::new(-1.25, 5.375, -1.25), max: Vec3::new(1.25, 6.0, 1.25), color: needle_mid },

        // Foliage Tier 3
        VoxelBox { min: Vec3::new(-1.125, 6.0, -0.75), max: Vec3::new(1.125, 6.625, 0.75), color: needle_mid },
        VoxelBox { min: Vec3::new(-0.75, 6.0, -1.125), max: Vec3::new(0.75, 6.625, 1.125), color: needle_mid },
        VoxelBox { min: Vec3::new(-0.875, 6.625, -0.875), max: Vec3::new(0.875, 7.25, 0.875), color: needle_light },

        // Foliage Tier 4
        VoxelBox { min: Vec3::new(-0.625, 7.25, -0.625), max: Vec3::new(0.625, 8.0, 0.625), color: needle_mid },
        VoxelBox { min: Vec3::new(-0.50, 8.0, -0.50), max: Vec3::new(0.50, 8.5, 0.50), color: needle_light },

        // Spire Tip
        VoxelBox { min: Vec3::new(-0.25, 8.5, -0.25), max: Vec3::new(0.25, 9.0, 0.25), color: needle_light },
        VoxelBox { min: Vec3::new(-0.125, 9.0, -0.125), max: Vec3::new(0.125, 9.35, 0.125), color: needle_light },
    ])
}

pub fn create_voxel_rock_mesh() -> Mesh {
    let slate = [0.35, 0.35, 0.38, 1.0];
    let granite_mid = [0.50, 0.50, 0.53, 1.0];
    let granite_light = [0.65, 0.65, 0.68, 1.0];
    let highlight = [0.75, 0.75, 0.78, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.875, 0.0, -0.75), max: Vec3::new(0.875, 0.35, 0.75), color: slate },
        VoxelBox { min: Vec3::new(-0.65, 0.35, -0.60), max: Vec3::new(0.65, 0.85, 0.60), color: granite_mid },
        VoxelBox { min: Vec3::new(-0.45, 0.85, -0.40), max: Vec3::new(0.40, 1.25, 0.40), color: granite_light },
        VoxelBox { min: Vec3::new(-0.20, 1.25, -0.20), max: Vec3::new(0.20, 1.50, 0.20), color: highlight },
        VoxelBox { min: Vec3::new(0.55, 0.0, -0.35), max: Vec3::new(1.05, 0.60, 0.45), color: slate },
        VoxelBox { min: Vec3::new(-1.05, 0.0, -0.15), max: Vec3::new(-0.60, 0.50, 0.55), color: granite_mid },
    ])
}

pub fn create_voxel_bush_mesh() -> Mesh {
    let green_dark = [0.14, 0.44, 0.14, 1.0];
    let green_mid = [0.20, 0.56, 0.20, 1.0];
    let green_bright = [0.26, 0.66, 0.26, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.625, 0.0, -0.625), max: Vec3::new(0.625, 0.65, 0.625), color: green_dark },
        VoxelBox { min: Vec3::new(-0.45, 0.65, -0.45), max: Vec3::new(0.45, 1.0, 0.45), color: green_bright },
        VoxelBox { min: Vec3::new(0.50, 0.15, -0.40), max: Vec3::new(0.875, 0.60, 0.40), color: green_mid },
        VoxelBox { min: Vec3::new(-0.875, 0.15, -0.40), max: Vec3::new(-0.50, 0.60, 0.40), color: green_mid },
        VoxelBox { min: Vec3::new(-0.40, 0.15, -0.875), max: Vec3::new(0.40, 0.60, -0.50), color: green_dark },
        VoxelBox { min: Vec3::new(-0.40, 0.15, 0.50), max: Vec3::new(0.40, 0.60, 0.875), color: green_bright },
    ])
}

pub fn create_voxel_branch_mesh() -> Mesh {
    let bark_dark = [0.28, 0.16, 0.08, 1.0];
    let bark_mid = [0.42, 0.26, 0.14, 1.0];
    let sapwood = [0.65, 0.48, 0.28, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.75, 0.04, -0.11), max: Vec3::new(0.75, 0.26, 0.11), color: bark_mid },
        VoxelBox { min: Vec3::new(-0.78, 0.04, -0.13), max: Vec3::new(-0.70, 0.28, 0.13), color: sapwood },
        VoxelBox { min: Vec3::new(-0.25, 0.04, 0.11), max: Vec3::new(-0.05, 0.22, 0.35), color: bark_dark },
        VoxelBox { min: Vec3::new(-0.05, 0.04, 0.35), max: Vec3::new(0.18, 0.20, 0.58), color: bark_mid },
        VoxelBox { min: Vec3::new(0.15, 0.04, -0.32), max: Vec3::new(0.35, 0.20, -0.11), color: bark_dark },
        VoxelBox { min: Vec3::new(0.35, 0.04, -0.52), max: Vec3::new(0.58, 0.18, -0.32), color: bark_mid },
        VoxelBox { min: Vec3::new(0.72, 0.06, -0.06), max: Vec3::new(0.82, 0.22, 0.06), color: sapwood },
        VoxelBox { min: Vec3::new(0.16, 0.06, 0.55), max: Vec3::new(0.24, 0.18, 0.65), color: sapwood },
    ])
}

pub fn create_voxel_flint_mesh() -> Mesh {
    let obsidian = [0.06, 0.07, 0.10, 1.0];
    let chert_body = [0.18, 0.32, 0.50, 1.0];
    let edge_cyan = [0.35, 0.75, 0.95, 1.0];
    let highlight = [0.75, 0.92, 1.0, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.25, 0.02, -0.22), max: Vec3::new(0.25, 0.18, 0.22), color: obsidian },
        VoxelBox { min: Vec3::new(-0.20, 0.18, -0.16), max: Vec3::new(0.20, 0.42, 0.16), color: chert_body },
        VoxelBox { min: Vec3::new(-0.12, 0.42, -0.10), max: Vec3::new(0.12, 0.65, 0.10), color: edge_cyan },
        VoxelBox { min: Vec3::new(-0.05, 0.65, -0.05), max: Vec3::new(0.05, 0.76, 0.05), color: highlight },
        VoxelBox { min: Vec3::new(-0.30, 0.10, -0.06), max: Vec3::new(-0.18, 0.35, 0.14), color: edge_cyan },
        VoxelBox { min: Vec3::new(0.18, 0.10, -0.14), max: Vec3::new(0.30, 0.35, 0.06), color: edge_cyan },
    ])
}

pub fn create_voxel_stone_mesh() -> Mesh {
    let granite_dark = [0.34, 0.33, 0.32, 1.0];
    let granite_mid = [0.52, 0.50, 0.48, 1.0];
    let granite_light = [0.72, 0.70, 0.66, 1.0];
    let mineral_white = [0.92, 0.90, 0.86, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.45, 0.02, -0.38), max: Vec3::new(0.45, 0.36, 0.38), color: granite_mid },
        VoxelBox { min: Vec3::new(-0.30, 0.36, -0.25), max: Vec3::new(0.28, 0.52, 0.25), color: granite_light },
        VoxelBox { min: Vec3::new(-0.12, 0.52, -0.12), max: Vec3::new(0.12, 0.60, 0.12), color: mineral_white },
        VoxelBox { min: Vec3::new(0.35, 0.02, -0.20), max: Vec3::new(0.68, 0.28, 0.28), color: granite_dark },
        VoxelBox { min: Vec3::new(-0.65, 0.02, 0.08), max: Vec3::new(-0.35, 0.26, 0.42), color: granite_light },
        VoxelBox { min: Vec3::new(-0.18, 0.02, -0.50), max: Vec3::new(0.28, 0.22, -0.30), color: granite_dark },
    ])
}

pub fn create_voxel_boar_mesh() -> Mesh {
    let hide = [0.26, 0.20, 0.16, 1.0];
    let mane = [0.18, 0.14, 0.10, 1.0];
    let snout = [0.55, 0.35, 0.32, 1.0];
    let tusk = [0.92, 0.90, 0.82, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.38, 0.25, -0.65), max: Vec3::new(0.38, 0.85, 0.55), color: hide },
        VoxelBox { min: Vec3::new(-0.10, 0.85, -0.55), max: Vec3::new(0.10, 1.02, 0.45), color: mane },
        VoxelBox { min: Vec3::new(-0.28, 0.32, 0.45), max: Vec3::new(0.28, 0.78, 0.95), color: hide },
        VoxelBox { min: Vec3::new(-0.16, 0.35, 0.95), max: Vec3::new(0.16, 0.58, 1.12), color: snout },
        VoxelBox { min: Vec3::new(-0.24, 0.45, 0.88), max: Vec3::new(-0.18, 0.68, 0.96), color: tusk },
        VoxelBox { min: Vec3::new(0.18, 0.45, 0.88), max: Vec3::new(0.24, 0.68, 0.96), color: tusk },
        VoxelBox { min: Vec3::new(-0.34, 0.0, 0.22), max: Vec3::new(-0.20, 0.25, 0.42), color: hide },
        VoxelBox { min: Vec3::new(0.20, 0.0, 0.22), max: Vec3::new(0.34, 0.25, 0.42), color: hide },
        VoxelBox { min: Vec3::new(-0.34, 0.0, -0.52), max: Vec3::new(-0.20, 0.25, -0.32), color: hide },
        VoxelBox { min: Vec3::new(0.20, 0.0, -0.52), max: Vec3::new(0.34, 0.25, -0.32), color: hide },
        VoxelBox { min: Vec3::new(-0.04, 0.55, -0.76), max: Vec3::new(0.04, 0.70, -0.65), color: mane },
    ])
}

pub fn create_voxel_deer_mesh() -> Mesh {
    let coat = [0.65, 0.42, 0.24, 1.0];
    let belly = [0.85, 0.75, 0.62, 1.0];
    let antler = [0.80, 0.75, 0.65, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.25, 0.55, -0.55), max: Vec3::new(0.25, 1.15, 0.55), color: coat },
        VoxelBox { min: Vec3::new(-0.20, 0.50, -0.45), max: Vec3::new(0.20, 0.65, 0.45), color: belly },
        VoxelBox { min: Vec3::new(-0.16, 0.95, 0.35), max: Vec3::new(0.16, 1.60, 0.70), color: coat },
        VoxelBox { min: Vec3::new(-0.15, 1.45, 0.55), max: Vec3::new(0.15, 1.80, 0.98), color: coat },
        VoxelBox { min: Vec3::new(-0.10, 1.45, 0.98), max: Vec3::new(0.10, 1.65, 1.18), color: belly },
        VoxelBox { min: Vec3::new(-0.25, 1.75, 0.55), max: Vec3::new(-0.15, 2.00, 0.70), color: coat },
        VoxelBox { min: Vec3::new(0.15, 1.75, 0.55), max: Vec3::new(0.25, 2.00, 0.70), color: coat },
        VoxelBox { min: Vec3::new(-0.18, 1.80, 0.55), max: Vec3::new(-0.12, 2.30, 0.62), color: antler },
        VoxelBox { min: Vec3::new(-0.28, 2.10, 0.55), max: Vec3::new(-0.16, 2.20, 0.72), color: antler },
        VoxelBox { min: Vec3::new(0.12, 1.80, 0.55), max: Vec3::new(0.18, 2.30, 0.62), color: antler },
        VoxelBox { min: Vec3::new(0.16, 2.10, 0.55), max: Vec3::new(0.28, 2.20, 0.72), color: antler },
        VoxelBox { min: Vec3::new(-0.22, 0.0, 0.30), max: Vec3::new(-0.12, 0.55, 0.44), color: coat },
        VoxelBox { min: Vec3::new(0.12, 0.0, 0.30), max: Vec3::new(0.22, 0.55, 0.44), color: coat },
        VoxelBox { min: Vec3::new(-0.22, 0.0, -0.48), max: Vec3::new(-0.12, 0.55, -0.34), color: coat },
        VoxelBox { min: Vec3::new(0.12, 0.0, -0.48), max: Vec3::new(0.22, 0.55, -0.34), color: coat },
        VoxelBox { min: Vec3::new(-0.06, 0.95, -0.65), max: Vec3::new(0.06, 1.15, -0.55), color: belly },
    ])
}

pub fn create_voxel_goblin_mesh() -> Mesh {
    let skin = [0.28, 0.68, 0.25, 1.0];
    let tunic = [0.42, 0.30, 0.18, 1.0];
    let eyes = [0.95, 0.85, 0.15, 1.0];
    let belt = [0.22, 0.16, 0.10, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.25, 0.35, -0.18), max: Vec3::new(0.25, 0.85, 0.18), color: tunic },
        VoxelBox { min: Vec3::new(-0.26, 0.45, -0.19), max: Vec3::new(0.26, 0.55, 0.19), color: belt },
        VoxelBox { min: Vec3::new(-0.22, 0.85, -0.16), max: Vec3::new(0.22, 1.25, 0.20), color: skin },
        VoxelBox { min: Vec3::new(-0.06, 0.92, 0.20), max: Vec3::new(0.06, 1.08, 0.35), color: skin },
        VoxelBox { min: Vec3::new(-0.45, 1.00, -0.06), max: Vec3::new(-0.22, 1.18, 0.08), color: skin },
        VoxelBox { min: Vec3::new(0.22, 1.00, -0.06), max: Vec3::new(0.45, 1.18, 0.08), color: skin },
        VoxelBox { min: Vec3::new(-0.16, 1.05, 0.19), max: Vec3::new(-0.08, 1.15, 0.21), color: eyes },
        VoxelBox { min: Vec3::new(0.08, 1.05, 0.19), max: Vec3::new(0.16, 1.15, 0.21), color: eyes },
        VoxelBox { min: Vec3::new(-0.38, 0.32, -0.08), max: Vec3::new(-0.25, 0.82, 0.08), color: skin },
        VoxelBox { min: Vec3::new(0.25, 0.32, -0.08), max: Vec3::new(0.38, 0.82, 0.08), color: skin },
        VoxelBox { min: Vec3::new(-0.22, 0.0, -0.12), max: Vec3::new(-0.06, 0.35, 0.12), color: tunic },
        VoxelBox { min: Vec3::new(0.06, 0.0, -0.12), max: Vec3::new(0.22, 0.35, 0.12), color: tunic },
    ])
}

pub fn create_voxel_peasant_mesh() -> Mesh {
    let skin = [0.86, 0.72, 0.60, 1.0];
    let shirt = [0.22, 0.42, 0.85, 1.0];
    let pants = [0.32, 0.26, 0.20, 1.0];
    let hair = [0.28, 0.18, 0.10, 1.0];
    let boots = [0.18, 0.12, 0.08, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.28, 0.65, -0.18), max: Vec3::new(0.28, 1.25, 0.18), color: shirt },
        VoxelBox { min: Vec3::new(-0.20, 1.25, -0.18), max: Vec3::new(0.20, 1.65, 0.18), color: skin },
        VoxelBox { min: Vec3::new(-0.22, 1.55, -0.20), max: Vec3::new(0.22, 1.72, 0.20), color: hair },
        VoxelBox { min: Vec3::new(-0.42, 0.60, -0.10), max: Vec3::new(-0.28, 1.22, 0.10), color: shirt },
        VoxelBox { min: Vec3::new(0.28, 0.60, -0.10), max: Vec3::new(0.42, 1.22, 0.10), color: shirt },
        VoxelBox { min: Vec3::new(-0.24, 0.15, -0.14), max: Vec3::new(-0.04, 0.65, 0.14), color: pants },
        VoxelBox { min: Vec3::new(0.04, 0.15, -0.14), max: Vec3::new(0.24, 0.65, 0.14), color: pants },
        VoxelBox { min: Vec3::new(-0.25, 0.0, -0.15), max: Vec3::new(-0.03, 0.15, 0.18), color: boots },
        VoxelBox { min: Vec3::new(0.03, 0.0, -0.15), max: Vec3::new(0.25, 0.15, 0.18), color: boots },
    ])
}

pub fn create_voxel_pet_mesh() -> Mesh {
    let coat = [0.86, 0.52, 0.18, 1.0];
    let cream = [0.95, 0.90, 0.80, 1.0];
    let nose = [0.10, 0.10, 0.10, 1.0];
    let ears = [0.68, 0.38, 0.12, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.22, 0.22, -0.38), max: Vec3::new(0.22, 0.55, 0.38), color: coat },
        VoxelBox { min: Vec3::new(-0.16, 0.20, -0.30), max: Vec3::new(0.16, 0.36, 0.35), color: cream },
        VoxelBox { min: Vec3::new(-0.18, 0.45, 0.22), max: Vec3::new(0.18, 0.72, 0.44), color: coat },
        VoxelBox { min: Vec3::new(-0.18, 0.60, 0.30), max: Vec3::new(0.18, 0.92, 0.62), color: coat },
        VoxelBox { min: Vec3::new(-0.10, 0.60, 0.62), max: Vec3::new(0.10, 0.76, 0.82), color: cream },
        VoxelBox { min: Vec3::new(-0.05, 0.70, 0.80), max: Vec3::new(0.05, 0.78, 0.85), color: nose },
        VoxelBox { min: Vec3::new(-0.24, 0.68, 0.32), max: Vec3::new(-0.16, 0.90, 0.52), color: ears },
        VoxelBox { min: Vec3::new(0.16, 0.68, 0.32), max: Vec3::new(0.24, 0.90, 0.52), color: ears },
        VoxelBox { min: Vec3::new(-0.20, 0.0, 0.20), max: Vec3::new(-0.10, 0.25, 0.32), color: coat },
        VoxelBox { min: Vec3::new(0.10, 0.0, 0.20), max: Vec3::new(0.20, 0.25, 0.32), color: coat },
        VoxelBox { min: Vec3::new(-0.20, 0.0, -0.32), max: Vec3::new(-0.10, 0.25, -0.20), color: coat },
        VoxelBox { min: Vec3::new(0.10, 0.0, -0.32), max: Vec3::new(0.20, 0.25, -0.20), color: coat },
        VoxelBox { min: Vec3::new(-0.06, 0.45, -0.48), max: Vec3::new(0.06, 0.72, -0.36), color: coat },
    ])
}

// ----------------------------------------------------------------------------
// NETWORK CONNECTION SYSTEM
// ----------------------------------------------------------------------------

// Architectural Note: Lightened, Non-Dense Atmospheric Fog Tuning.
// Uses a clean, luminous sky palette (0.75, 0.84, 0.92) with a wide falloff band
// (start: 35.0m, end: 65.0m). This preserves crystal-clear foreground visibility
// without dark fog walls, while smoothly blending the terrain perimeter into the sky.
pub fn init_network_connection(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let uri = std::env::var("SPACETIMEDB_URI").unwrap_or_else(|_| "http://localhost:3000".to_string());
    info!("Initializing SpacetimeDB connection to URI: {}", uri);

    let try_connect = |token: Option<String>| {
        let mut builder = module_bindings::DbConnection::builder()
            .with_uri(uri.as_str())
            .with_database_name(DB_NAME);

        if let Some(t) = token {
            builder = builder.with_token(Some(t));
        }

        let identity_store = Arc::new(Mutex::new(None));
        let store_clone = Arc::clone(&identity_store);

        let build_result = builder.on_connect(move |conn, identity, token| {
            if let Err(e) = std::fs::write("stdb_token.txt", token.to_string()) {
                warn!("Failed to persist SpacetimeDB token to disk: {}", e);
            }
            info!("Authenticated to SpacetimeDB with Identity: {}", identity.to_hex());
            
            let _handle = conn.subscription_builder().subscribe(vec![
                "SELECT * FROM player".to_string(),
                "SELECT * FROM transform".to_string(),
                "SELECT * FROM inventory".to_string(),
                "SELECT * FROM resource_node".to_string(),
                "SELECT * FROM combat_event".to_string(),
                "SELECT * FROM structure".to_string(),
                "SELECT * FROM peasant".to_string(), 
                "SELECT * FROM npc_brain".to_string(),
                "SELECT * FROM pet_component".to_string(),
                "SELECT * FROM faction_component".to_string(),
                "SELECT * FROM health".to_string(),
                "SELECT * FROM harvestable_corpse".to_string(),
                "SELECT * FROM player_perspective".to_string(),
                "SELECT * FROM voxel_chunk".to_string(),
            ]);

            if let Ok(mut guard) = store_clone.lock() {
                *guard = Some(identity.clone());
            }
        }).build();

        (build_result, identity_store)
    };

    let token = std::fs::read_to_string("stdb_token.txt").ok();
    let (mut build_result, mut identity_store) = try_connect(token.clone());

    if let Err(ref e) = build_result {
        if token.is_some() {
            warn!("Connection rejected (Error: {}). Deleting potentially stale stdb_token.txt and retrying...", e);
            let _ = std::fs::remove_file("stdb_token.txt");
            let retry = try_connect(None);
            build_result = retry.0;
            identity_store = retry.1;
        }
    }

    let db = match build_result {
        Ok(db) => db,
        Err(e) => {
            error!("FATAL: Could not connect to SpacetimeDB: {}", e);
            std::process::exit(1); 
        }
    };

    commands.insert_resource(SpacetimeConnection { db, identity: None });
    commands.insert_resource(IdentityStore(identity_store));

    let sky_fog_color = Color::srgb(0.75, 0.84, 0.92);

    commands.spawn((
        SpatialBundle::from_transform(BevyTransform::from_xyz(0.0, 0.0, 0.0)),
        RtsCameraRig,
    )).with_children(|rig| {
        rig.spawn((
            Camera3dBundle {
                transform: BevyTransform::from_xyz(0.0, 40.0, 25.0).looking_at(Vec3::ZERO, Vec3::Y),
                camera: Camera { is_active: false, ..default() },
                ..default()
            },
            FogSettings {
                color: sky_fog_color,
                falloff: FogFalloff::Linear {
                    start: 55.0,
                    end: 110.0,
                },
                ..default()
            },
            RenderLayers::from_layers(&[0, 2]),
            RtsCameraChild,
        ));
    });

    let mut player_entity_commands = commands.spawn((
        SpatialBundle::from_transform(BevyTransform::from_xyz(0.0, 25.0, 0.0)),
        PlayerBody,
        RigidBody::Dynamic, 
        Collider::capsule(0.4, 1.2),
        SweptCcd::default(),
        CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Terrain, GameLayer::Unit, GameLayer::Environment]),
        LockedAxes::ROTATION_LOCKED,
        GravityScale(0.0),
        LinearVelocity::ZERO,
        ExternalForce::default().with_persistence(false),
        Kcc { is_grounded: false },
        LogicalPosition(Vec3::new(0.0, 25.0, 0.0)),
        LogicalRotation(Quat::IDENTITY),
        crate::components::Faction::Player, 
        Selectable, 
    ));

    player_entity_commands.insert((
        crate::prediction::InputBuffer::default(),
        crate::prediction::AuthoritativeState::default(),
        crate::prediction::LocalMovementTracker { last_position: Vec3::new(0.0, 25.0, 0.0) },
        Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
    ));

    player_entity_commands.with_children(|parent| {
        parent.spawn((
            PbrBundle {
                mesh: meshes.add(create_voxel_peasant_mesh()),
                material: materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.85, ..default() }),
                ..default()
            },
            RenderLayers::layer(2), 
            RTSProxy,
        ));

        parent.spawn((
            PbrBundle {
                mesh: meshes.add(bevy::math::primitives::Torus::new(0.6, 0.05)),
                material: materials.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() }),
                transform: BevyTransform::from_xyz(0.0, -0.9, 0.0), 
                visibility: Visibility::Hidden,
                ..default()
            },
            RenderLayers::layer(2),
            SelectionRing,
        ));

        parent.spawn((
            Camera3dBundle { 
                transform: BevyTransform::from_xyz(0.0, 0.5, 0.0), 
                camera: Camera { is_active: true, ..default() },
                ..default() 
            }, 
            PlayerHead, FpsCamera,
            FogSettings {
                color: sky_fog_color,
                falloff: FogFalloff::Linear {
                    start: 35.0,
                    end: 65.0,
                },
                ..default()
            },
            RenderLayers::from_layers(&[0, 1]),
            DepthPrepass, NormalPrepass,
            ScreenSpaceAmbientOcclusionSettings { quality_level: ScreenSpaceAmbientOcclusionQualityLevel::High },
        )).with_children(|cam| {
            cam.spawn((
                PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.12, 0.12, 0.45)),
                    material: materials.add(StandardMaterial {
                        base_color: Color::srgb(0.86, 0.72, 0.60), perceptual_roughness: 0.9, ..default()
                    }),
                    transform: BevyTransform::from_xyz(0.3, -0.3, -0.5).with_rotation(Quat::from_rotation_x(1.0)),
                    ..default()
                },
                RenderLayers::layer(1),
                ViewModelArm, FPSMesh,
            ));
        });
    });

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight { illuminance: 8_000.0, shadows_enabled: true, ..default() },
        transform: BevyTransform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

pub fn wait_for_connection(
    store: Res<IdentityStore>,
    mut next_state: ResMut<NextState<GameState>>,
    mut connection: ResMut<SpacetimeConnection>,
    mut player_query: Query<(
        &mut BevyTransform, 
        &mut LinearVelocity, 
        &mut GravityScale, 
        &mut crate::prediction::LocalMovementTracker,
        &mut crate::prediction::InputBuffer,
    ), With<PlayerBody>>,
) {
    let _ = connection.db.frame_tick();

    if let Ok(guard) = store.0.lock() {
        if let Some(id) = guard.as_ref() {
            if connection.identity.is_none() {
                connection.identity = Some(id.clone());
                next_state.set(GameState::InGame);
                info!("Bootstrapping complete. Entering In-Game State.");
                
                let spawn_y = crate::terrain::get_terrain_height(0.0, 0.0) + 1.5;
                
                if let Ok((mut transform, mut velocity, mut gravity, mut tracker, mut buffer)) = player_query.get_single_mut() {
                    transform.translation = Vec3::new(0.0, spawn_y, 0.0);
                    velocity.x = 0.0; velocity.y = 0.0; velocity.z = 0.0;
                    gravity.0 = 8.0; 
                    tracker.last_position = transform.translation;
                    buffer.queue.clear();
                }
            }
        }
    }
}

pub fn update_spatial_subscriptions(
    player_query: Query<&LogicalPosition, With<PlayerBody>>,
    mut culling_state: ResMut<NetworkCullingState>,
) {
    if let Ok(pos) = player_query.get_single() {
        let current_x = (pos.0.x / 50.0).floor() as i32;
        let current_z = (pos.0.z / 50.0).floor() as i32;

        if culling_state.current_chunk.0 != current_x || culling_state.current_chunk.1 != current_z {
            let center_x = (culling_state.current_chunk.0 as f32 * 50.0) + 25.0;
            let center_z = (culling_state.current_chunk.1 as f32 * 50.0) + 25.0;
            
            if (pos.0.x - center_x).abs() > 27.0 || (pos.0.z - center_z).abs() > 27.0 {
                culling_state.current_chunk = (current_x, current_z);
                culling_state.needs_rebuild = true;
            }
        }
    }

    if culling_state.needs_rebuild {
        culling_state.needs_rebuild = false;
    }
}

pub fn sync_logical_components(
    time: Res<Time>,
    mut query: Query<(&LogicalPosition, &LogicalRotation, &mut BevyTransform), (Without<PlayerBody>, With<NetworkEntity>)>
) {
    let decay_factor = 1.0 - (-15.0_f32 * time.delta_seconds()).exp(); 
    
    for (log_pos, log_rot, mut transform) in query.iter_mut() {
        if transform.translation.distance(log_pos.0) > 5.0 { 
            transform.translation = log_pos.0; 
        } else { 
            transform.translation = transform.translation.lerp(log_pos.0, decay_factor); 
        }
        transform.rotation = transform.rotation.slerp(log_rot.0, decay_factor);
    }
}

// Architectural Note: Zero-Allocation Creature Synchronization Loop.
// Reuses persistent collections to eliminate per-frame heap churn.
// Dynamic creatures are bounded strictly within 40m load / 44m unload so they never
// populate the distant horizon line or draw silhouettes through the lightened fog.
pub fn sync_transforms(
    mut commands: Commands, 
    conn: Res<SpacetimeConnection>, 
    mut query: Query<(Entity, &NetworkEntity, &mut LogicalPosition, &mut LogicalRotation)>,
    peasant_query: Query<&PeasantUnit>,
    mut player_query: Query<&mut crate::prediction::AuthoritativeState, With<PlayerBody>>,
    player_body_query: Query<&BevyTransform, With<PlayerBody>>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned_ids: Local<std::collections::HashSet<u64>>,
) {
    let _ = conn.db.frame_tick();
    
    let my_entity_id = conn.identity.as_ref()
        .and_then(|id| conn.db.db.player().identity().find(id))
        .map(|p| p.entity_id);

    let player_pos = player_body_query.get_single().map(|t| t.translation).unwrap_or(Vec3::ZERO);

    const CREATURE_LOAD_RADIUS_SQ: f32 = 40.0 * 40.0;
    const CREATURE_UNLOAD_RADIUS_SQ: f32 = 44.0 * 44.0;

    spawned_ids.clear();

    // 1. Update existing local entities or despawn those exiting view bounds
    for (entity, net_entity, mut log_pos, _) in query.iter_mut() {
        if Some(net_entity.0) == my_entity_id {
            spawned_ids.insert(net_entity.0);
            continue;
        }

        if let Some(db_t) = conn.db.db.transform().entity_id().find(&net_entity.0) {
            let dist_sq = (db_t.x - player_pos.x).powi(2) + (db_t.z - player_pos.z).powi(2);
            if dist_sq > CREATURE_UNLOAD_RADIUS_SQ {
                commands.entity(entity).despawn_recursive();
                continue;
            }

            log_pos.0 = Vec3::new(db_t.x, db_t.y, db_t.z);
            spawned_ids.insert(net_entity.0);
        } else {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        if peasant_query.get(entity).is_err() && conn.db.db.peasant().entity_id().find(&net_entity.0).is_some() {
            commands.entity(entity).insert(PeasantUnit { entity_id: net_entity.0 });
        }
    }

    // 2. Stream new creatures only when within visible perimeter
    for db_t in conn.db.db.transform().iter() {
        let id = db_t.entity_id;
        if Some(id) == my_entity_id { 
            if let Ok(mut auth_state) = player_query.get_single_mut() {
                if auth_state.last_processed_tick != db_t.last_processed_tick {
                    auth_state.position = Vec3::new(db_t.x, db_t.y, db_t.z);
                    auth_state.last_processed_tick = db_t.last_processed_tick;
                }
            }
            continue; 
        }

        let dist_sq = (db_t.x - player_pos.x).powi(2) + (db_t.z - player_pos.z).powi(2);
        if dist_sq > CREATURE_LOAD_RADIUS_SQ || spawned_ids.contains(&id) {
            continue;
        }
        
        let is_peasant = conn.db.db.peasant().entity_id().find(&id).is_some();
        let is_pet = conn.db.db.pet_component().entity_id().find(&id).is_some();
        let npc_brain = conn.db.db.npc_brain().entity_id().find(&id);
        
        let mut visual_transform = BevyTransform::default();
        let (mesh_handle, root_collider) = if is_pet {
            (meshes.add(create_voxel_pet_mesh()), Collider::cuboid(0.5, 0.8, 0.9))
        } else if let Some(brain) = npc_brain {
            let m = match brain.ai_type {
                crate::module_bindings::AiType::Boar => {
                    (meshes.add(create_voxel_boar_mesh()), Collider::cuboid(0.8, 0.8, 1.4))
                }
                crate::module_bindings::AiType::Deer => {
                    (meshes.add(create_voxel_deer_mesh()), Collider::cuboid(0.6, 1.8, 1.2))
                }
                crate::module_bindings::AiType::Goblin => {
                    (meshes.add(create_voxel_goblin_mesh()), Collider::capsule(0.4, 1.3))
                }
                crate::module_bindings::AiType::Friendly | crate::module_bindings::AiType::Peasant => {
                    (meshes.add(create_voxel_peasant_mesh()), Collider::capsule(0.4, 1.8))
                }
            };

            if brain.state == crate::module_bindings::BrainState::Corpse {
                visual_transform.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
            }
            m
        } else if is_peasant {
            (meshes.add(create_voxel_peasant_mesh()), Collider::capsule(0.4, 1.8))
        } else {
            (meshes.add(create_voxel_peasant_mesh()), Collider::capsule(0.4, 1.8))
        };

        let mut entity_cmds = commands.spawn((
            NetworkEntity(id),
            SpatialBundle::from_transform(BevyTransform::from_xyz(db_t.x, db_t.y, db_t.z)),
            LogicalPosition(Vec3::new(db_t.x, db_t.y, db_t.z)),
            LogicalRotation(Quat::IDENTITY),
            Selectable, 
            RigidBody::Kinematic, 
            root_collider,
            CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Terrain, GameLayer::Environment]),
        ));

        if is_peasant {
            entity_cmds.insert(PeasantUnit { entity_id: id });
        }

        entity_cmds.with_children(|parent| {
            parent.spawn((
                PbrBundle {
                    mesh: mesh_handle,
                    material: materials.add(StandardMaterial {
                        base_color: Color::WHITE,
                        perceptual_roughness: 0.85,
                        ..default()
                    }),
                    transform: visual_transform,
                    ..default()
                },
                RenderLayers::from_layers(&[0, 1, 2]), RTSProxy,
            ));
            parent.spawn((
                PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Torus::new(0.6, 0.05)),
                    material: materials.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() }),
                    transform: BevyTransform::from_xyz(0.0, -0.4, 0.0), visibility: Visibility::Hidden, ..default()
                },
                RenderLayers::layer(2), SelectionRing,
            ));
        });
        spawned_ids.insert(id);
    }
}

// Architectural Note: Spatial Distance Culling for Resource Nodes.
// Nodes load within 42m and unload beyond 46m, keeping total scene entities bounded
// under ~150 and eliminating heap memory stalls.
pub fn sync_resource_nodes(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    node_query: Query<(Entity, &ResourceNodeItem, &BevyTransform)>, 
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    conn: Res<SpacetimeConnection>,
    mut default_node_mat: Local<Option<Handle<StandardMaterial>>>,
    mut local_nodes: Local<std::collections::HashSet<u64>>,
) {
    let Ok(player_transform) = player_query.get_single() else { return; };
    let player_pos = player_transform.translation;

    let node_mat = default_node_mat.get_or_insert_with(|| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.1,
            ..default()
        })
    }).clone();

    const NODE_LOAD_RADIUS_SQ: f32 = 42.0 * 42.0;
    const NODE_UNLOAD_RADIUS_SQ: f32 = 46.0 * 46.0;

    local_nodes.clear();

    // 1. Process active scene nodes
    for (entity, node_item, transform) in node_query.iter() {
        let origin = transform.translation;
        let dist_sq = (origin.x - player_pos.x).powi(2) + (origin.z - player_pos.z).powi(2);

        let server_node = conn.db.db.resource_node().node_id().find(&node_item.node_id);

        if server_node.is_none() {
            if dist_sq <= NODE_UNLOAD_RADIUS_SQ {
                match node_item.node_type.as_str() {
                    "Tree" => {
                        let mut seed = (origin.x.abs() * 1000.0 + origin.z.abs() * 100.0) as u64;
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        let angle_rand = ((seed as f32) / (u32::MAX as f32)) * std::f32::consts::TAU;
                        let fall_dir = Vec3::new(angle_rand.cos(), 0.0, angle_rand.sin()).normalize();

                        commands.spawn((
                            PbrBundle {
                                mesh: meshes.add(create_voxel_tree_mesh()),
                                material: node_mat.clone(),
                                transform: *transform,
                                ..default()
                            },
                            FallingTree {
                                base_pos: origin,
                                fall_dir,
                                angle: 0.0,
                                angular_vel: 0.35,
                                elapsed: 0.0,
                            },
                        ));
                    }
                    "Rock" => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            origin,
                            24,
                            Color::srgb(0.45, 0.45, 0.48),
                            Color::srgb(0.65, 0.65, 0.68),
                            0.10,
                        );
                    }
                    "Bush" => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            origin,
                            16,
                            Color::srgb(0.15, 0.50, 0.15),
                            Color::srgb(0.85, 0.08, 0.08),
                            0.08,
                        );
                    }
                    _ => {}
                }
            }
            commands.entity(entity).despawn_recursive();
        } else if dist_sq > NODE_UNLOAD_RADIUS_SQ {
            commands.entity(entity).despawn_recursive();
        } else {
            local_nodes.insert(node_item.node_id);
        }
    }

    // 2. Stream new nodes within loading perimeter
    for node in conn.db.db.resource_node().iter() {
        let dist_sq = (node.x - player_pos.x).powi(2) + (node.z - player_pos.z).powi(2);
        if dist_sq > NODE_LOAD_RADIUS_SQ || local_nodes.contains(&node.node_id) {
            continue;
        }

        let clean_type = node.node_type.trim();

        let (mesh, collider, y_offset) = match clean_type {
            "Tree" => (
                meshes.add(create_voxel_tree_mesh()),
                Collider::cylinder(0.35, 9.35),
                0.0,
            ),
            "Rock" => (
                meshes.add(create_voxel_rock_mesh()),
                Collider::cuboid(1.5, 1.4, 1.4),
                0.0,
            ),
            "Bush" => (
                meshes.add(create_voxel_bush_mesh()),
                Collider::sphere(0.85),
                0.0,
            ),
            "Branch" => (
                meshes.add(create_voxel_branch_mesh()),
                Collider::cuboid(1.5, 0.28, 0.9),
                0.02,
            ),
            "Flint" => (
                meshes.add(create_voxel_flint_mesh()),
                Collider::cuboid(0.55, 0.75, 0.45),
                0.02,
            ),
            "LooseStone" => (
                meshes.add(create_voxel_stone_mesh()),
                Collider::cuboid(1.2, 0.58, 0.95),
                0.02,
            ),
            _ => (
                meshes.add(create_voxel_stone_mesh()),
                Collider::cuboid(0.5, 0.5, 0.5),
                0.0,
            )
        };

        commands.spawn((
            PbrBundle {
                mesh, 
                material: node_mat.clone(),
                transform: BevyTransform::from_xyz(node.x, node.y + y_offset, node.z),
                ..default()
            },
            ResourceNodeItem { 
                node_id: node.node_id,
                node_type: clean_type.to_string(),
            },
            RigidBody::Static,
            collider,
            CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
        )).with_children(|parent| {
            if clean_type == "Bush" {
                let offsets = [
                    Vec3::new(0.55, 0.35, 0.0), Vec3::new(-0.35, 0.55, 0.45), 
                    Vec3::new(0.0, 0.30, 0.65), Vec3::new(0.45, 0.45, -0.45), 
                    Vec3::new(-0.55, 0.40, -0.45),
                ];
                
                let berry_mesh = meshes.add(bevy::math::primitives::Cuboid::new(0.18, 0.18, 0.18));
                let red_material = materials.add(StandardMaterial { 
                    base_color: Color::srgb(0.88, 0.08, 0.08), 
                    perceptual_roughness: 0.6,
                    ..default() 
                });

                for offset in offsets {
                    parent.spawn((
                        PbrBundle {
                            mesh: berry_mesh.clone(),
                            material: red_material.clone(),
                            transform: BevyTransform::from_translation(offset),
                            ..default()
                        },
                        BerryVisual { node_id: node.node_id }
                    ));
                }
            }
        });
        local_nodes.insert(node.node_id);
    }
}

pub fn update_falling_trees(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FallingTree, &mut BevyTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_seconds().min(0.1);
    for (entity, mut falling, mut transform) in query.iter_mut() {
        falling.elapsed += dt;

        let gravity_torque = 5.2 * (falling.angle.sin().max(0.15));
        falling.angular_vel += gravity_torque * dt;
        falling.angle += falling.angular_vel * dt;

        let tilt_axis = Vec3::new(falling.fall_dir.z, 0.0, -falling.fall_dir.x).normalize();
        let rot = Quat::from_axis_angle(tilt_axis, falling.angle);

        let kickback = falling.fall_dir * (falling.angle * 0.25);
        transform.translation = falling.base_pos + kickback;
        transform.rotation = rot;

        let tip_world = transform.translation + rot * Vec3::new(0.0, 9.2, 0.0);
        let mid_world = transform.translation + rot * Vec3::new(0.0, 5.0, 0.0);

        let ground_y_at_tip = crate::terrain::get_terrain_height(tip_world.x, tip_world.z);
        let ground_y_at_mid = crate::terrain::get_terrain_height(mid_world.x, mid_world.z);

        let tip_hit_ground = tip_world.y <= ground_y_at_tip + 0.35;
        let mid_hit_ground = mid_world.y <= ground_y_at_mid + 0.35;
        let reached_parallel = falling.angle >= (std::f32::consts::FRAC_PI_2 - 0.04);
        let timeout = falling.elapsed >= 3.5;

        let has_impacted = (falling.angle >= 0.75 && (tip_hit_ground || mid_hit_ground))
            || reached_parallel
            || timeout;

        if has_impacted {
            let base = transform.translation;
            let dir = falling.fall_dir;

            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                base + dir * 1.5,
                16,
                Color::srgb(0.34, 0.22, 0.12),
                Color::srgb(0.44, 0.28, 0.15),
                0.10,
            );

            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                base + dir * 5.0,
                20,
                Color::srgb(0.34, 0.22, 0.12),
                Color::srgb(0.20, 0.55, 0.20),
                0.10,
            );

            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                base + dir * 8.0,
                28,
                Color::srgb(0.18, 0.55, 0.18),
                Color::srgb(0.26, 0.68, 0.26),
                0.08,
            );

            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn update_berry_visuals(
    conn: Res<SpacetimeConnection>,
    mut query: Query<(&mut Visibility, &BerryVisual)>,
) {
    for (mut vis, berry) in query.iter_mut() {
        if let Some(node) = conn.db.db.resource_node().node_id().find(&berry.node_id) {
            *vis = if node.health > 0 { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

pub fn process_combat_events(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    conn: Res<SpacetimeConnection>,
    mut tracker: ResMut<EventTracker>,
) {
    let mut highest_id = tracker.last_event_id;

    for event in conn.db.db.combat_event().iter() {
        if event.id > tracker.last_event_id {
            highest_id = highest_id.max(event.id);
            let pos = Vec3::new(event.x, event.y, event.z);

            match event.event_type.as_str() {
                "HitTree" => {
                    crate::terrain::spawn_voxel_gibs(
                        &mut commands, &mut meshes, &mut materials,
                        pos, 8,
                        Color::srgb(0.35, 0.22, 0.12),
                        Color::srgb(0.20, 0.55, 0.20),
                        0.08,
                    );
                }
                "HitRock" => {
                    crate::terrain::spawn_voxel_gibs(
                        &mut commands, &mut meshes, &mut materials,
                        pos, 8,
                        Color::srgb(0.48, 0.48, 0.50),
                        Color::srgb(0.65, 0.65, 0.68),
                        0.08,
                    );
                }
                "VoxelCollapse" => {
                    crate::terrain::spawn_voxel_gibs(
                        &mut commands, &mut meshes, &mut materials,
                        pos, 24,
                        Color::srgb(0.45, 0.32, 0.20),
                        Color::srgb(0.50, 0.50, 0.52),
                        0.10,
                    );
                }
                "ExplosionBlast" | "SiegeImpact" | "MeteorImpact" => {
                    crate::terrain::spawn_voxel_gibs(
                        &mut commands, &mut meshes, &mut materials,
                        pos, 36,
                        Color::srgb(0.85, 0.45, 0.10),
                        Color::srgb(0.25, 0.25, 0.25),
                        0.10,
                    );
                }
                _ => {
                    let color = match event.event_type.as_str() {
                        "HitBush" => Color::srgb(0.2, 0.6, 0.2), 
                        "HitPlayer" => Color::srgb(0.9, 0.1, 0.1), 
                        _ => Color::WHITE,
                    };

                    let velocities = [
                        Vec3::new(1.0, 3.0, 1.0), Vec3::new(-1.0, 3.5, 0.5), Vec3::new(0.5, 2.5, -1.0),
                        Vec3::new(-0.5, 4.0, -0.5), Vec3::new(0.0, 3.0, 0.0),
                    ];

                    for vel in velocities {
                        commands.spawn((
                            PbrBundle {
                                mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.1, 0.1, 0.1)),
                                material: materials.add(StandardMaterial { base_color: color, unlit: event.event_type == "HitPlayer", ..default() }),
                                transform: BevyTransform::from_xyz(event.x, event.y + 0.5, event.z),
                                ..default()
                            },
                            RigidBody::Dynamic, Collider::cuboid(0.1, 0.1, 0.1), LinearVelocity(vel),
                            Particle { timer: Timer::from_seconds(0.5, TimerMode::Once) }, 
                        ));
                    }
                }
            }
        }
    }
    
    tracker.last_event_id = highest_id;
}