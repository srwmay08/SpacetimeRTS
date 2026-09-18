use bevy::prelude::{Transform as BevyTransform, *};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::RenderLayers;
use avian3d::prelude::*;
use noise::{NoiseFn, Perlin};

use crate::core::*;
use crate::components::*;

// ----------------------------------------------------------------------------
// PROCEDURAL TERRAIN & INFINITE CHUNK GENERATION
// ----------------------------------------------------------------------------

pub fn get_terrain_height(x: f32, z: f32) -> f32 {
    let scale = 0.015; 
    let base_height_amp = 18.0; 
    let noise_elevation = Perlin::new(42); 

    let nx = x as f64 * scale; 
    let nz = z as f64 * scale;

    let mut elevation = noise_elevation.get([nx, nz]) * 0.6
        + noise_elevation.get([nx * 2.0, nz * 2.0]) * 0.3
        + noise_elevation.get([nx * 4.0, nz * 4.0]) * 0.1;
    
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

pub fn spawn_initial_world(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_res: ResMut<GeneratedChunks>,
) {
    for cz in -1..=1 {
        for cx in -1..=1 {
            spawn_terrain_chunk(&mut commands, &mut meshes, &mut materials, &mut chunk_res, cx, cz);
        }
    }

    let pet_x = 5.0; let pet_z = 5.0;
    // Architectural Note: Raised spawn elevation to prevent narrow-phase overlap clipping on steep slopes.
    let pet_y = get_terrain_height(pet_x, pet_z) + 3.0; 
    
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(0.5).mesh()),
            material: materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.4, 0.1), ..default() }),
            transform: BevyTransform::from_xyz(pet_x, pet_y, pet_z),
            ..default()
        },
        RigidBody::Dynamic, Collider::sphere(0.5), LockedAxes::ROTATION_LOCKED, GravityScale(4.5),
        LinearVelocity::ZERO,
        CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Terrain, GameLayer::Unit, GameLayer::Environment]),
        Selectable, Name::new("Loki"), 
    )).with_children(|parent| {
        parent.spawn((
            PbrBundle {
                mesh: meshes.add(Torus::new(0.7, 0.05)),
                material: materials.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() }),
                transform: BevyTransform::from_xyz(0.0, -0.4, 0.0), visibility: Visibility::Hidden, ..default()
            },
            RenderLayers::layer(2), SelectionRing,
        ));
    });
}

pub fn update_infinite_terrain_chunks(
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_res: ResMut<GeneratedChunks>,
) {
    let Ok(player_transform) = player_query.get_single() else { return; };
    
    let chunk_size = 100.0;
    let p_chunk_x = (player_transform.translation.x / chunk_size).floor() as i32;
    let p_chunk_z = (player_transform.translation.z / chunk_size).floor() as i32;

    for cz in (p_chunk_z - 1)..=(p_chunk_z + 1) {
        for cx in (p_chunk_x - 1)..=(p_chunk_x + 1) {
            if !chunk_res.chunks.contains(&(cx, cz)) {
                spawn_terrain_chunk(&mut commands, &mut meshes, &mut materials, &mut chunk_res, cx, cz);
            }
        }
    }
}

pub fn spawn_terrain_chunk(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    chunk_res: &mut ResMut<GeneratedChunks>,
    chunk_x: i32,
    chunk_z: i32,
) {
    chunk_res.chunks.insert((chunk_x, chunk_z));

    let size = 50; 
    let chunk_scale = 100.0;
    let offset_x = chunk_x as f32 * chunk_scale;
    let offset_z = chunk_z as f32 * chunk_scale;

    let mut positions: Vec<[f32; 3]> = Vec::new(); 
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new(); 
    let mut indices: Vec<u32> = Vec::new();

    for z in 0..=size {
        for x in 0..=size {
            let px = offset_x + (x as f32 / size as f32) * chunk_scale - chunk_scale / 2.0;
            let pz = offset_z + (z as f32 / size as f32) * chunk_scale - chunk_scale / 2.0;
            let y = get_terrain_height(px, pz);

            let color = if y < 3.0 { [0.7, 0.6, 0.4, 1.0] } 
                        else if y > 12.0 { [0.9, 0.9, 0.9, 1.0] } 
                        else { [0.2, 0.5, 0.2, 1.0] };
            
            positions.push([px, y, pz]); 
            uvs.push([x as f32 / size as f32, z as f32 / size as f32]); 
            colors.push(color);
        }
    }

    for z in 0..size {
        for x in 0..size {
            let i0 = z * (size + 1) + x; let i1 = z * (size + 1) + (x + 1);
            let i2 = (z + 1) * (size + 1) + x; let i3 = (z + 1) * (size + 1) + (x + 1);
            indices.push(i0 as u32); indices.push(i2 as u32); indices.push(i1 as u32);
            indices.push(i1 as u32); indices.push(i2 as u32); indices.push(i3 as u32);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions); 
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors); 
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();

    commands.spawn((
        PbrBundle { 
            mesh: meshes.add(mesh.clone()), 
            material: materials.add(StandardMaterial { base_color: Color::srgb(1.0, 1.0, 1.0), perceptual_roughness: 0.85, reflectance: 0.05, ..default() }), 
            ..default() 
        },
        RigidBody::Static, Collider::trimesh_from_mesh(&mesh).expect("Failed to generate TriMesh chunk collider from procedural mesh"),
        // Architectural Note: Removed the massive Environment layer from the mask. 
        // Terrain now only processes collisions for Default (raycasts) and Units (Players/Pets).
        CollisionLayers::new([GameLayer::Terrain], [GameLayer::Default, GameLayer::Unit]),
        TerrainChunk { chunk_x, chunk_z },
    ));

    // Architectural Note: Removed the loop that generated hundreds of decorative local tree/rock colliders 
    // to strictly preserve the physics budget for the 800+ authoritative SpacetimeDB nodes.
}