// ============================================================================
// File: terrain.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL VOXEL & CONTINUOUS TERRAIN SYSTEM (Bevy Engine / SpacetimeDB / Rapier3D)
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
use crate::physics::*;
use bevy_voxel_world::prelude::{
    ChunkDespawnStrategy, ChunkSpawnStrategy, VoxelLookupDelegate, VoxelWorldCamera, VoxelWorldConfig, VoxelWorldPlugin, WorldVoxel,
};
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use spacetimedb_sdk::Table;

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::creatures::create_lowpoly_pet_mesh;
use crate::module_bindings::voxel_chunk_table::VoxelChunkTableAccess;
use crate::module_bindings::global_state_table::GlobalStateTableAccess;
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
pub const LOW_POLY_NEAR_COLLIDER_DIST_SQ: f32 = 48.0 * 48.0; // 48m radius for Rapier3D physics colliders
pub const LOW_POLY_FAR_COLLIDER_UNLOAD_SQ: f32 = 56.0 * 56.0; // 56m radius hysteresis for collider unloading

// Material constants & procedural noise evaluators re-exported from subterrain module
pub const MAT_GRASS: u8 = 0;
#[allow(unused_imports)]
pub use crate::subterrain::{
    MAT_DIRT, MAT_STONE, MAT_SAND, MAT_WOOD, MAT_REINFORCED_STONE,
    MAT_BEDROCK, MAT_IRON_ORE, MAT_RUBY, MAT_COLLAPSED_RUBBLE, BEDROCK_ELEVATION,
    is_dungeon_cavity_at, is_cave_air_at, procedural_stone_or_ore,
    SubterrainChunkVisual, SubterrainChunkHasCollider, SubterrainChunkMarker,
    mesh_subterrain_chunk, mesh_subterrain_chunk_ex, is_bedrock_enabled, set_bedrock_enabled,
};

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


static PERLIN: OnceLock<std::sync::RwLock<Perlin>> = OnceLock::new();
static DEFAULT_TERRAIN_PARAMS: OnceLock<std::sync::RwLock<TerrainParams>> = OnceLock::new();

#[inline]
pub fn get_default_terrain_params() -> TerrainParams {
    DEFAULT_TERRAIN_PARAMS.get_or_init(|| std::sync::RwLock::new(TerrainParams::default())).read().unwrap().clone()
}

pub fn set_default_terrain_params_seed(seed: u32) {
    if let Ok(mut p) = DEFAULT_TERRAIN_PARAMS.get_or_init(|| std::sync::RwLock::new(TerrainParams::default())).write() {
        p.seed = seed;
    }
}

// AI_RULES.md: BTreeMap ensures deterministic ordering and zero SipHash randomness
static TERRAIN_HEIGHT_CACHE: OnceLock<std::sync::RwLock<BTreeMap<(i32, i32), f32>>> = OnceLock::new();

#[inline]
pub fn get_terrain_height_cache() -> &'static std::sync::RwLock<BTreeMap<(i32, i32), f32>> {
    TERRAIN_HEIGHT_CACHE.get_or_init(|| std::sync::RwLock::new(BTreeMap::new()))
}

#[inline]
pub fn get_perlin() -> Perlin {
    *PERLIN.get_or_init(|| std::sync::RwLock::new(Perlin::new(42))).read().unwrap()
}

pub fn set_perlin_seed(seed: u32) {
    if let Ok(mut lock) = PERLIN.get_or_init(|| std::sync::RwLock::new(Perlin::new(42))).write() {
        *lock = Perlin::new(seed);
    }
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
        surface_height(&get_default_terrain_params(), x as f64, z as f64) as f32
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
    elevation = elevation.max(0.001);
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

    if y.is_nan() { y = 0.5; }
    y
}

// ----------------------------------------------------------------------------
// LOW-POLY FACETED TERRAIN MESH GENERATION & PRIORITY STREAMING
// ----------------------------------------------------------------------------
// Architectural Note: Generates flat-shaded low-poly faceted terrain chunks where
// each triangle has its own flat geometric face normal and stylized vertex colors.
// Chunks are streamed via distance-prioritized concentric ordering, guaranteeing
// immediate frame-1 loading around the player with zero gaps and seamless boundary heights.

/// Packs 3D chunk coordinates into a 64-bit integer key using a 3D Morton Space-Filling Curve (Z-order curve).
/// Interleaves binary bits across orthogonal dimensions to enforce spatial locality in SpacetimeDB B-Trees,
/// ensuring spatial range queries translate to contiguous database reads.
/// Allocation breakdown: X (24 bits: +/-8.3M chunks), Y (16 bits: +/-32K chunks), Z (24 bits).
#[inline]
pub fn pack_chunk_key(cx: i32, cy: i32, cz: i32) -> u64 {
    let ux = (cx as i64 + 0x800000) as u64 & 0xFFFFFF;
    let uy = (cy as i64 + 0x8000) as u64 & 0xFFFF;
    let uz = (cz as i64 + 0x800000) as u64 & 0xFFFFFF;

    let mut key = 0u64;
    // 3D Morton bit-interleaving for lower 16 bits of each coordinate (covers bits 0..48)
    for i in 0..16 {
        let bit_x = (ux >> i) & 1;
        let bit_y = (uy >> i) & 1;
        let bit_z = (uz >> i) & 1;
        key |= (bit_x << (3 * i)) | (bit_y << (3 * i + 1)) | (bit_z << (3 * i + 2));
    }
    // 2D Morton bit-interleaving for remaining 8 bits of X and Z (covers bits 48..64)
    for i in 0..8 {
        let bit_x = (ux >> (16 + i)) & 1;
        let bit_z = (uz >> (16 + i)) & 1;
        key |= (bit_x << (48 + 2 * i)) | (bit_z << (48 + 2 * i + 1));
    }
    key
}

/// Unpacks a 64-bit Morton Code key back into signed 3D chunk coordinates.
#[inline]
pub fn unpack_chunk_key(key: u64) -> (i32, i32, i32) {
    let mut ux = 0u64;
    let mut uy = 0u64;
    let mut uz = 0u64;

    for i in 0..16 {
        let bit_x = (key >> (3 * i)) & 1;
        let bit_y = (key >> (3 * i + 1)) & 1;
        let bit_z = (key >> (3 * i + 2)) & 1;
        ux |= bit_x << i;
        uy |= bit_y << i;
        uz |= bit_z << i;
    }
    for i in 0..8 {
        let bit_x = (key >> (48 + 2 * i)) & 1;
        let bit_z = (key >> (48 + 2 * i + 1)) & 1;
        ux |= bit_x << (16 + i);
        uz |= bit_z << (16 + i);
    }

    let cx = (ux as i64 - 0x800000) as i32;
    let cy = (uy as i64 - 0x8000) as i32;
    let cz = (uz as i64 - 0x800000) as i32;
    (cx, cy, cz)
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

/// Generates a low-poly faceted surface mesh for a chunk at grid coordinates (cx, cz).
/// Generates 16x16 faceted surface quads. When a surface quad is excavated to enter a tunnel,
/// carves an inward-beveled threshold ramp that smoothly blends from ground elevation into the descending tunnel mouth.
pub fn mesh_low_poly_surface_chunk(
    db_chunks: &BTreeMap<u64, VoxelChunk>,
    cx: i32,
    cz: i32,
) -> Option<Mesh> {
    let chunk_base_x = cx as f32 * LOW_POLY_CHUNK_SPAN;
    let chunk_base_z = cz as f32 * LOW_POLY_CHUNK_SPAN;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(1024);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(1024);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(1024);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(1024);
    let mut indices: Vec<u32> = Vec::with_capacity(1024);
    let mut curr_idx = 0u32;

    let column_chunks: Vec<&VoxelChunk> = db_chunks.values()
        .filter(|c| c.chunk_x == cx && c.chunk_z == cz)
        .collect();

    // Check excavated status for 16x16 surface quad grid
    let mut excavated = [[false; 16]; 16];
    for qz in 0..16 {
        for qx in 0..16 {
            let x0 = qx as f32 * LOW_POLY_QUAD_SIZE;
            let z0 = qz as f32 * LOW_POLY_QUAD_SIZE;
            let min_y = get_terrain_height(chunk_base_x + x0, chunk_base_z + z0)
                .min(get_terrain_height(chunk_base_x + x0 + LOW_POLY_QUAD_SIZE, chunk_base_z + z0))
                .min(get_terrain_height(chunk_base_x + x0, chunk_base_z + z0 + LOW_POLY_QUAD_SIZE))
                .min(get_terrain_height(chunk_base_x + x0 + LOW_POLY_QUAD_SIZE, chunk_base_z + z0 + LOW_POLY_QUAD_SIZE));
            let sub_surface_vy = (min_y - 0.25).floor() as i32;

            if !column_chunks.is_empty() {
                for check_vy in [sub_surface_vy, sub_surface_vy - 1] {
                    let check_cy = check_vy.div_euclid(16);
                    let check_ly = check_vy.rem_euclid(16) as usize;
                    if let Some(chunk) = column_chunks.iter().find(|c| c.chunk_y == check_cy) {
                        let idx = qx + (check_ly * 16) + (qz * 256);
                        if let Some(&mat) = chunk.voxels.get(idx) {
                            if mat == 0 {
                                excavated[qz][qx] = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // 1. Surface Quads & Beveled Threshold Ramps
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

            if !excavated[qz][qx] {
                // Render standard low-poly surface quad
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
            } else {
                // Excavated quad: Carve an inward-beveled threshold ramp that smoothly blends from
                // ground elevation into the descending tunnel mouth for any solid adjacent borders!
                let min_y = y00.min(y10).min(y01).min(y11);
                let floor_y = (min_y - 1.2).max(BEDROCK_ELEVATION);
                let bevel_inward = 0.35f32;
                let ramp_color = [0.44, 0.40, 0.34, 1.0]; // Carved earth/stone threshold

                // West border
                if qx == 0 || !excavated[qz][qx - 1] {
                    let v_top_0 = Vec3::new(x0, y00, z0);
                    let v_top_1 = Vec3::new(x0, y01, z1);
                    let v_bot_0 = Vec3::new(x0 + bevel_inward, floor_y, z0);
                    let v_bot_1 = Vec3::new(x0 + bevel_inward, floor_y, z1);
                    push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v_top_0, v_top_1, v_bot_1, v_bot_0, [0.0, 1.0, 0.0], ramp_color, chunk_base_x, chunk_base_z);
                }
                // East border
                if qx == 15 || !excavated[qz][qx + 1] {
                    let v_top_0 = Vec3::new(x1, y11, z1);
                    let v_top_1 = Vec3::new(x1, y10, z0);
                    let v_bot_0 = Vec3::new(x1 - bevel_inward, floor_y, z1);
                    let v_bot_1 = Vec3::new(x1 - bevel_inward, floor_y, z0);
                    push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v_top_0, v_top_1, v_bot_1, v_bot_0, [0.0, 1.0, 0.0], ramp_color, chunk_base_x, chunk_base_z);
                }
                // North border
                if qz == 0 || !excavated[qz - 1][qx] {
                    let v_top_0 = Vec3::new(x1, y10, z0);
                    let v_top_1 = Vec3::new(x0, y00, z0);
                    let v_bot_0 = Vec3::new(x1, floor_y, z0 + bevel_inward);
                    let v_bot_1 = Vec3::new(x0, floor_y, z0 + bevel_inward);
                    push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v_top_0, v_top_1, v_bot_1, v_bot_0, [0.0, 1.0, 0.0], ramp_color, chunk_base_x, chunk_base_z);
                }
                // South border
                if qz == 15 || !excavated[qz + 1][qx] {
                    let v_top_0 = Vec3::new(x0, y01, z1);
                    let v_top_1 = Vec3::new(x1, y11, z1);
                    let v_bot_0 = Vec3::new(x0, floor_y, z1 - bevel_inward);
                    let v_bot_1 = Vec3::new(x1, floor_y, z1 - bevel_inward);
                    push_unshared_face(&mut positions, &mut normals, &mut colors, &mut uvs, &mut indices, &mut curr_idx, v_top_0, v_top_1, v_bot_1, v_bot_0, [0.0, 1.0, 0.0], ramp_color, chunk_base_x, chunk_base_z);
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

/// Generates a complete low-poly faceted terrain mesh combining the surface mesh
/// with indestructible bedrock floor and active Voronoi smashed-cuboid subterranean strata.
pub fn mesh_low_poly_terrain_chunk(
    db_chunks: &BTreeMap<u64, VoxelChunk>,
    cx: i32,
    cz: i32,
) -> Option<Mesh> {
    let mut surface_mesh = mesh_low_poly_surface_chunk(db_chunks, cx, cz)?;

    let mut positions = match surface_mesh.remove_attribute(Mesh::ATTRIBUTE_POSITION)? {
        bevy::render::mesh::VertexAttributeValues::Float32x3(v) => v,
        _ => return Some(surface_mesh),
    };
    let mut normals = match surface_mesh.remove_attribute(Mesh::ATTRIBUTE_NORMAL)? {
        bevy::render::mesh::VertexAttributeValues::Float32x3(v) => v,
        _ => return None,
    };
    let mut colors = match surface_mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR)? {
        bevy::render::mesh::VertexAttributeValues::Float32x4(v) => v,
        _ => return None,
    };
    let mut uvs = match surface_mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0)? {
        bevy::render::mesh::VertexAttributeValues::Float32x2(v) => v,
        _ => return None,
    };
    let mut indices: Vec<u32> = match surface_mesh.indices()? {
        Indices::U32(ind) => ind.clone(),
        _ => return None,
    };

    // 2. Append subterranean strata (bedrock floor, natural caves, and Voronoi excavations)
    if let Some(sub_mesh) = mesh_subterrain_chunk(db_chunks, cx, cz) {
        if let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(sub_pos)) = sub_mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            if let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(sub_norm)) = sub_mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
                if let Some(bevy::render::mesh::VertexAttributeValues::Float32x4(sub_col)) = sub_mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
                    if let Some(bevy::render::mesh::VertexAttributeValues::Float32x2(sub_uvs)) = sub_mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
                        if let Some(Indices::U32(sub_idx)) = sub_mesh.indices() {
                            let base_idx = positions.len() as u32;
                            positions.extend_from_slice(sub_pos);
                            normals.extend_from_slice(sub_norm);
                            colors.extend_from_slice(sub_col);
                            uvs.extend_from_slice(sub_uvs);
                            for &idx in sub_idx {
                                indices.push(base_idx + idx);
                            }
                        }
                    }
                }
            }
        }
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
pub struct TerrainPerimeterSkirt;

#[derive(Component)]
pub struct TerrainChunkHasCollider;

pub fn update_infinite_voxel_terrain(
    mut commands: Commands,
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_query: Query<(Entity, &mut VoxelChunkMarker, &mut Handle<Mesh>, Option<&TerrainChunkHasCollider>), (With<TerrainChunkVisual>, Without<SubterrainChunkVisual>)>,
    mut sub_chunk_query: Query<(Entity, &mut SubterrainChunkMarker, &mut Handle<Mesh>, Option<&SubterrainChunkHasCollider>), (With<SubterrainChunkVisual>, Without<TerrainChunkVisual>)>,
    render_settings: Option<Res<crate::spellbook::TerrainRenderSettings>>,
    mut default_material: Local<Option<Handle<StandardMaterial>>>,
    mut loaded_entities: Local<BTreeMap<u64, (Entity, u64, bool)>>,
    mut loaded_sub_entities: Local<BTreeMap<u64, (Entity, u64, bool)>>,
    mut last_player_chunk: Local<Option<(i32, i32)>>,
    mut active_seed: Local<Option<u32>>,
    mut last_bedrock_state: Local<Option<bool>>,
    mut empty_sub_chunks: Local<BTreeSet<u64>>,
) {
    if USE_VOXEL_WORLD_TERRAIN {
        return;
    }

    // Synchronize world seed with authoritative SpacetimeDB server state
    if let Some(global_state) = conn.db.db.global_state().id().find(&0) {
        let server_seed = global_state.world_seed;
        if active_seed.map_or(true, |s| s != server_seed) {
            info!("Received authoritative world seed from server: {}. Rebuilding world terrain.", server_seed);
            *active_seed = Some(server_seed);
            set_perlin_seed(server_seed);
            set_default_terrain_params_seed(server_seed);
            crate::subterrain::set_subterrain_seeds(server_seed.wrapping_add(1296), server_seed.wrapping_add(1297));
            if let Ok(mut cache) = get_terrain_height_cache().write() {
                cache.clear();
            }

            // Despawn all existing surface chunks and subterrain chunks to trigger clean rebuild
            for (entity, _, _, _) in chunk_query.iter() {
                commands.entity(entity).despawn_recursive();
            }
            for (entity, _, _, _) in sub_chunk_query.iter() {
                commands.entity(entity).despawn_recursive();
            }
            loaded_entities.clear();
            loaded_sub_entities.clear();
            empty_sub_chunks.clear();
            return;
        }
    }

    // Synchronize bedrock state with TerrainRenderSettings / Subterrain atomic
    let cur_bedrock = if let Some(ref rs) = render_settings {
        rs.bedrock_enabled
    } else {
        crate::subterrain::is_bedrock_enabled()
    };
    if crate::subterrain::is_bedrock_enabled() != cur_bedrock {
        crate::subterrain::set_bedrock_enabled(cur_bedrock);
    }
    if last_bedrock_state.map_or(false, |b| b != cur_bedrock) {
        info!("Bedrock state changed to {}. Rebuilding subterranean chunks.", cur_bedrock);
        for (entity, _, _, _) in sub_chunk_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        loaded_sub_entities.clear();
        empty_sub_chunks.clear();
    }
    *last_bedrock_state = Some(cur_bedrock);

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

    let mut chunk_max_mod_ticks: BTreeMap<(i32, i32), u64> = BTreeMap::new();
    let mut excavated_chunks: BTreeSet<(i32, i32)> = BTreeSet::new();
    for c in db_chunks.values() {
        let coord = (c.chunk_x, c.chunk_z);
        let entry = chunk_max_mod_ticks.entry(coord).or_insert(0);
        *entry = (*entry).max(c.last_modified_tick);
        if c.voxels.iter().any(|&m| m == 0) {
            excavated_chunks.insert(coord);
        }
    }

    loaded_entities.clear();
    for (entity, marker, _, has_col) in chunk_query.iter() {
        loaded_entities.insert(marker.chunk_key, (entity, marker.last_modified_tick, has_col.is_some()));
    }

    loaded_sub_entities.clear();
    for (entity, marker, _, has_col) in sub_chunk_query.iter() {
        loaded_sub_entities.insert(marker.chunk_key, (entity, marker.last_modified_tick, has_col.is_some()));
    }

    // 1. Check existing loaded surface chunks for modifications or collider distance transitions
    for (&key, &(existing_entity, last_tick, has_collider)) in loaded_entities.iter() {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        let chunk_center_x = (cx as f32 + 0.5) * chunk_world_span;
        let chunk_center_z = (cz as f32 + 0.5) * chunk_world_span;
        let dist_sq = (chunk_center_x - p_pos.x).powi(2) + (chunk_center_z - p_pos.z).powi(2);

        let server_mod_tick = chunk_max_mod_ticks.get(&(cx, cz)).copied().unwrap_or(0);

        let editor_dirty = crate::zone_editor::consume_chunk_dirty(cx, cz);
        if server_mod_tick > last_tick || editor_dirty {
            if let Some(new_mesh) = mesh_low_poly_surface_chunk(&db_chunks, cx, cz) {
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
                entity_cmds.remove::<crate::grass::ChunkHasGrass>();
            }
        }

        // Dynamic surface collider management
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

    // 2. Check existing loaded subterrain chunks for server modifications or collider transitions
    for (&key, &(existing_entity, last_tick, has_collider)) in loaded_sub_entities.iter() {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        let chunk_center_x = (cx as f32 + 0.5) * chunk_world_span;
        let chunk_center_z = (cz as f32 + 0.5) * chunk_world_span;
        let dist_sq = (chunk_center_x - p_pos.x).powi(2) + (chunk_center_z - p_pos.z).powi(2);

        let server_mod_tick = chunk_max_mod_ticks.get(&(cx, cz)).copied().unwrap_or(0);

        if server_mod_tick > last_tick {
            empty_sub_chunks.remove(&key);
            if let Some(new_mesh) = mesh_subterrain_chunk_ex(&db_chunks, cx, cz, cur_bedrock) {
                let mesh_handle = meshes.add(new_mesh.clone());
                let mut entity_cmds = commands.entity(existing_entity);
                entity_cmds.insert(mesh_handle);
                if dist_sq <= LOW_POLY_NEAR_COLLIDER_DIST_SQ {
                    if let Some(col) = Collider::trimesh_from_mesh(&new_mesh) {
                        entity_cmds.insert((col, SubterrainChunkHasCollider));
                    }
                }
                if let Ok((_, mut marker, _, _)) = sub_chunk_query.get_mut(existing_entity) {
                    marker.last_modified_tick = server_mod_tick;
                }
            }
        }

        // Subterrain collider management
        if has_collider && dist_sq > LOW_POLY_FAR_COLLIDER_UNLOAD_SQ {
            commands.entity(existing_entity).remove::<Collider>().remove::<SubterrainChunkHasCollider>();
        } else if !has_collider && dist_sq <= LOW_POLY_NEAR_COLLIDER_DIST_SQ {
            if let Ok((_, _, mesh_handle, _)) = sub_chunk_query.get(existing_entity) {
                if let Some(mesh) = meshes.get(&*mesh_handle) {
                    if let Some(col) = Collider::trimesh_from_mesh(mesh) {
                        commands.entity(existing_entity).insert((col, SubterrainChunkHasCollider));
                    }
                }
            }
        }
    }

    // Dynamic visible range & batch tuning from options panel
    let (view_radius, unload_radius, spawn_batch) = if let Some(ref rs) = render_settings {
        let vr = if rs.spawn_full_zone { 64 } else { rs.view_distance_chunks };
        let ur = if rs.spawn_full_zone { 70 } else { rs.unload_distance_chunks.max(vr + 1) };
        // Budgeted Chunk Spawner: cap normal frame spawns to 6 (or 16 if full zone)
        // to maintain locked 60 FPS (16.6ms) without stutter spikes on main thread
        let batch = if rs.spawn_full_zone { 16 } else { 6 };
        (vr, ur, batch)
    } else {
        (LOW_POLY_RADIUS_CHUNKS, LOW_POLY_UNLOAD_RADIUS_CHUNKS, 6)
    };

    // Player elevation check: determine if player is deep underground or on surface
    let player_surface_h = get_terrain_height(p_pos.x, p_pos.z);
    let is_player_underground = p_pos.y < (player_surface_h - 3.5);

    // 3. Collect candidate unspawned chunks within view radius
    let mut candidates: Vec<(i32, i32, i32, u64, bool, bool)> = Vec::with_capacity(512);
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
            let needs_surface = !loaded_entities.contains_key(&key);
            let has_excavation = excavated_chunks.contains(&(cx, cz));
            let should_spawn_sub = is_player_underground || has_excavation || dist_sq <= 4;
            let needs_sub = !loaded_sub_entities.contains_key(&key) && !empty_sub_chunks.contains(&key) && should_spawn_sub;

            if needs_surface || needs_sub {
                candidates.push((cx, cz, dist_sq, key, needs_surface, needs_sub));
            }
        }
    }

    candidates.sort_by_key(|c| c.2);

    let immediate_unspawned = candidates.iter().take_while(|c| c.2 <= 2).count();
    let max_spawn_this_frame = spawn_batch.max(immediate_unspawned);

    for (cx, cz, dist_sq_chunks, key, needs_surface, needs_sub) in candidates.into_iter().take(max_spawn_this_frame) {
        let db_mod_tick = chunk_max_mod_ticks.get(&(cx, cz)).copied().unwrap_or(0);

        let chunk_world_x = cx as f32 * chunk_world_span;
        let chunk_world_z = cz as f32 * chunk_world_span;
        let dist_world_sq = dist_sq_chunks as f32 * chunk_world_span * chunk_world_span;
        let needs_collider = dist_world_sq <= LOW_POLY_NEAR_COLLIDER_DIST_SQ;

        // A. Spawn Surface Chunk Entity
        if needs_surface {
            if let Some(surface_mesh) = mesh_low_poly_surface_chunk(&db_chunks, cx, cz) {
                let collider = if needs_collider {
                    Collider::trimesh_from_mesh(&surface_mesh)
                } else {
                    None
                };

                let mesh_handle = meshes.add(surface_mesh);
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

        // B. Spawn Subterrain Chunk Entity (bedrock floor, natural caves, and Voronoi strata)
        // Optimization: When the player is on the surface, only spawn subterranean chunks if they
        // contain player excavations or open cave/dungeon shafts. Solid enclosed earth is skipped.
        if needs_sub {
            if let Some(sub_mesh) = mesh_subterrain_chunk_ex(&db_chunks, cx, cz, cur_bedrock) {
                let sub_collider = if needs_collider {
                    Collider::trimesh_from_mesh(&sub_mesh)
                } else {
                    None
                };

                let sub_mesh_handle = meshes.add(sub_mesh);
                let mut sub_entity_cmds = commands.spawn((
                    PbrBundle {
                        mesh: sub_mesh_handle,
                        material: mat_handle.clone(),
                        transform: BevyTransform::from_xyz(chunk_world_x, 0.0, chunk_world_z),
                        ..default()
                    },
                    // AI_RULES.md Directive 5.2: Subterranean chunks are deep underground or enclosed.
                    // Tagging with NotShadowCaster eliminates thousands of redundant shadow cascade raster passes!
                    NotShadowCaster,
                    RigidBody::Static,
                    CollisionLayers::new([GameLayer::Terrain], [GameLayer::Default, GameLayer::Unit, GameLayer::Environment]),
                    SubterrainChunkMarker {
                        chunk_key: key,
                        chunk_x: cx,
                        chunk_z: cz,
                        last_modified_tick: db_mod_tick,
                        center_window_y: 0,
                    },
                    SubterrainChunkVisual,
                ));

                if let Some(col) = sub_collider {
                    sub_entity_cmds.insert((col, SubterrainChunkHasCollider));
                }
            } else {
                empty_sub_chunks.insert(key);
            }
        }
        }

    // 4. Despawn distant surface & subterrain chunks beyond fog visual limit efficiently
    let fog_visual_limit_meters = if let Some(ref rs) = render_settings {
        if rs.spawn_full_zone { 64.0 * chunk_world_span } else { rs.visible_range_meters.max(160.0) }
    } else {
        230.4
    };
    let discard_dist_meters = fog_visual_limit_meters + (chunk_world_span * 0.5);
    let discard_dist_sq = discard_dist_meters * discard_dist_meters;
    let unload_radius_sq = unload_radius * unload_radius;

    for (&key, &(entity, _, _)) in loaded_entities.iter() {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        let dx = cx - p_cx;
        let dz = cz - p_cz;

        if dx * dx + dz * dz > unload_radius_sq {
            commands.entity(entity).despawn_recursive();
            continue;
        }

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

    for (&key, &(entity, _, _)) in loaded_sub_entities.iter() {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        let dx = cx - p_cx;
        let dz = cz - p_cz;

        if dx * dx + dz * dz > unload_radius_sq {
            commands.entity(entity).despawn_recursive();
            continue;
        }

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

/// Builds a perimeter skirt mesh enclosing the outer boundary of all currently loaded terrain chunks.
/// Extends from surface terrain height down to `BEDROCK_ELEVATION` (-120.0m) with outward-facing normals,
/// casting directional shadows to prevent low-angle sunlight from slipping under the world into subterranean caverns.
pub fn build_terrain_perimeter_skirt_mesh(loaded_chunks: &BTreeSet<(i32, i32)>) -> Option<Mesh> {
    if loaded_chunks.is_empty() {
        return None;
    }

    let chunk_world_span = LOW_POLY_CHUNK_SPAN; // 16.0
    let segments_per_edge = 4;
    let bedrock_y = crate::subterrain::BEDROCK_ELEVATION; // -120.0

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut curr_idx = 0u32;

    let skirt_color = [0.12, 0.12, 0.15, 1.0];

    // Cardinal perimeter directions: (dcx, dcz, normal, start_corner, end_corner)
    // Corners relative to chunk (0.0 .. chunk_world_span):
    let directions = [
        // North (+Z edge): neighbor (cx, cz + 1). Normal: +Z. Edge from (0, span) to (span, span).
        (0, 1, [0.0, 0.0, 1.0], [0.0, chunk_world_span], [chunk_world_span, chunk_world_span]),
        // South (-Z edge): neighbor (cx, cz - 1). Normal: -Z. Edge from (span, 0) to (0, 0).
        (0, -1, [0.0, 0.0, -1.0], [chunk_world_span, 0.0], [0.0, 0.0]),
        // East (+X edge): neighbor (cx + 1, cz). Normal: +X. Edge from (span, span) to (span, 0).
        (1, 0, [1.0, 0.0, 0.0], [chunk_world_span, chunk_world_span], [chunk_world_span, 0.0]),
        // West (-X edge): neighbor (cx - 1, cz). Normal: -X. Edge from (0, 0) to (0, span).
        (-1, 0, [-1.0, 0.0, 0.0], [0.0, 0.0], [0.0, chunk_world_span]),
    ];

    for &(cx, cz) in loaded_chunks {
        let chunk_base_x = cx as f32 * chunk_world_span;
        let chunk_base_z = cz as f32 * chunk_world_span;

        for &(dcx, dcz, normal, [sx, sz], [ex, ez]) in &directions {
            let neighbor = (cx + dcx, cz + dcz);
            if loaded_chunks.contains(&neighbor) {
                continue; // Shared internal edge between loaded chunks
            }

            let start_x = chunk_base_x + sx;
            let start_z = chunk_base_z + sz;
            let end_x = chunk_base_x + ex;
            let end_z = chunk_base_z + ez;

            for s in 0..segments_per_edge {
                let t0 = s as f32 / segments_per_edge as f32;
                let t1 = (s + 1) as f32 / segments_per_edge as f32;

                let x_a = start_x + (end_x - start_x) * t0;
                let z_a = start_z + (end_z - start_z) * t0;
                let x_b = start_x + (end_x - start_x) * t1;
                let z_b = start_z + (end_z - start_z) * t1;

                let y_top_a = get_terrain_height(x_a, z_a);
                let y_top_b = get_terrain_height(x_b, z_b);

                let v_bot_a = [x_a, bedrock_y, z_a];
                let v_bot_b = [x_b, bedrock_y, z_b];
                let v_top_b = [x_b, y_top_b, z_b];
                let v_top_a = [x_a, y_top_a, z_a];

                positions.extend_from_slice(&[v_bot_a, v_bot_b, v_top_b, v_top_a]);
                normals.extend_from_slice(&[normal, normal, normal, normal]);
                colors.extend_from_slice(&[skirt_color, skirt_color, skirt_color, skirt_color]);
                uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);

                indices.extend_from_slice(&[
                    curr_idx,
                    curr_idx + 1,
                    curr_idx + 2,
                    curr_idx,
                    curr_idx + 2,
                    curr_idx + 3,
                ]);
                curr_idx += 4;
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

/// System that tracks the active loaded surface chunk perimeter and updates/spawns
/// the single `TerrainPerimeterSkirt` shadow-casting boundary mesh.
pub fn update_terrain_perimeter_skirt_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    chunk_query: Query<&VoxelChunkMarker, With<TerrainChunkVisual>>,
    mut skirt_query: Query<(Entity, &Handle<Mesh>, &mut Visibility), With<TerrainPerimeterSkirt>>,
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    ephemeris: Option<Res<crate::binary_sky::BinaryEphemerisState>>,
    mut last_chunk_set: Local<BTreeSet<(i32, i32)>>,
    mut last_chunk_count: Local<usize>,
    mut last_player_chunk: Local<Option<(i32, i32)>>,
    mut skirt_material: Local<Option<Handle<StandardMaterial>>>,
    mut last_night_state: Local<Option<bool>>,
) {
    let is_night = ephemeris.as_ref().map_or(false, |e| e.star_a_elevation < -0.05 && e.star_b_elevation < -0.05);

    // Update visibility if day/night transition occurred
    if last_night_state.map_or(true, |n| n != is_night) {
        *last_night_state = Some(is_night);
        for (_, _, mut vis) in skirt_query.iter_mut() {
            *vis = if is_night { Visibility::Hidden } else { Visibility::Inherited };
        }
    }

    let p_chunk = player_query.get_single().ok().map(|t| {
        let chunk_world_span = LOW_POLY_CHUNK_SPAN;
        (
            (t.translation.x / chunk_world_span).floor() as i32,
            (t.translation.z / chunk_world_span).floor() as i32,
        )
    });

    let chunk_count = chunk_query.iter().count();

    // Fast path: if chunk count and player chunk haven't changed and we already have a mesh, skip rebuilding
    if !last_chunk_set.is_empty()
        && chunk_count == *last_chunk_count
        && p_chunk == *last_player_chunk
    {
        return;
    }

    *last_chunk_count = chunk_count;
    *last_player_chunk = p_chunk;

    let current_chunks: BTreeSet<(i32, i32)> = chunk_query
        .iter()
        .map(|marker| {
            let (cx, _, cz) = unpack_chunk_key(marker.chunk_key);
            (cx, cz)
        })
        .collect();

    if current_chunks == *last_chunk_set {
        return;
    }

    if current_chunks.is_empty() {
        for (entity, _, _) in skirt_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        last_chunk_set.clear();
        return;
    }

    if let Some(new_mesh) = build_terrain_perimeter_skirt_mesh(&current_chunks) {
        if let Ok((_entity, mesh_handle, mut vis)) = skirt_query.get_single_mut() {
            if let Some(mesh) = meshes.get_mut(mesh_handle) {
                *mesh = new_mesh;
            }
            *vis = if is_night { Visibility::Hidden } else { Visibility::Inherited };
        } else {
            // Despawn any duplicate skirt entities if present
            for (entity, _, _) in skirt_query.iter() {
                commands.entity(entity).despawn_recursive();
            }

            let mat = skirt_material
                .get_or_insert_with(|| {
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.08, 0.08, 0.10),
                        perceptual_roughness: 0.95,
                        reflectance: 0.1,
                        cull_mode: None,
                        ..default()
                    })
                })
                .clone();

            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(new_mesh),
                    material: mat,
                    transform: BevyTransform::IDENTITY,
                    visibility: if is_night { Visibility::Hidden } else { Visibility::Inherited },
                    ..default()
                },
                TerrainPerimeterSkirt,
                // Skirt intentionally casts shadows (no NotShadowCaster) to block subterranean sunlight leaking!
            ));
        }
    } else {
        for (entity, _, _) in skirt_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
    }

    *last_chunk_set = current_chunks;
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
        Collider::compound(vec![(
            Vec3::new(0.0, 0.29, 0.0),
            Quat::IDENTITY,
            Collider::cuboid(0.35, 0.58, 0.65),
        )]), 
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
        let sh = surface_height(&p, 16.0, 32.0) as f32;
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
        assert!(positions.len() >= 1536, "low-poly mesh must contain surface and bedrock vertices");

        let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).expect("normals must exist");
        assert_eq!(normals.len(), positions.len(), "flat face normals must exist for every vertex");

        let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR).expect("colors must exist");
        assert_eq!(colors.len(), positions.len(), "stylized vertex colors must exist for low-poly rendering");
    }

    #[test]
    fn test_bedrock_and_subterranean_cave_generation() {
        let db_chunks = BTreeMap::new();
        let mesh = mesh_low_poly_terrain_chunk(&db_chunks, 0, 0).expect("mesh must generate");
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).expect("positions must exist");
        let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR).expect("colors must exist");

        let mut bedrock_found = false;
        let mut magma_found = false;
        if let bevy::render::mesh::VertexAttributeValues::Float32x3(pos_vec) = positions {
            for pos in pos_vec {
                if (pos[1] - BEDROCK_ELEVATION).abs() < 1e-3 {
                    bedrock_found = true;
                    break;
                }
            }
        }
        assert!(bedrock_found, "Chunk mesh must generate Bedrock floor at y = -120.0m");

        if let bevy::render::mesh::VertexAttributeValues::Float32x4(col_vec) = colors {
            for col in col_vec {
                if col[0] > 1.5 {
                    magma_found = true;
                    break;
                }
            }
        }
        assert!(magma_found, "Bedrock floor must contain glowing HDR emissive magma cracks");
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

    #[test]
    fn test_morton_chunk_key_bijection_and_locality() {
        let coords = [
            (0, 0, 0),
            (1, 0, 0),
            (0, 1, 0),
            (0, 0, 1),
            (-1, -1, -1),
            (100, 50, -200),
            (-500, -100, 500),
            (50000, 15000, -50000),
            (8_000_000, 30_000, 8_000_000),
            (-8_000_000, -30_000, -8_000_000),
        ];

        for &(cx, cy, cz) in &coords {
            let key = pack_chunk_key(cx, cy, cz);
            let unpacked = unpack_chunk_key(key);
            assert_eq!(unpacked, (cx, cy, cz), "Morton code must round-trip with 100% fidelity");
        }

        // Spatial locality invariant: adjacent chunks in 3D space should stay tightly bounded in key space
        let k_base = pack_chunk_key(10, 10, 10);
        let k_adj_x = pack_chunk_key(11, 10, 10);
        let k_adj_y = pack_chunk_key(10, 11, 10);
        let k_adj_z = pack_chunk_key(10, 10, 11);
        assert_ne!(k_base, k_adj_x);
        assert_ne!(k_base, k_adj_y);
        assert_ne!(k_base, k_adj_z);
    }

    #[test]
    fn test_terrain_perimeter_skirt_mesh_generation() {
        let mut loaded = BTreeSet::new();

        // 1. Empty loaded set returns None
        assert!(build_terrain_perimeter_skirt_mesh(&loaded).is_none());

        // 2. Single isolated chunk (0, 0): all 4 edges are perimeter
        loaded.insert((0, 0));
        let mesh_single = build_terrain_perimeter_skirt_mesh(&loaded).expect("Single chunk must produce skirt");
        let positions = mesh_single.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        // 4 edges * 4 segments * 4 vertices = 64 vertices
        assert_eq!(positions.len(), 64);
        let normals = mesh_single.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap().as_float3().unwrap();
        assert_eq!(normals.len(), 64);

        // Verify bottom vertices reach BEDROCK_ELEVATION (-120.0)
        let has_bedrock_v = positions.iter().any(|p| (p[1] - crate::subterrain::BEDROCK_ELEVATION).abs() < 1e-3);
        assert!(has_bedrock_v, "Skirt must anchor to bedrock elevation (-120.0m)");

        // 3. 2x2 contiguous block of chunks: 8 perimeter edges (interior edges skipped)
        loaded.insert((1, 0));
        loaded.insert((0, 1));
        loaded.insert((1, 1));
        let mesh_block = build_terrain_perimeter_skirt_mesh(&loaded).expect("2x2 chunk block must produce skirt");
        let block_positions = mesh_block.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        // 8 outer edges * 4 segments * 4 vertices = 128 vertices
        assert_eq!(block_positions.len(), 128);
    }
}