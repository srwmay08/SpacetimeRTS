// ============================================================================
// File: network.rs
// ============================================================================
// ----------------------------------------------------------------------------
// CLIENT-SERVER REPLICATION & COMPOSITE MESH GENERATOR (Bevy Engine / SpacetimeDB)
// ----------------------------------------------------------------------------
// Architectural Note: Provides genuine high-density micro-voxel rasterization
// (0.012m–0.024m for items and creatures, 0.06m for trees) with hidden-face culling.
// Reuses GPU mesh handles in public CachedModelMeshes to preserve 60+ FPS stability.

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::RenderLayers;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::pbr::{FogFalloff, FogSettings};
use avian3d::prelude::*;
use std::collections::HashMap;
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
// PROCEDURAL COMPOSITE VOXEL MESH BUILDER (Building / Structure Compatibility)
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
// GENUINE MICRO-VOXEL RASTERIZER & EXPOSED-FACE EXTRACTOR
// ----------------------------------------------------------------------------

pub struct MicroVoxelGrid {
    pub pitch: f32,
    pub voxels: HashMap<(i32, i32, i32), [f32; 4]>,
}

impl MicroVoxelGrid {
    pub fn new(pitch: f32) -> Self {
        Self {
            pitch,
            voxels: HashMap::with_capacity(8192),
        }
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, z: i32, color: [f32; 4]) {
        self.voxels.insert((x, y, z), color);
    }

    pub fn fill_box(&mut self, min: [i32; 3], max: [i32; 3], color: [f32; 4]) {
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    self.voxels.insert((x, y, z), color);
                }
            }
        }
    }

    pub fn fill_cylinder_y(&mut self, cx: i32, cz: i32, y_min: i32, y_max: i32, radius: f32, color: [f32; 4]) {
        let r_sq = radius * radius;
        let r_ceil = radius.ceil() as i32;
        for dx in -r_ceil..=r_ceil {
            for dz in -r_ceil..=r_ceil {
                if (dx as f32 * dx as f32 + dz as f32 * dz as f32) <= r_sq {
                    for y in y_min..=y_max {
                        self.voxels.insert((cx + dx, y, cz + dz), color);
                    }
                }
            }
        }
    }

    pub fn fill_sphere(&mut self, cx: i32, cy: i32, cz: i32, radius: f32, color: [f32; 4]) {
        let r_sq = radius * radius;
        let r_ceil = radius.ceil() as i32;
        for dx in -r_ceil..=r_ceil {
            for dy in -r_ceil..=r_ceil {
                for dz in -r_ceil..=r_ceil {
                    if (dx as f32 * dx as f32 + dy as f32 * dy as f32 + dz as f32 * dz as f32) <= r_sq {
                        self.voxels.insert((cx + dx, cy + dy, cz + dz), color);
                    }
                }
            }
        }
    }

    pub fn fill_line(&mut self, start: [i32; 3], end: [i32; 3], thickness: i32, color: [f32; 4]) {
        let dx = (end[0] - start[0]) as f32;
        let dy = (end[1] - start[1]) as f32;
        let dz = (end[2] - start[2]) as f32;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0);
        let steps = (dist * 2.0).ceil() as usize;

        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let cx = (start[0] as f32 + dx * t).round() as i32;
            let cy = (start[1] as f32 + dy * t).round() as i32;
            let cz = (start[2] as f32 + dz * t).round() as i32;

            for ox in -thickness..=thickness {
                for oy in -thickness..=thickness {
                    for oz in -thickness..=thickness {
                        self.voxels.insert((cx + ox, cy + oy, cz + oz), color);
                    }
                }
            }
        }
    }

    pub fn build_mesh(&self) -> Mesh {
        let p = self.pitch;
        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(self.voxels.len() * 12);
        let mut normals: Vec<[f32; 3]> = Vec::with_capacity(self.voxels.len() * 12);
        let mut colors: Vec<[f32; 4]> = Vec::with_capacity(self.voxels.len() * 12);
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(self.voxels.len() * 12);
        let mut indices: Vec<u32> = Vec::with_capacity(self.voxels.len() * 18);

        for (&(x, y, z), &col) in &self.voxels {
            let x0 = x as f32 * p;
            let x1 = (x + 1) as f32 * p;
            let y0 = y as f32 * p;
            let y1 = (y + 1) as f32 * p;
            let z0 = z as f32 * p;
            let z1 = (z + 1) as f32 * p;

            // +Y (Top)
            if !self.voxels.contains_key(&(x, y + 1, z)) {
                let s = positions.len() as u32;
                positions.push([x0, y1, z1]);
                positions.push([x1, y1, z1]);
                positions.push([x1, y1, z0]);
                positions.push([x0, y1, z0]);
                for _ in 0..4 { normals.push([0.0, 1.0, 0.0]); colors.push(col); }
                uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
                indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
            }

            // -Y (Bottom)
            if !self.voxels.contains_key(&(x, y - 1, z)) {
                let s = positions.len() as u32;
                positions.push([x0, y0, z0]);
                positions.push([x1, y0, z0]);
                positions.push([x1, y0, z1]);
                positions.push([x0, y0, z1]);
                for _ in 0..4 { normals.push([0.0, -1.0, 0.0]); colors.push(col); }
                uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
                indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
            }

            // +X (East)
            if !self.voxels.contains_key(&(x + 1, y, z)) {
                let s = positions.len() as u32;
                positions.push([x1, y0, z1]);
                positions.push([x1, y0, z0]);
                positions.push([x1, y1, z0]);
                positions.push([x1, y1, z1]);
                for _ in 0..4 { normals.push([1.0, 0.0, 0.0]); colors.push(col); }
                uvs.extend_from_slice(&[[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
                indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
            }

            // -X (West)
            if !self.voxels.contains_key(&(x - 1, y, z)) {
                let s = positions.len() as u32;
                positions.push([x0, y0, z0]);
                positions.push([x0, y0, z1]);
                positions.push([x0, y1, z1]);
                positions.push([x0, y1, z0]);
                for _ in 0..4 { normals.push([-1.0, 0.0, 0.0]); colors.push(col); }
                uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
                indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
            }

            // +Z (South)
            if !self.voxels.contains_key(&(x, y, z + 1)) {
                let s = positions.len() as u32;
                positions.push([x0, y0, z1]);
                positions.push([x1, y0, z1]);
                positions.push([x1, y1, z1]);
                positions.push([x0, y1, z1]);
                for _ in 0..4 { normals.push([0.0, 0.0, 1.0]); colors.push(col); }
                uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
                indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
            }

            // -Z (North)
            if !self.voxels.contains_key(&(x, y, z - 1)) {
                let s = positions.len() as u32;
                positions.push([x1, y0, z0]);
                positions.push([x0, y0, z0]);
                positions.push([x0, y1, z0]);
                positions.push([x1, y1, z0]);
                for _ in 0..4 { normals.push([0.0, 0.0, -1.0]); colors.push(col); }
                uvs.extend_from_slice(&[[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
                indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
            }
        }

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(Indices::U32(indices));
        mesh
    }
}

// ----------------------------------------------------------------------------
// TRUE MICRO-VOXEL TREES (0.06m Voxel Pitch)
// ----------------------------------------------------------------------------

pub fn create_voxel_dead_tree_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.06);
    let bark_dry = [0.74, 0.60, 0.40, 1.0];
    let bark_shadow = [0.52, 0.40, 0.25, 1.0];

    grid.fill_box([-7, 0, -2], [-3, 3, 2], bark_dry);
    grid.fill_box([3, 0, -2], [7, 3, 2], bark_dry);
    grid.fill_box([-2, 0, 3], [2, 3, 7], bark_dry);
    grid.fill_box([-2, 0, -7], [2, 3, -3], bark_dry);

    grid.fill_cylinder_y(0, 0, 0, 20, 4.5, bark_dry);
    grid.fill_cylinder_y(0, 0, 20, 45, 3.8, bark_shadow);
    grid.fill_cylinder_y(0, 0, 45, 75, 3.0, bark_dry);
    grid.fill_cylinder_y(0, 0, 75, 105, 2.2, bark_shadow);

    grid.fill_line([-3, 42, 0], [-12, 54, -2], 1, bark_dry);
    grid.fill_line([-12, 54, -2], [-20, 68, 1], 1, bark_shadow);
    grid.fill_line([-20, 68, 1], [-27, 85, 3], 0, bark_dry);
    grid.fill_line([-20, 68, 1], [-16, 78, -6], 0, bark_dry);

    grid.fill_line([3, 48, 1], [14, 58, 6], 1, bark_dry);
    grid.fill_line([14, 58, 6], [24, 72, 12], 1, bark_shadow);
    grid.fill_line([24, 72, 12], [32, 88, 16], 0, bark_dry);
    grid.fill_line([24, 72, 12], [22, 82, 4], 0, bark_dry);

    grid.fill_line([0, 68, 3], [4, 82, 16], 1, bark_dry);
    grid.fill_line([4, 82, 16], [8, 98, 26], 0, bark_shadow);
    grid.fill_line([4, 82, 16], [-3, 92, 22], 0, bark_dry);

    grid.fill_line([0, 105, 0], [-8, 125, -4], 1, bark_dry);
    grid.fill_line([-8, 125, -4], [-14, 142, -6], 0, bark_dry);
    grid.fill_line([-8, 125, -4], [-4, 138, 4], 0, bark_dry);

    grid.fill_line([0, 105, 0], [7, 126, 3], 1, bark_shadow);
    grid.fill_line([7, 126, 3], [15, 145, 6], 0, bark_dry);
    grid.fill_line([7, 126, 3], [4, 140, -5], 0, bark_dry);

    grid.build_mesh()
}

pub fn create_voxel_oak_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.06);
    let bark = [0.44, 0.28, 0.16, 1.0];
    let amber = [0.94, 0.54, 0.16, 1.0];
    let orange = [0.86, 0.38, 0.10, 1.0];
    let gold = [0.96, 0.70, 0.18, 1.0];

    grid.fill_cylinder_y(0, 0, 0, 38, 6.0, bark);
    grid.fill_line([0, 32, 0], [15, 52, 6], 2, bark);
    grid.fill_line([0, 32, 0], [-15, 50, -5], 2, bark);
    grid.fill_line([0, 34, 0], [-4, 54, 14], 2, bark);

    grid.fill_sphere(0, 72, 0, 24.0, orange);
    grid.fill_sphere(0, 84, 0, 19.0, amber);
    grid.fill_sphere(0, 94, 0, 13.0, gold);

    grid.fill_sphere(-20, 64, -8, 16.0, amber);
    grid.fill_sphere(-24, 76, -6, 12.0, gold);

    grid.fill_sphere(20, 66, 8, 17.0, orange);
    grid.fill_sphere(24, 78, 6, 13.0, amber);

    grid.fill_sphere(-5, 68, 18, 15.0, amber);
    grid.fill_sphere(6, 68, -18, 15.0, orange);

    grid.build_mesh()
}

pub fn create_voxel_pine_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.06);
    let bark = [0.26, 0.16, 0.08, 1.0];
    let d_green = [0.14, 0.36, 0.16, 1.0];
    let m_green = [0.20, 0.50, 0.22, 1.0];
    let l_green = [0.28, 0.62, 0.28, 1.0];

    grid.fill_cylinder_y(0, 0, 0, 60, 3.5, bark);

    grid.fill_cylinder_y(0, 0, 38, 48, 28.0, d_green);
    grid.fill_cylinder_y(0, 0, 48, 58, 22.0, m_green);
    grid.fill_cylinder_y(0, 0, 58, 66, 16.0, m_green);

    grid.fill_cylinder_y(0, 0, 66, 76, 20.0, d_green);
    grid.fill_cylinder_y(0, 0, 76, 86, 15.0, m_green);
    grid.fill_cylinder_y(0, 0, 86, 96, 10.0, l_green);

    grid.fill_cylinder_y(0, 0, 96, 106, 12.0, d_green);
    grid.fill_cylinder_y(0, 0, 106, 116, 8.0, m_green);
    grid.fill_cylinder_y(0, 0, 116, 126, 4.5, l_green);
    grid.fill_cylinder_y(0, 0, 126, 134, 1.5, l_green);

    grid.build_mesh()
}

pub fn create_voxel_round_tree_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.06);
    let bark = [0.22, 0.16, 0.10, 1.0];
    let shadow = [0.45, 0.32, 0.10, 1.0];
    let gold_mid = [0.88, 0.72, 0.18, 1.0];
    let gold_bright = [0.96, 0.82, 0.22, 1.0];

    grid.fill_cylinder_y(0, 0, 0, 42, 4.0, bark);
    grid.fill_sphere(0, 46, 0, 26.0, shadow);
    grid.fill_sphere(0, 56, 0, 32.0, gold_mid);
    grid.fill_sphere(0, 70, 0, 24.0, gold_bright);
    grid.fill_sphere(0, 82, 0, 14.0, gold_bright);

    grid.build_mesh()
}

// ----------------------------------------------------------------------------
// TRUE MICRO-VOXEL CREATURES & NPCS (0.024m–0.028m Voxel Pitch)
// ----------------------------------------------------------------------------

pub fn create_voxel_deer_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.022);
    let tawny = [0.84, 0.62, 0.42, 1.0];
    let shadow = [0.65, 0.45, 0.30, 1.0];
    let white = [0.96, 0.94, 0.90, 1.0];
    let cream = [0.92, 0.86, 0.78, 1.0];
    let black = [0.12, 0.12, 0.14, 1.0];
    let antler = [0.22, 0.14, 0.10, 1.0];

    // Slender cloven hooves
    grid.fill_box([-7, 0, 9], [-4, 3, 13], black);
    grid.fill_box([4, 0, 9], [7, 3, 13], black);
    grid.fill_box([-7, 0, -13], [-4, 3, -9], black);
    grid.fill_box([4, 0, -13], [7, 3, -9], black);

    // Multi-jointed slender legs
    grid.fill_box([-7, 3, 9], [-5, 28, 12], shadow);
    grid.fill_box([5, 3, 9], [7, 28, 12], shadow);
    grid.fill_box([-7, 3, -13], [-5, 28, -10], shadow);
    grid.fill_box([5, 3, -13], [7, 28, -10], shadow);

    // Hock joints & knees
    grid.fill_box([-8, 18, 9], [-4, 22, 13], tawny);
    grid.fill_box([4, 18, 9], [8, 22, 13], tawny);
    grid.fill_box([-8, 20, -14], [-4, 24, -9], shadow);
    grid.fill_box([4, 20, -14], [8, 24, -9], shadow);

    // Contoured muscular haunches
    grid.fill_box([-9, 28, 7], [-3, 44, 15], tawny);
    grid.fill_box([3, 28, 7], [9, 44, 15], tawny);
    grid.fill_box([-9, 28, -15], [-3, 46, -7], shadow);
    grid.fill_box([3, 28, -15], [9, 46, -7], shadow);

    // Sculpted torso with narrow waist
    grid.fill_box([-8, 34, -16], [8, 52, 14], tawny);
    grid.fill_box([-7, 32, -14], [7, 39, 12], cream);

    // Dappled spots along flank
    let spots = [
        (-9, 46, -12), (-9, 48, -6), (-9, 44, 0), (-9, 49, 6),
        (9, 46, -12), (9, 48, -6), (9, 44, 0), (9, 49, 6),
    ];
    for (sx, sy, sz) in spots {
        grid.fill_box([sx, sy, sz], [sx, sy + 1, sz + 1], white);
    }

    // White chest bib and tail
    grid.fill_box([-6, 38, 13], [6, 54, 18], white);
    grid.fill_box([-2, 45, -20], [2, 53, -16], white);

    // Slender forward-angled neck
    grid.fill_box([-5, 48, 9], [5, 68, 17], tawny);
    grid.fill_box([-4, 50, 16], [4, 67, 19], white);

    // Head, muzzle, black nose pad
    grid.fill_box([-5, 62, 15], [5, 72, 26], tawny);
    grid.fill_box([-4, 62, 21], [4, 67, 30], cream);
    grid.fill_box([-4, 63, 28], [4, 69, 32], black);

    // Almond eyes & backward ears
    grid.fill_box([-6, 68, 20], [-5, 71, 22], black);
    grid.fill_box([5, 68, 20], [6, 71, 22], black);
    grid.fill_line([-5, 70, 15], [-11, 77, 12], 0, tawny);
    grid.fill_line([5, 70, 15], [11, 77, 12], 0, tawny);
    grid.fill_box([-10, 72, 13], [-6, 76, 14], white);
    grid.fill_box([6, 72, 13], [10, 76, 14], white);

    // Branching antler rack
    grid.fill_line([-4, 71, 16], [-6, 84, 14], 1, antler);
    grid.fill_line([4, 71, 16], [6, 84, 14], 1, antler);
    grid.fill_line([-6, 84, 14], [-13, 98, 11], 0, antler);
    grid.fill_line([6, 84, 14], [13, 98, 11], 0, antler);
    grid.fill_line([-6, 84, 14], [-2, 91, 23], 0, antler);
    grid.fill_line([6, 84, 14], [2, 91, 23], 0, antler);
    grid.fill_line([-11, 92, 12], [-15, 105, 16], 0, antler);
    grid.fill_line([11, 92, 12], [15, 105, 16], 0, antler);

    grid.build_mesh()
}

pub fn create_voxel_boar_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.024);
    let umber = [0.28, 0.16, 0.08, 1.0];
    let ochre = [0.65, 0.44, 0.22, 1.0];
    let highlight = [0.82, 0.62, 0.36, 1.0];
    let snout = [0.65, 0.45, 0.40, 1.0];
    let tusk = [0.96, 0.93, 0.86, 1.0];
    let hoof = [0.14, 0.12, 0.10, 1.0];

    // Hooves
    grid.fill_box([-11, 0, 8], [-7, 3, 12], hoof);
    grid.fill_box([7, 0, 8], [11, 3, 12], hoof);
    grid.fill_box([-11, 0, -14], [-7, 3, -10], hoof);
    grid.fill_box([7, 0, -14], [11, 3, -10], hoof);

    // Sturdy legs
    grid.fill_box([-10, 3, 8], [-8, 14, 12], umber);
    grid.fill_box([8, 3, 8], [10, 14, 12], umber);
    grid.fill_box([-10, 3, -14], [-8, 14, -10], umber);
    grid.fill_box([8, 3, -14], [10, 14, -10], umber);

    // Heavy stocky body with ochre stripes
    grid.fill_box([-13, 12, -20], [13, 30, 16], umber);
    grid.fill_box([-12, 14, -16], [12, 28, 13], ochre);
    grid.fill_box([-11, 17, -12], [11, 26, 9], highlight);

    // Raised spine bristle crest
    grid.fill_box([-3, 30, -18], [3, 37, 13], umber);
    grid.fill_box([-1, 36, -14], [1, 39, 10], ochre);

    // Sloping wedge head & heavy jowls
    grid.fill_box([-9, 14, 13], [9, 28, 29], umber);
    grid.fill_box([-6, 15, 27], [6, 23, 38], snout);

    // Upward-curved ivory tusks
    grid.fill_box([-8, 16, 28], [-6, 25, 31], tusk);
    grid.fill_box([6, 16, 28], [8, 25, 31], tusk);

    grid.build_mesh()
}

pub fn create_voxel_goblin_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.024);
    let skin = [0.38, 0.68, 0.22, 1.0];
    let skin_shadow = [0.26, 0.48, 0.16, 1.0];
    let iron = [0.46, 0.48, 0.52, 1.0];
    let iron_dark = [0.28, 0.30, 0.34, 1.0];
    let leather = [0.36, 0.22, 0.14, 1.0];
    let gold = [0.94, 0.78, 0.18, 1.0];
    let eye = [0.98, 0.92, 0.15, 1.0];
    let bone = [0.92, 0.88, 0.78, 1.0];

    // Armored boots
    grid.fill_box([-8, 0, -5], [-3, 6, 5], iron_dark);
    grid.fill_box([3, 0, -5], [8, 6, 5], iron_dark);
    grid.fill_box([-7, 6, -4], [-4, 18, 4], skin);
    grid.fill_box([4, 6, -4], [7, 18, 4], skin);

    // Studded war belt, gold buckle & tassets
    grid.fill_box([-8, 18, -6], [8, 23, 6], leather);
    grid.fill_box([-4, 18, 6], [4, 23, 7], gold);
    grid.fill_box([-4, 11, 5], [4, 18, 6], leather);

    // Segmented breastplate
    grid.fill_box([-8, 23, -5], [8, 38, 5], iron);
    grid.fill_box([-7, 24, -6], [7, 37, -5], iron_dark);

    // Rounded dual-tier pauldrons
    grid.fill_box([-14, 33, -5], [-8, 41, 5], iron_dark);
    grid.fill_box([-15, 35, -4], [-8, 40, 4], iron);
    grid.fill_box([8, 33, -5], [14, 41, 5], iron_dark);
    grid.fill_box([8, 35, -4], [15, 40, 4], iron);

    // Arms & bracers
    grid.fill_box([-13, 21, -4], [-8, 33, 4], skin);
    grid.fill_box([8, 21, -4], [13, 33, 4], skin);
    grid.fill_box([-13, 16, -4], [-8, 22, 4], iron);
    grid.fill_box([8, 16, -4], [13, 22, 4], iron);

    // Goblin head & jaw
    grid.fill_box([-8, 38, -5], [8, 52, 6], skin);
    grid.fill_box([-7, 38, 4], [7, 43, 7], skin_shadow);

    // Lower jaw tusks
    grid.fill_box([-5, 40, 6], [-4, 44, 7], bone);
    grid.fill_box([4, 40, 6], [5, 44, 7], bone);

    // Glowing eyes
    grid.fill_box([-6, 45, 6], [-4, 47, 7], eye);
    grid.fill_box([4, 45, 6], [6, 47, 7], eye);

    // Pointed lateral ears
    grid.fill_line([-8, 44, 0], [-16, 50, -1], 0, skin);
    grid.fill_line([8, 44, 0], [16, 50, -1], 0, skin);

    // Horned iron helmet
    grid.fill_box([-8, 49, -6], [8, 56, 6], iron);
    grid.fill_line([-6, 54, 0], [-12, 64, 4], 0, bone);
    grid.fill_line([6, 54, 0], [12, 64, 4], 0, bone);

    grid.build_mesh()
}

pub fn create_voxel_peasant_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.026);
    let skin = [0.86, 0.72, 0.60, 1.0];
    let shirt = [0.22, 0.42, 0.85, 1.0];
    let pants = [0.32, 0.26, 0.20, 1.0];
    let hair = [0.28, 0.18, 0.10, 1.0];
    let boots = [0.18, 0.12, 0.08, 1.0];

    grid.fill_box([-7, 0, -4], [-2, 5, 4], boots);
    grid.fill_box([2, 0, -4], [7, 5, 4], boots);
    grid.fill_box([-6, 5, -3], [-2, 18, 3], pants);
    grid.fill_box([2, 5, -3], [6, 18, 3], pants);

    grid.fill_box([-8, 18, -5], [8, 34, 5], shirt);
    grid.fill_box([-12, 18, -3], [-8, 33, 3], shirt);
    grid.fill_box([8, 18, -3], [12, 33, 3], shirt);
    grid.fill_box([-12, 14, -3], [-8, 18, 3], skin);
    grid.fill_box([8, 14, -3], [12, 18, 3], skin);

    grid.fill_box([-5, 34, -5], [5, 44, 5], skin);
    grid.fill_box([-6, 42, -6], [6, 47, 6], hair);

    grid.build_mesh()
}

pub fn create_voxel_pet_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.026);
    let coat = [0.86, 0.52, 0.18, 1.0];
    let cream = [0.95, 0.90, 0.80, 1.0];
    let nose = [0.10, 0.10, 0.10, 1.0];
    let ears = [0.68, 0.38, 0.12, 1.0];

    grid.fill_box([-4, 0, -6], [-2, 5, -4], coat);
    grid.fill_box([2, 0, -6], [4, 5, -4], coat);
    grid.fill_box([-4, 0, 4], [-2, 5, 6], coat);
    grid.fill_box([2, 0, 4], [4, 5, 6], coat);

    grid.fill_box([-4, 5, -8], [4, 11, 8], coat);
    grid.fill_box([-3, 4, -6], [3, 7, 6], cream);

    grid.fill_box([-3, 10, 5], [3, 16, 11], coat);
    grid.fill_box([-2, 10, 11], [2, 13, 14], cream);
    grid.set(0, 13, 14, nose);

    grid.fill_box([-5, 14, 6], [-3, 18, 9], ears);
    grid.fill_box([3, 14, 6], [5, 18, 9], ears);

    grid.build_mesh()
}

pub fn create_voxel_rock_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.045);
    let slate = [0.35, 0.35, 0.38, 1.0];
    let granite_mid = [0.50, 0.50, 0.53, 1.0];
    let highlight = [0.75, 0.75, 0.78, 1.0];

    grid.fill_sphere(0, 10, 0, 17.0, slate);
    grid.fill_sphere(0, 15, 0, 13.0, granite_mid);
    grid.fill_sphere(-3, 20, -3, 8.0, highlight);
    grid.build_mesh()
}

pub fn create_voxel_bush_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.045);
    let dark = [0.14, 0.44, 0.14, 1.0];
    let bright = [0.26, 0.66, 0.26, 1.0];
    let berry = [0.92, 0.10, 0.10, 1.0];

    grid.fill_sphere(0, 10, 0, 16.0, dark);
    grid.fill_sphere(0, 15, 0, 11.0, bright);

    grid.fill_box([10, 12, 0], [12, 14, 2], berry);
    grid.fill_box([-11, 13, 2], [-9, 15, 4], berry);
    grid.fill_box([0, 17, 9], [2, 19, 11], berry);
    grid.fill_box([-3, 15, -10], [-1, 17, -8], berry);

    grid.build_mesh()
}

pub fn create_voxel_branch_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.015);
    let bark_dark = [0.34, 0.22, 0.12, 1.0];
    let bark_mid = [0.46, 0.30, 0.16, 1.0];
    let broken_sapwood = [0.78, 0.65, 0.46, 1.0];

    grid.fill_cylinder_y(-18, 0, 1, 4, 3.0, broken_sapwood);
    grid.fill_cylinder_y(-18, 0, 0, 1, 3.5, bark_dark);

    grid.fill_line([-18, 2, 0], [-6, 3, 2], 1, bark_mid);
    grid.fill_line([-6, 3, 2], [6, 2, -1], 1, bark_dark);
    grid.fill_line([6, 2, -1], [18, 4, 3], 1, bark_mid);
    grid.fill_line([18, 4, 3], [28, 5, 1], 0, bark_dark);

    grid.fill_line([-4, 3, 2], [2, 5, 8], 0, bark_mid);
    grid.fill_line([2, 5, 8], [6, 7, 14], 0, bark_mid);
    grid.fill_line([8, 3, 0], [14, 5, -8], 0, bark_dark);
    grid.fill_line([14, 5, -8], [19, 6, -14], 0, bark_mid);

    grid.build_mesh()
}

pub fn create_voxel_flint_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.012);
    let chert_black = [0.06, 0.08, 0.10, 1.0];
    let chert_dark = [0.14, 0.22, 0.32, 1.0];
    let chert_blue = [0.24, 0.42, 0.58, 1.0];
    let cyan_facet = [0.42, 0.72, 0.88, 1.0];
    let highlight = [0.82, 0.94, 1.0, 1.0];

    grid.fill_box([-7, 1, -6], [7, 6, 6], chert_black);
    grid.fill_box([-5, 6, -5], [5, 11, 5], chert_dark);
    grid.fill_box([-4, 11, -4], [4, 16, 4], chert_blue);
    grid.fill_box([-2, 16, -2], [2, 21, 2], cyan_facet);
    grid.fill_box([-1, 21, -1], [1, 25, 1], highlight);

    grid.fill_line([-7, 3, 6], [0, 23, 2], 0, cyan_facet);
    grid.fill_line([7, 3, -6], [0, 23, -2], 0, cyan_facet);
    grid.fill_line([-7, 3, -6], [0, 23, -2], 0, cyan_facet);
    grid.fill_line([7, 3, 6], [0, 23, 2], 0, cyan_facet);

    grid.build_mesh()
}

pub fn create_voxel_stone_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.02);
    let granite = [0.52, 0.50, 0.48, 1.0];
    let granite_dark = [0.38, 0.36, 0.35, 1.0];
    let quartz_fleck = [0.85, 0.85, 0.82, 1.0];

    grid.fill_sphere(0, 5, 0, 7.0, granite);
    grid.fill_box([-3, 2, -2], [4, 7, 3], granite_dark);
    grid.set(2, 6, 3, quartz_fleck);
    grid.set(-2, 7, -1, quartz_fleck);

    grid.build_mesh()
}

// ----------------------------------------------------------------------------
// STATIC MESH CACHING (Public Interface for Bevy SystemParam Safety)
// ----------------------------------------------------------------------------

pub struct CachedModelMeshes {
    pub dead_tree: Handle<Mesh>,
    pub oak_tree: Handle<Mesh>,
    pub pine_tree: Handle<Mesh>,
    pub round_tree: Handle<Mesh>,
    pub rock: Handle<Mesh>,
    pub bush: Handle<Mesh>,
    pub branch: Handle<Mesh>,
    pub flint: Handle<Mesh>,
    pub stone: Handle<Mesh>,
    pub deer: Handle<Mesh>,
    pub boar: Handle<Mesh>,
    pub goblin: Handle<Mesh>,
    pub peasant: Handle<Mesh>,
    pub pet: Handle<Mesh>,
}

// ----------------------------------------------------------------------------
// NETWORK CONNECTION SYSTEM
// ----------------------------------------------------------------------------

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
        ColliderDensity(1.0),
        SweptCcd::default(),
        CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Terrain, GameLayer::Environment]),
        LockedAxes::ROTATION_LOCKED,
        GravityScale(0.0),
        LinearVelocity::ZERO,
        ExternalForce::default().with_persistence(false),
        Kcc { is_grounded: false },
    ));

    player_entity_commands.insert((
        LogicalPosition(Vec3::new(0.0, 25.0, 0.0)),
        LogicalRotation(Quat::IDENTITY),
        crate::components::Faction::Player, 
        Selectable, 
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
                transform: BevyTransform::from_xyz(0.0, -1.05, 0.0),
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
        directional_light: DirectionalLight { illuminance: 9_000.0, shadows_enabled: true, ..default() },
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
                
                let spawn_y = crate::terrain::get_terrain_height(0.0, 0.0) + 1.05;
                
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
    mut model_cache: Local<Option<CachedModelMeshes>>,
) {
    let _ = conn.db.frame_tick();
    
    let my_entity_id = conn.identity.as_ref()
        .and_then(|id| conn.db.db.player().identity().find(id))
        .map(|p| p.entity_id);

    let player_pos = player_body_query.get_single().map(|t| t.translation).unwrap_or(Vec3::ZERO);

    const CREATURE_LOAD_RADIUS_SQ: f32 = 75.0 * 75.0;
    const CREATURE_UNLOAD_RADIUS_SQ: f32 = 80.0 * 80.0;

    let cache = model_cache.get_or_insert_with(|| {
        CachedModelMeshes {
            dead_tree: meshes.add(create_voxel_dead_tree_mesh()),
            oak_tree: meshes.add(create_voxel_oak_mesh()),
            pine_tree: meshes.add(create_voxel_pine_mesh()),
            round_tree: meshes.add(create_voxel_round_tree_mesh()),
            rock: meshes.add(create_voxel_rock_mesh()),
            bush: meshes.add(create_voxel_bush_mesh()),
            branch: meshes.add(create_voxel_branch_mesh()),
            flint: meshes.add(create_voxel_flint_mesh()),
            stone: meshes.add(create_voxel_stone_mesh()),
            deer: meshes.add(create_voxel_deer_mesh()),
            boar: meshes.add(create_voxel_boar_mesh()),
            goblin: meshes.add(create_voxel_goblin_mesh()),
            peasant: meshes.add(create_voxel_peasant_mesh()),
            pet: meshes.add(create_voxel_pet_mesh()),
        }
    });

    spawned_ids.clear();

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
        
        let mut visual_transform = BevyTransform::from_xyz(0.0, -1.05, 0.0);

        let (mesh_handle, root_collider) = if is_pet {
            visual_transform.translation.y = -0.45;
            (cache.pet.clone(), Collider::cuboid(0.5, 0.8, 0.9))
        } else if let Some(brain) = npc_brain {
            let m = match brain.ai_type {
                crate::module_bindings::AiType::Boar => {
                    visual_transform.translation.y = -1.05;
                    (cache.boar.clone(), Collider::cuboid(0.8, 0.8, 1.4))
                }
                crate::module_bindings::AiType::Deer => {
                    visual_transform.translation.y = -1.05;
                    (cache.deer.clone(), Collider::cuboid(0.6, 1.8, 1.2))
                }
                crate::module_bindings::AiType::Goblin => {
                    visual_transform.translation.y = -1.05;
                    (cache.goblin.clone(), Collider::capsule(0.4, 1.3))
                }
                crate::module_bindings::AiType::Friendly | crate::module_bindings::AiType::Peasant => {
                    visual_transform.translation.y = -1.05;
                    (cache.peasant.clone(), Collider::capsule(0.4, 1.8))
                }
            };

            if brain.state == crate::module_bindings::BrainState::Corpse {
                visual_transform.translation.y = -0.35;
                visual_transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
            }
            m
        } else if is_peasant {
            visual_transform.translation.y = -1.05;
            (cache.peasant.clone(), Collider::capsule(0.4, 1.8))
        } else {
            visual_transform.translation.y = -1.05;
            (cache.peasant.clone(), Collider::capsule(0.4, 1.8))
        };

        let mut entity_cmds = commands.spawn((
            NetworkEntity(id),
            SpatialBundle::from_transform(BevyTransform::from_xyz(db_t.x, db_t.y, db_t.z)),
            LogicalPosition(Vec3::new(db_t.x, db_t.y, db_t.z)),
            LogicalRotation(Quat::IDENTITY),
            Selectable, 
            RigidBody::Kinematic, 
            root_collider,
            CollisionLayers::new([GameLayer::Unit], [GameLayer::Terrain]),
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

pub fn sync_resource_nodes(
    mut commands: Commands, 
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    node_query: Query<(Entity, &ResourceNodeItem, &BevyTransform)>, 
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    conn: Res<SpacetimeConnection>,
    mut default_node_mat: Local<Option<Handle<StandardMaterial>>>,
    mut local_nodes: Local<std::collections::HashSet<u64>>,
    mut scan_timer: Local<Option<Timer>>,
    mut model_cache: Local<Option<CachedModelMeshes>>,
) {
    let Ok(player_transform) = player_query.get_single() else { return; };
    let player_pos = player_transform.translation;

    let timer = scan_timer.get_or_insert_with(|| Timer::from_seconds(0.1, TimerMode::Repeating));
    if !timer.tick(time.delta()).just_finished() {
        return;
    }

    let cache = model_cache.get_or_insert_with(|| {
        CachedModelMeshes {
            dead_tree: meshes.add(create_voxel_dead_tree_mesh()),
            oak_tree: meshes.add(create_voxel_oak_mesh()),
            pine_tree: meshes.add(create_voxel_pine_mesh()),
            round_tree: meshes.add(create_voxel_round_tree_mesh()),
            rock: meshes.add(create_voxel_rock_mesh()),
            bush: meshes.add(create_voxel_bush_mesh()),
            branch: meshes.add(create_voxel_branch_mesh()),
            flint: meshes.add(create_voxel_flint_mesh()),
            stone: meshes.add(create_voxel_stone_mesh()),
            deer: meshes.add(create_voxel_deer_mesh()),
            boar: meshes.add(create_voxel_boar_mesh()),
            goblin: meshes.add(create_voxel_goblin_mesh()),
            peasant: meshes.add(create_voxel_peasant_mesh()),
            pet: meshes.add(create_voxel_pet_mesh()),
        }
    });

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

                        let tree_style = (node_item.node_id % 4) as u8;
                        let tree_mesh = match tree_style {
                            0 => cache.dead_tree.clone(),
                            1 => cache.oak_tree.clone(),
                            2 => cache.pine_tree.clone(),
                            _ => cache.round_tree.clone(),
                        };

                        commands.spawn((
                            PbrBundle {
                                mesh: tree_mesh,
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

    for node in conn.db.db.resource_node().iter() {
        let dist_sq = (node.x - player_pos.x).powi(2) + (node.z - player_pos.z).powi(2);
        if dist_sq > NODE_LOAD_RADIUS_SQ || local_nodes.contains(&node.node_id) {
            continue;
        }

        let clean_type = node.node_type.trim();

        let (mesh, collider, y_offset) = match clean_type {
            "Tree" => {
                let tree_style = (node.node_id % 4) as u8;
                let tree_mesh = match tree_style {
                    0 => cache.dead_tree.clone(),
                    1 => cache.oak_tree.clone(),
                    2 => cache.pine_tree.clone(),
                    _ => cache.round_tree.clone(),
                };
                (
                    tree_mesh,
                    Collider::cylinder(0.40, 8.5),
                    0.0,
                )
            }
            "Rock" => (
                cache.rock.clone(),
                Collider::cuboid(1.5, 1.4, 1.4),
                0.0,
            ),
            "Bush" => (
                cache.bush.clone(),
                Collider::sphere(0.85),
                0.0,
            ),
            "Branch" => (
                cache.branch.clone(),
                Collider::cuboid(1.5, 0.28, 0.9),
                0.02,
            ),
            "Flint" => (
                cache.flint.clone(),
                Collider::cuboid(0.55, 0.75, 0.45),
                0.02,
            ),
            "LooseStone" => (
                cache.stone.clone(),
                Collider::cuboid(1.2, 0.58, 0.95),
                0.02,
            ),
            _ => (
                cache.stone.clone(),
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
        ));
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
    _conn: Res<SpacetimeConnection>,
    _query: Query<(&mut Visibility, &BerryVisual)>,
) {}

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
                            RigidBody::Dynamic,
                            Collider::cuboid(0.1, 0.1, 0.1),
                            ColliderDensity(1.0),
                            LinearVelocity(vel),
                            Particle { timer: Timer::from_seconds(0.5, TimerMode::Once) }, 
                        ));
                    }
                }
            }
        }
    }
    
    tracker.last_event_id = highest_id;
}