// ============================================================================
// File: client/src/building/destruction.rs
// ============================================================================
// Structural collapse physics, generic rubble pools, falling hazards, and ruin spawning.

use bevy::prelude::*;
use std::collections::BTreeSet;
use tracing::info;
use spacetimedb_sdk::Table;

use crate::physics::*;
use crate::core::GameLayer;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::module_bindings::fall_hazard_table::FallHazardTableAccess;

use super::types::*;

// ----------------------------------------------------------------------------
// LETHAL FALLS & HARVESTABLE RUIN SPAWNING (THE ILLUSION OF PHYSICS)
// ----------------------------------------------------------------------------
// Architectural Note:
// Spawns a permanent static ruin pile (rubble / fallen timber) that blocks player
// movement and provides harvestable mining yields.

pub fn spawn_ruin_pile(
    commands: &mut Commands,
    mesh_cache: &BuildingMeshCache,
    material: Handle<StandardMaterial>,
    position: Vec3,
    yield_amount: u32,
    ruin_type: &str,
) -> Entity {
    commands.spawn((
        PbrBundle {
            mesh: mesh_cache.rubble_node_mesh.clone(),
            material,
            transform: Transform::from_translation(position),
            ..default()
        },
        Collider::cuboid(2.0, 1.0, 2.0),
        RigidBody::Static,
        CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
        HarvestableRuin {
            yield_amount,
            node_type: ruin_type.to_string(),
        },
    )).id()
}

pub fn sync_fall_hazards(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing_hazards: Query<&VisualFallHazard>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));
    let mut known = BTreeSet::new();
    for h in existing_hazards.iter() {
        known.insert(h.hazard_id);
    }

    for h in conn.db.db.fall_hazard().iter() {
        if !known.contains(&h.hazard_id) {
            let origin = Vec3::new(h.origin_x, h.origin_y, h.origin_z);
            let dir = Vec3::new(h.dir_x, 0.0, h.dir_z).normalize_or_zero();

            let mesh = match h.kind.as_str() {
                "CollapsingTower" => mesh_cache.get_piece_mesh(ModularPieceType::Wall, VisualDamageState::Damaged),
                _ => mesh_cache.rubble_node_mesh.clone(),
            };

            let material = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.85,
                ..default()
            });

            commands.spawn((
                PbrBundle {
                    mesh,
                    material,
                    transform: Transform::from_translation(origin),
                    ..default()
                },
                VisualFallHazard {
                    hazard_id: h.hazard_id,
                    kind: h.kind.clone(),
                    origin,
                    dir,
                    length: h.length,
                    elapsed: 0.0,
                    duration: (h.duration_ms as f32) / 1000.0,
                    initial_rotation: Quat::IDENTITY,
                },
            ));
        }
    }
}

pub fn update_fall_hazards(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut VisualFallHazard, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));
    let dt = time.delta_seconds().min(0.1);
    for (entity, mut hazard, mut transform) in query.iter_mut() {
        hazard.elapsed += dt;
        let progress = (hazard.elapsed / hazard.duration.max(0.1)).min(1.0);

        // Smooth visual topple curve (0 to 90 degrees)
        let angle = progress * std::f32::consts::FRAC_PI_2;
        let tilt_axis = Vec3::new(hazard.dir.z, 0.0, -hazard.dir.x).normalize_or_zero();
        let rot = Quat::from_axis_angle(tilt_axis, angle);

        transform.rotation = rot * hazard.initial_rotation;

        if progress >= 1.0 {
            let impact_pos = hazard.origin + hazard.dir * (hazard.length * 0.5);

            // Impact dust cloud
            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                hazard.origin + hazard.dir * (hazard.length * 0.4),
                28,
                Color::srgb(0.48, 0.46, 0.44),
                Color::srgb(0.60, 0.50, 0.40),
                0.12,
            );

            // Spawn the permanent harvestable ruin pile
            let ruin_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.55, 0.48, 0.42),
                perceptual_roughness: 0.9,
                ..default()
            });

            let (ruin_type, yield_amt) = if hazard.kind == "FellingTree" {
                ("FallenLog", 50)
            } else {
                ("Rubble", 75)
            };

            spawn_ruin_pile(&mut commands, mesh_cache, ruin_mat, impact_pos, yield_amt, ruin_type);

            commands.entity(entity).despawn_recursive();
        }
    }
}

// ----------------------------------------------------------------------------
// FACTION DESTRUCTION ANIMATIONS & SHARD GENERATION
// ----------------------------------------------------------------------------
// Architectural Note:
// Early 2000s MMO Architectural Style Destruction Profiles:
// 1. High Elf (The Pristine Bastion): Shatters into clean geometric shards
//    (glass-like break patterns) with high angular velocity, mint marble & lapis colors.
// 2. Human (The Utilitarian Fortress): Heavy blunt rubble cubes, splintered timber
//    planks, and thick billowing dust clouds.
// 3. Dark Elf (The Subterranean Spire): Dark jagged obsidian chunks and violently
//    bursting neon runes that flicker out via RuneLightDecay.

pub fn handle_building_destruction(
    mut commands: Commands,
    mut events: EventReader<BuildingDestructionEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));

    for event in events.read() {
        info!("Handling building destruction for {:?} at {:?}", event.piece_type, event.position);

        let damaged_mesh = mesh_cache.get_piece_mesh(event.piece_type, VisualDamageState::Damaged);
        let mat = mesh_cache.get_material(false);

        // 1. Ephemeral collapse entity that shudders, tilts, and sinks into the ground
        #[allow(deprecated)]
        commands.spawn((
            PbrBundle {
                mesh: damaged_mesh,
                material: mat,
                transform: Transform::from_translation(event.position).with_rotation(event.rotation),
                ..default()
            },
            BuildingDestructionAnimation {
                faction: event.faction,
                piece_type: event.piece_type,
                elapsed: 0.0,
                duration: 1.4,
                origin: event.position,
            },
        ));

        // 2. Frontier Timber & Stone debris & particle effects via generic rubble pool
        spawn_frontier_destruction_fx(&mut commands, mesh_cache, &mut meshes, &mut materials, event.position);
    }
}

/// Frontier Timber & Stone destruction FX pipeline using the pre-cached GenericRubblePool.
/// Decouples visual destruction from wall geometry by spawning stone cubes, brick blocks,
/// timber planks, and billowing mortar dust clouds.
pub fn spawn_frontier_destruction_fx(
    commands: &mut Commands,
    manifest: &BuildingAssetManifest,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    origin: Vec3,
) {
    let mut rng_seed = (origin.x.abs() * 1000.0 + origin.z.abs() * 100.0) as u64;
    let pool = &manifest.generic_rubble_pool;
    let frontier_mat = manifest.get_material(false);

    // 1. Heavy blunt rubble cubes & brick blocks
    for i in 0..16 {
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let ry = ((rng_seed >> 32) as u32 % 30) as f32 / 10.0 + 1.0;

        let mesh = if i % 2 == 0 { pool.stone_cube.clone() } else { pool.brick_block.clone() };
        let offset = Vec3::new(rx * 0.3, (i as f32 * 0.05).min(0.8), rz * 0.3);

        commands.spawn((
            PbrBundle {
                mesh,
                material: frontier_mat.clone(),
                transform: Transform::from_translation(origin + offset),
                ..default()
            },
            VoxelGib {
                timer: Timer::from_seconds(1.3 + (i as f32 * 0.03), TimerMode::Once),
                velocity: Vec3::new(rx * 4.2, ry, rz * 4.2),
                angular_velocity: Vec3::new(rx * 12.0, ry * 6.0, rz * 12.0),
            },
        ));
    }

    // 2. Splintered timber planks
    for i in 0..8 {
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let ry = ((rng_seed >> 32) as u32 % 35) as f32 / 10.0 + 1.5;

        let offset = Vec3::new(rx * 0.25, 0.4 + (i as f32 * 0.08), rz * 0.25);
        commands.spawn((
            PbrBundle {
                mesh: pool.timber_plank.clone(),
                material: frontier_mat.clone(),
                transform: Transform::from_translation(origin + offset),
                ..default()
            },
            VoxelGib {
                timer: Timer::from_seconds(1.5 + (i as f32 * 0.04), TimerMode::Once),
                velocity: Vec3::new(rx * 4.8, ry, rz * 4.8),
                angular_velocity: Vec3::new(rx * 16.0, ry * 10.0, rz * 16.0),
            },
        ));
    }

    // 3. Billowing mortar dust clouds
    crate::terrain::spawn_voxel_gibs(
        commands,
        meshes,
        materials,
        origin + Vec3::Y * 0.5,
        20,
        Color::srgb(0.66, 0.63, 0.58),
        Color::srgb(0.48, 0.46, 0.44),
        0.14,
    );
}

pub fn update_building_destruction_animations(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut BuildingDestructionAnimation, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));
    let dt = time.delta_seconds().min(0.1);

    for (entity, mut anim, mut transform) in query.iter_mut() {
        anim.elapsed += dt;
        let progress = (anim.elapsed / anim.duration.max(0.1)).min(1.0);

        // Structural shudder vibration: high frequency shake that dampens slightly
        let shake = (anim.elapsed * 40.0).sin() * 0.04 * (1.0 - progress * 0.5);
        let sway = (anim.elapsed * 25.0).cos() * 0.03 * (1.0 - progress * 0.5);

        // Collapse sinking along Y axis
        let sink_y = progress * 1.2;
        transform.translation = anim.origin + Vec3::new(shake, -sink_y, sway);

        // Subtle structural failure tilt
        transform.rotate_local_x(0.18 * dt);

        if progress >= 1.0 {
            // Sinking collapse complete: spawn permanent static HarvestableRuin node
            let (ruin_type, yield_amt, ruin_color) = ("TimberStoneRubble", 60, Color::srgb(0.50, 0.48, 0.45));

            let ruin_mat = materials.add(StandardMaterial {
                base_color: ruin_color,
                perceptual_roughness: 0.9,
                ..default()
            });

            // Permanent static ruin pile that blocks movement and provides harvestable yields
            spawn_ruin_pile(&mut commands, mesh_cache, ruin_mat, anim.origin + Vec3::new(0.0, 0.2, 0.0), yield_amt, ruin_type);

            // Ground impact dust burst
            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                anim.origin + Vec3::Y * 0.3,
                16,
                ruin_color,
                Color::srgb(0.35, 0.35, 0.35),
                0.10,
            );

            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn update_rune_light_decay(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut PointLight, &mut RuneLightDecay)>,
) {
    for (entity, mut light, mut decay) in query.iter_mut() {
        if decay.timer.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn_recursive();
        } else {
            let frac = 1.0 - decay.timer.fraction();
            let flicker = 0.65 + 0.35 * (decay.timer.elapsed_secs() * 32.0).sin();
            light.intensity = decay.base_intensity * frac * flicker;
        }
    }
}
