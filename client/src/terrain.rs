// ============================================================================
// File: terrain.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL VOXEL & CONTINUOUS TERRAIN SYSTEM (Bevy Engine / Avian3D)
// ----------------------------------------------------------------------------
// Architectural Note: Provides seamless, crack-free terrain meshing by anchoring
// 2D horizontal column chunks at y = 0.0 with analytic central-difference normal
// sampling. Decouples heavy Avian3D trimesh collision generation to a tight 16m
// radius around the player avatar, while rendering visual-only meshes out to 48m.

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::RenderLayers;
use avian3d::prelude::*;
use noise::{NoiseFn, Perlin};
use std::sync::OnceLock;

use spacetimedb_sdk::Table;

use crate::core::*;
use crate::components::*;
use crate::network::{SpacetimeConnection, create_voxel_pet_mesh};
use crate::module_bindings::voxel_chunk_table::VoxelChunkTableAccess;
use crate::module_bindings::VoxelChunk;

pub const VOXEL_CHUNK_SIZE: usize = 16;
pub const VOXEL_SIZE: f32 = 0.25;

static PERLIN: OnceLock<Perlin> = OnceLock::new();

// P0 Fix 1: Terrain height cache to avoid redundant Perlin noise calculations
// Key: (x, z) quantized to 0.25m grid (VOXEL_SIZE), Value: terrain height
static TERRAIN_HEIGHT_CACHE: OnceLock<std::sync::RwLock<std::collections::HashMap<(i32, i32), f32>>> = OnceLock::new();

#[inline]
pub fn get_terrain_height_cache() -> &'static std::sync::RwLock<std::collections::HashMap<(i32, i32), f32>> {
    TERRAIN_HEIGHT_CACHE.get_or_init(|| std::sync::RwLock::new(std::collections::HashMap::with_capacity(10000)))
}

#[inline]
pub fn get_perlin() -> &'static Perlin {
    PERLIN.get_or_init(|| Perlin::new(42))
}

// P0 Fix 1: Cached terrain height lookup — quantizes to 0.25m grid for cache efficiency
pub fn get_terrain_height(x: f32, z: f32) -> f32 {
    // Quantize to 0.25m grid for cache key
    let qx = (x / VOXEL_SIZE).round() as i32;
    let qz = (z / VOXEL_SIZE).round() as i32;
    
    // Try cache first (read lock)
    {
        let cache = get_terrain_height_cache();
        if let Ok(cache) = cache.read() {
            if let Some(&height) = cache.get(&(qx, qz)) {
                return height;
            }
        }
    }
    
    // Cache miss — compute height
    let height = compute_terrain_height(x, z);
    
    // Store in cache (write lock)
    {
        let cache = get_terrain_height_cache();
        if let Ok(mut cache) = cache.write() {
            // Limit cache size to prevent unbounded growth
            if cache.len() > 50000 {
                cache.clear();
            }
            cache.insert((qx, qz), height);
        }
    }
    
    height
}

// Original terrain height computation (renamed for internal use)
fn compute_terrain_height(x: f32, z: f32) -> f32 {
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

pub fn mesh_voxel_chunk_surface_nets(
    db_chunks: &std::collections::HashMap<u64, VoxelChunk>,
    cx: i32,
    cz: i32,
) -> Option<Mesh> {
    let chunk_world_span = VOXEL_CHUNK_SIZE as f32 * VOXEL_SIZE; // 4.0m
    let chunk_base_x = cx as f32 * chunk_world_span;
    let chunk_base_z = cz as f32 * chunk_world_span;

    let grid_dim = 17;
    let num_verts = grid_dim * grid_dim;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(num_verts);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(num_verts);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(num_verts);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(num_verts);
    let mut indices: Vec<u32> = Vec::with_capacity(VOXEL_CHUNK_SIZE * VOXEL_CHUNK_SIZE * 6);

    const DELTA: f32 = 0.15;

    for lz in 0..=16 {
        for lx in 0..=16 {
            let local_x = lx as f32 * VOXEL_SIZE;
            let local_z = lz as f32 * VOXEL_SIZE;
            let wx = chunk_base_x + local_x;
            let wz = chunk_base_z + local_z;
            let mut world_y = get_terrain_height(wx, wz);

            let vx = (wx / VOXEL_SIZE).floor() as i32;
            let vz = (wz / VOXEL_SIZE).floor() as i32;
            let v_cx = vx.div_euclid(VOXEL_CHUNK_SIZE as i32);
            let v_cz = vz.div_euclid(VOXEL_CHUNK_SIZE as i32);
            let v_lx = vx.rem_euclid(VOXEL_CHUNK_SIZE as i32) as usize;
            let v_lz = vz.rem_euclid(VOXEL_CHUNK_SIZE as i32) as usize;

            let surface_cy = (world_y / chunk_world_span).floor() as i32;
            for cy_check in (surface_cy.saturating_sub(1)..=surface_cy).rev() {
                let v_key = pack_chunk_key(v_cx, cy_check, v_cz);
                if let Some(db_chunk) = db_chunks.get(&v_key) {
                    for sly in (0..16).rev() {
                        let idx = v_lx + (sly * VOXEL_CHUNK_SIZE) + (v_lz * VOXEL_CHUNK_SIZE * VOXEL_CHUNK_SIZE);
                        if let Some(&mat_byte) = db_chunk.voxels.get(idx) {
                            if mat_byte == 0 {
                                let air_top = (cy_check as f32 * chunk_world_span) + (sly as f32 * VOXEL_SIZE);
                                if air_top < world_y {
                                    world_y = air_top;
                                }
                            }
                        }
                    }
                }
            }

            positions.push([local_x, world_y, local_z]);

            let h_l = get_terrain_height(wx - DELTA, wz);
            let h_r = get_terrain_height(wx + DELTA, wz);
            let h_d = get_terrain_height(wx, wz - DELTA);
            let h_u = get_terrain_height(wx, wz + DELTA);

            let normal = Vec3::new(h_l - h_r, 2.0 * DELTA, h_d - h_u).normalize_or_zero();
            normals.push(normal.to_array());

            uvs.push([wx * 0.25, wz * 0.25]);

            let color = if world_y < 2.5 && normal.y >= 0.55 {
                [0.76, 0.70, 0.50, 1.0]
            } else if normal.y >= 0.45 {
                [0.26, 0.62, 0.26, 1.0]
            } else if normal.y >= 0.30 {
                let t = (normal.y - 0.30) / 0.15;
                [
                    0.48 * (1.0 - t) + 0.26 * t,
                    0.45 * (1.0 - t) + 0.62 * t,
                    0.40 * (1.0 - t) + 0.26 * t,
                    1.0,
                ]
            } else {
                [0.48, 0.45, 0.42, 1.0]
            };

            colors.push(color);
        }
    }

    for lz in 0..16 {
        for lx in 0..16 {
            let i00 = (lx + lz * grid_dim) as u32;
            let i10 = (lx + 1 + lz * grid_dim) as u32;
            let i01 = (lx + (lz + 1) * grid_dim) as u32;
            let i11 = (lx + 1 + (lz + 1) * grid_dim) as u32;

            indices.extend_from_slice(&[i00, i01, i11, i00, i11, i10]);
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
    mut default_material: Local<Option<Handle<StandardMaterial>>>,
    mut loaded_entities: Local<std::collections::HashMap<u64, (Entity, u64, bool)>>,
    mut last_player_chunk: Local<Option<(i32, i32)>>,
    mut scan_throttle: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let Ok(player_transform) = player_query.get_single() else { return; };

    let timer = scan_throttle.get_or_insert_with(|| Timer::from_seconds(0.08, TimerMode::Repeating));
    let timer_fired = timer.tick(time.delta()).just_finished();

    let mat_handle = default_material.get_or_insert_with(|| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.1,
            ..default()
        })
    }).clone();

    let chunk_world_span = VOXEL_CHUNK_SIZE as f32 * VOXEL_SIZE; // 4.0m
    let p_pos = player_transform.translation;
    let p_cx = (p_pos.x / chunk_world_span).floor() as i32;
    let p_cz = (p_pos.z / chunk_world_span).floor() as i32;

    const RADIUS_H: i32 = 12; // 48m radius
    const RADIUS_H_SQ: f32 = (RADIUS_H * RADIUS_H) as f32;
    const NEAR_COLLIDER_DIST_SQ: f32 = 16.0 * 16.0;
    const FAR_COLLIDER_UNLOAD_SQ: f32 = 20.0 * 20.0;

    let player_moved_chunks = match *last_player_chunk {
        Some(c) => c != (p_cx, p_cz),
        None => true,
    };

    if !timer_fired && !player_moved_chunks {
        return;
    }
    *last_player_chunk = Some((p_cx, p_cz));

    let db_chunks: std::collections::HashMap<u64, VoxelChunk> = conn.db.db.voxel_chunk()
        .iter()
        .map(|c| (c.chunk_key, c))
        .collect();

    loaded_entities.clear();
    for (entity, marker, _, has_col) in chunk_query.iter() {
        loaded_entities.insert(marker.chunk_key, (entity, marker.last_modified_tick, has_col.is_some()));
    }

    for (&key, &(existing_entity, last_tick, has_collider)) in loaded_entities.iter() {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        let chunk_center_x = (cx as f32 + 0.5) * chunk_world_span;
        let chunk_center_z = (cz as f32 + 0.5) * chunk_world_span;
        let dist_sq = (chunk_center_x - p_pos.x).powi(2) + (chunk_center_z - p_pos.z).powi(2);

        let server_mod_tick = db_chunks.get(&key).map(|c| c.last_modified_tick).unwrap_or(0);
        if server_mod_tick > last_tick {
            if let Some(new_mesh) = mesh_voxel_chunk_surface_nets(&db_chunks, cx, cz) {
                let mesh_handle = meshes.add(new_mesh.clone());
                let mut entity_cmds = commands.entity(existing_entity);
                entity_cmds.insert(mesh_handle);
                if dist_sq <= NEAR_COLLIDER_DIST_SQ {
                    if let Some(col) = Collider::trimesh_from_mesh(&new_mesh) {
                        entity_cmds.insert((col, TerrainChunkHasCollider));
                    }
                }
                if let Ok((_, mut marker, _, _)) = chunk_query.get_mut(existing_entity) {
                    marker.last_modified_tick = server_mod_tick;
                }
            }
        }

        if has_collider && dist_sq > FAR_COLLIDER_UNLOAD_SQ {
            commands.entity(existing_entity).remove::<Collider>().remove::<TerrainChunkHasCollider>();
        } else if !has_collider && dist_sq <= NEAR_COLLIDER_DIST_SQ {
            if let Ok((_, _, mesh_handle, _)) = chunk_query.get(existing_entity) {
                if let Some(mesh) = meshes.get(&*mesh_handle) {
                    if let Some(col) = Collider::trimesh_from_mesh(mesh) {
                        commands.entity(existing_entity).insert((col, TerrainChunkHasCollider));
                    }
                }
            }
        }
    }

    let mut candidates: Vec<(i32, i32, f32, u64)> = Vec::with_capacity(192);

    for cz in (p_cz - RADIUS_H)..=(p_cz + RADIUS_H) {
        let dz = (cz - p_cz) as f32;
        for cx in (p_cx - RADIUS_H)..=(p_cx + RADIUS_H) {
            let dx = (cx - p_cx) as f32;
            let dist_sq = dx * dx + dz * dz;
            if dist_sq > RADIUS_H_SQ {
                continue;
            }

            let key = pack_chunk_key(cx, 0, cz);
            if !loaded_entities.contains_key(&key) {
                candidates.push((cx, cz, dist_sq, key));
            }
        }
    }

    candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    const MAX_CHUNKS_PER_FRAME: usize = 16;
    for (cx, cz, dist_sq_chunks, key) in candidates.into_iter().take(MAX_CHUNKS_PER_FRAME) {
        let db_mod_tick = db_chunks.get(&key).map(|c| c.last_modified_tick).unwrap_or(0);
        if let Some(new_mesh) = mesh_voxel_chunk_surface_nets(&db_chunks, cx, cz) {
            let dist_world_sq = dist_sq_chunks * chunk_world_span * chunk_world_span;
            let needs_collider = dist_world_sq <= NEAR_COLLIDER_DIST_SQ;

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

    for (&key, &(entity, _, _)) in loaded_entities.iter() {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        if (cx - p_cx).abs() > RADIUS_H + 1 || (cz - p_cz).abs() > RADIUS_H + 1 {
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
            mesh: meshes.add(create_voxel_pet_mesh()),
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
            RigidBody::Dynamic,
            Collider::cuboid(gib_size, gib_size, gib_size),
            GravityScale(2.5),
            LinearVelocity(Vec3::new(rx * 4.5, ry, rz * 4.5)),
            AngularVelocity(Vec3::new(rx * 14.0, ry * 8.0, rz * 14.0)),
            CollisionLayers::new([GameLayer::Default], [GameLayer::Terrain]),
            Friction::new(0.85),
            Restitution::new(0.15),
            VoxelGib { timer: Timer::from_seconds(1.2 + (i as f32 * 0.03), TimerMode::Once) },
        ));
    }
}

pub fn tick_voxel_gibs(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut VoxelGib)>,
) {
    for (entity, mut gib) in query.iter_mut() {
        if gib.timer.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}