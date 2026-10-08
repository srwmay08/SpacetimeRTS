// ============================================================================
// File: terrain.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL VOXEL & CONTINUOUS TERRAIN SYSTEM (Bevy Engine / SpacetimeDB / Avian3D)
// ----------------------------------------------------------------------------
// Architectural Note: Implements noise-driven procedural voxel terrain streaming
// via `bevy_voxel_world` while preserving full deterministic synchronization with
// SpacetimeDB authoritative server reducers. BTree data structures enforce strict
// deterministic ordering in compliance with SpacetimeDB agent guidelines.

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::RenderLayers;
use bevy::pbr::NotShadowCaster;
use avian3d::prelude::*;
use bevy_voxel_world::prelude::{
    ChunkDespawnStrategy, ChunkSpawnStrategy, VoxelLookupDelegate, VoxelWorldCamera, VoxelWorldConfig, VoxelWorldPlugin, WorldVoxel,
};
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use spacetimedb_sdk::Table;

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::creatures::create_lowpoly_pet_mesh;
use crate::module_bindings::voxel_chunk_table::VoxelChunkTableAccess;
use crate::module_bindings::VoxelChunk;

/// Feature flag allowing seamless toggling between bevy_voxel_world procedural terrain
/// and the high-performance low-poly faceted terrain system.
pub const USE_VOXEL_WORLD_TERRAIN: bool = false;

// Low-poly terrain meshing and streaming constants
pub const LOW_POLY_CHUNK_SPAN: f32 = 16.0; // 16m chunk boundary
pub const LOW_POLY_QUADS_PER_AXIS: usize = 16; // 16 quads per chunk
pub const LOW_POLY_QUAD_SIZE: f32 = LOW_POLY_CHUNK_SPAN / LOW_POLY_QUADS_PER_AXIS as f32; // 1.0m low-poly facet scale
pub const LOW_POLY_RADIUS_CHUNKS: i32 = 14; // 224m radius (reduced by 20% from 288m for performance optimization)
pub const LOW_POLY_UNLOAD_RADIUS_CHUNKS: i32 = 15; // 240m radius (immediately beyond 230.4m fog visual limit)
pub const LOW_POLY_NEAR_COLLIDER_DIST_SQ: f32 = 48.0 * 48.0; // 48m radius for Avian3D physics colliders
pub const LOW_POLY_FAR_COLLIDER_UNLOAD_SQ: f32 = 56.0 * 56.0; // 56m radius hysteresis for collider unloading

// Material constants for procedural voxel generation
pub const MAT_GRASS: u8 = 0;
pub const MAT_DIRT: u8 = 1;
pub const MAT_STONE: u8 = 2;

// ----------------------------------------------------------------------------
// PROCEDURAL TERRAIN CONFIGURATION & SAMPLER
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, Reflect)]
pub struct TerrainParams {
    pub seed: u32,                             // 1337
    pub base_height: f64,                      // 32.0 average ground level in standalone mode
    pub height_amplitude: f64,                 // 24.0 hills rise/fall
    pub height_frequency: f64,                 // 0.005 lower = wider, smoother hills
    pub height_octaves: usize,                 // 5 more = finer surface detail
    pub cave_frequency: f64,                   // 0.03 cave size (lower = bigger caverns)
    pub cave_threshold: f64,                   // 0.35 lower = more caves; >1.0 disables caves
    pub cave_crust: f64,                       // 4.0 solid layer kept under the surface
    pub dirt_depth: f64,                       // 3.0 thickness of dirt below the grass
    pub spawning_distance: u32,                // 6 chunks (192m radius, covers 165m fog view distance)
    pub sync_with_spacetimedb_elevation: bool, // Synchronizes with SpacetimeDB server authoritative elevation
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            seed: 1337,
            base_height: 32.0,
            height_amplitude: 24.0,
            height_frequency: 0.005,
            height_octaves: 5,
            cave_frequency: 0.03,
            cave_threshold: 0.35,
            cave_crust: 4.0,
            dirt_depth: 3.0,
            spawning_distance: 6,
            sync_with_spacetimedb_elevation: true,
        }
    }
}

/// Pure function to evaluate the surface height at world coordinates (x, z).
/// When `sync_with_spacetimedb_elevation` is active, evaluates canonical elevation to
/// maintain mathematical parity with SpacetimeDB server-authoritative movement.
pub fn surface_height(params: &TerrainParams, x: f64, z: f64) -> f64 {
    if params.sync_with_spacetimedb_elevation {
        compute_canonical_terrain_height(x as f32, z as f32) as f64
    } else {
        let surface_noise = Fbm::<Perlin>::new(params.seed)
            .set_octaves(params.height_octaves)
            .set_frequency(params.height_frequency);
        let sample = surface_noise.get([x, z]);
        params.base_height + sample * params.height_amplitude
    }
}

/// Evaluates a single voxel at world position `pos` with precomputed surface height.
pub fn sample_voxel_with_height(
    params: &TerrainParams,
    cave_noise: &Fbm<Perlin>,
    pos: IVec3,
    height: f64,
) -> WorldVoxel<u8> {
    let y = pos.y as f64;

    // Rule: Return early above the surface, before doing any 3D noise sampling.
    if y > height {
        return WorldVoxel::Air;
    }

    // Material by depth below the surface:
    // grass (top), dirt (next dirt_depth blocks), stone (everything else).
    let depth = height - y;

    // Deep bedrock optimization: deep underground voxels (>18m below surface) are solid stone.
    // Return early before sampling expensive 3D fractal noise.
    if depth > 18.0 {
        return WorldVoxel::Solid(MAT_STONE);
    }

    // Caves: 3D fBm (seed + 1) sampled at (x, y, z).
    // Carve to air where value > cave_threshold, but only when y < height - cave_crust.
    if params.cave_threshold <= 1.0 && y < height - params.cave_crust {
        let cave_sample = cave_noise.get([pos.x as f64, y, pos.z as f64]);
        if cave_sample > params.cave_threshold {
            return WorldVoxel::Air;
        }
    }

    if depth < 1.0 {
        WorldVoxel::Solid(MAT_GRASS)
    } else if depth <= 1.0 + params.dirt_depth {
        WorldVoxel::Solid(MAT_DIRT)
    } else {
        WorldVoxel::Solid(MAT_STONE)
    }
}

/// Pure voxel sampler function.
#[allow(dead_code)]
pub fn sample_voxel(
    params: &TerrainParams,
    surface_noise: &Fbm<Perlin>,
    cave_noise: &Fbm<Perlin>,
    pos: IVec3,
) -> WorldVoxel<u8> {
    let sample = surface_noise.get([pos.x as f64, pos.z as f64]);
    let height = if params.sync_with_spacetimedb_elevation {
        compute_canonical_terrain_height(pos.x as f32, pos.z as f32) as f64
    } else {
        params.base_height + sample * params.height_amplitude
    };
    sample_voxel_with_height(params, cave_noise, pos, height)
}

#[derive(Resource, Clone, Default)]
pub struct ProceduralTerrainConfig {
    pub params: TerrainParams,
}

impl VoxelWorldConfig for ProceduralTerrainConfig {
    type MaterialIndex = u8;

    fn spawning_distance(&self) -> u32 {
        self.params.spawning_distance
    }

    fn chunk_despawn_strategy(&self) -> ChunkDespawnStrategy {
        ChunkDespawnStrategy::FarAway
    }

    fn chunk_spawn_strategy(&self) -> ChunkSpawnStrategy {
        ChunkSpawnStrategy::Close
    }

    fn max_spawn_per_frame(&self) -> usize {
        4096
    }

    fn spawning_rays(&self) -> usize {
        100
    }

    fn spawning_ray_margin(&self) -> u32 {
        25
    }

    fn texture_index_mapper(&self) -> Arc<dyn Fn(Self::MaterialIndex) -> [u32; 3] + Send + Sync> {
        Arc::new(|mat| match mat {
            MAT_GRASS => [0, 1, 2],
            MAT_DIRT => [2, 2, 2],
            MAT_STONE => [3, 3, 3],
            _ => [0, 0, 0],
        })
    }

    fn voxel_lookup_delegate(&self) -> VoxelLookupDelegate<Self::MaterialIndex> {
        let params = self.params.clone();
        let surface_noise = Fbm::<Perlin>::new(params.seed)
            .set_octaves(params.height_octaves)
            .set_frequency(params.height_frequency);
        let cave_noise = Fbm::<Perlin>::new(params.seed.wrapping_add(1))
            .set_frequency(params.cave_frequency);

        Box::new(move |chunk_pos| {
            // High altitude sky chunks: if the lowest voxel of the chunk is > 64m (well above 25m/56m max terrain),
            // all voxels in this chunk are guaranteed to be Air.
            if chunk_pos.y * 32 > 64 {
                return Box::new(|_| WorldVoxel::Air);
            }
            // Deep bedrock chunks: if the highest voxel of the chunk is < -32m, all voxels are guaranteed to be Solid Stone.
            if chunk_pos.y * 32 < -32 {
                return Box::new(|_| WorldVoxel::Solid(MAT_STONE));
            }

            let surface_noise = surface_noise.clone();
            let cave_noise = cave_noise.clone();
            let params = params.clone();
            // AI_RULES.md: BTreeMap guarantees deterministic order without randomized SipHash
            let mut height_cache = BTreeMap::<(i32, i32), f64>::new();

            Box::new(move |pos: IVec3| {
                let height = *height_cache.entry((pos.x, pos.z)).or_insert_with(|| {
                    if params.sync_with_spacetimedb_elevation {
                        compute_canonical_terrain_height(pos.x as f32, pos.z as f32) as f64
                    } else {
                        let sample = surface_noise.get([pos.x as f64, pos.z as f64]);
                        params.base_height + sample * params.height_amplitude
                    }
                });

                sample_voxel_with_height(&params, &cave_noise, pos, height)
            })
        })
    }
}

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        if USE_VOXEL_WORLD_TERRAIN {
            app.add_plugins(VoxelWorldPlugin::with_config(
                ProceduralTerrainConfig::default(),
            ))
            .add_systems(
                Update,
                sync_voxel_world_camera.run_if(in_state(GameState::InGame)),
            );
        }
    }
}

// ----------------------------------------------------------------------------
// TERRAIN HEIGHT LOOKUP & DETERMINISTIC CANONICAL NOISE
// ----------------------------------------------------------------------------

#[allow(dead_code)]
pub const VOXEL_CHUNK_SIZE: usize = 16;
pub const VOXEL_SIZE: f32 = 1.0;


static PERLIN: OnceLock<Perlin> = OnceLock::new();
static DEFAULT_TERRAIN_PARAMS: OnceLock<TerrainParams> = OnceLock::new();

#[inline]
pub fn get_default_terrain_params() -> &'static TerrainParams {
    DEFAULT_TERRAIN_PARAMS.get_or_init(TerrainParams::default)
}

// AI_RULES.md: BTreeMap ensures deterministic ordering and zero SipHash randomness
static TERRAIN_HEIGHT_CACHE: OnceLock<std::sync::RwLock<BTreeMap<(i32, i32), f32>>> = OnceLock::new();

#[inline]
pub fn get_terrain_height_cache() -> &'static std::sync::RwLock<BTreeMap<(i32, i32), f32>> {
    TERRAIN_HEIGHT_CACHE.get_or_init(|| std::sync::RwLock::new(BTreeMap::new()))
}

#[inline]
pub fn get_perlin() -> &'static Perlin {
    PERLIN.get_or_init(|| Perlin::new(42))
}

// Cached terrain height lookup — quantizes to 0.25m grid for cache efficiency
pub fn get_terrain_height(x: f32, z: f32) -> f32 {
    let qx = (x * 4.0).round() as i32;
    let qz = (z * 4.0).round() as i32;
    let delta = crate::zone_editor::get_sculpted_height_delta(x, z);
    
    // Try cache first (read lock)
    {
        let cache = get_terrain_height_cache();
        if let Ok(cache) = cache.read() {
            if let Some(&height) = cache.get(&(qx, qz)) {
                return height + delta;
            }
        }
    }
    
    // Cache miss — compute height
    let height = compute_terrain_height(x, z);
    
    // Store in cache (write lock)
    {
        let cache = get_terrain_height_cache();
        if let Ok(mut cache) = cache.write() {
            if cache.len() > 50000 {
                cache.clear();
            }
            cache.insert((qx, qz), height);
        }
    }
    
    height + delta
}

// Terrain height computation: delegates to procedural voxel terrain or canonical height
fn compute_terrain_height(x: f32, z: f32) -> f32 {
    if USE_VOXEL_WORLD_TERRAIN {
        surface_height(get_default_terrain_params(), x as f64, z as f64) as f32
    } else {
        compute_canonical_terrain_height(x, z)
    }
}

// Canonical SpacetimeDB terrain elevation function synchronized with server module reducers
pub fn compute_canonical_terrain_height(x: f32, z: f32) -> f32 {
    let scale = 0.015; 
    let base_height_amp = 18.0; 
    let noise = get_perlin();

    let nx = x as f64 * scale; 
    let nz = z as f64 * scale;

    let mut elevation = noise.get([nx, nz]) * 0.6
        + noise.get([nx * 2.0, nz * 2.0]) * 0.3
        + noise.get([nx * 4.0, nz * 4.0]) * 0.1;
    
    elevation = (elevation + 1.0) * 0.5;
    let mut y = (elevation.powf(1.4)) as f32 * base_height_amp;

    let river_factor = (x * 0.04).cos().abs() * 3.5;
    if river_factor < 2.0 {
        y = (y - (2.0 - river_factor)).max(0.5);
    }

    let lake_dist = ((x + 35.0) * (x + 35.0) + (z + 35.0) * (z + 35.0)).sqrt();
    if lake_dist < 25.0 {
        let basin_depth = (1.0 - (lake_dist / 25.0)).max(0.0) * 4.0;
        y = (y - basin_depth).max(0.2);
    }

    y
}

// ----------------------------------------------------------------------------
// LOW-POLY FACETED TERRAIN MESH GENERATION & PRIORITY STREAMING
// ----------------------------------------------------------------------------
// Architectural Note: Generates flat-shaded low-poly faceted terrain chunks where
// each triangle has its own flat geometric face normal and stylized vertex colors.
// Chunks are streamed via distance-prioritized concentric ordering, guaranteeing
// immediate frame-1 loading around the player with zero gaps and seamless boundary heights.

#[inline]
pub fn pack_chunk_key(cx: i32, cy: i32, cz: i32) -> u64 {
    let x_bits = (cx as i64 + 0x800000) as u64 & 0xFFFFFF;
    let y_bits = (cy as i64 + 0x8000) as u64 & 0xFFFF;
    let z_bits = (cz as i64 + 0x800000) as u64 & 0xFFFFFF;
    (x_bits << 40) | (y_bits << 24) | z_bits
}

#[inline]
pub fn unpack_chunk_key(key: u64) -> (i32, i32, i32) {
    let x_bits = ((key >> 40) & 0xFFFFFF) as i64 - 0x800000;
    let y_bits = ((key >> 24) & 0xFFFF) as i64 - 0x8000;
    let z_bits = (key & 0xFFFFFF) as i64 - 0x800000;
    (x_bits as i32, y_bits as i32, z_bits as i32)
}

/// Generates a low-poly faceted terrain mesh for a chunk at grid coordinates (cx, cz).
/// Every triangle has separate unshared vertices and a flat geometric face normal,
/// producing a distinctive, crisp low-poly aesthetic rather than smooth curves or Minecraft blocks.
/// Helper checking if a neighbor voxel is solid.
/// Checks the local chunk, adjacent loaded db_chunks, or falls back to procedural terrain and cave noise.
fn is_solid_voxel_neighbor(
    db_chunks: &BTreeMap<u64, VoxelChunk>,
    cx: i32,
    cy: i32,
    cz: i32,
    lx: i32,
    ly: i32,
    lz: i32,
) -> bool {
    let mut n_cx = cx;
    let mut n_cy = cy;
    let mut n_cz = cz;
    let mut n_lx = lx;
    let mut n_ly = ly;
    let mut n_lz = lz;

    if n_lx < 0 {
        n_cx -= 1;
        n_lx += 16;
    } else if n_lx >= 16 {
        n_cx += 1;
        n_lx -= 16;
    }

    if n_ly < 0 {
        n_cy -= 1;
        n_ly += 16;
    } else if n_ly >= 16 {
        n_cy += 1;
        n_ly -= 16;
    }

    if n_lz < 0 {
        n_cz -= 1;
        n_lz += 16;
    } else if n_lz >= 16 {
        n_cz += 1;
        n_lz -= 16;
    }

    let key = pack_chunk_key(n_cx, n_cy, n_cz);
    if let Some(chunk) = db_chunks.get(&key) {
        if n_lx >= 0 && n_lx < 16 && n_ly >= 0 && n_ly < 16 && n_lz >= 0 && n_lz < 16 {
            let idx = n_lx as usize + (n_ly as usize * 16) + (n_lz as usize * 256);
            if let Some(&mat) = chunk.voxels.get(idx) {
                return mat != 0;
            }
        }
        return false;
    }

    // Procedural evaluation for un-persisted neighbors
    let wx = (n_cx * 16 + n_lx) as f32 * VOXEL_SIZE;
    let wy = (n_cy * 16 + n_ly) as f32 * VOXEL_SIZE;
    let wz = (n_cz * 16 + n_lz) as f32 * VOXEL_SIZE;

    if wy <= -120.0 {
        return true;
    }
    let h = compute_canonical_terrain_height(wx, wz);
    if wy > h {
        return false;
    }
    if wy < h - 4.0 && wy > -118.0 {
        let cave_noise = Perlin::new(1338);
        let freq = 0.035;
        let sample = cave_noise.get([wx as f64 * freq, wy as f64 * freq, wz as f64 * freq]);
        if sample > 0.38 {
            return false;
        }
    }
    true
}

fn get_voxel_face_color(mat: u8, is_top: bool, wy: f32, vx: i32, vy: i32, vz: i32) -> [f32; 4] {
    let facet_hash = (((vx.wrapping_mul(37) ^ vy.wrapping_mul(59) ^ vz.wrapping_mul(71)) as f32 * 0.17).sin().abs() * 43758.5453).fract();
    let facet_variation = (facet_hash - 0.5) * 0.09;

    if is_top && wy >= 2.0 && wy <= 15.5 {
        let r = (0.28 + facet_variation * 0.8).clamp(0.18, 0.40);
        let g = (0.64 + facet_variation).clamp(0.48, 0.76);
        let b = (0.28 + facet_variation * 0.8).clamp(0.18, 0.40);
        [r, g, b, 1.0] // Meadow grass top
    } else {
        match mat {
            1 => {
                // Dirt
                let r = (0.46 + facet_variation).clamp(0.35, 0.60);
                let g = (0.38 + facet_variation).clamp(0.30, 0.50);
                let b = (0.28 + facet_variation).clamp(0.20, 0.40);
                [r, g, b, 1.0]
            }
            2 => {
                // Stone: Low-poly BlendSwap #9440 faceted rock
                let r = (0.42 + facet_variation).clamp(0.28, 0.56);
                let g = (0.44 + facet_variation).clamp(0.30, 0.58);
                let b = (0.46 + facet_variation * 0.8).clamp(0.32, 0.60);
                [r, g, b, 1.0]
            }
            3 => {
                // Sand
                let r = (0.78 + facet_variation).clamp(0.65, 0.90);
                let g = (0.72 + facet_variation).clamp(0.60, 0.85);
                let b = (0.52 + facet_variation).clamp(0.40, 0.70);
                [r, g, b, 1.0]
            }
            4 => [0.55, 0.40, 0.25, 1.0], // Wood
            5 => {
                // Reinforced stone
                let r = (0.35 + facet_variation).clamp(0.25, 0.45);
                let g = (0.35 + facet_variation).clamp(0.25, 0.45);
                let b = (0.40 + facet_variation).clamp(0.30, 0.50);
                [r, g, b, 1.0]
            }
            6 => [0.18, 0.18, 0.20, 1.0], // Bedrock
            7 => {
                // IronOre: Rich oxidized metallic flecks
                let r = (0.64 + facet_variation * 1.2).clamp(0.46, 0.80);
                let g = (0.38 + facet_variation * 0.6).clamp(0.26, 0.50);
                let b = (0.24 + facet_variation * 0.4).clamp(0.14, 0.36);
                [r, g, b, 1.0]
            }
            8 => {
                // Ruby: Vivid crystalline facet
                let r = (0.88 + facet_variation * 0.8).clamp(0.72, 1.0);
                let g = (0.15 + facet_variation * 0.4).clamp(0.08, 0.25);
                let b = (0.35 + facet_variation * 0.6).clamp(0.22, 0.48);
                [r, g, b, 1.0]
            }
            9 => {
                // CollapsedRubble
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
    _fallback_norm: [f32; 3],
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

/// Generates a low-poly faceted terrain mesh for a chunk at grid coordinates (cx, cz).
/// Combines canonical unmined low-poly surface facets with 3D Face-Culled cubic blocks
/// (flat 0° floors, 90° sheer walls, and 0° ceilings) for excavated volumes.
pub fn mesh_low_poly_terrain_chunk(
    db_chunks: &BTreeMap<u64, VoxelChunk>,
    cx: i32,
    cz: i32,
) -> Option<Mesh> {
    let chunk_base_x = cx as f32 * LOW_POLY_CHUNK_SPAN;
    let chunk_base_z = cz as f32 * LOW_POLY_CHUNK_SPAN;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(2048);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(2048);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(2048);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(2048);
    let mut indices: Vec<u32> = Vec::with_capacity(2048);
    let mut curr_idx = 0u32;

    let column_chunks: Vec<&VoxelChunk> = db_chunks.values()
        .filter(|c| c.chunk_x == cx && c.chunk_z == cz)
        .collect();

    // 1. Surface Quads (16x16 grid, 1.0m quads):
    // For each quad (qx, qz), render surface terrain unless this quad has been excavated.
    for qz in 0..16 {
        for qx in 0..16 {
            let x0 = qx as f32 * LOW_POLY_QUAD_SIZE;
            let x1 = (qx + 1) as f32 * LOW_POLY_QUAD_SIZE;
            let z0 = qz as f32 * LOW_POLY_QUAD_SIZE;
            let z1 = (qz + 1) as f32 * LOW_POLY_QUAD_SIZE;

            let y00 = get_terrain_height(chunk_base_x + x0, chunk_base_z + z0);
            let y10 = get_terrain_height(chunk_base_x + x1, chunk_base_z + z0);
            let y01 = get_terrain_height(chunk_base_x + x0, chunk_base_z + z1);
            let y11 = get_terrain_height(chunk_base_x + x1, chunk_base_z + z1);

            let min_y = y00.min(y10).min(y01).min(y11);
            let sub_surface_vy = (min_y - 0.25).floor() as i32;

            // Check if surface block is genuinely excavated (mined to air below natural terrain)
            let mut surface_excavated = false;
            if !column_chunks.is_empty() {
                for check_vy in [sub_surface_vy, sub_surface_vy - 1] {
                    let check_cy = check_vy.div_euclid(16);
                    let check_ly = check_vy.rem_euclid(16) as usize;
                    if let Some(chunk) = column_chunks.iter().find(|c| c.chunk_y == check_cy) {
                        let idx = qx + (check_ly * 16) + (qz * 256);
                        if let Some(&mat) = chunk.voxels.get(idx) {
                            if mat == 0 {
                                surface_excavated = true;
                                break;
                            }
                        }
                    }
                }
            }

            if !surface_excavated {
                let v00 = Vec3::new(x0, y00, z0);
                let v10 = Vec3::new(x1, y10, z0);
                let v01 = Vec3::new(x0, y01, z1);
                let v11 = Vec3::new(x1, y11, z1);

                let (tri1, tri2) = if (qx + qz) % 2 == 0 {
                    ((v00, v01, v11), (v00, v11, v10))
                } else {
                    ((v00, v01, v10), (v10, v01, v11))
                };

                for tri in [tri1, tri2] {
                    let va = tri.0;
                    let vb = tri.1;
                    let vc = tri.2;

                    let edge1 = vb - va;
                    let edge2 = vc - va;
                    let normal = edge1.cross(edge2).normalize_or_zero();

                    let world_centroid = Vec3::new(
                        chunk_base_x + (va.x + vb.x + vc.x) / 3.0,
                        (va.y + vb.y + vc.y) / 3.0,
                        chunk_base_z + (va.z + vb.z + vc.z) / 3.0,
                    );

                    let facet_hash = ((world_centroid.x * 37.17 + world_centroid.z * 53.31).sin().abs() * 43758.5453).fract();
                    let facet_variation = (facet_hash - 0.5) * 0.08;

                    let color = if let Some(painted) = crate::zone_editor::get_painted_biome_color(world_centroid.x, world_centroid.z) {
                        painted
                    } else if world_centroid.y > 15.5 {
                        let snow_white = 0.94 + facet_variation * 0.5;
                        [snow_white, snow_white + 0.02, snow_white + 0.05, 1.0]
                    } else if normal.y < 0.60 {
                        let r = (0.42 + facet_variation).clamp(0.25, 0.65);
                        let g = (0.44 + facet_variation).clamp(0.25, 0.65);
                        let b = (0.46 + facet_variation).clamp(0.25, 0.65);
                        [r, g, b, 1.0]
                    } else if world_centroid.y < 2.5 && normal.y >= 0.60 {
                        let r = (0.78 + facet_variation).clamp(0.65, 0.90);
                        let g = (0.72 + facet_variation).clamp(0.60, 0.85);
                        let b = (0.52 + facet_variation).clamp(0.40, 0.70);
                        [r, g, b, 1.0]
                    } else if normal.y < 0.72 {
                        let r = (0.46 + facet_variation).clamp(0.35, 0.60);
                        let g = (0.38 + facet_variation).clamp(0.30, 0.50);
                        let b = (0.28 + facet_variation).clamp(0.20, 0.40);
                        [r, g, b, 1.0]
                    } else {
                        let r = (0.28 + facet_variation * 0.8).clamp(0.18, 0.40);
                        let g = (0.64 + facet_variation).clamp(0.48, 0.76);
                        let b = (0.28 + facet_variation * 0.8).clamp(0.18, 0.40);
                        [r, g, b, 1.0]
                    };

                    let norm_arr = normal.to_array();
                    positions.push(va.to_array());
                    positions.push(vb.to_array());
                    positions.push(vc.to_array());

                    normals.push(norm_arr);
                    normals.push(norm_arr);
                    normals.push(norm_arr);

                    colors.push(color);
                    colors.push(color);
                    colors.push(color);

                    uvs.push([(chunk_base_x + va.x) * 0.1, (chunk_base_z + va.z) * 0.1]);
                    uvs.push([(chunk_base_x + vb.x) * 0.1, (chunk_base_z + vb.z) * 0.1]);
                    uvs.push([(chunk_base_x + vc.x) * 0.1, (chunk_base_z + vc.z) * 0.1]);

                    indices.push(curr_idx);
                    indices.push(curr_idx + 1);
                    indices.push(curr_idx + 2);
                    curr_idx += 3;
                }
            }
        }
    }

    // 2. Excavated / Volumetric Blocks (Face-Culled 3D Voxel Mesher):
    // For every solid voxel in column_chunks, emit faces exposed to air.
    for chunk in column_chunks {
        let base_vx = chunk.chunk_x * 16;
        let base_vy = chunk.chunk_y * 16;
        let base_vz = chunk.chunk_z * 16;

        for lz in 0..16 {
            for lx in 0..16 {
                let quad_avg_y = get_terrain_height(chunk_base_x + lx as f32 + 0.5, chunk_base_z + lz as f32 + 0.5);
                let surface_vy = (quad_avg_y / VOXEL_SIZE).floor() as i32;

                for ly in 0..16 {
                    let idx = lx + (ly * 16) + (lz * 256);
                    let mat = chunk.voxels[idx];
                    if mat == 0 {
                        continue;
                    }

                    let vx = base_vx + lx as i32;
                    let vy = base_vy + ly as i32;
                    let vz = base_vz + lz as i32;

                    let wx = vx as f32 * VOXEL_SIZE;
                    let wy = vy as f32 * VOXEL_SIZE;
                    let wz = vz as f32 * VOXEL_SIZE;

                    let x0 = wx - chunk_base_x;
                    let x1 = x0 + VOXEL_SIZE;
                    let y0 = wy;
                    let y1 = y0 + VOXEL_SIZE;
                    let z0 = wz - chunk_base_z;
                    let z1 = z0 + VOXEL_SIZE;

                    // Subtle depth jitter for imperfect cuboid quarry faces
                    let facet_seed = (vx.wrapping_mul(73856093) ^ vy.wrapping_mul(19349663) ^ vz.wrapping_mul(83492791)) as u32;
                    let jitter_x = ((facet_seed % 101) as f32 / 100.0 - 0.5) * 0.035;
                    let jitter_z = (((facet_seed >> 8) % 101) as f32 / 100.0 - 0.5) * 0.035;
                    let jitter_ceil = (((facet_seed >> 16) % 101) as f32 / 100.0 - 0.5) * 0.025;

                    // +Y (Top Face - walkable horizontal floor at 0°)
                    if !is_solid_voxel_neighbor(db_chunks, chunk.chunk_x, chunk.chunk_y, chunk.chunk_z, lx as i32, ly as i32 + 1, lz as i32) {
                        let is_surface_solid = vy >= surface_vy;
                        let surface_cy = surface_vy.div_euclid(16);
                        let surface_ly = surface_vy.rem_euclid(16) as usize;
                        let surface_intact = if is_surface_solid {
                            if chunk.chunk_y == surface_cy {
                                let s_idx = lx + (surface_ly * 16) + (lz * 256);
                                chunk.voxels.get(s_idx).copied().unwrap_or(0) != 0
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if !surface_intact {
                            let norm = [0.0, 1.0, 0.0];
                            let col = get_voxel_face_color(mat, true, y1, vx, vy, vz);
                            let v0 = Vec3::new(x0, y1, z0);
                            let v1 = Vec3::new(x0, y1, z1);
                            let v2 = Vec3::new(x1, y1, z1);
                            let v3 = Vec3::new(x1, y1, z0);
                            push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v0, v1, v2, v3, norm, col, chunk_base_x, chunk_base_z);
                        }
                    }

                    // -Y (Bottom Face - horizontal ceiling)
                    if !is_solid_voxel_neighbor(db_chunks, chunk.chunk_x, chunk.chunk_y, chunk.chunk_z, lx as i32, ly as i32 - 1, lz as i32) {
                        let norm = [0.0, -1.0, 0.0];
                        let col = get_voxel_face_color(mat, false, y0, vx, vy, vz);
                        let cy = y0 + jitter_ceil;
                        let v0 = Vec3::new(x0, cy, z0);
                        let v1 = Vec3::new(x1, cy, z0);
                        let v2 = Vec3::new(x1, cy, z1);
                        let v3 = Vec3::new(x0, cy, z1);
                        push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v0, v1, v2, v3, norm, col, chunk_base_x, chunk_base_z);
                    }

                    // +X (East Wall)
                    if !is_solid_voxel_neighbor(db_chunks, chunk.chunk_x, chunk.chunk_y, chunk.chunk_z, lx as i32 + 1, ly as i32, lz as i32) {
                        let norm = [1.0, 0.0, 0.0];
                        let col = get_voxel_face_color(mat, false, y0, vx, vy, vz);
                        let cx1 = x1 + jitter_x;
                        let v0 = Vec3::new(cx1, y0, z0);
                        let v1 = Vec3::new(cx1, y1, z0);
                        let v2 = Vec3::new(cx1, y1, z1);
                        let v3 = Vec3::new(cx1, y0, z1);
                        push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v0, v1, v2, v3, norm, col, chunk_base_x, chunk_base_z);
                    }

                    // -X (West Wall)
                    if !is_solid_voxel_neighbor(db_chunks, chunk.chunk_x, chunk.chunk_y, chunk.chunk_z, lx as i32 - 1, ly as i32, lz as i32) {
                        let norm = [-1.0, 0.0, 0.0];
                        let col = get_voxel_face_color(mat, false, y0, vx, vy, vz);
                        let cx0 = x0 + jitter_x;
                        let v0 = Vec3::new(cx0, y0, z1);
                        let v1 = Vec3::new(cx0, y1, z1);
                        let v2 = Vec3::new(cx0, y1, z0);
                        let v3 = Vec3::new(cx0, y0, z0);
                        push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v0, v1, v2, v3, norm, col, chunk_base_x, chunk_base_z);
                    }

                    // +Z (South Wall)
                    if !is_solid_voxel_neighbor(db_chunks, chunk.chunk_x, chunk.chunk_y, chunk.chunk_z, lx as i32, ly as i32, lz as i32 + 1) {
                        let norm = [0.0, 0.0, 1.0];
                        let col = get_voxel_face_color(mat, false, y0, vx, vy, vz);
                        let cz1 = z1 + jitter_z;
                        let v0 = Vec3::new(x1, y0, cz1);
                        let v1 = Vec3::new(x1, y1, cz1);
                        let v2 = Vec3::new(x0, y1, cz1);
                        let v3 = Vec3::new(x0, y0, cz1);
                        push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v0, v1, v2, v3, norm, col, chunk_base_x, chunk_base_z);
                    }

                    // -Z (North Wall)
                    if !is_solid_voxel_neighbor(db_chunks, chunk.chunk_x, chunk.chunk_y, chunk.chunk_z, lx as i32, ly as i32, lz as i32 - 1) {
                        let norm = [0.0, 0.0, -1.0];
                        let col = get_voxel_face_color(mat, false, y0, vx, vy, vz);
                        let cz0 = z0 + jitter_z;
                        let v0 = Vec3::new(x0, y0, cz0);
                        let v1 = Vec3::new(x0, y1, cz0);
                        let v2 = Vec3::new(x1, y1, cz0);
                        let v3 = Vec3::new(x1, y0, cz0);
                        push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v0, v1, v2, v3, norm, col, chunk_base_x, chunk_base_z);
                    }
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

/// Backward compatibility alias for legacy surface-nets callers.
#[allow(dead_code)]
#[inline]
pub fn mesh_voxel_chunk_surface_nets(
    db_chunks: &BTreeMap<u64, VoxelChunk>,
    cx: i32,
    cz: i32,
) -> Option<Mesh> {
    mesh_low_poly_terrain_chunk(db_chunks, cx, cz)
}

#[derive(Component)]
pub struct TerrainChunkVisual;

#[derive(Component)]
pub struct TerrainChunkHasCollider;

pub fn update_infinite_voxel_terrain(
    mut commands: Commands,
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_query: Query<(Entity, &mut VoxelChunkMarker, &mut Handle<Mesh>, Option<&TerrainChunkHasCollider>)>,
    render_settings: Option<Res<crate::spellbook::TerrainRenderSettings>>,
    mut default_material: Local<Option<Handle<StandardMaterial>>>,
    mut loaded_entities: Local<BTreeMap<u64, (Entity, u64, bool)>>,
    mut last_player_chunk: Local<Option<(i32, i32)>>,
) {
    if USE_VOXEL_WORLD_TERRAIN {
        return;
    }

    let Ok(player_transform) = player_query.get_single() else { return; };

    let mat_handle = default_material.get_or_insert_with(|| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            reflectance: 0.15,
            ..default()
        })
    }).clone();

    let chunk_world_span = LOW_POLY_CHUNK_SPAN; // 16.0m
    let p_pos = player_transform.translation;
    let p_cx = (p_pos.x / chunk_world_span).floor() as i32;
    let p_cz = (p_pos.z / chunk_world_span).floor() as i32;

    *last_player_chunk = Some((p_cx, p_cz));

    let db_chunks: BTreeMap<u64, VoxelChunk> = conn.db.db.voxel_chunk()
        .iter()
        .map(|c| (c.chunk_key, c))
        .collect();

    loaded_entities.clear();
    for (entity, marker, _, has_col) in chunk_query.iter() {
        loaded_entities.insert(marker.chunk_key, (entity, marker.last_modified_tick, has_col.is_some()));
    }

    // 1. Check existing loaded chunks for server modifications or collider distance transitions
    for (&key, &(existing_entity, last_tick, has_collider)) in loaded_entities.iter() {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        let chunk_center_x = (cx as f32 + 0.5) * chunk_world_span;
        let chunk_center_z = (cz as f32 + 0.5) * chunk_world_span;
        let dist_sq = (chunk_center_x - p_pos.x).powi(2) + (chunk_center_z - p_pos.z).powi(2);

        // Check if any server voxel chunk within this 16m chunk column has been modified
        let server_mod_tick = db_chunks.values()
            .filter(|c| c.chunk_x == cx && c.chunk_z == cz)
            .map(|c| c.last_modified_tick)
            .max()
            .unwrap_or(0);

        let editor_dirty = crate::zone_editor::consume_chunk_dirty(cx, cz);
        if server_mod_tick > last_tick || editor_dirty {
            if let Some(new_mesh) = mesh_low_poly_terrain_chunk(&db_chunks, cx, cz) {
                let mesh_handle = meshes.add(new_mesh.clone());
                let mut entity_cmds = commands.entity(existing_entity);
                entity_cmds.insert(mesh_handle);
                if dist_sq <= LOW_POLY_NEAR_COLLIDER_DIST_SQ {
                    if let Some(col) = Collider::trimesh_from_mesh(&new_mesh) {
                        entity_cmds.insert((col, TerrainChunkHasCollider));
                    }
                }
                if let Ok((_, mut marker, _, _)) = chunk_query.get_mut(existing_entity) {
                    marker.last_modified_tick = server_mod_tick;
                }
                // Requirement 5: Remove ChunkHasGrass so sync_chunk_grass despawns floating grass and regenerates without mined areas
                entity_cmds.remove::<crate::grass::ChunkHasGrass>();
            }
        }

        // Dynamic collider management: add colliders near player, strip distant colliders
        if has_collider && dist_sq > LOW_POLY_FAR_COLLIDER_UNLOAD_SQ {
            commands.entity(existing_entity).remove::<Collider>().remove::<TerrainChunkHasCollider>();
        } else if !has_collider && dist_sq <= LOW_POLY_NEAR_COLLIDER_DIST_SQ {
            if let Ok((_, _, mesh_handle, _)) = chunk_query.get(existing_entity) {
                if let Some(mesh) = meshes.get(&*mesh_handle) {
                    if let Some(col) = Collider::trimesh_from_mesh(mesh) {
                        commands.entity(existing_entity).insert((col, TerrainChunkHasCollider));
                    }
                }
            }
        }
    }

    // Dynamic visible range & batch tuning from options panel
    let (view_radius, unload_radius, spawn_batch) = if let Some(ref rs) = render_settings {
        let vr = if rs.spawn_full_zone { 64 } else { rs.view_distance_chunks };
        let ur = if rs.spawn_full_zone { 70 } else { rs.unload_distance_chunks.max(vr + 1) };
        let batch = if rs.spawn_full_zone { 256 } else { 32.max(vr as usize * 3) };
        (vr, ur, batch)
    } else {
        (LOW_POLY_RADIUS_CHUNKS, LOW_POLY_UNLOAD_RADIUS_CHUNKS, 32)
    };

    // 2. Collect candidate unspawned chunks within view radius
    let mut candidates: Vec<(i32, i32, i32, u64)> = Vec::with_capacity(512);
    let radius_sq = view_radius * view_radius;

    for cz in (p_cz - view_radius)..=(p_cz + view_radius) {
        let dz = cz - p_cz;
        for cx in (p_cx - view_radius)..=(p_cx + view_radius) {
            let dx = cx - p_cx;
            let dist_sq = dx * dx + dz * dz;
            if dist_sq > radius_sq {
                continue;
            }

            let key = pack_chunk_key(cx, 0, cz);
            if !loaded_entities.contains_key(&key) {
                candidates.push((cx, cz, dist_sq, key));
            }
        }
    }

    // STRICT DISTANCE PRIORITY:
    // Closest chunks to player (dist_sq = 0, then 1, 2, 4, 5...) are sorted first.
    candidates.sort_by_key(|c| c.2);

    // Guaranteed immediate frame-1 loading for the player's immediate 3x3 surrounding chunks (dist_sq <= 2)
    let immediate_unspawned = candidates.iter().take_while(|c| c.2 <= 2).count();
    let max_spawn_this_frame = spawn_batch.max(immediate_unspawned);

    for (cx, cz, dist_sq_chunks, key) in candidates.into_iter().take(max_spawn_this_frame) {
        let db_mod_tick = db_chunks.values()
            .filter(|c| c.chunk_x == cx && c.chunk_z == cz)
            .map(|c| c.last_modified_tick)
            .max()
            .unwrap_or(0);
        if let Some(new_mesh) = mesh_low_poly_terrain_chunk(&db_chunks, cx, cz) {
            let dist_world_sq = dist_sq_chunks as f32 * chunk_world_span * chunk_world_span;
            let needs_collider = dist_world_sq <= LOW_POLY_NEAR_COLLIDER_DIST_SQ;

            let collider = if needs_collider {
                Collider::trimesh_from_mesh(&new_mesh)
            } else {
                None
            };

            let mesh_handle = meshes.add(new_mesh);
            let chunk_world_x = cx as f32 * chunk_world_span;
            let chunk_world_z = cz as f32 * chunk_world_span;

            let mut entity_cmds = commands.spawn((
                PbrBundle {
                    mesh: mesh_handle,
                    material: mat_handle.clone(),
                    transform: BevyTransform::from_xyz(chunk_world_x, 0.0, chunk_world_z),
                    ..default()
                },
                RigidBody::Static,
                CollisionLayers::new([GameLayer::Terrain], [GameLayer::Default, GameLayer::Unit, GameLayer::Environment]),
                VoxelChunkMarker {
                    chunk_key: key,
                    chunk_x: cx,
                    chunk_y: 0,
                    chunk_z: cz,
                    last_modified_tick: db_mod_tick,
                },
                TerrainChunkVisual,
            ));

            if let Some(col) = collider {
                entity_cmds.insert((col, TerrainChunkHasCollider));
            }
        }
    }

    // 3. Despawn distant chunks beyond fog visual limit efficiently
    // In Bevy atmospheric linear fog, terrain reaches 100% opacity at visible_range_meters (230.4m default).
    // Any chunk whose nearest boundary to the player exceeds the fog limit is completely invisible.
    let fog_visual_limit_meters = if let Some(ref rs) = render_settings {
        if rs.spawn_full_zone { 64.0 * chunk_world_span } else { rs.visible_range_meters.max(160.0) }
    } else {
        230.4
    };
    // Include an 8.0m (half-chunk) hysteresis buffer to prevent boundary thrashing while moving
    let discard_dist_meters = fog_visual_limit_meters + (chunk_world_span * 0.5);
    let discard_dist_sq = discard_dist_meters * discard_dist_meters;
    let unload_radius_sq = unload_radius * unload_radius;

    for (&key, &(entity, _, _)) in loaded_entities.iter() {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        let dx = cx - p_cx;
        let dz = cz - p_cz;

        // Fast integer reject for chunks beyond grid unload radius
        if dx * dx + dz * dz > unload_radius_sq {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        // Efficient spatial bounding check: calculate squared distance to closest point on chunk AABB
        let chunk_min_x = cx as f32 * chunk_world_span;
        let chunk_max_x = chunk_min_x + chunk_world_span;
        let chunk_min_z = cz as f32 * chunk_world_span;
        let chunk_max_z = chunk_min_z + chunk_world_span;

        let closest_x = p_pos.x.clamp(chunk_min_x, chunk_max_x);
        let closest_z = p_pos.z.clamp(chunk_min_z, chunk_max_z);
        let dist_to_chunk_sq = (closest_x - p_pos.x).powi(2) + (closest_z - p_pos.z).powi(2);

        if dist_to_chunk_sq > discard_dist_sq {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn spawn_initial_world(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let pet_x = 5.0; 
    let pet_z = 5.0;
    let pet_y = get_terrain_height(pet_x, pet_z) + 1.5; 

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(create_lowpoly_pet_mesh()),
            material: materials.add(StandardMaterial { 
                base_color: Color::WHITE, 
                perceptual_roughness: 0.85, 
                ..default() 
            }),
            transform: BevyTransform::from_xyz(pet_x, pet_y, pet_z),
            ..default()
        },
        RigidBody::Dynamic, 
        Collider::cuboid(0.5, 0.8, 0.9), 
        ColliderDensity(1.0),
        LockedAxes::ROTATION_LOCKED, 
        GravityScale(4.5),
        LinearVelocity::ZERO,
        CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Terrain, GameLayer::Unit, GameLayer::Environment]),
        Selectable, 
        Name::new("Loki"), 
    )).with_children(|parent| {
        parent.spawn((
            PbrBundle {
                mesh: meshes.add(bevy::math::primitives::Torus::new(0.7, 0.05)),
                material: materials.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() }),
                transform: BevyTransform::from_xyz(0.0, -0.4, 0.0), visibility: Visibility::Hidden, ..default()
            },
            RenderLayers::layer(2), SelectionRing,
            NotShadowCaster,
        ));
    });
}

pub fn spawn_voxel_gibs(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    origin: Vec3,
    count: usize,
    primary_color: Color,
    secondary_color: Color,
    gib_size: f32,
) {
    let mut rng_seed = (origin.x.abs() * 1000.0 + origin.z.abs() * 100.0) as u64;
    let cube_mesh = meshes.add(bevy::math::primitives::Cuboid::new(gib_size, gib_size, gib_size));
    let mat_primary = materials.add(StandardMaterial {
        base_color: primary_color,
        perceptual_roughness: 0.8,
        ..default()
    });
    let mat_secondary = materials.add(StandardMaterial {
        base_color: secondary_color,
        perceptual_roughness: 0.8,
        ..default()
    });

    for i in 0..count {
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let ry = ((rng_seed >> 32) as u32 % 35) as f32 / 10.0 + 1.0;

        let mat = if i % 3 == 0 { mat_secondary.clone() } else { mat_primary.clone() };
        let offset = Vec3::new(rx * 0.25, (i as f32 * 0.05).min(0.5), rz * 0.25);

        commands.spawn((
            PbrBundle {
                mesh: cube_mesh.clone(),
                material: mat,
                transform: BevyTransform::from_translation(origin + offset),
                ..default()
            },
            VoxelGib {
                timer: Timer::from_seconds(1.2 + (i as f32 * 0.03), TimerMode::Once),
                velocity: Vec3::new(rx * 4.5, ry, rz * 4.5),
                angular_velocity: Vec3::new(rx * 14.0, ry * 8.0, rz * 14.0),
            },
        ));
    }
}

pub fn tick_voxel_gibs(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut VoxelGib, &mut BevyTransform)>,
) {
    let dt = time.delta_seconds().min(0.1);
    for (entity, mut gib, mut transform) in query.iter_mut() {
        if gib.timer.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        // Apply lightweight kinematic bounce and gravity simulation
        gib.velocity.y -= 19.6 * dt;
        transform.translation += gib.velocity * dt;
        transform.rotate_local_x(gib.angular_velocity.x * dt);
        transform.rotate_local_y(gib.angular_velocity.y * dt);

        let ground_y = crate::terrain::get_terrain_height(transform.translation.x, transform.translation.z);
        if transform.translation.y <= ground_y + 0.05 {
            transform.translation.y = ground_y + 0.05;
            if gib.velocity.y < 0.0 {
                gib.velocity.y = -gib.velocity.y * 0.25;
                gib.velocity.x *= 0.65;
                gib.velocity.z *= 0.65;
            }
        }
    }
}

// ----------------------------------------------------------------------------
// CAMERA DRIVER & DETERMINISTIC SYNCHRONIZATION
// ----------------------------------------------------------------------------

/// Ensures only the active camera (FPS vs RTS) carries the `VoxelWorldCamera` marker component,
/// preventing panics in `bevy_voxel_world`'s `.single()` query across perspective transitions.
pub fn sync_voxel_world_camera(
    mut commands: Commands,
    camera_mode: Res<State<CameraMode>>,
    fps_cam: Query<Entity, With<FpsCamera>>,
    rts_cam: Query<Entity, With<RtsCameraChild>>,
    marked_cam: Query<Entity, With<VoxelWorldCamera<ProceduralTerrainConfig>>>,
) {
    let target_entity = match camera_mode.get() {
        CameraMode::FPS => fps_cam.get_single().ok(),
        CameraMode::RTS => rts_cam.get_single().ok(),
    };

    if let Some(target) = target_entity {
        let mut target_already_has_marker = false;
        for entity in marked_cam.iter() {
            if entity == target {
                target_already_has_marker = true;
            } else {
                commands.entity(entity).remove::<VoxelWorldCamera<ProceduralTerrainConfig>>();
            }
        }
        if !target_already_has_marker {
            commands.entity(target).insert(VoxelWorldCamera::<ProceduralTerrainConfig>::default());
        }
    }
}

// ----------------------------------------------------------------------------
// UNIT TESTS (AI_RULES.md Compliant)
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_params_default() {
        let p = TerrainParams::default();
        assert_eq!(p.seed, 1337);
        assert_eq!(p.base_height, 32.0);
        assert_eq!(p.height_amplitude, 24.0);
        assert_eq!(p.height_frequency, 0.005);
        assert_eq!(p.height_octaves, 5);
        assert_eq!(p.cave_frequency, 0.03);
        assert_eq!(p.cave_threshold, 0.35);
        assert_eq!(p.cave_crust, 4.0);
        assert_eq!(p.dirt_depth, 3.0);
        assert_eq!(p.spawning_distance, 6);
        assert!(p.sync_with_spacetimedb_elevation);
    }

    #[test]
    fn test_surface_height_deterministic_and_seed_variance() {
        let p1 = TerrainParams {
            sync_with_spacetimedb_elevation: false,
            ..Default::default()
        };
        let h1_a = surface_height(&p1, 100.0, -50.0);
        let h1_b = surface_height(&p1, 100.0, -50.0);
        assert_eq!(h1_a, h1_b, "surface_height must be strictly deterministic");

        let p2 = TerrainParams {
            sync_with_spacetimedb_elevation: false,
            seed: 42,
            ..Default::default()
        };
        let h2 = surface_height(&p2, 100.0, -50.0);
        assert_ne!(h1_a, h2, "different seeds must produce different terrain elevations");

        let p3 = TerrainParams {
            sync_with_spacetimedb_elevation: false,
            seed: 9999,
            ..Default::default()
        };
        let h3 = surface_height(&p3, 100.0, -50.0);
        assert_ne!(h1_a, h3);
    }

    #[test]
    fn test_sample_voxel_surface_grass_dirt_stone_and_air() {
        let p = TerrainParams {
            sync_with_spacetimedb_elevation: false,
            ..Default::default()
        };
        let surface_noise = Fbm::<Perlin>::new(p.seed)
            .set_octaves(p.height_octaves)
            .set_frequency(p.height_frequency);
        let cave_noise = Fbm::<Perlin>::new(p.seed.wrapping_add(1))
            .set_frequency(p.cave_frequency);

        let ground_h = surface_height(&p, 0.0, 0.0);
        let surface_y = ground_h.floor() as i32;

        // Voxel above ground height must be Air
        let air_voxel = sample_voxel(&p, &surface_noise, &cave_noise, IVec3::new(0, surface_y + 2, 0));
        assert_eq!(air_voxel, WorldVoxel::Air);

        // Top surface voxel must be Grass
        let grass_voxel = sample_voxel(&p, &surface_noise, &cave_noise, IVec3::new(0, surface_y, 0));
        assert_eq!(grass_voxel, WorldVoxel::Solid(MAT_GRASS));

        // Subsurface voxel within dirt_depth (e.g. 2 blocks below surface) must be Dirt
        let dirt_voxel = sample_voxel(&p, &surface_noise, &cave_noise, IVec3::new(0, surface_y - 2, 0));
        assert_eq!(dirt_voxel, WorldVoxel::Solid(MAT_DIRT));

        // Deep voxel (e.g. 15 blocks below surface with caves disabled) must be Stone
        let mut no_caves_p = p.clone();
        no_caves_p.cave_threshold = 2.0; // disables caves
        let stone_voxel = sample_voxel(&no_caves_p, &surface_noise, &cave_noise, IVec3::new(0, surface_y - 15, 0));
        assert_eq!(stone_voxel, WorldVoxel::Solid(MAT_STONE));
    }

    #[test]
    fn test_cave_crust_preservation() {
        let p = TerrainParams {
            sync_with_spacetimedb_elevation: false,
            cave_threshold: -1.0, // forces cave carve everywhere if allowed
            cave_crust: 4.0,
            ..Default::default()
        };

        let surface_noise = Fbm::<Perlin>::new(p.seed)
            .set_octaves(p.height_octaves)
            .set_frequency(p.height_frequency);
        let cave_noise = Fbm::<Perlin>::new(p.seed.wrapping_add(1))
            .set_frequency(p.cave_frequency);

        let ground_h = surface_height(&p, 0.0, 0.0);
        let surface_y = ground_h.floor() as i32;

        // In the crust region (surface_y - 1), cave carving is blocked, so block must remain solid
        let crust_voxel = sample_voxel(&p, &surface_noise, &cave_noise, IVec3::new(0, surface_y - 1, 0));
        assert!(crust_voxel.is_solid(), "crust layer below surface must not be carved by caves");

        // Below the crust (y < height - cave_crust), cave carving is active
        let deep_voxel = sample_voxel(&p, &surface_noise, &cave_noise, IVec3::new(0, surface_y - 6, 0));
        assert_eq!(deep_voxel, WorldVoxel::Air, "below crust, cave carving should carve to air");
    }

    #[test]
    fn test_get_terrain_height_matches_surface_height() {
        let p = get_default_terrain_params();
        let sh = surface_height(p, 16.0, 32.0) as f32;
        let gh = get_terrain_height(16.0, 32.0);
        assert!((sh - gh).abs() < 1e-4, "get_terrain_height must match surface_height");
    }

    #[test]
    fn test_mesh_low_poly_terrain_chunk_generation() {
        let db_chunks = BTreeMap::new();
        let mesh = mesh_low_poly_terrain_chunk(&db_chunks, 0, 0);
        assert!(mesh.is_some(), "low-poly chunk mesh must generate successfully");
        let mesh = mesh.unwrap();

        // 16x16 quads = 256 quads = 512 triangles = 1536 unshared vertices
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).expect("positions must exist");
        assert_eq!(positions.len(), 1536, "low-poly mesh must have 1536 vertices for 512 unshared triangles");

        let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).expect("normals must exist");
        assert_eq!(normals.len(), 1536, "flat face normals must exist for every vertex");

        let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR).expect("colors must exist");
        assert_eq!(colors.len(), 1536, "stylized vertex colors must exist for low-poly rendering");
    }

    #[test]
    fn test_mesh_low_poly_terrain_chunk_excavated_staggered_cuboids() {
        let mut db_chunks = BTreeMap::new();
        // Create an excavated chunk at scx=0, scy=0, scz=0 (sub-cell 0,0 of 16m chunk 0,0)
        let key = pack_chunk_key(0, 0, 0);
        let mut voxels = vec![2u8; 4096]; // Solid Stone
        // Carve an excavated cavity in the center: 4x8x4 voxels (a rock chunk corridor)
        for ly in 4..12 {
            for lz in 6..10 {
                for lx in 6..10 {
                    let idx = lx + ly * 16 + lz * 256;
                    voxels[idx] = 0; // Air
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

        let mesh = mesh_low_poly_terrain_chunk(&db_chunks, 0, 0);
        assert!(mesh.is_some(), "excavated chunk with staggered cuboids must mesh successfully");
        let mesh = mesh.unwrap();

        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).expect("positions must exist");
        assert!(!positions.is_empty(), "excavated cavity must emit boundary faces");

        let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).expect("normals must exist");
        assert_eq!(normals.len(), positions.len());

        let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR).expect("colors must exist");
        assert_eq!(colors.len(), positions.len());
    }

    #[test]
    fn test_low_poly_chunk_priority_sorting() {
        let mut candidates = vec![
            (5, 5, 50, 0u64),
            (0, 1, 1, 1u64),
            (0, 0, 0, 2u64),
            (2, 2, 8, 3u64),
            (1, 0, 1, 4u64),
        ];
        candidates.sort_by_key(|c| c.2);

        assert_eq!(candidates[0].2, 0, "chunk at player origin (dist_sq = 0) must be first priority");
        assert_eq!(candidates[1].2, 1, "immediate cardinal neighbor must be next priority");
        assert_eq!(candidates[2].2, 1);
        assert_eq!(candidates[3].2, 8);
        assert_eq!(candidates[4].2, 50, "distant chunk must be last priority");
    }
}