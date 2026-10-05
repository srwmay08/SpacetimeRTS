// ============================================================================
// File: client/src/grass.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL CHUNK-BATCHED LOW-POLY GRASS SYSTEM WITH TWO-RING DISTANCE LOD
// ----------------------------------------------------------------------------
// Architectural Note:
// Implements high-performance, stylized wind-blown grass matching the BlendSwap
// #9440 geometric aesthetic. Batches grass blades into single chunk meshes attached
// as children of active near terrain chunks (<=48m radius, ~25-35 draw calls total).
//
// Key Optimization & Visual Pillars:
// 1. Zero Individual ECS Blade Entities: Each chunk holds a single unified mesh.
// 2. Fanned 4-Blade Star Clumps: Each tuft fans 4 geometric blades outward into
//    distinct quadrants with varied organic curvature and 0.13m blade width,
//    multiplying visual meadow coverage by ~6x over narrow needle blades.
// 3. Two-Ring Distance LOD (High: <=32m, Low: 32m..48m):
//    - Inner Tactical Ring (<=32m): High density (200 tufts / chunk = 800 blades)
//    - Outer Perimeter Ring (32m..48m): Low density (70 tufts / chunk = 280 blades)
//    - Beyond 48m: Zero geometry; voxel terrain green texture provides horizon tint.
//    - Hysteresis Deadband (32m..38m): Prevents ping-ponging across chunk boundaries.
//    - Deterministic PRNG alignment guarantees that the first 70 tufts occupy
//      identical coordinates in both LODs, preventing visual pop during transitions.
// 4. GPU Vertex Wind Waves: Tips sway in a dual-sine wave wind gust via WGSL
//    vertex displacement shader without requiring per-vertex CPU buffer uploads.
// 5. Zero Shadow Overhead: Equipped with `NotShadowCaster` to protect the 5
//    cascaded shadow map passes from geometry explosion.
// 6. Automatic Chunk Lifecycle: Attached to terrain chunk entities; despawns
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
    /// Number of grass blade tufts generated per 16m chunk in the inner tactical LOD ring (default: 200).
    pub tufts_per_chunk: usize,
    /// Number of grass blade tufts generated per 16m chunk in the outer perimeter LOD ring (default: 70).
    pub outer_tufts_per_chunk: usize,
    /// Distance in 16m chunk units for the high-density inner tactical LOD ring (default: 2 chunks = 32m).
    pub inner_lod_distance_chunks: i32,
    /// Maximum distance from the player in 16m chunk units where grass spawns (default: 3 chunks = 48m).
    pub max_distance_chunks: i32,
    /// Minimum height of an individual grass blade in meters.
    pub blade_height_min: f32,
    /// Maximum height of an individual grass blade in meters.
    pub blade_height_max: f32,
    /// Width of grass blade base in meters (default: 0.13m).
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
            tufts_per_chunk: 200,      // High LOD inner ring (was 80)
            outer_tufts_per_chunk: 70, // Low LOD outer perimeter ring
            inner_lod_distance_chunks: 2, // 32m radius (tactical zone)
            max_distance_chunks: 3,       // 48m radius (edge horizon)
            blade_height_min: 0.45,
            blade_height_max: 0.72,
            blade_width: 0.13,         // Widened from 0.08 to 0.13 for bold stylized silhouette
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
// 2. COMPONENT TAGS & LOD STATES
// ----------------------------------------------------------------------------

/// Level of detail for chunk-batched grass geometry.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum GrassLod {
    /// High density (200 tufts, 4 blades/tuft = 800 blades) for chunks near the player (<=32m).
    High,
    /// Reduced density (70 tufts, 4 blades/tuft = 280 blades) for perimeter chunks (32m..48m).
    Low,
}

impl Default for GrassLod {
    fn default() -> Self {
        Self::High
    }
}

/// Tag component inserted on terrain chunk entities that currently have grass children spawned,
/// storing the current active LOD level of the grass mesh.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub struct ChunkHasGrass(pub GrassLod);

/// Tag component marking the grass mesh entity spawned as a child of a terrain chunk.
#[derive(Component, Debug, Default, Reflect)]
pub struct GrassChildMarker;

// ----------------------------------------------------------------------------
// 3. PROCEDURAL LOW-POLY GRASS MESH GENERATION
// ----------------------------------------------------------------------------

/// Generates a unified low-poly faceted grass tuft mesh for a 16m chunk at `(cx, cz)`.
/// Returns `None` if the chunk contains no valid meadow terrain (e.g. deep water, cliff, or snow).
pub fn generate_chunk_grass_mesh(
    cx: i32,
    cz: i32,
    config: &GrassConfig,
    lod: GrassLod,
) -> Option<Mesh> {
    let chunk_world_span = 16.0f32;
    let chunk_base_x = cx as f32 * chunk_world_span;
    let chunk_base_z = cz as f32 * chunk_world_span;

    // Stable deterministic pseudo-random seed unique to this chunk coordinate
    let seed = ((cx as i64 * 73856093) ^ (cz as i64 * 19349663) ^ 0x9E3779B97F4A7C15u64 as i64) as u64;
    let mut rng = Prng::new(seed);

    let target_tufts = match lod {
        GrassLod::High => config.tufts_per_chunk,
        GrassLod::Low => config.outer_tufts_per_chunk,
    };

    let blade_count = 4;
    let est_verts_per_blade = 5;
    let est_indices_per_blade = 9;
    let est_total_blades = target_tufts * blade_count;

    let mut positions = Vec::with_capacity(est_total_blades * est_verts_per_blade);
    let mut normals = Vec::with_capacity(est_total_blades * est_verts_per_blade);
    let mut colors = Vec::with_capacity(est_total_blades * est_verts_per_blade);
    let mut uvs = Vec::with_capacity(est_total_blades * est_verts_per_blade);
    let mut indices = Vec::with_capacity(est_total_blades * est_indices_per_blade);

    let mut tufts_placed = 0;

    for _ in 0..target_tufts {
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

        // Subtly vary palette per clump for handcrafted, organic richness
        let clump_hue_shift = rng.range(-0.02, 0.02);
        let clump_bright = rng.range(0.92, 1.08);

        // Root: Deep jade green / soil contact shadow
        let root_color = [
            (0.18 * clump_bright).clamp(0.0, 1.0),
            (0.42 * clump_bright + clump_hue_shift).clamp(0.0, 1.0),
            (0.16 * clump_bright).clamp(0.0, 1.0),
            1.0,
        ];
        // Mid: Fresh vibrant spring meadow green
        let mid_color = [
            (0.32 * clump_bright).clamp(0.0, 1.0),
            (0.68 * clump_bright + clump_hue_shift).clamp(0.0, 1.0),
            (0.22 * clump_bright).clamp(0.0, 1.0),
            1.0,
        ];
        // Tip: Sunlit chartreuse / golden lime catching stellar rays
        let tip_color = [
            (0.46 * clump_bright).clamp(0.0, 1.0),
            (0.82 * clump_bright + clump_hue_shift).clamp(0.0, 1.0),
            (0.28 * clump_bright).clamp(0.0, 1.0),
            1.0,
        ];

        let tuft_base_pos = Vec3::new(lx, world_y, lz);

        for b in 0..blade_count {
            // 4 blades fanned across 180 degrees (double-sided rendering gives 360-degree coverage)
            let base_angle = (b as f32 / blade_count as f32) * PI;
            let blade_angle = base_angle + rng.range(-0.15, 0.15);
            let h = rng.range(config.blade_height_min, config.blade_height_max);
            let w = config.blade_width * rng.range(0.88, 1.15);

            // Width direction vector perpendicular to blade face
            let u_dir = Vec3::new(blade_angle.cos(), 0.0, blade_angle.sin()) * (w * 0.5);

            // Alternating outward lean creates a natural fanning bowl star
            let lean_sign = if b % 2 == 0 { 1.0 } else { -1.0 };
            let outward_radial = Vec3::new(-blade_angle.sin(), 0.0, blade_angle.cos()) * lean_sign;

            // Curvature vectors for mid and tip
            let mid_lean = outward_radial * rng.range(0.07, 0.13);
            let tip_lean = outward_radial * rng.range(0.16, 0.28);

            // 5-vertex 2-segment tapered blade geometry:
            // V0: Root Left (0% height, 0% wind sway)
            let v0 = tuft_base_pos - u_dir;
            // V1: Root Right (0% height, 0% wind sway)
            let v1 = tuft_base_pos + u_dir;
            // V2: Mid Left (50% height, 50% wind sway, gentle lean curve)
            let v2 = tuft_base_pos - u_dir * 0.70 + mid_lean + Vec3::Y * (h * 0.5);
            // V3: Mid Right (50% height, 50% wind sway, gentle lean curve)
            let v3 = tuft_base_pos + u_dir * 0.70 + mid_lean + Vec3::Y * (h * 0.5);
            // V4: Blade Tip (100% height, 100% wind sway, full tip lean)
            let v4 = tuft_base_pos + tip_lean + Vec3::Y * h;

            // Flat face normal for lower quad
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

/// Synchronizes grass meshes on near terrain chunks around the player using two-ring distance LOD.
/// - Inner ring (<=32m): High LOD (200 tufts/chunk)
/// - Outer ring (32m..48m): Low LOD (70 tufts/chunk)
/// - Unload radius (>64m): Despawn
/// - LOD hysteresis (32m..38m): Prevents ping-ponging during player traversal
pub fn sync_chunk_grass(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GrassConfig>,
    material_handle: Option<Res<GrassMaterialHandle>>,
    mut meshes: ResMut<Assets<Mesh>>,
    player_query: Query<&Transform, With<PlayerBody>>,
    unspawned_chunks: Query<(Entity, &VoxelChunkMarker, &Transform), (With<TerrainChunkVisual>, Without<ChunkHasGrass>)>,
    spawned_chunks: Query<(Entity, &VoxelChunkMarker, &Transform, &ChunkHasGrass, &Children), With<TerrainChunkVisual>>,
    grass_children: Query<(Entity, &Handle<Mesh>), With<GrassChildMarker>>,
    mut scan_timer: Local<Option<Timer>>,
) {
    if !config.enabled {
        // If grass is disabled, remove all grass children and clear markers
        for (chunk_entity, _, _, _, children) in spawned_chunks.iter() {
            commands.entity(chunk_entity).remove::<ChunkHasGrass>();
            for &child in children.iter() {
                if grass_children.get(child).is_ok() {
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
    let inner_lod_radius_meters = config.inner_lod_distance_chunks as f32 * chunk_world_span; // 32.0m
    let inner_lod_radius_sq = inner_lod_radius_meters * inner_lod_radius_meters; // 1024.0

    // Hysteresis deadband for LOD switching:
    // Low -> High upgrades when within 32m.
    // High -> Low downgrades only when pushed past 38m (6m hysteresis buffer).
    let lod_downgrade_radius_meters = inner_lod_radius_meters + 6.0; // 38.0m
    let lod_downgrade_radius_sq = lod_downgrade_radius_meters * lod_downgrade_radius_meters;

    let spawn_radius_meters = config.max_distance_chunks as f32 * chunk_world_span; // 48.0m
    let spawn_radius_sq = spawn_radius_meters * spawn_radius_meters; // 2304.0

    // 16m hysteresis buffer to prevent boundary oscillation for unloading
    let unload_radius_meters = spawn_radius_meters + chunk_world_span; // 64.0m
    let unload_radius_sq = unload_radius_meters * unload_radius_meters; // 4096.0

    // 1. Process active spawned chunks: check unloads and LOD upgrades/downgrades
    for (chunk_entity, marker, chunk_tf, grass_state, children) in spawned_chunks.iter() {
        let chunk_center_x = chunk_tf.translation.x + chunk_world_span * 0.5;
        let chunk_center_z = chunk_tf.translation.z + chunk_world_span * 0.5;
        let dist_sq = (chunk_center_x - p_pos.x).powi(2) + (chunk_center_z - p_pos.z).powi(2);

        // A. Despawn distant chunks beyond unload radius (>64m)
        if dist_sq > unload_radius_sq {
            commands.entity(chunk_entity).remove::<ChunkHasGrass>();
            for &child in children.iter() {
                if grass_children.get(child).is_ok() {
                    commands.entity(child).despawn_recursive();
                }
            }
            continue;
        }

        // B. Check two-ring LOD transition with hysteresis
        let current_lod = grass_state.0;
        let next_lod = match current_lod {
            GrassLod::High => {
                if dist_sq > lod_downgrade_radius_sq {
                    GrassLod::Low
                } else {
                    GrassLod::High
                }
            }
            GrassLod::Low => {
                if dist_sq <= inner_lod_radius_sq {
                    GrassLod::High
                } else {
                    GrassLod::Low
                }
            }
        };

        if next_lod != current_lod {
            commands.entity(chunk_entity).insert(ChunkHasGrass(next_lod));

            if let Some(new_mesh) = generate_chunk_grass_mesh(marker.chunk_x, marker.chunk_z, &config, next_lod) {
                let mut found_child = false;
                for &child in children.iter() {
                    if let Ok((child_entity, mesh_handle)) = grass_children.get(child) {
                        // In-place mesh asset update leverages Bevy's asset change detection
                        if let Some(mesh_asset) = meshes.get_mut(mesh_handle) {
                            *mesh_asset = new_mesh.clone();
                            found_child = true;
                            break;
                        } else {
                            let new_handle = meshes.add(new_mesh.clone());
                            commands.entity(child_entity).insert(new_handle);
                            found_child = true;
                            break;
                        }
                    }
                }
                if !found_child {
                    let new_handle = meshes.add(new_mesh);
                    commands.entity(chunk_entity).with_children(|parent| {
                        parent.spawn((
                            MaterialMeshBundle {
                                mesh: new_handle,
                                material: mat_handle.0.clone(),
                                transform: Transform::IDENTITY,
                                ..default()
                            },
                            GrassChildMarker,
                            NotShadowCaster,
                            RenderLayers::layer(0),
                            Name::new("Chunk Low-Poly Grass"),
                        ));
                    });
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
            let initial_lod = if dist_sq <= inner_lod_radius_sq {
                GrassLod::High
            } else {
                GrassLod::Low
            };

            commands.entity(chunk_entity).insert(ChunkHasGrass(initial_lod));

            if let Some(grass_mesh) = generate_chunk_grass_mesh(marker.chunk_x, marker.chunk_z, &config, initial_lod) {
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
            .register_type::<GrassConfig>()
            .register_type::<GrassLod>()
            .register_type::<ChunkHasGrass>()
            .register_type::<GrassChildMarker>()
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
        assert!(config.tufts_per_chunk >= 100 && config.tufts_per_chunk <= 400);
        assert!(config.outer_tufts_per_chunk >= 30 && config.outer_tufts_per_chunk <= 150);
        assert!(config.inner_lod_distance_chunks >= 1 && config.inner_lod_distance_chunks < config.max_distance_chunks);
        assert!(config.max_distance_chunks >= 2 && config.max_distance_chunks <= 6);
        assert!(config.blade_height_min > 0.0);
        assert!(config.blade_height_min < config.blade_height_max);
        assert!(config.blade_width >= 0.10 && config.blade_width <= 0.20);
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
        // Test High LOD
        if let Some(mesh_high) = generate_chunk_grass_mesh(0, 0, &config, GrassLod::High) {
            assert!(mesh_high.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
            assert!(mesh_high.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
            assert!(mesh_high.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
            assert!(mesh_high.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
            assert!(mesh_high.indices().is_some());

            let vert_count_high = mesh_high.count_vertices();
            // 200 tufts * 4 blades * 5 verts = 4000 verts max
            assert!(vert_count_high >= 400, "High LOD grass mesh should contain multiple blades: {vert_count_high}");
            assert!(vert_count_high <= 6000, "High LOD grass mesh exceeds low-poly chunk budget: {vert_count_high}");

            // Verify UV.y height factor bounds: values must be in [0.0, 1.0]
            use bevy::render::mesh::VertexAttributeValues;
            if let Some(VertexAttributeValues::Float32x2(ref uvs)) = mesh_high.attribute(Mesh::ATTRIBUTE_UV_0) {
                for uv in uvs {
                    assert!(uv[1] >= 0.0 && uv[1] <= 1.0, "UV.y height factor must be in [0.0, 1.0]");
                }
            } else {
                panic!("Grass mesh must have Float32x2 UV coordinates");
            }
        }

        // Test Low LOD
        if let Some(mesh_low) = generate_chunk_grass_mesh(0, 0, &config, GrassLod::Low) {
            let vert_count_low = mesh_low.count_vertices();
            // 70 tufts * 4 blades * 5 verts = 1400 verts max
            assert!(vert_count_low >= 100, "Low LOD grass mesh should contain multiple blades: {vert_count_low}");
            assert!(vert_count_low <= 2500, "Low LOD grass mesh exceeds perimeter budget: {vert_count_low}");
        }
    }

    #[test]
    fn test_chunk_grass_seed_determinism() {
        let config = GrassConfig::default();
        let m1 = generate_chunk_grass_mesh(2, 3, &config, GrassLod::High);
        let m2 = generate_chunk_grass_mesh(2, 3, &config, GrassLod::High);

        if let (Some(mesh1), Some(mesh2)) = (m1, m2) {
            let pos1 = mesh1.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
            let pos2 = mesh2.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
            assert_eq!(pos1.len(), pos2.len(), "Identical coordinates must generate identical vertex counts");
            assert_eq!(pos1[0], pos2[0], "Identical coordinates must generate identical positions");
        }
    }

    #[test]
    fn test_chunk_grass_lod_hierarchy() {
        let config = GrassConfig::default();
        let high = generate_chunk_grass_mesh(0, 0, &config, GrassLod::High);
        let low = generate_chunk_grass_mesh(0, 0, &config, GrassLod::Low);

        if let (Some(mesh_high), Some(mesh_low)) = (high, low) {
            let verts_high = mesh_high.count_vertices();
            let verts_low = mesh_low.count_vertices();
            assert!(verts_high > verts_low, "High LOD must produce strictly more vertices than Low LOD");

            // Verify that the initial vertices of Low LOD match High LOD due to deterministic PRNG sequence
            let pos_high = mesh_high.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
            let pos_low = mesh_low.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
            assert_eq!(pos_high[0], pos_low[0], "First blade root must match across LODs to avoid pop");
        }
    }
}
