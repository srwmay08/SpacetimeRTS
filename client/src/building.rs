// ============================================================================
// File: client/src/building.rs
// ============================================================================
// ----------------------------------------------------------------------------
// MODULAR BUILDING, SELECTIVE PERMEABILITY, KINEMATIC DOORS & DESTRUCTION
// ----------------------------------------------------------------------------
// Architectural Note:
// Manages the client-side lifecycle of modular architectural construction:
// 1. SELECTIVE PERMEABILITY (WINDOWS):
//    Window pieces feature solid structural surround (GameLayer::Environment)
//    and a central translucent glass pane (GameLayer::Glass). The glass collides
//    with physical bodies (GameLayer::Unit, GameLayer::Default) to block movement,
//    while vision raycasts (which mask only Environment/Unit/Default) pass right
//    through the window opening without hitting the glass.
// 2. MODEL SWAPPING FOR LOW-POLY DESTRUCTION:
//    Authoritative building health is owned by SpacetimeDB. When current_health /
//    max_health falls to <= 50%, the Bevy mesh is swapped to its damaged variant
//    (VisualDamageState::Damaged) with a puff of dust. Repairing back above 50%
//    restores the pristine model.
// 3. MOVING DOORS VIA KINEMATIC INTERPOLATION:
//    Doors avoid networked physics joints / hinges (preventing jitter & desync).
//    Instead, the stationary doorframe is RigidBody::Static and the swinging leaf
//    is RigidBody::Kinematic. When door_state updates, Bevy smoothly slerps the
//    leaf rotation towards target_rotation.
// 4. LETHAL FALLS & RUINS (THE ILLUSION OF PHYSICS):
//    Listens to SpacetimeDB `fall_hazard` events. Animates purely visual entities
//    (no physics body) tilting along the mathematical fall vector over 2.5s.
//    At impact timestamp, the server resolves lethal damage on any entity in the
//    sweep, and Bevy spawns or binds the static HarvestableRuin collider.
// ----------------------------------------------------------------------------

use std::collections::BTreeSet;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use avian3d::prelude::*;
use tracing::info;
use spacetimedb_sdk::Table;

use crate::core::GameLayer;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::voxel_mesh::{VoxelBox, build_voxel_mesh};
use crate::module_bindings::structure_table::StructureTableAccess; 
use crate::module_bindings::door_state_table::DoorStateTableAccess;
use crate::module_bindings::fall_hazard_table::FallHazardTableAccess;
use crate::module_bindings::place_structure_reducer::place_structure;
use crate::module_bindings::spawn_template_blueprint_reducer::spawn_template_blueprint;
use crate::templates::BuildingTemplateType;

// ----------------------------------------------------------------------------
// DATA-DRIVEN CONFIGURATIONS
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Component)]
pub enum ModularPieceType {
    Foundation,
    Workbench,
    Campfire,
    Wall,
    Window,
    Door,
    Floor,
    Roof,
    Ramp,
}

impl ModularPieceType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Foundation => "Foundation",
            Self::Workbench => "Workbench",
            Self::Campfire => "Campfire",
            Self::Wall => "Wall",
            Self::Window => "Window",
            Self::Door => "Door",
            Self::Floor => "Floor",
            Self::Roof => "Roof",
            Self::Ramp => "Ramp",
        }
    }

    pub fn wood_cost(&self) -> u32 {
        match self {
            Self::Foundation => 20,
            Self::Workbench => 8,
            Self::Campfire => 4,
            Self::Wall => 8,
            Self::Window => 8,
            Self::Door => 12,
            Self::Floor => 12,
            Self::Roof => 12,
            Self::Ramp => 16,
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
            Self::Wall | Self::Window | Self::Door => vec![
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
                Socket { name: "Top".into(), local_offset: Vec3::new(0.0, 1.5, -2.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "Bottom".into(), local_offset: Vec3::new(0.0, -1.5, 2.0), local_rotation: Quat::IDENTITY, is_occupied: false },
            ],
            Self::Workbench => vec![],
            Self::Campfire => vec![],
        }
    }
}

#[derive(Resource)]
pub struct BuildModeState {
    pub is_active: bool,
    pub selected_piece: ModularPieceType,
    pub selected_faction: BuildingFaction,
    pub rotation_steps: u8,
    pub selected_template: Option<BuildingTemplateType>,
}

impl Default for BuildModeState {
    fn default() -> Self {
        Self {
            is_active: false,
            selected_piece: ModularPieceType::Foundation,
            selected_faction: BuildingFaction::Human,
            rotation_steps: 0,
            selected_template: None,
        }
    }
}

// ----------------------------------------------------------------------------
// PROCEDURAL MESH GENERATION (PRISTINE & DAMAGED VARIANTS)
// ----------------------------------------------------------------------------

pub fn create_ramp_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let positions = vec![
        [-2.0, -1.5,  2.0], [ 2.0, -1.5,  2.0], [ 2.0, -1.5, -2.0], [-2.0, -1.5, -2.0],
        [ 2.0, -1.5, -2.0], [-2.0, -1.5, -2.0], [-2.0,  1.5, -2.0], [ 2.0,  1.5, -2.0],
        [-2.0, -1.5,  2.0], [ 2.0, -1.5,  2.0], [ 2.0,  1.5, -2.0], [-2.0,  1.5, -2.0],
        [-2.0, -1.5,  2.0], [-2.0, -1.5, -2.0], [-2.0,  1.5, -2.0],
        [ 2.0, -1.5,  2.0], [ 2.0, -1.5, -2.0], [ 2.0,  1.5, -2.0],
    ];
    
    let normals = vec![
        [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0],
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0],
        [0.0, 0.8, 0.6], [0.0, 0.8, 0.6], [0.0, 0.8, 0.6], [0.0, 0.8, 0.6],
        [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0],
    ];
    
    let indices = vec![
        0, 2, 1,  0, 3, 2,
        4, 6, 5,  4, 7, 6,
        8, 9, 10,  8, 10, 11,
        12, 14, 13,
        15, 16, 17,
    ];

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

pub fn create_workbench_mesh() -> Mesh {
    let wood = [0.42, 0.28, 0.16, 1.0];
    let top_wood = [0.55, 0.38, 0.22, 1.0];
    let iron = [0.35, 0.35, 0.38, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.70, 0.0, -0.40), max: Vec3::new(-0.52, 0.75, -0.22), color: wood },
        VoxelBox { min: Vec3::new(0.52, 0.0, -0.40), max: Vec3::new(0.70, 0.75, -0.22), color: wood },
        VoxelBox { min: Vec3::new(-0.70, 0.0, 0.22), max: Vec3::new(-0.52, 0.75, 0.40), color: wood },
        VoxelBox { min: Vec3::new(0.52, 0.0, 0.22), max: Vec3::new(0.70, 0.75, 0.40), color: wood },
        VoxelBox { min: Vec3::new(-0.62, 0.15, -0.32), max: Vec3::new(0.62, 0.22, 0.32), color: wood },
        VoxelBox { min: Vec3::new(-0.80, 0.75, -0.50), max: Vec3::new(0.80, 0.95, 0.50), color: top_wood },
        VoxelBox { min: Vec3::new(-0.55, 0.95, -0.25), max: Vec3::new(-0.25, 1.18, 0.05), color: iron },
        VoxelBox { min: Vec3::new(0.35, 0.95, -0.35), max: Vec3::new(0.65, 1.10, -0.15), color: wood },
    ])
}

pub fn create_campfire_mesh() -> Mesh {
    let stone = [0.48, 0.48, 0.50, 1.0];
    let wood = [0.32, 0.18, 0.10, 1.0];
    let embers = [0.88, 0.32, 0.08, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.65, 0.0, -0.65), max: Vec3::new(0.65, 0.25, -0.42), color: stone },
        VoxelBox { min: Vec3::new(-0.65, 0.0, 0.42), max: Vec3::new(0.65, 0.25, 0.65), color: stone },
        VoxelBox { min: Vec3::new(-0.65, 0.0, -0.42), max: Vec3::new(-0.42, 0.25, 0.42), color: stone },
        VoxelBox { min: Vec3::new(0.42, 0.0, -0.42), max: Vec3::new(0.65, 0.25, 0.42), color: stone },
        VoxelBox { min: Vec3::new(-0.40, 0.0, -0.40), max: Vec3::new(0.40, 0.12, 0.40), color: embers },
        VoxelBox { min: Vec3::new(-0.45, 0.10, -0.12), max: Vec3::new(0.45, 0.24, 0.12), color: wood },
        VoxelBox { min: Vec3::new(-0.12, 0.20, -0.45), max: Vec3::new(0.12, 0.34, 0.45), color: wood },
    ])
}

#[allow(dead_code)]
pub fn create_catapult_mesh() -> Mesh {
    let wood_dark = [0.35, 0.22, 0.12, 1.0];
    let wood_light = [0.52, 0.35, 0.20, 1.0];
    let iron = [0.25, 0.26, 0.28, 1.0];
    let rope = [0.65, 0.52, 0.28, 1.0];
    let stone = [0.50, 0.48, 0.46, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.85, 0.20, -1.50), max: Vec3::new(-0.65, 0.45, 1.50), color: wood_dark },
        VoxelBox { min: Vec3::new(0.65, 0.20, -1.50), max: Vec3::new(0.85, 0.45, 1.50), color: wood_dark },
        VoxelBox { min: Vec3::new(-0.65, 0.22, -1.35), max: Vec3::new(0.65, 0.42, -1.15), color: wood_light },
        VoxelBox { min: Vec3::new(-0.65, 0.22, -0.30), max: Vec3::new(0.65, 0.42, -0.10), color: wood_light },
        VoxelBox { min: Vec3::new(-0.65, 0.22, 1.15), max: Vec3::new(0.65, 0.42, 1.35), color: wood_light },
        VoxelBox { min: Vec3::new(-1.05, 0.0, -1.25), max: Vec3::new(-0.88, 0.65, -0.85), color: iron },
        VoxelBox { min: Vec3::new(0.88, 0.0, -1.25), max: Vec3::new(1.05, 0.65, -0.85), color: iron },
        VoxelBox { min: Vec3::new(-1.05, 0.0, 0.85), max: Vec3::new(-0.88, 0.65, 1.25), color: iron },
        VoxelBox { min: Vec3::new(0.88, 0.0, 0.85), max: Vec3::new(1.05, 0.65, 1.25), color: iron },
        VoxelBox { min: Vec3::new(-1.12, 0.25, -1.10), max: Vec3::new(1.12, 0.40, -1.00), color: wood_dark },
        VoxelBox { min: Vec3::new(-1.12, 0.25, 1.00), max: Vec3::new(1.12, 0.40, 1.10), color: wood_dark },
        VoxelBox { min: Vec3::new(-0.82, 0.45, -0.35), max: Vec3::new(-0.65, 1.85, -0.15), color: wood_dark },
        VoxelBox { min: Vec3::new(0.65, 0.45, -0.35), max: Vec3::new(0.82, 1.85, -0.15), color: wood_dark },
        VoxelBox { min: Vec3::new(-0.80, 0.45, -0.95), max: Vec3::new(-0.68, 1.35, -0.35), color: wood_light },
        VoxelBox { min: Vec3::new(0.68, 0.45, -0.95), max: Vec3::new(0.80, 1.35, -0.35), color: wood_light },
        VoxelBox { min: Vec3::new(-0.65, 0.50, -0.32), max: Vec3::new(0.65, 0.80, -0.18), color: rope },
        VoxelBox { min: Vec3::new(-0.95, 0.52, -0.30), max: Vec3::new(-0.82, 0.78, -0.20), color: iron },
        VoxelBox { min: Vec3::new(0.82, 0.52, -0.30), max: Vec3::new(0.95, 0.78, -0.20), color: iron },
        VoxelBox { min: Vec3::new(-0.12, 0.55, -0.30), max: Vec3::new(0.12, 1.10, 1.35), color: wood_light },
        VoxelBox { min: Vec3::new(-0.25, 0.95, 1.20), max: Vec3::new(0.25, 1.25, 1.55), color: wood_dark },
        VoxelBox { min: Vec3::new(-0.18, 1.18, 1.25), max: Vec3::new(0.18, 1.50, 1.50), color: stone },
        VoxelBox { min: Vec3::new(-0.65, 1.60, -0.32), max: Vec3::new(0.65, 1.82, -0.18), color: iron },
    ])
}

#[allow(dead_code)]
pub fn create_trebuchet_mesh() -> Mesh {
    let timber = [0.38, 0.24, 0.14, 1.0];
    let timber_light = [0.55, 0.38, 0.22, 1.0];
    let iron = [0.26, 0.27, 0.28, 1.0];
    let stone = [0.48, 0.46, 0.45, 1.0];
    let rope = [0.70, 0.58, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-1.30, 0.0, -1.80), max: Vec3::new(-1.05, 0.28, 1.80), color: timber },
        VoxelBox { min: Vec3::new(1.05, 0.0, -1.80), max: Vec3::new(1.30, 0.28, 1.80), color: timber },
        VoxelBox { min: Vec3::new(-1.05, 0.0, -1.60), max: Vec3::new(1.05, 0.25, -1.35), color: timber },
        VoxelBox { min: Vec3::new(-1.05, 0.0, 1.35), max: Vec3::new(1.05, 0.25, 1.60), color: timber },
        VoxelBox { min: Vec3::new(-1.25, 0.25, -0.15), max: Vec3::new(-1.05, 3.20, 0.15), color: timber },
        VoxelBox { min: Vec3::new(-1.22, 0.25, -1.40), max: Vec3::new(-1.08, 3.10, -0.10), color: timber_light },
        VoxelBox { min: Vec3::new(-1.22, 0.25, 0.10), max: Vec3::new(-1.08, 3.10, 1.40), color: timber_light },
        VoxelBox { min: Vec3::new(1.05, 0.25, -0.15), max: Vec3::new(1.25, 3.20, 0.15), color: timber },
        VoxelBox { min: Vec3::new(1.08, 0.25, -1.40), max: Vec3::new(1.22, 3.10, -0.10), color: timber_light },
        VoxelBox { min: Vec3::new(1.08, 0.25, 0.10), max: Vec3::new(1.22, 3.10, 1.40), color: timber_light },
        VoxelBox { min: Vec3::new(-1.35, 3.00, -0.12), max: Vec3::new(1.35, 3.22, 0.12), color: iron },
        VoxelBox { min: Vec3::new(-0.16, 2.70, -1.20), max: Vec3::new(0.16, 3.35, 2.80), color: timber_light },
        VoxelBox { min: Vec3::new(-0.55, 1.60, -1.45), max: Vec3::new(0.55, 2.80, -0.95), color: timber },
        VoxelBox { min: Vec3::new(-0.48, 1.70, -1.38), max: Vec3::new(0.48, 2.75, -1.02), color: stone },
        VoxelBox { min: Vec3::new(-0.04, 0.60, 2.60), max: Vec3::new(0.04, 2.80, 2.68), color: rope },
        VoxelBox { min: Vec3::new(-0.25, 0.40, 2.50), max: Vec3::new(0.25, 0.85, 2.95), color: stone },
    ])
}

#[allow(dead_code)]
pub fn create_ballista_mesh() -> Mesh {
    let dark_wood = [0.36, 0.22, 0.12, 1.0];
    let light_wood = [0.54, 0.36, 0.20, 1.0];
    let iron = [0.28, 0.29, 0.30, 1.0];
    let bronze = [0.72, 0.58, 0.24, 1.0];
    let red_mat = [0.85, 0.15, 0.15, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-0.65, 0.0, -0.65), max: Vec3::new(-0.45, 0.85, -0.45), color: dark_wood },
        VoxelBox { min: Vec3::new(0.45, 0.0, -0.65), max: Vec3::new(0.65, 0.85, -0.45), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.12, 0.0, 0.55), max: Vec3::new(0.12, 0.85, 0.75), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.25, 0.80, -0.25), max: Vec3::new(0.25, 1.15, 0.25), color: iron },
        VoxelBox { min: Vec3::new(-0.15, 1.15, -0.15), max: Vec3::new(0.15, 1.35, 0.15), color: bronze },
        VoxelBox { min: Vec3::new(-0.16, 1.25, -1.40), max: Vec3::new(0.16, 1.45, 1.20), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.06, 1.45, -1.35), max: Vec3::new(0.06, 1.48, 1.15), color: iron },
        VoxelBox { min: Vec3::new(-0.95, 1.15, -1.45), max: Vec3::new(0.95, 1.55, -1.25), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.90, 1.00, -1.40), max: Vec3::new(-0.70, 1.70, -1.20), color: bronze },
        VoxelBox { min: Vec3::new(0.70, 1.00, -1.40), max: Vec3::new(0.90, 1.70, -1.20), color: bronze },
        VoxelBox { min: Vec3::new(-1.45, 1.28, -1.15), max: Vec3::new(-0.85, 1.42, -1.35), color: light_wood },
        VoxelBox { min: Vec3::new(0.85, 1.28, -1.15), max: Vec3::new(1.45, 1.42, -1.35), color: light_wood },
        VoxelBox { min: Vec3::new(-1.40, 1.32, -1.15), max: Vec3::new(1.40, 1.38, -0.20), color: iron },
        VoxelBox { min: Vec3::new(-0.35, 1.20, 0.95), max: Vec3::new(0.35, 1.45, 1.15), color: iron },
        VoxelBox { min: Vec3::new(-0.45, 1.15, 1.00), max: Vec3::new(-0.35, 1.65, 1.10), color: bronze },
        VoxelBox { min: Vec3::new(-0.03, 1.48, -1.25), max: Vec3::new(0.03, 1.54, 0.35), color: light_wood },
        VoxelBox { min: Vec3::new(-0.06, 1.47, -1.45), max: Vec3::new(0.06, 1.55, -1.25), color: iron },
        VoxelBox { min: Vec3::new(-0.08, 1.46, 0.15), max: Vec3::new(0.08, 1.56, 0.30), color: red_mat },
    ])
}

#[allow(dead_code)]
pub fn create_battering_ram_mesh() -> Mesh {
    let timber = [0.35, 0.22, 0.12, 1.0];
    let roof_shingle = [0.45, 0.28, 0.15, 1.0];
    let iron = [0.26, 0.27, 0.28, 1.0];
    let bronze = [0.75, 0.55, 0.22, 1.0];
    let log_wood = [0.48, 0.32, 0.18, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-1.15, 0.0, -1.45), max: Vec3::new(-0.95, 0.70, -0.95), color: iron },
        VoxelBox { min: Vec3::new(0.95, 0.0, -1.45), max: Vec3::new(1.15, 0.70, -0.95), color: iron },
        VoxelBox { min: Vec3::new(-1.15, 0.0, 0.95), max: Vec3::new(-0.95, 0.70, 1.45), color: iron },
        VoxelBox { min: Vec3::new(0.95, 0.0, 0.95), max: Vec3::new(1.15, 0.70, 1.45), color: iron },
        VoxelBox { min: Vec3::new(-0.95, 0.25, -1.70), max: Vec3::new(-0.75, 0.50, 1.70), color: timber },
        VoxelBox { min: Vec3::new(0.75, 0.25, -1.70), max: Vec3::new(0.95, 0.50, 1.70), color: timber },
        VoxelBox { min: Vec3::new(-0.92, 0.50, -1.55), max: Vec3::new(-0.75, 2.20, -1.35), color: timber },
        VoxelBox { min: Vec3::new(0.75, 0.50, -1.55), max: Vec3::new(0.92, 2.20, -1.35), color: timber },
        VoxelBox { min: Vec3::new(-0.92, 0.50, 1.35), max: Vec3::new(-0.75, 2.20, 1.55), color: timber },
        VoxelBox { min: Vec3::new(0.75, 0.50, 1.35), max: Vec3::new(0.92, 2.20, 1.55), color: timber },
        VoxelBox { min: Vec3::new(-0.85, 2.10, -1.75), max: Vec3::new(0.85, 2.25, 1.75), color: timber },
        VoxelBox { min: Vec3::new(-0.95, 2.10, -1.75), max: Vec3::new(0.0, 2.65, 1.75), color: roof_shingle },
        VoxelBox { min: Vec3::new(0.0, 2.10, -1.75), max: Vec3::new(0.95, 2.65, 1.75), color: roof_shingle },
        VoxelBox { min: Vec3::new(-0.06, 1.25, -0.90), max: Vec3::new(0.06, 2.15, -0.82), color: iron },
        VoxelBox { min: Vec3::new(-0.06, 1.25, 0.82), max: Vec3::new(0.06, 2.15, 0.90), color: iron },
        VoxelBox { min: Vec3::new(-0.22, 0.85, -1.95), max: Vec3::new(0.22, 1.30, 1.85), color: log_wood },
        VoxelBox { min: Vec3::new(-0.28, 0.80, -2.40), max: Vec3::new(0.28, 1.35, -1.95), color: bronze },
        VoxelBox { min: Vec3::new(-0.45, 1.00, -2.25), max: Vec3::new(-0.28, 1.45, -2.05), color: iron },
        VoxelBox { min: Vec3::new(0.28, 1.00, -2.25), max: Vec3::new(0.45, 1.45, -2.05), color: iron },
        VoxelBox { min: Vec3::new(-0.32, 1.00, -0.45), max: Vec3::new(0.32, 1.10, -0.35), color: iron },
        VoxelBox { min: Vec3::new(-0.32, 1.00, 0.35), max: Vec3::new(0.32, 1.10, 0.45), color: iron },
    ])
}

/// Pristine architectural wall model engineered for FPS crosshair alignment.
/// Embeds a distinctive high-contrast horizontal datum band / sightline trim at exactly y = +0.25m
/// (1.75m above floor level, corresponding to player eye-level in first-person perspective).
pub fn create_wall_mesh() -> Mesh {
    let timber_dark = [0.36, 0.24, 0.14, 1.0];
    let timber_light = [0.52, 0.38, 0.22, 1.0];
    let stone_body = [0.48, 0.46, 0.44, 1.0];
    let iron_trim = [0.25, 0.26, 0.28, 1.0];
    let datum_band = [0.78, 0.62, 0.28, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-1.75, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(1.75, -1.5, -0.22), max: Vec3::new(2.0, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.75, -1.5, -0.18), max: Vec3::new(1.75, -0.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-1.80, -0.35, -0.21), max: Vec3::new(1.80, -0.20, 0.21), color: timber_light },
        VoxelBox { min: Vec3::new(-1.75, -0.20, -0.18), max: Vec3::new(1.75, 0.20, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-1.80, 0.20, -0.21), max: Vec3::new(1.80, 0.30, 0.21), color: datum_band },
        VoxelBox { min: Vec3::new(-1.75, 0.30, -0.18), max: Vec3::new(1.75, 1.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(2.0, 1.50, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.85, 0.18, -0.22), max: Vec3::new(-1.70, 0.32, 0.22), color: iron_trim },
        VoxelBox { min: Vec3::new(1.70, 0.18, -0.22), max: Vec3::new(1.85, 0.32, 0.22), color: iron_trim },
    ])
}

/// Damaged wall model used for model swapping when structure health drops to <= 50%.
/// Displays exposed broken laths, missing masonry chunks, and jagged fractures.
pub fn create_damaged_wall_mesh() -> Mesh {
    let timber_dark = [0.32, 0.20, 0.10, 1.0];
    let timber_split = [0.45, 0.30, 0.16, 1.0];
    let stone_body = [0.42, 0.40, 0.38, 1.0];
    let stone_dark = [0.30, 0.28, 0.26, 1.0];
    let iron_trim = [0.20, 0.20, 0.22, 1.0];
    let datum_broken = [0.60, 0.48, 0.20, 1.0];

    build_voxel_mesh(&[
        // Left upright post (splintered top)
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-1.75, 1.10, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.95, 1.10, -0.12), max: Vec3::new(-1.80, 1.38, 0.12), color: timber_split },
        // Right upright post
        VoxelBox { min: Vec3::new(1.75, -1.5, -0.22), max: Vec3::new(2.0, 1.5, 0.22), color: timber_dark },
        // Lower masonry course with missing chunk on right
        VoxelBox { min: Vec3::new(-1.75, -1.5, -0.18), max: Vec3::new(0.60, -0.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(0.60, -1.5, -0.15), max: Vec3::new(1.75, -0.85, 0.15), color: stone_dark },
        // Broken tactical ledge
        VoxelBox { min: Vec3::new(-1.80, -0.35, -0.21), max: Vec3::new(0.40, -0.20, 0.21), color: timber_split },
        // Mid wall breach (hole in center-right)
        VoxelBox { min: Vec3::new(-1.75, -0.20, -0.18), max: Vec3::new(-0.25, 0.20, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(1.05, -0.20, -0.18), max: Vec3::new(1.75, 0.20, 0.18), color: stone_body },
        // Fractured datum band
        VoxelBox { min: Vec3::new(-1.80, 0.20, -0.21), max: Vec3::new(-0.30, 0.30, 0.21), color: datum_broken },
        VoxelBox { min: Vec3::new(1.10, 0.20, -0.21), max: Vec3::new(1.80, 0.30, 0.21), color: datum_broken },
        // Upper damaged masonry
        VoxelBox { min: Vec3::new(-1.75, 0.30, -0.18), max: Vec3::new(-0.10, 1.15, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(0.95, 0.30, -0.18), max: Vec3::new(1.75, 1.35, 0.18), color: stone_body },
        // Broken header beam
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(-0.80, 1.50, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.40, 1.35, -0.22), max: Vec3::new(2.0, 1.50, 0.22), color: timber_dark },
        // Chipped iron brace
        VoxelBox { min: Vec3::new(-1.85, 0.18, -0.22), max: Vec3::new(-1.70, 0.32, 0.22), color: iron_trim },
    ])
}

/// Window wall model: 4.0m wide x 3.0m high structural frame with a 1.9m wide x 1.15m high
/// central window opening at eye level (+0.25m datum). Embedded frosted glass pane allows
/// line-of-sight raycasts through Rapier CollisionGroups while blocking player collision.
pub fn create_window_mesh() -> Mesh {
    let timber_dark = [0.36, 0.24, 0.14, 1.0];
    let timber_light = [0.52, 0.38, 0.22, 1.0];
    let stone_body = [0.48, 0.46, 0.44, 1.0];
    let iron_trim = [0.25, 0.26, 0.28, 1.0];
    let datum_band = [0.78, 0.62, 0.28, 1.0];
    // Light translucent blue crystalline glass pane
    let glass_tint = [0.52, 0.78, 0.90, 0.70];

    build_voxel_mesh(&[
        // Left & Right Structural Posts (x = -2.0 to -0.95 and +0.95 to +2.0)
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-0.95, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.22), max: Vec3::new(2.0, 1.5, 0.22), color: timber_dark },
        // Lower Masonry Sill Course (y = -1.5 to -0.32)
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.18), max: Vec3::new(0.95, -0.32, 0.18), color: stone_body },
        // Tactical Crouch Ledge on Sill
        VoxelBox { min: Vec3::new(-1.0, -0.38, -0.24), max: Vec3::new(1.0, -0.24, 0.24), color: timber_light },
        // Upper Masonry Header Course (y = +0.85 to +1.35)
        VoxelBox { min: Vec3::new(-0.95, 0.85, -0.18), max: Vec3::new(0.95, 1.35, 0.18), color: stone_body },
        // Top Roof Header Beam (y = +1.35 to +1.50)
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(2.0, 1.50, 0.22), color: timber_dark },
        // Eye-Level Datum Line on side jambs
        VoxelBox { min: Vec3::new(-2.0, 0.20, -0.23), max: Vec3::new(-0.95, 0.30, 0.23), color: datum_band },
        VoxelBox { min: Vec3::new(0.95, 0.20, -0.23), max: Vec3::new(2.0, 0.30, 0.23), color: datum_band },
        // Iron Window Sill Brackets
        VoxelBox { min: Vec3::new(-0.98, -0.36, -0.22), max: Vec3::new(-0.90, -0.22, 0.22), color: iron_trim },
        VoxelBox { min: Vec3::new(0.90, -0.36, -0.22), max: Vec3::new(0.98, -0.22, 0.22), color: iron_trim },
        // Central Iron Window Mullions (vertical & horizontal crossbars)
        VoxelBox { min: Vec3::new(-0.04, -0.24, -0.06), max: Vec3::new(0.04, 0.85, 0.06), color: iron_trim },
        VoxelBox { min: Vec3::new(-0.95, 0.25, -0.06), max: Vec3::new(0.95, 0.33, 0.06), color: iron_trim },
        // Translucent Glass Pane (y = -0.24 to +0.85, x = -0.95 to +0.95)
        VoxelBox { min: Vec3::new(-0.92, -0.22, -0.03), max: Vec3::new(0.92, 0.83, 0.03), color: glass_tint },
    ])
}

/// Damaged window wall model with fractured masonry and shattered glass pane.
pub fn create_damaged_window_mesh() -> Mesh {
    let timber_dark = [0.32, 0.20, 0.10, 1.0];
    let stone_body = [0.42, 0.40, 0.38, 1.0];
    let iron_trim = [0.20, 0.20, 0.22, 1.0];
    let glass_tint = [0.52, 0.78, 0.90, 0.70];

    build_voxel_mesh(&[
        // Left & Right Structural Posts
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-0.95, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.22), max: Vec3::new(2.0, 1.25, 0.22), color: timber_dark },
        // Cracked Lower Sill
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.18), max: Vec3::new(0.35, -0.32, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(0.35, -1.5, -0.15), max: Vec3::new(0.95, -0.65, 0.15), color: stone_body },
        // Upper Header Course
        VoxelBox { min: Vec3::new(-0.95, 0.95, -0.18), max: Vec3::new(0.95, 1.35, 0.18), color: stone_body },
        // Top Roof Header Beam
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(1.10, 1.50, 0.22), color: timber_dark },
        // Bent Mullion
        VoxelBox { min: Vec3::new(-0.04, -0.24, -0.06), max: Vec3::new(0.04, 0.45, 0.06), color: iron_trim },
        // Shattered glass shards remaining in corners
        VoxelBox { min: Vec3::new(-0.92, -0.22, -0.03), max: Vec3::new(-0.45, 0.15, 0.03), color: glass_tint },
        VoxelBox { min: Vec3::new(0.55, 0.45, -0.03), max: Vec3::new(0.92, 0.83, 0.03), color: glass_tint },
    ])
}

/// Stationary doorframe model: 4.0m wide x 3.0m high wall with an open 1.4m wide x 2.4m high doorway.
pub fn create_door_frame_mesh() -> Mesh {
    let timber_dark = [0.36, 0.24, 0.14, 1.0];
    let timber_light = [0.52, 0.38, 0.22, 1.0];
    let stone_body = [0.48, 0.46, 0.44, 1.0];
    let iron_trim = [0.25, 0.26, 0.28, 1.0];
    let datum_band = [0.78, 0.62, 0.28, 1.0];

    build_voxel_mesh(&[
        // Left Structural Wing (x = -2.0 to -0.70)
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-1.65, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.65, -1.5, -0.18), max: Vec3::new(-0.70, 1.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-0.78, -1.5, -0.24), max: Vec3::new(-0.68, 1.00, 0.24), color: timber_dark },
        // Right Structural Wing (x = +0.70 to +2.0)
        VoxelBox { min: Vec3::new(1.65, -1.5, -0.22), max: Vec3::new(2.0, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.18), max: Vec3::new(1.65, 1.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(0.68, -1.5, -0.24), max: Vec3::new(0.78, 1.00, 0.24), color: timber_dark },
        // Door Threshold Plate on Ground
        VoxelBox { min: Vec3::new(-0.70, -1.5, -0.22), max: Vec3::new(0.70, -1.42, 0.22), color: iron_trim },
        // Top Lintel Header Beam (above door opening: y = 0.90 to 1.50)
        VoxelBox { min: Vec3::new(-0.78, 0.90, -0.24), max: Vec3::new(0.78, 1.05, 0.24), color: timber_light },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.18), max: Vec3::new(0.70, 1.35, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(2.0, 1.50, 0.22), color: timber_dark },
        // Eye-Level Datum Line on outer masonry wings
        VoxelBox { min: Vec3::new(-1.65, 0.20, -0.21), max: Vec3::new(-0.70, 0.30, 0.21), color: datum_band },
        VoxelBox { min: Vec3::new(0.70, 0.20, -0.21), max: Vec3::new(1.65, 0.30, 0.21), color: datum_band },
        // Heavy Cast-Iron Door Hinges on Left Post
        VoxelBox { min: Vec3::new(-0.76, -0.80, -0.10), max: Vec3::new(-0.66, -0.60, 0.10), color: iron_trim },
        VoxelBox { min: Vec3::new(-0.76, 0.40, -0.10), max: Vec3::new(-0.66, 0.60, 0.10), color: iron_trim },
    ])
}

/// Swinging door leaf model: 1.36m wide x 2.30m high reinforced timber door leaf.
/// Pivot axis is located at x = 0.0 relative to its hinge joint so rotating about Y swings it naturally.
pub fn create_door_leaf_mesh() -> Mesh {
    let timber = [0.42, 0.26, 0.14, 1.0];
    let timber_plank = [0.48, 0.32, 0.18, 1.0];
    let iron = [0.24, 0.25, 0.26, 1.0];

    // Pivot is at local x = 0.0; the door leaf extends from x = 0.0 to x = +1.36
    build_voxel_mesh(&[
        // Vertical Planks
        VoxelBox { min: Vec3::new(0.02, -1.15, -0.06), max: Vec3::new(0.44, 1.15, 0.06), color: timber },
        VoxelBox { min: Vec3::new(0.46, -1.15, -0.06), max: Vec3::new(0.90, 1.15, 0.06), color: timber_plank },
        VoxelBox { min: Vec3::new(0.92, -1.15, -0.06), max: Vec3::new(1.34, 1.15, 0.06), color: timber },
        // Horizontal Heavy Iron Bracing Straps
        VoxelBox { min: Vec3::new(0.0, -0.75, -0.08), max: Vec3::new(1.30, -0.65, 0.08), color: iron },
        VoxelBox { min: Vec3::new(0.0, 0.45, -0.08), max: Vec3::new(1.30, 0.55, 0.08), color: iron },
        // Diagonal Cross-Brace
        VoxelBox { min: Vec3::new(0.15, -0.65, -0.07), max: Vec3::new(1.15, 0.45, 0.07), color: timber_plank },
        // Iron Door Ring Handle & Latch (at x = 1.15m)
        VoxelBox { min: Vec3::new(1.10, -0.15, 0.06), max: Vec3::new(1.25, -0.05, 0.14), color: iron },
        VoxelBox { min: Vec3::new(1.10, -0.15, -0.14), max: Vec3::new(1.25, -0.05, -0.06), color: iron },
    ])
}

/// Damaged doorframe model for visual destruction state.
pub fn create_damaged_door_frame_mesh() -> Mesh {
    let timber_dark = [0.32, 0.20, 0.10, 1.0];
    let stone_body = [0.42, 0.40, 0.38, 1.0];
    let iron_trim = [0.20, 0.20, 0.22, 1.0];

    build_voxel_mesh(&[
        // Left Wing
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-1.65, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(-1.65, -1.5, -0.18), max: Vec3::new(-0.70, 0.90, 0.18), color: stone_body },
        VoxelBox { min: Vec3::new(-0.78, -1.5, -0.24), max: Vec3::new(-0.68, 1.00, 0.24), color: timber_dark },
        // Right Wing
        VoxelBox { min: Vec3::new(1.65, -1.5, -0.22), max: Vec3::new(2.0, 1.25, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.18), max: Vec3::new(1.65, 1.35, 0.18), color: stone_body },
        // Threshold
        VoxelBox { min: Vec3::new(-0.70, -1.5, -0.22), max: Vec3::new(0.70, -1.42, 0.22), color: iron_trim },
        // Cracked Top Header
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(0.20, 1.50, 0.22), color: timber_dark },
    ])
}

/// Procedural low-poly rubble pile (2.0m x 1.0m x 2.0m) used for permanent collapse ruins.
pub fn create_rubble_mesh() -> Mesh {
    let stone = [0.48, 0.46, 0.44, 1.0];
    let stone_dark = [0.36, 0.35, 0.34, 1.0];
    let timber = [0.38, 0.24, 0.14, 1.0];

    build_voxel_mesh(&[
        // Base masonry block mound
        VoxelBox { min: Vec3::new(-1.40, 0.0, -1.40), max: Vec3::new(1.40, 0.45, 1.40), color: stone_dark },
        VoxelBox { min: Vec3::new(-1.00, 0.35, -0.90), max: Vec3::new(0.90, 0.85, 0.95), color: stone },
        VoxelBox { min: Vec3::new(-0.60, 0.70, -0.50), max: Vec3::new(0.50, 1.15, 0.60), color: stone },
        // Scattered fallen timber beams
        VoxelBox { min: Vec3::new(-1.30, 0.25, -0.30), max: Vec3::new(1.20, 0.42, -0.12), color: timber },
        VoxelBox { min: Vec3::new(-0.25, 0.65, -1.10), max: Vec3::new(0.45, 0.82, 1.20), color: timber },
        // Broken stone caps
        VoxelBox { min: Vec3::new(-1.25, 0.10, 0.65), max: Vec3::new(-0.75, 0.55, 1.15), color: stone },
        VoxelBox { min: Vec3::new(0.65, 0.10, -1.25), max: Vec3::new(1.15, 0.52, -0.75), color: stone },
    ])
}

// ----------------------------------------------------------------------------
// PIECE NAME RESOLUTION
// ----------------------------------------------------------------------------

pub fn base_piece_name(piece_type: &str) -> &str {
    if let Some((_, base)) = piece_type.split_once('_') {
        base
    } else if let Some((_, base)) = piece_type.split_once(':') {
        base
    } else {
        piece_type
    }
}

// ----------------------------------------------------------------------------
// HIGH ELF (THE PRISTINE BASTION) PROCEDURAL MODELS
// ----------------------------------------------------------------------------
// Early 2000s MMO aesthetic:
// - Polished mint-green marble walls
// - Dark heavily-veined lapis/obsidian roofs
// - Clean light interior framed by polished brown wood and gold filigree
// - Sharp, sweeping geometry and symmetrical pilasters

pub fn create_high_elf_wall_mesh() -> Mesh {
    let mint_marble = [0.68, 0.88, 0.78, 1.0];
    let mint_light = [0.78, 0.94, 0.84, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];
    let polished_wood = [0.45, 0.28, 0.16, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];

    build_voxel_mesh(&[
        // Symmetrical slender pilasters on left/right
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-1.75, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(1.75, -1.5, -0.24), max: Vec3::new(2.0, 1.5, 0.24), color: polished_wood },
        // Gold capital rings at base and top of pilasters
        VoxelBox { min: Vec3::new(-2.02, -1.5, -0.26), max: Vec3::new(-1.73, -1.35, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.73, -1.5, -0.26), max: Vec3::new(2.02, -1.35, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(-2.02, 1.35, -0.26), max: Vec3::new(-1.73, 1.50, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.73, 1.35, -0.26), max: Vec3::new(2.02, 1.50, 0.26), color: gold_filigree },
        // Pristine mint marble central field
        VoxelBox { min: Vec3::new(-1.75, -1.5, -0.18), max: Vec3::new(1.75, 1.35, 0.18), color: mint_marble },
        // Horizontal gold filigree sightline trim (eye level at y = +0.25)
        VoxelBox { min: Vec3::new(-1.80, 0.20, -0.21), max: Vec3::new(1.80, 0.30, 0.21), color: gold_filigree },
        // Sweeping angular header cornice
        VoxelBox { min: Vec3::new(-1.85, 1.35, -0.22), max: Vec3::new(1.85, 1.50, 0.22), color: lapis },
        VoxelBox { min: Vec3::new(-1.0, 1.45, -0.24), max: Vec3::new(1.0, 1.55, 0.24), color: gold_filigree },
        // Central mint diamond medallion
        VoxelBox { min: Vec3::new(-0.35, -0.10, -0.20), max: Vec3::new(0.35, 0.60, 0.20), color: mint_light },
    ])
}

pub fn create_high_elf_wall_damaged_mesh() -> Mesh {
    let mint_marble = [0.65, 0.84, 0.75, 1.0];
    let mint_fissure = [0.55, 0.72, 0.64, 1.0];
    let gold_broken = [0.80, 0.68, 0.28, 1.0];
    let polished_wood = [0.42, 0.26, 0.15, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-1.75, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(1.75, -1.5, -0.24), max: Vec3::new(2.0, 1.15, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-1.75, -1.5, -0.18), max: Vec3::new(0.50, 1.35, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(0.50, -1.5, -0.18), max: Vec3::new(1.75, -0.20, 0.18), color: mint_fissure },
        VoxelBox { min: Vec3::new(0.70, 0.40, -0.16), max: Vec3::new(1.75, 1.30, 0.16), color: mint_marble },
        VoxelBox { min: Vec3::new(-1.80, 0.20, -0.21), max: Vec3::new(0.40, 0.30, 0.21), color: gold_broken },
        VoxelBox { min: Vec3::new(0.85, 0.20, -0.21), max: Vec3::new(1.80, 0.30, 0.21), color: gold_broken },
        VoxelBox { min: Vec3::new(-1.85, 1.35, -0.22), max: Vec3::new(0.60, 1.50, 0.22), color: lapis },
    ])
}

pub fn create_high_elf_window_mesh() -> Mesh {
    let mint_marble = [0.68, 0.88, 0.78, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];
    let polished_wood = [0.45, 0.28, 0.16, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];
    let glass_tint = [0.60, 0.88, 0.95, 0.75];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-0.95, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.24), max: Vec3::new(2.0, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.18), max: Vec3::new(0.95, -0.32, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(-1.0, -0.36, -0.22), max: Vec3::new(1.0, -0.24, 0.22), color: gold_filigree },
        VoxelBox { min: Vec3::new(-0.95, 0.85, -0.18), max: Vec3::new(0.95, 1.35, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.24), max: Vec3::new(2.0, 1.50, 0.24), color: lapis },
        VoxelBox { min: Vec3::new(-0.03, -0.24, -0.05), max: Vec3::new(0.03, 0.85, 0.05), color: gold_filigree },
        VoxelBox { min: Vec3::new(-0.95, 0.25, -0.05), max: Vec3::new(0.95, 0.31, 0.05), color: gold_filigree },
        VoxelBox { min: Vec3::new(-0.92, -0.22, -0.03), max: Vec3::new(0.92, 0.83, 0.03), color: glass_tint },
    ])
}

pub fn create_high_elf_window_damaged_mesh() -> Mesh {
    let mint_marble = [0.65, 0.84, 0.75, 1.0];
    let gold_broken = [0.80, 0.68, 0.28, 1.0];
    let polished_wood = [0.42, 0.26, 0.15, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-0.95, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.24), max: Vec3::new(2.0, 1.20, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.18), max: Vec3::new(0.30, -0.32, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(-0.95, 0.95, -0.18), max: Vec3::new(0.95, 1.35, 0.18), color: mint_marble },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.24), max: Vec3::new(1.20, 1.50, 0.24), color: lapis },
        VoxelBox { min: Vec3::new(-0.95, 0.25, -0.05), max: Vec3::new(0.20, 0.31, 0.05), color: gold_broken },
    ])
}

pub fn create_high_elf_door_frame_mesh() -> Mesh {
    let polished_wood = [0.45, 0.28, 0.16, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-0.70, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.24), max: Vec3::new(2.0, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-0.76, -1.5, -0.26), max: Vec3::new(-0.68, 1.15, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(0.68, -1.5, -0.26), max: Vec3::new(0.76, 1.15, 0.26), color: gold_filigree },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.22), max: Vec3::new(0.70, 1.50, 0.22), color: lapis },
        VoxelBox { min: Vec3::new(-0.50, 1.30, -0.25), max: Vec3::new(0.50, 1.48, 0.25), color: gold_filigree },
    ])
}

pub fn create_high_elf_door_frame_damaged_mesh() -> Mesh {
    let polished_wood = [0.42, 0.26, 0.15, 1.0];
    let lapis = [0.12, 0.16, 0.32, 1.0];
    let gold_broken = [0.80, 0.68, 0.28, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.24), max: Vec3::new(-0.70, 1.5, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.24), max: Vec3::new(2.0, 1.10, 0.24), color: polished_wood },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.22), max: Vec3::new(0.30, 1.50, 0.22), color: lapis },
        VoxelBox { min: Vec3::new(-0.76, -1.5, -0.26), max: Vec3::new(-0.68, 0.80, 0.26), color: gold_broken },
    ])
}

pub fn create_high_elf_door_leaf_mesh() -> Mesh {
    let lapis = [0.14, 0.18, 0.36, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(0.04, -1.15, -0.06), max: Vec3::new(1.36, 1.15, 0.06), color: lapis },
        VoxelBox { min: Vec3::new(0.08, -1.10, -0.08), max: Vec3::new(1.32, -0.95, 0.08), color: gold_filigree },
        VoxelBox { min: Vec3::new(0.08, 0.95, -0.08), max: Vec3::new(1.32, 1.10, 0.08), color: gold_filigree },
        VoxelBox { min: Vec3::new(0.60, -0.30, -0.08), max: Vec3::new(0.80, 0.30, 0.08), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.15, -0.10, -0.12), max: Vec3::new(1.25, 0.10, 0.12), color: gold_filigree },
    ])
}

pub fn create_high_elf_roof_mesh() -> Mesh {
    let lapis = [0.12, 0.16, 0.32, 1.0];
    let lapis_light = [0.18, 0.24, 0.44, 1.0];
    let gold_ridge = [0.92, 0.78, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.1, -0.12, -2.1), max: Vec3::new(2.1, 0.12, 2.1), color: lapis },
        VoxelBox { min: Vec3::new(-1.8, 0.12, -1.8), max: Vec3::new(1.8, 0.25, 1.8), color: lapis_light },
        VoxelBox { min: Vec3::new(-2.15, 0.22, -0.15), max: Vec3::new(2.15, 0.35, 0.15), color: gold_ridge },
    ])
}

pub fn create_high_elf_foundation_mesh() -> Mesh {
    let mint_marble = [0.68, 0.88, 0.78, 1.0];
    let gold_filigree = [0.92, 0.78, 0.32, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -0.5, -2.0), max: Vec3::new(2.0, 0.5, 2.0), color: mint_marble },
        VoxelBox { min: Vec3::new(-2.05, 0.35, -2.05), max: Vec3::new(-1.70, 0.52, -1.70), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.70, 0.35, -2.05), max: Vec3::new(2.05, 0.52, -1.70), color: gold_filigree },
        VoxelBox { min: Vec3::new(-2.05, 0.35, 1.70), max: Vec3::new(-1.70, 0.52, 2.05), color: gold_filigree },
        VoxelBox { min: Vec3::new(1.70, 0.35, 1.70), max: Vec3::new(2.05, 0.52, 2.05), color: gold_filigree },
    ])
}

// ----------------------------------------------------------------------------
// DARK ELF (THE SUBTERRANEAN SPIRE) PROCEDURAL MODELS
// ----------------------------------------------------------------------------
// Early 2000s MMO aesthetic:
// - Dark indigo, deep purple, and black cavern stone
// - Accented by glowing neon runes (pink, purple, cyan emissive inlays)
// - Heavy, imposing structural pillars with harsh spiked angles

pub fn create_dark_elf_wall_mesh() -> Mesh {
    let cavern_stone = [0.10, 0.08, 0.15, 1.0];
    let basalt_dark = [0.06, 0.05, 0.10, 1.0];
    let obsidian_purple = [0.22, 0.12, 0.28, 1.0];
    let neon_pink = [0.95, 0.15, 0.85, 1.0];
    let neon_cyan = [0.20, 0.80, 1.0, 1.0];
    let spiked_iron = [0.16, 0.15, 0.18, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-1.70, 1.35, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(1.70, -1.5, -0.26), max: Vec3::new(2.0, 1.35, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-1.95, 1.35, -0.22), max: Vec3::new(-1.75, 1.65, 0.22), color: spiked_iron },
        VoxelBox { min: Vec3::new(1.75, 1.35, -0.22), max: Vec3::new(1.95, 1.65, 0.22), color: spiked_iron },
        VoxelBox { min: Vec3::new(-1.70, -1.5, -0.20), max: Vec3::new(1.70, 1.35, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-1.75, -0.35, -0.22), max: Vec3::new(1.75, -0.22, 0.22), color: obsidian_purple },
        VoxelBox { min: Vec3::new(-1.75, 1.25, -0.22), max: Vec3::new(1.75, 1.38, 0.22), color: obsidian_purple },
        VoxelBox { min: Vec3::new(-1.60, 0.22, -0.22), max: Vec3::new(1.60, 0.28, 0.22), color: neon_pink },
        VoxelBox { min: Vec3::new(-0.80, 0.40, -0.22), max: Vec3::new(-0.65, 0.85, 0.22), color: neon_pink },
        VoxelBox { min: Vec3::new(0.65, 0.40, -0.22), max: Vec3::new(0.80, 0.85, 0.22), color: neon_pink },
        VoxelBox { min: Vec3::new(-0.25, 0.45, -0.23), max: Vec3::new(0.25, 0.75, 0.23), color: neon_cyan },
    ])
}

pub fn create_dark_elf_wall_damaged_mesh() -> Mesh {
    let cavern_stone = [0.08, 0.07, 0.12, 1.0];
    let basalt_dark = [0.05, 0.04, 0.08, 1.0];
    let neon_flicker = [0.55, 0.10, 0.50, 1.0];
    let spiked_iron = [0.14, 0.13, 0.16, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-1.70, 1.35, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(1.70, -1.5, -0.26), max: Vec3::new(2.0, 0.95, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-1.95, 1.35, -0.22), max: Vec3::new(-1.75, 1.65, 0.22), color: spiked_iron },
        VoxelBox { min: Vec3::new(-1.70, -1.5, -0.20), max: Vec3::new(0.40, 1.35, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.18), max: Vec3::new(1.70, -0.30, 0.18), color: cavern_stone },
        VoxelBox { min: Vec3::new(-1.60, 0.22, -0.22), max: Vec3::new(0.20, 0.28, 0.22), color: neon_flicker },
    ])
}

pub fn create_dark_elf_window_mesh() -> Mesh {
    let cavern_stone = [0.10, 0.08, 0.15, 1.0];
    let basalt_dark = [0.06, 0.05, 0.10, 1.0];
    let spiked_iron = [0.16, 0.15, 0.18, 1.0];
    let neon_pink = [0.95, 0.15, 0.85, 1.0];
    let glass_purple = [0.45, 0.15, 0.55, 0.70];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-0.95, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.26), max: Vec3::new(2.0, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.20), max: Vec3::new(0.95, -0.32, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-0.95, 0.85, -0.20), max: Vec3::new(0.95, 1.35, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.26), max: Vec3::new(2.0, 1.50, 0.26), color: spiked_iron },
        VoxelBox { min: Vec3::new(-0.04, -0.24, -0.06), max: Vec3::new(0.04, 0.85, 0.06), color: spiked_iron },
        VoxelBox { min: Vec3::new(-0.95, 0.24, -0.06), max: Vec3::new(0.95, 0.32, 0.06), color: spiked_iron },
        VoxelBox { min: Vec3::new(-0.85, -0.30, -0.22), max: Vec3::new(0.85, -0.24, 0.22), color: neon_pink },
        VoxelBox { min: Vec3::new(-0.92, -0.22, -0.03), max: Vec3::new(0.92, 0.83, 0.03), color: glass_purple },
    ])
}

pub fn create_dark_elf_window_damaged_mesh() -> Mesh {
    let cavern_stone = [0.08, 0.07, 0.12, 1.0];
    let basalt_dark = [0.05, 0.04, 0.08, 1.0];
    let spiked_iron = [0.14, 0.13, 0.16, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-0.95, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(0.95, -1.5, -0.26), max: Vec3::new(2.0, 1.15, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-0.95, -1.5, -0.20), max: Vec3::new(0.35, -0.32, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-0.95, 0.95, -0.20), max: Vec3::new(0.95, 1.35, 0.20), color: cavern_stone },
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.26), max: Vec3::new(1.10, 1.50, 0.26), color: spiked_iron },
    ])
}

pub fn create_dark_elf_door_frame_mesh() -> Mesh {
    let basalt_dark = [0.06, 0.05, 0.10, 1.0];
    let spiked_iron = [0.16, 0.15, 0.18, 1.0];
    let neon_purple = [0.75, 0.12, 0.95, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-0.70, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.26), max: Vec3::new(2.0, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.24), max: Vec3::new(0.70, 1.50, 0.24), color: spiked_iron },
        VoxelBox { min: Vec3::new(-0.75, 1.15, -0.26), max: Vec3::new(0.75, 1.25, 0.26), color: neon_purple },
    ])
}

pub fn create_dark_elf_door_frame_damaged_mesh() -> Mesh {
    let basalt_dark = [0.05, 0.04, 0.08, 1.0];
    let spiked_iron = [0.14, 0.13, 0.16, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.26), max: Vec3::new(-0.70, 1.5, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(0.70, -1.5, -0.26), max: Vec3::new(2.0, 1.0, 0.26), color: basalt_dark },
        VoxelBox { min: Vec3::new(-0.70, 1.05, -0.24), max: Vec3::new(0.20, 1.50, 0.24), color: spiked_iron },
    ])
}

pub fn create_dark_elf_door_leaf_mesh() -> Mesh {
    let spiked_iron = [0.18, 0.17, 0.20, 1.0];
    let neon_pink = [0.95, 0.15, 0.85, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(0.04, -1.15, -0.07), max: Vec3::new(1.36, 1.15, 0.07), color: spiked_iron },
        VoxelBox { min: Vec3::new(0.55, -0.40, -0.09), max: Vec3::new(0.85, 0.40, 0.09), color: neon_pink },
        VoxelBox { min: Vec3::new(0.40, -0.10, -0.09), max: Vec3::new(1.00, 0.10, 0.09), color: neon_pink },
    ])
}

pub fn create_dark_elf_roof_mesh() -> Mesh {
    let basalt = [0.08, 0.06, 0.12, 1.0];
    let spiked_iron = [0.16, 0.15, 0.18, 1.0];
    let neon_purple = [0.75, 0.12, 0.95, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -0.10, -2.0), max: Vec3::new(2.0, 0.10, 2.0), color: basalt },
        VoxelBox { min: Vec3::new(-1.8, 0.10, -1.8), max: Vec3::new(1.8, 0.22, 1.8), color: basalt },
        VoxelBox { min: Vec3::new(-2.05, 0.10, -2.05), max: Vec3::new(-1.85, 0.45, -1.85), color: spiked_iron },
        VoxelBox { min: Vec3::new(1.85, 0.10, -2.05), max: Vec3::new(2.05, 0.45, -1.85), color: spiked_iron },
        VoxelBox { min: Vec3::new(-2.05, 0.10, 1.85), max: Vec3::new(-1.85, 0.45, 2.05), color: spiked_iron },
        VoxelBox { min: Vec3::new(1.85, 0.10, 1.85), max: Vec3::new(2.05, 0.45, 2.05), color: spiked_iron },
        VoxelBox { min: Vec3::new(-1.5, 0.22, -0.08), max: Vec3::new(1.5, 0.26, 0.08), color: neon_purple },
    ])
}

pub fn create_dark_elf_foundation_mesh() -> Mesh {
    let basalt = [0.08, 0.06, 0.12, 1.0];
    let neon_pink = [0.95, 0.15, 0.85, 1.0];

    build_voxel_mesh(&[
        VoxelBox { min: Vec3::new(-2.0, -0.5, -2.0), max: Vec3::new(2.0, 0.5, 2.0), color: basalt },
        VoxelBox { min: Vec3::new(-2.02, 0.42, -2.02), max: Vec3::new(2.02, 0.50, -1.94), color: neon_pink },
        VoxelBox { min: Vec3::new(-2.02, 0.42, 1.94), max: Vec3::new(2.02, 0.50, 2.02), color: neon_pink },
    ])
}

// ----------------------------------------------------------------------------
// DATA-DRIVEN BUILDING ASSET MANIFEST & GENERIC RUBBLE POOL RESOURCES
// ----------------------------------------------------------------------------
// Architectural Note:
// 1. DATA-DRIVEN SPAWNING:
//    Instead of separate spawn functions per race (spawn_human_wall, spawn_elf_wall),
//    all faction pieces, colliders, and materials are configured in `BuildingAssetManifest`.
//    A single unified spawning system looks up (Faction, Piece) and instantiates the structure.
//    Adding new factions (e.g. Barbarian Bear-Claw) requires only configuration entries here.
// 2. EVENT-DRIVEN GENERIC RUBBLE POOLS:
//    Destruction does not pre-fracture every unique 3D model. Instead, `GenericRubblePool`
//    holds generic low-poly chunks (cubes, shards, splinters, debris). When any wall breaks,
//    a unified event handler spawns these generic chunks with the destroyed faction's material.

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct BuildingPieceVisuals {
    pub pristine_mesh: Handle<Mesh>,
    pub damaged_mesh: Handle<Mesh>,
    pub collider: Collider,
}

#[derive(Resource, Clone, Debug)]
pub struct GenericRubblePool {
    pub stone_cube: Handle<Mesh>,
    pub brick_block: Handle<Mesh>,
    pub timber_plank: Handle<Mesh>,
    pub geometric_shard_a: Handle<Mesh>,
    pub geometric_shard_b: Handle<Mesh>,
    pub fine_debris: Handle<Mesh>,
    pub sparkle_shard: Handle<Mesh>,
}

#[derive(Resource)]
pub struct BuildingAssetManifest {
    pub pieces: BTreeMap<(BuildingFaction, ModularPieceType), BuildingPieceVisuals>,
    pub door_leaves: BTreeMap<BuildingFaction, Handle<Mesh>>,
    pub faction_materials: BTreeMap<BuildingFaction, Handle<StandardMaterial>>,
    pub blueprint_material: Handle<StandardMaterial>,
    pub glass_pane_mesh: Handle<Mesh>,
    pub glass_material: Handle<StandardMaterial>,
    pub rubble_node_mesh: Handle<Mesh>,
    pub generic_rubble_pool: GenericRubblePool,
}

pub type BuildingMeshCache = BuildingAssetManifest;

impl BuildingAssetManifest {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        // Base procedural / voxel models
        let human_wall_p = meshes.add(create_wall_mesh());
        let human_wall_d = meshes.add(create_damaged_wall_mesh());
        let human_win_p = meshes.add(create_window_mesh());
        let human_win_d = meshes.add(create_damaged_window_mesh());
        let human_df_p = meshes.add(create_door_frame_mesh());
        let human_df_d = meshes.add(create_damaged_door_frame_mesh());
        let human_dl = meshes.add(create_door_leaf_mesh());
        let human_fnd = meshes.add(Cuboid::new(4.0, 1.0, 4.0));
        let human_rf = meshes.add(Cuboid::new(4.0, 0.2, 4.0));

        let high_elf_wall_p = meshes.add(create_high_elf_wall_mesh());
        let high_elf_wall_d = meshes.add(create_high_elf_wall_damaged_mesh());
        let high_elf_win_p = meshes.add(create_high_elf_window_mesh());
        let high_elf_win_d = meshes.add(create_high_elf_window_damaged_mesh());
        let high_elf_df_p = meshes.add(create_high_elf_door_frame_mesh());
        let high_elf_df_d = meshes.add(create_high_elf_door_frame_damaged_mesh());
        let high_elf_dl = meshes.add(create_high_elf_door_leaf_mesh());
        let high_elf_rf = meshes.add(create_high_elf_roof_mesh());
        let high_elf_fnd = meshes.add(create_high_elf_foundation_mesh());

        let dark_elf_wall_p = meshes.add(create_dark_elf_wall_mesh());
        let dark_elf_wall_d = meshes.add(create_dark_elf_wall_damaged_mesh());
        let dark_elf_win_p = meshes.add(create_dark_elf_window_mesh());
        let dark_elf_win_d = meshes.add(create_dark_elf_window_damaged_mesh());
        let dark_elf_df_p = meshes.add(create_dark_elf_door_frame_mesh());
        let dark_elf_df_d = meshes.add(create_dark_elf_door_frame_damaged_mesh());
        let dark_elf_dl = meshes.add(create_dark_elf_door_leaf_mesh());
        let dark_elf_rf = meshes.add(create_dark_elf_roof_mesh());
        let dark_elf_fnd = meshes.add(create_dark_elf_foundation_mesh());

        let floor_mesh = meshes.add(Cuboid::new(4.0, 0.2, 4.0));
        let ramp_mesh = meshes.add(create_ramp_mesh());
        let workbench_mesh = meshes.add(create_workbench_mesh());
        let campfire_mesh = meshes.add(create_campfire_mesh());
        let rubble_node_mesh = meshes.add(create_rubble_mesh());

        let generic_rubble_pool = GenericRubblePool {
            stone_cube: meshes.add(Cuboid::new(0.24, 0.20, 0.24)),
            brick_block: meshes.add(Cuboid::new(0.18, 0.12, 0.28)),
            timber_plank: meshes.add(Cuboid::new(0.09, 0.50, 0.09)),
            geometric_shard_a: meshes.add(Cuboid::new(0.06, 0.32, 0.14)),
            geometric_shard_b: meshes.add(Cuboid::new(0.12, 0.28, 0.06)),
            fine_debris: meshes.add(Cuboid::new(0.14, 0.14, 0.14)),
            sparkle_shard: meshes.add(Cuboid::new(0.05, 0.05, 0.05)),
        };

        // Standard colliders
        let wall_col = Collider::cuboid(4.0, 3.0, 0.4);
        let fnd_col = Collider::cuboid(4.0, 1.0, 4.0);
        let rf_col = Collider::cuboid(4.0, 0.2, 4.0);
        let fl_col = Collider::cuboid(4.0, 0.2, 4.0);
        let ramp_col = Collider::cuboid(4.0, 2.0, 4.0);
        let wb_col = Collider::cuboid(1.6, 1.2, 1.0);
        let cf_col = Collider::cylinder(0.7, 0.35);

        // Window & Door frame colliders
        let win_frame_col = Collider::compound(vec![
            (Vec3::new(-1.475, 0.0, 0.0), Quat::IDENTITY, Collider::cuboid(1.05, 3.0, 0.4)),
            (Vec3::new(1.475, 0.0, 0.0), Quat::IDENTITY, Collider::cuboid(1.05, 3.0, 0.4)),
            (Vec3::new(0.0, -0.925, 0.0), Quat::IDENTITY, Collider::cuboid(1.9, 1.15, 0.4)),
            (Vec3::new(0.0, 1.175, 0.0), Quat::IDENTITY, Collider::cuboid(1.9, 0.65, 0.4)),
        ]);
        let door_frame_col = Collider::compound(vec![
            (Vec3::new(-1.35, 0.0, 0.0), Quat::IDENTITY, Collider::cuboid(1.30, 3.0, 0.4)),
            (Vec3::new(1.35, 0.0, 0.0), Quat::IDENTITY, Collider::cuboid(1.30, 3.0, 0.4)),
            (Vec3::new(0.0, 1.20, 0.0), Quat::IDENTITY, Collider::cuboid(1.40, 0.60, 0.4)),
        ]);

        let mut pieces = BTreeMap::new();

        // 1. Human (Utilitarian Fortress)
        pieces.insert((BuildingFaction::Human, ModularPieceType::Wall), BuildingPieceVisuals { pristine_mesh: human_wall_p.clone(), damaged_mesh: human_wall_d.clone(), collider: wall_col.clone() });
        pieces.insert((BuildingFaction::Human, ModularPieceType::Window), BuildingPieceVisuals { pristine_mesh: human_win_p.clone(), damaged_mesh: human_win_d.clone(), collider: win_frame_col.clone() });
        pieces.insert((BuildingFaction::Human, ModularPieceType::Door), BuildingPieceVisuals { pristine_mesh: human_df_p.clone(), damaged_mesh: human_df_d.clone(), collider: door_frame_col.clone() });
        pieces.insert((BuildingFaction::Human, ModularPieceType::Foundation), BuildingPieceVisuals { pristine_mesh: human_fnd.clone(), damaged_mesh: human_fnd.clone(), collider: fnd_col.clone() });
        pieces.insert((BuildingFaction::Human, ModularPieceType::Roof), BuildingPieceVisuals { pristine_mesh: human_rf.clone(), damaged_mesh: human_rf.clone(), collider: rf_col.clone() });
        pieces.insert((BuildingFaction::Human, ModularPieceType::Floor), BuildingPieceVisuals { pristine_mesh: floor_mesh.clone(), damaged_mesh: floor_mesh.clone(), collider: fl_col.clone() });
        pieces.insert((BuildingFaction::Human, ModularPieceType::Ramp), BuildingPieceVisuals { pristine_mesh: ramp_mesh.clone(), damaged_mesh: ramp_mesh.clone(), collider: ramp_col.clone() });
        pieces.insert((BuildingFaction::Human, ModularPieceType::Workbench), BuildingPieceVisuals { pristine_mesh: workbench_mesh.clone(), damaged_mesh: workbench_mesh.clone(), collider: wb_col.clone() });
        pieces.insert((BuildingFaction::Human, ModularPieceType::Campfire), BuildingPieceVisuals { pristine_mesh: campfire_mesh.clone(), damaged_mesh: campfire_mesh.clone(), collider: cf_col.clone() });

        // 2. High Elf (Pristine Bastion)
        pieces.insert((BuildingFaction::HighElf, ModularPieceType::Wall), BuildingPieceVisuals { pristine_mesh: high_elf_wall_p, damaged_mesh: high_elf_wall_d, collider: wall_col.clone() });
        pieces.insert((BuildingFaction::HighElf, ModularPieceType::Window), BuildingPieceVisuals { pristine_mesh: high_elf_win_p, damaged_mesh: high_elf_win_d, collider: win_frame_col.clone() });
        pieces.insert((BuildingFaction::HighElf, ModularPieceType::Door), BuildingPieceVisuals { pristine_mesh: high_elf_df_p, damaged_mesh: high_elf_df_d, collider: door_frame_col.clone() });
        pieces.insert((BuildingFaction::HighElf, ModularPieceType::Foundation), BuildingPieceVisuals { pristine_mesh: high_elf_fnd, damaged_mesh: human_fnd.clone(), collider: fnd_col.clone() });
        pieces.insert((BuildingFaction::HighElf, ModularPieceType::Roof), BuildingPieceVisuals { pristine_mesh: high_elf_rf, damaged_mesh: human_rf.clone(), collider: rf_col.clone() });
        pieces.insert((BuildingFaction::HighElf, ModularPieceType::Floor), BuildingPieceVisuals { pristine_mesh: floor_mesh.clone(), damaged_mesh: floor_mesh.clone(), collider: fl_col.clone() });
        pieces.insert((BuildingFaction::HighElf, ModularPieceType::Ramp), BuildingPieceVisuals { pristine_mesh: ramp_mesh.clone(), damaged_mesh: ramp_mesh.clone(), collider: ramp_col.clone() });
        pieces.insert((BuildingFaction::HighElf, ModularPieceType::Workbench), BuildingPieceVisuals { pristine_mesh: workbench_mesh.clone(), damaged_mesh: workbench_mesh.clone(), collider: wb_col.clone() });
        pieces.insert((BuildingFaction::HighElf, ModularPieceType::Campfire), BuildingPieceVisuals { pristine_mesh: campfire_mesh.clone(), damaged_mesh: campfire_mesh.clone(), collider: cf_col.clone() });

        // 3. Dark Elf (Subterranean Spire)
        pieces.insert((BuildingFaction::DarkElf, ModularPieceType::Wall), BuildingPieceVisuals { pristine_mesh: dark_elf_wall_p, damaged_mesh: dark_elf_wall_d, collider: wall_col.clone() });
        pieces.insert((BuildingFaction::DarkElf, ModularPieceType::Window), BuildingPieceVisuals { pristine_mesh: dark_elf_win_p, damaged_mesh: dark_elf_win_d, collider: win_frame_col.clone() });
        pieces.insert((BuildingFaction::DarkElf, ModularPieceType::Door), BuildingPieceVisuals { pristine_mesh: dark_elf_df_p, damaged_mesh: dark_elf_df_d, collider: door_frame_col.clone() });
        pieces.insert((BuildingFaction::DarkElf, ModularPieceType::Foundation), BuildingPieceVisuals { pristine_mesh: dark_elf_fnd, damaged_mesh: human_fnd.clone(), collider: fnd_col.clone() });
        pieces.insert((BuildingFaction::DarkElf, ModularPieceType::Roof), BuildingPieceVisuals { pristine_mesh: dark_elf_rf, damaged_mesh: human_rf.clone(), collider: rf_col.clone() });
        pieces.insert((BuildingFaction::DarkElf, ModularPieceType::Floor), BuildingPieceVisuals { pristine_mesh: floor_mesh.clone(), damaged_mesh: floor_mesh.clone(), collider: fl_col.clone() });
        pieces.insert((BuildingFaction::DarkElf, ModularPieceType::Ramp), BuildingPieceVisuals { pristine_mesh: ramp_mesh.clone(), damaged_mesh: ramp_mesh.clone(), collider: ramp_col.clone() });
        pieces.insert((BuildingFaction::DarkElf, ModularPieceType::Workbench), BuildingPieceVisuals { pristine_mesh: workbench_mesh.clone(), damaged_mesh: workbench_mesh.clone(), collider: wb_col.clone() });
        pieces.insert((BuildingFaction::DarkElf, ModularPieceType::Campfire), BuildingPieceVisuals { pristine_mesh: campfire_mesh.clone(), damaged_mesh: campfire_mesh.clone(), collider: cf_col.clone() });

        // 4. Barbarian (Bear-Claw Stronghold fallback to rugged human timber/stone)
        pieces.insert((BuildingFaction::Barbarian, ModularPieceType::Wall), BuildingPieceVisuals { pristine_mesh: human_wall_p.clone(), damaged_mesh: human_wall_d.clone(), collider: wall_col });
        pieces.insert((BuildingFaction::Barbarian, ModularPieceType::Window), BuildingPieceVisuals { pristine_mesh: human_win_p.clone(), damaged_mesh: human_win_d.clone(), collider: win_frame_col });
        pieces.insert((BuildingFaction::Barbarian, ModularPieceType::Door), BuildingPieceVisuals { pristine_mesh: human_df_p.clone(), damaged_mesh: human_df_d.clone(), collider: door_frame_col });
        pieces.insert((BuildingFaction::Barbarian, ModularPieceType::Foundation), BuildingPieceVisuals { pristine_mesh: human_fnd.clone(), damaged_mesh: human_fnd.clone(), collider: fnd_col });
        pieces.insert((BuildingFaction::Barbarian, ModularPieceType::Roof), BuildingPieceVisuals { pristine_mesh: human_rf.clone(), damaged_mesh: human_rf.clone(), collider: rf_col });
        pieces.insert((BuildingFaction::Barbarian, ModularPieceType::Floor), BuildingPieceVisuals { pristine_mesh: floor_mesh, damaged_mesh: human_rf.clone(), collider: fl_col });
        pieces.insert((BuildingFaction::Barbarian, ModularPieceType::Ramp), BuildingPieceVisuals { pristine_mesh: ramp_mesh, damaged_mesh: human_rf.clone(), collider: ramp_col });
        pieces.insert((BuildingFaction::Barbarian, ModularPieceType::Workbench), BuildingPieceVisuals { pristine_mesh: workbench_mesh, damaged_mesh: human_rf.clone(), collider: wb_col });
        pieces.insert((BuildingFaction::Barbarian, ModularPieceType::Campfire), BuildingPieceVisuals { pristine_mesh: campfire_mesh, damaged_mesh: human_rf.clone(), collider: cf_col });

        let mut door_leaves = BTreeMap::new();
        door_leaves.insert(BuildingFaction::Human, human_dl.clone());
        door_leaves.insert(BuildingFaction::HighElf, high_elf_dl);
        door_leaves.insert(BuildingFaction::DarkElf, dark_elf_dl);
        door_leaves.insert(BuildingFaction::Barbarian, human_dl);

        let mut faction_materials = BTreeMap::new();
        faction_materials.insert(BuildingFaction::Human, materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.35,
            ..default()
        }));
        faction_materials.insert(BuildingFaction::HighElf, materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.25,
            reflectance: 0.85,
            ..default()
        }));
        faction_materials.insert(BuildingFaction::DarkElf, materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.90,
            reflectance: 0.45,
            emissive: LinearRgba::new(0.65, 0.10, 0.55, 1.0),
            ..default()
        }));
        faction_materials.insert(BuildingFaction::Barbarian, materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.38, 0.28),
            perceptual_roughness: 0.95,
            reflectance: 0.20,
            ..default()
        }));

        let blueprint_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.6, 1.0, 0.65),
            alpha_mode: AlphaMode::Blend,
            unlit: false,
            ..default()
        });

        let glass_pane_mesh = meshes.add(Cuboid::new(1.9, 1.15, 0.08));
        let glass_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.80, 0.95, 0.35),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            reflectance: 0.8,
            ..default()
        });

        Self {
            pieces,
            door_leaves,
            faction_materials,
            blueprint_material,
            glass_pane_mesh,
            glass_material,
            rubble_node_mesh,
            generic_rubble_pool,
        }
    }

    pub fn get_piece_mesh(
        &self,
        faction: BuildingFaction,
        piece_type: ModularPieceType,
        damage: VisualDamageState,
    ) -> Handle<Mesh> {
        let is_damaged = matches!(damage, VisualDamageState::Damaged);
        if let Some(visuals) = self.pieces.get(&(faction, piece_type)) {
            if is_damaged {
                visuals.damaged_mesh.clone()
            } else {
                visuals.pristine_mesh.clone()
            }
        } else if let Some(fallback) = self.pieces.get(&(BuildingFaction::Human, piece_type)) {
            if is_damaged {
                fallback.damaged_mesh.clone()
            } else {
                fallback.pristine_mesh.clone()
            }
        } else {
            self.pieces.get(&(BuildingFaction::Human, ModularPieceType::Foundation))
                .map(|v| v.pristine_mesh.clone())
                .unwrap_or_default()
        }
    }

    pub fn get_door_leaf_mesh(&self, faction: BuildingFaction) -> Handle<Mesh> {
        self.door_leaves.get(&faction)
            .cloned()
            .unwrap_or_else(|| self.door_leaves[&BuildingFaction::Human].clone())
    }

    pub fn get_material(&self, faction: BuildingFaction, is_blueprint: bool) -> Handle<StandardMaterial> {
        if is_blueprint {
            self.blueprint_material.clone()
        } else {
            self.faction_materials.get(&faction)
                .cloned()
                .unwrap_or_else(|| self.faction_materials[&BuildingFaction::Human].clone())
        }
    }

    pub fn get_collider(&self, faction: BuildingFaction, piece_type: ModularPieceType) -> Collider {
        self.pieces.get(&(faction, piece_type))
            .map(|v| v.collider.clone())
            .unwrap_or_else(|| Collider::cuboid(4.0, 1.0, 4.0))
    }
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

    if build_state.is_active {
        if keys.just_pressed(KeyCode::KeyY) {
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

        if keys.just_pressed(KeyCode::KeyR) {
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

        if keys.just_pressed(KeyCode::KeyT) {
            build_state.selected_faction = match build_state.selected_faction {
                BuildingFaction::Human => BuildingFaction::HighElf,
                BuildingFaction::HighElf => BuildingFaction::DarkElf,
                BuildingFaction::DarkElf => BuildingFaction::Barbarian,
                BuildingFaction::Barbarian => BuildingFaction::Human,
            };
            info!("Selected Architectural Faction: {:?}", build_state.selected_faction);

            for entity in hologram_query.iter() {
                commands.entity(entity).despawn_recursive();
            }
        }

        if keys.just_pressed(KeyCode::KeyQ) {
            build_state.rotation_steps = (build_state.rotation_steps + 1) % 4;
        }
        if keys.just_pressed(KeyCode::KeyE) {
            build_state.rotation_steps = (build_state.rotation_steps.wrapping_sub(1)) % 4;
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
    children_query: Query<&Children>,
    socket_query: Query<(&GlobalTransform, &Socket)>,
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
            let tmpl = template_type.to_template(build_state.selected_faction.into());
            let root_id = commands.spawn((
                SpatialBundle::default(),
                material.clone(),
                BuildHologram,
            )).id();

            for (offset, piece) in &tmpl.blocks {
                let modular_piece = ModularPieceType::from(*piece);
                let mesh = mesh_cache.get_piece_mesh(build_state.selected_faction, modular_piece, VisualDamageState::Pristine);
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
            let mesh = mesh_cache.get_piece_mesh(build_state.selected_faction, build_state.selected_piece, VisualDamageState::Pristine);
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
        ray_dir.into(),
        50.0,
        true,
        filter,
    );

    let mut target_transform = Transform::from_xyz(0.0, 0.0, 0.0);
    let mut target_parent_id = None;
    let mut snapped = false;

    let manual_rotation_offset = Quat::from_rotation_y(build_state.rotation_steps as f32 * std::f32::consts::FRAC_PI_2);

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
                            target_transform.rotation = rotation * manual_rotation_offset; 
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
            let faction_str = build_state.selected_faction.as_prefix();
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
            let piece_name = if build_state.selected_faction == BuildingFaction::Human {
                build_state.selected_piece.name().to_string()
            } else {
                format!("{}_{}", build_state.selected_faction.as_prefix(), build_state.selected_piece.name())
            };

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

// ----------------------------------------------------------------------------
// STRUCTURE SYNCHRONIZATION WITH SELECTIVE PERMEABILITY & DOORS
// ----------------------------------------------------------------------------

pub fn sync_structures(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<BuildingMeshCache>>,
    existing_structures: Query<(Entity, &NetworkStructure, &BuildingFaction, &ModularPieceType, &Transform)>,
    mut destruction_events: EventWriter<BuildingDestructionEvent>,
) {
    let _ = conn.db.frame_tick();
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));

    let db_structures: Vec<_> = conn.db.db.structure().iter().collect();
    let db_ids: BTreeSet<u64> = db_structures.iter().map(|s| s.structure_id).collect();

    // 1. Detect structures removed from SpacetimeDB (e.g. via server hazard sweep, decay, or despawn)
    for (entity, net_struct, faction, piece_type, transform) in existing_structures.iter() {
        if !db_ids.contains(&net_struct.structure_id) {
            destruction_events.send(BuildingDestructionEvent {
                structure_id: net_struct.structure_id,
                faction: *faction,
                piece_type: *piece_type,
                position: transform.translation,
                rotation: transform.rotation,
            });
            commands.entity(entity).despawn_recursive();
        }
    }

    // 2. Track already spawned structures
    let mut spawned_ids = BTreeSet::new();
    for (_entity, net_struct, _fac, _pt, _t) in existing_structures.iter() {
        if db_ids.contains(&net_struct.structure_id) {
            spawned_ids.insert(net_struct.structure_id);
        }
    }

    // 3. Spawn newly added structures through unified data-driven spawner
    for s in db_structures {
        if !spawned_ids.contains(&s.structure_id) {
            let faction = BuildingFaction::from_piece_name(&s.piece_type);
            let base_name = base_piece_name(&s.piece_type);
            let piece_type = match base_name {
                "Foundation" => ModularPieceType::Foundation,
                "Wall" => ModularPieceType::Wall,
                "Window" => ModularPieceType::Window,
                "Door" => ModularPieceType::Door,
                "Floor" => ModularPieceType::Floor,
                "Roof" => ModularPieceType::Roof,
                "Ramp" => ModularPieceType::Ramp,
                "Workbench" => ModularPieceType::Workbench,
                "Campfire" => ModularPieceType::Campfire,
                _ => ModularPieceType::Foundation,
            };

            let transform = Transform::from_xyz(s.x, s.y, s.z)
                .with_rotation(Quat::from_xyzw(s.rot_x, s.rot_y, s.rot_z, s.rot_w));

            let is_open = if piece_type == ModularPieceType::Door {
                conn.db.db.door_state().structure_id().find(&s.structure_id).map_or(false, |d| d.is_open)
            } else {
                false
            };

            spawn_modular_building_entity(
                &mut commands,
                mesh_cache,
                s.structure_id,
                faction,
                piece_type,
                transform,
                s.is_blueprint,
                is_open,
            );
        }
    }
}

/// Unified data-driven spawner for any modular architectural piece across all factions.
pub fn spawn_modular_building_entity(
    commands: &mut Commands,
    manifest: &BuildingAssetManifest,
    structure_id: u64,
    faction: BuildingFaction,
    piece_type: ModularPieceType,
    transform: Transform,
    is_blueprint: bool,
    is_open: bool,
) -> Entity {
    let mesh = manifest.get_piece_mesh(faction, piece_type, VisualDamageState::Pristine);
    let material = manifest.get_material(faction, is_blueprint);
    let collider = manifest.get_collider(faction, piece_type);
    let sockets = piece_type.default_sockets();

    match piece_type {
        ModularPieceType::Window => {
            commands.spawn((
                PbrBundle {
                    mesh,
                    material: material.clone(),
                    transform,
                    ..default()
                },
                RigidBody::Static,
                collider,
                CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
                NetworkStructure { structure_id },
                SpacetimeBuildingId(structure_id),
                BuildingHealth { current: 100, max: 100 },
                VisualDamageState::Pristine,
                piece_type,
                faction,
            )).with_children(|parent| {
                // Central Glass Pane with selective permeability
                parent.spawn((
                    PbrBundle {
                        mesh: manifest.glass_pane_mesh.clone(),
                        material: manifest.glass_material.clone(),
                        transform: Transform::from_xyz(0.0, 0.25, 0.0),
                        ..default()
                    },
                    Collider::cuboid(1.9, 1.15, 0.08),
                    CollisionLayers::new([GameLayer::Glass], [GameLayer::Default, GameLayer::Unit]),
                    GlassPane,
                ));

                for socket in sockets {
                    parent.spawn((
                        SpatialBundle::from_transform(
                            Transform::from_translation(socket.local_offset)
                                      .with_rotation(socket.local_rotation)
                        ),
                        socket,
                    ));
                }
            }).id()
        }

        ModularPieceType::Door => {
            let initial_rotation = if is_open { Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY };
            let leaf_mesh = manifest.get_door_leaf_mesh(faction);

            commands.spawn((
                PbrBundle {
                    mesh,
                    material: material.clone(),
                    transform,
                    ..default()
                },
                RigidBody::Static,
                collider,
                CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
                NetworkStructure { structure_id },
                SpacetimeBuildingId(structure_id),
                BuildingHealth { current: 100, max: 100 },
                VisualDamageState::Pristine,
                piece_type,
                faction,
            )).with_children(|parent| {
                // Kinematic Door Leaf
                parent.spawn((
                    PbrBundle {
                        mesh: leaf_mesh,
                        material: material.clone(),
                        transform: Transform::from_xyz(-0.68, -0.35, 0.0).with_rotation(initial_rotation),
                        ..default()
                    },
                    RigidBody::Kinematic,
                    Collider::compound(vec![
                        (Vec3::new(0.68, 0.0, 0.0), Quat::IDENTITY, Collider::cuboid(1.36, 2.30, 0.14))
                    ]),
                    CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
                    Door {
                        structure_id,
                        target_rotation: initial_rotation,
                        is_open,
                        is_swinging: false,
                    },
                ));

                for socket in sockets {
                    parent.spawn((
                        SpatialBundle::from_transform(
                            Transform::from_translation(socket.local_offset)
                                      .with_rotation(socket.local_rotation)
                        ),
                        socket,
                    ));
                }
            }).id()
        }

        _ => {
            commands.spawn((
                PbrBundle {
                    mesh,
                    material,
                    transform,
                    ..default()
                },
                RigidBody::Static,
                collider,
                CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
                NetworkStructure { structure_id },
                SpacetimeBuildingId(structure_id),
                BuildingHealth { current: 100, max: 100 },
                VisualDamageState::Pristine,
                piece_type,
                faction,
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
            }).id()
        }
    }
}

pub fn spawn_modular_building_system(
    mut commands: Commands,
    mut events: EventReader<SpawnBuildingEvent>,
    manifest: Res<BuildingAssetManifest>,
) {
    for ev in events.read() {
        let transform = Transform::from_translation(ev.position).with_rotation(ev.rotation);
        spawn_modular_building_entity(
            &mut commands,
            &manifest,
            ev.entity_id,
            ev.faction,
            ev.piece,
            transform,
            ev.is_blueprint,
            false,
        );
    }
}

// ----------------------------------------------------------------------------
// MODEL SWAPPING FOR LOW-POLY DESTRUCTION
// ----------------------------------------------------------------------------
// Architectural Note:
// The client does not authoritatively calculate building health; it subscribes
// to SpacetimeDB table updates. When health crosses the 50% threshold, this system
// seamlessly swaps the Mesh handle between Pristine and Damaged variants, triggering
// local dust particles. When current_health reaches 0, it dispatches BuildingDestructionEvent
// and despawns the structure.

pub fn update_building_destruction_visuals(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    mut query: Query<(
        Entity,
        &SpacetimeBuildingId,
        &mut Handle<Mesh>,
        &mut VisualDamageState,
        &BuildingFaction,
        &ModularPieceType,
        &Transform,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut destruction_events: EventWriter<BuildingDestructionEvent>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));

    for (entity, st_id, mut mesh_handle, mut state, faction, piece_type, transform) in query.iter_mut() {
        if let Some(st_building) = conn.db.db.structure().structure_id().find(&st_id.0) {
            let max_hp = st_building.max_health.max(1.0);
            let hp_percent = st_building.current_health / max_hp;

            if st_building.current_health <= 0.0 {
                // Building destroyed by health depletion
                destruction_events.send(BuildingDestructionEvent {
                    structure_id: st_id.0,
                    faction: *faction,
                    piece_type: *piece_type,
                    position: transform.translation,
                    rotation: transform.rotation,
                });
                commands.entity(entity).despawn_recursive();
                continue;
            }

            if hp_percent <= 0.5 && matches!(*state, VisualDamageState::Pristine) {
                let damaged = mesh_cache.get_piece_mesh(*faction, *piece_type, VisualDamageState::Damaged);
                *mesh_handle = damaged;
                *state = VisualDamageState::Damaged;

                // Faction-specific structural damage particles
                match faction {
                    BuildingFaction::HighElf => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            transform.translation + Vec3::Y * 1.5,
                            14,
                            Color::srgb(0.75, 0.95, 0.88),
                            Color::srgb(0.18, 0.25, 0.45),
                            0.07,
                        );
                    }
                    BuildingFaction::Human => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            transform.translation + Vec3::Y * 1.5,
                            14,
                            Color::srgb(0.50, 0.48, 0.45),
                            Color::srgb(0.42, 0.28, 0.16),
                            0.09,
                        );
                    }
                    BuildingFaction::DarkElf => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            transform.translation + Vec3::Y * 1.5,
                            14,
                            Color::srgb(0.12, 0.10, 0.18),
                            Color::srgb(0.85, 0.15, 0.75),
                            0.08,
                        );
                    }
                    BuildingFaction::Barbarian => {
                        crate::terrain::spawn_voxel_gibs(
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            transform.translation + Vec3::Y * 1.5,
                            14,
                            Color::srgb(0.48, 0.38, 0.28),
                            Color::srgb(0.35, 0.25, 0.15),
                            0.09,
                        );
                    }
                }
            } else if hp_percent > 0.5 && matches!(*state, VisualDamageState::Damaged) {
                let pristine = mesh_cache.get_piece_mesh(*faction, *piece_type, VisualDamageState::Pristine);
                *mesh_handle = pristine;
                *state = VisualDamageState::Pristine;
            }
        }
    }
}

// ----------------------------------------------------------------------------
// KINEMATIC DOOR ROTATION INTERPOLATION
// ----------------------------------------------------------------------------
// Architectural Note:
// Listens to SpacetimeDB door_state table. Slerps the Kinematic door leaf
// rotation smoothly towards target_rotation. Prevents network jitter while
// keeping Avian3D's kinematic collider perfectly synchronized with the mesh.

pub fn sync_door_states(
    conn: Res<SpacetimeConnection>,
    mut query: Query<&mut Door>,
) {
    for mut door in query.iter_mut() {
        let is_open = conn.db.db.door_state().structure_id().find(&door.structure_id).map_or(false, |d| d.is_open);
        if is_open != door.is_open {
            door.is_open = is_open;
            door.target_rotation = if is_open {
                Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
            } else {
                Quat::IDENTITY
            };
            door.is_swinging = true;
        }
    }
}

pub fn animate_doors(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Door)>,
) {
    let dt = time.delta_seconds();
    for (mut transform, mut door) in query.iter_mut() {
        if door.is_swinging {
            transform.rotation = transform.rotation.slerp(door.target_rotation, dt * 6.0);
            if transform.rotation.angle_between(door.target_rotation) < 0.01 {
                transform.rotation = door.target_rotation;
                door.is_swinging = false;
            }
        }
    }
}

// ----------------------------------------------------------------------------
// LETHAL FALLS & HARVESTABLE RUIN SPAWNING (THE ILLUSION OF PHYSICS)
// ----------------------------------------------------------------------------
// Architectural Note:
// Spawns a permanent static ruin pile (rubble / fallen timber) that blocks player
// movement and provides harvestable mining yields.

pub fn spawn_ruin_pile(
    commands: &mut Commands,
    mesh_cache: &BuildingMeshCache,
    material: Handle<StandardMaterial>,
    position: Vec3,
    yield_amount: u32,
    ruin_type: &str,
) -> Entity {
    commands.spawn((
        PbrBundle {
            mesh: mesh_cache.rubble_node_mesh.clone(),
            material,
            transform: Transform::from_translation(position),
            ..default()
        },
        Collider::cuboid(2.0, 1.0, 2.0),
        RigidBody::Static,
        CollisionLayers::new([GameLayer::Environment], [GameLayer::Default, GameLayer::Unit]),
        HarvestableRuin {
            yield_amount,
            node_type: ruin_type.to_string(),
        },
    )).id()
}

pub fn sync_fall_hazards(
    mut commands: Commands,
    conn: Res<SpacetimeConnection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing_hazards: Query<&VisualFallHazard>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));
    let mut known = BTreeSet::new();
    for h in existing_hazards.iter() {
        known.insert(h.hazard_id);
    }

    for h in conn.db.db.fall_hazard().iter() {
        if !known.contains(&h.hazard_id) {
            let origin = Vec3::new(h.origin_x, h.origin_y, h.origin_z);
            let dir = Vec3::new(h.dir_x, 0.0, h.dir_z).normalize_or_zero();

            let mesh = match h.kind.as_str() {
                "CollapsingTower" => mesh_cache.get_piece_mesh(BuildingFaction::Human, ModularPieceType::Wall, VisualDamageState::Damaged),
                _ => mesh_cache.rubble_node_mesh.clone(),
            };

            let material = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.85,
                ..default()
            });

            commands.spawn((
                PbrBundle {
                    mesh,
                    material,
                    transform: Transform::from_translation(origin),
                    ..default()
                },
                VisualFallHazard {
                    hazard_id: h.hazard_id,
                    kind: h.kind.clone(),
                    origin,
                    dir,
                    length: h.length,
                    elapsed: 0.0,
                    duration: (h.duration_ms as f32) / 1000.0,
                    initial_rotation: Quat::IDENTITY,
                },
            ));
        }
    }
}

pub fn update_fall_hazards(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut VisualFallHazard, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));
    let dt = time.delta_seconds().min(0.1);
    for (entity, mut hazard, mut transform) in query.iter_mut() {
        hazard.elapsed += dt;
        let progress = (hazard.elapsed / hazard.duration.max(0.1)).min(1.0);

        // Smooth visual topple curve (0 to 90 degrees)
        let angle = progress * std::f32::consts::FRAC_PI_2;
        let tilt_axis = Vec3::new(hazard.dir.z, 0.0, -hazard.dir.x).normalize_or_zero();
        let rot = Quat::from_axis_angle(tilt_axis, angle);

        transform.rotation = rot * hazard.initial_rotation;

        if progress >= 1.0 {
            let impact_pos = hazard.origin + hazard.dir * (hazard.length * 0.5);

            // Impact dust cloud
            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                hazard.origin + hazard.dir * (hazard.length * 0.4),
                28,
                Color::srgb(0.48, 0.46, 0.44),
                Color::srgb(0.60, 0.50, 0.40),
                0.12,
            );

            // Spawn the permanent harvestable ruin pile
            let ruin_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.55, 0.48, 0.42),
                perceptual_roughness: 0.9,
                ..default()
            });

            let (ruin_type, yield_amt) = if hazard.kind == "FellingTree" {
                ("FallenLog", 50)
            } else {
                ("Rubble", 75)
            };

            spawn_ruin_pile(&mut commands, mesh_cache, ruin_mat, impact_pos, yield_amt, ruin_type);

            commands.entity(entity).despawn_recursive();
        }
    }
}

// ----------------------------------------------------------------------------
// FACTION DESTRUCTION ANIMATIONS & SHARD GENERATION
// ----------------------------------------------------------------------------
// Architectural Note:
// Early 2000s MMO Architectural Style Destruction Profiles:
// 1. High Elf (The Pristine Bastion): Shatters into clean geometric shards
//    (glass-like break patterns) with high angular velocity, mint marble & lapis colors.
// 2. Human (The Utilitarian Fortress): Heavy blunt rubble cubes, splintered timber
//    planks, and thick billowing dust clouds.
// 3. Dark Elf (The Subterranean Spire): Dark jagged obsidian chunks and violently
//    bursting neon runes that flicker out via RuneLightDecay.

pub fn handle_building_destruction(
    mut commands: Commands,
    mut events: EventReader<BuildingDestructionEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));

    for event in events.read() {
        info!("Handling building destruction for {:?} {:?} at {:?}", event.faction, event.piece_type, event.position);

        let damaged_mesh = mesh_cache.get_piece_mesh(event.faction, event.piece_type, VisualDamageState::Damaged);
        let mat = mesh_cache.get_material(event.faction, false);

        // 1. Ephemeral collapse entity that shudders, tilts, and sinks into the ground
        commands.spawn((
            PbrBundle {
                mesh: damaged_mesh,
                material: mat,
                transform: Transform::from_translation(event.position).with_rotation(event.rotation),
                ..default()
            },
            BuildingDestructionAnimation {
                faction: event.faction,
                piece_type: event.piece_type,
                elapsed: 0.0,
                duration: 1.4,
                origin: event.position,
            },
        ));

        // 2. Faction-specific debris & particle effects via generic rubble pool
        spawn_faction_destruction_fx(&mut commands, mesh_cache, &mut meshes, &mut materials, event.position, event.faction);
    }
}

/// Event-driven gibs and destruction FX pipeline using the pre-cached GenericRubblePool.
/// Decouples visual destruction from wall geometry by applying faction materials to generic low-poly chunks.
pub fn spawn_faction_destruction_fx(
    commands: &mut Commands,
    manifest: &BuildingAssetManifest,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    origin: Vec3,
    faction: BuildingFaction,
) {
    let mut rng_seed = (origin.x.abs() * 1000.0 + origin.z.abs() * 100.0) as u64;
    let pool = &manifest.generic_rubble_pool;
    let faction_mat = manifest.get_material(faction, false);

    match faction {
        BuildingFaction::HighElf => {
            // High Elf (The Pristine Bastion): Shatters into clean geometric shards
            // (glass-like break patterns) with high angular velocity, mint marble & lapis accents.
            let mat_lapis = materials.add(StandardMaterial {
                base_color: Color::srgb(0.16, 0.22, 0.45),
                perceptual_roughness: 0.3,
                metallic: 0.2,
                ..default()
            });
            let mat_sparkle = materials.add(StandardMaterial {
                base_color: Color::srgb(0.85, 1.0, 0.95),
                unlit: true,
                ..default()
            });

            // Geometric shards from generic rubble pool
            for i in 0..22 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 40) as f32 / 10.0 + 1.2;

                let mat = if i % 2 == 0 { faction_mat.clone() } else { mat_lapis.clone() };
                let mesh = if i % 2 == 0 { pool.geometric_shard_a.clone() } else { pool.geometric_shard_b.clone() };
                let offset = Vec3::new(rx * 0.35, (i as f32 * 0.06).min(1.2), rz * 0.35);

                commands.spawn((
                    PbrBundle {
                        mesh,
                        material: mat,
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.4 + (i as f32 * 0.03), TimerMode::Once),
                        velocity: Vec3::new(rx * 6.5, ry * 1.5, rz * 6.5),
                        angular_velocity: Vec3::new(rx * 26.0, ry * 18.0, rz * 26.0),
                    },
                ));
            }

            // Crystalline sparkles from generic rubble pool
            for i in 0..8 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;

                commands.spawn((
                    PbrBundle {
                        mesh: pool.sparkle_shard.clone(),
                        material: mat_sparkle.clone(),
                        transform: Transform::from_translation(origin + Vec3::new(rx * 0.5, 1.0 + (i as f32 * 0.1), rz * 0.5)),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(0.9 + (i as f32 * 0.04), TimerMode::Once),
                        velocity: Vec3::new(rx * 3.5, 3.5, rz * 3.5),
                        angular_velocity: Vec3::new(rx * 12.0, 10.0, rz * 12.0),
                    },
                ));
            }
        }

        BuildingFaction::Human => {
            // Human (The Utilitarian Fortress): Heavy blunt rubble cubes, splintered timber
            // planks, and thick billowing dust clouds.
            for i in 0..16 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 30) as f32 / 10.0 + 1.0;

                let mesh = if i % 2 == 0 { pool.stone_cube.clone() } else { pool.brick_block.clone() };
                let offset = Vec3::new(rx * 0.3, (i as f32 * 0.05).min(0.8), rz * 0.3);

                commands.spawn((
                    PbrBundle {
                        mesh,
                        material: faction_mat.clone(),
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.3 + (i as f32 * 0.03), TimerMode::Once),
                        velocity: Vec3::new(rx * 4.2, ry, rz * 4.2),
                        angular_velocity: Vec3::new(rx * 12.0, ry * 6.0, rz * 12.0),
                    },
                ));
            }

            // Splintered timber planks from generic rubble pool
            for i in 0..8 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 35) as f32 / 10.0 + 1.5;

                let offset = Vec3::new(rx * 0.25, 0.4 + (i as f32 * 0.08), rz * 0.25);
                commands.spawn((
                    PbrBundle {
                        mesh: pool.timber_plank.clone(),
                        material: faction_mat.clone(),
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.5 + (i as f32 * 0.04), TimerMode::Once),
                        velocity: Vec3::new(rx * 4.8, ry, rz * 4.8),
                        angular_velocity: Vec3::new(rx * 16.0, ry * 10.0, rz * 16.0),
                    },
                ));
            }

            // Billowing mortar dust clouds
            crate::terrain::spawn_voxel_gibs(
                commands,
                meshes,
                materials,
                origin + Vec3::Y * 0.5,
                20,
                Color::srgb(0.66, 0.63, 0.58),
                Color::srgb(0.48, 0.46, 0.44),
                0.14,
            );
        }

        BuildingFaction::DarkElf => {
            // Dark Elf (The Subterranean Spire): Dark jagged obsidian chunks and violently
            // bursting neon runes that flicker out via RuneLightDecay.
            let mat_neon_pink = materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.15, 0.85),
                unlit: true,
                ..default()
            });
            let mat_neon_cyan = materials.add(StandardMaterial {
                base_color: Color::srgb(0.15, 0.85, 0.95),
                unlit: true,
                ..default()
            });

            // Obsidian chunks from generic rubble pool
            for i in 0..16 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 35) as f32 / 10.0 + 1.2;

                let mesh = if i % 2 == 0 { pool.stone_cube.clone() } else { pool.geometric_shard_a.clone() };
                let offset = Vec3::new(rx * 0.3, (i as f32 * 0.06).min(1.0), rz * 0.3);

                commands.spawn((
                    PbrBundle {
                        mesh,
                        material: faction_mat.clone(),
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.4 + (i as f32 * 0.03), TimerMode::Once),
                        velocity: Vec3::new(rx * 5.0, ry, rz * 5.0),
                        angular_velocity: Vec3::new(rx * 18.0, ry * 12.0, rz * 18.0),
                    },
                ));
            }

            // Violently bursting neon rune sparks from generic rubble pool
            for i in 0..14 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 45) as f32 / 10.0 + 1.8;

                let mat = if i % 2 == 0 { mat_neon_pink.clone() } else { mat_neon_cyan.clone() };
                let offset = Vec3::new(rx * 0.25, 0.5 + (i as f32 * 0.05), rz * 0.25);

                commands.spawn((
                    PbrBundle {
                        mesh: pool.sparkle_shard.clone(),
                        material: mat,
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.1 + (i as f32 * 0.04), TimerMode::Once),
                        velocity: Vec3::new(rx * 7.5, ry, rz * 7.5),
                        angular_velocity: Vec3::new(rx * 22.0, ry * 16.0, rz * 22.0),
                    },
                ));
            }

            // Bursting & flickering neon rune point light
            commands.spawn((
                PointLightBundle {
                    point_light: PointLight {
                        color: Color::srgb(0.92, 0.20, 0.85),
                        intensity: 45000.0,
                        range: 16.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    transform: Transform::from_translation(origin + Vec3::Y * 1.5),
                    ..default()
                },
                RuneLightDecay {
                    timer: Timer::from_seconds(1.4, TimerMode::Once),
                    base_intensity: 45000.0,
                },
            ));
        }

        BuildingFaction::Barbarian => {
            // Barbarian (Bear-Claw Stronghold): Splintered timber beams, rough rock, wild dust.
            for i in 0..14 {
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rx = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let rz = ((rng_seed >> 32) as i32 % 100) as f32 / 50.0 - 1.0;
                rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let ry = ((rng_seed >> 32) as u32 % 35) as f32 / 10.0 + 1.2;

                let mesh = match i % 3 {
                    0 => pool.timber_plank.clone(),
                    1 => pool.stone_cube.clone(),
                    _ => pool.fine_debris.clone(),
                };
                let offset = Vec3::new(rx * 0.35, (i as f32 * 0.07).min(1.0), rz * 0.35);

                commands.spawn((
                    PbrBundle {
                        mesh,
                        material: faction_mat.clone(),
                        transform: Transform::from_translation(origin + offset),
                        ..default()
                    },
                    VoxelGib {
                        timer: Timer::from_seconds(1.5 + (i as f32 * 0.03), TimerMode::Once),
                        velocity: Vec3::new(rx * 5.2, ry, rz * 5.2),
                        angular_velocity: Vec3::new(rx * 15.0, ry * 8.0, rz * 15.0),
                    },
                ));
            }

            // Rugged forest/earth dust burst
            crate::terrain::spawn_voxel_gibs(
                commands,
                meshes,
                materials,
                origin + Vec3::Y * 0.5,
                18,
                Color::srgb(0.48, 0.38, 0.28),
                Color::srgb(0.35, 0.25, 0.15),
                0.12,
            );
        }
    }
}

pub fn update_building_destruction_animations(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut BuildingDestructionAnimation, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<BuildingMeshCache>>,
) {
    let mesh_cache = cache.get_or_insert_with(|| BuildingMeshCache::new(&mut meshes, &mut materials));
    let dt = time.delta_seconds().min(0.1);

    for (entity, mut anim, mut transform) in query.iter_mut() {
        anim.elapsed += dt;
        let progress = (anim.elapsed / anim.duration.max(0.1)).min(1.0);

        // Structural shudder vibration: high frequency shake that dampens slightly
        let shake = (anim.elapsed * 40.0).sin() * 0.04 * (1.0 - progress * 0.5);
        let sway = (anim.elapsed * 25.0).cos() * 0.03 * (1.0 - progress * 0.5);

        // Collapse sinking along Y axis
        let sink_y = progress * 1.2;
        transform.translation = anim.origin + Vec3::new(shake, -sink_y, sway);

        // Subtle structural failure tilt
        transform.rotate_local_x(0.18 * dt);

        if progress >= 1.0 {
            // Sinking collapse complete: spawn permanent static HarvestableRuin node
            let (ruin_type, yield_amt, ruin_color) = match anim.faction {
                BuildingFaction::HighElf => ("MintMarbleShards", 50, Color::srgb(0.70, 0.90, 0.84)),
                BuildingFaction::Human => ("Rubble", 60, Color::srgb(0.50, 0.48, 0.45)),
                BuildingFaction::DarkElf => ("ObsidianVein", 50, Color::srgb(0.15, 0.12, 0.22)),
                BuildingFaction::Barbarian => ("TimberStoneRubble", 60, Color::srgb(0.48, 0.38, 0.28)),
            };

            let ruin_mat = materials.add(StandardMaterial {
                base_color: ruin_color,
                perceptual_roughness: 0.9,
                ..default()
            });

            // Permanent static ruin pile that blocks movement and provides harvestable yields
            spawn_ruin_pile(&mut commands, mesh_cache, ruin_mat, anim.origin + Vec3::new(0.0, 0.2, 0.0), yield_amt, ruin_type);

            // Ground impact dust burst
            crate::terrain::spawn_voxel_gibs(
                &mut commands,
                &mut meshes,
                &mut materials,
                anim.origin + Vec3::Y * 0.3,
                16,
                ruin_color,
                Color::srgb(0.35, 0.35, 0.35),
                0.10,
            );

            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn update_rune_light_decay(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut PointLight, &mut RuneLightDecay)>,
) {
    for (entity, mut light, mut decay) in query.iter_mut() {
        if decay.timer.tick(time.delta()).just_finished() {
            commands.entity(entity).despawn_recursive();
        } else {
            let frac = 1.0 - decay.timer.fraction();
            let flicker = 0.65 + 0.35 * (decay.timer.elapsed_secs() * 32.0).sin();
            light.intensity = decay.base_intensity * frac * flicker;
        }
    }
}