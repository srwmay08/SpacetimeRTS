// ============================================================================
// File: client/src/binary_sky/meshes.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL CELESTIAL MESH GENERATION (STARFIELD, SKY DOME, PRECIPITATION)
// ----------------------------------------------------------------------------

use std::f32::consts::PI;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

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

        let up = if dir.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
        let right = dir.cross(up).normalize();
        let star_up = right.cross(dir).normalize();

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

        let is_guide_star = u3 > 0.97;
        let is_nav_star = u3 > 0.82;

        let (cutoff_y, full_y) = if is_guide_star {
            (0.015, 0.075)
        } else if is_nav_star {
            (0.025, 0.110)
        } else {
            (0.040, 0.145)
        };

        let horizon_factor = ((y - cutoff_y) / (full_y - cutoff_y)).clamp(0.0, 1.0);
        let extinction = horizon_factor * (2.0 - horizon_factor);

        let ext_sq = extinction * extinction;
        c[0] *= extinction;
        c[1] *= extinction * 0.98;
        c[2] *= ext_sq;
        c[3] *= extinction;

        let half = size * 0.5;
        let rot_angle = u1 * (2.0 * PI);
        let cos_r = rot_angle.cos();
        let sin_r = rot_angle.sin();
        let axis_a = (right * cos_r + star_up * sin_r).normalize();
        let axis_b = (-right * sin_r + star_up * cos_r).normalize();

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
            normals.push([-cos_phi * sin_theta, -sin_phi, cos_phi * cos_theta]);
            uvs.push([s as f32 / sectors as f32, r as f32 / rings as f32]);
            colors.push([0.15, 0.40, 0.85, 1.0]);
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
        let dist = 1.0 + u2.sqrt() * 15.0;
        let x = angle.cos() * dist;
        let z = angle.sin() * dist;
        let y = -2.0 + u3 * 24.0;
        let len = 0.65 + u4 * 0.45;

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
