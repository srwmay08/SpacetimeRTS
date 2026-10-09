// ----------------------------------------------------------------------------
// CLIENT RAPIER3D PHYSICS MODULE
// ----------------------------------------------------------------------------
// Pure Rapier3D (v0.22) & Parry3D (v0.17) integration mirroring backend/spacetimedb/src/physics.rs.
// Completely replaces Avian3D with Rapier QueryPipeline, ColliderSet, swept-shape CCD,
// and identical server-side collision geometry.

use std::collections::HashMap;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use rapier3d::prelude as rapier;
use rapier3d::prelude::*;
use rapier3d::parry::query::ShapeCastOptions;
use crate::core::GameLayer;

// ----------------------------------------------------------------------------
// 1. COMPONENTS & COLLISION TYPES
// ----------------------------------------------------------------------------

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RigidBody {
    #[default]
    Dynamic,
    Kinematic,
    Fixed,
    Static,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Default, Deref, DerefMut)]
pub struct LinearVelocity(pub Vec3);

impl LinearVelocity {
    pub const ZERO: Self = Self(Vec3::ZERO);
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Default, Deref, DerefMut)]
pub struct GravityScale(pub f32);

#[derive(Component, Clone, Copy, Debug, PartialEq, Default, Deref, DerefMut)]
pub struct PhysicsPosition(pub Vec3);

#[derive(Component, Clone, Copy, Debug, PartialEq, Default, Deref, DerefMut)]
pub struct PhysicsRotation(pub Quat);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LockedAxes(pub u8);

impl LockedAxes {
    pub const ROTATION_LOCKED: Self = Self(1);
    pub const ALL_LOCKED: Self = Self(2);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CoefficientCombine {
    #[default]
    Average,
    Min,
    Multiply,
    Max,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Default)]
pub struct Friction {
    pub dynamic_coefficient: f32,
    pub static_coefficient: f32,
    pub combine_rule: CoefficientCombine,
}

impl Friction {
    pub fn new(f: f32) -> Self {
        Self {
            dynamic_coefficient: f,
            static_coefficient: f,
            combine_rule: CoefficientCombine::Average,
        }
    }

    pub fn with_combine_rule(mut self, rule: CoefficientCombine) -> Self {
        self.combine_rule = rule;
        self
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Default)]
pub struct ColliderDensity(pub f32);

#[derive(Component, Clone, Copy, Debug, PartialEq, Default)]
pub struct ExternalForce {
    pub force: Vec3,
    pub persistent: bool,
}

impl ExternalForce {
    pub fn with_persistence(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SweptCcd;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Sensor;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionLayers {
    pub memberships: u32,
    pub filters: u32,
}

impl Default for CollisionLayers {
    fn default() -> Self {
        Self {
            memberships: GameLayer::Default.bit(),
            filters: !0,
        }
    }
}

impl CollisionLayers {
    pub const NONE: Self = Self { memberships: 0, filters: 0 };
    pub const ALL: Self = Self { memberships: !0, filters: !0 };

    pub fn new<M, F>(memberships: M, filters: F) -> Self
    where
        M: IntoIterator<Item = GameLayer>,
        F: IntoIterator<Item = GameLayer>,
    {
        let mut m_bits = 0u32;
        for m in memberships {
            m_bits |= m.bit();
        }
        let mut f_bits = 0u32;
        for f in filters {
            f_bits |= f.bit();
        }
        Self {
            memberships: m_bits,
            filters: f_bits,
        }
    }

    pub fn contains_layer(&self, layer: GameLayer) -> bool {
        (self.memberships & layer.bit()) != 0
    }

    pub fn allows_layer(&self, layer: GameLayer) -> bool {
        (self.filters & layer.bit()) != 0
    }

    pub fn interacts_with(&self, other: &Self) -> bool {
        (self.memberships & other.filters) != 0 && (other.memberships & self.filters) != 0
    }
}

// ----------------------------------------------------------------------------
// 2. RAPIER COLLIDER COMPONENT
// ----------------------------------------------------------------------------

#[derive(Component, Clone)]
pub struct Collider {
    pub shape: SharedShape,
}

impl std::fmt::Debug for Collider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Collider({:?})", self.shape.shape_type())
    }
}

pub fn quat_to_rapier(q: Quat) -> rapier3d::na::UnitQuaternion<f32> {
    rapier3d::na::UnitQuaternion::new_normalize(rapier3d::na::Quaternion::new(q.w, q.x, q.y, q.z))
}

impl Collider {
    /// Full-extent cuboid (e.g., width x, height y, depth z in meters).
    pub fn cuboid(x: f32, y: f32, z: f32) -> Self {
        Self {
            shape: SharedShape::cuboid(x * 0.5, y * 0.5, z * 0.5),
        }
    }

    /// Half-extent cuboid matching server Rapier3D convention directly.
    pub fn half_cuboid(hx: f32, hy: f32, hz: f32) -> Self {
        Self {
            shape: SharedShape::cuboid(hx, hy, hz),
        }
    }

    /// Vertical Y-aligned capsule with hemispherical caps.
    /// `radius` is the cap radius; `cylinder_height` is the central cylindrical segment length.
    /// Total capsule height = `cylinder_height + 2.0 * radius`.
    pub fn capsule(radius: f32, cylinder_height: f32) -> Self {
        Self {
            shape: SharedShape::capsule_y(cylinder_height * 0.5, radius),
        }
    }

    /// Vertical Y-aligned flat-capped cylinder.
    pub fn cylinder(radius: f32, height: f32) -> Self {
        Self {
            shape: SharedShape::cylinder(height * 0.5, radius),
        }
    }

    /// Sphere shape.
    pub fn sphere(radius: f32) -> Self {
        Self {
            shape: SharedShape::ball(radius),
        }
    }

    /// Compound collider formed by offset sub-shapes with relative transforms.
    pub fn compound(shapes: Vec<(Vec3, Quat, Collider)>) -> Self {
        let compounds: Vec<(Isometry<f32>, SharedShape)> = shapes
            .into_iter()
            .map(|(pos, rot, col)| {
                let iso = Isometry::from_parts(
                    Translation::new(pos.x, pos.y, pos.z),
                    quat_to_rapier(rot),
                );
                (iso, col.shape)
            })
            .collect();

        Self {
            shape: SharedShape::compound(compounds),
        }
    }

    /// Generates a trimesh collider directly from a Bevy 3D Mesh asset.
    pub fn trimesh_from_mesh(mesh: &Mesh) -> Option<Self> {
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?;
        let vertices: Vec<Point<f32>> = positions
            .iter()
            .map(|&[x, y, z]| Point::new(x, y, z))
            .collect();

        let indices = mesh.indices()?;
        let mut tri_indices: Vec<[u32; 3]> = Vec::new();
        match indices {
            bevy::render::mesh::Indices::U16(idx) => {
                for chunk in idx.chunks_exact(3) {
                    tri_indices.push([chunk[0] as u32, chunk[1] as u32, chunk[2] as u32]);
                }
            }
            bevy::render::mesh::Indices::U32(idx) => {
                for chunk in idx.chunks_exact(3) {
                    tri_indices.push([chunk[0], chunk[1], chunk[2]]);
                }
            }
        }

        if vertices.is_empty() || tri_indices.is_empty() {
            return None;
        }

        Some(Self {
            shape: SharedShape::trimesh(vertices, tri_indices),
        })
    }

    /// Computes the world AABB for spatial bounds testing.
    pub fn aabb(&self, translation: Vec3, rotation: Quat) -> bevy::math::bounding::Aabb3d {
        let iso = Isometry::from_parts(
            Translation::new(translation.x, translation.y, translation.z),
            quat_to_rapier(rotation),
        );
        let aabb = self.shape.compute_aabb(&iso);
        bevy::math::bounding::Aabb3d {
            min: bevy::math::Vec3A::new(aabb.mins.x, aabb.mins.y, aabb.mins.z),
            max: bevy::math::Vec3A::new(aabb.maxs.x, aabb.maxs.y, aabb.maxs.z),
        }
    }
}

// ----------------------------------------------------------------------------
// 3. SPATIAL QUERY FILTER & HIT DATA
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHitData {
    pub entity: Entity,
    pub time_of_impact: f32,
    pub normal: Vec3,
    pub point: Vec3,
}

#[derive(Clone, Debug, Default)]
pub struct SpatialQueryFilter {
    pub mask: u32,
    pub excluded_entities: Vec<Entity>,
}

impl SpatialQueryFilter {
    pub fn from_mask<I>(layers: I) -> Self
    where
        I: IntoIterator<Item = GameLayer>,
    {
        let mut mask = 0u32;
        for l in layers {
            mask |= l.bit();
        }
        Self {
            mask,
            excluded_entities: Vec::new(),
        }
    }

    pub fn from_excluded_entities<I>(entities: I) -> Self
    where
        I: IntoIterator<Item = Entity>,
    {
        Self {
            mask: !0,
            excluded_entities: entities.into_iter().collect(),
        }
    }

    pub fn with_excluded_entities<I>(mut self, entities: I) -> Self
    where
        I: IntoIterator<Item = Entity>,
    {
        self.excluded_entities.extend(entities);
        self
    }

    pub fn with_mask<I>(mut self, layers: I) -> Self
    where
        I: IntoIterator<Item = GameLayer>,
    {
        self.mask = 0;
        for l in layers {
            self.mask |= l.bit();
        }
        self
    }
}

// ----------------------------------------------------------------------------
// 4. CLIENT RAPIER PHYSICS WORLD RESOURCE
// ----------------------------------------------------------------------------

#[derive(Resource)]
pub struct ClientRapierWorld {
    pub colliders: ColliderSet,
    pub query_pipeline: QueryPipeline,
    pub entity_to_handle: HashMap<Entity, ColliderHandle>,
    pub handle_to_entity: HashMap<ColliderHandle, Entity>,
    pub entity_layers: HashMap<Entity, CollisionLayers>,
}

impl Default for ClientRapierWorld {
    fn default() -> Self {
        Self {
            colliders: ColliderSet::new(),
            query_pipeline: QueryPipeline::new(),
            entity_to_handle: HashMap::new(),
            handle_to_entity: HashMap::new(),
            entity_layers: HashMap::new(),
        }
    }
}

impl ClientRapierWorld {
    pub fn cast_ray(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_dist: f32,
        _solid: bool,
        filter: &SpatialQueryFilter,
    ) -> Option<RayHitData> {
        let dir_len_sq = dir.length_squared();
        if dir_len_sq < 1e-6 {
            return None;
        }
        let norm_dir = dir.normalize();

        let ray = Ray::new(
            Point::new(origin.x, origin.y, origin.z),
            Vector::new(norm_dir.x, norm_dir.y, norm_dir.z),
        );

        let predicate = |handle: ColliderHandle, _collider: &rapier::Collider| {
            if let Some(&entity) = self.handle_to_entity.get(&handle) {
                if filter.excluded_entities.contains(&entity) {
                    return false;
                }
                if let Some(layers) = self.entity_layers.get(&entity) {
                    if (layers.memberships & filter.mask) == 0 {
                        return false;
                    }
                }
            }
            true
        };

        let q_filter = QueryFilter::default().predicate(&predicate);
        let rigid_bodies = RigidBodySet::new();

        if let Some((handle, intersection)) = self.query_pipeline.cast_ray_and_get_normal(
            &rigid_bodies,
            &self.colliders,
            &ray,
            max_dist,
            true,
            q_filter,
        ) {
            let toi = intersection.time_of_impact;
            let normal = Vec3::new(
                intersection.normal.x,
                intersection.normal.y,
                intersection.normal.z,
            );
            let hit_pt = origin + norm_dir * toi;
            let entity = self.handle_to_entity.get(&handle).copied().unwrap_or(Entity::PLACEHOLDER);

            return Some(RayHitData {
                entity,
                time_of_impact: toi,
                normal,
                point: hit_pt,
            });
        }

        None
    }

    pub fn cast_swept_sphere(
        &self,
        origin: Vec3,
        dir: Vec3,
        radius: f32,
        max_dist: f32,
        filter: &SpatialQueryFilter,
    ) -> Option<RayHitData> {
        let dir_len_sq = dir.length_squared();
        if dir_len_sq < 1e-6 {
            return None;
        }
        let norm_dir = dir.normalize();

        let shape = Ball::new(radius);
        let shape_pos = Isometry::translation(origin.x, origin.y, origin.z);
        let shape_vel = Vector::new(norm_dir.x, norm_dir.y, norm_dir.z);

        let predicate = |handle: ColliderHandle, _collider: &rapier::Collider| {
            if let Some(&entity) = self.handle_to_entity.get(&handle) {
                if filter.excluded_entities.contains(&entity) {
                    return false;
                }
                if let Some(layers) = self.entity_layers.get(&entity) {
                    if (layers.memberships & filter.mask) == 0 {
                        return false;
                    }
                }
            }
            true
        };

        let q_filter = QueryFilter::default().predicate(&predicate);
        let rigid_bodies = RigidBodySet::new();
        let options = ShapeCastOptions {
            max_time_of_impact: max_dist,
            stop_at_penetration: true,
            target_distance: 0.0,
            compute_impact_geometry_on_penetration: true,
        };

        if let Some((handle, hit)) = self.query_pipeline.cast_shape(
            &rigid_bodies,
            &self.colliders,
            &shape_pos,
            &shape_vel,
            &shape,
            options,
            q_filter,
        ) {
            let toi = hit.time_of_impact;
            let hit_pt = origin + norm_dir * toi;
            let normal = Vec3::new(hit.normal1.x, hit.normal1.y, hit.normal1.z);
            let entity = self.handle_to_entity.get(&handle).copied().unwrap_or(Entity::PLACEHOLDER);

            return Some(RayHitData {
                entity,
                time_of_impact: toi,
                normal,
                point: hit_pt,
            });
        }

        None
    }
}

// ----------------------------------------------------------------------------
// 5. SPATIAL QUERY SYSTEM PARAM (DROP-IN COMPATIBLE WITH SYSTEM CODE)
// ----------------------------------------------------------------------------

#[derive(SystemParam)]
pub struct SpatialQuery<'w> {
    pub world: Res<'w, ClientRapierWorld>,
}

impl<'w> SpatialQuery<'w> {
    pub fn cast_ray(
        &self,
        origin: Vec3,
        dir: impl Into<Vec3>,
        max_dist: f32,
        solid: bool,
        filter: SpatialQueryFilter,
    ) -> Option<RayHitData> {
        self.world.cast_ray(origin, dir.into(), max_dist, solid, &filter)
    }

    pub fn cast_swept_sphere(
        &self,
        origin: Vec3,
        dir: impl Into<Vec3>,
        radius: f32,
        max_dist: f32,
        filter: SpatialQueryFilter,
    ) -> Option<RayHitData> {
        self.world.cast_swept_sphere(origin, dir.into(), radius, max_dist, &filter)
    }
}

// ----------------------------------------------------------------------------
// 6. RAPIER PHYSICS ENGINE ECS SYNCHRONIZATION SYSTEMS
// ----------------------------------------------------------------------------

pub fn sync_rapier_colliders_system(
    mut world: ResMut<ClientRapierWorld>,
    added_colliders_q: Query<(Entity, &Collider, &GlobalTransform, Option<&CollisionLayers>), Added<Collider>>,
    mut removed_colliders: RemovedComponents<Collider>,
    mut moved_colliders_q: Query<(Entity, &GlobalTransform, Option<&CollisionLayers>), (With<Collider>, Changed<GlobalTransform>)>,
) {
    let mut dirty = false;

    // 1. Despawn removed colliders from Rapier ColliderSet
    let mut islands = IslandManager::new();
    let mut bodies = RigidBodySet::new();
    for removed_entity in removed_colliders.read() {
        if let Some(handle) = world.entity_to_handle.remove(&removed_entity) {
            world.colliders.remove(handle, &mut islands, &mut bodies, true);
            world.handle_to_entity.remove(&handle);
            world.entity_layers.remove(&removed_entity);
            dirty = true;
        }
    }

    // 2. Insert newly spawned colliders into Rapier ColliderSet
    for (entity, collider, gt, layers_opt) in added_colliders_q.iter() {
        let (_scale, rot, trans) = gt.to_scale_rotation_translation();
        let iso = Isometry::from_parts(
            Translation::new(trans.x, trans.y, trans.z),
            quat_to_rapier(rot),
        );

        let rapier_col = rapier::ColliderBuilder::new(collider.shape.clone())
            .position(iso)
            .user_data(entity.to_bits() as u128)
            .build();

        let handle = world.colliders.insert(rapier_col);
        world.entity_to_handle.insert(entity, handle);
        world.handle_to_entity.insert(handle, entity);

        if let Some(layers) = layers_opt {
            world.entity_layers.insert(entity, *layers);
        }
        dirty = true;
    }

    // 3. Update transformed dynamic / kinematic entities
    for (entity, gt, layers_opt) in moved_colliders_q.iter_mut() {
        if let Some(&handle) = world.entity_to_handle.get(&entity) {
            let (_scale, rot, trans) = gt.to_scale_rotation_translation();
            let iso = Isometry::from_parts(
                Translation::new(trans.x, trans.y, trans.z),
                quat_to_rapier(rot),
            );

            if let Some(col) = world.colliders.get_mut(handle) {
                if col.position() != &iso {
                    col.set_position(iso);
                    dirty = true;
                }
            }

            if let Some(layers) = layers_opt {
                world.entity_layers.insert(entity, *layers);
            }
        }
    }

    // 4. Incremental BVH update / refit (only when dirty!)
    if dirty {
        let ClientRapierWorld {
            ref colliders,
            ref mut query_pipeline,
            ..
        } = *world;
        query_pipeline.update(colliders);
    }
}

/// Integrates velocities, external forces, and gravity scales for dynamic bodies.
pub fn step_rapier_dynamics_system(
    time: Res<Time>,
    mut dynamic_q: Query<(
        &RigidBody,
        &mut Transform,
        Option<&mut PhysicsPosition>,
        Option<&mut PhysicsRotation>,
        &mut LinearVelocity,
        Option<&GravityScale>,
        Option<&ExternalForce>,
    )>,
) {
    let dt = time.delta_seconds();
    if dt <= 0.0 {
        return;
    }

    for (rb, mut transform, mut phys_pos, mut phys_rot, mut lin_vel, grav_opt, ext_force) in dynamic_q.iter_mut() {
        if *rb != RigidBody::Dynamic {
            continue;
        }

        // Apply external forces
        if let Some(ext) = ext_force {
            lin_vel.0 += ext.force * dt;
        }

        // Only integrate if actively moving to avoid continuous micro-jitter & dirtying
        if lin_vel.0.length_squared() > 1e-6 {
            // Apply gravity
            let grav_scale = grav_opt.map_or(1.0, |g| g.0);
            if grav_scale > 0.0 {
                lin_vel.0.y -= 9.81 * grav_scale * dt;
            }

            transform.translation += lin_vel.0 * dt;

            // Keep legacy Position and Rotation components synchronized
            if let Some(ref mut p) = phys_pos {
                p.0 = transform.translation;
            }
            if let Some(ref mut r) = phys_rot {
                r.0 = transform.rotation;
            }
        }
    }
}

// ----------------------------------------------------------------------------
// 7. CLIENT RAPIER PHYSICS PLUGIN REGISTRATION
// ----------------------------------------------------------------------------

pub struct ClientRapierPhysicsPlugin;

impl Plugin for ClientRapierPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientRapierWorld>()
            .add_systems(
                Update,
                (
                    sync_rapier_colliders_system,
                    step_rapier_dynamics_system,
                )
                    .chain()
                    .in_set(crate::core::UpdateSet::Physics),
            );
    }
}
