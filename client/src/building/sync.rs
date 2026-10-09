// ============================================================================
// File: client/src/building/sync.rs
// ============================================================================
// Structure replication with SpacetimeDB, collider assembly, and destruction swapping.

use bevy::prelude::*;
use std::collections::BTreeSet;
use spacetimedb_sdk::Table;

use crate::physics::*;
use crate::core::GameLayer;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::module_bindings::structure_table::StructureTableAccess;
use crate::module_bindings::door_state_table::DoorStateTableAccess;

use super::types::*;

// ----------------------------------------------------------------------------
// STRUCTURE SYNCHRONIZATION WITH SELECTIVE PERMEABILITY & DOORS
// ----------------------------------------------------------------------------

pub fn sync_structures(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<BuildingMeshCache>>,
    existing_structures: Query<(Entity, &NetworkStructure, Option<&BuildingFaction>, &ModularPieceType, &Transform)>,
    mut destruction_events: EventWriter<BuildingDestructionEvent>,
) {
    let _ = conn.db.frame_tick();
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));

    let db_structures: Vec<_> = conn.db.db.structure().iter().collect();
    let db_ids: BTreeSet<u64> = db_structures.iter().map(|s| s.structure_id).collect();

    // 1. Detect structures removed from SpacetimeDB (e.g. via server hazard sweep, decay, or despawn)
    for (entity, net_struct, faction, piece_type, transform) in existing_structures.iter() {
        if !db_ids.contains(&net_struct.structure_id) {
            #[allow(deprecated)]
            destruction_events.send(BuildingDestructionEvent {
                structure_id: net_struct.structure_id,
                faction: faction.copied().unwrap_or(BuildingFaction::Human),
                piece_type: *piece_type,
                position: transform.translation,
                rotation: transform.rotation,
            });
            commands.entity(entity).despawn_recursive();
        }
    }

    // 2. Track already spawned structures
    let mut spawned_ids = BTreeSet::new();
    for (_entity, net_struct, _fac, _pt, _t) in existing_structures.iter() {
        if db_ids.contains(&net_struct.structure_id) {
            spawned_ids.insert(net_struct.structure_id);
        }
    }

    // 3. Spawn newly added structures through unified data-driven spawner
    for s in db_structures {
        if !spawned_ids.contains(&s.structure_id) {
            #[allow(deprecated)]
            let faction = BuildingFaction::from_piece_name(&s.piece_type);
            let base_name = base_piece_name(&s.piece_type);
            let piece_type = match base_name {
                "Foundation" => ModularPieceType::Foundation,
                "Wall" => ModularPieceType::Wall,
                "Window" => ModularPieceType::Window,
                "Door" => ModularPieceType::Door,
                "Floor" => ModularPieceType::Floor,
                "Roof" => ModularPieceType::Roof,
                "Ramp" => ModularPieceType::Ramp,
                "Workbench" => ModularPieceType::Workbench,
                "Campfire" => ModularPieceType::Campfire,
                _ => ModularPieceType::Foundation,
            };

            let transform = Transform::from_xyz(s.x, s.y, s.z)
                .with_rotation(Quat::from_xyzw(s.rot_x, s.rot_y, s.rot_z, s.rot_w));

            let is_open = if piece_type == ModularPieceType::Door {
                conn.db.db.door_state().structure_id().find(&s.structure_id).map_or(false, |d| d.is_open)
            } else {
                false
            };

            spawn_modular_building_entity(
                &mut commands,
                mesh_cache,
                s.structure_id,
                faction,
                piece_type,
                transform,
                s.is_blueprint,
                is_open,
            );
        }
    }
}

/// Unified data-driven spawner for any modular architectural piece (Frontier Wood & Stone).
pub fn spawn_modular_building_entity(
    commands: &mut Commands,
    manifest: &BuildingAssetManifest,
    structure_id: u64,
    #[allow(deprecated)]
    faction: BuildingFaction,
    piece_type: ModularPieceType,
    transform: Transform,
    is_blueprint: bool,
    is_open: bool,
) -> Entity {
    let mesh = manifest.get_piece_mesh(piece_type, VisualDamageState::Pristine);
    let material = manifest.get_material(is_blueprint);
    let collider = manifest.get_collider(piece_type);
    let sockets = piece_type.default_sockets();

    match piece_type {
        ModularPieceType::Window => {
            #[allow(deprecated)]
            commands.spawn((
                PbrBundle {
                    mesh,
                    material: material.clone(),
                    transform,
                    ..default()
                },
                RigidBody::Static,
                collider,
                CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
                NetworkStructure { structure_id },
                SpacetimeBuildingId(structure_id),
                BuildingHealth { current: 100, max: 100 },
                VisualDamageState::Pristine,
                piece_type,
                faction,
            )).with_children(|parent| {
                // Central Glass Pane with selective permeability
                parent.spawn((
                    PbrBundle {
                        mesh: manifest.glass_pane_mesh.clone(),
                        material: manifest.glass_material.clone(),
                        transform: Transform::from_xyz(0.0, 0.25, 0.0),
                        ..default()
                    },
                    Collider::cuboid(1.9, 1.15, 0.08),
                    CollisionLayers::new([GameLayer::Glass], [GameLayer::Default, GameLayer::Unit]),
                    GlassPane,
                ));

                for socket in sockets {
                    parent.spawn((
                        SpatialBundle::from_transform(
                            Transform::from_translation(socket.local_offset)
                                      .with_rotation(socket.local_rotation)
                        ),
                        socket,
                    ));
                }
            }).id()
        }

        ModularPieceType::Door => {
            let initial_rotation = if is_open { Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY };
            let leaf_mesh = manifest.get_door_leaf_mesh();

            #[allow(deprecated)]
            commands.spawn((
                PbrBundle {
                    mesh,
                    material: material.clone(),
                    transform,
                    ..default()
                },
                RigidBody::Static,
                collider,
                CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
                NetworkStructure { structure_id },
                SpacetimeBuildingId(structure_id),
                BuildingHealth { current: 100, max: 100 },
                VisualDamageState::Pristine,
                piece_type,
                faction,
            )).with_children(|parent| {
                // Kinematic Door Leaf
                parent.spawn((
                    PbrBundle {
                        mesh: leaf_mesh,
                        material: material.clone(),
                        transform: Transform::from_xyz(-0.68, -0.35, 0.0).with_rotation(initial_rotation),
                        ..default()
                    },
                    RigidBody::Kinematic,
                    Collider::compound(vec![
                        (Vec3::new(0.68, 0.0, 0.0), Quat::IDENTITY, Collider::cuboid(1.36, 2.30, 0.14))
                    ]),
                    CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
                    Door {
                        structure_id,
                        target_rotation: initial_rotation,
                        is_open,
                        is_swinging: false,
                    },
                ));

                for socket in sockets {
                    parent.spawn((
                        SpatialBundle::from_transform(
                            Transform::from_translation(socket.local_offset)
                                      .with_rotation(socket.local_rotation)
                        ),
                        socket,
                    ));
                }
            }).id()
        }

        _ => {
            #[allow(deprecated)]
            commands.spawn((
                PbrBundle {
                    mesh,
                    material,
                    transform,
                    ..default()
                },
                RigidBody::Static,
                collider,
                CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
                NetworkStructure { structure_id },
                SpacetimeBuildingId(structure_id),
                BuildingHealth { current: 100, max: 100 },
                VisualDamageState::Pristine,
                piece_type,
                faction,
            )).with_children(|parent| {
                for socket in sockets {
                    parent.spawn((
                        SpatialBundle::from_transform(
                            Transform::from_translation(socket.local_offset)
                                      .with_rotation(socket.local_rotation)
                        ),
                        socket,
                    ));
                }
            }).id()
        }
    }
}

pub fn spawn_modular_building_system(
    mut commands: Commands,
    mut events: EventReader<SpawnBuildingEvent>,
    manifest: Option<Res<BuildingAssetManifest>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let local_cache;
    let manifest_ref: &BuildingAssetManifest = match manifest {
        Some(ref m) => m.as_ref(),
        None => {
            local_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));
            local_cache
        }
    };

    for ev in events.read() {
        let transform = Transform::from_translation(ev.position).with_rotation(ev.rotation);
        spawn_modular_building_entity(
            &mut commands,
            manifest_ref,
            ev.entity_id,
            ev.faction,
            ev.piece,
            transform,
            ev.is_blueprint,
            false,
        );
    }
}

// ----------------------------------------------------------------------------
// MODEL SWAPPING FOR LOW-POLY DESTRUCTION
// ----------------------------------------------------------------------------
// Architectural Note:
// The client does not authoritatively calculate building health; it subscribes
// to SpacetimeDB table updates. When health crosses the 50% threshold, this system
// seamlessly swaps the Mesh handle between Pristine and Damaged variants, triggering
// local dust particles. When current_health reaches 0, it dispatches BuildingDestructionEvent
// and despawns the structure.

pub fn update_building_destruction_visuals(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    mut query: Query<(
        Entity,
        &SpacetimeBuildingId,
        &mut Handle<Mesh>,
        &mut VisualDamageState,
        Option<&BuildingFaction>,
        &ModularPieceType,
        &Transform,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut destruction_events: EventWriter<BuildingDestructionEvent>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));

    for (entity, st_id, mut mesh_handle, mut state, faction, piece_type, transform) in query.iter_mut() {
        if let Some(st_building) = conn.db.db.structure().structure_id().find(&st_id.0) {
            let max_hp = st_building.max_health.max(1.0);
            let hp_percent = st_building.current_health / max_hp;

            if st_building.current_health <= 0.0 {
                // Building destroyed by health depletion
                #[allow(deprecated)]
                destruction_events.send(BuildingDestructionEvent {
                    structure_id: st_id.0,
                    faction: faction.copied().unwrap_or(BuildingFaction::Human),
                    piece_type: *piece_type,
                    position: transform.translation,
                    rotation: transform.rotation,
                });
                commands.entity(entity).despawn_recursive();
                continue;
            }

            if hp_percent <= 0.5 && matches!(*state, VisualDamageState::Pristine) {
                let damaged = mesh_cache.get_piece_mesh(*piece_type, VisualDamageState::Damaged);
                *mesh_handle = damaged;
                *state = VisualDamageState::Damaged;

                // Frontier timber & stone structural damage particles
                crate::terrain::spawn_voxel_gibs(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    transform.translation + Vec3::Y * 1.5,
                    14,
                    Color::srgb(0.50, 0.48, 0.45),
                    Color::srgb(0.42, 0.28, 0.16),
                    0.09,
                );
            } else if hp_percent > 0.5 && matches!(*state, VisualDamageState::Damaged) {
                let pristine = mesh_cache.get_piece_mesh(*piece_type, VisualDamageState::Pristine);
                *mesh_handle = pristine;
                *state = VisualDamageState::Pristine;
            }
        }
    }
}
