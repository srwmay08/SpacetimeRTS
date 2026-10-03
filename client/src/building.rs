use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use avian3d::prelude::*;
use tracing::info;
use spacetimedb_sdk::Table;

use crate::components::*;
use crate::network::{SpacetimeConnection, VoxelBox, build_voxel_mesh};
use crate::module_bindings::place_structure_reducer::place_structure; 
use crate::module_bindings::structure_table::StructureTableAccess; 

// ----------------------------------------------------------------------------
// DATA-DRIVEN CONFIGURATIONS
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModularPieceType {
    Foundation,
    Workbench,
    Campfire,
    Wall,
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
            Self::Wall => vec![
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
    pub rotation_steps: u8,
}

impl Default for BuildModeState {
    fn default() -> Self {
        Self {
            is_active: false,
            selected_piece: ModularPieceType::Foundation,
            rotation_steps: 0,
        }
    }
}

// ----------------------------------------------------------------------------
// PROCEDURAL MESH GENERATION
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

pub fn create_catapult_mesh() -> Mesh {
    let wood_dark = [0.35, 0.22, 0.12, 1.0];
    let wood_light = [0.52, 0.35, 0.20, 1.0];
    let iron = [0.25, 0.26, 0.28, 1.0];
    let rope = [0.65, 0.52, 0.28, 1.0];
    let stone = [0.50, 0.48, 0.46, 1.0];

    build_voxel_mesh(&[
        // Left & right oak chassis rails
        VoxelBox { min: Vec3::new(-0.85, 0.20, -1.50), max: Vec3::new(-0.65, 0.45, 1.50), color: wood_dark },
        VoxelBox { min: Vec3::new(0.65, 0.20, -1.50), max: Vec3::new(0.85, 0.45, 1.50), color: wood_dark },
        // Chassis cross-beams
        VoxelBox { min: Vec3::new(-0.65, 0.22, -1.35), max: Vec3::new(0.65, 0.42, -1.15), color: wood_light },
        VoxelBox { min: Vec3::new(-0.65, 0.22, -0.30), max: Vec3::new(0.65, 0.42, -0.10), color: wood_light },
        VoxelBox { min: Vec3::new(-0.65, 0.22, 1.15), max: Vec3::new(0.65, 0.42, 1.35), color: wood_light },
        // 4 Spoke Timber Wheels with Iron Rims
        VoxelBox { min: Vec3::new(-1.05, 0.0, -1.25), max: Vec3::new(-0.88, 0.65, -0.85), color: iron },
        VoxelBox { min: Vec3::new(0.88, 0.0, -1.25), max: Vec3::new(1.05, 0.65, -0.85), color: iron },
        VoxelBox { min: Vec3::new(-1.05, 0.0, 0.85), max: Vec3::new(-0.88, 0.65, 1.25), color: iron },
        VoxelBox { min: Vec3::new(0.88, 0.0, 0.85), max: Vec3::new(1.05, 0.65, 1.25), color: iron },
        // Wheel Hub Axles
        VoxelBox { min: Vec3::new(-1.12, 0.25, -1.10), max: Vec3::new(1.12, 0.40, -1.00), color: wood_dark },
        VoxelBox { min: Vec3::new(-1.12, 0.25, 1.00), max: Vec3::new(1.12, 0.40, 1.10), color: wood_dark },
        // Vertical Upright Stanchions
        VoxelBox { min: Vec3::new(-0.82, 0.45, -0.35), max: Vec3::new(-0.65, 1.85, -0.15), color: wood_dark },
        VoxelBox { min: Vec3::new(0.65, 0.45, -0.35), max: Vec3::new(0.82, 1.85, -0.15), color: wood_dark },
        // Diagonal Braces
        VoxelBox { min: Vec3::new(-0.80, 0.45, -0.95), max: Vec3::new(-0.68, 1.35, -0.35), color: wood_light },
        VoxelBox { min: Vec3::new(0.68, 0.45, -0.95), max: Vec3::new(0.80, 1.35, -0.35), color: wood_light },
        // Torsion Rope Skein Cross-Bundle
        VoxelBox { min: Vec3::new(-0.65, 0.50, -0.32), max: Vec3::new(0.65, 0.80, -0.18), color: rope },
        // Iron Ratchet Winch
        VoxelBox { min: Vec3::new(-0.95, 0.52, -0.30), max: Vec3::new(-0.82, 0.78, -0.20), color: iron },
        VoxelBox { min: Vec3::new(0.82, 0.52, -0.30), max: Vec3::new(0.95, 0.78, -0.20), color: iron },
        // Throwing Arm (Lever)
        VoxelBox { min: Vec3::new(-0.12, 0.55, -0.30), max: Vec3::new(0.12, 1.10, 1.35), color: wood_light },
        // Payload Cup & Catapult Boulder
        VoxelBox { min: Vec3::new(-0.25, 0.95, 1.20), max: Vec3::new(0.25, 1.25, 1.55), color: wood_dark },
        VoxelBox { min: Vec3::new(-0.18, 1.18, 1.25), max: Vec3::new(0.18, 1.50, 1.50), color: stone },
        // Padded Crossbar Stop Bumper
        VoxelBox { min: Vec3::new(-0.65, 1.60, -0.32), max: Vec3::new(0.65, 1.82, -0.18), color: iron },
    ])
}

pub fn create_trebuchet_mesh() -> Mesh {
    let timber = [0.38, 0.24, 0.14, 1.0];
    let timber_light = [0.55, 0.38, 0.22, 1.0];
    let iron = [0.26, 0.27, 0.28, 1.0];
    let stone = [0.48, 0.46, 0.45, 1.0];
    let rope = [0.70, 0.58, 0.32, 1.0];

    build_voxel_mesh(&[
        // Ground Skid Base Rails
        VoxelBox { min: Vec3::new(-1.30, 0.0, -1.80), max: Vec3::new(-1.05, 0.28, 1.80), color: timber },
        VoxelBox { min: Vec3::new(1.05, 0.0, -1.80), max: Vec3::new(1.30, 0.28, 1.80), color: timber },
        VoxelBox { min: Vec3::new(-1.05, 0.0, -1.60), max: Vec3::new(1.05, 0.25, -1.35), color: timber },
        VoxelBox { min: Vec3::new(-1.05, 0.0, 1.35), max: Vec3::new(1.05, 0.25, 1.60), color: timber },
        // Left A-Frame Scaffold (angled timbers)
        VoxelBox { min: Vec3::new(-1.25, 0.25, -0.15), max: Vec3::new(-1.05, 3.20, 0.15), color: timber },
        VoxelBox { min: Vec3::new(-1.22, 0.25, -1.40), max: Vec3::new(-1.08, 3.10, -0.10), color: timber_light },
        VoxelBox { min: Vec3::new(-1.22, 0.25, 0.10), max: Vec3::new(-1.08, 3.10, 1.40), color: timber_light },
        // Right A-Frame Scaffold
        VoxelBox { min: Vec3::new(1.05, 0.25, -0.15), max: Vec3::new(1.25, 3.20, 0.15), color: timber },
        VoxelBox { min: Vec3::new(1.08, 0.25, -1.40), max: Vec3::new(1.22, 3.10, -0.10), color: timber_light },
        VoxelBox { min: Vec3::new(1.08, 0.25, 0.10), max: Vec3::new(1.22, 3.10, 1.40), color: timber_light },
        // Heavy Pivot Axle on Bronze/Iron Bearings
        VoxelBox { min: Vec3::new(-1.35, 3.00, -0.12), max: Vec3::new(1.35, 3.22, 0.12), color: iron },
        // Asymmetric Swing Beam
        VoxelBox { min: Vec3::new(-0.16, 2.70, -1.20), max: Vec3::new(0.16, 3.35, 2.80), color: timber_light },
        // Counterweight Box (Short arm, filled with stones)
        VoxelBox { min: Vec3::new(-0.55, 1.60, -1.45), max: Vec3::new(0.55, 2.80, -0.95), color: timber },
        VoxelBox { min: Vec3::new(-0.48, 1.70, -1.38), max: Vec3::new(0.48, 2.75, -1.02), color: stone },
        // Sling Line & Pouch (Long arm)
        VoxelBox { min: Vec3::new(-0.04, 0.60, 2.60), max: Vec3::new(0.04, 2.80, 2.68), color: rope },
        // Heavy Fortified Shell in Sling
        VoxelBox { min: Vec3::new(-0.25, 0.40, 2.50), max: Vec3::new(0.25, 0.85, 2.95), color: stone },
    ])
}

pub fn create_ballista_mesh() -> Mesh {
    let dark_wood = [0.36, 0.22, 0.12, 1.0];
    let light_wood = [0.54, 0.36, 0.20, 1.0];
    let iron = [0.28, 0.29, 0.30, 1.0];
    let bronze = [0.72, 0.58, 0.24, 1.0];
    let red_mat = [0.85, 0.15, 0.15, 1.0];

    build_voxel_mesh(&[
        // Tripod Support Legs
        VoxelBox { min: Vec3::new(-0.65, 0.0, -0.65), max: Vec3::new(-0.45, 0.85, -0.45), color: dark_wood },
        VoxelBox { min: Vec3::new(0.45, 0.0, -0.65), max: Vec3::new(0.65, 0.85, -0.45), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.12, 0.0, 0.55), max: Vec3::new(0.12, 0.85, 0.75), color: dark_wood },
        // Central Swivel Pedestal & Elevating Arc
        VoxelBox { min: Vec3::new(-0.25, 0.80, -0.25), max: Vec3::new(0.25, 1.15, 0.25), color: iron },
        VoxelBox { min: Vec3::new(-0.15, 1.15, -0.15), max: Vec3::new(0.15, 1.35, 0.15), color: bronze },
        // Longitudinal Flight Rail Track
        VoxelBox { min: Vec3::new(-0.16, 1.25, -1.40), max: Vec3::new(0.16, 1.45, 1.20), color: dark_wood },
        // Flight Trough Guide Groove
        VoxelBox { min: Vec3::new(-0.06, 1.45, -1.35), max: Vec3::new(0.06, 1.48, 1.15), color: iron },
        // Front Bow Table & Dual Torsion Skein Cylinders
        VoxelBox { min: Vec3::new(-0.95, 1.15, -1.45), max: Vec3::new(0.95, 1.55, -1.25), color: dark_wood },
        VoxelBox { min: Vec3::new(-0.90, 1.00, -1.40), max: Vec3::new(-0.70, 1.70, -1.20), color: bronze },
        VoxelBox { min: Vec3::new(0.70, 1.00, -1.40), max: Vec3::new(0.90, 1.70, -1.20), color: bronze },
        // Composite Spring Bow Arms (Left & Right)
        VoxelBox { min: Vec3::new(-1.45, 1.28, -1.15), max: Vec3::new(-0.85, 1.42, -1.35), color: light_wood },
        VoxelBox { min: Vec3::new(0.85, 1.28, -1.15), max: Vec3::new(1.45, 1.42, -1.35), color: light_wood },
        // Twisted Steel/Sinew Cable String
        VoxelBox { min: Vec3::new(-1.40, 1.32, -1.15), max: Vec3::new(1.40, 1.38, -0.20), color: iron },
        // Rear Windlass & Hand Winch Spoke Cranks
        VoxelBox { min: Vec3::new(-0.35, 1.20, 0.95), max: Vec3::new(0.35, 1.45, 1.15), color: iron },
        VoxelBox { min: Vec3::new(-0.45, 1.15, 1.00), max: Vec3::new(-0.35, 1.65, 1.10), color: bronze },
        // Loaded Siege Spear
        VoxelBox { min: Vec3::new(-0.03, 1.48, -1.25), max: Vec3::new(0.03, 1.54, 0.35), color: light_wood },
        VoxelBox { min: Vec3::new(-0.06, 1.47, -1.45), max: Vec3::new(0.06, 1.55, -1.25), color: iron },
        VoxelBox { min: Vec3::new(-0.08, 1.46, 0.15), max: Vec3::new(0.08, 1.56, 0.30), color: red_mat },
    ])
}

pub fn create_battering_ram_mesh() -> Mesh {
    let timber = [0.35, 0.22, 0.12, 1.0];
    let roof_shingle = [0.45, 0.28, 0.15, 1.0];
    let iron = [0.26, 0.27, 0.28, 1.0];
    let bronze = [0.75, 0.55, 0.22, 1.0];
    let log_wood = [0.48, 0.32, 0.18, 1.0];

    build_voxel_mesh(&[
        // 4 Heavy Spoke Wheels with Iron Hoops
        VoxelBox { min: Vec3::new(-1.15, 0.0, -1.45), max: Vec3::new(-0.95, 0.70, -0.95), color: iron },
        VoxelBox { min: Vec3::new(0.95, 0.0, -1.45), max: Vec3::new(1.15, 0.70, -0.95), color: iron },
        VoxelBox { min: Vec3::new(-1.15, 0.0, 0.95), max: Vec3::new(-0.95, 0.70, 1.45), color: iron },
        VoxelBox { min: Vec3::new(0.95, 0.0, 0.95), max: Vec3::new(1.15, 0.70, 1.45), color: iron },
        // Chassis Platform Rails
        VoxelBox { min: Vec3::new(-0.95, 0.25, -1.70), max: Vec3::new(-0.75, 0.50, 1.70), color: timber },
        VoxelBox { min: Vec3::new(0.75, 0.25, -1.70), max: Vec3::new(0.95, 0.50, 1.70), color: timber },
        // 4 Corner Scaffold Posts
        VoxelBox { min: Vec3::new(-0.92, 0.50, -1.55), max: Vec3::new(-0.75, 2.20, -1.35), color: timber },
        VoxelBox { min: Vec3::new(0.75, 0.50, -1.55), max: Vec3::new(0.92, 2.20, -1.35), color: timber },
        VoxelBox { min: Vec3::new(-0.92, 0.50, 1.35), max: Vec3::new(-0.75, 2.20, 1.55), color: timber },
        VoxelBox { min: Vec3::new(0.75, 0.50, 1.35), max: Vec3::new(0.92, 2.20, 1.55), color: timber },
        // Top Roof Ridge & Sloped Protective Canopy
        VoxelBox { min: Vec3::new(-0.85, 2.10, -1.75), max: Vec3::new(0.85, 2.25, 1.75), color: timber },
        VoxelBox { min: Vec3::new(-0.95, 2.10, -1.75), max: Vec3::new(0.0, 2.65, 1.75), color: roof_shingle },
        VoxelBox { min: Vec3::new(0.0, 2.10, -1.75), max: Vec3::new(0.95, 2.65, 1.75), color: roof_shingle },
        // Suspended Iron Chains (Hangers)
        VoxelBox { min: Vec3::new(-0.06, 1.25, -0.90), max: Vec3::new(0.06, 2.15, -0.82), color: iron },
        VoxelBox { min: Vec3::new(-0.06, 1.25, 0.82), max: Vec3::new(0.06, 2.15, 0.90), color: iron },
        // Giant Suspended Ramming Tree Trunk Log
        VoxelBox { min: Vec3::new(-0.22, 0.85, -1.95), max: Vec3::new(0.22, 1.30, 1.85), color: log_wood },
        // Ornate Bronze/Iron Horned Ram Head
        VoxelBox { min: Vec3::new(-0.28, 0.80, -2.40), max: Vec3::new(0.28, 1.35, -1.95), color: bronze },
        // Ram Horns (Curved Left & Right)
        VoxelBox { min: Vec3::new(-0.45, 1.00, -2.25), max: Vec3::new(-0.28, 1.45, -2.05), color: iron },
        VoxelBox { min: Vec3::new(0.28, 1.00, -2.25), max: Vec3::new(0.45, 1.45, -2.05), color: iron },
        // Crew Hand Grips along Log
        VoxelBox { min: Vec3::new(-0.32, 1.00, -0.45), max: Vec3::new(0.32, 1.10, -0.35), color: iron },
        VoxelBox { min: Vec3::new(-0.32, 1.00, 0.35), max: Vec3::new(0.32, 1.10, 0.45), color: iron },
    ])
}

/// Architectural Note: Procedural architectural wall model engineered for FPS crosshair alignment.
/// Embeds a distinctive high-contrast horizontal datum band / sightline trim at exactly y = +0.25m
/// (1.75m above floor level, corresponding to player eye-level in first-person perspective).
/// Clearing corners alongside this wall trains players to maintain crosshair placement at head height.
/// Also embeds a 1.25m crouch-cover horizontal ledge and vertical framing posts.
pub fn create_wall_mesh() -> Mesh {
    let timber_dark = [0.36, 0.24, 0.14, 1.0];
    let timber_light = [0.52, 0.38, 0.22, 1.0];
    let stone_body = [0.48, 0.46, 0.44, 1.0];
    let iron_trim = [0.25, 0.26, 0.28, 1.0];
    // High-contrast eye-level head datum line (1.75m eye level)
    let datum_band = [0.78, 0.62, 0.28, 1.0]; // Bronze/gold architectural datum line

    build_voxel_mesh(&[
        // Left & Right Structural Upright Framing Posts
        VoxelBox { min: Vec3::new(-2.0, -1.5, -0.22), max: Vec3::new(-1.75, 1.5, 0.22), color: timber_dark },
        VoxelBox { min: Vec3::new(1.75, -1.5, -0.22), max: Vec3::new(2.0, 1.5, 0.22), color: timber_dark },
        // Lower Masonry Foundation Course (y = -1.5 to -0.35)
        VoxelBox { min: Vec3::new(-1.75, -1.5, -0.18), max: Vec3::new(1.75, -0.35, 0.18), color: stone_body },
        // 1.25m Crouch Cover Tactical Ledge / Crossbar (-0.25 relative to wall center)
        VoxelBox { min: Vec3::new(-1.80, -0.35, -0.21), max: Vec3::new(1.80, -0.20, 0.21), color: timber_light },
        // Mid Wall Section (-0.20 to +0.20)
        VoxelBox { min: Vec3::new(-1.75, -0.20, -0.18), max: Vec3::new(1.75, 0.20, 0.18), color: stone_body },
        // 1.75m Head-Level Visual Datum Alignment Band (+0.20 to +0.30)
        VoxelBox { min: Vec3::new(-1.80, 0.20, -0.21), max: Vec3::new(1.80, 0.30, 0.21), color: datum_band },
        // Upper Wall Section (+0.30 to +1.35)
        VoxelBox { min: Vec3::new(-1.75, 0.30, -0.18), max: Vec3::new(1.75, 1.35, 0.18), color: stone_body },
        // Top Roof Support Header Beam (+1.35 to +1.50)
        VoxelBox { min: Vec3::new(-2.0, 1.35, -0.22), max: Vec3::new(2.0, 1.50, 0.22), color: timber_dark },
        // Iron Corner Braces
        VoxelBox { min: Vec3::new(-1.85, 0.18, -0.22), max: Vec3::new(-1.70, 0.32, 0.22), color: iron_trim },
        VoxelBox { min: Vec3::new(1.70, 0.18, -0.22), max: Vec3::new(1.85, 0.32, 0.22), color: iron_trim },
    ])
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
        if keys.just_pressed(KeyCode::KeyR) {
            build_state.selected_piece = match build_state.selected_piece {
                ModularPieceType::Foundation => ModularPieceType::Workbench,
                ModularPieceType::Workbench => ModularPieceType::Campfire,
                ModularPieceType::Campfire => ModularPieceType::Wall,
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
) {
    if !build_state.is_active { return; }

    let Ok((cam_t, _camera)) = camera_query.get_single() else { return; };
    
    let ray_origin = cam_t.translation();
    let ray_dir = cam_t.forward();

    let (hologram_entity, mat_handle) = if let Ok((entity, _, mat)) = hologram_query.get_single() {
        (entity, mat.clone())
    } else {
        let mesh = match build_state.selected_piece {
            ModularPieceType::Foundation => meshes.add(Cuboid::new(4.0, 1.0, 4.0)),
            ModularPieceType::Workbench => meshes.add(create_workbench_mesh()),
            ModularPieceType::Campfire => meshes.add(create_campfire_mesh()),
            ModularPieceType::Wall => meshes.add(create_wall_mesh()),
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

        let entity = commands.spawn((
            PbrBundle { mesh, material: material.clone(), ..default() },
            BuildHologram,
        )).id();
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
            
            let y_lift = match build_state.selected_piece {
                ModularPieceType::Foundation => 0.5,
                _ => 0.0,
            };

            target_transform.translation = Vec3::new(snapped_x, true_y + y_lift, snapped_z);
            target_transform.rotation = Quat::IDENTITY * manual_rotation_offset;
        }
    } else {
        target_transform.translation = ray_origin + ray_dir * 5.0;
        target_transform.rotation = Quat::IDENTITY * manual_rotation_offset;
    }

    // Architectural Note: Realistic Workbench Proximity Validation.
    // Foundations, Workbenches, and Campfires can be placed anywhere on terrain.
    // Walls, Floors, Roofs, and Ramps require proximity (< 20m) to a constructed Workbench.
    let is_starter_piece = matches!(
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
                "Wall" => (meshes.add(create_wall_mesh()), Color::WHITE, Collider::cuboid(4.0, 3.0, 0.4)),
                "Floor" => (meshes.add(Cuboid::new(4.0, 0.2, 4.0)), Color::srgb(0.5, 0.4, 0.3), Collider::cuboid(4.0, 0.2, 4.0)),
                "Roof" => (meshes.add(Cuboid::new(4.0, 0.2, 4.0)), Color::srgb(0.4, 0.3, 0.2), Collider::cuboid(4.0, 0.2, 4.0)),
                "Ramp" => { 
                    let ramp_mesh = create_ramp_mesh();
                    let col = Collider::trimesh_from_mesh(&ramp_mesh).unwrap_or_else(|| Collider::cuboid(4.0, 2.0, 4.0));
                    (meshes.add(ramp_mesh), Color::srgb(0.5, 0.5, 0.5), col)
                },
                "Workbench" => {
                    let wb_mesh = create_workbench_mesh();
                    (meshes.add(wb_mesh), Color::WHITE, Collider::cuboid(1.6, 1.2, 1.0))
                },
                "Campfire" => {
                    let cf_mesh = create_campfire_mesh();
                    (meshes.add(cf_mesh), Color::WHITE, Collider::cylinder(0.7, 0.35))
                },
                "Catapult" => {
                    let mesh = create_catapult_mesh();
                    (meshes.add(mesh), Color::WHITE, Collider::cuboid(2.4, 2.0, 3.2))
                },
                "Trebuchet" => {
                    let mesh = create_trebuchet_mesh();
                    (meshes.add(mesh), Color::WHITE, Collider::cuboid(2.8, 3.5, 4.2))
                },
                "Ballista" => {
                    let mesh = create_ballista_mesh();
                    (meshes.add(mesh), Color::WHITE, Collider::cuboid(2.6, 1.8, 2.8))
                },
                "Battering Ram" | "BatteringRam" => {
                    let mesh = create_battering_ram_mesh();
                    (meshes.add(mesh), Color::WHITE, Collider::cuboid(2.4, 2.8, 4.5))
                },
                _ => (meshes.add(Cuboid::new(1.0, 1.0, 1.0)), Color::srgb(0.5, 0.5, 0.5), Collider::cuboid(1.0, 1.0, 1.0)),
            };

            let transform = Transform::from_xyz(s.x, s.y, s.z)
                .with_rotation(Quat::from_xyzw(s.rot_x, s.rot_y, s.rot_z, s.rot_w));

            let sockets = match s.piece_type.as_str() {
                "Foundation" => ModularPieceType::Foundation.default_sockets(),
                "Wall" => ModularPieceType::Wall.default_sockets(),
                "Floor" => ModularPieceType::Floor.default_sockets(),
                "Roof" => ModularPieceType::Roof.default_sockets(),
                "Ramp" => ModularPieceType::Ramp.default_sockets(),
                _ => Vec::new(),
            };

            let material = if s.is_blueprint {
                materials.add(StandardMaterial {
                    base_color: Color::srgba(0.2, 0.6, 1.0, 0.65),
                    alpha_mode: AlphaMode::Blend,
                    unlit: false,
                    ..default()
                })
            } else {
                materials.add(StandardMaterial {
                    base_color: color,
                    perceptual_roughness: 0.85,
                    ..default()
                })
            };

            commands.spawn((
                PbrBundle {
                    mesh,
                    material,
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