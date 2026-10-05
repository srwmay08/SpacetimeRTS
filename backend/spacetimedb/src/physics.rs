use spacetimedb::{ReducerContext, Table};
use std::sync::RwLock;
use rapier3d::prelude::*;
use crate::movement::transform;
use crate::building::structure;
use crate::ai::{harvestable_corpse, npc_brain};
use crate::combat::health;

/// Global cache for our purely stateless QueryPipeline
static PHYSICS_CACHE: RwLock<Option<(ColliderSet, QueryPipeline)>> = RwLock::new(None);

/// Called at the start of `high_frequency_tick` to reconstruct the world state.
/// Because this constructs the physics tree deterministically entirely from the DB,
/// it is completely rollback-safe.
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

/// Helper function to perform a raycast against the cached pipeline.
pub fn cast_ray(
    origin_x: f32, origin_y: f32, origin_z: f32,
    dir_x: f32, dir_y: f32, dir_z: f32,
    max_dist: f32,
    exclude_entity: u64
) -> Option<(u128, f32, f32, f32, f32)> {
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
    if let Some((handle, toi)) = query_pipeline.cast_ray(
        &rigid_bodies,
        colliders,
        &ray,
        max_dist,
        true,
        filter
    ) {
        let hit_point = ray.point_at(toi);
        let collider = &colliders[handle];
        let user_data = collider.user_data;
        
        return Some((user_data, hit_point.x, hit_point.y, hit_point.z, toi));
    }
    
    None
}
