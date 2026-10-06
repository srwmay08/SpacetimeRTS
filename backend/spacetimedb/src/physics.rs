// ----------------------------------------------------------------------------
// PHYSICS & COLLISION SIMULATION ENGINE (Rapier3D & Parry3D)
// ----------------------------------------------------------------------------
// Architectural Note: Provides server-authoritative geometric queries, continuous
// collision detection (Adaptive CCD), swept-volume melee validation, and dynamic
// capsule separation. The query pipeline is deterministically reconstructed
// from SpacetimeDB table snapshots, ensuring 100% rollback-safety across nodes.

use spacetimedb::{ReducerContext, Table};
use std::sync::RwLock;
use rapier3d::prelude::*;
use rapier3d::parry::query::ShapeCastOptions;
use crate::movement::transform;
use crate::building::structure;
use crate::ai::{harvestable_corpse, npc_brain};
use crate::combat::health;

/// Standardized physics hit outcome for rays, swept spheres, and swept capsules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitResult {
    pub entity_id: u64,
    pub is_structure: bool,
    pub point: (f32, f32, f32),
    pub normal: (f32, f32, f32),
    pub time_of_impact: f32,
    pub user_data: u128,
}

/// Global cache for our purely stateless QueryPipeline
static PHYSICS_CACHE: RwLock<Option<(ColliderSet, QueryPipeline)>> = RwLock::new(None);

/// Called at the start of `high_frequency_tick` to reconstruct the world state.
/// Because this constructs the physics tree deterministically entirely from the DB,
/// it is completely rollback-safe and thread-safe.
pub fn rebuild_physics_cache(ctx: &ReducerContext) {
    let mut colliders = ColliderSet::new();

    // 1. Add all dynamic entities (Players, NPCs) using Bestiary archetypes
    for t in ctx.db.transform().iter() {
        if ctx.db.harvestable_corpse().entity_id().find(t.entity_id).is_some() { continue; }
        if ctx.db.health().entity_id().find(t.entity_id).is_none() { continue; }

        let collider = if let Some(brain) = ctx.db.npc_brain().entity_id().find(t.entity_id) {
            let arch = crate::bestiary::get_archetype_by_ai_type(brain.ai_type);
            match arch.collider_shape {
                crate::bestiary::ColliderShape::Cuboid => {
                    let (hx, hy, hz) = arch.collider_half_extents;
                    ColliderBuilder::cuboid(hx, hy, hz)
                        .translation(Vector::new(t.x, t.y + arch.vertical_offset, t.z))
                        .user_data(t.entity_id as u128)
                        .build()
                }
                crate::bestiary::ColliderShape::Capsule => {
                    let (rad, hh, _) = arch.collider_half_extents;
                    ColliderBuilder::capsule_y(hh, rad)
                        .translation(Vector::new(t.x, t.y + arch.vertical_offset, t.z))
                        .user_data(t.entity_id as u128)
                        .build()
                }
            }
        } else {
            // Humanoid shape: 1.8m height total (half-height 0.5, radius 0.4)
            ColliderBuilder::capsule_y(0.5, 0.4)
                .translation(Vector::new(t.x, t.y - 0.15, t.z)) 
                .user_data(t.entity_id as u128) 
                .build()
        };
        colliders.insert(collider);
    }

    // 2. Add static modular structures as Cuboids
    for s in ctx.db.structure().iter() {
        // Architectural Note: An open door is a hole in the wall; it must not stop projectiles.
        if !crate::building::structure_blocks_projectiles(ctx, &s) { continue; }
        // Basic 2.5m x 2.5m block representation
        let collider = ColliderBuilder::cuboid(1.25, 1.25, 1.25)
            .translation(Vector::new(s.x, s.y, s.z))
            .user_data((s.structure_id as u128) | (1 << 64)) // High bit indicates Structure
            .build();
        colliders.insert(collider);
    }

    // 3. Update the Broad-Phase / BVH tree
    let mut query_pipeline = QueryPipeline::new();
    query_pipeline.update(&colliders);

    *PHYSICS_CACHE.write().unwrap() = Some((colliders, query_pipeline));
}

/// Helper function to perform a raycast against the cached pipeline, returning a HitResult.
pub fn cast_ray_hit(
    origin_x: f32, origin_y: f32, origin_z: f32,
    dir_x: f32, dir_y: f32, dir_z: f32,
    max_dist: f32,
    exclude_entity: u64
) -> Option<HitResult> {
    let cache_guard = PHYSICS_CACHE.read().unwrap();
    let cache = cache_guard.as_ref()?;

    let colliders = &cache.0;
    let query_pipeline = &cache.1;

    let ray = Ray::new(Point::new(origin_x, origin_y, origin_z), Vector::new(dir_x, dir_y, dir_z));
    
    // Ignore the shooter so bullets don't instantly hit them
    let predicate = |_, collider: &Collider| {
        let id = (collider.user_data & 0xFFFFFFFFFFFFFFFF) as u64;
        id != exclude_entity
    };
    let filter = QueryFilter::default().predicate(&predicate);
    
    let rigid_bodies = RigidBodySet::new();
    if let Some((handle, intersection)) = query_pipeline.cast_ray_and_get_normal(
        &rigid_bodies,
        colliders,
        &ray,
        max_dist,
        true,
        filter
    ) {
        let toi = intersection.time_of_impact;
        let normal = intersection.normal;
        let hit_point = ray.point_at(toi);
        let collider = &colliders[handle];
        let user_data = collider.user_data;
        let is_structure = (user_data >> 64) == 1;
        let entity_id = (user_data & 0xFFFFFFFFFFFFFFFF) as u64;

        return Some(HitResult {
            entity_id,
            is_structure,
            point: (hit_point.x, hit_point.y, hit_point.z),
            normal: (normal.x, normal.y, normal.z),
            time_of_impact: toi,
            user_data,
        });
    }
    
    None
}

/// Backwards-compatible tuple-based raycast helper.
pub fn cast_ray(
    origin_x: f32, origin_y: f32, origin_z: f32,
    dir_x: f32, dir_y: f32, dir_z: f32,
    max_dist: f32,
    exclude_entity: u64
) -> Option<(u128, f32, f32, f32, f32)> {
    cast_ray_hit(origin_x, origin_y, origin_z, dir_x, dir_y, dir_z, max_dist, exclude_entity)
        .map(|hit| (hit.user_data, hit.point.0, hit.point.1, hit.point.2, hit.time_of_impact))
}

/// Swept sphere continuous collision detection (Adaptive CCD) for anti-tunneling ballistics.
pub fn cast_swept_sphere(
    origin_x: f32, origin_y: f32, origin_z: f32,
    dir_x: f32, dir_y: f32, dir_z: f32,
    radius: f32,
    max_dist: f32,
    exclude_entity: u64
) -> Option<HitResult> {
    let cache_guard = PHYSICS_CACHE.read().unwrap();
    let cache = cache_guard.as_ref()?;

    let colliders = &cache.0;
    let query_pipeline = &cache.1;

    let shape = Ball::new(radius);
    let shape_pos = Isometry::translation(origin_x, origin_y, origin_z);
    let shape_vel = Vector::new(dir_x, dir_y, dir_z);

    let predicate = |_, collider: &Collider| {
        let id = (collider.user_data & 0xFFFFFFFFFFFFFFFF) as u64;
        id != exclude_entity
    };
    let filter = QueryFilter::default().predicate(&predicate);

    let rigid_bodies = RigidBodySet::new();
    let options = ShapeCastOptions {
        max_time_of_impact: max_dist,
        stop_at_penetration: true,
        target_distance: 0.0,
        compute_impact_geometry_on_penetration: true,
    };
    if let Some((handle, hit)) = query_pipeline.cast_shape(
        &rigid_bodies,
        colliders,
        &shape_pos,
        &shape_vel,
        &shape,
        options,
        filter,
    ) {
        let hit_point = shape_pos.translation.vector + shape_vel * hit.time_of_impact;
        let collider = &colliders[handle];
        let user_data = collider.user_data;
        let normal = hit.normal1;
        let is_structure = (user_data >> 64) == 1;
        let entity_id = (user_data & 0xFFFFFFFFFFFFFFFF) as u64;

        return Some(HitResult {
            entity_id,
            is_structure,
            point: (hit_point.x, hit_point.y, hit_point.z),
            normal: (normal.x, normal.y, normal.z),
            time_of_impact: hit.time_of_impact,
            user_data,
        });
    }

    None
}

/// Swept capsule continuous collision detection for lunges, rolls, and melee weapon swings.
pub fn cast_swept_capsule(
    origin_x: f32, origin_y: f32, origin_z: f32,
    dir_x: f32, dir_y: f32, dir_z: f32,
    half_height: f32,
    radius: f32,
    max_dist: f32,
    exclude_entity: u64
) -> Option<HitResult> {
    let cache_guard = PHYSICS_CACHE.read().unwrap();
    let cache = cache_guard.as_ref()?;

    let colliders = &cache.0;
    let query_pipeline = &cache.1;

    let shape = Capsule::new_y(half_height, radius);
    let shape_pos = Isometry::translation(origin_x, origin_y, origin_z);
    let shape_vel = Vector::new(dir_x, dir_y, dir_z);

    let predicate = |_, collider: &Collider| {
        let id = (collider.user_data & 0xFFFFFFFFFFFFFFFF) as u64;
        id != exclude_entity
    };
    let filter = QueryFilter::default().predicate(&predicate);

    let rigid_bodies = RigidBodySet::new();
    let options = ShapeCastOptions {
        max_time_of_impact: max_dist,
        stop_at_penetration: true,
        target_distance: 0.0,
        compute_impact_geometry_on_penetration: true,
    };
    if let Some((handle, hit)) = query_pipeline.cast_shape(
        &rigid_bodies,
        colliders,
        &shape_pos,
        &shape_vel,
        &shape,
        options,
        filter,
    ) {
        let hit_point = shape_pos.translation.vector + shape_vel * hit.time_of_impact;
        let collider = &colliders[handle];
        let user_data = collider.user_data;
        let normal = hit.normal1;
        let is_structure = (user_data >> 64) == 1;
        let entity_id = (user_data & 0xFFFFFFFFFFFFFFFF) as u64;

        return Some(HitResult {
            entity_id,
            is_structure,
            point: (hit_point.x, hit_point.y, hit_point.z),
            normal: (normal.x, normal.y, normal.z),
            time_of_impact: hit.time_of_impact,
            user_data,
        });
    }

    None
}

/// Compute capsule-to-capsule contact manifold for dynamic body pushback and separation.
/// Returns Some((contact_normal, signed_distance)). If signed_distance < 0.0, shapes penetrate.
pub fn compute_capsule_contact(
    pos1: (f32, f32, f32),
    half_height1: f32,
    radius1: f32,
    pos2: (f32, f32, f32),
    half_height2: f32,
    radius2: f32,
    prediction_dist: f32,
) -> Option<((f32, f32, f32), f32)> {
    let p1 = Isometry::translation(pos1.0, pos1.1, pos1.2);
    let p2 = Isometry::translation(pos2.0, pos2.1, pos2.2);
    let c1 = Capsule::new_y(half_height1, radius1);
    let c2 = Capsule::new_y(half_height2, radius2);

    if let Ok(Some(contact)) = rapier3d::parry::query::contact(&p1, &c1, &p2, &c2, prediction_dist) {
        let n = contact.normal1;
        let dist = contact.dist;
        return Some(((n.x, n.y, n.z), dist));
    }
    None
}

/// Swept blade volume test against a target capsule using Parry's shape-cast solver.
pub fn check_blade_sweep_toi(
    blade_pos: (f32, f32, f32),
    blade_vel: (f32, f32, f32),
    blade_half_height: f32,
    blade_radius: f32,
    target_pos: (f32, f32, f32),
    target_half_height: f32,
    target_radius: f32,
    max_toi: f32,
) -> Option<f32> {
    let p1 = Isometry::translation(blade_pos.0, blade_pos.1, blade_pos.2);
    let v1 = Vector::new(blade_vel.0, blade_vel.1, blade_vel.2);
    let c1 = Capsule::new_y(blade_half_height, blade_radius);

    let p2 = Isometry::translation(target_pos.0, target_pos.1, target_pos.2);
    let v2 = Vector::zeros();
    let c2 = Capsule::new_y(target_half_height, target_radius);

    if let Ok(Some(toi)) = rapier3d::parry::query::cast_shapes(
        &p1, &v1, &c1,
        &p2, &v2, &c2,
        ShapeCastOptions {
            max_time_of_impact: max_toi,
            stop_at_penetration: true,
            target_distance: 0.0,
            compute_impact_geometry_on_penetration: true,
        },
    ) {
        return Some(toi.time_of_impact);
    }
    None
}

/// Find all entity IDs overlapping with a spherical volume.
pub fn test_shape_intersections(
    origin_x: f32, origin_y: f32, origin_z: f32,
    radius: f32,
    exclude_entity: u64
) -> Vec<u64> {
    let cache_guard = PHYSICS_CACHE.read().unwrap();
    let Some(cache) = cache_guard.as_ref() else { return Vec::new(); };

    let colliders = &cache.0;
    let query_pipeline = &cache.1;

    let shape = Ball::new(radius);
    let shape_pos = Isometry::translation(origin_x, origin_y, origin_z);

    let predicate = |_, collider: &Collider| {
        let id = (collider.user_data & 0xFFFFFFFFFFFFFFFF) as u64;
        id != exclude_entity
    };
    let filter = QueryFilter::default().predicate(&predicate);

    let rigid_bodies = RigidBodySet::new();
    let mut hits = Vec::new();
    query_pipeline.intersections_with_shape(
        &rigid_bodies,
        colliders,
        &shape_pos,
        &shape,
        filter,
        |handle| {
            let col = &colliders[handle];
            hits.push((col.user_data & 0xFFFFFFFFFFFFFFFF) as u64);
            true
        }
    );

    hits
}
