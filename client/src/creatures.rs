// ============================================================================
// File: client/src/creatures.rs
// ============================================================================
// ----------------------------------------------------------------------------
// PROCEDURAL CREATURE & NPC MICRO-VOXEL MESHES
// ----------------------------------------------------------------------------
// Architectural Note:
// Generates 3D models for fauna and humanoid NPCs:
// Deer, wild boars, goblin raiders, peasants, and player pets.
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use crate::voxel_mesh::MicroVoxelGrid;

pub fn create_voxel_deer_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.022);
    let tawny = [0.84, 0.62, 0.42, 1.0];
    let shadow = [0.65, 0.45, 0.30, 1.0];
    let white = [0.96, 0.94, 0.90, 1.0];
    let cream = [0.92, 0.86, 0.78, 1.0];
    let black = [0.12, 0.12, 0.14, 1.0];
    let antler = [0.22, 0.14, 0.10, 1.0];

    // Slender cloven hooves
    grid.fill_box([-7, 0, 9], [-4, 3, 13], black);
    grid.fill_box([4, 0, 9], [7, 3, 13], black);
    grid.fill_box([-7, 0, -13], [-4, 3, -9], black);
    grid.fill_box([4, 0, -13], [7, 3, -9], black);

    // Multi-jointed slender legs
    grid.fill_box([-7, 3, 9], [-5, 28, 12], shadow);
    grid.fill_box([5, 3, 9], [7, 28, 12], shadow);
    grid.fill_box([-7, 3, -13], [-5, 28, -10], shadow);
    grid.fill_box([5, 3, -13], [7, 28, -10], shadow);

    // Hock joints & knees
    grid.fill_box([-8, 18, 9], [-4, 22, 13], tawny);
    grid.fill_box([4, 18, 9], [8, 22, 13], tawny);
    grid.fill_box([-8, 20, -14], [-4, 24, -9], shadow);
    grid.fill_box([4, 20, -14], [8, 24, -9], shadow);

    // Contoured muscular haunches
    grid.fill_box([-9, 28, 7], [-3, 44, 15], tawny);
    grid.fill_box([3, 28, 7], [9, 44, 15], tawny);
    grid.fill_box([-9, 28, -15], [-3, 46, -7], shadow);
    grid.fill_box([3, 28, -15], [9, 46, -7], shadow);

    // Sculpted torso with narrow waist
    grid.fill_box([-8, 34, -16], [8, 52, 14], tawny);
    grid.fill_box([-7, 32, -14], [7, 39, 12], cream);

    // Dappled spots along flank
    let spots = [
        (-9, 46, -12), (-9, 48, -6), (-9, 44, 0), (-9, 49, 6),
        (9, 46, -12), (9, 48, -6), (9, 44, 0), (9, 49, 6),
    ];
    for (sx, sy, sz) in spots {
        grid.fill_box([sx, sy, sz], [sx, sy + 1, sz + 1], white);
    }

    // White chest bib and tail
    grid.fill_box([-6, 38, 13], [6, 54, 18], white);
    grid.fill_box([-2, 45, -20], [2, 53, -16], white);

    // Slender forward-angled neck
    grid.fill_box([-5, 48, 9], [5, 68, 17], tawny);
    grid.fill_box([-4, 50, 16], [4, 67, 19], white);

    // Head, muzzle, black nose pad
    grid.fill_box([-5, 62, 15], [5, 72, 26], tawny);
    grid.fill_box([-4, 62, 21], [4, 67, 30], cream);
    grid.fill_box([-4, 63, 28], [4, 69, 32], black);

    // Almond eyes & backward ears
    grid.fill_box([-6, 68, 20], [-5, 71, 22], black);
    grid.fill_box([5, 68, 20], [6, 71, 22], black);
    grid.fill_line([-5, 70, 15], [-11, 77, 12], 0, tawny);
    grid.fill_line([5, 70, 15], [11, 77, 12], 0, tawny);
    grid.fill_box([-10, 72, 13], [-6, 76, 14], white);
    grid.fill_box([6, 72, 13], [10, 76, 14], white);

    // Branching antler rack
    grid.fill_line([-4, 71, 16], [-6, 84, 14], 1, antler);
    grid.fill_line([4, 71, 16], [6, 84, 14], 1, antler);
    grid.fill_line([-6, 84, 14], [-13, 98, 11], 0, antler);
    grid.fill_line([6, 84, 14], [13, 98, 11], 0, antler);
    grid.fill_line([-6, 84, 14], [-2, 91, 23], 0, antler);
    grid.fill_line([6, 84, 14], [2, 91, 23], 0, antler);
    grid.fill_line([-11, 92, 12], [-15, 105, 16], 0, antler);
    grid.fill_line([11, 92, 12], [15, 105, 16], 0, antler);

    grid.build_mesh()
}

pub fn create_voxel_boar_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.024);
    let umber = [0.28, 0.16, 0.08, 1.0];
    let ochre = [0.65, 0.44, 0.22, 1.0];
    let highlight = [0.82, 0.62, 0.36, 1.0];
    let snout = [0.65, 0.45, 0.40, 1.0];
    let tusk = [0.96, 0.93, 0.86, 1.0];
    let hoof = [0.14, 0.12, 0.10, 1.0];

    // Hooves
    grid.fill_box([-11, 0, 8], [-7, 3, 12], hoof);
    grid.fill_box([7, 0, 8], [11, 3, 12], hoof);
    grid.fill_box([-11, 0, -14], [-7, 3, -10], hoof);
    grid.fill_box([7, 0, -14], [11, 3, -10], hoof);

    // Sturdy legs
    grid.fill_box([-10, 3, 8], [-8, 14, 12], umber);
    grid.fill_box([8, 3, 8], [10, 14, 12], umber);
    grid.fill_box([-10, 3, -14], [-8, 14, -10], umber);
    grid.fill_box([8, 3, -14], [10, 14, -10], umber);

    // Heavy stocky body with ochre stripes
    grid.fill_box([-13, 12, -20], [13, 30, 16], umber);
    grid.fill_box([-12, 14, -16], [12, 28, 13], ochre);
    grid.fill_box([-11, 17, -12], [11, 26, 9], highlight);

    // Raised spine bristle crest
    grid.fill_box([-3, 30, -18], [3, 37, 13], umber);
    grid.fill_box([-1, 36, -14], [1, 39, 10], ochre);

    // Sloping wedge head & heavy jowls
    grid.fill_box([-9, 14, 13], [9, 28, 29], umber);
    grid.fill_box([-6, 15, 27], [6, 23, 38], snout);

    // Upward-curved ivory tusks
    grid.fill_box([-8, 16, 28], [-6, 25, 31], tusk);
    grid.fill_box([6, 16, 28], [8, 25, 31], tusk);

    grid.build_mesh()
}

pub fn create_voxel_goblin_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.024);
    let skin = [0.38, 0.68, 0.22, 1.0];
    let skin_shadow = [0.26, 0.48, 0.16, 1.0];
    let iron = [0.46, 0.48, 0.52, 1.0];
    let iron_dark = [0.28, 0.30, 0.34, 1.0];
    let leather = [0.36, 0.22, 0.14, 1.0];
    let gold = [0.94, 0.78, 0.18, 1.0];
    let eye = [0.98, 0.92, 0.15, 1.0];
    let bone = [0.92, 0.88, 0.78, 1.0];

    // Armored boots
    grid.fill_box([-8, 0, -5], [-3, 6, 5], iron_dark);
    grid.fill_box([3, 0, -5], [8, 6, 5], iron_dark);
    grid.fill_box([-7, 6, -4], [-4, 18, 4], skin);
    grid.fill_box([4, 6, -4], [7, 18, 4], skin);

    // Studded war belt, gold buckle & tassets
    grid.fill_box([-8, 18, -6], [8, 23, 6], leather);
    grid.fill_box([-4, 18, 6], [4, 23, 7], gold);
    grid.fill_box([-4, 11, 5], [4, 18, 6], leather);

    // Segmented breastplate
    grid.fill_box([-8, 23, -5], [8, 38, 5], iron);
    grid.fill_box([-7, 24, -6], [7, 37, -5], iron_dark);

    // Rounded dual-tier pauldrons
    grid.fill_box([-14, 33, -5], [-8, 41, 5], iron_dark);
    grid.fill_box([-15, 35, -4], [-8, 40, 4], iron);
    grid.fill_box([8, 33, -5], [14, 41, 5], iron_dark);
    grid.fill_box([8, 35, -4], [15, 40, 4], iron);

    // Arms & bracers
    grid.fill_box([-13, 21, -4], [-8, 33, 4], skin);
    grid.fill_box([8, 21, -4], [13, 33, 4], skin);
    grid.fill_box([-13, 16, -4], [-8, 22, 4], iron);
    grid.fill_box([8, 16, -4], [13, 22, 4], iron);

    // Goblin head & jaw
    grid.fill_box([-8, 38, -5], [8, 52, 6], skin);
    grid.fill_box([-7, 38, 4], [7, 43, 7], skin_shadow);

    // Lower jaw tusks
    grid.fill_box([-5, 40, 6], [-4, 44, 7], bone);
    grid.fill_box([4, 40, 6], [5, 44, 7], bone);

    // Glowing eyes
    grid.fill_box([-6, 45, 6], [-4, 47, 7], eye);
    grid.fill_box([4, 45, 6], [6, 47, 7], eye);

    // Pointed lateral ears
    grid.fill_line([-8, 44, 0], [-16, 50, -1], 0, skin);
    grid.fill_line([8, 44, 0], [16, 50, -1], 0, skin);

    // Horned iron helmet
    grid.fill_box([-8, 49, -6], [8, 56, 6], iron);
    grid.fill_line([-6, 54, 0], [-12, 64, 4], 0, bone);
    grid.fill_line([6, 54, 0], [12, 64, 4], 0, bone);

    grid.build_mesh()
}

pub fn create_voxel_peasant_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.026);
    let skin = [0.86, 0.72, 0.60, 1.0];
    let shirt = [0.22, 0.42, 0.85, 1.0];
    let pants = [0.32, 0.26, 0.20, 1.0];
    let hair = [0.28, 0.18, 0.10, 1.0];
    let boots = [0.18, 0.12, 0.08, 1.0];

    grid.fill_box([-7, 0, -4], [-2, 5, 4], boots);
    grid.fill_box([2, 0, -4], [7, 5, 4], boots);
    grid.fill_box([-6, 5, -3], [-2, 18, 3], pants);
    grid.fill_box([2, 5, -3], [6, 18, 3], pants);

    grid.fill_box([-8, 18, -5], [8, 34, 5], shirt);
    grid.fill_box([-12, 18, -3], [-8, 33, 3], shirt);
    grid.fill_box([8, 18, -3], [12, 33, 3], shirt);
    grid.fill_box([-12, 14, -3], [-8, 18, 3], skin);
    grid.fill_box([8, 14, -3], [12, 18, 3], skin);

    grid.fill_box([-5, 34, -5], [5, 44, 5], skin);
    grid.fill_box([-6, 42, -6], [6, 47, 6], hair);

    grid.build_mesh()
}

pub fn create_voxel_pet_mesh() -> Mesh {
    let mut grid = MicroVoxelGrid::new(0.026);
    let coat = [0.86, 0.52, 0.18, 1.0];
    let cream = [0.95, 0.90, 0.80, 1.0];
    let nose = [0.10, 0.10, 0.10, 1.0];
    let ears = [0.68, 0.38, 0.12, 1.0];

    grid.fill_box([-4, 0, -6], [-2, 5, -4], coat);
    grid.fill_box([2, 0, -6], [4, 5, -4], coat);
    grid.fill_box([-4, 0, 4], [-2, 5, 6], coat);
    grid.fill_box([2, 0, 4], [4, 5, 6], coat);

    grid.fill_box([-4, 5, -8], [4, 11, 8], coat);
    grid.fill_box([-3, 4, -6], [3, 7, 6], cream);

    grid.fill_box([-3, 10, 5], [3, 16, 11], coat);
    grid.fill_box([-2, 10, 11], [2, 13, 14], cream);
    grid.set(0, 13, 14, nose);

    grid.fill_box([-5, 14, 6], [-3, 18, 9], ears);
    grid.fill_box([3, 14, 6], [5, 18, 9], ears);

    grid.build_mesh()
}

// ----------------------------------------------------------------------------
// CREATURE ENTITY CREATION & MESH CACHING
// ----------------------------------------------------------------------------

use avian3d::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::pbr::NotShadowCaster;
use crate::components::{NetworkEntity, LogicalPosition, LogicalRotation, Selectable, PeasantUnit, SelectionRing, RTSProxy};
use crate::core::{GameState, GameLayer};

/// Cached GPU mesh handles for fauna and humanoid NPCs.
pub struct CachedCreatureMeshes {
    pub deer: Handle<Mesh>,
    pub boar: Handle<Mesh>,
    pub goblin: Handle<Mesh>,
    pub peasant: Handle<Mesh>,
    pub pet: Handle<Mesh>,
}

impl CachedCreatureMeshes {
    pub fn new(meshes: &mut Assets<Mesh>) -> Self {
        Self {
            deer: meshes.add(create_voxel_deer_mesh()),
            boar: meshes.add(create_voxel_boar_mesh()),
            goblin: meshes.add(create_voxel_goblin_mesh()),
            peasant: meshes.add(create_voxel_peasant_mesh()),
            pet: meshes.add(create_voxel_pet_mesh()),
        }
    }
}

/// Spawns a fully assembled 3D creature / NPC entity with its visual PBR mesh, physics collider, and selection ring.
pub fn spawn_creature_visual_entity(
    commands: &mut Commands,
    cache: &CachedCreatureMeshes,
    materials: &mut Assets<StandardMaterial>,
    meshes: &mut Assets<Mesh>,
    id: u64,
    db_t: &crate::module_bindings::Transform,
    is_peasant: bool,
    is_pet: bool,
    npc_brain: Option<&crate::module_bindings::NpcBrain>,
) -> Entity {
    let mut visual_transform = Transform::from_xyz(0.0, -1.05, 0.0);

    let (mesh_handle, root_collider) = if is_pet {
        visual_transform.translation.y = -0.45;
        (cache.pet.clone(), Collider::cuboid(0.5, 0.8, 0.9))
    } else if let Some(ref brain) = npc_brain {
        let m = match brain.ai_type {
            crate::module_bindings::AiType::Boar => {
                visual_transform.translation.y = -1.05;
                (cache.boar.clone(), Collider::cuboid(0.8, 0.8, 1.4))
            }
            crate::module_bindings::AiType::Deer => {
                visual_transform.translation.y = -1.05;
                (cache.deer.clone(), Collider::cuboid(0.6, 1.8, 1.2))
            }
            crate::module_bindings::AiType::Goblin => {
                visual_transform.translation.y = -1.05;
                (cache.goblin.clone(), Collider::capsule(0.4, 1.3))
            }
            crate::module_bindings::AiType::Friendly | crate::module_bindings::AiType::Peasant => {
                visual_transform.translation.y = -1.05;
                (cache.peasant.clone(), Collider::capsule(0.4, 1.8))
            }
        };

        if brain.state == crate::module_bindings::BrainState::Corpse {
            visual_transform.translation.y = -0.35;
            visual_transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        }
        m
    } else if is_peasant {
        visual_transform.translation.y = -1.05;
        (cache.peasant.clone(), Collider::capsule(0.4, 1.8))
    } else {
        visual_transform.translation.y = -1.05;
        (cache.peasant.clone(), Collider::capsule(0.4, 1.8))
    };

    let npc_type_name = if is_pet {
        "Pet"
    } else if let Some(ref brain) = npc_brain {
        match brain.ai_type {
            crate::module_bindings::AiType::Boar => "Boar",
            crate::module_bindings::AiType::Deer => "Deer",
            crate::module_bindings::AiType::Goblin => "Goblin",
            crate::module_bindings::AiType::Friendly | crate::module_bindings::AiType::Peasant => "Peasant",
        }
    } else if is_peasant {
        "Peasant"
    } else {
        "NPC"
    };

    let mut entity_cmds = commands.spawn((
        Name::new(format!("NPC_{}_{}", npc_type_name, id)),
        StateScoped(GameState::InGame),
        NetworkEntity(id),
        SpatialBundle::from_transform(Transform::from_xyz(db_t.x, db_t.y, db_t.z)),
        LogicalPosition(Vec3::new(db_t.x, db_t.y, db_t.z)),
        LogicalRotation(Quat::IDENTITY),
        Selectable, 
        RigidBody::Kinematic, 
        root_collider,
        CollisionLayers::new([GameLayer::Unit], [GameLayer::Default]),
    ));

    if is_peasant {
        entity_cmds.insert(PeasantUnit { entity_id: id });
    }

    entity_cmds.with_children(|parent| {
        parent.spawn((
            PbrBundle {
                mesh: mesh_handle,
                material: materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.85,
                    ..default()
                }),
                transform: visual_transform,
                ..default()
            },
            RenderLayers::from_layers(&[0, 1, 2]), RTSProxy,
        ));
        parent.spawn((
            PbrBundle {
                mesh: meshes.add(bevy::math::primitives::Torus::new(0.6, 0.05)),
                material: materials.add(StandardMaterial { base_color: Color::srgb(0.0, 1.0, 0.0), unlit: true, ..default() }),
                transform: Transform::from_xyz(0.0, -0.4, 0.0), visibility: Visibility::Hidden, ..default()
            },
            RenderLayers::layer(2), SelectionRing,
            NotShadowCaster,
        ));
    });

    entity_cmds.id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_creature_meshes_initialization() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Mesh>>();

        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        let cache = CachedCreatureMeshes::new(&mut meshes);

        assert!(meshes.get(&cache.deer).is_some());
        assert!(meshes.get(&cache.boar).is_some());
        assert!(meshes.get(&cache.goblin).is_some());
        assert!(meshes.get(&cache.peasant).is_some());
        assert!(meshes.get(&cache.pet).is_some());
    }
}
