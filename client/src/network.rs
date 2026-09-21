use bevy::prelude::{Transform as BevyTransform, *};
use bevy::math::primitives::{Capsule3d, Cuboid, Cylinder, Sphere, Torus};
use bevy::render::view::RenderLayers;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::pbr::{ScreenSpaceAmbientOcclusionQualityLevel, ScreenSpaceAmbientOcclusionSettings};
use avian3d::prelude::*;
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

use spacetimedb_sdk::{DbContext, Table}; 

use crate::module_bindings::{self, *};
use crate::module_bindings::peasant_table::PeasantTableAccess; 
use crate::module_bindings::npc_brain_table::NpcBrainTableAccess; 
use crate::module_bindings::pet_component_table::PetComponentTableAccess; 
use crate::core::*;
use crate::components::*;

const DB_NAME: &str = "hybrid-backend";

#[derive(Resource)] 
pub struct SpacetimeConnection {
    pub db: module_bindings::DbConnection, 
    pub identity: Option<spacetimedb_sdk::Identity>,
}

#[derive(Resource)] 
pub struct IdentityStore(pub Arc<Mutex<Option<spacetimedb_sdk::Identity>>>);

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
                "SELECT * FROM transform WHERE chunk_x >= -1 AND chunk_x <= 1 AND chunk_z >= -1 AND chunk_z <= 1".to_string(),
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
            RenderLayers::from_layers(&[0, 2]),
            RtsCameraChild,
        ));
    });

    commands.spawn((
        (
            SpatialBundle::from_transform(BevyTransform::from_xyz(0.0, 25.0, 0.0)),
            PlayerBody,
            RigidBody::Dynamic, 
            Collider::capsule(0.4, 1.2),
            SweptCcd::default(),
            CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Terrain, GameLayer::Unit, GameLayer::Environment]),
            LockedAxes::ROTATION_LOCKED,
            GravityScale(0.0),
            LinearVelocity::ZERO,
            ExternalForce::default().with_persistence(false),
            Kcc { is_grounded: false },
            LogicalPosition(Vec3::new(0.0, 25.0, 0.0)),
            LogicalRotation(Quat::IDENTITY),
            crate::components::Faction::Player, 
            Selectable, 
        ),
        (
            crate::prediction::InputBuffer::default(),
            crate::prediction::AuthoritativeState::default(),
            crate::prediction::LocalMovementTracker { last_position: Vec3::new(0.0, 25.0, 0.0) },
            Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
        )
    )).with_children(|parent| {
        parent.spawn((
            PbrBundle {
                mesh: meshes.add(Cylinder::new(0.5, 2.0)),
                material: materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.1, 0.1), ..default() }),
                ..default()
            },
            RenderLayers::layer(2), 
            RTSProxy,
        ));

        parent.spawn((
            PbrBundle {
                mesh: meshes.add(Torus::new(0.6, 0.05)),
                material: materials.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() }),
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
                        base_color: Color::srgb(0.9, 0.7, 0.6), perceptual_roughness: 1.0, ..default()
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

pub fn wait_for_connection(
    store: Res<IdentityStore>,
    mut next_state: ResMut<NextState<GameState>>,
    mut connection: ResMut<SpacetimeConnection>,
    mut player_query: Query<(
        &mut BevyTransform, 
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
                
                let spawn_y = crate::terrain::get_terrain_height(0.0, 0.0) + 10.0;
                
                if let Ok((mut transform, mut velocity, mut gravity, mut tracker, mut buffer)) = player_query.get_single_mut() {
                    transform.translation = Vec3::new(0.0, spawn_y, 0.0);
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
    conn: Res<SpacetimeConnection>,
) {
    if let Ok(pos) = player_query.get_single() {
        let current_x = (pos.0.x / 50.0).floor() as i32;
        let current_z = (pos.0.z / 50.0).floor() as i32;

        if culling_state.current_chunk.0 != current_x || culling_state.current_chunk.1 != current_z {
            
            // Architectural Note: Hysteresis Deadzone Validation
            // Only commits to a new chunk index if the player successfully crossed the boundary 
            // by at least 2.0 full coordinate units. This halts `needs_rebuild` from oscillating 
            // continuously when resolving floating-point edge borders, fixing entity generation spam.
            let center_x = (culling_state.current_chunk.0 as f32 * 50.0) + 25.0;
            let center_z = (culling_state.current_chunk.1 as f32 * 50.0) + 25.0;
            
            if (pos.0.x - center_x).abs() > 27.0 || (pos.0.z - center_z).abs() > 27.0 {
                culling_state.current_chunk = (current_x, current_z);
                culling_state.needs_rebuild = true;
            }
        }
    }

    if culling_state.needs_rebuild && conn.identity.is_some() {
        let cx = culling_state.current_chunk.0;
        let cz = culling_state.current_chunk.1;
        let rad = culling_state.radius;

        let mut subscriptions = vec![
            "SELECT * FROM player".to_string(),
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
        ];

        if culling_state.in_interior {
            subscriptions.push(format!("SELECT * FROM transform WHERE chunk_x = {} AND chunk_z = {}", cx, cz));
        } else {
            subscriptions.push(format!(
                "SELECT * FROM transform WHERE chunk_x >= {} AND chunk_x <= {} AND chunk_z >= {} AND chunk_z <= {}",
                cx - rad, cx + rad, cz - rad, cz + rad
            ));
        }

        let _handle = conn.db.subscription_builder().subscribe(subscriptions);
        culling_state.needs_rebuild = false;
    }
}

pub fn sync_logical_components(
    time: Res<Time>,
    mut query: Query<(&LogicalPosition, &LogicalRotation, &mut BevyTransform), Without<PlayerBody>>
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
    mut player_query: Query<&mut crate::prediction::AuthoritativeState, With<PlayerBody>>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let _ = conn.db.frame_tick();
    
    let my_entity_id = conn.identity.as_ref()
        .and_then(|id| conn.db.db.player().identity().find(id))
        .map(|p| p.entity_id);

    let db_transforms_map: std::collections::HashMap<_, _> = conn.db.db.transform()
        .iter()
        .map(|t| (t.entity_id, t.clone()))
        .collect();

    let mut spawned_ids = std::collections::HashSet::with_capacity(query.iter().len());

    for (entity, net_entity, mut log_pos, _) in query.iter_mut() {
        spawned_ids.insert(net_entity.0);
        if let Some(db_t) = db_transforms_map.get(&net_entity.0) {
            log_pos.0 = Vec3::new(db_t.x, db_t.y, db_t.z);
        } else {
            commands.entity(entity).despawn_recursive();
        }
    }

    for (id, db_t) in db_transforms_map {
        if Some(id) == my_entity_id { 
            if let Ok(mut auth_state) = player_query.get_single_mut() {
                if auth_state.last_processed_tick != db_t.last_processed_tick {
                    auth_state.position = Vec3::new(db_t.x, db_t.y, db_t.z);
                    auth_state.last_processed_tick = db_t.last_processed_tick;
                }
            }
            continue; 
        }
        
        if !spawned_ids.contains(&id) {
            let is_peasant = conn.db.db.peasant().entity_id().find(&id).is_some();
            let is_pet = conn.db.db.pet_component().entity_id().find(&id).is_some();
            let npc_brain = conn.db.db.npc_brain().entity_id().find(&id);
            
            let mut color = Color::srgb(0.8, 0.1, 0.1); 
            let mut mesh_handle = meshes.add(Capsule3d::new(0.4, 1.8));
            let mut visual_transform = BevyTransform::default();
            
            let mut root_collider = Collider::capsule(0.4, 1.8);

            if is_pet {
                color = Color::srgb(0.9, 0.5, 0.1); 
                mesh_handle = meshes.add(Sphere::new(0.6).mesh());
                root_collider = Collider::sphere(0.6);
            } else if let Some(brain) = npc_brain {
                match brain.ai_type {
                    crate::module_bindings::AiType::Boar => {
                        color = Color::srgb(0.1, 0.1, 0.1); 
                        mesh_handle = meshes.add(Cylinder::new(0.5, 1.5));
                        visual_transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
                        root_collider = Collider::sphere(0.8);
                    }
                    crate::module_bindings::AiType::Deer => {
                        color = Color::srgb(0.4, 0.2, 0.1); 
                        mesh_handle = meshes.add(Cylinder::new(0.5, 1.5));
                        visual_transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
                        root_collider = Collider::sphere(0.8);
                    }
                    crate::module_bindings::AiType::Goblin => {
                        color = Color::srgb(0.1, 0.8, 0.1); 
                        mesh_handle = meshes.add(Capsule3d::new(0.4, 1.5));
                        root_collider = Collider::capsule(0.4, 1.5);
                    }
                    crate::module_bindings::AiType::Friendly | crate::module_bindings::AiType::Peasant => {
                        color = Color::srgb(0.1, 0.3, 0.9); 
                        mesh_handle = meshes.add(Capsule3d::new(0.4, 1.8));
                    }
                }
                
                // Color corpses distinctly
                if brain.state == crate::module_bindings::BrainState::Corpse {
                    color = Color::srgb(0.2, 0.2, 0.2);
                    visual_transform.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
                }
                
            } else if is_peasant {
                color = Color::srgb(0.1, 0.3, 0.9); 
                mesh_handle = meshes.add(Capsule3d::new(0.4, 1.8));
            }

            let mut entity_cmds = commands.spawn((
                NetworkEntity(id),
                SpatialBundle::from_transform(BevyTransform::from_xyz(db_t.x, db_t.y, db_t.z)),
                LogicalPosition(Vec3::new(db_t.x, db_t.y, db_t.z)),
                LogicalRotation(Quat::IDENTITY),
                Selectable, 
                RigidBody::Kinematic, 
                root_collider,
                CollisionLayers::new([GameLayer::Unit], [GameLayer::Default, GameLayer::Terrain, GameLayer::Environment]),
            ));

            if is_peasant {
                entity_cmds.insert(PeasantUnit { entity_id: id });
            }

            entity_cmds.with_children(|parent| {
                parent.spawn((
                    PbrBundle {
                        mesh: mesh_handle,
                        material: materials.add(StandardMaterial { base_color: color, ..default() }),
                        transform: visual_transform,
                        ..default()
                    },
                    RenderLayers::from_layers(&[0, 1, 2]), RTSProxy,
                ));
                parent.spawn((
                    PbrBundle {
                        mesh: meshes.add(Torus::new(0.6, 0.05)),
                        material: materials.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() }),
                        transform: BevyTransform::from_xyz(0.0, -0.4, 0.0), visibility: Visibility::Hidden, ..default()
                    },
                    RenderLayers::layer(2), SelectionRing,
                ));
            });
        }
    }
}

pub fn sync_resource_nodes(
    mut commands: Commands, 
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    node_query: Query<(Entity, &ResourceNodeItem)>, 
    conn: Res<SpacetimeConnection>,
) {
    let mut db_node_ids = std::collections::HashSet::new();

    let mut local_nodes = std::collections::HashSet::new();
    for (_, n) in node_query.iter() {
        local_nodes.insert(n.node_id);
    }

    for node in conn.db.db.resource_node().iter() {
        db_node_ids.insert(node.node_id);
        
        if !local_nodes.contains(&node.node_id) {
            let (mesh, color, collider, y_offset) = match node.node_type.as_str() {
                "Tree" => (meshes.add(Cylinder::new(0.5, 4.0)), Color::srgb(0.3, 0.2, 0.1), Collider::cylinder(0.5, 4.0), 2.0),
                "Rock" => (meshes.add(Cuboid::new(1.5, 1.2, 1.5)), Color::srgb(0.5, 0.5, 0.5), Collider::cuboid(1.5, 1.2, 1.5), 0.6),
                "Bush" => (meshes.add(Sphere::new(0.8).mesh()), Color::srgb(0.2, 0.6, 0.2), Collider::sphere(0.8), 0.8),
                _ => (meshes.add(Sphere::new(1.0).mesh()), Color::WHITE, Collider::sphere(1.0), 1.0)
            };

            commands.spawn((
                PbrBundle {
                    mesh, 
                    material: materials.add(StandardMaterial { base_color: color, perceptual_roughness: 0.9, reflectance: 0.05, ..default() }),
                    transform: BevyTransform::from_xyz(node.x, node.y + (y_offset * node.scale) + 0.05, node.z)
                        .with_scale(Vec3::splat(node.scale)),
                    ..default()
                },
                ResourceNodeItem { node_id: node.node_id },
                RigidBody::Static, collider,
                CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
            )).with_children(|parent| {
                if node.node_type == "Bush" {
                    let offsets = [
                        Vec3::new(0.6, 0.2, 0.0), Vec3::new(-0.4, 0.5, 0.5), 
                        Vec3::new(0.0, -0.3, 0.6), Vec3::new(0.5, -0.5, -0.5), 
                        Vec3::new(-0.5, 0.1, -0.5),
                    ];
                    
                    let red_material = materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.1, 0.1), ..default() });
                    for offset in offsets {
                        parent.spawn((
                            PbrBundle {
                                mesh: meshes.add(Sphere::new(0.15).mesh()),
                                material: red_material.clone(),
                                transform: BevyTransform::from_translation(offset),
                                ..default()
                            },
                            BerryVisual { node_id: node.node_id }
                        ));
                    }
                }
            });
        }
    }

    for (entity, node_item) in node_query.iter() {
        if !db_node_ids.contains(&node_item.node_id) { commands.entity(entity).despawn_recursive(); }
    }
}

pub fn update_berry_visuals(
    conn: Res<SpacetimeConnection>,
    mut query: Query<(&mut Visibility, &BerryVisual)>,
) {
    for (mut vis, berry) in query.iter_mut() {
        if let Some(node) = conn.db.db.resource_node().node_id().find(&berry.node_id) {
            *vis = if node.health > 0 { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

pub fn process_combat_events(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    conn: Res<SpacetimeConnection>,
    mut tracker: ResMut<EventTracker>,
) {
    let mut highest_id = tracker.last_event_id;

    for event in conn.db.db.combat_event().iter() {
        if event.id > tracker.last_event_id {
            highest_id = highest_id.max(event.id);
            
            let color = match event.event_type.as_str() {
                "HitTree" => Color::srgb(0.4, 0.2, 0.1), 
                "HitRock" => Color::srgb(0.5, 0.5, 0.5), 
                "HitBush" => Color::srgb(0.2, 0.6, 0.2), 
                "HitPlayer" => Color::srgb(0.9, 0.1, 0.1), 
                _ => Color::WHITE,
            };

            let velocities = [
                Vec3::new(1.0, 3.0, 1.0), Vec3::new(-1.0, 3.5, 0.5), Vec3::new(0.5, 2.5, -1.0),
                Vec3::new(-0.5, 4.0, -0.5), Vec3::new(0.0, 3.0, 0.0),
            ];

            for vel in velocities {
                commands.spawn((
                    PbrBundle {
                        mesh: meshes.add(Cuboid::new(0.1, 0.1, 0.1)),
                        material: materials.add(StandardMaterial { base_color: color, unlit: event.event_type == "HitPlayer", ..default() }),
                        transform: BevyTransform::from_xyz(event.x, event.y + 0.5, event.z),
                        ..default()
                    },
                    RigidBody::Dynamic, Collider::cuboid(0.1, 0.1, 0.1), LinearVelocity(vel),
                    Particle { timer: Timer::from_seconds(0.5, TimerMode::Once) }, 
                ));
            }
        }
    }
    
    tracker.last_event_id = highest_id;
}