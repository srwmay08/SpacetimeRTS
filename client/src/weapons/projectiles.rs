// ============================================================================
// File: client/src/weapons/projectiles.rs
// ============================================================================
// ----------------------------------------------------------------------------
// AUTHORITATIVE PROJECTILE RENDERING & BALLISTIC ORIENTATION
// ----------------------------------------------------------------------------

use std::collections::{BTreeMap, BTreeSet};
use bevy::prelude::{Transform as BevyTransform, *};
use spacetimedb_sdk::Table;

use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::physics::LinearVelocity;
use crate::module_bindings::active_projectile_table::ActiveProjectileTableAccess;
use crate::module_bindings::projectile_kind_type::ProjectileKind;
use super::mesh_builder::create_lowpoly_arrow_mesh;

pub fn sync_active_projectiles(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut existing_projectiles: Query<(Entity, &NetworkProjectile, &mut BevyTransform)>,
) {
    let db_projectiles: Vec<_> = conn.db.db.active_projectile().iter().collect();
    let mut current_ids = BTreeSet::new();

    let mut entity_map = BTreeMap::new();
    for (entity, net_proj, transform) in existing_projectiles.iter_mut() {
        entity_map.insert(net_proj.0, (entity, transform));
    }

    for p in &db_projectiles {
        current_ids.insert(p.projectile_id);
        let vel = Vec3::new(p.vel_x, p.vel_y, p.vel_z);
        let pos = Vec3::new(p.pos_x, p.pos_y, p.pos_z);
        let rot = if vel.length_squared() > 0.001 {
            BevyTransform::from_xyz(pos.x, pos.y, pos.z).looking_to(vel.normalize(), Vec3::Y).rotation
        } else {
            Quat::IDENTITY
        };

        if let Some((_, ref mut transform)) = entity_map.get_mut(&p.projectile_id) {
            transform.translation = pos;
            transform.rotation = rot;
        } else {
            spawn_projectile_entity(&mut commands, &mut meshes, &mut materials, p.projectile_id, p.kind, pos, rot);
        }
    }

    for (entity, net_proj, _) in existing_projectiles.iter() {
        if !current_ids.contains(&net_proj.0) {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn spawn_projectile_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    projectile_id: u64,
    kind: ProjectileKind,
    pos: Vec3,
    rot: Quat,
) {
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.38, 0.22),
        perceptual_roughness: 0.8,
        ..default()
    });
    let iron_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.36, 0.38),
        metallic: 0.8,
        perceptual_roughness: 0.35,
        ..default()
    });
    let stone_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.48, 0.46, 0.44),
        perceptual_roughness: 0.9,
        ..default()
    });
    let brass_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.75, 0.25),
        metallic: 0.85,
        perceptual_roughness: 0.3,
        ..default()
    });
    let arcane_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.8, 1.0),
        unlit: true,
        ..default()
    });
    let fire_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.35, 0.05),
        unlit: true,
        ..default()
    });
    let red_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.15, 0.15),
        ..default()
    });

    let mut parent = commands.spawn((
        SpatialBundle {
            transform: BevyTransform::from_translation(pos).with_rotation(rot),
            ..default()
        },
        NetworkProjectile(projectile_id),
    ));

    parent.with_children(|builder| {
        match kind {
            ProjectileKind::Arrow => {
                let arrow_mesh = meshes.add(create_lowpoly_arrow_mesh());
                let arrow_mat = materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.65,
                    metallic: 0.35,
                    cull_mode: None,
                    ..default()
                });
                builder.spawn(PbrBundle {
                    mesh: arrow_mesh,
                    material: arrow_mat,
                    ..default()
                });
            }
            ProjectileKind::HandCrossbowBolt => {
                // Compact bolt body
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.018, 0.018, 0.26)),
                    material: iron_mat.clone(),
                    ..default()
                });
                // Piercing steel point
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.025, 0.025, 0.04)),
                    material: brass_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, -0.14),
                    ..default()
                });
            }
            ProjectileKind::RevolverBullet => {
                // Brass bullet body & copper tip
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cylinder::new(0.015, 0.06)),
                    material: brass_mat.clone(),
                    transform: BevyTransform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ..default()
                });
            }
            ProjectileKind::ShotgunPellet => {
                // Concentrated lead buckshot pellet
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.02)),
                    material: iron_mat.clone(),
                    ..default()
                });
            }
            ProjectileKind::SniperBullet => {
                // High velocity tungsten penetrator
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cylinder::new(0.014, 0.12)),
                    material: iron_mat.clone(),
                    transform: BevyTransform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ..default()
                });
            }
            ProjectileKind::MagicMissile => {
                // Glowing arcane crystal dart
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.04, 0.04, 0.16)),
                    material: arcane_mat.clone(),
                    ..default()
                });
                // Pulsing energy halo
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.06)),
                    material: arcane_mat.clone(),
                    ..default()
                });
            }
            ProjectileKind::FireballBall => {
                // Blazing outer fireball
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.22)),
                    material: fire_mat.clone(),
                    ..default()
                });
                // Inner core
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.12)),
                    material: brass_mat.clone(),
                    ..default()
                });
            }
            ProjectileKind::CatapultRock => {
                // Rough-hewn chiseled stone boulder
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.55, 0.48, 0.52)),
                    material: stone_mat.clone(),
                    ..default()
                });
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.38, 0.38, 0.38)),
                    material: stone_mat.clone(),
                    transform: BevyTransform::from_xyz(0.1, -0.05, 0.1),
                    ..default()
                });
            }
            ProjectileKind::TrebuchetShell => {
                // Giant fortified iron-banded stone shell
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Sphere::new(0.38)),
                    material: stone_mat.clone(),
                    ..default()
                });
                // Reinforcing iron hoops
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.78, 0.06, 0.78)),
                    material: iron_mat.clone(),
                    ..default()
                });
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.06, 0.78, 0.78)),
                    material: iron_mat.clone(),
                    ..default()
                });
            }
            ProjectileKind::BallistaSpear => {
                // Massive siege harpoon
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.045, 0.045, 1.4)),
                    material: wood_mat.clone(),
                    ..default()
                });
                // Barbed iron spearhead
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.08, 0.025, 0.22)),
                    material: iron_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, -0.75),
                    ..default()
                });
                // Crossbar stabilizer fins
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.18, 0.015, 0.12)),
                    material: red_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, 0.55),
                    ..default()
                });
            }
        }
    });
}

/// Smoothly reorients in-flight arrow projectiles along their velocity vector,
/// creating authentic aerodynamic ballistic arc curvature as gravity pulls the arrow.
pub fn update_arrow_projectiles(
    mut query: Query<(&mut BevyTransform, &LinearVelocity), With<ArrowProjectile>>,
) {
    for (mut transform, velocity) in query.iter_mut() {
        if velocity.0.length_squared() > 1.0 {
            transform.look_to(velocity.0.normalize(), Vec3::Y);
        }
    }
}
