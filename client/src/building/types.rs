// ============================================================================
// File: client/src/building/types.rs
// ============================================================================
// Core building types, piece definitions, voxel mesh builder, sockets, and asset manifests.

use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use std::collections::BTreeMap;

use crate::components::*;
use crate::templates::BuildingTemplateType;
use crate::physics::Collider;

// ----------------------------------------------------------------------------
// COMPOSITE BUILDING BOX MESH BUILDER
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct BuildingBox {
    pub min: Vec3,
    pub max: Vec3,
    pub color: [f32; 4],
}

/// Backwards compatibility alias for modular piece definitions
pub type VoxelBox = BuildingBox;

pub fn build_building_mesh(boxes: &[BuildingBox]) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(boxes.len() * 24);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(boxes.len() * 24);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(boxes.len() * 24);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(boxes.len() * 24);
    let mut indices: Vec<u32> = Vec::with_capacity(boxes.len() * 36);

    for b in boxes {
        let min = b.min;
        let max = b.max;
        let c = b.color;

        // Top Face (+Y)
        let s = positions.len() as u32;
        positions.push([min.x, max.y, max.z]);
        positions.push([max.x, max.y, max.z]);
        positions.push([max.x, max.y, min.z]);
        positions.push([min.x, max.y, min.z]);
        for _ in 0..4 { normals.push([0.0, 1.0, 0.0]); colors.push(c); }
        uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // Bottom Face (-Y)
        let s = positions.len() as u32;
        positions.push([min.x, min.y, min.z]);
        positions.push([max.x, min.y, min.z]);
        positions.push([max.x, min.y, max.z]);
        positions.push([min.x, min.y, max.z]);
        for _ in 0..4 { normals.push([0.0, -1.0, 0.0]); colors.push(c); }
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // East Face (+X)
        let s = positions.len() as u32;
        positions.push([max.x, min.y, max.z]);
        positions.push([max.x, min.y, min.z]);
        positions.push([max.x, max.y, min.z]);
        positions.push([max.x, max.y, max.z]);
        for _ in 0..4 { normals.push([1.0, 0.0, 0.0]); colors.push(c); }
        uvs.extend_from_slice(&[[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // West Face (-X)
        let s = positions.len() as u32;
        positions.push([min.x, min.y, min.z]);
        positions.push([min.x, min.y, max.z]);
        positions.push([min.x, max.y, max.z]);
        positions.push([min.x, max.y, min.z]);
        for _ in 0..4 { normals.push([-1.0, 0.0, 0.0]); colors.push(c); }
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // South Face (+Z)
        let s = positions.len() as u32;
        positions.push([min.x, min.y, max.z]);
        positions.push([max.x, min.y, max.z]);
        positions.push([max.x, max.y, max.z]);
        positions.push([min.x, max.y, max.z]);
        for _ in 0..4 { normals.push([0.0, 0.0, 1.0]); colors.push(c); }
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);

        // North Face (-Z)
        let s = positions.len() as u32;
        positions.push([max.x, min.y, min.z]);
        positions.push([min.x, min.y, min.z]);
        positions.push([min.x, max.y, min.z]);
        positions.push([max.x, max.y, min.z]);
        for _ in 0..4 { normals.push([0.0, 0.0, -1.0]); colors.push(c); }
        uvs.extend_from_slice(&[[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        indices.extend_from_slice(&[s, s + 1, s + 2, s, s + 2, s + 3]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

/// Backwards compatibility alias for building piece mesh construction
pub use build_building_mesh as build_voxel_mesh;

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
                Socket { name: "FoundationNorth".into(), local_offset: Vec3::new(0.0, 0.0, -4.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "FoundationSouth".into(), local_offset: Vec3::new(0.0, 0.0, 4.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "FoundationEast".into(), local_offset: Vec3::new(4.0, 0.0, 0.0), local_rotation: Quat::IDENTITY, is_occupied: false },
                Socket { name: "FoundationWest".into(), local_offset: Vec3::new(-4.0, 0.0, 0.0), local_rotation: Quat::IDENTITY, is_occupied: false },
            ],
            Self::Wall | Self::Window | Self::Door => vec![
                Socket { name: "WallTop".into(), local_offset: Vec3::new(0.0, 3.0, 0.0), local_rotation: Quat::IDENTITY, is_occupied: false },
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

pub fn is_socket_compatible(piece_type: ModularPieceType, socket_name: &str) -> bool {
    match piece_type {
        ModularPieceType::Wall | ModularPieceType::Window | ModularPieceType::Door => {
            matches!(socket_name, "North" | "South" | "East" | "West" | "WallTop" | "TopCenter")
        }
        ModularPieceType::Foundation => {
            matches!(socket_name, "FoundationNorth" | "FoundationSouth" | "FoundationEast" | "FoundationWest")
        }
        ModularPieceType::Floor | ModularPieceType::Roof => {
            matches!(socket_name, "Top" | "TopCenter" | "TopForward" | "TopBackward" | "Bottom")
        }
        ModularPieceType::Ramp => {
            matches!(socket_name, "North" | "South" | "East" | "West" | "Top" | "Bottom")
        }
        ModularPieceType::Workbench | ModularPieceType::Campfire => {
            matches!(socket_name, "Top")
        }
    }
}

#[derive(Resource)]
pub struct BuildModeState {
    pub is_active: bool,
    pub selected_piece: ModularPieceType,
    #[deprecated(note = "Frontier Wood & Stone style is canonical; multi-race architectural cycling is archived")]
    #[allow(deprecated)]
    pub selected_faction: BuildingFaction,
    pub rotation_steps: u8,
    pub selected_template: Option<BuildingTemplateType>,
}

#[allow(deprecated)]
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
// DATA-DRIVEN BUILDING ASSET MANIFEST & GENERIC RUBBLE POOL
// ----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct BuildingPieceVisuals {
    pub pristine_mesh: Handle<Mesh>,
    pub damaged_mesh: Handle<Mesh>,
    pub collider: Collider,
}

#[derive(Resource, Clone, Debug)]
#[allow(dead_code)]
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
    pub pieces: BTreeMap<ModularPieceType, BuildingPieceVisuals>,
    pub door_leaf: Handle<Mesh>,
    pub standard_material: Handle<StandardMaterial>,
    pub blueprint_material: Handle<StandardMaterial>,
    pub glass_pane_mesh: Handle<Mesh>,
    pub glass_material: Handle<StandardMaterial>,
    pub rubble_node_mesh: Handle<Mesh>,
    pub generic_rubble_pool: GenericRubblePool,
}

pub type BuildingMeshCache = BuildingAssetManifest;

impl BuildingAssetManifest {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        use super::meshes::*;

        // Frontier Wood & Stone procedural models
        let wall_p = meshes.add(create_wall_mesh());
        let wall_d = meshes.add(create_damaged_wall_mesh());
        let win_p = meshes.add(create_window_mesh());
        let win_d = meshes.add(create_damaged_window_mesh());
        let df_p = meshes.add(create_door_frame_mesh());
        let df_d = meshes.add(create_damaged_door_frame_mesh());
        let door_leaf = meshes.add(create_door_leaf_mesh());
        let fnd_mesh = meshes.add(Cuboid::new(4.0, 1.0, 4.0));
        let rf_mesh = meshes.add(Cuboid::new(4.0, 0.2, 4.0));
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
        pieces.insert(ModularPieceType::Wall, BuildingPieceVisuals { pristine_mesh: wall_p.clone(), damaged_mesh: wall_d.clone(), collider: wall_col });
        pieces.insert(ModularPieceType::Window, BuildingPieceVisuals { pristine_mesh: win_p.clone(), damaged_mesh: win_d.clone(), collider: win_frame_col });
        pieces.insert(ModularPieceType::Door, BuildingPieceVisuals { pristine_mesh: df_p.clone(), damaged_mesh: df_d.clone(), collider: door_frame_col });
        pieces.insert(ModularPieceType::Foundation, BuildingPieceVisuals { pristine_mesh: fnd_mesh.clone(), damaged_mesh: fnd_mesh.clone(), collider: fnd_col });
        pieces.insert(ModularPieceType::Roof, BuildingPieceVisuals { pristine_mesh: rf_mesh.clone(), damaged_mesh: rf_mesh.clone(), collider: rf_col });
        pieces.insert(ModularPieceType::Floor, BuildingPieceVisuals { pristine_mesh: floor_mesh.clone(), damaged_mesh: floor_mesh.clone(), collider: fl_col });
        pieces.insert(ModularPieceType::Ramp, BuildingPieceVisuals { pristine_mesh: ramp_mesh.clone(), damaged_mesh: ramp_mesh.clone(), collider: ramp_col });
        pieces.insert(ModularPieceType::Workbench, BuildingPieceVisuals { pristine_mesh: workbench_mesh.clone(), damaged_mesh: workbench_mesh.clone(), collider: wb_col });
        pieces.insert(ModularPieceType::Campfire, BuildingPieceVisuals { pristine_mesh: campfire_mesh.clone(), damaged_mesh: campfire_mesh.clone(), collider: cf_col });

        // Canonical Frontier Wood & Stone PBR material
        let standard_material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.35,
            ..default()
        });

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
            door_leaf,
            standard_material,
            blueprint_material,
            glass_pane_mesh,
            glass_material,
            rubble_node_mesh,
            generic_rubble_pool,
        }
    }

    pub fn get_piece_mesh(
        &self,
        piece_type: ModularPieceType,
        damage: VisualDamageState,
    ) -> Handle<Mesh> {
        let is_damaged = matches!(damage, VisualDamageState::Damaged);
        if let Some(visuals) = self.pieces.get(&piece_type) {
            if is_damaged {
                visuals.damaged_mesh.clone()
            } else {
                visuals.pristine_mesh.clone()
            }
        } else {
            self.pieces.get(&ModularPieceType::Foundation)
                .map(|v| v.pristine_mesh.clone())
                .unwrap_or_default()
        }
    }

    pub fn get_door_leaf_mesh(&self) -> Handle<Mesh> {
        self.door_leaf.clone()
    }

    pub fn get_material(&self, is_blueprint: bool) -> Handle<StandardMaterial> {
        if is_blueprint {
            self.blueprint_material.clone()
        } else {
            self.standard_material.clone()
        }
    }

    pub fn get_collider(&self, piece_type: ModularPieceType) -> Collider {
        self.pieces.get(&piece_type)
            .map(|v| v.collider.clone())
            .unwrap_or_else(|| Collider::cuboid(4.0, 1.0, 4.0))
    }
}

impl FromWorld for BuildingAssetManifest {
    fn from_world(world: &mut World) -> Self {
        world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
            let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
            BuildingAssetManifest::new(&mut meshes, &mut materials)
        })
    }
}
