use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use avian3d::prelude::*;
use tracing::info;
use spacetimedb_sdk::Table; 

use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::module_bindings::place_structure_reducer::place_structure; 
use crate::module_bindings::structure_table::StructureTableAccess; 

// ----------------------------------------------------------------------------
// DATA-DRIVEN CONFIGURATIONS
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModularPieceType {
    Foundation,
    Wall,
    Floor,
    Roof,
    Ramp,
}

impl ModularPieceType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Foundation => "Foundation",
            Self::Wall => "Wall",
            Self::Floor => "Floor",
            Self::Roof => "Roof",
            Self::Ramp => "Ramp",
        }
    }

    pub fn wood_cost(&self) -> u32 {
        match self {
            Self::Foundation => 20,
            Self::Wall => 10,
            Self::Floor => 15,
            Self::Roof => 15,
            Self::Ramp => 20,
        }
    }

    pub fn default_sockets(&self) -> Vec<Socket> {
        match self {
            Self::Foundation => vec![
                Socket { name: "Top".into(), local_offset: Vec3::new(0.0, 1.0, 0.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "North".into(), local_offset: Vec3::new(0.0, 2.0, -2.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "South".into(), local_offset: Vec3::new(0.0, 2.0, 2.0), local_rotation: Quat::from_rotation_y(std::f32::consts::PI), is_occupied: false },
                Socket { name: "East".into(), local_offset: Vec3::new(2.0, 2.0, 0.0), local_rotation: Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2), is_occupied: false },
                Socket { name: "West".into(), local_offset: Vec3::new(-2.0, 2.0, 0.0), local_rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), is_occupied: false },
            ],
            Self::Wall => vec![
                // Architectural Note: Added offset Edge sockets. This fixes the Roofs "splitting" the wall
                // by allowing 4x4 roofs to snap exactly 2.0 units outwards from the center of the thin wall profile.
                Socket { name: "TopCenter".into(), local_offset: Vec3::new(0.0, 1.5, 0.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "TopForward".into(), local_offset: Vec3::new(0.0, 1.5, -2.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "TopBackward".into(), local_offset: Vec3::new(0.0, 1.5, 2.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "BottomCenter".into(), local_offset: Vec3::new(0.0, -1.5, 0.0), local_rotation: Quat::IDENTITY, is_occupied: false },
            ],
            Self::Floor => vec![
                Socket { name: "Top".into(), local_offset: Vec3::new(0.0, 0.5, 0.0), local_rotation: Quat::IDENTITY, is_occupied: false },
            ],
            Self::Roof => vec![
                Socket { name: "Bottom".into(), local_offset: Vec3::new(0.0, 0.0, 0.0), local_rotation: Quat::IDENTITY, is_occupied: false },
            ],
            Self::Ramp => vec![
                // Architectural Note: Aligned Ramp snap points to complement the custom sloped mesh bounds.
                Socket { name: "Top".into(), local_offset: Vec3::new(0.0, 1.5, -2.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "Bottom".into(), local_offset: Vec3::new(0.0, -1.5, 2.0), local_rotation: Quat::IDENTITY, is_occupied: false },
            ],
        }
    }
}

#[derive(Resource)]
pub struct BuildModeState {
    pub is_active: bool,
    pub selected_piece: ModularPieceType,
}

impl Default for BuildModeState {
    fn default() -> Self {
        Self {
            is_active: false,
            selected_piece: ModularPieceType::Foundation,
        }
    }
}

// ----------------------------------------------------------------------------
// PROCEDURAL MESH GENERATION
// ----------------------------------------------------------------------------

/// Architectural Note: Dynamically generates a true wedge geometry for the Ramp piece.
/// This prevents us from having to rely on a generic 4x4 bounding block or importing an external GLTF.
pub fn create_ramp_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let positions = vec![
        [-2.0, -1.5,  2.0], [ 2.0, -1.5,  2.0], [ 2.0, -1.5, -2.0], [-2.0, -1.5, -2.0], // Bottom Face
        [ 2.0, -1.5, -2.0], [-2.0, -1.5, -2.0], [-2.0,  1.5, -2.0], [ 2.0,  1.5, -2.0], // Back Wall Face
        [-2.0, -1.5,  2.0], [ 2.0, -1.5,  2.0], [ 2.0,  1.5, -2.0], [-2.0,  1.5, -2.0], // Sloped Face
        [-2.0, -1.5,  2.0], [-2.0, -1.5, -2.0], [-2.0,  1.5, -2.0],                     // Left Triangle
        [ 2.0, -1.5,  2.0], [ 2.0, -1.5, -2.0], [ 2.0,  1.5, -2.0],                     // Right Triangle
    ];
    
    let normals = vec![
        [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0],
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0],
        [0.0, 0.8, 0.6], [0.0, 0.8, 0.6], [0.0, 0.8, 0.6], [0.0, 0.8, 0.6],
        [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0],
    ];
    
    let indices = vec![
        0, 2, 1,  0, 3, 2,       // Bottom
        4, 6, 5,  4, 7, 6,       // Back
        8, 9, 10,  8, 10, 11,    // Slope
        12, 14, 13,              // Left
        15, 16, 17,              // Right
    ];

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

// ----------------------------------------------------------------------------
// BUILD MODE & SNAPPING SYSTEMS
// ----------------------------------------------------------------------------

pub fn toggle_build_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut build_state: ResMut<BuildModeState>,
    mut commands: Commands,
    hologram_query: Query<Entity, With<BuildHologram>>,
) {
    if keys.just_pressed(KeyCode::KeyB) {
        build_state.is_active = !build_state.is_active;
        info!("Build Mode Active: {}", build_state.is_active);

        if !build_state.is_active {
            for entity in hologram_query.iter() {
                commands.entity(entity).despawn_recursive();
            }
        }
    }

    if build_state.is_active && keys.just_pressed(KeyCode::KeyR) {
        build_state.selected_piece = match build_state.selected_piece {
            ModularPieceType::Foundation => ModularPieceType::Wall,
            ModularPieceType::Wall => ModularPieceType::Floor,
            ModularPieceType::Floor => ModularPieceType::Roof,
            ModularPieceType::Roof => ModularPieceType::Ramp,
            ModularPieceType::Ramp => ModularPieceType::Foundation,
        };
        info!("Selected Modular Piece: {:?}", build_state.selected_piece);
        
        for entity in hologram_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn update_build_hologram(
    mut commands: Commands,
    build_state: Res<BuildModeState>,
    camera_query: Query<(&GlobalTransform, &Camera), With<FpsCamera>>,
    player_query: Query<Entity, With<PlayerBody>>, 
    spatial_query: SpatialQuery,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut hologram_query: Query<(Entity, &mut Transform), With<BuildHologram>>,
    structure_query: Query<&NetworkStructure>,
    children_query: Query<&Children>,
    socket_query: Query<(&GlobalTransform, &Socket)>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    conn: Res<SpacetimeConnection>,
) {
    if !build_state.is_active { return; }

    let Ok((cam_t, _camera)) = camera_query.get_single() else { return; };
    
    let ray_origin = cam_t.translation();
    let ray_dir = cam_t.forward();

    let hologram_entity = if let Ok((entity, _)) = hologram_query.get_single() {
        entity
    } else {
        let mesh = match build_state.selected_piece {
            ModularPieceType::Foundation => meshes.add(Cuboid::new(4.0, 1.0, 4.0)),
            ModularPieceType::Wall => meshes.add(Cuboid::new(4.0, 3.0, 0.4)),
            ModularPieceType::Floor => meshes.add(Cuboid::new(4.0, 0.2, 4.0)),
            ModularPieceType::Roof => meshes.add(Cuboid::new(4.0, 0.2, 4.0)),
            ModularPieceType::Ramp => meshes.add(create_ramp_mesh()), 
        };

        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.8, 1.0, 0.5),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });

        commands.spawn((
            PbrBundle { mesh, material, ..default() },
            BuildHologram,
        )).id()
    };

    let mut filter = SpatialQueryFilter::default();
    if let Ok(player_entity) = player_query.get_single() {
        filter = filter.with_excluded_entities([player_entity]);
    }

    let ray_hit = spatial_query.cast_ray(
        ray_origin,
        ray_dir.into(),
        50.0,
        true,
        filter,
    );

    let mut target_transform = Transform::from_xyz(0.0, 0.0, 0.0);
    let mut target_parent_id = None;
    let mut snapped = false;

    if let Some(hit) = ray_hit {
        if let Ok(net_struct) = structure_query.get(hit.entity) {
            target_parent_id = Some(net_struct.structure_id);
        }

        if let Ok(children) = children_query.get(hit.entity) {
            let hit_point = ray_origin + ray_dir * hit.time_of_impact;
            let mut closest_dist = 16.0; 
            
            for &child in children.iter() {
                if let Ok((socket_t, socket)) = socket_query.get(child) {
                    if !socket.is_occupied {
                        let dist = socket_t.translation().distance_squared(hit_point);
                        if dist < closest_dist {
                            closest_dist = dist;
                            target_transform.translation = socket_t.translation();
                            let (_, rotation, _) = socket_t.to_scale_rotation_translation();
                            target_transform.rotation = rotation; 
                            snapped = true;
                        }
                    }
                }
            }
        }

        if !snapped {
            target_parent_id = None; 
            let hit_point = ray_origin + ray_dir * hit.time_of_impact;
            let grid_size = 4.0;
            let snapped_x = (hit_point.x / grid_size).round() * grid_size;
            let snapped_z = (hit_point.z / grid_size).round() * grid_size;
            let true_y = crate::terrain::get_terrain_height(snapped_x, snapped_z);
            
            target_transform.translation = Vec3::new(snapped_x, true_y, snapped_z);
            target_transform.rotation = Quat::IDENTITY;
        }
    } else {
        target_transform.translation = ray_origin + ray_dir * 5.0;
    }

    if let Ok((_, mut transform)) = hologram_query.get_mut(hologram_entity) {
        *transform = target_transform;
    }

    if mouse_buttons.just_pressed(MouseButton::Left) {
        let pos = target_transform.translation;
        let rot = target_transform.rotation;
        let piece_name = build_state.selected_piece.name().to_string();

        info!("Dispatching place_structure reducer for {} at {:?}", piece_name, pos);
        
        let _ = conn.db.reducers.place_structure(
            target_parent_id, piece_name, pos.x, pos.y, pos.z, rot.x, rot.y, rot.z, rot.w,
        );
    }
}

pub fn sync_structures(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing_structures: Query<(Entity, &NetworkStructure)>,
) {
    let _ = conn.db.frame_tick();
    let db_structures: Vec<_> = conn.db.db.structure().iter().collect();
    let mut spawned_ids = std::collections::HashSet::with_capacity(existing_structures.iter().len());

    for (_entity, net_struct) in existing_structures.iter() {
        spawned_ids.insert(net_struct.structure_id);
    }

    for s in db_structures {
        if !spawned_ids.contains(&s.structure_id) {
            
            let (mesh, color, collider) = match s.piece_type.as_str() {
                "Foundation" => (meshes.add(Cuboid::new(4.0, 1.0, 4.0)), Color::srgb(0.5, 0.4, 0.3), Collider::cuboid(4.0, 1.0, 4.0)),
                "Wall" => (meshes.add(Cuboid::new(4.0, 3.0, 0.4)), Color::srgb(0.6, 0.5, 0.4), Collider::cuboid(4.0, 3.0, 0.4)),
                "Floor" => (meshes.add(Cuboid::new(4.0, 0.2, 4.0)), Color::srgb(0.5, 0.4, 0.3), Collider::cuboid(4.0, 0.2, 4.0)),
                "Roof" => (meshes.add(Cuboid::new(4.0, 0.2, 4.0)), Color::srgb(0.4, 0.3, 0.2), Collider::cuboid(4.0, 0.2, 4.0)),
                _ => { 
                    // Architectural Note: Ramps utilize the dynamically generated wedge mesh 
                    // and apply `trimesh_from_mesh` so the physics colliders match the visual slopes.
                    let ramp_mesh = create_ramp_mesh();
                    let col = Collider::trimesh_from_mesh(&ramp_mesh).unwrap_or_else(|| Collider::cuboid(4.0, 2.0, 4.0));
                    (meshes.add(ramp_mesh), Color::srgb(0.5, 0.5, 0.5), col)
                },
            };

            let transform = Transform::from_xyz(s.x, s.y, s.z)
                .with_rotation(Quat::from_xyzw(s.rot_x, s.rot_y, s.rot_z, s.rot_w));

            let sockets = match s.piece_type.as_str() {
                "Foundation" => ModularPieceType::Foundation.default_sockets(),
                "Wall" => ModularPieceType::Wall.default_sockets(),
                "Floor" => ModularPieceType::Floor.default_sockets(),
                "Roof" => ModularPieceType::Roof.default_sockets(),
                _ => ModularPieceType::Ramp.default_sockets(),
            };

            commands.spawn((
                PbrBundle {
                    mesh,
                    material: materials.add(StandardMaterial { base_color: color, perceptual_roughness: 0.8, ..default() }),
                    transform,
                    ..default()
                },
                RigidBody::Static,
                collider, 
                NetworkStructure { structure_id: s.structure_id },
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
            });
        }
    }
}