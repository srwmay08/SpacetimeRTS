// ----------------------------------------------------------------------------
// VOXEL WORLD & DESTRUCTION MODULE (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Represents the server-authoritative voxel volumetric system.
// Coordinates spherical blast excavation, structural support evaluations, and
// continuous physical invalidation checks. Keeps SpacetimeDB compute energy (TeV)
// minimal by operating on packed 64-bit chunk keys with deterministic 16^3 indexing.

use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use noise::{NoiseFn, Perlin};
use crate::movement::player_session;
use crate::CombatEvent;
use crate::combat_event;
use crate::nav_event;

use crate::combat::active_projectile;

/// Chunk dimensions along each orthogonal axis (16x16x16 = 4,096 voxels per chunk).
/// Architectural Note: 16^3 yields exactly 4KB of raw data per chunk, which aligns
/// with SpacetimeDB BSATN serialization limits and efficient WebSocket packet framing.
pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

// Architectural Note: Authoritative Metric Voxel Scale (0.25m / 25cm).
// Defines each discrete voxel cell as 0.25m x 0.25m x 0.25m, aligning the backend
// volume model with the client Surface Nets dual-contouring mesher.
pub const VOXEL_SIZE: f32 = 0.25;

/// Absolute bottom boundary of the world.
/// Bedrock forms an indestructible strata at and below this elevation that players
/// and mining tools cannot dig, excavate, or tunnel through.
pub const BEDROCK_ELEVATION: f32 = -120.0;

/// Voxel density and material categorization.
#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoxelMaterial {
    Air = 0,
    Dirt = 1,
    Stone = 2,
    Sand = 3,
    Wood = 4,
    ReinforcedStone = 5,
    Bedrock = 6,
    IronOre = 7,
    Ruby = 8,
    CollapsedRubble = 9,
}

impl VoxelMaterial {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Dirt,
            2 => Self::Stone,
            3 => Self::Sand,
            4 => Self::Wood,
            5 => Self::ReinforcedStone,
            6 => Self::Bedrock,
            7 => Self::IronOre,
            8 => Self::Ruby,
            9 => Self::CollapsedRubble,
            _ => Self::Air,
        }
    }

    pub fn hardness(&self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Dirt | Self::Sand => 20.0,
            Self::CollapsedRubble => 40.0,
            Self::Wood => 50.0,
            Self::Stone => 100.0,
            Self::IronOre => 120.0,
            Self::Ruby => 150.0,
            Self::ReinforcedStone => 250.0,
            Self::Bedrock => f32::INFINITY,
        }
    }

    pub fn is_solid(&self) -> bool {
        !matches!(self, Self::Air)
    }
}

/// Evaluates procedural rock strata to embed Iron ore veins and deep Ruby crystal pockets
/// via deterministic 3D integer coordinate hashing.
#[inline]
pub fn procedural_stone_or_ore(vx: i32, vy: i32, vz: i32, wy: f32) -> VoxelMaterial {
    let hash = ((vx.wrapping_mul(73856093) ^ vy.wrapping_mul(19349663) ^ vz.wrapping_mul(83492791)) as u32) % 10000;
    if wy <= -50.0 && wy >= -115.0 && hash < 120 {
        VoxelMaterial::Ruby
    } else if wy <= -5.0 && wy >= -75.0 && hash < 450 {
        VoxelMaterial::IronOre
    } else {
        VoxelMaterial::Stone
    }
}

/// Evaluates if 3D procedural coordinates carve out a natural subterranean cavern / hollow chamber.
/// Preserves a 4.0m solid surface crust and stops above bedrock.
#[inline]
pub fn is_cave_air_at(wx: f32, wy: f32, wz: f32, terrain_height: f32) -> bool {
    let cave_crust = 4.0;
    if wy > terrain_height - cave_crust || wy <= BEDROCK_ELEVATION + 2.0 {
        return false;
    }
    let cave_noise = Perlin::new(1338);
    let freq = 0.035;
    let sample = cave_noise.get([wx as f64 * freq, wy as f64 * freq, wz as f64 * freq]);
    sample > 0.38
}

/// Architectural Note: Primary key `chunk_key` maps spatial coordinates via
/// a bijective bit-packing scheme. BTree indexes on orthogonal axes enable
/// spatial range subscriptions when clients request localized voxel chunks.
#[table(accessor = voxel_chunk, public)]
#[derive(Clone)]
pub struct VoxelChunk {
    #[primary_key]
    pub chunk_key: u64,
    #[index(btree)]
    pub chunk_x: i32,
    #[index(btree)]
    pub chunk_y: i32,
    #[index(btree)]
    pub chunk_z: i32,
    pub voxels: Vec<u8>,
    pub last_modified_tick: u64,
}

/// Packs 3D chunk coordinates into a 64-bit integer key without collisions.
/// Allocation breakdown: X (24 bits: +/-8.3M chunks), Y (16 bits: +/-32K chunks), Z (24 bits).
#[inline]
pub fn pack_chunk_key(cx: i32, cy: i32, cz: i32) -> u64 {
    let x_bits = (cx as i64 + 0x800000) as u64 & 0xFFFFFF;
    let y_bits = (cy as i64 + 0x8000) as u64 & 0xFFFF;
    let z_bits = (cz as i64 + 0x800000) as u64 & 0xFFFFFF;
    (x_bits << 40) | (y_bits << 24) | z_bits
}

/// Unpacks a 64-bit chunk key back into signed 3D chunk coordinates.
#[inline]
pub fn unpack_chunk_key(key: u64) -> (i32, i32, i32) {
    let x_bits = ((key >> 40) & 0xFFFFFF) as i64 - 0x800000;
    let y_bits = ((key >> 24) & 0xFFFF) as i64 - 0x8000;
    let z_bits = (key & 0xFFFFFF) as i64 - 0x800000;
    (x_bits as i32, y_bits as i32, z_bits as i32)
}

/// Converts local chunk voxel coordinates [0..15] to a contiguous array index.
#[inline]
pub fn local_to_index(lx: usize, ly: usize, lz: usize) -> usize {
    lx + (ly * CHUNK_SIZE) + (lz * CHUNK_SIZE * CHUNK_SIZE)
}

// Architectural Note: Metric World-to-Voxel Transformation.
// Dividing metric world floating point coordinates by `VOXEL_SIZE` (0.25m)
// maps continuous entity and projectile positions into discrete 25cm grid spaces.
pub fn world_to_voxel(wx: f32, wy: f32, wz: f32) -> (i32, i32, i32, usize, usize, usize) {
    let vx = (wx / VOXEL_SIZE).floor() as i32;
    let vy = (wy / VOXEL_SIZE).floor() as i32;
    let vz = (wz / VOXEL_SIZE).floor() as i32;

    let cx = vx.div_euclid(CHUNK_SIZE as i32);
    let cy = vy.div_euclid(CHUNK_SIZE as i32);
    let cz = vz.div_euclid(CHUNK_SIZE as i32);

    let lx = vx.rem_euclid(CHUNK_SIZE as i32) as usize;
    let ly = vy.rem_euclid(CHUNK_SIZE as i32) as usize;
    let lz = vz.rem_euclid(CHUNK_SIZE as i32) as usize;

    (cx, cy, cz, lx, ly, lz)
}

/// Reads a voxel's material safely from a chunk, populating air if the chunk is not loaded.
pub fn get_voxel_at(ctx: &ReducerContext, wx: f32, wy: f32, wz: f32) -> VoxelMaterial {
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(wx, wy, wz);
    let key = pack_chunk_key(cx, cy, cz);

    if let Some(chunk) = ctx.db.voxel_chunk().chunk_key().find(key) {
        let idx = local_to_index(lx, ly, lz);
        if let Some(&mat_byte) = chunk.voxels.get(idx) {
            return VoxelMaterial::from_u8(mat_byte);
        }
    }
    VoxelMaterial::Air
}

// Architectural Note: Procedural Chunk Generation with Scaled Voxel Metric Offsets.
// Each discrete voxel step advances world coordinate elevation queries by `VOXEL_SIZE` (0.25m),
// populating soil and stone strata that align with sub-meter continuous terrain height.
pub fn ensure_or_create_chunk(ctx: &ReducerContext, cx: i32, cy: i32, cz: i32) -> VoxelChunk {
    let key = pack_chunk_key(cx, cy, cz);
    if let Some(chunk) = ctx.db.voxel_chunk().chunk_key().find(key) {
        return chunk;
    }

    let mut voxels = vec![VoxelMaterial::Air as u8; CHUNK_VOLUME];
    let base_voxel_x = cx * CHUNK_SIZE as i32;
    let base_voxel_y = cy * CHUNK_SIZE as i32;
    let base_voxel_z = cz * CHUNK_SIZE as i32;

    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            let wx = (base_voxel_x + lx as i32) as f32 * VOXEL_SIZE;
            let wz = (base_voxel_z + lz as i32) as f32 * VOXEL_SIZE;
            let terrain_height = crate::get_terrain_height(wx, wz);

            for ly in 0..CHUNK_SIZE {
                let wy = (base_voxel_y + ly as i32) as f32 * VOXEL_SIZE;
                let idx = local_to_index(lx, ly, lz);

                if wy <= BEDROCK_ELEVATION {
                    voxels[idx] = VoxelMaterial::Bedrock as u8;
                } else if wy > terrain_height {
                    voxels[idx] = VoxelMaterial::Air as u8;
                } else if wy > terrain_height - 3.0 {
                    voxels[idx] = VoxelMaterial::Dirt as u8;
                } else if is_cave_air_at(wx, wy, wz, terrain_height) {
                    voxels[idx] = VoxelMaterial::Air as u8;
                } else {
                    voxels[idx] = procedural_stone_or_ore(base_voxel_x + lx as i32, base_voxel_y + ly as i32, base_voxel_z + lz as i32, wy) as u8;
                }
            }
        }
    }

    let chunk = VoxelChunk {
        chunk_key: key,
        chunk_x: cx,
        chunk_y: cy,
        chunk_z: cz,
        voxels,
        last_modified_tick: ctx.timestamp.to_micros_since_unix_epoch() as u64,
    };

    ctx.db.voxel_chunk().insert(chunk.clone());
    chunk
}

// Architectural Note: Column Voxel Ground Surface Scan for Locomotion.
// Evaluates the highest solid support elevation directly beneath the point (wx, wy, wz).
// Scans downward through voxel chunks if excavated, falling back to procedural terrain
// above ground and clamping at BEDROCK_ELEVATION underground.
pub fn find_ground_surface_below(ctx: &ReducerContext, wx: f32, wy: f32, wz: f32) -> f32 {
    let terrain_height = crate::get_terrain_height(wx, wz);
    let feet_y = wy - 1.05;
    let start_y = (feet_y + 0.5).min(terrain_height + 2.0);

    let vx = (wx / VOXEL_SIZE).floor() as i32;
    let vz = (wz / VOXEL_SIZE).floor() as i32;
    let cx = vx.div_euclid(CHUNK_SIZE as i32);
    let cz = vz.div_euclid(CHUNK_SIZE as i32);
    let lx = vx.rem_euclid(CHUNK_SIZE as i32) as usize;
    let lz = vz.rem_euclid(CHUNK_SIZE as i32) as usize;

    let start_cy = (start_y / 4.0).floor() as i32;
    let min_cy = (BEDROCK_ELEVATION / 4.0).floor() as i32;

    for cy in (min_cy..=start_cy).rev() {
        let key = pack_chunk_key(cx, cy, cz);
        if let Some(chunk) = ctx.db.voxel_chunk().chunk_key().find(key) {
            for ly in (0..16).rev() {
                let vy = cy * 16 + ly as i32;
                let voxel_top_y = (vy as f32 + 1.0) * VOXEL_SIZE;
                if voxel_top_y <= start_y + 0.1 {
                    let idx = local_to_index(lx, ly, lz);
                    if let Some(&mat_byte) = chunk.voxels.get(idx) {
                        let mat = VoxelMaterial::from_u8(mat_byte);
                        if mat.is_solid() {
                            return voxel_top_y.max(BEDROCK_ELEVATION);
                        }
                    }
                }
            }
        } else {
            let chunk_bottom = cy as f32 * 4.0;
            let chunk_top = (cy as f32 + 1.0) * 4.0;

            if chunk_bottom > terrain_height {
                continue;
            } else if chunk_top <= BEDROCK_ELEVATION {
                return BEDROCK_ELEVATION;
            } else {
                for ly in (0..16).rev() {
                    let vy = cy * 16 + ly as i32;
                    let voxel_top_y = (vy as f32 + 1.0) * VOXEL_SIZE;
                    if voxel_top_y <= start_y + 0.1 {
                        let mat = get_voxel_or_procedural_at_voxel_coords(ctx, vx, vy, vz);
                        if mat.is_solid() {
                            return voxel_top_y.max(BEDROCK_ELEVATION);
                        }
                    }
                }
            }
        }
    }

    BEDROCK_ELEVATION
}

/// Result of an authoritative 3D DDA voxel raymarch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoxelRayHit {
    pub vx: i32,
    pub vy: i32,
    pub vz: i32,
    pub material: VoxelMaterial,
    pub normal: (f32, f32, f32),
    pub distance: f32,
}

/// Evaluates the voxel material at discrete integer voxel coordinates (vx, vy, vz),
/// checking DB chunk tables first, then falling back to deterministic procedural strata.
pub fn get_voxel_or_procedural_at_voxel_coords(
    ctx: &ReducerContext,
    vx: i32,
    vy: i32,
    vz: i32,
) -> VoxelMaterial {
    let cx = vx.div_euclid(CHUNK_SIZE as i32);
    let cy = vy.div_euclid(CHUNK_SIZE as i32);
    let cz = vz.div_euclid(CHUNK_SIZE as i32);
    let lx = vx.rem_euclid(CHUNK_SIZE as i32) as usize;
    let ly = vy.rem_euclid(CHUNK_SIZE as i32) as usize;
    let lz = vz.rem_euclid(CHUNK_SIZE as i32) as usize;

    let key = pack_chunk_key(cx, cy, cz);
    if let Some(chunk) = ctx.db.voxel_chunk().chunk_key().find(key) {
        let idx = local_to_index(lx, ly, lz);
        if let Some(&mat_byte) = chunk.voxels.get(idx) {
            return VoxelMaterial::from_u8(mat_byte);
        }
    }

    let wx = (vx as f32 + 0.5) * VOXEL_SIZE;
    let wy = (vy as f32 + 0.5) * VOXEL_SIZE;
    let wz = (vz as f32 + 0.5) * VOXEL_SIZE;
    let terrain_height = crate::get_terrain_height(wx, wz);

    if wy <= BEDROCK_ELEVATION {
        VoxelMaterial::Bedrock
    } else if wy > terrain_height {
        VoxelMaterial::Air
    } else if wy > terrain_height - 3.0 {
        VoxelMaterial::Dirt
    } else if is_cave_air_at(wx, wy, wz, terrain_height) {
        VoxelMaterial::Air
    } else {
        procedural_stone_or_ore(vx, vy, vz, wy)
    }
}

/// Evaluates the voxel material at metric world coordinates (wx, wy, wz),
/// evaluating loaded DB chunks first, then deterministic procedural strata.
pub fn get_voxel_or_procedural_at(ctx: &ReducerContext, wx: f32, wy: f32, wz: f32) -> VoxelMaterial {
    let vx = (wx / VOXEL_SIZE).floor() as i32;
    let vy = (wy / VOXEL_SIZE).floor() as i32;
    let vz = (wz / VOXEL_SIZE).floor() as i32;
    get_voxel_or_procedural_at_voxel_coords(ctx, vx, vy, vz)
}

/// Checks if a world position is directly supported by or anchored to solid rock or bedrock.
/// Tests below (-0.5m), above (+2.5m ceiling), and 4 cardinal lateral directions (+-1.5m).
pub fn is_anchored_to_solid_voxel(ctx: &ReducerContext, wx: f32, wy: f32, wz: f32) -> bool {
    // Check floor directly below
    if get_voxel_or_procedural_at(ctx, wx, wy - 0.5, wz).is_solid() {
        return true;
    }
    // Check ceiling directly above
    if get_voxel_or_procedural_at(ctx, wx, wy + 2.5, wz).is_solid() {
        return true;
    }
    // Check 4 horizontal walls
    let offsets = [(1.5, 0.0), (-1.5, 0.0), (0.0, 1.5), (0.0, -1.5)];
    for (ox, oz) in offsets {
        if get_voxel_or_procedural_at(ctx, wx + ox, wy, wz + oz).is_solid() {
            return true;
        }
    }
    false
}

// Architectural Note: Amanatides & Woo 3D Digital Differential Analyzer (DDA).
// Traverses the voxel grid ray-step by ray-step in exact volumetric order.
// Runs in O(N) integer and addition arithmetic without square roots or trigonometric
// operations in the inner loop, guaranteeing deterministic, low-energy Wasm reducer execution.
pub fn dda_raymarch_pure<F>(
    ox: f32,
    oy: f32,
    oz: f32,
    dx: f32,
    dy: f32,
    dz: f32,
    max_dist: f32,
    mut query_voxel: F,
) -> Option<VoxelRayHit>
where
    F: FnMut(i32, i32, i32) -> VoxelMaterial,
{
    let dir_len = (dx * dx + dy * dy + dz * dz).sqrt();
    if dir_len < 1e-6 {
        return None;
    }
    let (dir_x, dir_y, dir_z) = (dx / dir_len, dy / dir_len, dz / dir_len);

    let mut vx = (ox / VOXEL_SIZE).floor() as i32;
    let mut vy = (oy / VOXEL_SIZE).floor() as i32;
    let mut vz = (oz / VOXEL_SIZE).floor() as i32;

    // Initial check: if ray origin starts inside solid voxel
    let initial_mat = query_voxel(vx, vy, vz);
    if initial_mat.is_solid() {
        return Some(VoxelRayHit {
            vx,
            vy,
            vz,
            material: initial_mat,
            normal: (-dir_x, -dir_y, -dir_z),
            distance: 0.0,
        });
    }

    let step_x: i32;
    let mut t_max_x: f32;
    let t_delta_x: f32;
    if dir_x > 0.0 {
        step_x = 1;
        let next_boundary_x = (vx + 1) as f32 * VOXEL_SIZE;
        t_max_x = (next_boundary_x - ox) / dir_x;
        t_delta_x = VOXEL_SIZE / dir_x;
    } else if dir_x < 0.0 {
        step_x = -1;
        let next_boundary_x = vx as f32 * VOXEL_SIZE;
        t_max_x = (next_boundary_x - ox) / dir_x;
        t_delta_x = VOXEL_SIZE / -dir_x;
    } else {
        step_x = 0;
        t_max_x = f32::INFINITY;
        t_delta_x = f32::INFINITY;
    }

    let step_y: i32;
    let mut t_max_y: f32;
    let t_delta_y: f32;
    if dir_y > 0.0 {
        step_y = 1;
        let next_boundary_y = (vy + 1) as f32 * VOXEL_SIZE;
        t_max_y = (next_boundary_y - oy) / dir_y;
        t_delta_y = VOXEL_SIZE / dir_y;
    } else if dir_y < 0.0 {
        step_y = -1;
        let next_boundary_y = vy as f32 * VOXEL_SIZE;
        t_max_y = (next_boundary_y - oy) / dir_y;
        t_delta_y = VOXEL_SIZE / -dir_y;
    } else {
        step_y = 0;
        t_max_y = f32::INFINITY;
        t_delta_y = f32::INFINITY;
    }

    let step_z: i32;
    let mut t_max_z: f32;
    let t_delta_z: f32;
    if dir_z > 0.0 {
        step_z = 1;
        let next_boundary_z = (vz + 1) as f32 * VOXEL_SIZE;
        t_max_z = (next_boundary_z - oz) / dir_z;
        t_delta_z = VOXEL_SIZE / dir_z;
    } else if dir_z < 0.0 {
        step_z = -1;
        let next_boundary_z = vz as f32 * VOXEL_SIZE;
        t_max_z = (next_boundary_z - oz) / dir_z;
        t_delta_z = VOXEL_SIZE / -dir_z;
    } else {
        step_z = 0;
        t_max_z = f32::INFINITY;
        t_delta_z = f32::INFINITY;
    }

    let max_steps = ((max_dist / VOXEL_SIZE).ceil() as usize * 3).min(128);

    for _ in 0..max_steps {
        let (current_dist, step_normal) = if t_max_x < t_max_y && t_max_x < t_max_z {
            if t_max_x > max_dist {
                break;
            }
            vx += step_x;
            let d = t_max_x;
            t_max_x += t_delta_x;
            (d, (-step_x as f32, 0.0, 0.0))
        } else if t_max_y < t_max_z {
            if t_max_y > max_dist {
                break;
            }
            vy += step_y;
            let d = t_max_y;
            t_max_y += t_delta_y;
            (d, (0.0, -step_y as f32, 0.0))
        } else {
            if t_max_z > max_dist {
                break;
            }
            vz += step_z;
            let d = t_max_z;
            t_max_z += t_delta_z;
            (d, (0.0, 0.0, -step_z as f32))
        };

        let mat = query_voxel(vx, vy, vz);
        if mat.is_solid() {
            return Some(VoxelRayHit {
                vx,
                vy,
                vz,
                material: mat,
                normal: step_normal,
                distance: current_dist,
            });
        }
    }

    None
}

/// Authoritative 3D DDA voxel raymarch against SpacetimeDB chunk tables and procedural strata.
pub fn dda_raymarch_voxel(
    ctx: &ReducerContext,
    ox: f32,
    oy: f32,
    oz: f32,
    dx: f32,
    dy: f32,
    dz: f32,
    max_dist: f32,
) -> Option<VoxelRayHit> {
    dda_raymarch_pure(ox, oy, oz, dx, dy, dz, max_dist, |vx, vy, vz| {
        get_voxel_or_procedural_at_voxel_coords(ctx, vx, vy, vz)
    })
}

/// Authoritative single-voxel excavation.
/// Mutates the target voxel to Air if tool hardness is sufficient, updates chunk replication
/// timestamps, and emits pathfinding invalidation events.
pub fn mine_single_voxel(
    ctx: &ReducerContext,
    vx: i32,
    vy: i32,
    vz: i32,
    tool_damage: f32,
) -> Result<VoxelMaterial, String> {
    let cx = vx.div_euclid(CHUNK_SIZE as i32);
    let cy = vy.div_euclid(CHUNK_SIZE as i32);
    let cz = vz.div_euclid(CHUNK_SIZE as i32);
    let lx = vx.rem_euclid(CHUNK_SIZE as i32) as usize;
    let ly = vy.rem_euclid(CHUNK_SIZE as i32) as usize;
    let lz = vz.rem_euclid(CHUNK_SIZE as i32) as usize;

    let mut chunk = ensure_or_create_chunk(ctx, cx, cy, cz);
    let idx = local_to_index(lx, ly, lz);
    let current_mat = VoxelMaterial::from_u8(chunk.voxels[idx]);

    if current_mat == VoxelMaterial::Bedrock {
        return Err("Bedrock is indestructible.".to_string());
    }

    if current_mat == VoxelMaterial::Air {
        return Err("Cannot mine air.".to_string());
    }

    if tool_damage < current_mat.hardness() {
        return Err(format!("Insufficient tool damage: {} required for {:?}", current_mat.hardness(), current_mat));
    }

    chunk.voxels[idx] = VoxelMaterial::Air as u8;
    chunk.last_modified_tick = ctx.timestamp.to_micros_since_unix_epoch() as u64;
    ctx.db.voxel_chunk().chunk_key().update(chunk);

    let wx = (vx as f32 + 0.5) * VOXEL_SIZE;
    let wy = (vy as f32 + 0.5) * VOXEL_SIZE;
    let wz = (vz as f32 + 0.5) * VOXEL_SIZE;

    ctx.db.nav_event().insert(crate::NavEvent {
        id: 0,
        min_x: wx - 1.0,
        min_y: wy - 1.0,
        min_z: wz - 1.0,
        max_x: wx + 1.0,
        max_y: wy + 1.0,
        max_z: wz + 1.0,
    });

    crate::building::invalidate_structures_at(ctx, wx, wy, wz);

    evaluate_structural_collapse(ctx, &[(vx, vy, vz)]);

    Ok(current_mat)
}

// Architectural Note: Sub-Meter Voxel Sphere Mutation.
// Bounds testing divides metric explosion radius by `VOXEL_SIZE` and measures
// distances from voxel cell centers `(vx + 0.5) * VOXEL_SIZE` to the detonation epicenter,
// creating spherical blast cavities with smooth, sub-meter resolution.
pub fn mutate_voxel_sphere(
    ctx: &ReducerContext,
    center_x: f32,
    center_y: f32,
    center_z: f32,
    radius: f32,
    blast_damage: f32,
) -> Vec<(i32, i32, i32)> {
    let mut modified_chunk_keys = std::collections::BTreeSet::new();
    let mut invalidated_voxels = Vec::new();

    let min_x = ((center_x - radius) / VOXEL_SIZE).floor() as i32;
    let max_x = ((center_x + radius) / VOXEL_SIZE).ceil() as i32;
    let min_y = ((center_y - radius) / VOXEL_SIZE).floor() as i32;
    let max_y = ((center_y + radius) / VOXEL_SIZE).ceil() as i32;
    let min_z = ((center_z - radius) / VOXEL_SIZE).floor() as i32;
    let max_z = ((center_z + radius) / VOXEL_SIZE).ceil() as i32;

    let rad_sq = radius * radius;

    for vy in min_y..=max_y {
        for vz in min_z..=max_z {
            for vx in min_x..=max_x {
                let dx = (vx as f32 + 0.5) * VOXEL_SIZE - center_x;
                let dy = (vy as f32 + 0.5) * VOXEL_SIZE - center_y;
                let dz = (vz as f32 + 0.5) * VOXEL_SIZE - center_z;
                let dist_sq = dx * dx + dy * dy + dz * dz;

                if dist_sq <= rad_sq {
                    let cx = vx.div_euclid(CHUNK_SIZE as i32);
                    let cy = vy.div_euclid(CHUNK_SIZE as i32);
                    let cz = vz.div_euclid(CHUNK_SIZE as i32);
                    let lx = vx.rem_euclid(CHUNK_SIZE as i32) as usize;
                    let ly = vy.rem_euclid(CHUNK_SIZE as i32) as usize;
                    let lz = vz.rem_euclid(CHUNK_SIZE as i32) as usize;

                    let mut chunk = ensure_or_create_chunk(ctx, cx, cy, cz);
                    let idx = local_to_index(lx, ly, lz);
                    let current_mat = VoxelMaterial::from_u8(chunk.voxels[idx]);

                    if current_mat != VoxelMaterial::Air && current_mat != VoxelMaterial::Bedrock {
                        if blast_damage >= current_mat.hardness() {
                            chunk.voxels[idx] = VoxelMaterial::Air as u8;
                            chunk.last_modified_tick = ctx.timestamp.to_micros_since_unix_epoch() as u64;
                            ctx.db.voxel_chunk().chunk_key().update(chunk);

                            invalidated_voxels.push((vx, vy, vz));
                            modified_chunk_keys.insert((cx, cy, cz));
                        }
                    }
                }
            }
        }
    }

    if !invalidated_voxels.is_empty() {
        ctx.db.nav_event().insert(crate::NavEvent {
            id: 0,
            min_x: center_x - radius - 2.0,
            min_y: center_y - radius - 2.0,
            min_z: center_z - radius - 2.0,
            max_x: center_x + radius + 2.0,
            max_y: center_y + radius + 2.0,
            max_z: center_z + radius + 2.0,
        });

        evaluate_structural_collapse(ctx, &invalidated_voxels);
    }

    modified_chunk_keys.into_iter().collect()
}

// Architectural Note: Structural Collapse Evaluation with 32-Step Search Depth.
// Because voxels are 0.25m, search depth extends to 32 steps, maintaining an identical
// 8.0-meter physical connectivity span before overhangs collapse into falling rubble.
fn evaluate_structural_collapse(ctx: &ReducerContext, removed_voxels: &[(i32, i32, i32)]) {
    let mut check_queue = std::collections::VecDeque::new();
    let mut visited = std::collections::BTreeSet::new();

    for &(rx, ry, rz) in removed_voxels {
        let neighbors = [
            (rx, ry + 1, rz),
            (rx, ry - 1, rz),
            (rx + 1, ry, rz),
            (rx - 1, ry, rz),
            (rx, ry, rz + 1),
            (rx, ry, rz - 1),
        ];

        for n in neighbors {
            if !visited.contains(&n) {
                visited.insert(n);
                check_queue.push_back(n);
            }
        }
    }

    let mut collapsing_voxels = Vec::new();

    while let Some((x, y, z)) = check_queue.pop_front() {
        let wx = (x as f32 + 0.5) * VOXEL_SIZE;
        let wy = (y as f32 + 0.5) * VOXEL_SIZE;
        let wz = (z as f32 + 0.5) * VOXEL_SIZE;
        let mat = get_voxel_at(ctx, wx, wy, wz);
        if !mat.is_solid() || mat == VoxelMaterial::Bedrock {
            continue;
        }

        let mut has_ground_support = false;
        let ground_y = crate::get_terrain_height(wx, wz);

        if wy <= ground_y || y <= 0 {
            has_ground_support = true;
        } else {
            let mut search_visited = std::collections::BTreeSet::new();
            let mut search_queue = std::collections::VecDeque::new();
            search_queue.push_back((x, y, z, 0));
            search_visited.insert((x, y, z));

            while let Some((sx, sy, sz, depth)) = search_queue.pop_front() {
                let swy = (sy as f32 + 0.5) * VOXEL_SIZE;
                if swy <= ground_y || sy <= 0 {
                    has_ground_support = true;
                    break;
                }

                if depth >= 32 {
                    continue;
                }

                let down_neighbors = [
                    (sx, sy - 1, sz),
                    (sx + 1, sy, sz),
                    (sx - 1, sy, sz),
                    (sx, sy, sz + 1),
                    (sx, sy, sz - 1),
                ];

                for dn in down_neighbors {
                    if !search_visited.contains(&dn) {
                        search_visited.insert(dn);
                        let d_wx = (dn.0 as f32 + 0.5) * VOXEL_SIZE;
                        let d_wy = (dn.1 as f32 + 0.5) * VOXEL_SIZE;
                        let d_wz = (dn.2 as f32 + 0.5) * VOXEL_SIZE;
                        let neighbor_mat = get_voxel_at(ctx, d_wx, d_wy, d_wz);
                        if neighbor_mat.is_solid() {
                            search_queue.push_back((dn.0, dn.1, dn.2, depth + 1));
                        }
                    }
                }
            }
        }

        if !has_ground_support {
            collapsing_voxels.push((x, y, z));
        }
    }

    for (cx_v, cy_v, cz_v) in collapsing_voxels {
        let wx = (cx_v as f32 + 0.5) * VOXEL_SIZE;
        let wy = (cy_v as f32 + 0.5) * VOXEL_SIZE;
        let wz = (cz_v as f32 + 0.5) * VOXEL_SIZE;

        let (cx, cy, cz, lx, ly, lz) = world_to_voxel(wx, wy, wz);

        let mut chunk = ensure_or_create_chunk(ctx, cx, cy, cz);
        let idx = local_to_index(lx, ly, lz);
        if chunk.voxels[idx] != VoxelMaterial::Air as u8 {
            chunk.voxels[idx] = VoxelMaterial::Air as u8;
            chunk.last_modified_tick = ctx.timestamp.to_micros_since_unix_epoch() as u64;
            ctx.db.voxel_chunk().chunk_key().update(chunk);

            ctx.db.combat_event().insert(CombatEvent {
                id: 0,
                event_type: "VoxelCollapse".to_string(),
                x: wx,
                y: wy,
                z: wz,
            });
        }

        crate::building::invalidate_structures_at(ctx, wx, wy, wz);
    }
}

// ----------------------------------------------------------------------------
// HIGH-TIER MAGIC & SIEGE DEMOLITION REDUCERS
// ----------------------------------------------------------------------------

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiegeWeaponType {
    CatapultBoulder,
    TrebuchetPayload,
    BallistaBolt,
    BatteringRamImpact,
}

#[derive(SpacetimeType, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagicSpellType {
    MeteorStrike,
    EarthquakeShatter,
    ChainLightningBlast,
    VoidImplosion,
}

/// Reducer for detonating an explosive charge planted at a voxel coordinate.
#[reducer]
pub fn detonate_explosive_charge(
    ctx: &ReducerContext,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    power: f32,
    radius: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active player session.".to_string())?;

    log::debug!(
        "Player {} detonated explosive charge at ({:.1}, {:.1}, {:.1}) with radius {:.1}",
        session.entity_id, pos_x, pos_y, pos_z, radius
    );

    mutate_voxel_sphere(ctx, pos_x, pos_y, pos_z, radius, power);

    ctx.db.combat_event().insert(CombatEvent {
        id: 0,
        event_type: "ExplosionBlast".to_string(),
        x: pos_x,
        y: pos_y,
        z: pos_z,
    });

    Ok(())
}

/// Reducer for launching siege artillery targeting structural assets and voxel fortresses.
#[reducer]
pub fn fire_siege_weapon(
    ctx: &ReducerContext,
    siege_type: SiegeWeaponType,
    origin_x: f32,
    origin_y: f32,
    origin_z: f32,
    dir_x: f32,
    dir_y: f32,
    dir_z: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active player session.".to_string())?;

    let (blast_radius, damage, speed, gravity) = match siege_type {
        SiegeWeaponType::CatapultBoulder => (3.5, 300.0, 35.0, 9.81),
        SiegeWeaponType::TrebuchetPayload => (6.0, 800.0, 45.0, 9.81),
        SiegeWeaponType::BallistaBolt => (1.0, 450.0, 75.0, 3.2),
        SiegeWeaponType::BatteringRamImpact => (1.5, 500.0, 5.0, 0.0),
    };

    let dir_len = (dir_x * dir_x + dir_y * dir_y + dir_z * dir_z).sqrt();
    if dir_len < 0.0001 {
        return Err("Invalid firing vector: near-zero magnitude.".to_string());
    }
    let norm_dir = (dir_x / dir_len, dir_y / dir_len, dir_z / dir_len);

    ctx.db.active_projectile().insert(crate::combat::ActiveProjectile {
        projectile_id: 0,
        shooter_id: session.entity_id,
        kind: match siege_type {
            SiegeWeaponType::CatapultBoulder => crate::combat::ProjectileKind::CatapultRock,
            SiegeWeaponType::TrebuchetPayload => crate::combat::ProjectileKind::TrebuchetShell,
            SiegeWeaponType::BallistaBolt => crate::combat::ProjectileKind::BallistaSpear,
            SiegeWeaponType::BatteringRamImpact => crate::combat::ProjectileKind::Arrow,
        },
        pos_x: origin_x,
        pos_y: origin_y,
        pos_z: origin_z,
        vel_x: norm_dir.0 * speed,
        vel_y: norm_dir.1 * speed,
        vel_z: norm_dir.2 * speed,
        gravity,
        drag: 0.002,
        damage,
        blast_radius,
        start_tick: ctx.timestamp.to_micros_since_unix_epoch() as u64,
        lifetime: 12.0,
    });

    log::debug!(
        "Siege weapon {:?} dispatched by player {} towards ({:.2}, {:.2}, {:.2})",
        siege_type, session.entity_id, norm_dir.0, norm_dir.1, norm_dir.2
    );

    Ok(())
}

/// Reducer for casting high-tier area spells that mutate the voxel grid and shatter foundations.
#[reducer]
pub fn cast_high_tier_magic(
    ctx: &ReducerContext,
    spell_type: MagicSpellType,
    target_x: f32,
    target_y: f32,
    target_z: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active player session.".to_string())?;

    let (radius, damage, event_name) = match spell_type {
        MagicSpellType::MeteorStrike => (5.5, 600.0, "MeteorImpact"),
        MagicSpellType::EarthquakeShatter => (7.0, 400.0, "EarthquakeRift"),
        MagicSpellType::ChainLightningBlast => (3.0, 350.0, "LightningStorm"),
        MagicSpellType::VoidImplosion => (4.5, 750.0, "VoidCollapse"),
    };

    mutate_voxel_sphere(ctx, target_x, target_y, target_z, radius, damage);

    ctx.db.combat_event().insert(CombatEvent {
        id: 0,
        event_type: event_name.to_string(),
        x: target_x,
        y: target_y,
        z: target_z,
    });

    log::debug!(
        "High-tier spell {:?} cast by player {} at ({:.1}, {:.1}, {:.1})",
        spell_type, session.entity_id, target_x, target_y, target_z
    );

    Ok(())
}