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

// ----------------------------------------------------------------------------
// CONSTANTS & PROCEDURAL CONFIGURATIONS
// ----------------------------------------------------------------------------
pub const VOXEL_CHUNK_SIZE: usize = 16;
pub const VOXEL_SIZE: f32 = 1.0;

// ----------------------------------------------------------------------------
// GLOBAL NOISE SINGLETON
// ----------------------------------------------------------------------------
static PERLIN: OnceLock<Perlin> = OnceLock::new();

#[inline]
pub fn get_perlin() -> &'static Perlin {
    PERLIN.get_or_init(|| Perlin::new(42))
}

// ----------------------------------------------------------------------------
// PROCEDURAL TERRAIN HEIGHTMAP DENSITY FUNCTION
// ----------------------------------------------------------------------------

pub fn get_terrain_height(x: f32, z: f32) -> f32 {
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
// SURFACE NETS DUAL-CONTOURING SLOPED VOXEL MESHER
// ----------------------------------------------------------------------------

#[inline]
pub fn pack_chunk_key(cx: i32, cy: i32, cz: i32) -> u64 {
    let x_bits = (cx as i64 + 0x800000) as u64 & 0xFFFFFF;
    let y_bits = (cy as i64 + 0x8000) as u64 & 0xFFFF;
    let z_bits = (cz as i64 + 0x800000) as u64 & 0xFFFFFF;
    (x_bits << 40) | (y_bits << 24) | z_bits
}

pub fn mesh_voxel_chunk_surface_nets(
    db_chunks: &std::collections::HashMap<u64, VoxelChunk>,
    cx: i32,
    cy: i32,
    cz: i32,
) -> Option<Mesh> {
    let chunk_base_x = cx * VOXEL_CHUNK_SIZE as i32;
    let chunk_base_y = cy * VOXEL_CHUNK_SIZE as i32;
    let chunk_base_z = cz * VOXEL_CHUNK_SIZE as i32;

    let key = pack_chunk_key(cx, cy, cz);
    let is_in_db = db_chunks.contains_key(&key);

    let mut height_cache = [[0.0f32; 19]; 19];
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;

    for lz in -1..=17 {
        for lx in -1..=17 {
            let wx = chunk_base_x + lx;
            let wz = chunk_base_z + lz;
            let h = get_terrain_height(wx as f32, wz as f32);
            height_cache[(lx + 1) as usize][(lz + 1) as usize] = h;
            if h < min_h { min_h = h; }
            if h > max_h { max_h = h; }
        }
    }

    let chunk_bottom_y = chunk_base_y as f32;
    let chunk_top_y = (chunk_base_y + VOXEL_CHUNK_SIZE as i32) as f32;

    if !is_in_db {
        if chunk_bottom_y > max_h + 1.0 {
            return None;
        }
        if chunk_top_y < min_h - 1.0 {
            return None;
        }
    }

    let is_solid = |lx: i32, ly: i32, lz: i32| -> bool {
        let wx = chunk_base_x + lx;
        let wy = chunk_base_y + ly;
        let wz = chunk_base_z + lz;

        let scx = wx.div_euclid(VOXEL_CHUNK_SIZE as i32);
        let scy = wy.div_euclid(VOXEL_CHUNK_SIZE as i32);
        let scz = wz.div_euclid(VOXEL_CHUNK_SIZE as i32);
        let sample_key = pack_chunk_key(scx, scy, scz);

        if let Some(db_chunk) = db_chunks.get(&sample_key) {
            let slx = wx.rem_euclid(VOXEL_CHUNK_SIZE as i32) as usize;
            let sly = wy.rem_euclid(VOXEL_CHUNK_SIZE as i32) as usize;
            let slz = wz.rem_euclid(VOXEL_CHUNK_SIZE as i32) as usize;
            let idx = slx + (sly * VOXEL_CHUNK_SIZE) + (slz * VOXEL_CHUNK_SIZE * VOXEL_CHUNK_SIZE);
            if let Some(&b) = db_chunk.voxels.get(idx) {
                return b > 0;
            }
        }

        let clamped_x = (lx + 1).clamp(0, 18) as usize;
        let clamped_z = (lz + 1).clamp(0, 18) as usize;
        let th = height_cache[clamped_x][clamped_z];
        (wy as f32) <= th
    };

    const CELL_DIM: usize = 18;
    let mut cell_vertex_map = vec![-1i32; CELL_DIM * CELL_DIM * CELL_DIM];

    let cell_idx = |ccx: i32, ccy: i32, ccz: i32| -> usize {
        let x = (ccx + 1) as usize;
        let y = (ccy + 1) as usize;
        let z = (ccz + 1) as usize;
        x + (y * CELL_DIM) + (z * CELL_DIM * CELL_DIM)
    };

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for c_z in -1..=VOXEL_CHUNK_SIZE as i32 {
        for c_y in -1..=VOXEL_CHUNK_SIZE as i32 {
            for c_x in -1..=VOXEL_CHUNK_SIZE as i32 {
                let s000 = is_solid(c_x, c_y, c_z);
                let s100 = is_solid(c_x + 1, c_y, c_z);
                let s010 = is_solid(c_x, c_y + 1, c_z);
                let s110 = is_solid(c_x + 1, c_y + 1, c_z);
                let s001 = is_solid(c_x, c_y, c_z + 1);
                let s101 = is_solid(c_x + 1, c_y, c_z + 1);
                let s011 = is_solid(c_x, c_y + 1, c_z + 1);
                let s111 = is_solid(c_x + 1, c_y + 1, c_z + 1);

                let mask = (s000 as u32)
                    | ((s100 as u32) << 1)
                    | ((s010 as u32) << 2)
                    | ((s110 as u32) << 3)
                    | ((s001 as u32) << 4)
                    | ((s101 as u32) << 5)
                    | ((s011 as u32) << 6)
                    | ((s111 as u32) << 7);

                if mask == 0 || mask == 0xFF {
                    continue;
                }

                let p000 = Vec3::new(c_x as f32, c_y as f32, c_z as f32) * VOXEL_SIZE;
                let p100 = Vec3::new((c_x + 1) as f32, c_y as f32, c_z as f32) * VOXEL_SIZE;
                let p010 = Vec3::new(c_x as f32, (c_y + 1) as f32, c_z as f32) * VOXEL_SIZE;
                let p110 = Vec3::new((c_x + 1) as f32, (c_y + 1) as f32, c_z as f32) * VOXEL_SIZE;
                let p001 = Vec3::new(c_x as f32, c_y as f32, (c_z + 1) as f32) * VOXEL_SIZE;
                let p101 = Vec3::new((c_x + 1) as f32, c_y as f32, (c_z + 1) as f32) * VOXEL_SIZE;
                let p011 = Vec3::new(c_x as f32, (c_y + 1) as f32, (c_z + 1) as f32) * VOXEL_SIZE;
                let p111 = Vec3::new((c_x + 1) as f32, (c_y + 1) as f32, (c_z + 1) as f32) * VOXEL_SIZE;

                let mut sum_p = Vec3::ZERO;
                let mut edge_count = 0.0f32;

                let mut test_edge = |e1: bool, e2: bool, p1: Vec3, p2: Vec3| {
                    if e1 != e2 {
                        sum_p += (p1 + p2) * 0.5;
                        edge_count += 1.0;
                    }
                };

                test_edge(s000, s100, p000, p100);
                test_edge(s010, s110, p010, p110);
                test_edge(s001, s101, p001, p101);
                test_edge(s011, s111, p011, p111);

                test_edge(s000, s010, p000, p010);
                test_edge(s100, s110, p100, p110);
                test_edge(s001, s011, p001, p011);
                test_edge(s101, s111, p101, p111);

                test_edge(s000, s001, p000, p001);
                test_edge(s100, s101, p100, p101);
                test_edge(s010, s011, p010, p011);
                test_edge(s110, s111, p110, p111);

                if edge_count > 0.0 {
                    let cell_v = sum_p / edge_count;
                    let v_idx = positions.len() as i32;
                    positions.push(cell_v.to_array());
                    cell_vertex_map[cell_idx(c_x, c_y, c_z)] = v_idx;
                }
            }
        }
    }

    if positions.is_empty() {
        return None;
    }

    for z in 0..VOXEL_CHUNK_SIZE as i32 {
        for y in 0..VOXEL_CHUNK_SIZE as i32 {
            for x in 0..VOXEL_CHUNK_SIZE as i32 {
                let s_curr = is_solid(x, y, z);

                // Edge +X
                let s_x = is_solid(x + 1, y, z);
                if s_curr != s_x {
                    let c0 = cell_vertex_map[cell_idx(x, y, z)];
                    let c1 = cell_vertex_map[cell_idx(x, y - 1, z)];
                    let c2 = cell_vertex_map[cell_idx(x, y - 1, z - 1)];
                    let c3 = cell_vertex_map[cell_idx(x, y, z - 1)];

                    if c0 >= 0 && c1 >= 0 && c2 >= 0 && c3 >= 0 {
                        if s_curr {
                            indices.extend_from_slice(&[c0 as u32, c1 as u32, c2 as u32, c0 as u32, c2 as u32, c3 as u32]);
                        } else {
                            indices.extend_from_slice(&[c0 as u32, c2 as u32, c1 as u32, c0 as u32, c3 as u32, c2 as u32]);
                        }
                    }
                }

                // Edge +Y
                let s_y = is_solid(x, y + 1, z);
                if s_curr != s_y {
                    let c0 = cell_vertex_map[cell_idx(x, y, z)];
                    let c1 = cell_vertex_map[cell_idx(x - 1, y, z)];
                    let c2 = cell_vertex_map[cell_idx(x - 1, y, z - 1)];
                    let c3 = cell_vertex_map[cell_idx(x, y, z - 1)];

                    if c0 >= 0 && c1 >= 0 && c2 >= 0 && c3 >= 0 {
                        if s_curr {
                            indices.extend_from_slice(&[c0 as u32, c3 as u32, c2 as u32, c0 as u32, c2 as u32, c1 as u32]);
                        } else {
                            indices.extend_from_slice(&[c0 as u32, c1 as u32, c2 as u32, c0 as u32, c2 as u32, c3 as u32]);
                        }
                    }
                }

                // Edge +Z
                let s_z = is_solid(x, y, z + 1);
                if s_curr != s_z {
                    let c0 = cell_vertex_map[cell_idx(x, y, z)];
                    let c1 = cell_vertex_map[cell_idx(x - 1, y, z)];
                    let c2 = cell_vertex_map[cell_idx(x - 1, y - 1, z)];
                    let c3 = cell_vertex_map[cell_idx(x, y - 1, z)];

                    if c0 >= 0 && c1 >= 0 && c2 >= 0 && c3 >= 0 {
                        if s_curr {
                            indices.extend_from_slice(&[c0 as u32, c1 as u32, c2 as u32, c0 as u32, c2 as u32, c3 as u32]);
                        } else {
                            indices.extend_from_slice(&[c0 as u32, c3 as u32, c2 as u32, c0 as u32, c2 as u32, c1 as u32]);
                        }
                    }
                }
            }
        }
    }

    if indices.is_empty() {
        return None;
    }

    let mut vertex_normals = vec![Vec3::ZERO; positions.len()];
    for chunk_indices in indices.chunks_exact(3) {
        let i0 = chunk_indices[0] as usize;
        let i1 = chunk_indices[1] as usize;
        let i2 = chunk_indices[2] as usize;

        let p0 = Vec3::from_array(positions[i0]);
        let p1 = Vec3::from_array(positions[i1]);
        let p2 = Vec3::from_array(positions[i2]);

        let face_norm = (p1 - p0).cross(p2 - p0);
        vertex_normals[i0] += face_norm;
        vertex_normals[i1] += face_norm;
        vertex_normals[i2] += face_norm;
    }

    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(positions.len());
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(positions.len());
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(positions.len());

    for (i, unnormalized_norm) in vertex_normals.iter().enumerate() {
        let norm = unnormalized_norm.normalize_or_zero();
        normals.push(norm.to_array());

        let p = positions[i];
        let wy = chunk_base_y as f32 + p[1];
        uvs.push([p[0] * 0.25, p[2] * 0.25]);

        let color = if wy < 3.0 && norm.y >= 0.6 {
            [0.72, 0.68, 0.48, 1.0]
        } else if norm.y >= 0.50 {
            [0.24, 0.58, 0.24, 1.0]
        } else if norm.y >= 0.30 {
            let t = (norm.y - 0.30) / 0.20;
            [
                0.46 * (1.0 - t) + 0.24 * t,
                0.38 * (1.0 - t) + 0.58 * t,
                0.28 * (1.0 - t) + 0.24 * t,
                1.0,
            ]
        } else {
            [0.48, 0.45, 0.42, 1.0]
        };

        colors.push(color);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

// ----------------------------------------------------------------------------
// UNIFIED INFINITE VOXEL STREAMING SYSTEM
// ----------------------------------------------------------------------------

pub fn update_infinite_voxel_terrain(
    mut commands: Commands,
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_query: Query<(Entity, &mut VoxelChunkMarker, &mut Handle<Mesh>)>,
    mut default_material: Local<Option<Handle<StandardMaterial>>>,
    mut empty_chunks: Local<std::collections::HashSet<u64>>,
) {
    let Ok(player_transform) = player_query.get_single() else { return; };

    let mat_handle = default_material.get_or_insert_with(|| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.1,
            ..default()
        })
    }).clone();

    let db_chunks: std::collections::HashMap<u64, VoxelChunk> = conn.db.db.voxel_chunk()
        .iter()
        .map(|c| (c.chunk_key, c))
        .collect();

    let mut loaded_entities: std::collections::HashMap<u64, (Entity, u64)> = std::collections::HashMap::new();
    for (entity, marker, _) in chunk_query.iter() {
        loaded_entities.insert(marker.chunk_key, (entity, marker.last_modified_tick));
    }

    let p_pos = player_transform.translation;
    let p_cx = (p_pos.x / (VOXEL_CHUNK_SIZE as f32 * VOXEL_SIZE)).floor() as i32;
    let p_cz = (p_pos.z / (VOXEL_CHUNK_SIZE as f32 * VOXEL_SIZE)).floor() as i32;

    let radius_h = 4;
    let min_cy = -1;
    let max_cy = 2;

    for (&key, &(existing_entity, last_tick)) in &loaded_entities {
        if let Some(db_chunk) = db_chunks.get(&key) {
            if db_chunk.last_modified_tick > last_tick {
                let (cx, cy, cz) = unpack_chunk_key(key);
                if let Some(new_mesh) = mesh_voxel_chunk_surface_nets(&db_chunks, cx, cy, cz) {
                    let new_collider = Collider::trimesh_from_mesh(&new_mesh);
                    let mesh_handle = meshes.add(new_mesh);

                    let mut entity_cmds = commands.entity(existing_entity);
                    entity_cmds.insert(mesh_handle);
                    if let Some(col) = new_collider {
                        entity_cmds.insert(col);
                    }
                    if let Ok((_, mut marker, _)) = chunk_query.get_mut(existing_entity) {
                        marker.last_modified_tick = db_chunk.last_modified_tick;
                    }
                } else {
                    commands.entity(existing_entity).despawn_recursive();
                }
            }
        }
    }

    let mut candidates: Vec<(i32, i32, i32, f32, u64)> = Vec::new();

    for cz in (p_cz - radius_h)..=(p_cz + radius_h) {
        for cx in (p_cx - radius_h)..=(p_cx + radius_h) {
            for cy in min_cy..=max_cy {
                let key = pack_chunk_key(cx, cy, cz);
                if loaded_entities.contains_key(&key) || empty_chunks.contains(&key) {
                    continue;
                }
                let dist_sq = ((cx - p_cx) * (cx - p_cx) + (cz - p_cz) * (cz - p_cz)) as f32;
                candidates.push((cx, cy, cz, dist_sq, key));
            }
        }
    }

    candidates.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));

    let max_chunks_per_frame = 8;
    for (cx, cy, cz, _, key) in candidates.into_iter().take(max_chunks_per_frame) {
        let db_mod_tick = db_chunks.get(&key).map(|c| c.last_modified_tick).unwrap_or(0);
        if let Some(new_mesh) = mesh_voxel_chunk_surface_nets(&db_chunks, cx, cy, cz) {
            let collider = Collider::trimesh_from_mesh(&new_mesh);
            let mesh_handle = meshes.add(new_mesh);

            let chunk_world_x = cx as f32 * VOXEL_CHUNK_SIZE as f32 * VOXEL_SIZE;
            let chunk_world_y = cy as f32 * VOXEL_CHUNK_SIZE as f32 * VOXEL_SIZE;
            let chunk_world_z = cz as f32 * VOXEL_CHUNK_SIZE as f32 * VOXEL_SIZE;

            let mut entity_cmds = commands.spawn((
                PbrBundle {
                    mesh: mesh_handle,
                    material: mat_handle.clone(),
                    transform: BevyTransform::from_xyz(chunk_world_x, chunk_world_y, chunk_world_z),
                    ..default()
                },
                RigidBody::Static,
                CollisionLayers::new([GameLayer::Terrain], [GameLayer::Default, GameLayer::Unit, GameLayer::Environment]),
                VoxelChunkMarker {
                    chunk_key: key,
                    chunk_x: cx,
                    chunk_y: cy,
                    chunk_z: cz,
                    last_modified_tick: db_mod_tick,
                },
            ));

            if let Some(col) = collider {
                entity_cmds.insert(col);
            }
        } else {
            empty_chunks.insert(key);
        }
    }

    for (&key, &(entity, _)) in &loaded_entities {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        if (cx - p_cx).abs() > radius_h + 2 || (cz - p_cz).abs() > radius_h + 2 {
            commands.entity(entity).despawn_recursive();
        }
    }

    empty_chunks.retain(|&key| {
        let (cx, _cy, cz) = unpack_chunk_key(key);
        (cx - p_cx).abs() <= radius_h + 3 && (cz - p_cz).abs() <= radius_h + 3
    });
}

#[inline]
pub fn unpack_chunk_key(key: u64) -> (i32, i32, i32) {
    let x_bits = ((key >> 40) & 0xFFFFFF) as i64 - 0x800000;
    let y_bits = ((key >> 24) & 0xFFFF) as i64 - 0x8000;
    let z_bits = (key & 0xFFFFFF) as i64 - 0x800000;
    (x_bits as i32, y_bits as i32, z_bits as i32)
}

// ----------------------------------------------------------------------------
// INITIAL WORLD SPAWN SYSTEM
// ----------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------
// VOXEL GIB & SPLINTER PARTICLES
// ----------------------------------------------------------------------------

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
        let ry = ((rng_seed >> 32) as u32 % 80) as f32 / 10.0 + 2.0;

        let mat = if i % 3 == 0 { mat_secondary.clone() } else { mat_primary.clone() };
        let offset = Vec3::new(rx * 0.4, i as f32 * 0.12, rz * 0.4);

        // Architectural Note: Voxel Gib Narrow-Phase Collision Fix.
        // Restricting collision mask strictly to `GameLayer::Terrain` prevents
        // bursts of dynamic micro-cubes from reporting narrow-phase overlap
        // collisions against one another on frame 0.
        commands.spawn((
            PbrBundle {
                mesh: cube_mesh.clone(),
                material: mat,
                transform: BevyTransform::from_translation(origin + offset),
                ..default()
            },
            RigidBody::Dynamic,
            Collider::cuboid(gib_size, gib_size, gib_size),
            LinearVelocity(Vec3::new(rx * 6.5, ry, rz * 6.5)),
            AngularVelocity(Vec3::new(rx * 12.0, ry * 6.0, rz * 12.0)),
            CollisionLayers::new([GameLayer::Default], [GameLayer::Terrain]),
            Friction::new(0.7),
            Restitution::new(0.2),
            VoxelGib { timer: Timer::from_seconds(3.0 + (i as f32 * 0.1), TimerMode::Once) },
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