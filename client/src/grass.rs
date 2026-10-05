// ============================================================================
// File: client/src/grass.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL CHUNK-BATCHED LOW-POLY GRASS SYSTEM
// ----------------------------------------------------------------------------
// Architectural Note:
// Implements high-performance, stylized wind-blown grass matching the BlendSwap
// #9440 geometric aesthetic. Batches grass blades into single chunk meshes attached
// as children of active near terrain chunks (<=48m radius, ~25-35 draw calls total).
//
// Key Optimization & Visual Pillars:
// 1. Zero Individual ECS Blade Entities: Each chunk holds a single unified mesh.
// 2. GPU Vertex Wind Waves: Tips sway in a dual-sine wave wind gust via WGSL
//    vertex displacement shader without requiring per-vertex CPU buffer uploads.
// 3. Slope & Elevation Gating: Grass strictly populates fertile meadow facets
//    (normal.y >= 0.72, 2.5m <= y <= 15.5m), avoiding cliffs, riverbeds, and snow.
// 4. Zero Shadow Overhead: Equipped with `NotShadowCaster` to protect the 5
//    cascaded shadow map passes from geometry explosion.
// 5. Automatic Chunk Lifecycle: Attached to terrain chunk entities; despawns
//    automatically with terrain chunks via recursive hierarchy despawning.
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin, NotShadowCaster};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::render::view::RenderLayers;
use std::f32::consts::PI;

use crate::components::{PlayerBody, VoxelChunkMarker};
use crate::terrain::{compute_canonical_terrain_height, TerrainChunkVisual};
use crate::voxel_mesh::Prng;

// ----------------------------------------------------------------------------
// 1. CONFIGURATION & EXTENSION MATERIAL
// ----------------------------------------------------------------------------

/// Configuration resource controlling procedural grass density, distribution, and wind.
#[derive(Resource, Debug, Clone, Reflect)]
pub struct GrassConfig {
    /// Master toggle for the procedural grass system.
    pub enabled: bool,
    /// Number of grass blade tufts generated per 16m terrain chunk (default: 80).
    pub tufts_per_chunk: usize,
    /// Maximum distance from the player in 16m chunk units where grass spawns (default: 3 chunks = 48m).
    pub max_distance_chunks: i32,
    /// Minimum height of an individual grass blade in meters.
    pub blade_height_min: f32,
    /// Maximum height of an individual grass blade in meters.
    pub blade_height_max: f32,
    /// Width of grass blade base in meters.
    pub blade_width: f32,
    /// Wind gust propagation speed.
    pub wind_speed: f32,
    /// Wind tip displacement amplitude in meters.
    pub wind_strength: f32,
    /// Spatial frequency / wavelength scale of wind waves across the world.
    pub wind_frequency: f32,
}

impl Default for GrassConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tufts_per_chunk: 80,
            max_distance_chunks: 3, // 48m radius (covers immediate tactical view)
            blade_height_min: 0.45,
            blade_height_max: 0.70,
            blade_width: 0.08,
            wind_speed: 2.2,
            wind_strength: 0.14,
            wind_frequency: 0.30,
        }
    }
}

/// Material extension enabling procedural vertex-shader wind wave displacement
/// over Bevy's built-in physically based `StandardMaterial`.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GrassExtension {
    /// Uniform buffer: x = wind_speed, y = wind_strength, z = wind_frequency, w = time_seconds.
    #[uniform(100)]
    pub wind_params: Vec4,
}

impl MaterialExtension for GrassExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/grass.wgsl".into()
    }
}

/// Extended PBR material combining Bevy's physical celestial lighting with wind vertex sway.
pub type GrassMaterial = ExtendedMaterial<StandardMaterial, GrassExtension>;

/// Resource caching the shared `Handle<GrassMaterial>` across all chunk grass meshes.
#[derive(Resource, Clone)]
pub struct GrassMaterialHandle(pub Handle<GrassMaterial>);

// ----------------------------------------------------------------------------
// 2. COMPONENT TAGS
// ----------------------------------------------------------------------------

/// Tag component inserted on terrain chunk entities that currently have grass children spawned.
#[derive(Component, Debug, Default)]
pub struct ChunkHasGrass;

/// Tag component marking the grass mesh entity spawned as a child of a terrain chunk.
#[derive(Component, Debug, Default)]
pub struct GrassChildMarker;

// ----------------------------------------------------------------------------
// 3. PROCEDURAL LOW-POLY GRASS MESH GENERATION
// ----------------------------------------------------------------------------

/// Generates a unified low-poly faceted grass tuft mesh for a 16m chunk at `(cx, cz)`.
/// Returns `None` if the chunk contains no valid meadow terrain (e.g. deep water, cliff, or snow).
pub fn generate_chunk_grass_mesh(cx: i32, cz: i32, config: &GrassConfig) -> Option<Mesh> {
    let chunk_world_span = 16.0f32;
    let chunk_base_x = cx as f32 * chunk_world_span;
    let chunk_base_z = cz as f32 * chunk_world_span;

    // Stable deterministic pseudo-random seed unique to this chunk coordinate
    let seed = ((cx as i64 * 73856093) ^ (cz as i64 * 19349663) ^ 0x9E3779B97F4A7C15u64 as i64) as u64;
    let mut rng = Prng::new(seed);

    let max_blades_per_tuft = 3;
    let est_verts_per_blade = 5;
    let est_indices_per_blade = 9;
    let est_total_blades = config.tufts_per_chunk * max_blades_per_tuft;

    let mut positions = Vec::with_capacity(est_total_blades * est_verts_per_blade);
    let mut normals = Vec::with_capacity(est_total_blades * est_verts_per_blade);
    let mut colors = Vec::with_capacity(est_total_blades * est_verts_per_blade);
    let mut uvs = Vec::with_capacity(est_total_blades * est_verts_per_blade);
    let mut indices = Vec::with_capacity(est_total_blades * est_indices_per_blade);

    // Natural meadow grass color gradient (ambient occlusion from ground to sky):
    // Root: Deep jade green / soil contact shadow
    let root_color = [0.18, 0.42, 0.16, 1.0];
    // Mid: Fresh vibrant spring meadow green
    let mid_color = [0.32, 0.68, 0.22, 1.0];
    // Tip: Sunlit chartreuse / golden lime catching stellar rays
    let tip_color = [0.46, 0.82, 0.28, 1.0];

    let mut tufts_placed = 0;

    for _ in 0..config.tufts_per_chunk {
        let lx = rng.range(0.4, 15.6);
        let lz = rng.range(0.4, 15.6);
        let world_x = chunk_base_x + lx;
        let world_z = chunk_base_z + lz;

        let world_y = compute_canonical_terrain_height(world_x, world_z);

        // Evaluate terrain slope via central finite differences:
        let eps = 0.5;
        let h_dx = compute_canonical_terrain_height(world_x + eps, world_z) - compute_canonical_terrain_height(world_x - eps, world_z);
        let h_dz = compute_canonical_terrain_height(world_x, world_z + eps) - compute_canonical_terrain_height(world_x, world_z - eps);
        let normal = Vec3::new(-h_dx, 2.0 * eps, -h_dz).normalize_or_zero();

        // Slope & elevation gating: strictly match meadow grass palette criteria
        // (cliffs normal.y < 0.60, steep dirt normal.y < 0.72, water/beach y < 2.5m, alpine snow y > 15.5m)
        if normal.y < 0.72 || world_y < 2.5 || world_y > 15.5 {
            continue;
        }

        // Spawn a multi-blade grass tuft fanning outward around the root point
        let blade_count = 3;
        let tuft_base_pos = Vec3::new(lx, world_y, lz);

        for b in 0..blade_count {
            let blade_angle = (b as f32 / blade_count as f32) * PI + rng.range(-0.25, 0.25);
            let h = rng.range(config.blade_height_min, config.blade_height_max);
            let w = config.blade_width * rng.range(0.85, 1.15);

            // Width direction vector perpendicular to blade face
            let u_dir = Vec3::new(blade_angle.cos(), 0.0, blade_angle.sin()) * (w * 0.5);
            // Lean direction vector fanning blade outward from center
            let lean_dir = Vec3::new(blade_angle.sin(), 0.0, -blade_angle.cos()) * rng.range(0.08, 0.16);

            // 5-vertex 2-segment tapered blade geometry:
            // V0: Root Left (0% height, 0% wind sway)
            let v0 = tuft_base_pos - u_dir;
            // V1: Root Right (0% height, 0% wind sway)
            let v1 = tuft_base_pos + u_dir;
            // V2: Mid Left (50% height, 50% wind sway)
            let v2 = tuft_base_pos - u_dir * 0.65 + lean_dir * 0.4 + Vec3::Y * (h * 0.5);
            // V3: Mid Right (50% height, 50% wind sway)
            let v3 = tuft_base_pos + u_dir * 0.65 + lean_dir * 0.4 + Vec3::Y * (h * 0.5);
            // V4: Blade Tip (100% height, 100% wind sway)
            let v4 = tuft_base_pos + lean_dir + Vec3::Y * h;

            // Flat face normal for lower segment
            let e1 = v1 - v0;
            let e2 = v2 - v0;
            let n_lower = e1.cross(e2).normalize_or_zero();
            let n_lower_arr = if n_lower.length_squared() > 1e-4 { n_lower.to_array() } else { [0.0, 1.0, 0.0] };

            // Flat face normal for upper tip triangle
            let e3 = v3 - v2;
            let e4 = v4 - v2;
            let n_upper = e3.cross(e4).normalize_or_zero();
            let n_upper_arr = if n_upper.length_squared() > 1e-4 { n_upper.to_array() } else { [0.0, 1.0, 0.0] };

            let start_idx = positions.len() as u32;

            positions.push(v0.to_array());
            positions.push(v1.to_array());
            positions.push(v2.to_array());
            positions.push(v3.to_array());
            positions.push(v4.to_array());

            normals.push(n_lower_arr);
            normals.push(n_lower_arr);
            normals.push(n_lower_arr);
            normals.push(n_upper_arr);
            normals.push(n_upper_arr);

            colors.push(root_color);
            colors.push(root_color);
            colors.push(mid_color);
            colors.push(mid_color);
            colors.push(tip_color);

            // UVs store wind height weight in .y component:
            // root = 0.0 (anchored to ground), mid = 0.5, tip = 1.0 (full wind displacement)
            uvs.push([0.0, 0.0]);
            uvs.push([1.0, 0.0]);
            uvs.push([0.1, 0.5]);
            uvs.push([0.9, 0.5]);
            uvs.push([0.5, 1.0]);

            // Lower quad: (V0, V1, V3) and (V0, V3, V2)
            indices.push(start_idx);
            indices.push(start_idx + 1);
            indices.push(start_idx + 3);

            indices.push(start_idx);
            indices.push(start_idx + 3);
            indices.push(start_idx + 2);

            // Upper triangle: (V2, V3, V4)
            indices.push(start_idx + 2);
            indices.push(start_idx + 3);
            indices.push(start_idx + 4);
        }

        tufts_placed += 1;
    }

    if tufts_placed == 0 || positions.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    Some(mesh)
}

// ----------------------------------------------------------------------------
// 4. ECS SYSTEMS & LIFECYCLE
// ----------------------------------------------------------------------------

/// Startup system: initializes the shared `GrassMaterial` with PBR parameters,
/// double-sided shading, and the procedural WGSL wind extension.
pub fn setup_grass_material(
    mut commands: Commands,
    config: Res<GrassConfig>,
    mut materials: ResMut<Assets<GrassMaterial>>,
) {
    let grass_extension = GrassExtension {
        wind_params: Vec4::new(config.wind_speed, config.wind_strength, config.wind_frequency, 0.0),
    };

    let base_pbr = StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        reflectance: 0.12,
        cull_mode: None, // Double-sided blade visibility
        double_sided: true,
        ..default()
    };

    let grass_material = GrassMaterial {
        base: base_pbr,
        extension: grass_extension,
    };

    let handle = materials.add(grass_material);
    commands.insert_resource(GrassMaterialHandle(handle));
}

/// Updates the wind simulation time uniform across the shared grass material.
pub fn update_grass_wind(
    time: Res<Time>,
    material_handle: Option<Res<GrassMaterialHandle>>,
    materials: Option<ResMut<Assets<GrassMaterial>>>,
) {
    let (Some(handle), Some(mut mats)) = (material_handle, materials) else { return; };
    if let Some(mat) = mats.get_mut(&handle.0) {
        mat.extension.wind_params.w = time.elapsed_seconds();
    }
}

/// Synchronizes grass meshes on near terrain chunks around the player.
/// Spawns grass within 48m (3 chunks) and unloads beyond 64m (4 chunks) with hysteresis.
pub fn sync_chunk_grass(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GrassConfig>,
    material_handle: Option<Res<GrassMaterialHandle>>,
    mut meshes: ResMut<Assets<Mesh>>,
    player_query: Query<&Transform, With<PlayerBody>>,
    unspawned_chunks: Query<(Entity, &VoxelChunkMarker, &Transform), (With<TerrainChunkVisual>, Without<ChunkHasGrass>)>,
    spawned_chunks: Query<(Entity, &Transform, &Children), (With<TerrainChunkVisual>, With<ChunkHasGrass>)>,
    grass_children: Query<Entity, With<GrassChildMarker>>,
    mut scan_timer: Local<Option<Timer>>,
) {
    if !config.enabled {
        // If grass is disabled, remove all grass children and clear markers
        for (chunk_entity, _, children) in spawned_chunks.iter() {
            commands.entity(chunk_entity).remove::<ChunkHasGrass>();
            for &child in children.iter() {
                if grass_children.contains(child) {
                    commands.entity(child).despawn_recursive();
                }
            }
        }
        return;
    }

    let Some(mat_handle) = material_handle else { return; };
    let Ok(player_tf) = player_query.get_single() else { return; };
    let p_pos = player_tf.translation;

    // Throttle scanning to 10 Hz (every 100ms) to ensure zero per-frame CPU overhead
    let timer = scan_timer.get_or_insert_with(|| Timer::from_seconds(0.1, TimerMode::Repeating));
    if !timer.tick(time.delta()).just_finished() {
        return;
    }

    let chunk_world_span = 16.0f32;
    let spawn_radius_meters = config.max_distance_chunks as f32 * chunk_world_span; // 48.0m
    let spawn_radius_sq = spawn_radius_meters * spawn_radius_meters;

    // 16m hysteresis buffer to prevent boundary oscillation
    let unload_radius_meters = spawn_radius_meters + chunk_world_span; // 64.0m
    let unload_radius_sq = unload_radius_meters * unload_radius_meters;

    // 1. Unload grass on distant chunks (>64m)
    for (chunk_entity, chunk_tf, children) in spawned_chunks.iter() {
        let chunk_center_x = chunk_tf.translation.x + chunk_world_span * 0.5;
        let chunk_center_z = chunk_tf.translation.z + chunk_world_span * 0.5;
        let dist_sq = (chunk_center_x - p_pos.x).powi(2) + (chunk_center_z - p_pos.z).powi(2);

        if dist_sq > unload_radius_sq {
            commands.entity(chunk_entity).remove::<ChunkHasGrass>();
            for &child in children.iter() {
                if grass_children.contains(child) {
                    commands.entity(child).despawn_recursive();
                }
            }
        }
    }

    // 2. Spawn grass on candidate near chunks (<=48m)
    for (chunk_entity, marker, chunk_tf) in unspawned_chunks.iter() {
        let chunk_center_x = chunk_tf.translation.x + chunk_world_span * 0.5;
        let chunk_center_z = chunk_tf.translation.z + chunk_world_span * 0.5;
        let dist_sq = (chunk_center_x - p_pos.x).powi(2) + (chunk_center_z - p_pos.z).powi(2);

        if dist_sq <= spawn_radius_sq {
            commands.entity(chunk_entity).insert(ChunkHasGrass);

            if let Some(grass_mesh) = generate_chunk_grass_mesh(marker.chunk_x, marker.chunk_z, &config) {
                let mesh_handle = meshes.add(grass_mesh);
                commands.entity(chunk_entity).with_children(|parent| {
                    parent.spawn((
                        MaterialMeshBundle {
                            mesh: mesh_handle,
                            material: mat_handle.0.clone(),
                            transform: Transform::IDENTITY,
                            ..default()
                        },
                        GrassChildMarker,
                        NotShadowCaster, // Crucial: grass must NOT burden shadow cascades
                        RenderLayers::layer(0),
                        Name::new("Chunk Low-Poly Grass"),
                    ));
                });
            }
        }
    }
}

// ----------------------------------------------------------------------------
// 5. MODULAR BEVY PLUGIN
// ----------------------------------------------------------------------------

/// Modular plugin registering procedural chunk-batched low-poly grass,
/// PBR wind material extension, and spatial near-culling systems.
pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GrassConfig>()
            .add_plugins(MaterialPlugin::<GrassMaterial> {
                prepass_enabled: false,
                shadows_enabled: false,
                ..default()
            })
            .add_systems(Startup, setup_grass_material)
            .add_systems(Update, (update_grass_wind, sync_chunk_grass).chain());
    }
}

// ----------------------------------------------------------------------------
// 6. COMPREHENSIVE UNIT TEST SUITE
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grass_config_defaults_and_invariants() {
        let config = GrassConfig::default();
        assert!(config.enabled);
        assert!(config.tufts_per_chunk >= 40 && config.tufts_per_chunk <= 200);
        assert!(config.max_distance_chunks >= 2 && config.max_distance_chunks <= 6);
        assert!(config.blade_height_min > 0.0);
        assert!(config.blade_height_min < config.blade_height_max);
        assert!(config.wind_speed > 0.0);
        assert!(config.wind_strength > 0.0);
    }

    #[test]
    fn test_grass_material_registration() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.init_asset::<GrassExtension>();
        app.init_asset::<GrassMaterial>();
        app.add_plugins(MaterialPlugin::<GrassMaterial> {
            prepass_enabled: false,
            shadows_enabled: false,
            ..default()
        });
        app.update();
    }

    #[test]
    fn test_generate_chunk_grass_mesh_attributes_and_budget() {
        let config = GrassConfig::default();
        // Chunk (0, 0) is central meadow terrain
        if let Some(mesh) = generate_chunk_grass_mesh(0, 0, &config) {
            assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
            assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
            assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
            assert!(mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
            assert!(mesh.indices().is_some());

            let vert_count = mesh.count_vertices();
            assert!(vert_count >= 100, "Grass mesh should contain multiple blades: {vert_count}");
            assert!(vert_count <= 5000, "Grass mesh exceeds low-poly chunk budget: {vert_count}");

            // Verify UV.y height factor bounds: values must be in [0.0, 1.0]
            use bevy::render::mesh::VertexAttributeValues;
            if let Some(VertexAttributeValues::Float32x2(ref uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
                for uv in uvs {
                    assert!(uv[1] >= 0.0 && uv[1] <= 1.0, "UV.y height factor must be in [0.0, 1.0]");
                }
            } else {
                panic!("Grass mesh must have Float32x2 UV coordinates");
            }
        }
    }

    #[test]
    fn test_chunk_grass_seed_determinism() {
        let config = GrassConfig::default();
        let m1 = generate_chunk_grass_mesh(2, 3, &config);
        let m2 = generate_chunk_grass_mesh(2, 3, &config);

        if let (Some(mesh1), Some(mesh2)) = (m1, m2) {
            let pos1 = mesh1.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
            let pos2 = mesh2.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
            assert_eq!(pos1.len(), pos2.len(), "Identical coordinates must generate identical vertex counts");
            assert_eq!(pos1[0], pos2[0], "Identical coordinates must generate identical positions");
        }
    }
}
