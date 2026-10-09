// ============================================================================
// File: client/src/resource_nodes.rs
// ============================================================================
// ----------------------------------------------------------------------------
// HARVESTABLE RESOURCE NODE ENTITY REPLICATION & SIMULATION
// ----------------------------------------------------------------------------
// Architectural Note:
// Manages the lifecycle, 3D entity generation, collider assignment, and
// visual physics simulation (falling trees, impact gibs) for harvestable
// environment nodes replicated from the SpacetimeDB `resource_node` table:
// - Trees: Oak, Pine, Round, Dead (with species & variant assignments)
// - Fallen Logs
// - Granite Boulders
// - Berry Bushes (shadow-casting foliage + non-shadowcaster ruby berries)
// - Fallen Branches, Knapped Flint, Loose River Stones
// ----------------------------------------------------------------------------

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::pbr::NotShadowCaster;
use crate::physics::*;
use std::collections::BTreeSet;

use spacetimedb_sdk::Table;
use crate::module_bindings::resource_node_table::ResourceNodeTableAccess;
use crate::network::SpacetimeConnection;
use crate::components::*;
use crate::core::GameLayer;
use crate::trees::{TreeMeshCache, create_lowpoly_fallen_log_mesh};
use crate::props::{
    create_lowpoly_rock_mesh, create_lowpoly_bush_mesh, create_lowpoly_bush_foliage_mesh,
    create_lowpoly_bush_berries_mesh, create_lowpoly_branch_mesh, create_lowpoly_flint_mesh,
    create_lowpoly_stone_mesh,
};

/// Maximum distance (m) to load resource nodes into the client ECS bubble.
/// 128m radius provides a dense, continuous forest and resource field around the player
/// while keeping active ECS entities strictly within the 2,000–3,500 budget (~2,400 entities).
pub const RESOURCE_NODE_LOAD_RADIUS: f32 = 128.0;
pub const RESOURCE_NODE_LOAD_RADIUS_SQ: f32 = RESOURCE_NODE_LOAD_RADIUS * RESOURCE_NODE_LOAD_RADIUS; // 16,384 m^2

/// Unload distance (m) with hysteresis to prevent churn at the boundary.
pub const RESOURCE_NODE_UNLOAD_RADIUS: f32 = 144.0;
pub const RESOURCE_NODE_UNLOAD_RADIUS_SQ: f32 = RESOURCE_NODE_UNLOAD_RADIUS * RESOURCE_NODE_UNLOAD_RADIUS; // 20,736 m^2

/// Near distance threshold (m) within which trees and foliage cast directional shadows (cascades 0 & 1).
pub const TREE_SHADOW_NEAR_DIST: f32 = 56.0;
pub const TREE_SHADOW_NEAR_DIST_SQ: f32 = TREE_SHADOW_NEAR_DIST * TREE_SHADOW_NEAR_DIST; // 3,136 m^2

/// Far distance threshold (m) beyond which tree and foliage shadows are culled via `NotShadowCaster`.
pub const TREE_SHADOW_FAR_DIST: f32 = 64.0;
pub const TREE_SHADOW_FAR_DIST_SQ: f32 = TREE_SHADOW_FAR_DIST * TREE_SHADOW_FAR_DIST; // 4,096 m^2

/// Small ground clutter load radius (m) (Flint, Branch, LooseStone)
pub const SMALL_CLUTTER_LOAD_DIST: f32 = 36.0;
pub const SMALL_CLUTTER_LOAD_DIST_SQ: f32 = SMALL_CLUTTER_LOAD_DIST * SMALL_CLUTTER_LOAD_DIST; // 1,296 m^2

/// Small ground clutter unload radius (m) with 8m hysteresis
pub const SMALL_CLUTTER_UNLOAD_DIST: f32 = 44.0;
pub const SMALL_CLUTTER_UNLOAD_DIST_SQ: f32 = SMALL_CLUTTER_UNLOAD_DIST * SMALL_CLUTTER_UNLOAD_DIST; // 1,936 m^2

/// Medium node load radius (m) (Bush, FallenLog)
pub const MEDIUM_NODE_LOAD_DIST: f32 = 64.0;
pub const MEDIUM_NODE_LOAD_DIST_SQ: f32 = MEDIUM_NODE_LOAD_DIST * MEDIUM_NODE_LOAD_DIST; // 4,096 m^2

/// Medium node unload radius (m) with 8m hysteresis
pub const MEDIUM_NODE_UNLOAD_DIST: f32 = 72.0;
pub const MEDIUM_NODE_UNLOAD_DIST_SQ: f32 = MEDIUM_NODE_UNLOAD_DIST * MEDIUM_NODE_UNLOAD_DIST; // 5,184 m^2

#[inline]
pub fn is_small_clutter(node_type: &str) -> bool {
    let clean = node_type.trim();
    clean == "Branch" || clean == "Flint" || clean == "LooseStone"
}

#[inline]
pub fn is_medium_node(node_type: &str) -> bool {
    let clean = node_type.trim();
    clean == "Bush" || clean == "FallenLog"
}

#[inline]
pub fn get_node_load_radius_sq(node_type: &str, max_load_radius_sq: f32) -> f32 {
    if is_small_clutter(node_type) {
        SMALL_CLUTTER_LOAD_DIST_SQ.min(max_load_radius_sq)
    } else if is_medium_node(node_type) {
        MEDIUM_NODE_LOAD_DIST_SQ.min(max_load_radius_sq)
    } else {
        max_load_radius_sq
    }
}

#[inline]
pub fn get_node_unload_radius_sq(node_type: &str, max_unload_radius_sq: f32) -> f32 {
    if is_small_clutter(node_type) {
        SMALL_CLUTTER_UNLOAD_DIST_SQ.min(max_unload_radius_sq)
    } else if is_medium_node(node_type) {
        MEDIUM_NODE_UNLOAD_DIST_SQ.min(max_unload_radius_sq)
    } else {
        max_unload_radius_sq
    }
}

/// Stores the original collider definition for dynamic physics distance culling.
/// Prevents distant static nodes (>56m) from populating Rapier3D's broadphase BVH.
#[derive(Component, Clone)]
pub struct NodeCollider(pub Collider);

/// Identifies whether a resource node is a tree species or bush foliage eligible for distance shadow culling.
#[inline]
pub fn is_tree_or_bush_type(node_type: &str) -> bool {
    let clean = node_type.trim();
    clean == "Bush"
        || clean == "Tree"
        || clean == "Oak"
        || clean == "Pine"
        || clean == "Dead"
        || clean == "Round"
        || clean.starts_with("Tree")
        || clean.ends_with(":Oak")
        || clean.ends_with(":Pine")
        || clean.ends_with(":Dead")
        || clean.ends_with(":Round")
}

/// Cached GPU mesh and material handles for harvestable resource nodes and environment clutter.
pub struct CachedResourceMeshes {
    pub tree_cache: TreeMeshCache,
    pub fallen_log: Handle<Mesh>,
    pub rock: Handle<Mesh>,
    pub rock_variants: Vec<Handle<Mesh>>,
    #[allow(dead_code)] pub bush: Handle<Mesh>,
    pub bush_variants: Vec<Handle<Mesh>>,
    pub bush_foliage_variants: Vec<Handle<Mesh>>,
    pub bush_berries_variants: Vec<Handle<Mesh>>,
    pub branch: Handle<Mesh>,
    pub flint: Handle<Mesh>,
    pub stone: Handle<Mesh>,
    pub default_mat: Handle<StandardMaterial>,
    pub iron_mat: Handle<StandardMaterial>,
    pub ruby_mat: Handle<StandardMaterial>,
    pub rubble_mat: Handle<StandardMaterial>,
}

impl CachedResourceMeshes {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let tree_cache = TreeMeshCache::new(meshes);
        let fallen_log = meshes.add(create_lowpoly_fallen_log_mesh(5050));

        let rock_variants = vec![
            meshes.add(create_lowpoly_rock_mesh(1337)),
            meshes.add(create_lowpoly_rock_mesh(2468)),
            meshes.add(create_lowpoly_rock_mesh(3579)),
            meshes.add(create_lowpoly_rock_mesh(4680)),
        ];
        let rock = rock_variants[0].clone();

        let bush_foliage_variants = vec![
            meshes.add(create_lowpoly_bush_foliage_mesh(1337)),
            meshes.add(create_lowpoly_bush_foliage_mesh(2468)),
            meshes.add(create_lowpoly_bush_foliage_mesh(3579)),
            meshes.add(create_lowpoly_bush_foliage_mesh(4680)),
        ];
        let bush_berries_variants = vec![
            meshes.add(create_lowpoly_bush_berries_mesh(1337)),
            meshes.add(create_lowpoly_bush_berries_mesh(2468)),
            meshes.add(create_lowpoly_bush_berries_mesh(3579)),
            meshes.add(create_lowpoly_bush_berries_mesh(4680)),
        ];
        let bush_variants = vec![
            meshes.add(create_lowpoly_bush_mesh(1337)),
            meshes.add(create_lowpoly_bush_mesh(2468)),
            meshes.add(create_lowpoly_bush_mesh(3579)),
            meshes.add(create_lowpoly_bush_mesh(4680)),
        ];
        let bush = bush_variants[0].clone();

        let default_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.1,
            ..default()
        });
        let iron_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.65, 0.38, 0.22),
            metallic: 0.75,
            perceptual_roughness: 0.40,
            reflectance: 0.5,
            ..default()
        });
        let ruby_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.05, 0.15),
            metallic: 0.2,
            perceptual_roughness: 0.15,
            reflectance: 0.8,
            emissive: Color::srgb(0.40, 0.02, 0.05).into(),
            ..default()
        });
        let rubble_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.46, 0.44),
            metallic: 0.05,
            perceptual_roughness: 0.90,
            reflectance: 0.1,
            ..default()
        });

        Self {
            tree_cache,
            fallen_log,
            rock,
            rock_variants,
            bush,
            bush_variants,
            bush_foliage_variants,
            bush_berries_variants,
            branch: meshes.add(create_lowpoly_branch_mesh(1337)),
            flint: meshes.add(create_lowpoly_flint_mesh(1337)),
            stone: meshes.add(create_lowpoly_stone_mesh(1337)),
            default_mat,
            iron_mat,
            ruby_mat,
            rubble_mat,
        }
    }
}

pub fn sync_resource_nodes(
    mut commands: Commands, 
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    node_query: Query<(Entity, &ResourceNodeItem, &BevyTransform, Has<NotShadowCaster>, Has<Collider>, Option<&NodeCollider>)>, 
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    conn: Res<SpacetimeConnection>,
    tree_mats: Option<Res<crate::tree_colors::TreeMaterialHandles>>,
    render_settings: Option<Res<crate::spellbook::TerrainRenderSettings>>,
    mut local_nodes: Local<BTreeSet<u64>>,
    mut scan_timer: Local<Option<Timer>>,
    mut model_cache: Local<Option<CachedResourceMeshes>>,
) {
    let Ok(player_transform) = player_query.get_single() else { return; };
    let player_pos = player_transform.translation;

    let timer = scan_timer.get_or_insert_with(|| Timer::from_seconds(0.1, TimerMode::Repeating));
    if !timer.tick(time.delta()).just_finished() {
        return;
    }

    let cache = model_cache.get_or_insert_with(|| CachedResourceMeshes::new(&mut meshes, &mut materials));
    let node_mat = cache.default_mat.clone();

    // Synchronize resource node load/unload distance with performance budget (128m load / 144m unload)
    let (node_load_radius, node_unload_radius) = if let Some(ref rs) = render_settings {
        if rs.spawn_full_zone {
            (64.0 * 16.0, 70.0 * 16.0)
        } else {
            let vr = (rs.view_distance_chunks as f32 * 16.0).min(RESOURCE_NODE_LOAD_RADIUS);
            let ur = (rs.unload_distance_chunks.max(rs.view_distance_chunks + 1) as f32 * 16.0)
                .min(RESOURCE_NODE_UNLOAD_RADIUS)
                .max(vr + 16.0);
            (vr, ur)
        }
    } else {
        (RESOURCE_NODE_LOAD_RADIUS, RESOURCE_NODE_UNLOAD_RADIUS)
    };
    let node_load_radius_sq = node_load_radius * node_load_radius;
    let node_unload_radius_sq = node_unload_radius * node_unload_radius;

    local_nodes.clear();

    for (entity, node_item, transform, has_not_shadow, has_collider, maybe_node_collider) in node_query.iter() {
        let origin = transform.translation;
        let dist_sq = (origin.x - player_pos.x).powi(2) + (origin.z - player_pos.z).powi(2);

        let server_node = conn.db.db.resource_node().node_id().find(&node_item.node_id);

        if server_node.is_none() {
            if dist_sq <= node_unload_radius_sq {
                let clean_item_type = node_item.node_type.trim();
                match clean_item_type {
                    "Tree" | "Oak" | "Pine" | "Dead" | "Round" | _ if clean_item_type.starts_with("Tree") => {
                        let mut seed = (origin.x.abs() * 1000.0 + origin.z.abs() * 100.0) as u64;
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        let angle_rand = ((seed as f32) / (u32::MAX as f32)) * std::f32::consts::TAU;
                        let fall_dir = Vec3::new(angle_rand.cos(), 0.0, angle_rand.sin()).normalize();

                        let biome = crate::tree_colors::get_biome(origin.y);
                        let tree_style = if clean_item_type == "Oak" || clean_item_type.ends_with(":Oak") {
                            1
                        } else if clean_item_type == "Pine" || clean_item_type.ends_with(":Pine") {
                            2
                        } else if clean_item_type == "Dead" || clean_item_type.ends_with(":Dead") {
                            0
                        } else if clean_item_type == "Round" || clean_item_type.ends_with(":Round") {
                            3
                        } else {
                            match crate::tree_colors::pick_tree_type_spatial(biome, origin.x, origin.z, node_item.node_id) {
                                "Dead" => 0,
                                "Oak" => 1,
                                "Pine" => 2,
                                _ => 3,
                            }
                        };

                        let variant_idx = (node_item.node_id % 6) as usize;
                        let tree_mesh = match tree_style {
                            0 => cache.tree_cache.dead_tree_variants[variant_idx].clone(),
                            1 => cache.tree_cache.oak_tree_variants[variant_idx].clone(),
                            2 => cache.tree_cache.pine_tree_variants[variant_idx].clone(),
                            _ => cache.tree_cache.round_tree_variants[variant_idx].clone(),
                        };

                        let tree_mat = if let Some(ref mats) = tree_mats {
                            match tree_style {
                                0 => mats.dead.clone(),
                                1 => mats.oak.clone(),
                                2 => mats.pine.clone(),
                                _ => mats.round.clone(),
                            }
                        } else {
                            node_mat.clone()
                        };

                        commands.spawn((
                            PbrBundle {
                                mesh: tree_mesh,
                                material: tree_mat,
                                transform: *transform,
                                ..default()
                            },
                            FallingTree {
                                base_pos: origin,
                                fall_dir,
                                initial_rotation: transform.rotation,
                                angle: 0.0,
                                angular_vel: 0.35,
                                elapsed: 0.0,
                            },
                        ));
                    }
                    "Rock" => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            origin,
                            24,
                            Color::srgb(0.45, 0.45, 0.48),
                            Color::srgb(0.65, 0.65, 0.68),
                            0.10,
                        );
                    }
                    "Ore:Iron" => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            origin,
                            28,
                            Color::srgb(0.65, 0.32, 0.18),
                            Color::srgb(0.85, 0.50, 0.25),
                            0.12,
                        );
                    }
                    "Gem:Ruby" => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            origin,
                            32,
                            Color::srgb(0.95, 0.08, 0.15),
                            Color::srgb(1.00, 0.35, 0.45),
                            0.08,
                        );
                    }
                    "Rubble" | "CollapsedRubble" => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            origin,
                            30,
                            Color::srgb(0.40, 0.38, 0.35),
                            Color::srgb(0.85, 0.72, 0.25),
                            0.11,
                        );
                    }
                    "Bush" => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            origin,
                            16,
                            Color::srgb(0.24, 0.58, 0.20),
                            Color::srgb(0.92, 0.12, 0.16),
                            0.06,
                        );
                    }
                    _ => {}
                }
            }
            commands.entity(entity).despawn_recursive();
        } else if dist_sq > get_node_unload_radius_sq(&node_item.node_type, node_unload_radius_sq) {
            commands.entity(entity).despawn_recursive();
        } else {
            local_nodes.insert(node_item.node_id);

            // Dynamic shadow caster distance culling with 8m hysteresis (56m near / 64m far):
            // Trees, bushes, rocks, ore, and rubble beyond 64m are stripped of shadow casting.
            // When moving closer (<56m), shadow casting is restored.
            // Small ground clutter (Branch, Flint, LooseStone) permanently retains NotShadowCaster.
            if !is_small_clutter(&node_item.node_type) {
                if !has_not_shadow && dist_sq > TREE_SHADOW_FAR_DIST_SQ {
                    commands.entity(entity).insert(NotShadowCaster);
                } else if has_not_shadow && dist_sq < TREE_SHADOW_NEAR_DIST_SQ {
                    commands.entity(entity).remove::<NotShadowCaster>();
                }
            }

            // Dynamic physics collider culling with 8m hysteresis (48m near / 56m far):
            // Strips Rapier3D colliders on distant static resource nodes to relieve broadphase BVH.
            if let Some(nc) = maybe_node_collider {
                if has_collider && dist_sq > crate::terrain::LOW_POLY_FAR_COLLIDER_UNLOAD_SQ {
                    commands.entity(entity).remove::<Collider>();
                } else if !has_collider && dist_sq <= crate::terrain::LOW_POLY_NEAR_COLLIDER_DIST_SQ {
                    commands.entity(entity).insert(nc.0.clone());
                }
            }
        }
    }

    let mut spawned_this_tick = 0;
    for node in conn.db.db.resource_node().iter() {
        let dist_sq = (node.x - player_pos.x).powi(2) + (node.z - player_pos.z).powi(2);
        let clean_type = node.node_type.trim();
        let req_load_sq = get_node_load_radius_sq(clean_type, node_load_radius_sq);
        if dist_sq > req_load_sq || local_nodes.contains(&node.node_id) {
            continue;
        }

        let clean_type = node.node_type.trim();

        let (mesh, collider, y_offset, tree_comp_opt) = match clean_type {
            "Tree" | "Oak" | "Pine" | "Dead" | "Round" | _ if clean_type.starts_with("Tree") => {
                let biome = crate::tree_colors::get_biome(node.y);
                let tree_style = if clean_type == "Oak" || clean_type.ends_with(":Oak") {
                    1
                } else if clean_type == "Pine" || clean_type.ends_with(":Pine") {
                    2
                } else if clean_type == "Dead" || clean_type.ends_with(":Dead") {
                    0
                } else if clean_type == "Round" || clean_type.ends_with(":Round") {
                    3
                } else {
                    match crate::tree_colors::pick_tree_type_spatial(biome, node.x, node.z, node.node_id) {
                        "Dead" => 0,
                        "Oak" => 1,
                        "Pine" => 2,
                        _ => 3,
                    }
                };

                let variant_idx = (node.node_id % 6) as usize;
                let tree_mesh = match tree_style {
                    0 => cache.tree_cache.dead_tree_variants[variant_idx].clone(),
                    1 => cache.tree_cache.oak_tree_variants[variant_idx].clone(),
                    2 => cache.tree_cache.pine_tree_variants[variant_idx].clone(),
                    _ => cache.tree_cache.round_tree_variants[variant_idx].clone(),
                };
                (
                    tree_mesh,
                    Collider::cylinder(0.45, 17.0),
                    0.0,
                    Some(crate::components::TreeComponent { species: tree_style, variant: variant_idx }),
                )
            }
            "FallenLog" => (
                cache.fallen_log.clone(),
                Collider::cylinder(0.35, 3.2),
                0.35,
                None,
            ),
            "Rock" => (
                cache.rock_variants[(node.node_id % 4) as usize].clone(),
                Collider::cuboid(1.5, 1.4, 1.4),
                0.0,
                None,
            ),
            "Ore:Iron" => (
                cache.rock_variants[(node.node_id % 4) as usize].clone(),
                Collider::cuboid(1.4, 1.3, 1.4),
                0.0,
                None,
            ),
            "Gem:Ruby" => (
                cache.rock_variants[(node.node_id % 4) as usize].clone(),
                Collider::sphere(0.65),
                0.0,
                None,
            ),
            "Rubble" | "CollapsedRubble" => (
                cache.rock_variants[(node.node_id % 4) as usize].clone(),
                Collider::cuboid(1.8, 1.0, 1.8),
                0.0,
                None,
            ),
            "Bush" => (
                cache.bush_variants[(node.node_id % 4) as usize].clone(),
                Collider::sphere(0.85),
                0.0,
                None,
            ),
            "Branch" => (
                cache.branch.clone(),
                Collider::cuboid(1.5, 0.28, 0.9),
                0.02,
                None,
            ),
            "Flint" => (
                cache.flint.clone(),
                Collider::cuboid(0.55, 0.75, 0.45),
                0.02,
                None,
            ),
            "LooseStone" => (
                cache.stone.clone(),
                Collider::cuboid(1.2, 0.58, 0.95),
                0.02,
                None,
            ),
            _ => (
                cache.stone.clone(),
                Collider::cuboid(0.5, 0.5, 0.5),
                0.0,
                None,
            )
        };

        let material = if let Some(ref tc) = tree_comp_opt {
            if let Some(ref mats) = tree_mats {
                match tc.species {
                    0 => mats.dead.clone(),
                    1 => mats.oak.clone(),
                    2 => mats.pine.clone(),
                    _ => mats.round.clone(),
                }
            } else {
                node_mat.clone()
            }
        } else if clean_type == "Ore:Iron" {
            cache.iron_mat.clone()
        } else if clean_type == "Gem:Ruby" {
            cache.ruby_mat.clone()
        } else if clean_type == "CollapsedRubble" || clean_type == "Rubble" {
            cache.rubble_mat.clone()
        } else {
            cache.default_mat.clone()
        };

        let is_rock_or_ore = clean_type == "Rock"
            || clean_type == "Ore:Iron"
            || clean_type == "Gem:Ruby"
            || clean_type == "CollapsedRubble"
            || clean_type == "Rubble";

        let (tree_rotation, tree_scale) = if tree_comp_opt.is_some() || is_rock_or_ore {
            // Stable deterministic pseudo-random hash based on node_id
            let hash1 = ((node.node_id.wrapping_mul(2654435761) ^ (node.node_id >> 16)) % 10000) as f32 / 10000.0;
            let hash2 = (((node.node_id.wrapping_mul(1664525) + 1013904223) ^ (node.node_id >> 11)) % 10000) as f32 / 10000.0;

            // Randomized degree of rotation around vertical axis (0 to 360 degrees)
            let yaw = hash1 * std::f32::consts::TAU;

            let (width_var, height_var) = if clean_type == "Gem:Ruby" {
                (0.6 + hash1 * 0.15, 0.6 + hash2 * 0.15)
            } else if clean_type == "CollapsedRubble" || clean_type == "Rubble" {
                (1.5 + hash1 * 0.30, 0.8 + hash2 * 0.20)
            } else if clean_type == "Rock" || clean_type == "Ore:Iron" {
                let h = 0.85 + (hash2 * 0.30);
                let w = 0.90 + (hash1 * 0.20);
                (w, h)
            } else {
                let h = 0.80 + (hash2 * 0.40);
                let w = 0.90 + (hash1 * 0.20);
                (w, h)
            };

            (Quat::from_rotation_y(yaw), Vec3::new(width_var * node.scale, height_var * node.scale, width_var * node.scale))
        } else {
            (Quat::IDENTITY, Vec3::splat(node.scale))
        };

        let is_solid = match clean_type {
            "Branch" | "Flint" | "LooseStone" | "Bush" => false,
            _ => true,
        };

        let mut entity_cmd = commands.spawn((
            PbrBundle {
                mesh, 
                material: material.clone(),
                transform: BevyTransform {
                    translation: Vec3::new(node.x, node.y + y_offset, node.z),
                    rotation: tree_rotation,
                    scale: tree_scale,
                },
                ..default()
            },
            ResourceNodeItem { 
                node_id: node.node_id,
                node_type: clean_type.to_string(),
            },
            NodeCollider(collider.clone()),
        ));

        let needs_near_collider = dist_sq <= crate::terrain::LOW_POLY_NEAR_COLLIDER_DIST_SQ;

        if is_solid {
            entity_cmd.insert((
                RigidBody::Static,
                CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
            ));
            if needs_near_collider {
                entity_cmd.insert(collider);
            }
        } else {
            entity_cmd.insert((
                Sensor,
                CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
            ));
            if needs_near_collider {
                entity_cmd.insert(collider);
            }
        }

        if let Some(tc) = tree_comp_opt {
            entity_cmd.insert(tc);
        }

        // Performance Optimization: Exclude small ground clutter permanently and cull distant
        // tree, bush, and rock shadows beyond 56m at spawn time.
        if is_small_clutter(clean_type) || dist_sq > TREE_SHADOW_NEAR_DIST_SQ {
            entity_cmd.insert(NotShadowCaster);
        }

        local_nodes.insert(node.node_id);
        spawned_this_tick += 1;
        if spawned_this_tick >= 300 {
            break;
        }
    }
}

pub fn update_falling_trees(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut FallingTree, &mut BevyTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_seconds().min(0.1);
    for (entity, mut falling, mut transform) in query.iter_mut() {
        falling.elapsed += dt;

        let gravity_torque = 5.2 * (falling.angle.sin().max(0.15));
        falling.angular_vel += gravity_torque * dt;
        falling.angle += falling.angular_vel * dt;

        let tilt_axis = Vec3::new(falling.fall_dir.z, 0.0, -falling.fall_dir.x).normalize();
        let rot = Quat::from_axis_angle(tilt_axis, falling.angle);

        let kickback = falling.fall_dir * (falling.angle * 0.25);
        transform.translation = falling.base_pos + kickback;
        transform.rotation = rot * falling.initial_rotation;

        let tip_world = transform.translation + rot * Vec3::new(0.0, 18.0, 0.0);
        let mid_world = transform.translation + rot * Vec3::new(0.0, 10.0, 0.0);

        let ground_y_at_tip = crate::terrain::get_terrain_height(tip_world.x, tip_world.z);
        let ground_y_at_mid = crate::terrain::get_terrain_height(mid_world.x, mid_world.z);

        let tip_hit_ground = tip_world.y <= ground_y_at_tip + 0.35;
        let mid_hit_ground = mid_world.y <= ground_y_at_mid + 0.35;
        let reached_parallel = falling.angle >= (std::f32::consts::FRAC_PI_2 - 0.04);
        let timeout = falling.elapsed >= 3.5;

        let has_impacted = (falling.angle >= 0.75 && (tip_hit_ground || mid_hit_ground))
            || reached_parallel
            || timeout;

        if has_impacted {
            let base = transform.translation;
            let dir = falling.fall_dir;

            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                base + dir * 3.0,
                24,
                Color::srgb(0.34, 0.22, 0.12),
                Color::srgb(0.44, 0.28, 0.15),
                0.12,
            );

            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                base + dir * 10.0,
                32,
                Color::srgb(0.34, 0.22, 0.12),
                Color::srgb(0.20, 0.55, 0.20),
                0.12,
            );

            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                base + dir * 17.0,
                40,
                Color::srgb(0.18, 0.55, 0.18),
                Color::srgb(0.26, 0.68, 0.26),
                0.10,
            );

            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn update_berry_visuals(
    _conn: Res<SpacetimeConnection>,
    _query: Query<(&mut Visibility, &BerryVisual)>,
) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_resource_meshes_initialization() {
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let cache = CachedResourceMeshes::new(&mut meshes, &mut materials);

        assert_eq!(cache.bush_variants.len(), 4);
        assert_eq!(cache.bush_foliage_variants.len(), 4);
        assert_eq!(cache.bush_berries_variants.len(), 4);
        assert_eq!(cache.rock_variants.len(), 4);
        for rock_handle in &cache.rock_variants {
            assert!(meshes.get(rock_handle).is_some());
        }
        assert!(meshes.get(&cache.fallen_log).is_some());
        assert!(meshes.get(&cache.rock).is_some());
        assert!(meshes.get(&cache.branch).is_some());
        assert!(meshes.get(&cache.flint).is_some());
        assert!(meshes.get(&cache.stone).is_some());
        assert!(materials.get(&cache.default_mat).is_some());
        assert!(materials.get(&cache.iron_mat).is_some());
        assert!(materials.get(&cache.ruby_mat).is_some());
        assert!(materials.get(&cache.rubble_mat).is_some());
    }

    #[test]
    fn test_is_tree_or_bush_type_classification() {
        assert!(is_tree_or_bush_type("Tree"));
        assert!(is_tree_or_bush_type("Tree:Pine"));
        assert!(is_tree_or_bush_type("Tree:Oak"));
        assert!(is_tree_or_bush_type("Tree:Dead"));
        assert!(is_tree_or_bush_type("Tree:Round"));
        assert!(is_tree_or_bush_type("Pine"));
        assert!(is_tree_or_bush_type("Oak"));
        assert!(is_tree_or_bush_type("Dead"));
        assert!(is_tree_or_bush_type("Round"));
        assert!(is_tree_or_bush_type("Bush"));

        // Ground clutter and minerals must NOT be classified as tree/bush
        assert!(!is_tree_or_bush_type("Branch"));
        assert!(!is_tree_or_bush_type("Flint"));
        assert!(!is_tree_or_bush_type("LooseStone"));
        assert!(!is_tree_or_bush_type("Rock"));
    }

    #[test]
    fn test_tiered_culling_distances_and_classification() {
        assert!(is_small_clutter("Branch"));
        assert!(is_small_clutter("Flint"));
        assert!(is_small_clutter("LooseStone"));
        assert!(!is_small_clutter("Bush"));
        assert!(!is_small_clutter("Tree"));
        assert!(!is_small_clutter("Rock"));

        assert!(is_medium_node("Bush"));
        assert!(is_medium_node("FallenLog"));
        assert!(!is_medium_node("Branch"));
        assert!(!is_medium_node("Oak"));

        assert_eq!(SMALL_CLUTTER_LOAD_DIST, 36.0);
        assert_eq!(SMALL_CLUTTER_UNLOAD_DIST, 44.0);
        assert_eq!(MEDIUM_NODE_LOAD_DIST, 64.0);
        assert_eq!(MEDIUM_NODE_UNLOAD_DIST, 72.0);

        let default_load_sq = RESOURCE_NODE_LOAD_RADIUS_SQ;
        assert_eq!(get_node_load_radius_sq("Flint", default_load_sq), SMALL_CLUTTER_LOAD_DIST_SQ);
        assert_eq!(get_node_load_radius_sq("Bush", default_load_sq), MEDIUM_NODE_LOAD_DIST_SQ);
        assert_eq!(get_node_load_radius_sq("Oak", default_load_sq), default_load_sq);
    }

    #[test]
    fn test_tree_shadow_distance_thresholds() {
        assert_eq!(TREE_SHADOW_NEAR_DIST, 56.0);
        assert_eq!(TREE_SHADOW_FAR_DIST, 64.0);
        assert!(TREE_SHADOW_NEAR_DIST < TREE_SHADOW_FAR_DIST);
        assert_eq!(TREE_SHADOW_NEAR_DIST_SQ, 3136.0);
        assert_eq!(TREE_SHADOW_FAR_DIST_SQ, 4096.0);
    }

    #[test]
    fn test_resource_node_load_and_unload_radius() {
        assert_eq!(RESOURCE_NODE_LOAD_RADIUS, 128.0);
        assert_eq!(RESOURCE_NODE_UNLOAD_RADIUS, 144.0);
        assert!(RESOURCE_NODE_LOAD_RADIUS < RESOURCE_NODE_UNLOAD_RADIUS);
        assert_eq!(RESOURCE_NODE_LOAD_RADIUS_SQ, 16384.0);
        assert_eq!(RESOURCE_NODE_UNLOAD_RADIUS_SQ, 20736.0);
    }
}
