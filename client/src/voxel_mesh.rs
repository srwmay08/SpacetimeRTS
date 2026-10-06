// ============================================================================
// File: client/src/voxel_mesh.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL VOXEL DATA STRUCTURES, PRNG & EXPOSED-FACE EXTRACTOR
// ----------------------------------------------------------------------------
// Architectural Note:
// Encapsulates voxel mesh generation primitives (VoxelBox, MicroVoxelGrid) and
// deterministic PRNG utilities. Used across building, creature, and prop systems.
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::collections::BTreeMap;

// ----------------------------------------------------------------------------
// SEEDED DETERMINISTIC PRNG UTILITY (RE-EXPORT FROM CRATE::PRNG)
// ----------------------------------------------------------------------------

pub use crate::prng::Prng;

// ----------------------------------------------------------------------------
// COMPOSITE BUILDING BOX MESH BUILDER (RE-EXPORT FROM CRATE::BUILDING)
// ----------------------------------------------------------------------------

pub use crate::building::{BuildingBox, VoxelBox, build_building_mesh, build_voxel_mesh};

// ----------------------------------------------------------------------------
// MICRO-VOXEL RASTERIZER & EXPOSED-FACE EXTRACTOR
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
        y_min: i32, y_max: i32,
        radius: f32,
        curve_dx: f32, curve_dz: f32,
        color: [f32; 4],
    ) {
        let total_h = (y_max - y_min).max(1) as f32;
        for y in y_min..=y_max {
            let frac = (y - y_min) as f32 / total_h;
            let cx = x0 + curve_dx * frac * frac;
            let cz = z0 + curve_dz * frac * frac;
            let r_curr = (radius * (1.0 - frac * 0.45)).max(1.5);
            let r_sq = r_curr * r_curr;
            let r_ceil = r_curr.ceil() as i32;

            let icx = cx.round() as i32;
            let icz = cz.round() as i32;

            for dx in -r_ceil..=r_ceil {
                for dz in -r_ceil..=r_ceil {
                    if (dx as f32 * dx as f32 + dz as f32 * dz as f32) <= r_sq {
                        self.voxels.insert((icx + dx, y, icz + dz), color);
                    }
                }
            }
        }
    }

    pub fn build_mesh(&self) -> Mesh {
        let p = self.pitch;
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut colors: Vec<[f32; 4]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

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
