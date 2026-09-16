use bevy::prelude::{Transform as BevyTransform, *};
use bevy::render::mesh::{Capsule3d, Cuboid, Cylinder, Sphere, Torus};
use bevy::render::view::RenderLayers;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::pbr::{ScreenSpaceAmbientOcclusionQualityLevel, ScreenSpaceAmbientOcclusionSettings};
use avian3d::prelude::*;
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

// Note: Assuming `module_bindings` is exposed at the crate root.
use crate::module_bindings::{self, *};
use crate::core::*;
use crate::components::*;

const SPACETIMEDB_URI: &str = "http://localhost:3000";
const DB_NAME: &str = "hybrid-backend";

// ----------------------------------------------------------------------------
// NETWORK RESOURCES
// ----------------------------------------------------------------------------

/// Maintains the active websocket connection to SpacetimeDB.
/// Architectural Note: Keeping this globally accessible allows decoupled input/UI 
/// systems to dispatch reducers directly without tight coupling to the network tick.
#[derive(Resource)] 
pub struct SpacetimeConnection {
    pub db: module_bindings::DbConnection, 
    pub identity: Option<spacetimedb_sdk::Identity>,
}

/// A thread-safe container for the identity resolved in the background connection callback.
#[derive(Resource)] 
pub struct IdentityStore(pub Arc<Mutex<Option<spacetimedb_sdk::Identity>>>);

// ----------------------------------------------------------------------------
// BOOTSTRAP SYSTEMS
// ----------------------------------------------------------------------------

/// Initializes the SDK connection, requests the initial spatial subscription, and spawns the local player prefab.
pub fn init_network_connection(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut builder = module_bindings::DbConnection::builder()
        .with_uri(SPACETIMEDB_URI)
        .with_database_name(DB_NAME);

    // Architectural Note: Attempt to load an existing auth token to avoid creating a new player record on every boot.
    if let Ok(token) = std::fs::read_to_string("stdb_token.txt") { 
        builder = builder.with_token(Some(token)); 
    }

    let identity_store = Arc::new(Mutex::new(None));
    let store_clone = Arc::clone(&identity_store);

    let build_result = builder.on_connect(move |conn, identity, token| {
        if let Err(e) = std::fs::write("stdb_token.txt", token.to_string()) {
            warn!("Failed to persist SpacetimeDB token to disk: {}", e);
        }
        
        info!("Authenticated to SpacetimeDB with Identity: {}", identity.to_hex());
        
        // Architectural Note: Default subscription initializes in tight FPS mode (50m bounds) to save server compute (TeV).
        if let Err(e) = conn.subscription_builder().subscribe(vec![
            "SELECT * FROM player",
            "SELECT * FROM transform WHERE x > -50 AND x < 50 AND z > -50 AND z < 50",
            "SELECT * FROM resource_stockpile",
            "SELECT * FROM ground_loot",
            "SELECT * FROM resource_node",
            "SELECT * FROM combat_event" 
        ]) {
            error!("Failed to register SpacetimeDB subscriptions: {}", e);
        }

        match store_clone.lock() {
            Ok(mut guard) => { *guard = Some(identity.clone()); },
            Err(_) => { error!("Identity store lock poisoned during connection callback."); }
        }
    }).build();

    // Architectural Note: Hard crash prevention. We explicitly handle builder failure.
    let db = match build_result {
        Ok(db) => db,
        Err(e) => {
            error!("Failed to construct SpacetimeDB connection: {}", e);
            return;
        }
    };

    commands.insert_resource(SpacetimeConnection { db, identity: None });
    commands.insert_resource(IdentityStore(identity_store));

    // Spawn initial local cameras and lighting
    commands.spawn((
        SpatialBundle::from_transform(BevyTransform::from_xyz(0.0, 0.0, 0.0)),
        RtsCameraRig,
    )).with_children(|rig| {
        rig.spawn((
            Camera3dBundle {
                transform: BevyTransform::from_xyz(0.0, 40.0, 25.0)
                    .looking_at(Vec3::ZERO, Vec3::Y),
                camera: Camera { is_active: false, ..default() },
                ..default()
            },
            RenderLayers::from_layers(&[0, 2]),
            RtsCameraChild,
        ));
    });

    commands.spawn((
        SpatialBundle::from_transform(BevyTransform::from_xyz(0.0, 25.0, 0.0)),
        PlayerBody,
        RigidBody::Dynamic, 
        Collider::capsule(0.4, 1.2),
        CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Terrain, GameLayer::Unit, GameLayer::Environment]),
        LockedAxes::ROTATION_LOCKED,
        GravityScale(2.5),
        LinearVelocity::ZERO,
        ExternalForce::default().with_persistence(false),
        Kcc { is_grounded: false },
        LogicalPosition(Vec3::new(0.0, 25.0, 0.0)),
        LogicalRotation(Quat::IDENTITY),
        Faction::Player,
        Selectable, 
    )).with_children(|parent| {
        parent.spawn((
            PbrBundle {
                mesh: meshes.add(Cylinder::new(0.5, 2.0)),
                material: materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.8, 0.2), ..default() }),
                ..default()
            },
            RenderLayers::layer(2), 
            RTSProxy,
        ));

        parent.spawn((
            PbrBundle {
                mesh: meshes.add(Torus::new(0.6, 0.05)),
                material: materials.add(StandardMaterial { 
                    base_color: Color::srgb(0.0, 1.0, 0.0), 
                    unlit: true, 
                    ..default() 
                }),
                transform: BevyTransform::from_xyz(0.0, -0.9, 0.0), 
                visibility: Visibility::Hidden,
                ..default()
            },
            RenderLayers::layer(2),
            SelectionRing,
        ));

        parent.spawn((
            Camera3dBundle { 
                transform: BevyTransform::from_xyz(0.0, 0.5, 0.0), 
                camera: Camera { is_active: true, ..default() },
                ..default() 
            }, 
            PlayerHead, FpsCamera,
            RenderLayers::from_layers(&[0, 1]),
            DepthPrepass, NormalPrepass,
            ScreenSpaceAmbientOcclusionSettings { quality_level: ScreenSpaceAmbientOcclusionQualityLevel::High },
        )).with_children(|cam| {
            cam.spawn((
                PbrBundle {
                    mesh: meshes.add(Capsule3d::new(0.08, 0.4)),
                    material: materials.add(StandardMaterial {
                        base_color: Color::srgb(0.9, 0.7, 0.6), 
                        perceptual_roughness: 1.0,
                        ..default()
                    }),
                    transform: BevyTransform::from_xyz(0.3, -0.3, -0.5).with_rotation(Quat::from_rotation_x(1.0)),
                    ..default()
                },
                RenderLayers::layer(1),
                ViewModelArm, FPSMesh,
            ));
        });
    });

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight { illuminance: 8_000.0, shadows_enabled: true, ..default() },
        transform: BevyTransform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

/// Processes SDK frame ticks until the background thread resolves our identity.
pub fn wait_for_connection(
    store: Res<IdentityStore>,
    mut next_state: ResMut<NextState<GameState>>,
    mut connection: ResMut<SpacetimeConnection>,
) {
    let _ = connection.db.frame_tick();

    if let Ok(guard) = store.0.lock() {
        if let Some(id) = guard.as_ref() {
            if connection.identity.is_none() {
                connection.identity = Some(id.clone());
                next_state.set(GameState::InGame);
                info!("Bootstrapping complete. Entering In-Game State.");
            }
        }
    }
}

// ----------------------------------------------------------------------------
// SYNCHRONIZATION SYSTEMS
// ----------------------------------------------------------------------------

/// Rubber-bands the local client character to the server's authoritative state if they drift too far.
pub fn reconcile_local_transform(
    mut query: Query<&mut BevyTransform, With<PlayerBody>>,
    conn: Res<SpacetimeConnection>,
) {
    let Ok(mut transform) = query.get_single_mut() else { return; };
    let Some(my_identity) = &conn.identity else { return; };
    
    // Architectural Note: Avoids unwrap/expect on client state mismatch, bails gracefully instead.
    let Some(my_player) = conn.db.db.player().identity().find(my_identity) else { return; };
    let Some(server_transform) = conn.db.db.transform().entity_id().find(&my_player.entity_id) else { return; };

    let server_pos = Vec3::new(server_transform.x, server_transform.y, server_transform.z);
    
    if transform.translation.distance(server_pos) > 20.0 {
        warn!("Client desync detected. Hard reconciling to server authoritative position.");
        transform.translation = server_pos;
    }
}

/// Smoothly interpolates foreign entities toward their authoritative logical positions.
pub fn sync_logical_components(
    time: Res<Time>,
    mut query: Query<(&LogicalPosition, &LogicalRotation, &mut BevyTransform), Without<PlayerBody>>
) {
    let dt = time.delta_seconds() * 15.0; 
    
    for (log_pos, log_rot, mut transform) in query.iter_mut() {
        if transform.translation.distance(log_pos.0) > 5.0 {
            transform.translation = log_pos.0; // Snap if drift is immense (e.g. teleporting)
        } else {
            transform.translation = transform.translation.lerp(log_pos.0, dt);
        }
        transform.rotation = transform.rotation.slerp(log_rot.0, dt);
    }
}

/// Replicates the SpacetimeDB `transform` table state into the Bevy ECS, spawning missing entities.
pub fn sync_transforms(
    mut commands: Commands, 
    conn: Res<SpacetimeConnection>, 
    mut query: Query<(&NetworkEntity, &mut LogicalPosition, &mut LogicalRotation)>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Architectural Note: We tick the SDK connection exactly once per frame update in this system.
    let _ = conn.db.frame_tick();
    
    let my_entity_id = conn.identity.as_ref()
        .and_then(|id| conn.db.db.player().identity().find(id))
        .map(|p| p.entity_id);

    let db_transforms: Vec<_> = conn.db.db.transform().iter().collect();
    
    // Architectural Note: Replaced raw Vector with HashSet for O(1) lookups. 
    // In RTS mode with a 500m radius, a Vec.contains() scan would severely throttle the tick rate.
    let mut spawned_ids = std::collections::HashSet::with_capacity(query.iter().len());

    for (net_entity, mut log_pos, mut log_rot) in query.iter_mut() {
        spawned_ids.insert(net_entity.0);
        if let Some(db_t) = db_transforms.iter().find(|t| t.entity_id == net_entity.0) {
            log_pos.0 = Vec3::new(db_t.x, db_t.y, db_t.z);
            if Some(db_t.entity_id) != my_entity_id {
                log_rot.0 = Quat::from_xyzw(db_t.rot_x, db_t.rot_y, db_t.rot_z, db_t.rot_w);
            }
        }
    }

    for db_t in db_transforms {
        if Some(db_t.entity_id) == my_entity_id { continue; }
        
        if !spawned_ids.contains(&db_t.entity_id) {
            let spawn_quat = Quat::from_xyzw(db_t.rot_x, db_t.rot_y, db_t.rot_z, db_t.rot_w);
            commands.spawn((
                NetworkEntity(db_t.entity_id),
                SpatialBundle::from_transform(
                    BevyTransform::from_xyz(db_t.x, db_t.y, db_t.z).with_rotation(spawn_quat)
                ),
                LogicalPosition(Vec3::new(db_t.x, db_t.y, db_t.z)),
                LogicalRotation(spawn_quat),
                Faction::Player, // Assuming player faction defaults for foreign clients for now
            )).with_children(|parent| {
                parent.spawn((
                    PbrBundle {
                        mesh: meshes.add(Capsule3d::new(0.4, 1.8)),
                        material: materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.4, 0.8), ..default() }),
                        ..default()
                    },
                    RenderLayers::from_layers(&[0, 1, 2]),
                    FPSMesh,
                ));
            });
        }
    }
}

/// Dispatches our local physics position to the SpacetimeDB `process_movement` reducer.
pub fn send_movement_input(
    mut timer: ResMut<NetworkTickTimer>, 
    time: Res<Time>,
    body_query: Query<&BevyTransform, With<PlayerBody>>,
    head_query: Query<&BevyTransform, With<PlayerHead>>,
    conn: Res<SpacetimeConnection>,
) {
    if !timer.0.tick(time.delta()).just_finished() { return; }

    let Ok(body_transform) = body_query.get_single() else { return; };
    let Ok(head_transform) = head_query.get_single() else { return; };

    let combined_rot = body_transform.rotation * head_transform.rotation;

    // Architectural Note: Rate-limited to 20Hz by NetworkTickTimer to respect DB energy constraints.
    let _ = conn.db.reducers.process_movement(
        body_transform.translation.x, body_transform.translation.y, body_transform.translation.z, 
        combined_rot.x, combined_rot.y, combined_rot.z, combined_rot.w
    );
}

/// Syncs static harvestable trees/rocks based on the `resource_node` table.
pub fn sync_resource_nodes(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    node_query: Query<(Entity, &ResourceNodeItem)>, 
    conn: Res<SpacetimeConnection>,
) {
    let mut db_node_ids = std::collections::HashSet::new();

    for node in conn.db.db.resource_node().iter() {
        db_node_ids.insert(node.node_id);
        
        if !node_query.iter().any(|(_, n)| n.node_id == node.node_id) {
            let (mesh, color, collider, y_offset) = match node.node_type.as_str() {
                "Tree" => (meshes.add(Cylinder::new(0.5, 4.0)), Color::srgb(0.3, 0.2, 0.1), Collider::cylinder(0.5, 4.0), 2.0),
                "Rock" => (meshes.add(Cuboid::new(1.5, 1.2, 1.5)), Color::srgb(0.5, 0.5, 0.5), Collider::cuboid(1.5, 1.2, 1.5), 0.6),
                "Bush" => (meshes.add(Sphere::new(0.8).mesh()), Color::srgb(0.2, 0.6, 0.2), Collider::sphere(0.8), 0.8),
                _ => (meshes.add(Sphere::new(1.0).mesh()), Color::WHITE, Collider::sphere(1.0), 1.0)
            };

            commands.spawn((
                PbrBundle {
                    mesh, 
                    material: materials.add(StandardMaterial {
                        base_color: color,
                        perceptual_roughness: 0.9,
                        reflectance: 0.05,
                        ..default()
                    }),
                    transform: BevyTransform::from_xyz(node.x, node.y + y_offset + 0.05, node.z),
                    ..default()
                },
                ResourceNodeItem { node_id: node.node_id },
                RigidBody::Static, collider,
            ));
        }
    }

    for (entity, node_item) in node_query.iter() {
        if !db_node_ids.contains(&node_item.node_id) { commands.entity(entity).despawn_recursive(); }
    }
}

/// Syncs physics-enabled dropped items based on the `ground_loot` table.
pub fn sync_ground_loot(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    loot_query: Query<(Entity, &GroundLootItem)>, 
    conn: Res<SpacetimeConnection>,
) {
    let mut db_loot_ids = std::collections::HashSet::new();

    for loot in conn.db.db.ground_loot().iter() {
        db_loot_ids.insert(loot.loot_id);
        
        if !loot_query.iter().any(|(_, l)| l.loot_id == loot.loot_id) {
            let color = match loot.item_type.as_str() {
                "Wood" | "Branch" => Color::srgb(0.4, 0.2, 0.1),
                "Berry" => Color::srgb(0.8, 0.2, 0.2), 
                _ => Color::srgb(0.5, 0.5, 0.5)                 
            };

            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Sphere::new(0.3).mesh()), 
                    material: materials.add(StandardMaterial {
                        base_color: color,
                        perceptual_roughness: 0.85,
                        reflectance: 0.05,
                        ..default()
                    }),
                    transform: BevyTransform::from_xyz(loot.x, loot.y + 0.2, loot.z),
                    ..default()
                },
                GroundLootItem { loot_id: loot.loot_id },
                RigidBody::Dynamic, Collider::sphere(0.3),
            ));
        }
    }

    for (entity, loot_item) in loot_query.iter() {
        if !db_loot_ids.contains(&loot_item.loot_id) { commands.entity(entity).despawn_recursive(); }
    }
}

/// Triggers transient visual effects (VFX) when the server table `combat_event` updates.
pub fn process_combat_events(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    conn: Res<SpacetimeConnection>,
    mut tracker: ResMut<EventTracker>,
) {
    let mut highest_id = tracker.last_event_id;

    // Architectural Note: We only spawn particle bursts for events with IDs higher than our tracked state.
    // This prevents re-processing the entire event history since SpacetimeDB syncs the full table locally.
    for event in conn.db.db.combat_event().iter() {
        if event.id > tracker.last_event_id {
            highest_id = highest_id.max(event.id);
            
            let color = match event.event_type.as_str() {
                "HitTree" => Color::srgb(0.4, 0.2, 0.1), 
                "HitRock" => Color::srgb(0.5, 0.5, 0.5), 
                "HitBush" => Color::srgb(0.2, 0.6, 0.2), 
                _ => Color::WHITE,
            };

            let velocities = [
                Vec3::new(1.0, 3.0, 1.0),
                Vec3::new(-1.0, 3.5, 0.5),
                Vec3::new(0.5, 2.5, -1.0),
                Vec3::new(-0.5, 4.0, -0.5),
                Vec3::new(0.0, 3.0, 0.0),
            ];

            for vel in velocities {
                commands.spawn((
                    PbrBundle {
                        mesh: meshes.add(Cuboid::new(0.1, 0.1, 0.1)),
                        material: materials.add(color),
                        transform: BevyTransform::from_xyz(event.x, event.y + 0.5, event.z),
                        ..default()
                    },
                    RigidBody::Dynamic,
                    Collider::cuboid(0.1, 0.1, 0.1),
                    LinearVelocity(vel),
                    Particle { timer: Timer::from_seconds(0.5, TimerMode::Once) }, 
                ));
            }
        }
    }
    
    tracker.last_event_id = highest_id;
}