// ============================================================================
// File: client/src/network.rs
// ============================================================================
// ----------------------------------------------------------------------------
// CLIENT-SERVER NETWORKING & REPLICATION ENGINE (Bevy Engine / SpacetimeDB)
// ----------------------------------------------------------------------------
// Architectural Note:
// Strictly contains networking-critical synchronization logic:
// - SpacetimeDB SDK connection lifecycle, token persistence, and auth handshake
// - Server table subscriptions & spatial interest management
// - Local player authoritative state reconciliation & tick processing
// - Remote network entity transform interpolation & lifetime tracking
// All visual entity assembly (trees, rocks, bushes, clutter, projectiles, audio)
// is decoupled into dedicated domains (resource_nodes, creatures, weapons, audio).
// ----------------------------------------------------------------------------

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::pbr::{FogFalloff, FogSettings, NotShadowCaster};
use bevy::render::view::RenderLayers;
use avian3d::prelude::*;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

use spacetimedb_sdk::{DbContext, Table}; 

use crate::module_bindings::{self, *};
use crate::module_bindings::peasant_table::PeasantTableAccess; 
use crate::module_bindings::npc_brain_table::NpcBrainTableAccess; 
use crate::module_bindings::pet_component_table::PetComponentTableAccess; 
use crate::core::*;
use crate::components::*;
use crate::creatures::create_lowpoly_peasant_mesh;

const DB_NAME: &str = "hybrid-backend";

#[derive(Resource)] 
pub struct SpacetimeConnection {
    pub db: module_bindings::DbConnection, 
    pub identity: Option<spacetimedb_sdk::Identity>,
}

#[derive(Resource)] 
pub struct IdentityStore(pub Arc<Mutex<Option<spacetimedb_sdk::Identity>>>);

// ----------------------------------------------------------------------------
// NETWORK CONNECTION SYSTEM
// ----------------------------------------------------------------------------

pub fn init_network_connection(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let uri = std::env::var("SPACETIMEDB_URI").unwrap_or_else(|_| "http://localhost:3000".to_string());
    info!("Initializing SpacetimeDB connection to URI: {}", uri);

    let try_connect = |token: Option<String>| {
        let mut builder = module_bindings::DbConnection::builder()
            .with_uri(uri.as_str())
            .with_database_name(DB_NAME);

        if let Some(t) = token {
            builder = builder.with_token(Some(t));
        }

        let identity_store = Arc::new(Mutex::new(None));
        let store_clone = Arc::clone(&identity_store);

        let build_result = builder.on_connect(move |conn, identity, token| {
            if let Err(e) = std::fs::write("stdb_token.txt", token.to_string()) {
                warn!("Failed to persist SpacetimeDB token to disk: {}", e);
            }
            info!("Authenticated to SpacetimeDB with Identity: {}", identity.to_hex());
            
            let _handle = conn.subscription_builder().subscribe(vec![
                "SELECT * FROM player".to_string(),
                "SELECT * FROM transform".to_string(),
                "SELECT * FROM inventory".to_string(),
                "SELECT * FROM resource_node".to_string(),
                "SELECT * FROM combat_event".to_string(),
                "SELECT * FROM structure".to_string(),
                "SELECT * FROM peasant".to_string(), 
                "SELECT * FROM npc_brain".to_string(),
                "SELECT * FROM pet_component".to_string(),
                "SELECT * FROM faction_component".to_string(),
                "SELECT * FROM health".to_string(),
                "SELECT * FROM harvestable_corpse".to_string(),
                "SELECT * FROM player_perspective".to_string(),
                "SELECT * FROM voxel_chunk".to_string(),
                "SELECT * FROM active_projectile".to_string(),
                "SELECT * FROM equipment_loadout".to_string(),
                "SELECT * FROM door_state".to_string(),
                "SELECT * FROM fall_hazard".to_string(),
                "SELECT * FROM node_facing".to_string(),
            ]);

            if let Ok(mut guard) = store_clone.lock() {
                *guard = Some(identity.clone());
            }
        }).build();

        (build_result, identity_store)
    };

    let token = std::fs::read_to_string("stdb_token.txt").ok();
    let (mut build_result, mut identity_store) = try_connect(token.clone());

    if let Err(ref e) = build_result {
        if token.is_some() {
            warn!("Connection rejected (Error: {}). Deleting potentially stale stdb_token.txt and retrying...", e);
            let _ = std::fs::remove_file("stdb_token.txt");
            let retry = try_connect(None);
            build_result = retry.0;
            identity_store = retry.1;
        }
    }

    let db = match build_result {
        Ok(db) => db,
        Err(e) => {
            error!("FATAL: Could not connect to SpacetimeDB: {}", e);
            std::process::exit(1); 
        }
    };

    commands.insert_resource(SpacetimeConnection { db, identity: None });
    commands.insert_resource(IdentityStore(identity_store));

    let sky_fog_color = Color::srgb(0.75, 0.84, 0.92);

    commands.spawn((
        SpatialBundle::from_transform(BevyTransform::from_xyz(0.0, 0.0, 0.0)),
        RtsCameraRig,
    )).with_children(|rig| {
        rig.spawn((
            Camera3dBundle {
                transform: BevyTransform::from_xyz(0.0, 40.0, 25.0).looking_at(Vec3::ZERO, Vec3::Y),
                camera: Camera { is_active: false, ..default() },
                ..default()
            },
            VisibilityBundle::default(),
            FogSettings {
                color: sky_fog_color,
                falloff: FogFalloff::Linear {
                    start: 46.08,
                    end: 230.4,
                },
                ..default()
            },
            RenderLayers::from_layers(&[0, 2]),
            RtsCameraChild,
            crate::binary_sky::AtmosphericCamera,
        ));
    });

    let spawn_x = 0.0;
    let spawn_z = 0.0;
    let spawn_y = crate::terrain::get_terrain_height(spawn_x, spawn_z) + 1.05;

    let mut player_entity_commands = commands.spawn((
        SpatialBundle::from_transform(BevyTransform::from_xyz(spawn_x, spawn_y, spawn_z)),
        PlayerBody,
        RigidBody::Dynamic, 
        Collider::capsule(0.4, 1.2),
        ColliderDensity(1.0),
        SweptCcd::default(),
        CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Terrain, GameLayer::Environment, GameLayer::Glass]),
        LockedAxes::ROTATION_LOCKED,
        GravityScale(0.0),
        LinearVelocity::ZERO,
        ExternalForce::default().with_persistence(false),
        Kcc { is_grounded: false },
        LocomotionState::default(),
    ));

    player_entity_commands.insert((
        LogicalPosition(Vec3::new(spawn_x, spawn_y, spawn_z)),
        LogicalRotation(Quat::IDENTITY),
        crate::components::Faction::Player, 
        Selectable, 
        crate::prediction::InputBuffer::default(),
        crate::prediction::AuthoritativeState {
            position: Vec3::new(spawn_x, spawn_y, spawn_z),
            last_processed_tick: 0,
        },
        crate::prediction::LocalMovementTracker { last_position: Vec3::new(spawn_x, spawn_y, spawn_z) },
        Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
    ));

    player_entity_commands.with_children(|parent| {
        parent.spawn((
            PbrBundle {
                mesh: meshes.add(create_lowpoly_peasant_mesh()),
                material: materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.85, ..default() }),
                transform: BevyTransform::from_xyz(0.0, -1.05, 0.0),
                ..default()
            },
            RenderLayers::layer(2), 
            RTSProxy,
        ));

        parent.spawn((
            PbrBundle {
                mesh: meshes.add(bevy::math::primitives::Torus::new(0.6, 0.05)),
                material: materials.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() }),
                transform: BevyTransform::from_xyz(0.0, -0.9, 0.0), 
                visibility: Visibility::Hidden,
                ..default()
            },
            RenderLayers::layer(2),
            SelectionRing,
            NotShadowCaster,
        ));

        parent.spawn((
            Camera3dBundle { 
                projection: Projection::Perspective(PerspectiveProjection {
                    fov: 65.0_f32.to_radians(),
                    near: 0.05,
                    ..default()
                }),
                transform: BevyTransform::from_xyz(0.0, 0.5, 0.0), 
                camera: Camera { is_active: true, ..default() },
                ..default() 
            }, 
            VisibilityBundle::default(),
            PlayerHead, FpsCamera,
            bevy_voxel_world::prelude::VoxelWorldCamera::<crate::terrain::ProceduralTerrainConfig>::default(),
            FogSettings {
                color: sky_fog_color,
                falloff: FogFalloff::Linear {
                    start: 46.08,
                    end: 230.4,
                },
                ..default()
            },
            RenderLayers::from_layers(&[0, 1]),
            crate::binary_sky::AtmosphericCamera,
        ));
    });
}

pub fn wait_for_connection(
    store: Res<IdentityStore>,
    mut next_state: ResMut<NextState<GameState>>,
    mut connection: ResMut<SpacetimeConnection>,
    mut player_query: Query<(
        &mut BevyTransform, 
        Option<&mut avian3d::prelude::Position>,
        &mut LinearVelocity, 
        &mut GravityScale, 
        &mut crate::prediction::LocalMovementTracker,
        &mut crate::prediction::InputBuffer,
    ), With<PlayerBody>>,
) {
    let _ = connection.db.frame_tick();

    if let Ok(guard) = store.0.lock() {
        if let Some(id) = guard.as_ref() {
            if connection.identity.is_none() {
                connection.identity = Some(id.clone());
                next_state.set(GameState::InGame);
                info!("Bootstrapping complete. Entering In-Game State.");
                
                let spawn_y = crate::terrain::get_terrain_height(0.0, 0.0) + 1.5;
                
                if let Ok((mut transform, maybe_phys_pos, mut velocity, mut gravity, mut tracker, mut buffer)) = player_query.get_single_mut() {
                    transform.translation = Vec3::new(0.0, spawn_y, 0.0);
                    if let Some(mut phys_pos) = maybe_phys_pos {
                        phys_pos.0 = transform.translation;
                    }
                    velocity.x = 0.0; velocity.y = 0.0; velocity.z = 0.0;
                    gravity.0 = 8.0; 
                    tracker.last_position = transform.translation;
                    buffer.queue.clear();
                }
            }
        }
    }
}

pub fn update_spatial_subscriptions(
    player_query: Query<&LogicalPosition, With<PlayerBody>>,
    mut culling_state: ResMut<NetworkCullingState>,
) {
    if let Ok(pos) = player_query.get_single() {
        let current_x = (pos.0.x / 50.0).floor() as i32;
        let current_z = (pos.0.z / 50.0).floor() as i32;

        if culling_state.current_chunk.0 != current_x || culling_state.current_chunk.1 != current_z {
            let center_x = (culling_state.current_chunk.0 as f32 * 50.0) + 25.0;
            let center_z = (culling_state.current_chunk.1 as f32 * 50.0) + 25.0;
            
            if (pos.0.x - center_x).abs() > 27.0 || (pos.0.z - center_z).abs() > 27.0 {
                culling_state.current_chunk = (current_x, current_z);
                culling_state.needs_rebuild = true;
            }
        }
    }

    if culling_state.needs_rebuild {
        culling_state.needs_rebuild = false;
    }
}

pub fn sync_logical_components(
    time: Res<Time>,
    mut query: Query<(&LogicalPosition, &LogicalRotation, &mut BevyTransform), (Without<PlayerBody>, With<NetworkEntity>)>
) {
    let decay_factor = 1.0 - (-15.0_f32 * time.delta_seconds()).exp(); 
    
    for (log_pos, log_rot, mut transform) in query.iter_mut() {
        if transform.translation.distance(log_pos.0) > 5.0 { 
            transform.translation = log_pos.0; 
        } else { 
            transform.translation = transform.translation.lerp(log_pos.0, decay_factor); 
        }
        transform.rotation = transform.rotation.slerp(log_rot.0, decay_factor);
    }
}

pub fn sync_transforms(
    mut commands: Commands, 
    conn: Res<SpacetimeConnection>, 
    mut query: Query<(Entity, &NetworkEntity, &mut LogicalPosition, &mut LogicalRotation)>,
    peasant_query: Query<&PeasantUnit>,
    mut player_query: Query<&mut crate::prediction::AuthoritativeState, With<PlayerBody>>,
    player_body_query: Query<&BevyTransform, With<PlayerBody>>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    render_settings: Option<Res<crate::spellbook::TerrainRenderSettings>>,
    mut spawned_ids: Local<BTreeSet<u64>>,
    mut creature_cache: Local<Option<crate::creatures::CachedCreatureMeshes>>,
) {
    let _ = conn.db.frame_tick();
    
    let my_entity_id = conn.identity.as_ref()
        .and_then(|id| conn.db.db.player().identity().find(id))
        .map(|p| p.entity_id);

    let player_pos = player_body_query.get_single().map(|t| t.translation).unwrap_or(Vec3::ZERO);

    // Synchronize creature load/unload distance with terrain chunks (224m default / 240m unload at fog limit)
    let (creature_load_radius, creature_unload_radius) = if let Some(ref rs) = render_settings {
        let vr = if rs.spawn_full_zone { 64.0 * 16.0 } else { rs.view_distance_chunks as f32 * 16.0 };
        let ur = if rs.spawn_full_zone { 70.0 * 16.0 } else { rs.unload_distance_chunks.max(rs.view_distance_chunks + 1) as f32 * 16.0 };
        (vr, ur)
    } else {
        (crate::terrain::LOW_POLY_RADIUS_CHUNKS as f32 * 16.0, crate::terrain::LOW_POLY_UNLOAD_RADIUS_CHUNKS as f32 * 16.0)
    };
    let creature_load_radius_sq = creature_load_radius * creature_load_radius;
    let creature_unload_radius_sq = creature_unload_radius * creature_unload_radius;

    let cache = creature_cache.get_or_insert_with(|| crate::creatures::CachedCreatureMeshes::new(&mut meshes));

    spawned_ids.clear();

    for (entity, net_entity, mut log_pos, _) in query.iter_mut() {
        if Some(net_entity.0) == my_entity_id {
            spawned_ids.insert(net_entity.0);
            continue;
        }

        if let Some(db_t) = conn.db.db.transform().entity_id().find(&net_entity.0) {
            let dist_sq = (db_t.x - player_pos.x).powi(2) + (db_t.z - player_pos.z).powi(2);
            if dist_sq > creature_unload_radius_sq {
                commands.entity(entity).despawn_recursive();
                continue;
            }

            log_pos.0 = Vec3::new(db_t.x, db_t.y, db_t.z);
            spawned_ids.insert(net_entity.0);
        } else {
            // Entity was removed from server DB (killed / despawned / harvested).
            // Spawn death voxel gibs at its last known position for instant combat impact feedback.
            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                log_pos.0 + Vec3::Y * 0.8,
                16,
                Color::srgb(0.65, 0.15, 0.15),
                Color::srgb(0.40, 0.35, 0.30),
                0.22,
            );
            commands.entity(entity).despawn_recursive();
            continue;
        }

        if peasant_query.get(entity).is_err() && conn.db.db.peasant().entity_id().find(&net_entity.0).is_some() {
            commands.entity(entity).insert(PeasantUnit { entity_id: net_entity.0 });
        }
    }

    for db_t in conn.db.db.transform().iter() {
        let id = db_t.entity_id;
        if Some(id) == my_entity_id { 
            if let Ok(mut auth_state) = player_query.get_single_mut() {
                let server_pos = Vec3::new(db_t.x, db_t.y, db_t.z);
                if auth_state.last_processed_tick != db_t.last_processed_tick || auth_state.position.distance_squared(server_pos) > 9.0 {
                    auth_state.position = server_pos;
                    auth_state.last_processed_tick = db_t.last_processed_tick;
                }
            }
            continue; 
        }

        let dist_sq = (db_t.x - player_pos.x).powi(2) + (db_t.z - player_pos.z).powi(2);
        if dist_sq > creature_load_radius_sq || spawned_ids.contains(&id) {
            continue;
        }

        let is_corpse = conn.db.db.harvestable_corpse().entity_id().find(&id).is_some();
        if is_corpse {
            crate::creatures::spawn_corpse_visual_entity(
                &mut commands,
                cache,
                &mut materials,
                &mut meshes,
                id,
                &db_t,
            );
            spawned_ids.insert(id);
            continue;
        }
        
        let is_peasant = conn.db.db.peasant().entity_id().find(&id).is_some();
        let is_pet = conn.db.db.pet_component().entity_id().find(&id).is_some();
        let npc_brain = conn.db.db.npc_brain().entity_id().find(&id);

        crate::creatures::spawn_creature_visual_entity(
            &mut commands,
            cache,
            &mut materials,
            &mut meshes,
            id,
            &db_t,
            is_peasant,
            is_pet,
            npc_brain.as_ref(),
        );

        spawned_ids.insert(id);
    }
}