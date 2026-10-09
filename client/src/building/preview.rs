// ============================================================================
// File: client/src/building/preview.rs
// ============================================================================
// Ghost preview placement, socket snapping, and template blueprint stamping.

use bevy::prelude::*;
use tracing::info;

use crate::physics::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::templates::BuildingTemplateType;
use crate::module_bindings::place_structure_reducer::place_structure;
use crate::module_bindings::spawn_template_blueprint_reducer::spawn_template_blueprint;
use crate::module_bindings::structure_table::StructureTableAccess;

use super::types::*;

// ----------------------------------------------------------------------------
// BUILD MODE & SNAPPING SYSTEMS
// ----------------------------------------------------------------------------

pub fn toggle_build_mode(
    mut toggle_evts: EventReader<crate::input::ToggleBuildModeEvent>,
    mut tmpl_evts: EventReader<crate::input::BuildCycleTemplateEvent>,
    mut piece_evts: EventReader<crate::input::BuildCyclePieceEvent>,
    mut rot_evts: EventReader<crate::input::BuildRotatePieceEvent>,
    mut build_state: ResMut<BuildModeState>,
    mut commands: Commands,
    hologram_query: Query<Entity, With<BuildHologram>>,
) {
    for _ in toggle_evts.read() {
        build_state.is_active = !build_state.is_active;
        info!("Build Mode Active: {}", build_state.is_active);

        if !build_state.is_active {
            for entity in hologram_query.iter() {
                commands.entity(entity).despawn_recursive();
            }
        }
    }

    if build_state.is_active {
        for _ in tmpl_evts.read() {
            build_state.selected_template = match build_state.selected_template {
                None => Some(BuildingTemplateType::Watchtower),
                Some(BuildingTemplateType::Watchtower) => Some(BuildingTemplateType::Palisade),
                Some(BuildingTemplateType::Palisade) => Some(BuildingTemplateType::Cottage),
                Some(BuildingTemplateType::Cottage) => Some(BuildingTemplateType::Settlement),
                Some(BuildingTemplateType::Settlement) => None,
            };
            if let Some(t) = build_state.selected_template {
                info!("Selected Multi-Piece Template: {:?}", t);
            } else {
                info!("Switched to Single Modular Piece Mode: {:?}", build_state.selected_piece);
            }
            for entity in hologram_query.iter() {
                commands.entity(entity).despawn_recursive();
            }
        }

        for _ in piece_evts.read() {
            if let Some(ref mut t) = build_state.selected_template {
                *t = t.next();
                info!("Selected Multi-Piece Template: {:?}", t);
            } else {
                build_state.selected_piece = match build_state.selected_piece {
                    ModularPieceType::Foundation => ModularPieceType::Workbench,
                    ModularPieceType::Workbench => ModularPieceType::Campfire,
                    ModularPieceType::Campfire => ModularPieceType::Wall,
                    ModularPieceType::Wall => ModularPieceType::Window,
                    ModularPieceType::Window => ModularPieceType::Door,
                    ModularPieceType::Door => ModularPieceType::Floor,
                    ModularPieceType::Floor => ModularPieceType::Roof,
                    ModularPieceType::Roof => ModularPieceType::Ramp,
                    ModularPieceType::Ramp => ModularPieceType::Foundation,
                };
                info!("Selected Modular Piece: {:?}", build_state.selected_piece);
            }
            
            for entity in hologram_query.iter() {
                commands.entity(entity).despawn_recursive();
            }
        }

        for ev in rot_evts.read() {
            if ev.clockwise {
                build_state.rotation_steps = (build_state.rotation_steps.wrapping_sub(1)) % 4;
            } else {
                build_state.rotation_steps = (build_state.rotation_steps + 1) % 4;
            }
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
    mut hologram_query: Query<(Entity, &mut Transform, &Handle<StandardMaterial>), With<BuildHologram>>,
    structure_query: Query<&NetworkStructure>,
    parent_query: Query<&Parent>,
    socket_query: Query<(Entity, &GlobalTransform, &Socket)>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    conn: Res<SpacetimeConnection>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    if !build_state.is_active { return; }

    let Ok((cam_t, _camera)) = camera_query.get_single() else { return; };
    
    let ray_origin = cam_t.translation();
    let ray_dir = cam_t.forward();
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));

    let (hologram_entity, mat_handle) = if let Ok((entity, _, mat)) = hologram_query.get_single() {
        (entity, mat.clone())
    } else {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.8, 1.0, 0.5),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });

        let entity = if let Some(template_type) = build_state.selected_template {
            #[allow(deprecated)]
            let tmpl = template_type.to_template(build_state.selected_faction.into());
            let root_id = commands.spawn((
                SpatialBundle::default(),
                material.clone(),
                BuildHologram,
            )).id();

            for (offset, piece) in &tmpl.blocks {
                let modular_piece = ModularPieceType::from(*piece);
                let mesh = mesh_cache.get_piece_mesh(modular_piece, VisualDamageState::Pristine);
                let local_pos = Vec3::new(offset.0 as f32 * 4.0, offset.1 as f32 * 3.0, offset.2 as f32 * 4.0);
                let child_id = commands.spawn(PbrBundle {
                    mesh,
                    material: material.clone(),
                    transform: Transform::from_translation(local_pos),
                    ..default()
                }).id();
                commands.entity(root_id).add_child(child_id);
            }
            root_id
        } else {
            let mesh = mesh_cache.get_piece_mesh(build_state.selected_piece, VisualDamageState::Pristine);
            commands.spawn((
                PbrBundle { mesh, material: material.clone(), ..default() },
                BuildHologram,
            )).id()
        };

        (entity, material)
    };

    let mut filter = SpatialQueryFilter::default();
    if let Ok(player_entity) = player_query.get_single() {
        filter = filter.with_excluded_entities([player_entity]);
    }

    let ray_hit = spatial_query.cast_ray(
        ray_origin,
        *ray_dir,
        50.0,
        true,
        filter,
    );

    let mut target_transform = Transform::from_xyz(0.0, 0.0, 0.0);
    let mut target_parent_id = None;
    let mut snapped = false;

    let manual_rotation_offset = Quat::from_rotation_y(build_state.rotation_steps as f32 * std::f32::consts::FRAC_PI_2);

    if let Some(hit) = ray_hit {
        let hit_point = ray_origin + ray_dir * hit.time_of_impact;
        let mut closest_dist = 25.0; // 5.0m max snapping radius

        for (child_ent, socket_t, socket) in socket_query.iter() {
            if !socket.is_occupied && is_socket_compatible(build_state.selected_piece, &socket.name) {
                let socket_pos = socket_t.translation();
                let dx = socket_pos.x - hit_point.x;
                let dz = socket_pos.z - hit_point.z;
                let dy = (socket_pos.y - hit_point.y).abs();

                // Allow up to 3.5m vertical difference, strongly prioritize 2D horizontal proximity to edge
                if dy <= 3.5 && (dx * dx + dz * dz) <= 25.0 {
                    let dist_metric = dx * dx + dz * dz + (dy * 0.35).powi(2);
                    if dist_metric < closest_dist {
                        closest_dist = dist_metric;
                        target_transform.translation = socket_pos;
                        let (_, rotation, _) = socket_t.to_scale_rotation_translation();
                        target_transform.rotation = rotation * manual_rotation_offset;
                        snapped = true;

                        if let Ok(parent) = parent_query.get(child_ent) {
                            if let Ok(net_struct) = structure_query.get(parent.get()) {
                                target_parent_id = Some(net_struct.structure_id);
                            }
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
            
            let y_lift = if build_state.selected_template.is_some() || build_state.selected_piece == ModularPieceType::Foundation {
                0.5
            } else {
                0.0
            };

            target_transform.translation = Vec3::new(snapped_x, true_y + y_lift, snapped_z);
            target_transform.rotation = Quat::IDENTITY * manual_rotation_offset;
        }
    } else {
        target_transform.translation = ray_origin + ray_dir * 5.0;
        target_transform.rotation = Quat::IDENTITY * manual_rotation_offset;
    }

    let is_starter_piece = build_state.selected_template.is_some() || matches!(
        build_state.selected_piece,
        ModularPieceType::Foundation | ModularPieceType::Workbench | ModularPieceType::Campfire
    );

    let near_workbench = structure_query.iter().any(|net_struct| {
        if let Some(s) = conn.db.db.structure().structure_id().find(&net_struct.structure_id) {
            if s.piece_type == "Workbench" && !s.is_blueprint {
                let dist_sq = (s.x - target_transform.translation.x).powi(2) + (s.z - target_transform.translation.z).powi(2);
                return dist_sq <= 400.0;
            }
        }
        false
    });

    let is_valid_placement = if is_starter_piece {
        snapped || ray_hit.is_some()
    } else {
        snapped && near_workbench
    };

    if let Some(mat) = materials.get_mut(&mat_handle) {
        mat.base_color = if is_valid_placement {
            Color::srgba(0.2, 0.9, 0.2, 0.5)
        } else {
            Color::srgba(0.9, 0.2, 0.2, 0.5)
        };
    }

    if let Ok((_, mut transform, _)) = hologram_query.get_mut(hologram_entity) {
        *transform = target_transform;
    }

    if mouse_buttons.just_pressed(MouseButton::Left) && is_valid_placement {
        let pos = target_transform.translation;
        let rot = target_transform.rotation;

        if let Some(template_type) = build_state.selected_template {
            let tmpl_name = template_type.to_api_name();
            let faction_str = "Human";
            info!("Dispatching spawn_template_blueprint reducer for {} ({}) at {:?}", tmpl_name, faction_str, pos);

            let res = conn.db.reducers.spawn_template_blueprint(
                tmpl_name.to_string(),
                faction_str.to_string(),
                pos.x,
                pos.z,
            );
            if let Err(e) = res {
                tracing::error!("Failed to spawn template blueprint: {:?}", e);
            }
        } else {
            let piece_name = build_state.selected_piece.name().to_string();

            info!("Dispatching place_structure reducer for {} at {:?}", piece_name, pos);
            
            let res = conn.db.reducers.place_structure(
                target_parent_id, piece_name, pos.x, pos.y, pos.z, rot.x, rot.y, rot.z, rot.w,
            );
            if let Err(e) = res {
                tracing::error!("Failed to place structure: {:?}", e);
            }
        }
    }
}
