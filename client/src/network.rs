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
use bevy::pbr::{FogFalloff, FogSettings, NotShadowCaster};
use avian3d::prelude::*;
// AI_RULES.md Rule 2.1 #3: BTreeMap and BTreeSet guarantee deterministic ordering without randomized SipHash
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

use spacetimedb_sdk::{DbContext, Table}; 

use crate::module_bindings::{self, *};
use crate::module_bindings::peasant_table::PeasantTableAccess; 
use crate::module_bindings::npc_brain_table::NpcBrainTableAccess; 
use crate::module_bindings::pet_component_table::PetComponentTableAccess; 
use crate::module_bindings::resource_node_table::ResourceNodeTableAccess;
use crate::module_bindings::active_projectile_table::ActiveProjectileTableAccess;
use crate::module_bindings::projectile_kind_type::ProjectileKind;
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
// SEEDED DETERMINISTIC PRNG UTILITY
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Prng {
    pub state: u64,
}

impl Prng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x517cc1b727220a95 } else { seed },
        }
    }

    pub fn next(&mut self) -> f64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as f64) / (u64::MAX as f64)
    }

    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (self.next() as f32) * (max - min)
    }

    pub fn blend_color(&mut self, c1: [f32; 4], c2: [f32; 4], t: f32) -> [f32; 4] {
        let f = t.clamp(0.0, 1.0);
        let inv = 1.0 - f;
        [
            c1[0] * inv + c2[0] * f,
            c1[1] * inv + c2[1] * f,
            c1[2] * inv + c2[2] * f,
            c1[3] * inv + c2[3] * f,
        ]
    }
}

// ----------------------------------------------------------------------------
// GENUINE MICRO-VOXEL RASTERIZER & EXPOSED-FACE EXTRACTOR
// ----------------------------------------------------------------------------

pub struct MicroVoxelGrid {
    pub pitch: f32,
    pub voxels: BTreeMap<(i32, i32, i32), [f32; 4]>,
}

impl MicroVoxelGrid {
    pub fn new(pitch: f32) -> Self {
        Self {
            pitch,
            voxels: BTreeMap::new(),
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

    pub fn fill_ellipsoid(
        &mut self,
        cx: f32, cy: f32, cz: f32,
        rx: f32, ry: f32, rz: f32,
        color: [f32; 4],
    ) {
        if rx <= 0.0 || ry <= 0.0 || rz <= 0.0 {
            return;
        }
        let min_x = (cx - rx).floor() as i32;
        let max_x = (cx + rx).ceil() as i32;
        let min_y = (cy - ry).floor() as i32;
        let max_y = (cy + ry).ceil() as i32;
        let min_z = (cz - rz).floor() as i32;
        let max_z = (cz + rz).ceil() as i32;

        let inv_rx2 = 1.0 / (rx * rx);
        let inv_ry2 = 1.0 / (ry * ry);
        let inv_rz2 = 1.0 / (rz * rz);

        for x in min_x..=max_x {
            let dx = x as f32 - cx;
            let term_x = dx * dx * inv_rx2;
            if term_x > 1.0 { continue; }
            for y in min_y..=max_y {
                let dy = y as f32 - cy;
                let term_y = dy * dy * inv_ry2;
                if term_x + term_y > 1.0 { continue; }
                for z in min_z..=max_z {
                    let dz = z as f32 - cz;
                    let term_z = dz * dz * inv_rz2;
                    if term_x + term_y + term_z <= 1.0 {
                        self.voxels.insert((x, y, z), color);
                    }
                }
            }
        }
    }

    pub fn fill_curved_cylinder_y(
        &mut self,
        x0: f32, z0: f32,
        y_start: i32, y_end: i32,
        radius: f32,
        curve_x: f32, curve_z: f32,
        color: [f32; 4],
    ) {
        if y_start >= y_end { return; }
        let h = (y_end - y_start) as f32;
        for y in y_start..=y_end {
            let t = (y - y_start) as f32 / h;
            let offset_x = curve_x * t * t;
            let offset_z = curve_z * t * t;
            let r = radius * (1.0 - t * 0.25).max(1.0);
            self.fill_cylinder_y(
                (x0 + offset_x).round() as i32,
                (z0 + offset_z).round() as i32,
                y,
                y,
                r,
                color,
            );
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

pub fn create_voxel_dead_tree_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut grid = MicroVoxelGrid::new(0.06);

    let bark_base = [0.74, 0.60, 0.40, 1.0];
    let shadow_base = [0.52, 0.40, 0.25, 1.0];
    let weathered = [0.62, 0.52, 0.36, 1.0];

    let t_col = rng.range(0.0, 1.0);
    let bark_dry = rng.blend_color(bark_base, weathered, t_col);
    let bark_shadow = rng.blend_color(shadow_base, weathered, t_col);

    let height_mult = rng.range(0.85, 1.15);
    let trunk_h = (105.0 * height_mult).round() as i32;
    let trunk_r = rng.range(3.8, 5.2);
    let curve_x = rng.range(-3.5, 3.5);
    let curve_z = rng.range(-3.5, 3.5);

    // Gnarled root flares
    grid.fill_box([-7, 0, -2], [-3, 3, 2], bark_dry);
    grid.fill_box([3, 0, -2], [7, 3, 2], bark_dry);
    grid.fill_box([-2, 0, 3], [2, 3, 7], bark_dry);
    grid.fill_box([-2, 0, -7], [2, 3, -3], bark_dry);

    // Curved tapered trunk
    grid.fill_curved_cylinder_y(0.0, 0.0, 0, trunk_h, trunk_r, curve_x, curve_z, bark_dry);

    // Bare branches with procedural lengths and angles
    let branch_scale = rng.range(0.80, 1.25);
    let b1_len = (18.0 * branch_scale) as i32;
    let b2_len = (22.0 * branch_scale) as i32;
    let b3_len = (16.0 * branch_scale) as i32;

    let h1 = (trunk_h as f32 * 0.42).round() as i32;
    let h2 = (trunk_h as f32 * 0.48).round() as i32;
    let h3 = (trunk_h as f32 * 0.68).round() as i32;
    let top_y = trunk_h;

    // Branch cluster 1 (West/North)
    let ang1 = rng.range(-0.3, 0.3);
    grid.fill_line([-3, h1, 0], [-12 + (ang1 * 10.0) as i32, h1 + 12, -2], 1, bark_dry);
    grid.fill_line([-12, h1 + 12, -2], [-12 - b1_len, h1 + 24, 1], 1, bark_shadow);
    grid.fill_line([-12 - b1_len, h1 + 24, 1], [-12 - b1_len - 6, h1 + 38, 3], 0, bark_dry);
    grid.fill_line([-12 - b1_len, h1 + 24, 1], [-12 - b1_len + 4, h1 + 32, -6], 0, bark_dry);

    // Branch cluster 2 (East/South)
    grid.fill_line([3, h2, 1], [14, h2 + 10, 6], 1, bark_dry);
    grid.fill_line([14, h2 + 10, 6], [14 + b2_len, h2 + 22, 12], 1, bark_shadow);
    grid.fill_line([14 + b2_len, h2 + 22, 12], [14 + b2_len + 8, h2 + 36, 16], 0, bark_dry);
    grid.fill_line([14 + b2_len, h2 + 22, 12], [14 + b2_len - 2, h2 + 30, 4], 0, bark_dry);

    // Branch cluster 3 (Upper South)
    grid.fill_line([0, h3, 3], [4, h3 + 14, 16], 1, bark_dry);
    grid.fill_line([4, h3 + 14, 16], [4 + (b3_len / 2), h3 + 28, 16 + b3_len], 0, bark_shadow);
    grid.fill_line([4, h3 + 14, 16], [-3, h3 + 24, 22], 0, bark_dry);

    // Crown split spires
    let top_cx = curve_x.round() as i32;
    let top_cz = curve_z.round() as i32;
    grid.fill_line([top_cx, top_y, top_cz], [top_cx - 8, top_y + 20, top_cz - 4], 1, bark_dry);
    grid.fill_line([top_cx - 8, top_y + 20, top_cz - 4], [top_cx - 14, top_y + 36, top_cz - 6], 0, bark_dry);

    grid.fill_line([top_cx, top_y, top_cz], [top_cx + 7, top_y + 21, top_cz + 3], 1, bark_shadow);
    grid.fill_line([top_cx + 7, top_y + 21, top_cz + 3], [top_cx + 15, top_y + 38, top_cz + 6], 0, bark_dry);

    grid.build_mesh()
}

pub fn create_voxel_oak_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut grid = MicroVoxelGrid::new(0.06);

    let bark = [0.44, 0.28, 0.16, 1.0];
    let amber_base = [0.94, 0.54, 0.16, 1.0];
    let orange_base = [0.86, 0.38, 0.10, 1.0];
    let gold_base = [0.96, 0.70, 0.18, 1.0];
    let russet = [0.76, 0.32, 0.12, 1.0];

    let t_palette = rng.range(0.0, 1.0);
    let orange = rng.blend_color(orange_base, russet, t_palette);
    let amber = rng.blend_color(amber_base, orange_base, t_palette * 0.5);
    let gold = rng.blend_color(gold_base, amber_base, t_palette * 0.3);

    let trunk_h = rng.range(33.0, 44.0).round() as i32;
    let trunk_r = rng.range(5.2, 6.8);
    let curve_x = rng.range(-3.0, 3.0);
    let curve_z = rng.range(-3.0, 3.0);

    // Curved organic trunk
    grid.fill_curved_cylinder_y(0.0, 0.0, 0, trunk_h, trunk_r, curve_x, curve_z, bark);

    // Gnarled root base
    grid.fill_cylinder_y(0, 0, 0, 4, trunk_r + 2.0, bark);

    // Heavy boughs spreading outward
    let b_y = trunk_h - 6;
    grid.fill_line([curve_x.round() as i32, b_y, curve_z.round() as i32], [15, b_y + 18, 6], 2, bark);
    grid.fill_line([curve_x.round() as i32, b_y, curve_z.round() as i32], [-15, b_y + 16, -5], 2, bark);
    grid.fill_line([curve_x.round() as i32, b_y + 2, curve_z.round() as i32], [-4, b_y + 20, 14], 2, bark);
    grid.fill_line([curve_x.round() as i32, b_y + 2, curve_z.round() as i32], [6, b_y + 18, -14], 2, bark);

    // Central organic canopy ellipsoids
    let cx = curve_x + rng.range(-2.0, 2.0);
    let cy = (trunk_h as f32) + 36.0 + rng.range(-3.0, 3.0);
    let cz = curve_z + rng.range(-2.0, 2.0);

    let rx = rng.range(21.0, 26.0);
    let ry = rng.range(15.0, 20.0);
    let rz = rng.range(21.0, 26.0);

    grid.fill_ellipsoid(cx, cy, cz, rx, ry, rz, orange);
    grid.fill_ellipsoid(cx, cy + 10.0, cz, rx * 0.82, ry * 0.75, rz * 0.82, amber);
    grid.fill_ellipsoid(cx, cy + 20.0, cz, rx * 0.58, ry * 0.50, rz * 0.58, gold);

    // Multiple overlapping asymmetric outer foliage lobes
    let num_lobes = 4;
    for i in 0..num_lobes {
        let ang = (i as f32 / num_lobes as f32) * std::f32::consts::TAU + rng.range(-0.3, 0.3);
        let dist = rng.range(16.0, 24.0);
        let ox = ang.cos() * dist;
        let oz = ang.sin() * dist;
        let oy = rng.range(-6.0, 6.0);

        let lobe_rx = rng.range(13.0, 18.0);
        let lobe_ry = rng.range(10.0, 15.0);
        let lobe_rz = rng.range(13.0, 18.0);

        let lobe_col = if i % 2 == 0 { amber } else { orange };
        grid.fill_ellipsoid(cx + ox, cy + oy, cz + oz, lobe_rx, lobe_ry, lobe_rz, lobe_col);

        // Highlight cap on lobe
        grid.fill_ellipsoid(cx + ox, cy + oy + 6.0, cz + oz, lobe_rx * 0.65, lobe_ry * 0.55, lobe_rz * 0.65, gold);
    }

    grid.build_mesh()
}

pub fn create_voxel_pine_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut grid = MicroVoxelGrid::new(0.06);

    let bark = [0.26, 0.16, 0.08, 1.0];
    let d_green = [0.14, 0.36, 0.16, 1.0];
    let m_green = [0.20, 0.50, 0.22, 1.0];
    let l_green = [0.28, 0.62, 0.28, 1.0];
    let tip_green = [0.35, 0.68, 0.32, 1.0];

    let height_mult = rng.range(0.88, 1.14);
    let trunk_h = (130.0 * height_mult).round() as i32;
    let trunk_r = rng.range(3.0, 4.2);
    let curve_x = rng.range(-1.8, 1.8);
    let curve_z = rng.range(-1.8, 1.8);

    // Tall tapered trunk with slight organic lean
    grid.fill_curved_cylinder_y(0.0, 0.0, 0, trunk_h, trunk_r, curve_x, curve_z, bark);

    // Tapering conical foliage tiers
    let tier_count = 5;
    let start_y = (34.0 * height_mult).round() as i32;
    let end_y = trunk_h;
    let y_step = (end_y - start_y) / tier_count;

    for i in 0..tier_count {
        let t = i as f32 / (tier_count as f32 - 1.0);
        let tier_y = start_y + i * y_step;
        let base_r = (28.0 * (1.0 - t * 0.78) * rng.range(0.85, 1.15)).max(3.0);
        let tier_h = (y_step as f32 * 1.15).round() as i32;

        let frac_h = (tier_y as f32) / (trunk_h as f32);
        let offset_x = curve_x * frac_h * frac_h;
        let offset_z = curve_z * frac_h * frac_h;

        // Under-tier shadow
        grid.fill_ellipsoid(offset_x, tier_y as f32, offset_z, base_r, (tier_h as f32) * 0.45, base_r, d_green);
        // Mid tier body
        grid.fill_ellipsoid(offset_x, (tier_y + 3) as f32, offset_z, base_r * 0.85, (tier_h as f32) * 0.50, base_r * 0.85, m_green);
        // Top tier highlight
        grid.fill_ellipsoid(offset_x, (tier_y + 6) as f32, offset_z, base_r * 0.65, (tier_h as f32) * 0.40, base_r * 0.65, l_green);
    }

    // Needle Spire
    let top_y = trunk_h;
    grid.fill_cylinder_y(curve_x.round() as i32, curve_z.round() as i32, top_y, top_y + 8, 2.0, tip_green);

    grid.build_mesh()
}

pub fn create_voxel_round_tree_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut grid = MicroVoxelGrid::new(0.06);

    let bark = [0.22, 0.16, 0.10, 1.0];
    let shadow = [0.45, 0.32, 0.10, 1.0];
    let gold_mid_base = [0.88, 0.72, 0.18, 1.0];
    let gold_bright_base = [0.96, 0.82, 0.22, 1.0];
    let warm_lime = [0.72, 0.82, 0.18, 1.0];

    let t_blend = rng.range(0.0, 1.0);
    let gold_mid = rng.blend_color(gold_mid_base, warm_lime, t_blend * 0.4);
    let gold_bright = rng.blend_color(gold_bright_base, gold_mid_base, t_blend * 0.3);

    let trunk_h = rng.range(36.0, 48.0).round() as i32;
    let trunk_r = rng.range(3.5, 4.8);
    let curve_x = rng.range(-2.5, 2.5);
    let curve_z = rng.range(-2.5, 2.5);

    // Curved trunk
    grid.fill_curved_cylinder_y(0.0, 0.0, 0, trunk_h, trunk_r, curve_x, curve_z, bark);

    // Base core dome
    let cx = curve_x + rng.range(-1.5, 1.5);
    let cy = (trunk_h as f32) + 16.0;
    let cz = curve_z + rng.range(-1.5, 1.5);

    let rx = rng.range(24.0, 30.0);
    let ry = rng.range(20.0, 25.0);
    let rz = rng.range(24.0, 30.0);

    // Under-canopy shadow
    grid.fill_ellipsoid(cx, cy, cz, rx * 0.95, ry * 0.75, rz * 0.95, shadow);
    // Main voluminous dome
    grid.fill_ellipsoid(cx, cy + 8.0, cz, rx, ry, rz, gold_mid);
    // Upper bright crown
    grid.fill_ellipsoid(cx, cy + 18.0, cz, rx * 0.75, ry * 0.70, rz * 0.75, gold_bright);

    // Overlapping cloud-like puffy lobes around the perimeter
    let num_lobes = 4;
    for i in 0..num_lobes {
        let ang = (i as f32 / num_lobes as f32) * std::f32::consts::TAU + rng.range(-0.35, 0.35);
        let dist = rng.range(12.0, 18.0);
        let lx = cx + ang.cos() * dist;
        let lz = cz + ang.sin() * dist;
        let ly = cy + rng.range(2.0, 12.0);

        let lrx = rng.range(12.0, 16.0);
        let lry = rng.range(10.0, 14.0);
        let lrz = rng.range(12.0, 16.0);

        grid.fill_ellipsoid(lx, ly, lz, lrx, lry, lrz, gold_mid);
        grid.fill_ellipsoid(lx, ly + 4.0, lz, lrx * 0.7, lry * 0.6, lrz * 0.7, gold_bright);
    }

    grid.build_mesh()
}

pub fn create_voxel_fallen_log_mesh(seed: u64) -> Mesh {
    let mut rng = Prng::new(seed);
    let mut grid = MicroVoxelGrid::new(0.045);

    let bark = [0.38, 0.24, 0.14, 1.0];
    let wood_interior = [0.65, 0.50, 0.32, 1.0];
    let moss = [0.24, 0.48, 0.18, 1.0];

    let length = rng.range(40.0, 60.0).round() as i32;
    let radius = rng.range(4.5, 6.5);
    let curve_z = rng.range(-3.5, 3.5);

    for x in -length..=length {
        let t = (x + length) as f32 / (2.0 * length as f32);
        let cz = (curve_z * (4.0 * t * (1.0 - t))).round() as i32;
        let r = radius * (1.0 - (t - 0.5).abs() * 0.18);
        let r_ceil = r.ceil() as i32;

        for dy in -r_ceil..=r_ceil {
            for dz in -r_ceil..=r_ceil {
                if (dy as f32 * dy as f32 + dz as f32 * dz as f32) <= r * r {
                    let col = if (x == -length || x == length) && dy.abs() < (r_ceil - 1) && dz.abs() < (r_ceil - 1) {
                        wood_interior
                    } else if dy > (r * 0.4) as i32 && rng.range(0.0, 1.0) < 0.65 {
                        moss
                    } else {
                        bark
                    };
                    grid.set(x, dy + r_ceil, cz + dz, col);
                }
            }
        }
    }

    grid.build_mesh()
}

/// Cache pre-generating 6 unique procedural variants per tree species.
#[derive(Clone)]
pub struct TreeMeshCache {
    pub dead_tree_variants: Vec<Handle<Mesh>>,
    pub oak_tree_variants: Vec<Handle<Mesh>>,
    pub pine_tree_variants: Vec<Handle<Mesh>>,
    pub round_tree_variants: Vec<Handle<Mesh>>,
}

impl TreeMeshCache {
    pub fn new(meshes: &mut Assets<Mesh>) -> Self {
        let mut dead = Vec::with_capacity(6);
        let mut oak = Vec::with_capacity(6);
        let mut pine = Vec::with_capacity(6);
        let mut round = Vec::with_capacity(6);

        for i in 0..6 {
            let seed_dead = 1010 + i as u64 * 37 + 7;
            let seed_oak = 2020 + i as u64 * 41 + 13;
            let seed_pine = 3030 + i as u64 * 53 + 19;
            let seed_round = 4040 + i as u64 * 67 + 23;

            dead.push(meshes.add(create_voxel_dead_tree_mesh(seed_dead)));
            oak.push(meshes.add(create_voxel_oak_mesh(seed_oak)));
            pine.push(meshes.add(create_voxel_pine_mesh(seed_pine)));
            round.push(meshes.add(create_voxel_round_tree_mesh(seed_round)));
        }

        Self {
            dead_tree_variants: dead,
            oak_tree_variants: oak,
            pine_tree_variants: pine,
            round_tree_variants: round,
        }
    }
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
    pub tree_cache: TreeMeshCache,
    #[allow(dead_code)] pub dead_tree: Handle<Mesh>,
    #[allow(dead_code)] pub oak_tree: Handle<Mesh>,
    #[allow(dead_code)] pub pine_tree: Handle<Mesh>,
    #[allow(dead_code)] pub round_tree: Handle<Mesh>,
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
    pub fallen_log: Handle<Mesh>,
}

impl CachedModelMeshes {
    pub fn new(meshes: &mut Assets<Mesh>) -> Self {
        let tree_cache = TreeMeshCache::new(meshes);
        let dead_tree = tree_cache.dead_tree_variants[0].clone();
        let oak_tree = tree_cache.oak_tree_variants[0].clone();
        let pine_tree = tree_cache.pine_tree_variants[0].clone();
        let round_tree = tree_cache.round_tree_variants[0].clone();
        let fallen_log = meshes.add(create_voxel_fallen_log_mesh(5050));

        Self {
            tree_cache,
            dead_tree,
            oak_tree,
            pine_tree,
            round_tree,
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
            fallen_log,
        }
    }
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
                "SELECT * FROM active_projectile".to_string(),
                "SELECT * FROM equipment_loadout".to_string(),
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
                    start: 82.5,
                    end: 165.0,
                },
                ..default()
            },
            RenderLayers::from_layers(&[0, 2]),
            RtsCameraChild,
            crate::binary_sky::AtmosphericCamera,
        ));
    });

    let spawn_x = 0.0;
    let spawn_z = 0.0;
    let spawn_y = crate::terrain::get_terrain_height(spawn_x, spawn_z) + 1.05;

    let mut player_entity_commands = commands.spawn((
        SpatialBundle::from_transform(BevyTransform::from_xyz(spawn_x, spawn_y, spawn_z)),
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
        LogicalPosition(Vec3::new(spawn_x, spawn_y, spawn_z)),
        LogicalRotation(Quat::IDENTITY),
        crate::components::Faction::Player, 
        Selectable, 
        crate::prediction::InputBuffer::default(),
        crate::prediction::AuthoritativeState {
            position: Vec3::new(spawn_x, spawn_y, spawn_z),
            last_processed_tick: 0,
        },
        crate::prediction::LocalMovementTracker { last_position: Vec3::new(spawn_x, spawn_y, spawn_z) },
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
            NotShadowCaster,
        ));

        parent.spawn((
            Camera3dBundle { 
                transform: BevyTransform::from_xyz(0.0, 0.5, 0.0), 
                camera: Camera { is_active: true, ..default() },
                ..default() 
            }, 
            PlayerHead, FpsCamera,
            bevy_voxel_world::prelude::VoxelWorldCamera::<crate::terrain::ProceduralTerrainConfig>::default(),
            FogSettings {
                color: sky_fog_color,
                falloff: FogFalloff::Linear {
                    start: 52.5,
                    end: 97.5,
                },
                ..default()
            },
            RenderLayers::from_layers(&[0, 1]),
            crate::binary_sky::AtmosphericCamera,
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
                NotShadowCaster,
            ));
        });
    });

    // Architectural Note: Directional lighting is dynamically governed by `BinarySkyPlugin`
    // with dual-source radiative transfer (Host Star A and Companion Star B) and adaptive
    // shadow cascade priority management.
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

pub fn sync_transforms(
    mut commands: Commands, 
    conn: Res<SpacetimeConnection>, 
    mut query: Query<(Entity, &NetworkEntity, &mut LogicalPosition, &mut LogicalRotation)>,
    peasant_query: Query<&PeasantUnit>,
    mut player_query: Query<&mut crate::prediction::AuthoritativeState, With<PlayerBody>>,
    player_body_query: Query<&BevyTransform, With<PlayerBody>>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned_ids: Local<BTreeSet<u64>>,
    mut model_cache: Local<Option<CachedModelMeshes>>,
) {
    let _ = conn.db.frame_tick();
    
    let my_entity_id = conn.identity.as_ref()
        .and_then(|id| conn.db.db.player().identity().find(id))
        .map(|p| p.entity_id);

    let player_pos = player_body_query.get_single().map(|t| t.translation).unwrap_or(Vec3::ZERO);

    const CREATURE_LOAD_RADIUS_SQ: f32 = 112.5 * 112.5;
    const CREATURE_UNLOAD_RADIUS_SQ: f32 = 120.0 * 120.0;

    let cache = model_cache.get_or_insert_with(|| CachedModelMeshes::new(&mut meshes));

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
        } else if let Some(ref brain) = npc_brain {
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

        // P1 Fix: Determine NPC type name for debuggability before spawn
        let npc_type_name = if is_pet {
            "Pet"
        } else if let Some(ref brain) = npc_brain {
            match brain.ai_type {
                crate::module_bindings::AiType::Boar => "Boar",
                crate::module_bindings::AiType::Deer => "Deer",
                crate::module_bindings::AiType::Goblin => "Goblin",
                crate::module_bindings::AiType::Friendly | crate::module_bindings::AiType::Peasant => "Peasant",
            }
        } else if is_peasant {
            "Peasant"
        } else {
            "NPC"
        };

        let mut entity_cmds = commands.spawn((
            // P1 Fix: Add Name component for debuggability
            Name::new(format!("NPC_{}_{}", npc_type_name, id)),
            // P1 Fix: Add StateScoped for automatic cleanup on state exit
            StateScoped(GameState::InGame),
            NetworkEntity(id),
            SpatialBundle::from_transform(BevyTransform::from_xyz(db_t.x, db_t.y, db_t.z)),
            LogicalPosition(Vec3::new(db_t.x, db_t.y, db_t.z)),
            LogicalRotation(Quat::IDENTITY),
            Selectable, 
            RigidBody::Kinematic, 
            root_collider,
            CollisionLayers::new([GameLayer::Unit], [GameLayer::Default]),
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
                NotShadowCaster,
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
    tree_mats: Option<Res<crate::tree_colors::TreeMaterialHandles>>,
    mut default_node_mat: Local<Option<Handle<StandardMaterial>>>,
    mut local_nodes: Local<BTreeSet<u64>>,
    mut scan_timer: Local<Option<Timer>>,
    mut model_cache: Local<Option<CachedModelMeshes>>,
) {
    let Ok(player_transform) = player_query.get_single() else { return; };
    let player_pos = player_transform.translation;

    let timer = scan_timer.get_or_insert_with(|| Timer::from_seconds(0.1, TimerMode::Repeating));
    if !timer.tick(time.delta()).just_finished() {
        return;
    }

    let cache = model_cache.get_or_insert_with(|| CachedModelMeshes::new(&mut meshes));

    let node_mat = default_node_mat.get_or_insert_with(|| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.1,
            ..default()
        })
    }).clone();

    const NODE_LOAD_RADIUS_SQ: f32 = 63.0 * 63.0;
    const NODE_UNLOAD_RADIUS_SQ: f32 = 69.0 * 69.0;

    local_nodes.clear();

    for (entity, node_item, transform) in node_query.iter() {
        let origin = transform.translation;
        let dist_sq = (origin.x - player_pos.x).powi(2) + (origin.z - player_pos.z).powi(2);

        let server_node = conn.db.db.resource_node().node_id().find(&node_item.node_id);

        if server_node.is_none() {
            if dist_sq <= NODE_UNLOAD_RADIUS_SQ {
                let clean_item_type = node_item.node_type.trim();
                match clean_item_type {
                    "Tree" | "Oak" | "Pine" | "Dead" | "Round" | _ if clean_item_type.starts_with("Tree") => {
                        let mut seed = (origin.x.abs() * 1000.0 + origin.z.abs() * 100.0) as u64;
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        let angle_rand = ((seed as f32) / (u32::MAX as f32)) * std::f32::consts::TAU;
                        let fall_dir = Vec3::new(angle_rand.cos(), 0.0, angle_rand.sin()).normalize();

                        let biome = crate::tree_colors::get_biome(origin.y);
                        let tree_style = if clean_item_type == "Oak" || clean_item_type.ends_with(":Oak") {
                            1
                        } else if clean_item_type == "Pine" || clean_item_type.ends_with(":Pine") {
                            2
                        } else if clean_item_type == "Dead" || clean_item_type.ends_with(":Dead") {
                            0
                        } else if clean_item_type == "Round" || clean_item_type.ends_with(":Round") {
                            3
                        } else {
                            let mut species_seed = (node_item.node_id ^ 0x517cc1b727220a95) as u64;
                            match crate::tree_colors::pick_tree_type(biome, &mut species_seed) {
                                "Dead" => 0,
                                "Oak" => 1,
                                "Pine" => 2,
                                _ => 3,
                            }
                        };

                        let variant_idx = (node_item.node_id % 6) as usize;
                        let tree_mesh = match tree_style {
                            0 => cache.tree_cache.dead_tree_variants[variant_idx].clone(),
                            1 => cache.tree_cache.oak_tree_variants[variant_idx].clone(),
                            2 => cache.tree_cache.pine_tree_variants[variant_idx].clone(),
                            _ => cache.tree_cache.round_tree_variants[variant_idx].clone(),
                        };

                        let tree_mat = if let Some(ref mats) = tree_mats {
                            match tree_style {
                                0 => mats.dead.clone(),
                                1 => mats.oak.clone(),
                                2 => mats.pine.clone(),
                                _ => mats.round.clone(),
                            }
                        } else {
                            node_mat.clone()
                        };

                        commands.spawn((
                            PbrBundle {
                                mesh: tree_mesh,
                                material: tree_mat,
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

        let (mesh, collider, y_offset, tree_comp_opt) = match clean_type {
            "Tree" | "Oak" | "Pine" | "Dead" | "Round" | _ if clean_type.starts_with("Tree") => {
                let biome = crate::tree_colors::get_biome(node.y);
                let tree_style = if clean_type == "Oak" || clean_type.ends_with(":Oak") {
                    1
                } else if clean_type == "Pine" || clean_type.ends_with(":Pine") {
                    2
                } else if clean_type == "Dead" || clean_type.ends_with(":Dead") {
                    0
                } else if clean_type == "Round" || clean_type.ends_with(":Round") {
                    3
                } else {
                    let mut species_seed = (node.node_id ^ 0x517cc1b727220a95) as u64;
                    match crate::tree_colors::pick_tree_type(biome, &mut species_seed) {
                        "Dead" => 0,
                        "Oak" => 1,
                        "Pine" => 2,
                        _ => 3,
                    }
                };

                let variant_idx = (node.node_id % 6) as usize;
                let tree_mesh = match tree_style {
                    0 => cache.tree_cache.dead_tree_variants[variant_idx].clone(),
                    1 => cache.tree_cache.oak_tree_variants[variant_idx].clone(),
                    2 => cache.tree_cache.pine_tree_variants[variant_idx].clone(),
                    _ => cache.tree_cache.round_tree_variants[variant_idx].clone(),
                };
                (
                    tree_mesh,
                    Collider::cylinder(0.40, 8.5),
                    0.0,
                    Some(crate::components::TreeComponent { species: tree_style, variant: variant_idx }),
                )
            }
            "FallenLog" => (
                cache.fallen_log.clone(),
                Collider::cylinder(0.35, 3.2),
                0.35,
                None,
            ),
            "Rock" => (
                cache.rock.clone(),
                Collider::cuboid(1.5, 1.4, 1.4),
                0.0,
                None,
            ),
            "Bush" => (
                cache.bush.clone(),
                Collider::sphere(0.85),
                0.0,
                None,
            ),
            "Branch" => (
                cache.branch.clone(),
                Collider::cuboid(1.5, 0.28, 0.9),
                0.02,
                None,
            ),
            "Flint" => (
                cache.flint.clone(),
                Collider::cuboid(0.55, 0.75, 0.45),
                0.02,
                None,
            ),
            "LooseStone" => (
                cache.stone.clone(),
                Collider::cuboid(1.2, 0.58, 0.95),
                0.02,
                None,
            ),
            _ => (
                cache.stone.clone(),
                Collider::cuboid(0.5, 0.5, 0.5),
                0.0,
                None,
            )
        };

        let material = if let Some(ref tc) = tree_comp_opt {
            if let Some(ref mats) = tree_mats {
                match tc.species {
                    0 => mats.dead.clone(),
                    1 => mats.oak.clone(),
                    2 => mats.pine.clone(),
                    _ => mats.round.clone(),
                }
            } else {
                node_mat.clone()
            }
        } else {
            node_mat.clone()
        };

        let mut entity_cmd = commands.spawn((
            PbrBundle {
                mesh, 
                material,
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

        if let Some(tc) = tree_comp_opt {
            entity_cmd.insert(tc);
        }

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
    audio_handles: Option<Res<CombatAudioHandles>>,
    mut hit_marker_state: Option<ResMut<HitMarkerState>>,
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
                    crate::tuner::spawn_comic_damage_floater(&mut commands, pos, "KABOOM!", true);
                }
                _ => {
                    let event_str = event.event_type.as_str();

                    // Auditory Cues & Reticle-Adjacent Hit Confirmation
                    if event_str.starts_with("Crit") {
                        if let Some(ref handles) = audio_handles {
                            crate::audio_feedback::play_sound(&mut commands, &handles.dink);
                        }
                        if let Some(ref mut hm) = hit_marker_state {
                            hm.timer = Timer::from_seconds(0.09, TimerMode::Once);
                            hm.is_crit = true;
                            hm.is_armor = false;
                        }
                        crate::tuner::spawn_comic_damage_floater(&mut commands, pos, "CRIT! 160", true);
                    } else if event_str.contains("Clang") || event_str.contains("Armor") {
                        if let Some(ref handles) = audio_handles {
                            crate::audio_feedback::play_sound(&mut commands, &handles.armor_break);
                        }
                        if let Some(ref mut hm) = hit_marker_state {
                            hm.timer = Timer::from_seconds(0.09, TimerMode::Once);
                            hm.is_crit = false;
                            hm.is_armor = true;
                        }
                        crate::tuner::spawn_comic_damage_floater(&mut commands, pos, "CLANG!", false);
                    } else if event_str == "HitPlayer" {
                        if let Some(ref handles) = audio_handles {
                            crate::audio_feedback::play_sound(&mut commands, &handles.bodyshot_tick);
                        }
                        if let Some(ref mut hm) = hit_marker_state {
                            hm.timer = Timer::from_seconds(0.07, TimerMode::Once);
                            hm.is_crit = false;
                            hm.is_armor = false;
                        }
                        crate::tuner::spawn_comic_damage_floater(&mut commands, pos, "-35", false);
                    } else if event_str.contains("Bonk") {
                        crate::tuner::spawn_comic_damage_floater(&mut commands, pos, "BONK!", false);
                    }

                    let color = match event_str {
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
                            RigidBody::Kinematic,
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

// ----------------------------------------------------------------------------
// AUTHORITATIVE PROJECTILE RENDERING LOOP
// ----------------------------------------------------------------------------

pub fn sync_active_projectiles(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut existing_projectiles: Query<(Entity, &NetworkProjectile, &mut BevyTransform)>,
) {
    let db_projectiles: Vec<_> = conn.db.db.active_projectile().iter().collect();
    let mut current_ids = BTreeSet::new();

    let mut entity_map = BTreeMap::new();
    for (entity, net_proj, transform) in existing_projectiles.iter_mut() {
        entity_map.insert(net_proj.0, (entity, transform));
    }

    for p in &db_projectiles {
        current_ids.insert(p.projectile_id);
        let vel = Vec3::new(p.vel_x, p.vel_y, p.vel_z);
        let pos = Vec3::new(p.pos_x, p.pos_y, p.pos_z);
        let rot = if vel.length_squared() > 0.001 {
            BevyTransform::from_xyz(pos.x, pos.y, pos.z).looking_to(vel.normalize(), Vec3::Y).rotation
        } else {
            Quat::IDENTITY
        };

        if let Some((_, ref mut transform)) = entity_map.get_mut(&p.projectile_id) {
            transform.translation = pos;
            transform.rotation = rot;
        } else {
            spawn_projectile_entity(&mut commands, &mut meshes, &mut materials, p.projectile_id, p.kind, pos, rot);
        }
    }

    for (entity, net_proj, _) in existing_projectiles.iter() {
        if !current_ids.contains(&net_proj.0) {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn spawn_projectile_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    projectile_id: u64,
    kind: ProjectileKind,
    pos: Vec3,
    rot: Quat,
) {
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.38, 0.22),
        perceptual_roughness: 0.8,
        ..default()
    });
    let iron_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.36, 0.38),
        metallic: 0.8,
        perceptual_roughness: 0.35,
        ..default()
    });
    let stone_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.48, 0.46, 0.44),
        perceptual_roughness: 0.9,
        ..default()
    });
    let brass_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.75, 0.25),
        metallic: 0.85,
        perceptual_roughness: 0.3,
        ..default()
    });
    let arcane_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.8, 1.0),
        unlit: true,
        ..default()
    });
    let fire_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.35, 0.05),
        unlit: true,
        ..default()
    });
    let red_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.15, 0.15),
        ..default()
    });

    let mut parent = commands.spawn((
        SpatialBundle {
            transform: BevyTransform::from_translation(pos).with_rotation(rot),
            ..default()
        },
        NetworkProjectile(projectile_id),
    ));

    parent.with_children(|builder| {
        match kind {
            ProjectileKind::Arrow => {
                // Shaft
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.015, 0.015, 0.52)),
                    material: wood_mat.clone(),
                    ..default()
                });
                // Flint arrowhead
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.03, 0.01, 0.05)),
                    material: iron_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, -0.27),
                    ..default()
                });
                // Red fletching fins
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.008, 0.04, 0.07)),
                    material: red_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, 0.22),
                    ..default()
                });
            }
            ProjectileKind::HandCrossbowBolt => {
                // Compact bolt body
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.018, 0.018, 0.26)),
                    material: iron_mat.clone(),
                    ..default()
                });
                // Piercing steel point
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.025, 0.025, 0.04)),
                    material: brass_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, -0.14),
                    ..default()
                });
            }
            ProjectileKind::RevolverBullet => {
                // Brass bullet body & copper tip
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cylinder::new(0.015, 0.06)),
                    material: brass_mat.clone(),
                    transform: BevyTransform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ..default()
                });
            }
            ProjectileKind::ShotgunPellet => {
                // Concentrated lead buckshot pellet
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.02)),
                    material: iron_mat.clone(),
                    ..default()
                });
            }
            ProjectileKind::SniperBullet => {
                // High velocity tungsten penetrator
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cylinder::new(0.014, 0.12)),
                    material: iron_mat.clone(),
                    transform: BevyTransform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ..default()
                });
            }
            ProjectileKind::MagicMissile => {
                // Glowing arcane crystal dart
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.04, 0.04, 0.16)),
                    material: arcane_mat.clone(),
                    ..default()
                });
                // Pulsing energy halo
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.06)),
                    material: arcane_mat.clone(),
                    ..default()
                });
            }
            ProjectileKind::FireballBall => {
                // Blazing outer fireball
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.22)),
                    material: fire_mat.clone(),
                    ..default()
                });
                // Inner core
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.12)),
                    material: brass_mat.clone(),
                    ..default()
                });
            }
            ProjectileKind::CatapultRock => {
                // Rough-hewn chiseled stone boulder
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.55, 0.48, 0.52)),
                    material: stone_mat.clone(),
                    ..default()
                });
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.38, 0.38, 0.38)),
                    material: stone_mat.clone(),
                    transform: BevyTransform::from_xyz(0.1, -0.05, 0.1),
                    ..default()
                });
            }
            ProjectileKind::TrebuchetShell => {
                // Giant fortified iron-banded stone shell
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.38)),
                    material: stone_mat.clone(),
                    ..default()
                });
                // Reinforcing iron hoops
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.78, 0.06, 0.78)),
                    material: iron_mat.clone(),
                    ..default()
                });
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.06, 0.78, 0.78)),
                    material: iron_mat.clone(),
                    ..default()
                });
            }
            ProjectileKind::BallistaSpear => {
                // Massive siege harpoon
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.045, 0.045, 1.4)),
                    material: wood_mat.clone(),
                    ..default()
                });
                // Barbed iron spearhead
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.08, 0.025, 0.22)),
                    material: iron_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, -0.75),
                    ..default()
                });
                // Crossbar stabilizer fins
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.18, 0.015, 0.12)),
                    material: red_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, 0.55),
                    ..default()
                });
            }
        }
    });
}