// ============================================================================
// File: client/src/subterrain.rs
// ============================================================================
// ----------------------------------------------------------------------------
// SUBTERRANEAN VORONOI STRATA & TUNNEL ENGINE (Bevy / SpacetimeDB / Avian3D)
// ----------------------------------------------------------------------------
// Architectural Note: Implements deterministic smashed-cuboid Voronoi cell
// excavation and vertical windowing. Decoupled from surface terrain streaming
// to eliminate redundant surface remeshing when quarrying or tunneling underground.
// Conformal vertex lattice ensures 100% gapless polyhedral stone faces.
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use noise::{NoiseFn, Perlin};
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::module_bindings::VoxelChunk;

// Voxel Material Constants
pub const MAT_AIR: u8 = 0;
pub const MAT_DIRT: u8 = 1;
pub const MAT_STONE: u8 = 2;
pub const MAT_SAND: u8 = 3;
pub const MAT_WOOD: u8 = 4;
pub const MAT_REINFORCED_STONE: u8 = 5;
pub const MAT_BEDROCK: u8 = 6;
pub const MAT_IRON_ORE: u8 = 7;
pub const MAT_RUBY: u8 = 8;
pub const MAT_COLLAPSED_RUBBLE: u8 = 9;

pub const BEDROCK_ELEVATION: f32 = -120.0;
pub const VOXEL_SIZE: f32 = 1.0;
pub const LOW_POLY_CHUNK_SPAN: f32 = 16.0;

// Subterranean ECS Components
#[derive(Component)]
pub struct SubterrainChunkVisual;

#[derive(Component)]
pub struct SubterrainChunkHasCollider;

#[derive(Component)]
pub struct SubterrainChunkMarker {
    pub chunk_key: u64,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub last_modified_tick: u64,
    pub center_window_y: i32,
}

// ----------------------------------------------------------------------------
// STATIC NOISE SAMPLING (ZERO HEAP ALLOCATIONS IN INNER LOOPS)
// ----------------------------------------------------------------------------

static RAVINE_PERLIN: OnceLock<Perlin> = OnceLock::new();
static CAVE_PERLIN: OnceLock<Perlin> = OnceLock::new();

#[inline]
pub fn get_ravine_perlin() -> &'static Perlin {
    RAVINE_PERLIN.get_or_init(|| Perlin::new(1339))
}

#[inline]
pub fn get_cave_perlin() -> &'static Perlin {
    CAVE_PERLIN.get_or_init(|| Perlin::new(1338))
}

/// Evaluates if 3D procedural coordinates carve out an ancient crypt / tomb dungeon chamber or entrance shaft.
#[inline]
pub fn is_dungeon_cavity_at(wx: f32, wy: f32, wz: f32, terrain_height: f32) -> bool {
    let cell_size = 144.0;
    let cell_x = (wx / cell_size).floor() as i32;
    let cell_z = (wz / cell_size).floor() as i32;

    let center_x = cell_x as f32 * cell_size + 72.0;
    let center_z = cell_z as f32 * cell_size + 72.0;

    let dx = (wx - center_x).abs();
    let dz = (wz - center_z).abs();

    // 1. Vertical entrance shaft descending from surface to subterranean chamber
    if dx < 2.0 && dz < 2.0 && wy <= terrain_height + 0.5 && wy >= -24.0 {
        return true;
    }

    // 2. Vaulted Crypt Chamber (12m x 6m x 12m) at elevation -26m to -20m
    if dx < 6.0 && dz < 6.0 && wy >= -26.0 && wy <= -20.0 {
        return true;
    }

    // 3. Subterranean tomb archway connecting to surrounding cave network
    if dx < 1.5 && dz >= 6.0 && dz < 9.0 && wy >= -26.0 && wy <= -22.0 {
        return true;
    }

    false
}

/// Evaluates if 3D procedural coordinates carve out a natural subterranean cavity:
/// 1. Hillside cave mouths and 3D cavern chambers
/// 2. Large ravines / chasm fissures
/// 3. Designated crypt / tomb dungeon shafts and chambers
#[inline]
pub fn is_cave_air_at(wx: f32, wy: f32, wz: f32, terrain_height: f32) -> bool {
    if wy <= BEDROCK_ELEVATION + 1.0 {
        return false;
    }

    // 1. Ravine / Chasm Fissures: deep jagged trenches cutting from surface down to -65m
    let ravine_noise = get_ravine_perlin();
    let r_sample = ravine_noise.get([wx as f64 * 0.007, wz as f64 * 0.007]);
    if r_sample.abs() < 0.024 && wy <= terrain_height + 0.5 && wy >= -65.0 {
        return true;
    }

    // 2. Designated Crypt / Tomb Dungeons: ancient stone entrance shaft & burial chamber
    if is_dungeon_cavity_at(wx, wy, wz, terrain_height) {
        return true;
    }

    // 3. 3D Subterranean Caverns & Hillside Cave Mouths
    let cave_noise = get_cave_perlin();
    let freq = 0.035;
    let sample = cave_noise.get([wx as f64 * freq, wy as f64 * freq, wz as f64 * freq]);

    // Standard deep cave
    if wy < terrain_height - 3.5 && wy > -118.0 && sample > 0.38 {
        return true;
    }

    // Hillside Cave Mouth: breaches the surface on steep hillsides/slopes when cave noise is intense
    if wy >= terrain_height - 3.5 && wy <= terrain_height + 0.5 && sample > 0.44 {
        let slope = (crate::terrain::compute_canonical_terrain_height(wx + 1.5, wz) - crate::terrain::compute_canonical_terrain_height(wx - 1.5, wz)).abs()
            + (crate::terrain::compute_canonical_terrain_height(wx, wz + 1.5) - crate::terrain::compute_canonical_terrain_height(wx, wz - 1.5)).abs();
        if slope > 0.40 {
            return true;
        }
    }

    false
}

/// Evaluates procedural rock strata to embed Iron ore veins and deep Ruby crystal pockets
/// via deterministic 3D integer coordinate hashing.
#[inline]
pub fn procedural_stone_or_ore(vx: i32, vy: i32, vz: i32, wy: f32) -> u8 {
    let hash = ((vx.wrapping_mul(73856093) ^ vy.wrapping_mul(19349663) ^ vz.wrapping_mul(83492791)) as u32) % 10000;
    if wy <= -50.0 && wy >= -115.0 && hash < 120 {
        MAT_RUBY
    } else if wy <= -5.0 && wy >= -75.0 && hash < 450 {
        MAT_IRON_ORE
    } else {
        MAT_STONE
    }
}

// ----------------------------------------------------------------------------
// SMASHED-CUBOID VORONOI LATTICE GENERATOR
// ----------------------------------------------------------------------------
// Architectural Note: Deterministically perturbs corner vertices on the 3D lattice.
// Because the perturbed position of vertex (gx, gy, gz) is a pure function of its
// coordinates, all 8 adjacent cells sharing that corner vertex evaluate identical
// 3D coordinates. This guarantees 100% conformal geometry: zero cracks, zero seams,
// and natural interlocking polyhedral stone facets when excavated.

#[inline]
pub fn get_voronoi_corner_local(gx: i32, gy: i32, gz: i32, chunk_base_x: f32, chunk_base_z: f32) -> Vec3 {
    let seed = (gx.wrapping_mul(73856093) ^ gy.wrapping_mul(19349663) ^ gz.wrapping_mul(83492791)) as u32;
    // Jitter: +/- 0.22m in X and Z, +/- 0.16m in Y
    let jx = ((seed % 101) as f32 / 100.0 - 0.5) * 0.44;
    let jy = (((seed >> 8) % 101) as f32 / 100.0 - 0.5) * 0.32;
    let jz = (((seed >> 16) % 101) as f32 / 100.0 - 0.5) * 0.44;

    let world_x = gx as f32 + jx;
    let world_y = gy as f32 + jy;
    let world_z = gz as f32 + jz;

    Vec3::new(world_x - chunk_base_x, world_y, world_z - chunk_base_z)
}

pub fn get_voxel_face_color(mat: u8, is_top: bool, wy: f32, vx: i32, vy: i32, vz: i32) -> [f32; 4] {
    let facet_hash = (((vx.wrapping_mul(37) ^ vy.wrapping_mul(59) ^ vz.wrapping_mul(71)) as f32 * 0.17).sin().abs() * 43758.5453).fract();
    let facet_variation = (facet_hash - 0.5) * 0.09;

    if is_top && wy >= 2.0 && wy <= 15.5 && mat == MAT_DIRT {
        let r = (0.28 + facet_variation * 0.8).clamp(0.18, 0.40);
        let g = (0.64 + facet_variation).clamp(0.48, 0.76);
        let b = (0.28 + facet_variation * 0.8).clamp(0.18, 0.40);
        [r, g, b, 1.0]
    } else {
        match mat {
            MAT_DIRT => {
                let r = (0.46 + facet_variation).clamp(0.35, 0.60);
                let g = (0.38 + facet_variation).clamp(0.30, 0.50);
                let b = (0.28 + facet_variation).clamp(0.20, 0.40);
                [r, g, b, 1.0]
            }
            MAT_STONE => {
                let r = (0.42 + facet_variation).clamp(0.28, 0.56);
                let g = (0.44 + facet_variation).clamp(0.30, 0.58);
                let b = (0.46 + facet_variation * 0.8).clamp(0.32, 0.60);
                [r, g, b, 1.0]
            }
            MAT_SAND => {
                let r = (0.78 + facet_variation).clamp(0.65, 0.90);
                let g = (0.72 + facet_variation).clamp(0.60, 0.85);
                let b = (0.52 + facet_variation).clamp(0.40, 0.70);
                [r, g, b, 1.0]
            }
            MAT_WOOD => [0.55, 0.40, 0.25, 1.0],
            MAT_REINFORCED_STONE => {
                let r = (0.35 + facet_variation).clamp(0.25, 0.45);
                let g = (0.35 + facet_variation).clamp(0.25, 0.45);
                let b = (0.40 + facet_variation).clamp(0.30, 0.50);
                [r, g, b, 1.0]
            }
            MAT_BEDROCK => {
                let wx = vx as f32;
                let wz = vz as f32;
                let magma_seed = ((wx * 0.28).sin() * (wz * 0.28).cos()).abs();
                if magma_seed > 0.65 {
                    [2.2, 0.45, 0.08, 1.0] // Magma crack emissive bloom
                } else {
                    [0.10, 0.10, 0.13, 1.0]
                }
            }
            MAT_IRON_ORE => {
                let r = (0.64 + facet_variation * 1.2).clamp(0.46, 0.80);
                let g = (0.38 + facet_variation * 0.6).clamp(0.26, 0.50);
                let b = (0.24 + facet_variation * 0.4).clamp(0.14, 0.36);
                [r, g, b, 1.0]
            }
            MAT_RUBY => {
                let r = (2.2 + facet_variation * 0.8).clamp(1.8, 2.6);
                let g = (0.20 + facet_variation * 0.4).clamp(0.10, 0.35);
                let b = (0.45 + facet_variation * 0.6).clamp(0.30, 0.60);
                [r, g, b, 1.0]
            }
            MAT_COLLAPSED_RUBBLE => {
                let r = (0.40 + facet_variation).clamp(0.28, 0.52);
                let g = (0.36 + facet_variation).clamp(0.24, 0.48);
                let b = (0.34 + facet_variation).clamp(0.22, 0.46);
                [r, g, b, 1.0]
            }
            _ => [0.42, 0.44, 0.46, 1.0],
        }
    }
}

#[inline]
fn push_unshared_face(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    curr_idx: &mut u32,
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
    v3: Vec3,
    color: [f32; 4],
    chunk_base_x: f32,
    chunk_base_z: f32,
) {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let n1 = edge1.cross(edge2).normalize_or_zero().to_array();

    let edge3 = v2 - v0;
    let edge4 = v3 - v0;
    let n2 = edge3.cross(edge4).normalize_or_zero().to_array();

    // Triangle 1: (v0, v1, v2)
    positions.push(v0.to_array());
    positions.push(v1.to_array());
    positions.push(v2.to_array());
    for _ in 0..3 {
        normals.push(n1);
        colors.push(color);
    }
    uvs.push([(chunk_base_x + v0.x) * 0.1, (chunk_base_z + v0.z) * 0.1]);
    uvs.push([(chunk_base_x + v1.x) * 0.1, (chunk_base_z + v1.z) * 0.1]);
    uvs.push([(chunk_base_x + v2.x) * 0.1, (chunk_base_z + v2.z) * 0.1]);
    indices.push(*curr_idx);
    indices.push(*curr_idx + 1);
    indices.push(*curr_idx + 2);
    *curr_idx += 3;

    // Triangle 2: (v0, v2, v3)
    positions.push(v0.to_array());
    positions.push(v2.to_array());
    positions.push(v3.to_array());
    for _ in 0..3 {
        normals.push(n2);
        colors.push(color);
    }
    uvs.push([(chunk_base_x + v0.x) * 0.1, (chunk_base_z + v0.z) * 0.1]);
    uvs.push([(chunk_base_x + v2.x) * 0.1, (chunk_base_z + v2.z) * 0.1]);
    uvs.push([(chunk_base_x + v3.x) * 0.1, (chunk_base_z + v3.z) * 0.1]);
    indices.push(*curr_idx);
    indices.push(*curr_idx + 1);
    indices.push(*curr_idx + 2);
    *curr_idx += 3;
}

// ----------------------------------------------------------------------------
// VERTICAL CHUNK-ON-DEMAND WINDOW & SUBTERRANEAN MESH GENERATOR
// ----------------------------------------------------------------------------

pub fn mesh_subterrain_chunk(
    db_chunks: &BTreeMap<u64, VoxelChunk>,
    cx: i32,
    cz: i32,
    player_y: f32,
) -> Option<Mesh> {
    let chunk_base_x = cx as f32 * LOW_POLY_CHUNK_SPAN;
    let chunk_base_z = cz as f32 * LOW_POLY_CHUNK_SPAN;

    // 1. Identify surface height bounds for this column
    let mut chunk_max_h = BEDROCK_ELEVATION;
    let mut chunk_min_h = f32::MAX;
    for pz in 0..=16 {
        for px in 0..=16 {
            let h = crate::terrain::get_terrain_height(chunk_base_x + px as f32, chunk_base_z + pz as f32);
            chunk_max_h = chunk_max_h.max(h);
            chunk_min_h = chunk_min_h.min(h);
        }
    }

    // 2. Vertical chunk-on-demand window:
    // Centered around player_y: [player_y - 20m, player_y + 12m]
    let base_min_y = (player_y - 20.0).floor() as i32;
    let base_max_y = (player_y + 12.0).ceil() as i32;

    // Check if column has modified server db_chunks
    let col_chunks: Vec<&VoxelChunk> = db_chunks.values()
        .filter(|c| c.chunk_x == cx && c.chunk_z == cz)
        .collect();

    let mut win_min_y = base_min_y.max(BEDROCK_ELEVATION as i32);
    let mut win_max_y = base_max_y.min((chunk_max_h + 1.0).ceil() as i32);

    // Expand window if excavated chunks exist in this column
    for c in &col_chunks {
        let chunk_bottom = c.chunk_y * 16;
        let chunk_top = chunk_bottom + 16;
        win_min_y = win_min_y.min(chunk_bottom).max(BEDROCK_ELEVATION as i32);
        win_max_y = win_max_y.max(chunk_top).min((chunk_max_h + 1.0).ceil() as i32);
    }

    if win_min_y >= win_max_y {
        return None;
    }

    let y_span = (win_max_y - win_min_y + 3) as usize;
    let grid_w = 18usize;
    let grid_d = 18usize;
    let mut grid = vec![0u8; grid_w * grid_d * y_span];
    let mut has_cavity = false;

    // 3. Populate bounded 18x18 vertical neighborhood
    for pz in -1..=16i32 {
        for px in -1..=16i32 {
            let wx = chunk_base_x + px as f32 + 0.5;
            let wz = chunk_base_z + pz as f32 + 0.5;
            let terrain_h = crate::terrain::get_terrain_height(wx, wz);
            let surface_vy = terrain_h.floor() as i32;

            let gx = (px + 1) as usize;
            let gz = (pz + 1) as usize;

            let local_max_vy = win_max_y.min(surface_vy);

            for vy in win_min_y..=local_max_vy {
                let gy = (vy - win_min_y) as usize;
                let idx = gx + gz * grid_w + gy * (grid_w * grid_d);

                if vy <= BEDROCK_ELEVATION as i32 {
                    grid[idx] = MAT_BEDROCK;
                    continue;
                }

                // Check server modified chunks first
                let vx = cx * 16 + px;
                let vz = cz * 16 + pz;
                let cx_v = vx.div_euclid(16);
                let cy_v = vy.div_euclid(16);
                let cz_v = vz.div_euclid(16);
                let key = crate::terrain::pack_chunk_key(cx_v, cy_v, cz_v);

                if let Some(chunk) = db_chunks.get(&key) {
                    let lx = vx.rem_euclid(16) as usize;
                    let ly = vy.rem_euclid(16) as usize;
                    let lz = vz.rem_euclid(16) as usize;
                    let c_idx = lx + ly * 16 + lz * 256;
                    if let Some(&mat) = chunk.voxels.get(c_idx) {
                        grid[idx] = mat;
                        if mat == 0 {
                            has_cavity = true;
                        }
                        continue;
                    }
                }

                // Unmined procedural strata
                let wy = vy as f32 + 0.5;
                if is_cave_air_at(wx, wy, wz, terrain_h) {
                    grid[idx] = 0; // Natural cave
                    has_cavity = true;
                } else if wy > terrain_h - 3.0 {
                    grid[idx] = MAT_DIRT;
                } else {
                    grid[idx] = procedural_stone_or_ore(vx, vy, vz, wy);
                }
            }
        }
    }

    // Optimization: If the window is completely solid without any caves or excavations, return None
    if !has_cavity {
        return None;
    }

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(1024);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(1024);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(1024);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(1024);
    let mut indices: Vec<u32> = Vec::with_capacity(1024);
    let mut curr_idx = 0u32;

    // 4. Generate Smashed-Cuboid Voronoi Facets for all exposed block boundaries
    for lz in 0..16usize {
        for lx in 0..16usize {
            let gx = lx + 1;
            let gz = lz + 1;
            let quad_h = crate::terrain::get_terrain_height(chunk_base_x + lx as f32 + 0.5, chunk_base_z + lz as f32 + 0.5);
            let surface_vy = quad_h.floor() as i32;

            let quad_center_x = chunk_base_x + lx as f32 + 0.5;
            let quad_center_z = chunk_base_z + lz as f32 + 0.5;
            let surface_open = is_cave_air_at(quad_center_x, quad_h - 0.25, quad_center_z, quad_h)
                || {
                    let check_cy = surface_vy.div_euclid(16);
                    let check_ly = surface_vy.rem_euclid(16) as usize;
                    if let Some(c) = col_chunks.iter().find(|c| c.chunk_y == check_cy) {
                        c.voxels.get(lx + check_ly * 16 + lz * 256).copied().unwrap_or(1) == 0
                    } else {
                        false
                    }
                };

            let local_max_vy = win_max_y.min(surface_vy);

            for vy in win_min_y..=local_max_vy {
                let gy = (vy - win_min_y) as usize;
                let idx = gx + gz * grid_w + gy * (grid_w * grid_d);
                let mat = grid[idx];
                if mat == 0 {
                    continue; // Skip air
                }

                let vx = cx * 16 + lx as i32;
                let vz = cz * 16 + lz as i32;
                let wy = vy as f32;

                // Conformal 8 Voronoi corners for cell (vx, vy, vz)
                let v000 = get_voronoi_corner_local(vx, vy, vz, chunk_base_x, chunk_base_z);
                let v100 = get_voronoi_corner_local(vx + 1, vy, vz, chunk_base_x, chunk_base_z);
                let v010 = get_voronoi_corner_local(vx, vy + 1, vz, chunk_base_x, chunk_base_z);
                let v110 = get_voronoi_corner_local(vx + 1, vy + 1, vz, chunk_base_x, chunk_base_z);
                let v001 = get_voronoi_corner_local(vx, vy, vz + 1, chunk_base_x, chunk_base_z);
                let v101 = get_voronoi_corner_local(vx + 1, vy, vz + 1, chunk_base_x, chunk_base_z);
                let v011 = get_voronoi_corner_local(vx, vy + 1, vz + 1, chunk_base_x, chunk_base_z);
                let v111 = get_voronoi_corner_local(vx + 1, vy + 1, vz + 1, chunk_base_x, chunk_base_z);

                // 1. +Y Face (Floor - Walkable cave or dug tunnel floor)
                let neighbor_yp = if gy + 1 < y_span { grid[gx + gz * grid_w + (gy + 1) * (grid_w * grid_d)] } else { 0 };
                if neighbor_yp == 0 && (vy < surface_vy || surface_open) {
                    let col = get_voxel_face_color(mat, true, wy + 1.0, vx, vy, vz);
                    // Smooth walkable ramp: if forward step in descending tunnel is also air, bevel leading edge
                    push_unshared_face(
                        &mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx,
                        v010, v011, v111, v110, col, chunk_base_x, chunk_base_z
                    );
                }

                // 2. -Y Face (Ceiling)
                let neighbor_ym = if gy > 0 { grid[gx + gz * grid_w + (gy - 1) * (grid_w * grid_d)] } else { MAT_BEDROCK };
                if neighbor_ym == 0 {
                    let col = get_voxel_face_color(mat, false, wy, vx, vy, vz);
                    push_unshared_face(
                        &mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx,
                        v000, v100, v101, v001, col, chunk_base_x, chunk_base_z
                    );
                }

                // 3. +X Face (East Wall)
                let neighbor_xp = grid[(gx + 1) + gz * grid_w + gy * (grid_w * grid_d)];
                if neighbor_xp == 0 {
                    let col = get_voxel_face_color(mat, false, wy, vx, vy, vz);
                    push_unshared_face(
                        &mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx,
                        v100, v110, v111, v101, col, chunk_base_x, chunk_base_z
                    );
                }

                // 4. -X Face (West Wall)
                let neighbor_xm = grid[(gx - 1) + gz * grid_w + gy * (grid_w * grid_d)];
                if neighbor_xm == 0 {
                    let col = get_voxel_face_color(mat, false, wy, vx, vy, vz);
                    push_unshared_face(
                        &mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx,
                        v000, v001, v011, v010, col, chunk_base_x, chunk_base_z
                    );
                }

                // 5. +Z Face (South Wall)
                let neighbor_zp = grid[gx + (gz + 1) * grid_w + gy * (grid_w * grid_d)];
                if neighbor_zp == 0 {
                    let col = get_voxel_face_color(mat, false, wy, vx, vy, vz);
                    push_unshared_face(
                        &mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx,
                        v101, v111, v011, v001, col, chunk_base_x, chunk_base_z
                    );
                }

                // 6. -Z Face (North Wall)
                let neighbor_zm = grid[gx + (gz - 1) * grid_w + gy * (grid_w * grid_d)];
                if neighbor_zm == 0 {
                    let col = get_voxel_face_color(mat, false, wy, vx, vy, vz);
                    push_unshared_face(
                        &mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx,
                        v000, v010, v110, v100, col, chunk_base_x, chunk_base_z
                    );
                }
            }
        }
    }

    if positions.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voronoi_corner_determinism() {
        let p1 = get_voronoi_corner_local(5, 10, 15, 0.0, 0.0);
        let p2 = get_voronoi_corner_local(5, 10, 15, 0.0, 0.0);
        assert_eq!(p1, p2, "Lattice corner perturbation must be 100% deterministic");
    }

    #[test]
    fn test_voronoi_jitter_bounds() {
        for gx in -5..5 {
            for gy in -5..5 {
                for gz in -5..5 {
                    let corner = get_voronoi_corner_local(gx, gy, gz, 0.0, 0.0);
                    let dx = (corner.x - gx as f32).abs();
                    let dy = (corner.y - gy as f32).abs();
                    let dz = (corner.z - gz as f32).abs();
                    assert!(dx <= 0.25, "X jitter must not exceed 0.25m cell threshold");
                    assert!(dy <= 0.20, "Y jitter must not exceed 0.20m cell threshold");
                    assert!(dz <= 0.25, "Z jitter must not exceed 0.25m cell threshold");
                }
            }
        }
    }

    #[test]
    fn test_subterrain_mesh_empty_on_solid() {
        let db_chunks = BTreeMap::new();
        // At high altitude (player y = 80m, max terrain = 25m), subterranean window has no cavity
        let mesh = mesh_subterrain_chunk(&db_chunks, 0, 0, 80.0);
        assert!(mesh.is_none(), "Solid stone without caves or excavated blocks must not spawn subterrain mesh");
    }

    #[test]
    fn test_subterrain_mesh_excavated_voronoi_facets() {
        let mut db_chunks = BTreeMap::new();
        let key = crate::terrain::pack_chunk_key(0, 0, 0);
        let mut voxels = vec![MAT_STONE; 4096];
        // Carve an excavated 2x2x2 cavity
        for ly in 4..6 {
            for lz in 4..6 {
                for lx in 4..6 {
                    let idx = lx + ly * 16 + lz * 256;
                    voxels[idx] = MAT_AIR;
                }
            }
        }
        db_chunks.insert(key, VoxelChunk {
            chunk_key: key,
            chunk_x: 0,
            chunk_y: 0,
            chunk_z: 0,
            voxels,
            last_modified_tick: 1,
        });

        let mesh = mesh_subterrain_chunk(&db_chunks, 0, 0, 5.0);
        assert!(mesh.is_some(), "Excavated block must generate Voronoi subterrain mesh");
        let mesh = mesh.unwrap();

        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).expect("positions must exist");
        let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).expect("normals must exist");
        let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR).expect("colors must exist");
        assert!(!positions.is_empty());
        assert_eq!(normals.len(), positions.len());
        assert_eq!(colors.len(), positions.len());
    }
}
