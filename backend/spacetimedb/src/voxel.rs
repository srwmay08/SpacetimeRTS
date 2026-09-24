// ----------------------------------------------------------------------------
// VOXEL WORLD & DESTRUCTION MODULE (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: This module implements a networked voxel data structure
// representing destructible terrain and volumetric fortifications. Voxel chunks
// are spatially hashed into 64-bit keys for single-cycle primary key lookups,
// keeping SpacetimeDB compute energy (TeV) minimal. Explosive charges, siege
// artillery, and high-tier magic mutate these volumes directly, triggering
// neighbor-invalidation checks that induce structural collapse when support is lost.

use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::player_session;
use crate::CombatEvent;
use crate::combat_event;
use crate::nav_event;

// Architectural Note: Bringing the `active_projectile` accessor trait into scope
// is strictly required by SpacetimeDB v2.x so `ctx.db.active_projectile()` can be
// called inside `fire_siege_weapon` without generating an E0599 missing method error.
use crate::combat::active_projectile;

/// Chunk dimensions along each orthogonal axis (16x16x16 = 4,096 voxels per chunk).
/// Architectural Note: 16^3 yields exactly 4KB of raw data per chunk, which aligns
/// with SpacetimeDB BSATN serialization limits and efficient WebSocket packet framing.
pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

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
            _ => Self::Air,
        }
    }

    pub fn hardness(&self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Dirt | Self::Sand => 20.0,
            Self::Wood => 50.0,
            Self::Stone => 100.0,
            Self::ReinforcedStone => 250.0,
            Self::Bedrock => f32::INFINITY,
        }
    }

    pub fn is_solid(&self) -> bool {
        !matches!(self, Self::Air)
    }
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

/// Converts world floating-point coordinates into chunk coordinates and local voxel indices.
pub fn world_to_voxel(wx: f32, wy: f32, wz: f32) -> (i32, i32, i32, usize, usize, usize) {
    let vx = wx.floor() as i32;
    let vy = wy.floor() as i32;
    let vz = wz.floor() as i32;

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

/// Generates a solid terrain chunk populated according to procedural elevation heights.
pub fn ensure_or_create_chunk(ctx: &ReducerContext, cx: i32, cy: i32, cz: i32) -> VoxelChunk {
    let key = pack_chunk_key(cx, cy, cz);
    if let Some(chunk) = ctx.db.voxel_chunk().chunk_key().find(key) {
        return chunk;
    }

    let mut voxels = vec![VoxelMaterial::Air as u8; CHUNK_VOLUME];
    let base_world_x = cx * CHUNK_SIZE as i32;
    let base_world_y = cy * CHUNK_SIZE as i32;
    let base_world_z = cz * CHUNK_SIZE as i32;

    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            let wx = (base_world_x + lx as i32) as f32;
            let wz = (base_world_z + lz as i32) as f32;
            let terrain_height = crate::get_terrain_height(wx, wz);

            for ly in 0..CHUNK_SIZE {
                let wy = (base_world_y + ly as i32) as f32;
                let idx = local_to_index(lx, ly, lz);

                if wy <= 0.0 {
                    voxels[idx] = VoxelMaterial::Bedrock as u8;
                } else if wy <= terrain_height - 3.0 {
                    voxels[idx] = VoxelMaterial::Stone as u8;
                } else if wy <= terrain_height {
                    voxels[idx] = VoxelMaterial::Dirt as u8;
                } else {
                    voxels[idx] = VoxelMaterial::Air as u8;
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

/// Architectural Note: Mutates voxels within a spherical blast zone and executes
/// structural neighbor-invalidation validation. High-tier magic, siege weaponry,
/// and explosive charges invoke this reducer directly.
pub fn mutate_voxel_sphere(
    ctx: &ReducerContext,
    center_x: f32,
    center_y: f32,
    center_z: f32,
    radius: f32,
    blast_damage: f32,
) -> Vec<(i32, i32, i32)> {
    let mut modified_chunk_keys = std::collections::HashSet::new();
    let mut invalidated_voxels = Vec::new();

    let min_x = (center_x - radius).floor() as i32;
    let max_x = (center_x + radius).ceil() as i32;
    let min_y = (center_y - radius).floor() as i32;
    let max_y = (center_y + radius).ceil() as i32;
    let min_z = (center_z - radius).floor() as i32;
    let max_z = (center_z + radius).ceil() as i32;

    let rad_sq = radius * radius;

    for vy in min_y..=max_y {
        for vz in min_z..=max_z {
            for vx in min_x..=max_x {
                let dx = vx as f32 + 0.5 - center_x;
                let dy = vy as f32 + 0.5 - center_y;
                let dz = vz as f32 + 0.5 - center_z;
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

/// Architectural Note: Structural Collapse Evaluation Algorithm.
/// When voxels are hollowed out, voxels directly above and surrounding the void
/// are tested for connection to solid ground. If a cluster loses grounded support,
/// it collapses, cascading damage to attached modular structural pieces.
fn evaluate_structural_collapse(ctx: &ReducerContext, removed_voxels: &[(i32, i32, i32)]) {
    let mut check_queue = std::collections::VecDeque::new();
    let mut visited = std::collections::HashSet::new();

    // Collect direct neighbors of destroyed voxels
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
        let mat = get_voxel_at(ctx, x as f32, y as f32, z as f32);
        if !mat.is_solid() || mat == VoxelMaterial::Bedrock {
            continue;
        }

        // Verify if voxel is anchored to base ground elevation or connected downwards
        let mut has_ground_support = false;
        let ground_y = crate::get_terrain_height(x as f32, z as f32).floor() as i32;

        if y <= ground_y || y <= 0 {
            has_ground_support = true;
        } else {
            // Check downward 6-connectivity support tree (bounded depth of 8)
            let mut search_visited = std::collections::HashSet::new();
            let mut search_queue = std::collections::VecDeque::new();
            search_queue.push_back((x, y, z, 0));
            search_visited.insert((x, y, z));

            while let Some((sx, sy, sz, depth)) = search_queue.pop_front() {
                if sy <= ground_y || sy <= 0 {
                    has_ground_support = true;
                    break;
                }

                if depth >= 8 {
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
                        let neighbor_mat = get_voxel_at(ctx, dn.0 as f32, dn.1 as f32, dn.2 as f32);
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

    // Mutate collapsed voxels to air and trigger collapse visual events
    for (cx_v, cy_v, cz_v) in collapsing_voxels {
        let (cx, cy, cz, lx, ly, lz) = world_to_voxel(cx_v as f32, cy_v as f32, cz_v as f32);
        let key = pack_chunk_key(cx, cy, cz);

        if let Some(mut chunk) = ctx.db.voxel_chunk().chunk_key().find(key) {
            let idx = local_to_index(lx, ly, lz);
            chunk.voxels[idx] = VoxelMaterial::Air as u8;
            chunk.last_modified_tick = ctx.timestamp.to_micros_since_unix_epoch() as u64;
            ctx.db.voxel_chunk().chunk_key().update(chunk);

            ctx.db.combat_event().insert(CombatEvent {
                id: 0,
                event_type: "VoxelCollapse".to_string(),
                x: cx_v as f32 + 0.5,
                y: cy_v as f32 + 0.5,
                z: cz_v as f32 + 0.5,
            });
        }

        // Trigger structural piece collapse if a Foundation or Wall rested on this voxel
        crate::building::invalidate_structures_at(ctx, cx_v as f32, cy_v as f32, cz_v as f32);
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