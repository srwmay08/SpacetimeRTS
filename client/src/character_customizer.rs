// ============================================================================
// File: client/src/character_customizer.rs
// ============================================================================
// ----------------------------------------------------------------------------
// IN-GAME PROCEDURAL CHARACTER CUSTOMIZER & HIERARCHICAL ANIMATION ENGINE
// ----------------------------------------------------------------------------
// Architectural Directives (AGENTS.md & Procedural Animation Specs):
// 1. Unified Entity Hierarchy (Body + Face):
//    - Couples procedural hierarchical primitives (Cuboids) for the body with
//      a raw custom vertex mesh for the face, animated entirely in Rust via
//      procedural joint rotations and upper-body animation masking.
//    - Zero runtime vertex skinning overhead or GPU bone uniform buffer limits.
// 2. Procedural Face Mesh Generation:
//    - Generates flat-shaded faceted low-poly facial features (chin, jaw, cheeks,
//      nose bridge, nose tip, brow ridge, eye sockets) at runtime.
//    - Uses duplicate_vertices + compute_flat_normals for crisp BlendSwap #9440 aesthetic.
// 3. Mathematical Locomotion & Action Dynamics:
//    - Gait phase accumulator driving opposing hip/knee sine waves with half-wave
//      rectified backward knee bends and forward torso lean.
//    - Velocity-driven vertical air states (ascending knee tuck vs falling reach).
//    - 3-phase melee weapon swing anticipation/snap/follow-through override.
//    - Two-handed bow aim and draw tension constraint kinematics.
//    - Exponential decay hit reactions.
// 4. In-Game Editor Workbench UI:
//    - Toggleable via [F5] hotkey or 'customizer' / 'char' / 'model' console commands.
//    - Live interactive morphology sculpting: Face sculpt, Body anatomy, Skin/Wardrobe,
//      and Real-time Animation Studio preview.
//    - Instant Rust code export for permanent preset compilation.
// ----------------------------------------------------------------------------

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::RenderLayers;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use crate::physics::LinearVelocity;
use tracing::info;

use rapier3d::prelude::{
    CCDSolver, ColliderBuilder, ColliderHandle, ColliderSet, DefaultBroadPhase,
    ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
    NarrowPhase, PhysicsPipeline, QueryPipeline, RigidBodyBuilder,
    RigidBodyHandle, RigidBodySet, SharedShape,
};

use crate::components::*;
use crate::core::*;
use crate::camera::CharacterCameraSettings;
use crate::weapons::{WeaponState, WeaponType};
use crate::ui::types::ClientEquippedArmor;

// ----------------------------------------------------------------------------
// 1. DATA STRUCTURES & MORPHOLOGY PROFILES
// ----------------------------------------------------------------------------

/// Geometric parameters governing the procedural raw facial mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceProfile {
    /// Lateral span of the jawbone from center (meters).
    pub jaw_width: f32,
    /// Vertical displacement of the chin tip relative to skull base (meters).
    pub jaw_height: f32,
    /// Lateral protrusion of the cheekbones (meters).
    pub cheekbone_width: f32,
    /// Vertical length of the nasal ridge (meters).
    pub nose_bridge_length: f32,
    /// Forward protrusion of the nose tip along the -Z axis (meters).
    pub nose_tip_z: f32,
    /// Protrusion of the brow ridge over the eyes (meters).
    pub brow_ridge: f32,
    /// Forward extension of the chin point along -Z (meters).
    pub chin_forward: f32,
    /// Depth of the recessed eye sockets (meters).
    pub eye_depth: f32,
}

impl Default for FaceProfile {
    fn default() -> Self {
        Self {
            jaw_width: 0.11,
            jaw_height: -0.14,
            cheekbone_width: 0.13,
            nose_bridge_length: 0.10,
            nose_tip_z: 0.06,
            brow_ridge: 0.035,
            chin_forward: 0.045,
            eye_depth: 0.015,
        }
    }
}

/// Anatomical proportions and material color palette for the character body.
#[derive(Clone, Debug, PartialEq)]
pub struct RaceAnatomyProfile {
    pub race_name: &'static str,
    /// Dimensions of the central torso cuboid (width, height, depth in meters).
    pub torso_size: Vec3,
    /// Lateral distance from torso center to left/right shoulder pivots (meters).
    pub shoulder_width_offset: f32,
    /// Lateral distance from torso center to left/right hip pivots (meters).
    pub hip_width_offset: f32,
    /// Length of the upper arm segment (meters).
    pub upper_arm_length: f32,
    /// Length of the forearm segment (meters).
    pub forearm_length: f32,
    /// Length of the upper leg / thigh (meters).
    pub upper_leg_length: f32,
    /// Length of the lower leg / calf (meters).
    pub lower_leg_length: f32,
    /// Cross-sectional thickness of the limb primitives (meters).
    pub limb_thickness: f32,
    /// Scaling multiplier applied to the cranial skull cuboid and face mesh.
    pub head_scale: f32,
    /// Base skin tone color.
    pub skin_color: Color,
    /// Tunic / upper torso cloth color.
    pub cloth_color: Color,
    /// Pants / lower body cloth color.
    pub pants_color: Color,
    /// Armor / detail accent color (spikes, pauldrons, buckles).
    pub armor_color: Color,
    /// Size of shoulder armor pauldrons (0.0 = none).
    pub pauldron_size: f32,
    /// Whether shoulder armor features pointed pyramid spikes.
    pub has_shoulder_spikes: bool,
}

impl Default for RaceAnatomyProfile {
    fn default() -> Self {
        Self {
            race_name: "Human",
            torso_size: Vec3::new(0.42, 0.58, 0.26),
            shoulder_width_offset: 0.26,
            hip_width_offset: 0.14,
            upper_arm_length: 0.32,
            forearm_length: 0.30,
            upper_leg_length: 0.40,
            lower_leg_length: 0.38,
            limb_thickness: 0.11,
            head_scale: 1.0,
            skin_color: Color::srgb(0.85, 0.68, 0.55),
            cloth_color: Color::srgb(0.24, 0.36, 0.52),
            pants_color: Color::srgb(0.18, 0.16, 0.15),
            armor_color: Color::srgb(0.45, 0.45, 0.48),
            pauldron_size: 0.16,
            has_shoulder_spikes: false,
        }
    }
}

// ----------------------------------------------------------------------------
// 2. PRESETS CATALOG
// ----------------------------------------------------------------------------

pub struct CharacterPreset {
    pub name: &'static str,
    pub description: &'static str,
    pub face: FaceProfile,
    pub anatomy: RaceAnatomyProfile,
}

pub const CHARACTER_PRESETS: &[CharacterPreset] = &[
    CharacterPreset {
        name: "Human Adventurer",
        description: "Balanced athletic humanoid proportions and versatile cranial silhouette.",
        face: FaceProfile {
            jaw_width: 0.11,
            jaw_height: -0.14,
            cheekbone_width: 0.13,
            nose_bridge_length: 0.10,
            nose_tip_z: 0.06,
            brow_ridge: 0.035,
            chin_forward: 0.045,
            eye_depth: 0.015,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Human",
            torso_size: Vec3::new(0.42, 0.58, 0.26),
            shoulder_width_offset: 0.26,
            hip_width_offset: 0.14,
            upper_arm_length: 0.32,
            forearm_length: 0.30,
            upper_leg_length: 0.40,
            lower_leg_length: 0.38,
            limb_thickness: 0.11,
            head_scale: 1.0,
            skin_color: Color::srgb(0.85, 0.68, 0.55),
            cloth_color: Color::srgb(0.22, 0.34, 0.48),
            pants_color: Color::srgb(0.18, 0.16, 0.15),
            armor_color: Color::srgb(0.45, 0.45, 0.48),
            pauldron_size: 0.16,
            has_shoulder_spikes: false,
        },
    },
    CharacterPreset {
        name: "Dark Elf (Drow)",
        description: "Slender agile frame, acute triangular jaw, high cheekbones, and sharp nasal bridge.",
        face: FaceProfile {
            jaw_width: 0.08,
            jaw_height: -0.16,
            cheekbone_width: 0.14,
            nose_bridge_length: 0.12,
            nose_tip_z: 0.08,
            brow_ridge: 0.045,
            chin_forward: 0.055,
            eye_depth: 0.020,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Dark Elf",
            torso_size: Vec3::new(0.36, 0.62, 0.22),
            shoulder_width_offset: 0.23,
            hip_width_offset: 0.13,
            upper_arm_length: 0.35,
            forearm_length: 0.33,
            upper_leg_length: 0.44,
            lower_leg_length: 0.42,
            limb_thickness: 0.095,
            head_scale: 0.95,
            skin_color: Color::srgb(0.55, 0.52, 0.65), // Dusky twilight purple
            cloth_color: Color::srgb(0.18, 0.12, 0.25), // Obsidian violet
            pants_color: Color::srgb(0.12, 0.10, 0.16),
            armor_color: Color::srgb(0.72, 0.65, 0.38), // Burnished elven brass
            pauldron_size: 0.14,
            has_shoulder_spikes: true,
        },
    },
    CharacterPreset {
        name: "Barbarian Berserker",
        description: "Massive muscular torso, wide blocky jaw, blunt nose, and spiked heavy pauldrons.",
        face: FaceProfile {
            jaw_width: 0.15,
            jaw_height: -0.12,
            cheekbone_width: 0.16,
            nose_bridge_length: 0.09,
            nose_tip_z: 0.05,
            brow_ridge: 0.055,
            chin_forward: 0.065,
            eye_depth: 0.022,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Barbarian",
            torso_size: Vec3::new(0.56, 0.66, 0.34),
            shoulder_width_offset: 0.34,
            hip_width_offset: 0.17,
            upper_arm_length: 0.36,
            forearm_length: 0.34,
            upper_leg_length: 0.42,
            lower_leg_length: 0.40,
            limb_thickness: 0.15,
            head_scale: 1.10,
            skin_color: Color::srgb(0.78, 0.54, 0.40), // Bronzed sun-hardened tan
            cloth_color: Color::srgb(0.55, 0.18, 0.15), // Crimson war-tunic
            pants_color: Color::srgb(0.25, 0.18, 0.14),
            armor_color: Color::srgb(0.32, 0.30, 0.34), // Blackened iron
            pauldron_size: 0.24,
            has_shoulder_spikes: true,
        },
    },
    CharacterPreset {
        name: "Forest Troll",
        description: "Elongated drooping snout, massive forward jaw, gangly limbs, and mossy green flesh.",
        face: FaceProfile {
            jaw_width: 0.14,
            jaw_height: -0.22,
            cheekbone_width: 0.15,
            nose_bridge_length: 0.16,
            nose_tip_z: 0.12,
            brow_ridge: 0.060,
            chin_forward: 0.075,
            eye_depth: 0.025,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Troll",
            torso_size: Vec3::new(0.46, 0.70, 0.30),
            shoulder_width_offset: 0.29,
            hip_width_offset: 0.15,
            upper_arm_length: 0.44,
            forearm_length: 0.42,
            upper_leg_length: 0.46,
            lower_leg_length: 0.44,
            limb_thickness: 0.12,
            head_scale: 1.05,
            skin_color: Color::srgb(0.38, 0.52, 0.32), // Mossy green
            cloth_color: Color::srgb(0.32, 0.26, 0.18), // Rough burlap
            pants_color: Color::srgb(0.20, 0.18, 0.14),
            armor_color: Color::srgb(0.40, 0.35, 0.25),
            pauldron_size: 0.18,
            has_shoulder_spikes: true,
        },
    },
    CharacterPreset {
        name: "Mountain Dwarf",
        description: "Broad stocky torso, low center of gravity, heavy brow, and dense thickset limbs.",
        face: FaceProfile {
            jaw_width: 0.15,
            jaw_height: -0.12,
            cheekbone_width: 0.15,
            nose_bridge_length: 0.09,
            nose_tip_z: 0.07,
            brow_ridge: 0.055,
            chin_forward: 0.055,
            eye_depth: 0.020,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Dwarf",
            torso_size: Vec3::new(0.52, 0.48, 0.34),
            shoulder_width_offset: 0.31,
            hip_width_offset: 0.18,
            upper_arm_length: 0.26,
            forearm_length: 0.24,
            upper_leg_length: 0.28,
            lower_leg_length: 0.26,
            limb_thickness: 0.14,
            head_scale: 1.05,
            skin_color: Color::srgb(0.82, 0.65, 0.52),
            cloth_color: Color::srgb(0.28, 0.42, 0.36), // Deep alpine green
            pants_color: Color::srgb(0.22, 0.18, 0.15),
            armor_color: Color::srgb(0.55, 0.52, 0.50), // Mountain steel
            pauldron_size: 0.22,
            has_shoulder_spikes: false,
        },
    },
];

// ----------------------------------------------------------------------------
// 3. RAW RAPIER PHYSICS WORLD RESOURCE & ECS HANDLES
// ----------------------------------------------------------------------------

/// Pure Rapier3D physics state wrapped as a standard Bevy Resource.
/// Allows manual deterministic stepping in FixedUpdate and identical simulation
/// matching the SpacetimeDB server.
#[derive(Resource)]
pub struct PhysicsWorld {
    pub gravity: rapier3d::na::Vector3<f32>,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub query_pipeline: QueryPipeline,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self {
            gravity: rapier3d::na::Vector3::new(0.0, -9.81, 0.0),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
        }
    }
}

impl PhysicsWorld {
    pub fn step(&mut self) {
        let physics_hooks = ();
        let event_handler = ();
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &physics_hooks,
            &event_handler,
        );
    }
}

/// Identifies the owning Rapier RigidBody for an entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RbHandle(pub RigidBodyHandle);

/// Identifies the corresponding Rapier Collider hitbox for an articulated visual joint.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColHandle(pub ColliderHandle);

/// Marks intermediate limb mesh segments for in-place scale and offset mutation.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimbMeshSegment(pub JointType);

// ----------------------------------------------------------------------------
// 4. ECS COMPONENTS & MARKERS
// ----------------------------------------------------------------------------

/// Marks the specific anatomical joint for procedural rotation updates.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointType {
    Torso,
    Head,
    ShoulderL,
    ElbowL,
    HandL,
    ShoulderR,
    ElbowR,
    HandR,
    HipL,
    KneeL,
    FootL,
    HipR,
    KneeR,
    FootR,
}

/// Identifies the owning character entity for an articulated joint.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct JointOwner(pub Entity);

/// Identifies the root container for the procedural character hierarchy.
#[derive(Component)]
pub struct CharacterModelRoot;

/// Marker for the generated procedural face mesh entity.
#[derive(Component)]
pub struct ProceduralFaceMesh;

/// Marker for the equipped 3D helmet mesh child entity.
#[derive(Component)]
pub struct EquippedHelmetMesh;

/// Attached to the player to hold their active customization data.
#[derive(Component, Clone, Debug)]
pub struct PlayerCharacterCustomization {
    pub face: FaceProfile,
    pub anatomy: RaceAnatomyProfile,
    pub dirty: bool,
}

impl Default for PlayerCharacterCustomization {
    fn default() -> Self {
        Self {
            face: CHARACTER_PRESETS[0].face.clone(),
            anatomy: CHARACTER_PRESETS[0].anatomy.clone(),
            dirty: true,
        }
    }
}

/// Procedural animation state machine governing joint phase, velocities, and action overrides.
#[derive(Component, Clone, Debug)]
pub struct AnimationState {
    pub gait_phase: f32,
    pub is_grounded: bool,
    pub vertical_velocity: f32,
    pub action: ActionState,
    pub hit_react_timer: f32,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            gait_phase: 0.0,
            is_grounded: true,
            vertical_velocity: 0.0,
            action: ActionState::None,
            hit_react_timer: 0.0,
        }
    }
}

/// Action state determining upper-body kinematic overrides.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum ActionState {
    #[default]
    None,
    MeleeSwing { timer: f32, duration: f32 },
    BowAim { draw_progress: f32, pitch: f32 },
}

// ----------------------------------------------------------------------------
// 5. PROCEDURAL MESH GENERATORS
// ----------------------------------------------------------------------------

/// Generates a stylized, low-poly faceted 3D face mesh matching the FaceProfile.
/// Winding order is counter-clockwise for outward normals, followed by flat normal calculation.
pub fn generate_custom_face(profile: &FaceProfile) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    // Coordinate positions in local space (Forward is -Z, Up is +Y, Right is +X)
    // Sized flush to the cranial skull bounds (width ~0.24m, height ~0.26m)
    let positions: Vec<[f32; 3]> = vec![
        // 0: Chin Tip
        [0.0, profile.jaw_height, -profile.chin_forward],
        // 1: Left Jaw
        [-profile.jaw_width, profile.jaw_height + 0.06, 0.0],
        // 2: Right Jaw
        [profile.jaw_width, profile.jaw_height + 0.06, 0.0],
        // 3: Nose Tip
        [0.0, -0.02, -profile.nose_tip_z],
        // 4: Nose Bridge (Midway up)
        [0.0, profile.nose_bridge_length * 0.35, -profile.nose_tip_z * 0.55],
        // 5: Left Cheekbone
        [-profile.cheekbone_width, -0.01, -0.01],
        // 6: Right Cheekbone
        [profile.cheekbone_width, -0.01, -0.01],
        // 7: Forehead Center (flush with top cranial brow)
        [0.0, 0.11, 0.0],
        // 8: Left Brow
        [-profile.cheekbone_width * 0.75, 0.06, -profile.brow_ridge],
        // 9: Right Brow
        [profile.cheekbone_width * 0.75, 0.06, -profile.brow_ridge],
        // 10: Left Eye Socket
        [-profile.cheekbone_width * 0.45, 0.02, -profile.eye_depth],
        // 11: Right Eye Socket
        [profile.cheekbone_width * 0.45, 0.02, -profile.eye_depth],
        // 12: Upper Lip / Philtrum
        [0.0, profile.jaw_height * 0.45, -profile.chin_forward * 0.6],
    ];

    // Connect vertices into triangular facets with counter-clockwise winding
    let indices = Indices::U32(vec![
        // Lower Chin / Jaw
        0, 1, 12,   // Chin to Left Jaw
        0, 12, 2,   // Chin to Right Jaw
        1, 5, 12,   // Left Jaw to Cheek
        2, 12, 6,   // Right Jaw to Cheek

        // Nose Pyramid
        12, 5, 3,   // Left under-nose to cheek
        12, 3, 6,   // Right under-nose to cheek
        3, 5, 4,    // Left nose bridge facet
        3, 4, 6,    // Right nose bridge facet

        // Eye Sockets & Cheeks
        5, 8, 10,   // Left cheek to brow
        4, 5, 10,   // Nose bridge to left eye
        4, 10, 8,   // Nose to left brow
        6, 11, 9,   // Right cheek to brow
        4, 11, 6,   // Nose bridge to right eye
        4, 9, 11,   // Nose to right brow

        // Forehead
        4, 8, 7,    // Forehead left
        4, 7, 9,    // Forehead right
        8, 5, 7,    // Temple left
        9, 7, 6,    // Temple right
    ]);

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(indices);

    // Duplicate vertices per face and compute flat normals to guarantee crisp low-poly facets
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}

/// Generates a sharp, faceted low-poly pyramid suitable for spiked armor pauldrons.
pub fn create_low_poly_pyramid() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    let positions = vec![
        [0.0, 0.24, 0.0],    // Apex
        [-0.12, 0.0, 0.12],  // Front-Left
        [0.12, 0.0, 0.12],   // Front-Right
        [0.12, 0.0, -0.12],  // Back-Right
        [-0.12, 0.0, -0.12], // Back-Left
    ];

    let indices = Indices::U32(vec![
        0, 1, 2, // Front Face
        0, 2, 3, // Right Face
        0, 3, 4, // Back Face
        0, 4, 1, // Left Face
        1, 4, 3, // Base Bottom 1
        1, 3, 2, // Base Bottom 2
    ]);

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(indices);
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}

/// Generates a stylized, low-poly faceted iron helmet with crown plates, crest, nasal guard,
/// and cheek protectors matching the SpacetimeRTS BlendSwap #9440 aesthetic.
pub fn create_low_poly_helmet() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    // Coordinate positions in local space of JointType::Head.
    // The cranial skull is centered at [0.0, 0.08, 0.0] with extents [-0.12..+0.12, -0.05..+0.21, -0.13..+0.13].
    // Forward is -Z, Up is +Y, Right is +X.
    let positions: Vec<[f32; 3]> = vec![
        // 0: Crown Apex (top center of the helmet dome)
        [0.0, 0.245, 0.0],
        // 1: Forehead Brow Center (rim right above the eyebrows)
        [0.0, 0.14, -0.142],
        // 2: Left Brow / Temple
        [-0.132, 0.13, -0.11],
        // 3: Right Brow / Temple
        [0.132, 0.13, -0.11],
        // 4: Left Ear / Side Rim
        [-0.136, 0.06, 0.01],
        // 5: Right Ear / Side Rim
        [0.136, 0.06, 0.01],
        // 6: Left Back Occipital
        [-0.132, 0.05, 0.136],
        // 7: Right Back Occipital
        [0.132, 0.05, 0.136],
        // 8: Back Neck Center Rim
        [0.0, 0.04, 0.142],
        // 9: Crest Ridge Front
        [0.0, 0.265, -0.08],
        // 10: Crest Ridge Peak
        [0.0, 0.28, 0.02],
        // 11: Crest Ridge Back
        [0.0, 0.23, 0.11],
        // 12: Nasal Guard Tip (pointed guard extending down the nose)
        [0.0, 0.02, -0.155],
        // 13: Left Cheek Guard Bottom
        [-0.125, -0.01, -0.05],
        // 14: Right Cheek Guard Bottom
        [0.125, -0.01, -0.05],
    ];

    let indices = Indices::U32(vec![
        // Dome Crown Facets (counter-clockwise winding for outward normals)
        0, 2, 1,   // Crown to Left Forehead
        0, 1, 3,   // Crown to Right Forehead
        0, 4, 2,   // Crown to Left Side
        0, 3, 5,   // Crown to Right Side
        0, 6, 4,   // Crown to Left Back
        0, 5, 7,   // Crown to Right Back
        0, 8, 6,   // Crown to Center Back Left
        0, 7, 8,   // Crown to Center Back Right

        // Central Crest Fin (raised spine along the skull)
        1, 9, 0,   // Front brow to crest front
        0, 9, 10,  // Crest front to peak
        0, 10, 11, // Crest peak to back
        0, 11, 8,  // Crest back to neck rim

        // Nasal Guard (pyramidal guard protecting nose bridge)
        1, 12, 2,  // Brow center to nasal tip to left brow
        1, 3, 12,  // Brow center to right brow to nasal tip

        // Cheek Guards (flanking jaw & ear)
        2, 4, 13,  // Left temple to ear to cheek bottom
        3, 14, 5,  // Right temple to cheek bottom to ear
    ]);

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(indices);
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}

// ----------------------------------------------------------------------------
// 6. PROCEDURAL CHARACTER HIERARCHY & COMPOUND PHYSICS SKELETON
// ----------------------------------------------------------------------------

/// Spawns the entire articulated character hierarchy matching the SpacetimeRTS directives.
/// Constructs the visual Bevy entity tree and builds the compound Rapier physics skeleton
/// simultaneously, attaching ColHandle to each joint and RbHandle to the root.
pub fn spawn_procedural_character_hierarchy(
    commands: &mut Commands,
    parent_entity: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    race: &RaceAnatomyProfile,
    face: &FaceProfile,
    render_layers: RenderLayers,
    physics: &mut PhysicsWorld,
) -> Entity {
    // 1. Create root kinematic RigidBody in Rapier physics world
    let root_rb = RigidBodyBuilder::kinematic_position_based().build();
    let root_rb_handle = physics.rigid_body_set.insert(root_rb);

    // 2. Unit cuboid mesh shared across all procedural limbs (sized purely via Transform::scale)
    let unit_cuboid = meshes.add(bevy::math::primitives::Cuboid::new(1.0, 1.0, 1.0));
    let face_mesh = meshes.add(generate_custom_face(face));
    let spike_mesh = meshes.add(create_low_poly_pyramid());

    // 3. Material assets (Two-sided PBR materials for crisp low-poly faceted shading)
    let skin_mat = materials.add(StandardMaterial {
        base_color: race.skin_color,
        perceptual_roughness: 0.82,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let cloth_mat = materials.add(StandardMaterial {
        base_color: race.cloth_color,
        perceptual_roughness: 0.88,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let pants_mat = materials.add(StandardMaterial {
        base_color: race.pants_color,
        perceptual_roughness: 0.85,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let armor_mat = materials.add(StandardMaterial {
        base_color: race.armor_color,
        metallic: 0.65,
        perceptual_roughness: 0.45,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    let layers = render_layers;
    let mut model_root_id = Entity::PLACEHOLDER;

    // 4. Construct Rapier compound colliders with parent root RigidBodyHandle (using half-extents)
    let shoulder_y = race.torso_size.y * 0.5 - 0.08;
    let hip_y = -race.torso_size.y * 0.5;

    // Torso Collider (center: 0, 0, 0)
    let torso_col = ColliderBuilder::cuboid(
        race.torso_size.x * 0.5,
        race.torso_size.y * 0.5,
        race.torso_size.z * 0.5,
    );
    let torso_col_h = physics.collider_set.insert_with_parent(
        torso_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Head Collider
    let head_col = ColliderBuilder::cuboid(
        0.24 * race.head_scale * 0.5,
        0.26 * race.head_scale * 0.5,
        0.26 * race.head_scale * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        0.0,
        race.torso_size.y * 0.5 + 0.22 * race.head_scale,
        0.0,
    ));
    let head_col_h = physics.collider_set.insert_with_parent(
        head_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Upper Arm L Collider
    let upper_arm_l_col = ColliderBuilder::cuboid(
        race.limb_thickness * 0.5,
        race.upper_arm_length * 0.5,
        race.limb_thickness * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        -race.shoulder_width_offset,
        shoulder_y - race.upper_arm_length * 0.5,
        0.0,
    ));
    let upper_arm_l_h = physics.collider_set.insert_with_parent(
        upper_arm_l_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Forearm L Collider
    let elbow_l_col = ColliderBuilder::cuboid(
        race.limb_thickness * 0.9 * 0.5,
        race.forearm_length * 0.5,
        race.limb_thickness * 0.9 * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        -race.shoulder_width_offset,
        shoulder_y - race.upper_arm_length - race.forearm_length * 0.5,
        0.0,
    ));
    let elbow_l_h = physics.collider_set.insert_with_parent(
        elbow_l_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Hand L Collider
    let hand_l_col = ColliderBuilder::cuboid(
        race.limb_thickness * 0.95 * 0.5,
        0.12 * 0.5,
        race.limb_thickness * 1.1 * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        -race.shoulder_width_offset,
        shoulder_y - race.upper_arm_length - race.forearm_length - 0.04,
        0.0,
    ));
    let hand_l_h = physics.collider_set.insert_with_parent(
        hand_l_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Upper Arm R Collider
    let upper_arm_r_col = ColliderBuilder::cuboid(
        race.limb_thickness * 0.5,
        race.upper_arm_length * 0.5,
        race.limb_thickness * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        race.shoulder_width_offset,
        shoulder_y - race.upper_arm_length * 0.5,
        0.0,
    ));
    let upper_arm_r_h = physics.collider_set.insert_with_parent(
        upper_arm_r_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Forearm R Collider
    let elbow_r_col = ColliderBuilder::cuboid(
        race.limb_thickness * 0.9 * 0.5,
        race.forearm_length * 0.5,
        race.limb_thickness * 0.9 * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        race.shoulder_width_offset,
        shoulder_y - race.upper_arm_length - race.forearm_length * 0.5,
        0.0,
    ));
    let elbow_r_h = physics.collider_set.insert_with_parent(
        elbow_r_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Hand R Collider
    let hand_r_col = ColliderBuilder::cuboid(
        race.limb_thickness * 0.95 * 0.5,
        0.12 * 0.5,
        race.limb_thickness * 1.1 * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        race.shoulder_width_offset,
        shoulder_y - race.upper_arm_length - race.forearm_length - 0.04,
        0.0,
    ));
    let hand_r_h = physics.collider_set.insert_with_parent(
        hand_r_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Upper Leg L Collider
    let upper_leg_l_col = ColliderBuilder::cuboid(
        race.limb_thickness * 1.15 * 0.5,
        race.upper_leg_length * 0.5,
        race.limb_thickness * 1.15 * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        -race.hip_width_offset,
        hip_y - race.upper_leg_length * 0.5,
        0.0,
    ));
    let upper_leg_l_h = physics.collider_set.insert_with_parent(
        upper_leg_l_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Lower Leg L Collider
    let knee_l_col = ColliderBuilder::cuboid(
        race.limb_thickness * 0.5,
        race.lower_leg_length * 0.5,
        race.limb_thickness * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        -race.hip_width_offset,
        hip_y - race.upper_leg_length - race.lower_leg_length * 0.5,
        0.0,
    ));
    let knee_l_h = physics.collider_set.insert_with_parent(
        knee_l_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Foot L Collider
    let foot_l_col = ColliderBuilder::cuboid(
        race.limb_thickness * 1.05 * 0.5,
        0.10 * 0.5,
        0.24 * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        -race.hip_width_offset,
        hip_y - race.upper_leg_length - race.lower_leg_length - 0.04,
        -0.05,
    ));
    let foot_l_h = physics.collider_set.insert_with_parent(
        foot_l_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Upper Leg R Collider
    let upper_leg_r_col = ColliderBuilder::cuboid(
        race.limb_thickness * 1.15 * 0.5,
        race.upper_leg_length * 0.5,
        race.limb_thickness * 1.15 * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        race.hip_width_offset,
        hip_y - race.upper_leg_length * 0.5,
        0.0,
    ));
    let upper_leg_r_h = physics.collider_set.insert_with_parent(
        upper_leg_r_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Lower Leg R Collider
    let knee_r_col = ColliderBuilder::cuboid(
        race.limb_thickness * 0.5,
        race.lower_leg_length * 0.5,
        race.limb_thickness * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        race.hip_width_offset,
        hip_y - race.upper_leg_length - race.lower_leg_length * 0.5,
        0.0,
    ));
    let knee_r_h = physics.collider_set.insert_with_parent(
        knee_r_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    // Foot R Collider
    let foot_r_col = ColliderBuilder::cuboid(
        race.limb_thickness * 1.05 * 0.5,
        0.10 * 0.5,
        0.24 * 0.5,
    )
    .translation(rapier3d::na::Vector3::new(
        race.hip_width_offset,
        hip_y - race.upper_leg_length - race.lower_leg_length - 0.04,
        -0.05,
    ));
    let foot_r_h = physics.collider_set.insert_with_parent(
        foot_r_col,
        root_rb_handle,
        &mut physics.rigid_body_set,
    );

    commands.entity(parent_entity).with_children(|root| {
        let r_id = root
            .spawn((
                SpatialBundle::from_transform(BevyTransform::from_xyz(0.0, -0.15, 0.0)),
                CharacterModelRoot,
                RbHandle(root_rb_handle),
                RTSProxy,
            ))
            .with_children(|model_parent| {
                // 1. TORSO PIVOT (Central skeletal anchor pivot - UNIFORM SCALE 1.0)
                model_parent
                    .spawn((
                        SpatialBundle::from_transform(BevyTransform::IDENTITY),
                        JointType::Torso,
                        JointOwner(parent_entity),
                        ColHandle(torso_col_h),
                    ))
                    .with_children(|torso| {
                        // Torso Visual Mesh (Leaf child scaled to torso_size)
                        torso.spawn((
                            PbrBundle {
                                mesh: unit_cuboid.clone(),
                                material: cloth_mat.clone(),
                                transform: BevyTransform {
                                    translation: Vec3::ZERO,
                                    scale: race.torso_size,
                                    ..default()
                                },
                                ..default()
                            },
                            LimbMeshSegment(JointType::Torso),
                            layers.clone(),
                        ));

                        // 2. HEAD PIVOT
                        let head_pivot_y = race.torso_size.y * 0.5 + 0.16 * race.head_scale;
                        torso
                            .spawn((
                                SpatialBundle::from_transform(BevyTransform {
                                    translation: Vec3::new(0.0, head_pivot_y, 0.0),
                                    scale: Vec3::splat(race.head_scale),
                                    ..default()
                                }),
                                JointType::Head,
                                JointOwner(parent_entity),
                                ColHandle(head_col_h),
                            ))
                            .with_children(|head_pivot| {
                                // Cranial Skull Base (proportional human head: 0.24m wide, 0.26m high, 0.26m deep)
                                head_pivot.spawn((
                                    PbrBundle {
                                        mesh: unit_cuboid.clone(),
                                        material: skin_mat.clone(),
                                        transform: BevyTransform {
                                            translation: Vec3::new(0.0, 0.08, 0.0),
                                            scale: Vec3::new(0.24, 0.26, 0.26),
                                            ..default()
                                        },
                                        ..default()
                                    },
                                    layers.clone(),
                                ));
                                // Raw Procedural Face (Pinned flush to anterior skull face: Z = -0.13)
                                head_pivot.spawn((
                                    PbrBundle {
                                        mesh: face_mesh,
                                        material: skin_mat.clone(),
                                        transform: BevyTransform::from_xyz(0.0, 0.08, -0.13),
                                        ..default()
                                    },
                                    ProceduralFaceMesh,
                                    layers.clone(),
                                ));
                            });

                        // 3. LEFT ARM (Shoulder -> UpperArm -> Elbow -> Forearm -> Hand)
                        torso
                            .spawn((
                                SpatialBundle::from_transform(BevyTransform::from_xyz(
                                    -race.shoulder_width_offset,
                                    shoulder_y,
                                    0.0,
                                )),
                                JointType::ShoulderL,
                                JointOwner(parent_entity),
                                ColHandle(upper_arm_l_h),
                            ))
                            .with_children(|shoulder_l| {
                                // Upper Arm mesh
                                shoulder_l.spawn((
                                    PbrBundle {
                                        mesh: unit_cuboid.clone(),
                                        material: cloth_mat.clone(),
                                        transform: BevyTransform {
                                            translation: Vec3::new(0.0, -race.upper_arm_length * 0.5, 0.0),
                                            scale: Vec3::new(race.limb_thickness, race.upper_arm_length, race.limb_thickness),
                                            ..default()
                                        },
                                        ..default()
                                    },
                                    LimbMeshSegment(JointType::ShoulderL),
                                    layers.clone(),
                                ));
                                // Pauldron / Spikes
                                if race.pauldron_size > 0.01 {
                                    shoulder_l.spawn((
                                        PbrBundle {
                                            mesh: unit_cuboid.clone(),
                                            material: armor_mat.clone(),
                                            transform: BevyTransform {
                                                translation: Vec3::new(-0.02, 0.04, 0.0),
                                                scale: Vec3::new(race.pauldron_size * 1.3, race.pauldron_size * 0.7, race.pauldron_size * 1.3),
                                                ..default()
                                            },
                                            ..default()
                                        },
                                        layers.clone(),
                                    ));
                                    if race.has_shoulder_spikes {
                                        shoulder_l.spawn((
                                            PbrBundle {
                                                mesh: spike_mesh.clone(),
                                                material: armor_mat.clone(),
                                                transform: BevyTransform::from_xyz(-0.02, 0.08 + race.pauldron_size * 0.35, 0.0),
                                                ..default()
                                            },
                                            layers.clone(),
                                        ));
                                    }
                                }
                                // Elbow L Pivot
                                shoulder_l
                                    .spawn((
                                        SpatialBundle::from_transform(BevyTransform::from_xyz(
                                            0.0,
                                            -race.upper_arm_length,
                                            0.0,
                                        )),
                                        JointType::ElbowL,
                                        JointOwner(parent_entity),
                                        ColHandle(elbow_l_h),
                                    ))
                                    .with_children(|elbow_l| {
                                        // Forearm mesh
                                        elbow_l.spawn((
                                            PbrBundle {
                                                mesh: unit_cuboid.clone(),
                                                material: skin_mat.clone(),
                                                transform: BevyTransform {
                                                    translation: Vec3::new(0.0, -race.forearm_length * 0.5, 0.0),
                                                    scale: Vec3::new(race.limb_thickness * 0.9, race.forearm_length, race.limb_thickness * 0.9),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            LimbMeshSegment(JointType::ElbowL),
                                            layers.clone(),
                                        ));
                                        // Hand L (Socket)
                                        elbow_l.spawn((
                                            PbrBundle {
                                                mesh: unit_cuboid.clone(),
                                                material: skin_mat.clone(),
                                                transform: BevyTransform {
                                                    translation: Vec3::new(0.0, -race.forearm_length - 0.04, 0.0),
                                                    scale: Vec3::new(race.limb_thickness * 0.95, 0.12, race.limb_thickness * 1.1),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            JointType::HandL,
                                            JointOwner(parent_entity),
                                            ColHandle(hand_l_h),
                                            layers.clone(),
                                        ));
                                    });
                            });

                        // 4. RIGHT ARM (Shoulder -> UpperArm -> Elbow -> Forearm -> Hand / Weapon Socket)
                        torso
                            .spawn((
                                SpatialBundle::from_transform(BevyTransform::from_xyz(
                                    race.shoulder_width_offset,
                                    shoulder_y,
                                    0.0,
                                )),
                                JointType::ShoulderR,
                                JointOwner(parent_entity),
                                ColHandle(upper_arm_r_h),
                            ))
                            .with_children(|shoulder_r| {
                                // Upper Arm mesh
                                shoulder_r.spawn((
                                    PbrBundle {
                                        mesh: unit_cuboid.clone(),
                                        material: cloth_mat.clone(),
                                        transform: BevyTransform {
                                            translation: Vec3::new(0.0, -race.upper_arm_length * 0.5, 0.0),
                                            scale: Vec3::new(race.limb_thickness, race.upper_arm_length, race.limb_thickness),
                                            ..default()
                                        },
                                        ..default()
                                    },
                                    LimbMeshSegment(JointType::ShoulderR),
                                    layers.clone(),
                                ));
                                // Pauldron / Spikes
                                if race.pauldron_size > 0.01 {
                                    shoulder_r.spawn((
                                        PbrBundle {
                                            mesh: unit_cuboid.clone(),
                                            material: armor_mat.clone(),
                                            transform: BevyTransform {
                                                translation: Vec3::new(0.02, 0.04, 0.0),
                                                scale: Vec3::new(race.pauldron_size * 1.3, race.pauldron_size * 0.7, race.pauldron_size * 1.3),
                                                ..default()
                                            },
                                            ..default()
                                        },
                                        layers.clone(),
                                    ));
                                    if race.has_shoulder_spikes {
                                        shoulder_r.spawn((
                                            PbrBundle {
                                                mesh: spike_mesh.clone(),
                                                material: armor_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.02, 0.08 + race.pauldron_size * 0.35, 0.0),
                                                ..default()
                                            },
                                            layers.clone(),
                                        ));
                                    }
                                }
                                // Elbow R Pivot
                                shoulder_r
                                    .spawn((
                                        SpatialBundle::from_transform(BevyTransform::from_xyz(
                                            0.0,
                                            -race.upper_arm_length,
                                            0.0,
                                        )),
                                        JointType::ElbowR,
                                        JointOwner(parent_entity),
                                        ColHandle(elbow_r_h),
                                    ))
                                    .with_children(|elbow_r| {
                                        // Forearm mesh
                                        elbow_r.spawn((
                                            PbrBundle {
                                                mesh: unit_cuboid.clone(),
                                                material: skin_mat.clone(),
                                                transform: BevyTransform {
                                                    translation: Vec3::new(0.0, -race.forearm_length * 0.5, 0.0),
                                                    scale: Vec3::new(race.limb_thickness * 0.9, race.forearm_length, race.limb_thickness * 0.9),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            LimbMeshSegment(JointType::ElbowR),
                                            layers.clone(),
                                        ));
                                        // Hand R (Weapon Socket)
                                        elbow_r.spawn((
                                            PbrBundle {
                                                mesh: unit_cuboid.clone(),
                                                material: skin_mat.clone(),
                                                transform: BevyTransform {
                                                    translation: Vec3::new(0.0, -race.forearm_length - 0.04, 0.0),
                                                    scale: Vec3::new(race.limb_thickness * 0.95, 0.12, race.limb_thickness * 1.1),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            JointType::HandR,
                                            JointOwner(parent_entity),
                                            ColHandle(hand_r_h),
                                            layers.clone(),
                                        ));
                                    });
                            });

                        // 5. LEFT LEG (Hip -> UpperLeg -> Knee -> LowerLeg -> Foot)
                        torso
                            .spawn((
                                SpatialBundle::from_transform(BevyTransform::from_xyz(
                                    -race.hip_width_offset,
                                    hip_y,
                                    0.0,
                                )),
                                JointType::HipL,
                                JointOwner(parent_entity),
                                ColHandle(upper_leg_l_h),
                            ))
                            .with_children(|hip_l| {
                                hip_l.spawn((
                                    PbrBundle {
                                        mesh: unit_cuboid.clone(),
                                        material: pants_mat.clone(),
                                        transform: BevyTransform {
                                            translation: Vec3::new(0.0, -race.upper_leg_length * 0.5, 0.0),
                                            scale: Vec3::new(race.limb_thickness * 1.15, race.upper_leg_length, race.limb_thickness * 1.15),
                                            ..default()
                                        },
                                        ..default()
                                    },
                                    LimbMeshSegment(JointType::HipL),
                                    layers.clone(),
                                ));
                                hip_l
                                    .spawn((
                                        SpatialBundle::from_transform(BevyTransform::from_xyz(
                                            0.0,
                                            -race.upper_leg_length,
                                            0.0,
                                        )),
                                        JointType::KneeL,
                                        JointOwner(parent_entity),
                                        ColHandle(knee_l_h),
                                    ))
                                    .with_children(|knee_l| {
                                        knee_l.spawn((
                                            PbrBundle {
                                                mesh: unit_cuboid.clone(),
                                                material: pants_mat.clone(),
                                                transform: BevyTransform {
                                                    translation: Vec3::new(0.0, -race.lower_leg_length * 0.5, 0.0),
                                                    scale: Vec3::new(race.limb_thickness, race.lower_leg_length, race.limb_thickness),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            LimbMeshSegment(JointType::KneeL),
                                            layers.clone(),
                                        ));
                                        knee_l.spawn((
                                            PbrBundle {
                                                mesh: unit_cuboid.clone(),
                                                material: pants_mat.clone(),
                                                transform: BevyTransform {
                                                    translation: Vec3::new(0.0, -race.lower_leg_length - 0.04, -0.05),
                                                    scale: Vec3::new(race.limb_thickness * 1.05, 0.10, 0.24),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            JointType::FootL,
                                            JointOwner(parent_entity),
                                            ColHandle(foot_l_h),
                                            layers.clone(),
                                        ));
                                    });
                            });

                        // 6. RIGHT LEG (Hip -> UpperLeg -> Knee -> LowerLeg -> Foot)
                        torso
                            .spawn((
                                SpatialBundle::from_transform(BevyTransform::from_xyz(
                                    race.hip_width_offset,
                                    hip_y,
                                    0.0,
                                )),
                                JointType::HipR,
                                JointOwner(parent_entity),
                                ColHandle(upper_leg_r_h),
                            ))
                            .with_children(|hip_r| {
                                hip_r.spawn((
                                    PbrBundle {
                                        mesh: unit_cuboid.clone(),
                                        material: pants_mat.clone(),
                                        transform: BevyTransform {
                                            translation: Vec3::new(0.0, -race.upper_leg_length * 0.5, 0.0),
                                            scale: Vec3::new(race.limb_thickness * 1.15, race.upper_leg_length, race.limb_thickness * 1.15),
                                            ..default()
                                        },
                                        ..default()
                                    },
                                    LimbMeshSegment(JointType::HipR),
                                    layers.clone(),
                                ));
                                hip_r
                                    .spawn((
                                        SpatialBundle::from_transform(BevyTransform::from_xyz(
                                            0.0,
                                            -race.upper_leg_length,
                                            0.0,
                                        )),
                                        JointType::KneeR,
                                        JointOwner(parent_entity),
                                        ColHandle(knee_r_h),
                                    ))
                                    .with_children(|knee_r| {
                                        knee_r.spawn((
                                            PbrBundle {
                                                mesh: unit_cuboid.clone(),
                                                material: pants_mat.clone(),
                                                transform: BevyTransform {
                                                    translation: Vec3::new(0.0, -race.lower_leg_length * 0.5, 0.0),
                                                    scale: Vec3::new(race.limb_thickness, race.lower_leg_length, race.limb_thickness),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            LimbMeshSegment(JointType::KneeR),
                                            layers.clone(),
                                        ));
                                        knee_r.spawn((
                                            PbrBundle {
                                                mesh: unit_cuboid.clone(),
                                                material: pants_mat.clone(),
                                                transform: BevyTransform {
                                                    translation: Vec3::new(0.0, -race.lower_leg_length - 0.04, -0.05),
                                                    scale: Vec3::new(race.limb_thickness * 1.05, 0.10, 0.24),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            JointType::FootR,
                                            JointOwner(parent_entity),
                                            ColHandle(foot_r_h),
                                            layers.clone(),
                                        ));
                                    });
                            });
                    });
            })
            .id();
        model_root_id = r_id;
    });

    model_root_id
}

// ----------------------------------------------------------------------------
// 7. PROCEDURAL ANIMATION ENGINE
// ----------------------------------------------------------------------------

/// Applies mathematical Euler/Quaternion rotations to all articulated joint pivots.
/// Strictly implements walk/run oscillations, velocity-driven jumping, upper-body
/// action overrides (melee swings, bow aiming), and hit reaction flinches.
/// Caches trigonometric evaluations (phase_sin, rect_sin) and flattens chained
/// Quat multiplications into single Quat::from_euler(EulerRot::XYZ, ...) calls.
pub fn procedural_animator_system(
    time: Res<Time>,
    mut character_q: Query<(Entity, &mut AnimationState, Option<&LinearVelocity>, Has<PlayerBody>)>,
    editor_state: Res<CharacterEditorState>,
    mut joint_q: Query<(&JointType, &JointOwner, &ColHandle, &mut BevyTransform)>,
    mut physics: Option<ResMut<PhysicsWorld>>,
) {
    let dt = time.delta_seconds();

    for (_entity, mut state, linvel_opt, is_player) in character_q.iter_mut() {
        let (speed, is_grounded, vy) = if is_player && editor_state.is_open && editor_state.studio_preview_mode != StudioPreviewMode::LiveGameplay {
            // Studio preview simulation isolated strictly to the local player
            match editor_state.studio_preview_mode {
                StudioPreviewMode::Walk => (2.2, true, 0.0),
                StudioPreviewMode::Run => (5.5, true, 0.0),
                StudioPreviewMode::JumpAscend => (0.5, false, 4.5),
                StudioPreviewMode::JumpFall => (0.5, false, -5.0),
                StudioPreviewMode::MeleeSwing => (0.0, true, 0.0),
                StudioPreviewMode::BowAim => (0.0, true, 0.0),
                StudioPreviewMode::HitReact => (0.0, true, 0.0),
                _ => (0.0, true, 0.0),
            }
        } else {
            // Live gameplay physics
            let vel = linvel_opt.map_or(Vec3::ZERO, |v| v.0);
            let horizontal_speed = Vec2::new(vel.x, vel.z).length();
            (horizontal_speed, state.is_grounded, vel.y)
        };

        state.is_grounded = is_grounded;
        state.vertical_velocity = vy;

        // 1. Advance Gait Clock (Frequency scales dynamically: Walk = ~7.0 rad/s, Run = ~12.0 rad/s)
        if state.is_grounded && speed > 0.1 {
            let frequency = if speed > 4.0 { 12.0 } else { 7.0 };
            state.gait_phase = (state.gait_phase + dt * frequency) % std::f32::consts::TAU;
        } else {
            // Smoothly settle back to neutral idle
            state.gait_phase *= (1.0 - dt * 10.0).max(0.0);
            if state.gait_phase.abs() < 1e-3 {
                state.gait_phase = 0.0;
            }
        }

        // 2. Advance Action States (Melee Swing normalized progress timer)
        if let ActionState::MeleeSwing { ref mut timer, duration } = state.action {
            *timer += dt;
            if *timer >= duration {
                state.action = ActionState::None;
            }
        }

        // 3. Decay Hit Reaction Flinch
        if state.hit_react_timer > 0.0 {
            state.hit_react_timer = (state.hit_react_timer - dt * 5.0).max(0.0);
        }
    }

    for (joint, owner, col_handle, mut tf) in joint_q.iter_mut() {
        let Ok((_, state, linvel_opt, is_player)) = character_q.get(owner.0) else { continue; };
        let speed = if is_player && editor_state.is_open && editor_state.studio_preview_mode != StudioPreviewMode::LiveGameplay {
            match editor_state.studio_preview_mode {
                StudioPreviewMode::Walk => 2.2,
                StudioPreviewMode::Run => 5.5,
                StudioPreviewMode::JumpAscend | StudioPreviewMode::JumpFall => 0.5,
                _ => 0.0,
            }
        } else {
            let vel = linvel_opt.map_or(Vec3::ZERO, |v| v.0);
            Vec2::new(vel.x, vel.z).length()
        };

        // Cache single trigonometric evaluation and half-wave rectifications per joint update
        let phase = state.gait_phase;
        let phase_sin = phase.sin();
        let rect_sin_pos = phase_sin.max(0.0);
        let rect_sin_neg = (-phase_sin).max(0.0);

        let walk_run_blend = ((speed - 0.5) / 3.5).clamp(0.0, 1.0);
        let speed_factor = if state.is_grounded { (speed / 0.5).clamp(0.0, 1.0) } else { 1.0 };
        let amplitude = (0.35 + 0.35 * walk_run_blend) * speed_factor;

        match joint {
            // TORSO: Forward lean during sprints + Hit reaction snap
            JointType::Torso => {
                let sprint_lean = -0.15 * walk_run_blend;
                let hit_lean = state.hit_react_timer * 0.30;
                tf.rotation = Quat::from_euler(EulerRot::XYZ, sprint_lean + hit_lean, 0.0, 0.0);
            }

            // HEAD: Compensate torso lean + Hit reaction flinch
            JointType::Head => {
                let hit_snap = state.hit_react_timer * 0.25;
                tf.rotation = Quat::from_euler(EulerRot::XYZ, hit_snap, 0.0, 0.0);
            }

            // LOWER BODY: Locomotion vs Airborne Trajectory
            JointType::HipL => {
                if state.is_grounded {
                    let rot_x = phase_sin * amplitude;
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, rot_x, 0.0, 0.0);
                } else if state.vertical_velocity > 0.0 {
                    // Ascending jump: Knees tuck upward
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, -0.40, 0.0, 0.0);
                } else {
                    // Falling: Straighten downward
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.10, 0.0, 0.0);
                }
            }

            JointType::HipR => {
                if state.is_grounded {
                    let rot_x = -phase_sin * amplitude;
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, rot_x, 0.0, 0.0);
                } else if state.vertical_velocity > 0.0 {
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, -0.40, 0.0, 0.0);
                } else {
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.10, 0.0, 0.0);
                }
            }

            JointType::KneeL => {
                if state.is_grounded {
                    // Half-wave rectified backward bend: Knees bend naturally backwards
                    let bend = rect_sin_neg * (amplitude * 1.4);
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, bend, 0.0, 0.0);
                } else if state.vertical_velocity > 0.0 {
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.60, 0.0, 0.0);
                } else {
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, 0.0);
                }
            }

            JointType::KneeR => {
                if state.is_grounded {
                    let bend = rect_sin_pos * (amplitude * 1.4);
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, bend, 0.0, 0.0);
                } else if state.vertical_velocity > 0.0 {
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.60, 0.0, 0.0);
                } else {
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, 0.0);
                }
            }

            // UPPER BODY LAYER: Counterbalance swing OR Action overrides
            JointType::ShoulderL => {
                match &state.action {
                    ActionState::BowAim { pitch, .. } => {
                        // Left arm holds bow firmly pointed forward along aiming pitch
                        tf.rotation = Quat::from_euler(EulerRot::XYZ, -1.57 + pitch, 0.15, 0.0);
                    }
                    _ => {
                        if !state.is_grounded {
                            tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, 0.30);
                        } else {
                            let arm_swing = -phase_sin * (amplitude * 0.85);
                            let hit_jitter = state.hit_react_timer * 0.20;
                            tf.rotation = Quat::from_euler(EulerRot::XYZ, arm_swing, 0.0, 0.08 + hit_jitter);
                        }
                    }
                }
            }

            JointType::ElbowL => {
                match &state.action {
                    ActionState::BowAim { .. } => {
                        tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.05, 0.0, 0.0);
                    }
                    _ => {
                        let elbow_bend = rect_sin_pos * 0.40;
                        tf.rotation = Quat::from_euler(EulerRot::XYZ, elbow_bend, 0.0, 0.0);
                    }
                }
            }

            JointType::ShoulderR => {
                match &state.action {
                    ActionState::MeleeSwing { timer, duration } => {
                        let t = (timer / duration.max(0.001)).clamp(0.0, 1.0);
                        if t < 0.30 {
                            let p = t / 0.30;
                            let rot_x = 0.0 + (-1.20 - 0.0) * p;
                            tf.rotation = Quat::from_euler(EulerRot::XYZ, rot_x, 0.40, 0.0);
                        } else if t < 0.50 {
                            let p = (t - 0.30) / 0.20;
                            let rot_x = -1.20 + (0.80 - -1.20) * p;
                            tf.rotation = Quat::from_euler(EulerRot::XYZ, rot_x, -0.60, 0.0);
                        } else {
                            let p = (t - 0.50) / 0.50;
                            let rot_x = 0.80 + (0.0 - 0.80) * p;
                            tf.rotation = Quat::from_euler(EulerRot::XYZ, rot_x, -0.60 * (1.0 - p), 0.0);
                        }
                    }
                    ActionState::BowAim { .. } => {
                        tf.rotation = Quat::from_euler(EulerRot::XYZ, -1.20, 0.35, 0.0);
                    }
                    _ => {
                        if !state.is_grounded {
                            tf.rotation = Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, -0.30);
                        } else {
                            let arm_swing = phase_sin * (amplitude * 0.85);
                            let hit_jitter = state.hit_react_timer * -0.20;
                            tf.rotation = Quat::from_euler(EulerRot::XYZ, arm_swing, 0.0, -0.08 + hit_jitter);
                        }
                    }
                }

                // Sync swinging arm rotation down to Rapier collider for precise hit-detection
                if let Some(ref mut phys) = physics {
                    if let Some(col) = phys.collider_set.get_mut(col_handle.0) {
                        let cur_pos = col.position_wrt_parent().map_or(rapier3d::na::Isometry3::identity(), |iso| *iso);
                        let rapier_rot = rapier3d::na::UnitQuaternion::new_normalize(
                            rapier3d::na::Quaternion::new(tf.rotation.w, tf.rotation.x, tf.rotation.y, tf.rotation.z)
                        );
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::from_parts(
                            cur_pos.translation,
                            rapier_rot,
                        ));
                    }
                }
            }

            JointType::ElbowR => {
                match &state.action {
                    ActionState::MeleeSwing { timer, duration } => {
                        let t = (timer / duration.max(0.001)).clamp(0.0, 1.0);
                        let bend = if t < 0.30 { 0.80 } else if t < 0.50 { 0.25 } else { 0.10 };
                        tf.rotation = Quat::from_euler(EulerRot::XYZ, bend, 0.0, 0.0);
                    }
                    ActionState::BowAim { draw_progress, .. } => {
                        let draw_bend = 0.20 + (1.80 - 0.20) * draw_progress.clamp(0.0, 1.0);
                        tf.rotation = Quat::from_euler(EulerRot::XYZ, draw_bend, 0.0, 0.0);
                    }
                    _ => {
                        let elbow_bend = rect_sin_neg * 0.40;
                        tf.rotation = Quat::from_euler(EulerRot::XYZ, elbow_bend, 0.0, 0.0);
                    }
                }

                if let Some(ref mut phys) = physics {
                    if let Some(col) = phys.collider_set.get_mut(col_handle.0) {
                        let cur_pos = col.position_wrt_parent().map_or(rapier3d::na::Isometry3::identity(), |iso| *iso);
                        let rapier_rot = rapier3d::na::UnitQuaternion::new_normalize(
                            rapier3d::na::Quaternion::new(tf.rotation.w, tf.rotation.x, tf.rotation.y, tf.rotation.z)
                        );
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::from_parts(
                            cur_pos.translation,
                            rapier_rot,
                        ));
                    }
                }
            }

            _ => {}
        }
    }
}

/// Synchronizes gameplay combat states (melee swing, bow drawing, grounded state)
/// with the player's procedural AnimationState component.
pub fn sync_player_animation_state(
    weapon_state: Res<WeaponState>,
    swing_state: Res<SwingState>,
    cam_settings: Res<CharacterCameraSettings>,
    mut player_q: Query<(&mut AnimationState, &Kcc), With<PlayerBody>>,
) {
    let Ok((mut anim_state, kcc)) = player_q.get_single_mut() else { return; };
    anim_state.is_grounded = kcc.is_grounded;

    if swing_state.is_swinging {
        anim_state.action = ActionState::MeleeSwing {
            timer: swing_state.timer.elapsed_secs(),
            duration: swing_state.timer.duration().as_secs_f32(),
        };
    } else if weapon_state.current_weapon == WeaponType::Bow && weapon_state.bow_drawing {
        anim_state.action = ActionState::BowAim {
            draw_progress: weapon_state.bow_charge,
            pitch: cam_settings.current_pitch,
        };
    } else if let ActionState::MeleeSwing { .. } = anim_state.action {
        // Maintained until timer finishes
    } else {
        anim_state.action = ActionState::None;
    }
}

// ----------------------------------------------------------------------------
// 8. MANUAL PHYSICS STEPPING & TRANSFORM SYNCHRONIZATION
// ----------------------------------------------------------------------------

/// Manually drives the pure Rapier physics simulation pipeline in Bevy's FixedUpdate schedule.
/// After stepping, synchronizes the RigidBody translation to the corresponding Bevy Transform.
pub fn step_physics_world_system(
    mut physics: ResMut<PhysicsWorld>,
    mut rb_q: Query<(&RbHandle, &mut BevyTransform)>,
) {
    physics.step();

    for (rb_handle, mut transform) in rb_q.iter_mut() {
        if let Some(rb) = physics.rigid_body_set.get(rb_handle.0) {
            let pos = rb.translation();
            transform.translation = Vec3::new(pos.x, pos.y, pos.z);
            let rot = rb.rotation();
            transform.rotation = Quat::from_xyzw(rot.i, rot.j, rot.k, rot.w);
        }
    }
}

// ----------------------------------------------------------------------------
// 9. IN-PLACE HITBOX & VISUAL MORPHOLOGY MUTATION
// ----------------------------------------------------------------------------

/// Live character update system responding to Changed<PlayerCharacterCustomization>.
/// Spawns the character hierarchy initially if not present, and subsequently mutates
/// visual Transform::scale/translations and Rapier Collider SharedShapes in-place,
/// completely preventing memory leaks and entity reallocation churn.
pub fn live_character_update_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut physics: ResMut<PhysicsWorld>,
    player_q: Query<(Entity, &PlayerCharacterCustomization), (With<PlayerBody>, Changed<PlayerCharacterCustomization>)>,
    existing_root_q: Query<(Entity, &Parent), With<CharacterModelRoot>>,
    mut joint_q: Query<(&JointType, &JointOwner, &ColHandle, &mut BevyTransform)>,
    mut limb_mesh_q: Query<(&LimbMeshSegment, &mut BevyTransform), Without<JointType>>,
    face_mesh_q: Query<(&ProceduralFaceMesh, &Handle<Mesh>)>,
) {
    for (player_entity, custom) in player_q.iter() {
        let has_model = existing_root_q.iter().any(|(_, p)| p.get() == player_entity);
        if !has_model {
            spawn_procedural_character_hierarchy(
                &mut commands,
                player_entity,
                &mut meshes,
                &mut materials,
                &custom.anatomy,
                &custom.face,
                RenderLayers::layer(2),
                &mut physics,
            );
            continue;
        }

        let race = &custom.anatomy;

        // 1. Mutate visual joint transforms and Rapier compound hitboxes in-place
        for (joint, owner, col_handle, mut tf) in joint_q.iter_mut() {
            if owner.0 != player_entity {
                continue;
            }

            match joint {
                JointType::Torso => {
                    tf.scale = Vec3::ONE;
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.torso_size.x * 0.5,
                            race.torso_size.y * 0.5,
                            race.torso_size.z * 0.5,
                        ));
                    }
                }
                JointType::Head => {
                    tf.translation.y = race.torso_size.y * 0.5 + 0.16 * race.head_scale;
                    tf.scale = Vec3::splat(race.head_scale);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            0.24 * race.head_scale * 0.5,
                            0.26 * race.head_scale * 0.5,
                            0.26 * race.head_scale * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            0.0,
                            race.torso_size.y * 0.5 + 0.22 * race.head_scale,
                            0.0,
                        ));
                    }
                }
                JointType::ShoulderL => {
                    tf.translation = Vec3::new(-race.shoulder_width_offset, race.torso_size.y * 0.5 - 0.08, 0.0);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 0.5,
                            race.upper_arm_length * 0.5,
                            race.limb_thickness * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            -race.shoulder_width_offset,
                            race.torso_size.y * 0.5 - 0.08 - race.upper_arm_length * 0.5,
                            0.0,
                        ));
                    }
                }
                JointType::ShoulderR => {
                    tf.translation = Vec3::new(race.shoulder_width_offset, race.torso_size.y * 0.5 - 0.08, 0.0);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 0.5,
                            race.upper_arm_length * 0.5,
                            race.limb_thickness * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            race.shoulder_width_offset,
                            race.torso_size.y * 0.5 - 0.08 - race.upper_arm_length * 0.5,
                            0.0,
                        ));
                    }
                }
                JointType::ElbowL => {
                    tf.translation = Vec3::new(0.0, -race.upper_arm_length, 0.0);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 0.9 * 0.5,
                            race.forearm_length * 0.5,
                            race.limb_thickness * 0.9 * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            -race.shoulder_width_offset,
                            race.torso_size.y * 0.5 - 0.08 - race.upper_arm_length - race.forearm_length * 0.5,
                            0.0,
                        ));
                    }
                }
                JointType::ElbowR => {
                    tf.translation = Vec3::new(0.0, -race.upper_arm_length, 0.0);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 0.9 * 0.5,
                            race.forearm_length * 0.5,
                            race.limb_thickness * 0.9 * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            race.shoulder_width_offset,
                            race.torso_size.y * 0.5 - 0.08 - race.upper_arm_length - race.forearm_length * 0.5,
                            0.0,
                        ));
                    }
                }
                JointType::HandL => {
                    tf.translation = Vec3::new(0.0, -race.forearm_length - 0.04, 0.0);
                    tf.scale = Vec3::new(race.limb_thickness * 0.95, 0.12, race.limb_thickness * 1.1);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 0.95 * 0.5,
                            0.12 * 0.5,
                            race.limb_thickness * 1.1 * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            -race.shoulder_width_offset,
                            race.torso_size.y * 0.5 - 0.08 - race.upper_arm_length - race.forearm_length - 0.04,
                            0.0,
                        ));
                    }
                }
                JointType::HandR => {
                    tf.translation = Vec3::new(0.0, -race.forearm_length - 0.04, 0.0);
                    tf.scale = Vec3::new(race.limb_thickness * 0.95, 0.12, race.limb_thickness * 1.1);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 0.95 * 0.5,
                            0.12 * 0.5,
                            race.limb_thickness * 1.1 * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            race.shoulder_width_offset,
                            race.torso_size.y * 0.5 - 0.08 - race.upper_arm_length - race.forearm_length - 0.04,
                            0.0,
                        ));
                    }
                }
                JointType::HipL => {
                    tf.translation = Vec3::new(-race.hip_width_offset, -race.torso_size.y * 0.5, 0.0);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 1.15 * 0.5,
                            race.upper_leg_length * 0.5,
                            race.limb_thickness * 1.15 * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            -race.hip_width_offset,
                            -race.torso_size.y * 0.5 - race.upper_leg_length * 0.5,
                            0.0,
                        ));
                    }
                }
                JointType::HipR => {
                    tf.translation = Vec3::new(race.hip_width_offset, -race.torso_size.y * 0.5, 0.0);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 1.15 * 0.5,
                            race.upper_leg_length * 0.5,
                            race.limb_thickness * 1.15 * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            race.hip_width_offset,
                            -race.torso_size.y * 0.5 - race.upper_leg_length * 0.5,
                            0.0,
                        ));
                    }
                }
                JointType::KneeL => {
                    tf.translation = Vec3::new(0.0, -race.upper_leg_length, 0.0);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 0.5,
                            race.lower_leg_length * 0.5,
                            race.limb_thickness * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            -race.hip_width_offset,
                            -race.torso_size.y * 0.5 - race.upper_leg_length - race.lower_leg_length * 0.5,
                            0.0,
                        ));
                    }
                }
                JointType::KneeR => {
                    tf.translation = Vec3::new(0.0, -race.upper_leg_length, 0.0);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 0.5,
                            race.lower_leg_length * 0.5,
                            race.limb_thickness * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            race.hip_width_offset,
                            -race.torso_size.y * 0.5 - race.upper_leg_length - race.lower_leg_length * 0.5,
                            0.0,
                        ));
                    }
                }
                JointType::FootL => {
                    tf.translation = Vec3::new(0.0, -race.lower_leg_length - 0.04, -0.05);
                    tf.scale = Vec3::new(race.limb_thickness * 1.05, 0.10, 0.24);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 1.05 * 0.5,
                            0.10 * 0.5,
                            0.24 * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            -race.hip_width_offset,
                            -race.torso_size.y * 0.5 - race.upper_leg_length - race.lower_leg_length - 0.04,
                            -0.05,
                        ));
                    }
                }
                JointType::FootR => {
                    tf.translation = Vec3::new(0.0, -race.lower_leg_length - 0.04, -0.05);
                    tf.scale = Vec3::new(race.limb_thickness * 1.05, 0.10, 0.24);
                    if let Some(col) = physics.collider_set.get_mut(col_handle.0) {
                        col.set_shape(SharedShape::cuboid(
                            race.limb_thickness * 1.05 * 0.5,
                            0.10 * 0.5,
                            0.24 * 0.5,
                        ));
                        col.set_position_wrt_parent(rapier3d::na::Isometry3::translation(
                            race.hip_width_offset,
                            -race.torso_size.y * 0.5 - race.upper_leg_length - race.lower_leg_length - 0.04,
                            -0.05,
                        ));
                    }
                }
            }
        }

        // 2. Update visual limb mesh scales and offsets
        for (segment, mut tf) in limb_mesh_q.iter_mut() {
            match segment.0 {
                JointType::Torso => {
                    tf.scale = race.torso_size;
                }
                JointType::ShoulderL | JointType::ShoulderR => {
                    tf.translation = Vec3::new(0.0, -race.upper_arm_length * 0.5, 0.0);
                    tf.scale = Vec3::new(race.limb_thickness, race.upper_arm_length, race.limb_thickness);
                }
                JointType::ElbowL | JointType::ElbowR => {
                    tf.translation = Vec3::new(0.0, -race.forearm_length * 0.5, 0.0);
                    tf.scale = Vec3::new(race.limb_thickness * 0.9, race.forearm_length, race.limb_thickness * 0.9);
                }
                JointType::HipL | JointType::HipR => {
                    tf.translation = Vec3::new(0.0, -race.upper_leg_length * 0.5, 0.0);
                    tf.scale = Vec3::new(race.limb_thickness * 1.15, race.upper_leg_length, race.limb_thickness * 1.15);
                }
                JointType::KneeL | JointType::KneeR => {
                    tf.translation = Vec3::new(0.0, -race.lower_leg_length * 0.5, 0.0);
                    tf.scale = Vec3::new(race.limb_thickness, race.lower_leg_length, race.limb_thickness);
                }
                _ => {}
            }
        }

        // 3. Update procedural face mesh in-place without asset recreation
        for (_, mesh_handle) in face_mesh_q.iter() {
            if let Some(mesh) = meshes.get_mut(mesh_handle) {
                *mesh = generate_custom_face(&custom.face);
            }
        }

        info!("🎭 Mutated character anatomy and Rapier hitboxes in-place for Race: {}", race.race_name);
    }
}

// ----------------------------------------------------------------------------
// 10. IN-GAME CHARACTER CUSTOMIZER WORKBENCH UI
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTab {
    FaceSculpt,
    BodyAnatomy,
    WardrobeColors,
    AnimationStudio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StudioPreviewMode {
    LiveGameplay,
    Walk,
    Run,
    JumpAscend,
    JumpFall,
    MeleeSwing,
    BowAim,
    HitReact,
}

#[derive(Resource)]
pub struct CharacterEditorState {
    pub is_open: bool,
    pub current_tab: EditorTab,
    pub preset_index: usize,
    pub face: FaceProfile,
    pub anatomy: RaceAnatomyProfile,
    pub studio_preview_mode: StudioPreviewMode,
    pub studio_bow_draw: f32,
    pub previous_cam_distance: f32,
    /// Horizontal orbit angle in radians (0.0 = directly in front of character)
    pub studio_yaw: f32,
    /// Vertical elevation angle in radians (0.0 = level with target)
    pub studio_pitch: f32,
    /// Camera distance from target anchor point in meters
    pub studio_distance: f32,
    /// Vertical height offset of the inspection target in local space (meters)
    pub studio_target_height: f32,
}

impl Default for CharacterEditorState {
    fn default() -> Self {
        Self {
            is_open: false,
            current_tab: EditorTab::FaceSculpt,
            preset_index: 0,
            face: CHARACTER_PRESETS[0].face.clone(),
            anatomy: CHARACTER_PRESETS[0].anatomy.clone(),
            studio_preview_mode: StudioPreviewMode::LiveGameplay,
            studio_bow_draw: 0.0,
            previous_cam_distance: 0.0,
            studio_yaw: 0.0,
            studio_pitch: 0.0,
            studio_distance: 1.15,
            studio_target_height: 0.18,
        }
    }
}

impl CharacterEditorState {
    pub fn load_preset(&mut self, idx: usize) {
        if idx >= CHARACTER_PRESETS.len() { return; }
        self.preset_index = idx;
        self.face = CHARACTER_PRESETS[idx].face.clone();
        self.anatomy = CHARACTER_PRESETS[idx].anatomy.clone();
    }

    pub fn randomize(&mut self) {
        let p_idx = (self.preset_index + 1) % CHARACTER_PRESETS.len();
        self.preset_index = p_idx;
        let base = &CHARACTER_PRESETS[p_idx];
        self.face = base.face.clone();
        self.anatomy = base.anatomy.clone();
    }

    pub fn export_rust_code(&self) -> String {
        let skin = self.anatomy.skin_color.to_srgba();
        let cloth = self.anatomy.cloth_color.to_srgba();
        let pants = self.anatomy.pants_color.to_srgba();
        let armor = self.anatomy.armor_color.to_srgba();

        format!(
            "// ============================================================================\n\
             // Procedurally Generated Character Morphology Definition\n\
             // ============================================================================\n\
             pub const CUSTOM_CHARACTER_PRESET: CharacterPreset = CharacterPreset {{\n\
             \x20   name: \"{}\",\n\
             \x20   description: \"Custom user-tuned character profile exported from in-game studio.\",\n\
             \x20   face: FaceProfile {{\n\
             \x20       jaw_width: {:.3},\n\
             \x20       jaw_height: {:.3},\n\
             \x20       cheekbone_width: {:.3},\n\
             \x20       nose_bridge_length: {:.3},\n\
             \x20       nose_tip_z: {:.3},\n\
             \x20       brow_ridge: {:.3},\n\
             \x20       chin_forward: {:.3},\n\
             \x20       eye_depth: {:.3},\n\
             \x20   }},\n\
             \x20   anatomy: RaceAnatomyProfile {{\n\
             \x20       race_name: \"{}\",\n\
             \x20       torso_size: Vec3::new({:.2}, {:.2}, {:.2}),\n\
             \x20       shoulder_width_offset: {:.3},\n\
             \x20       hip_width_offset: {:.3},\n\
             \x20       upper_arm_length: {:.3},\n\
             \x20       forearm_length: {:.3},\n\
             \x20       upper_leg_length: {:.3},\n\
             \x20       lower_leg_length: {:.3},\n\
             \x20       limb_thickness: {:.3},\n\
             \x20       head_scale: {:.2},\n\
             \x20       skin_color: Color::srgb({:.2}, {:.2}, {:.2}),\n\
             \x20       cloth_color: Color::srgb({:.2}, {:.2}, {:.2}),\n\
             \x20       pants_color: Color::srgb({:.2}, {:.2}, {:.2}),\n\
             \x20       armor_color: Color::srgb({:.2}, {:.2}, {:.2}),\n\
             \x20       pauldron_size: {:.2},\n\
             \x20       has_shoulder_spikes: {},\n\
             \x20   }},\n\
             }};",
            self.anatomy.race_name,
            self.face.jaw_width,
            self.face.jaw_height,
            self.face.cheekbone_width,
            self.face.nose_bridge_length,
            self.face.nose_tip_z,
            self.face.brow_ridge,
            self.face.chin_forward,
            self.face.eye_depth,
            self.anatomy.race_name,
            self.anatomy.torso_size.x,
            self.anatomy.torso_size.y,
            self.anatomy.torso_size.z,
            self.anatomy.shoulder_width_offset,
            self.anatomy.hip_width_offset,
            self.anatomy.upper_arm_length,
            self.anatomy.forearm_length,
            self.anatomy.upper_leg_length,
            self.anatomy.lower_leg_length,
            self.anatomy.limb_thickness,
            self.anatomy.head_scale,
            skin.red, skin.green, skin.blue,
            cloth.red, cloth.green, cloth.blue,
            pants.red, pants.green, pants.blue,
            armor.red, armor.green, armor.blue,
            self.anatomy.pauldron_size,
            self.anatomy.has_shoulder_spikes,
        )
    }
}

// ----------------------------------------------------------------------------
// 11. UI COMPONENT TAGS & ACTIONS
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct CharacterEditorRoot;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorTabPanel(pub EditorTab);

#[derive(Component, Clone, Debug)]
pub enum EditorAction {
    SetTab(EditorTab),
    PrevPreset,
    NextPreset,
    AdjustFaceJawWidth(f32),
    AdjustFaceJawHeight(f32),
    AdjustFaceCheekbones(f32),
    AdjustFaceNoseBridge(f32),
    AdjustFaceNoseTipZ(f32),
    AdjustFaceBrowRidge(f32),
    AdjustFaceChinForward(f32),
    AdjustFaceEyeDepth(f32),
    AdjustTorsoWidth(f32),
    AdjustTorsoHeight(f32),
    AdjustShoulderWidth(f32),
    AdjustHipWidth(f32),
    AdjustArmLength(f32),
    AdjustLegLength(f32),
    AdjustLimbThickness(f32),
    AdjustHeadScale(f32),
    AdjustPauldronSize(f32),
    CycleSkinColor,
    CycleClothColor,
    CycleArmorColor,
    ToggleShoulderSpikes,
    SetStudioMode(StudioPreviewMode),
    TriggerMeleeSwing,
    TriggerBowAim,
    TriggerHitFlinch,
    RandomizeAll,
    ResetCurrentPreset,
    ExportCode,
    CloseEditor,
}

#[derive(Component)]
pub struct EditorValueDisplay(pub String);

// ----------------------------------------------------------------------------
// 12. UI SETUP & HIERARCHY SPAWNING
// ----------------------------------------------------------------------------

pub fn setup_character_editor_ui(mut commands: Commands) {
    let font_title = 15.0;
    let gold_header = Color::srgb(0.96, 0.76, 0.25);
    let obsidian_bg = Color::srgba(0.07, 0.08, 0.11, 0.94);
    let panel_border = Color::srgb(0.35, 0.38, 0.46);

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(24.0),
                    top: Val::Px(24.0),
                    width: Val::Px(450.0),
                    max_height: Val::Percent(92.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(12.0)),
                    row_gap: Val::Px(8.0),
                    border: UiRect::all(Val::Px(2.0)),
                    display: Display::None,
                    ..default()
                },
                background_color: obsidian_bg.into(),
                border_color: panel_border.into(),
                ..default()
            },
            CharacterEditorRoot,
        ))
        .with_children(|root| {
            // 1. Header (Title + Preset Picker + Close)
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::bottom(Val::Px(6.0)),
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                border_color: panel_border.into(),
                ..default()
            })
            .with_children(|header| {
                header.spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|col| {
                    col.spawn(TextBundle::from_section(
                        "🎭 CHARACTER STUDIO & MORPHOLOGY",
                        TextStyle {
                            font_size: font_title,
                            color: gold_header,
                            ..default()
                        },
                    ));
                    col.spawn(TextBundle::from_section(
                        "💡 [Hold RMB Drag] Orbit 360° | [Scroll] Zoom",
                        TextStyle {
                            font_size: 10.5,
                            color: Color::srgb(0.65, 0.72, 0.82),
                            ..default()
                        },
                    ));
                });

                spawn_editor_button(header, " [X] ", EditorAction::CloseEditor, 36.0, 24.0);
            });

            // Preset Navigation Row: [<] Preset: Human Adventurer [>]
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                },
                ..default()
            })
            .with_children(|picker| {
                spawn_editor_button(picker, "<", EditorAction::PrevPreset, 28.0, 24.0);
                picker.spawn((
                    EditorValueDisplay("preset_name".into()),
                    TextBundle::from_section(
                        "Preset: Human Adventurer",
                        TextStyle {
                            font_size: 13.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    ),
                ));
                spawn_editor_button(picker, ">", EditorAction::NextPreset, 28.0, 24.0);
            });

            // 2. Tab Navigation Bar
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                ..default()
            })
            .with_children(|tabs| {
                spawn_tab_button(tabs, "1. Face", EditorAction::SetTab(EditorTab::FaceSculpt));
                spawn_tab_button(tabs, "2. Anatomy", EditorAction::SetTab(EditorTab::BodyAnatomy));
                spawn_tab_button(tabs, "3. Wardrobe", EditorAction::SetTab(EditorTab::WardrobeColors));
                spawn_tab_button(tabs, "4. Studio", EditorAction::SetTab(EditorTab::AnimationStudio));
            });

            // 3a. Tab Panel: Face Sculpt
            root.spawn((
                NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(5.0),
                        padding: UiRect::vertical(Val::Px(4.0)),
                        display: Display::Flex,
                        ..default()
                    },
                    ..default()
                },
                EditorTabPanel(EditorTab::FaceSculpt),
            ))
            .with_children(|face_panel| {
                spawn_section_label(face_panel, "FACIAL MORPHOLOGY SCULPT");
                spawn_param_row(face_panel, "Jaw Width", EditorAction::AdjustFaceJawWidth(-0.02), EditorAction::AdjustFaceJawWidth(0.02), "face_jaw_width");
                spawn_param_row(face_panel, "Jaw Height", EditorAction::AdjustFaceJawHeight(-0.02), EditorAction::AdjustFaceJawHeight(0.02), "face_jaw_height");
                spawn_param_row(face_panel, "Cheekbones", EditorAction::AdjustFaceCheekbones(-0.02), EditorAction::AdjustFaceCheekbones(0.02), "face_cheekbones");
                spawn_param_row(face_panel, "Nose Bridge", EditorAction::AdjustFaceNoseBridge(-0.02), EditorAction::AdjustFaceNoseBridge(0.02), "face_nose_bridge");
                spawn_param_row(face_panel, "Nose Tip Depth", EditorAction::AdjustFaceNoseTipZ(-0.03), EditorAction::AdjustFaceNoseTipZ(0.03), "face_nose_tip_z");
                spawn_param_row(face_panel, "Brow Ridge", EditorAction::AdjustFaceBrowRidge(-0.02), EditorAction::AdjustFaceBrowRidge(0.02), "face_brow_ridge");
                spawn_param_row(face_panel, "Chin Forward", EditorAction::AdjustFaceChinForward(-0.02), EditorAction::AdjustFaceChinForward(0.02), "face_chin_forward");
                spawn_param_row(face_panel, "Eye Depth", EditorAction::AdjustFaceEyeDepth(-0.02), EditorAction::AdjustFaceEyeDepth(0.02), "face_eye_depth");
            });

            // 3b. Tab Panel: Body Anatomy
            root.spawn((
                NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(5.0),
                        padding: UiRect::vertical(Val::Px(4.0)),
                        display: Display::None,
                        ..default()
                    },
                    ..default()
                },
                EditorTabPanel(EditorTab::BodyAnatomy),
            ))
            .with_children(|body_panel| {
                spawn_section_label(body_panel, "SKELETAL PROPORTIONS & ANATOMY");
                spawn_param_row(body_panel, "Torso Width", EditorAction::AdjustTorsoWidth(-0.04), EditorAction::AdjustTorsoWidth(0.04), "torso_width");
                spawn_param_row(body_panel, "Torso Height", EditorAction::AdjustTorsoHeight(-0.04), EditorAction::AdjustTorsoHeight(0.04), "torso_height");
                spawn_param_row(body_panel, "Shoulder Width", EditorAction::AdjustShoulderWidth(-0.03), EditorAction::AdjustShoulderWidth(0.03), "shoulder_width");
                spawn_param_row(body_panel, "Hip Width", EditorAction::AdjustHipWidth(-0.02), EditorAction::AdjustHipWidth(0.02), "hip_width");
                spawn_param_row(body_panel, "Arm Length", EditorAction::AdjustArmLength(-0.03), EditorAction::AdjustArmLength(0.03), "arm_length");
                spawn_param_row(body_panel, "Leg Length", EditorAction::AdjustLegLength(-0.03), EditorAction::AdjustLegLength(0.03), "leg_length");
                spawn_param_row(body_panel, "Limb Thickness", EditorAction::AdjustLimbThickness(-0.02), EditorAction::AdjustLimbThickness(0.02), "limb_thickness");
                spawn_param_row(body_panel, "Head Scale", EditorAction::AdjustHeadScale(-0.04), EditorAction::AdjustHeadScale(0.04), "head_scale");
                spawn_param_row(body_panel, "Pauldron Size", EditorAction::AdjustPauldronSize(-0.03), EditorAction::AdjustPauldronSize(0.03), "pauldron_size");
            });

            // 3c. Tab Panel: Wardrobe & Colors
            root.spawn((
                NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(7.0),
                        padding: UiRect::vertical(Val::Px(4.0)),
                        display: Display::None,
                        ..default()
                    },
                    ..default()
                },
                EditorTabPanel(EditorTab::WardrobeColors),
            ))
            .with_children(|wardrobe_panel| {
                spawn_section_label(wardrobe_panel, "WARDROBE & PBR PALETTES");
                spawn_palette_row(wardrobe_panel, "Skin Complexion", EditorAction::CycleSkinColor, "skin_color_label");
                spawn_palette_row(wardrobe_panel, "Tunic & Cloth", EditorAction::CycleClothColor, "cloth_color_label");
                spawn_palette_row(wardrobe_panel, "Armor Material", EditorAction::CycleArmorColor, "armor_color_label");
                spawn_palette_row(wardrobe_panel, "Pauldrons & Spikes", EditorAction::ToggleShoulderSpikes, "spikes_label");
            });

            // 3d. Tab Panel: Animation Studio
            root.spawn((
                NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(7.0),
                        padding: UiRect::vertical(Val::Px(4.0)),
                        display: Display::None,
                        ..default()
                    },
                    ..default()
                },
                EditorTabPanel(EditorTab::AnimationStudio),
            ))
            .with_children(|studio_panel| {
                spawn_section_label(studio_panel, "PROCEDURAL GAIT & ACTION STUDIO");
                
                studio_panel.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|poses| {
                    spawn_editor_button(poses, "Live Gameplay", EditorAction::SetStudioMode(StudioPreviewMode::LiveGameplay), 95.0, 24.0);
                    spawn_editor_button(poses, "Walk (2.2m/s)", EditorAction::SetStudioMode(StudioPreviewMode::Walk), 95.0, 24.0);
                    spawn_editor_button(poses, "Run (5.5m/s)", EditorAction::SetStudioMode(StudioPreviewMode::Run), 95.0, 24.0);
                    spawn_editor_button(poses, "Jump Ascend", EditorAction::SetStudioMode(StudioPreviewMode::JumpAscend), 95.0, 24.0);
                    spawn_editor_button(poses, "Airborne Fall", EditorAction::SetStudioMode(StudioPreviewMode::JumpFall), 95.0, 24.0);
                });

                spawn_section_label(studio_panel, "COMBAT ACTION TRIGGERS");
                studio_panel.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(6.0),
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|actions| {
                    spawn_editor_button(actions, "⚔ Melee Swing", EditorAction::TriggerMeleeSwing, 120.0, 24.0);
                    spawn_editor_button(actions, "🏹 Bow Aim Stance", EditorAction::TriggerBowAim, 130.0, 24.0);
                    spawn_editor_button(actions, "💥 Hit Reaction", EditorAction::TriggerHitFlinch, 120.0, 24.0);
                });

                studio_panel.spawn(TextBundle::from_section(
                    "💡 Camera zoomed to 3rd person. Hold Alt + mouse to free-look orbit.",
                    TextStyle {
                        font_size: 11.0,
                        color: Color::srgb(0.70, 0.75, 0.85),
                        ..default()
                    },
                ));
            });

            // 4. Footer Action Bar (Randomize / Reset / Export Rust)
            root.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::top(Val::Px(6.0)),
                    border: UiRect::top(Val::Px(1.0)),
                    ..default()
                },
                border_color: panel_border.into(),
                ..default()
            })
            .with_children(|footer| {
                spawn_editor_button(footer, "🎲 Randomize", EditorAction::RandomizeAll, 100.0, 26.0);
                spawn_editor_button(footer, "↺ Reset", EditorAction::ResetCurrentPreset, 75.0, 26.0);
                spawn_editor_button(footer, "📋 Export Code", EditorAction::ExportCode, 115.0, 26.0);
                spawn_editor_button(footer, "✓ Close [F5]", EditorAction::CloseEditor, 95.0, 26.0);
            });
        });
}

// ----------------------------------------------------------------------------
// 13. UI HELPER SPAWNERS
// ----------------------------------------------------------------------------

fn spawn_editor_button(
    parent: &mut ChildBuilder,
    text: &str,
    action: EditorAction,
    w: f32,
    h: f32,
) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(w),
                    height: Val::Px(h),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                    ..default()
                },
                border_color: Color::srgb(0.40, 0.44, 0.52).into(),
                background_color: Color::srgb(0.16, 0.18, 0.24).into(),
                ..default()
            },
            action,
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                text,
                TextStyle {
                    font_size: 11.5,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
}

fn spawn_tab_button(parent: &mut ChildBuilder, text: &str, action: EditorAction) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    flex_grow: 1.0,
                    height: Val::Px(24.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                border_color: Color::srgb(0.40, 0.44, 0.52).into(),
                background_color: Color::srgb(0.20, 0.22, 0.30).into(),
                ..default()
            },
            action,
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                text,
                TextStyle {
                    font_size: 11.5,
                    color: Color::srgb(0.92, 0.88, 0.76),
                    ..default()
                },
            ));
        });
}

fn spawn_param_row(
    parent: &mut ChildBuilder,
    label: &str,
    dec_action: EditorAction,
    inc_action: EditorAction,
    val_key: &str,
) {
    parent
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|row| {
            row.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font_size: 11.5,
                    color: Color::srgb(0.85, 0.85, 0.88),
                    ..default()
                },
            ));

            row.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|ctrls| {
                spawn_editor_button(ctrls, "-", dec_action, 22.0, 20.0);
                ctrls.spawn((
                    EditorValueDisplay(val_key.into()),
                    TextBundle::from_section(
                        "0.00",
                        TextStyle {
                            font_size: 11.5,
                            color: Color::srgb(0.96, 0.76, 0.25),
                            ..default()
                        },
                    ),
                ));
                spawn_editor_button(ctrls, "+", inc_action, 22.0, 20.0);
            });
        });
}

fn spawn_section_label(parent: &mut ChildBuilder, text: &str) {
    parent.spawn(
        TextBundle::from_section(
            text,
            TextStyle {
                font_size: 11.0,
                color: Color::srgb(0.70, 0.78, 0.90),
                ..default()
            },
        )
        .with_style(Style {
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        }),
    );
}

fn spawn_palette_row(
    parent: &mut ChildBuilder,
    label: &str,
    cycle_action: EditorAction,
    val_key: &str,
) {
    parent
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|row| {
            row.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font_size: 11.5,
                    color: Color::srgb(0.85, 0.85, 0.88),
                    ..default()
                },
            ));

            row.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|ctrls| {
                ctrls.spawn((
                    EditorValueDisplay(val_key.into()),
                    TextBundle::from_section(
                        "Default",
                        TextStyle {
                            font_size: 11.5,
                            color: Color::srgb(0.96, 0.76, 0.25),
                            ..default()
                        },
                    ),
                ));
                spawn_editor_button(ctrls, "Cycle >", cycle_action, 64.0, 20.0);
            });
        });
}

// ----------------------------------------------------------------------------
// 14. TOGGLE & INTERACTION SYSTEMS
// ----------------------------------------------------------------------------

pub use crate::input::ToggleCharacterEditorEvent;

pub fn toggle_character_editor_ui(
    mut evts: EventReader<ToggleCharacterEditorEvent>,
    mut editor: ResMut<CharacterEditorState>,
    mut root_q: Query<&mut Style, With<CharacterEditorRoot>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    mut cam_settings: ResMut<CharacterCameraSettings>,
    camera_mode: Res<State<CameraMode>>,
) {
    for _ in evts.read() {
        editor.is_open = !editor.is_open;

        if let Ok(mut style) = root_q.get_single_mut() {
            style.display = if editor.is_open { Display::Flex } else { Display::None };
        }

        if let Ok(mut window) = window_q.get_single_mut() {
            if editor.is_open {
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;

                // Configure studio camera parameters
                editor.previous_cam_distance = cam_settings.target_distance;
                editor.studio_yaw = 0.0;
                editor.studio_pitch = 0.0;
                if editor.current_tab == EditorTab::FaceSculpt {
                    editor.studio_distance = 1.15;
                    editor.studio_target_height = 0.38;
                } else {
                    editor.studio_distance = 2.80;
                    editor.studio_target_height = -0.15;
                }
                info!("🎭 Opened Character Model Customizer Studio [F5]. Studio camera orbit active.");
            } else {
                if *camera_mode.get() == CameraMode::FPS {
                    window.cursor.grab_mode = CursorGrabMode::Locked;
                    window.cursor.visible = false;
                }
                cam_settings.target_distance = editor.previous_cam_distance;
                info!("🎭 Closed Character Model Customizer Studio.");
            }
        }
    }
}

/// Smoothly orbits and frames the camera around the player character in Studio inspection mode.
/// Supports 360° orbital rotation via Right Mouse Button drag and continuous zoom via mouse wheel.
/// Automatically adjusts framing distance and focal height between face close-up and full-body.
pub fn studio_camera_orbit_system(
    time: Res<Time>,
    mut editor: ResMut<CharacterEditorState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut scroll_evts: EventReader<MouseWheel>,
    mut head_q: Query<(&mut BevyTransform, &mut RenderLayers), With<PlayerHead>>,
) {
    if !editor.is_open {
        return;
    }

    let dt = time.delta_seconds();
    let Ok((mut head_transform, mut head_layers)) = head_q.get_single_mut() else { return; };

    // 1. Ensure 3rd-person render layers so character model is visible
    let studio_layers = RenderLayers::from_layers(&[0, 2]);
    if *head_layers != studio_layers {
        *head_layers = studio_layers;
    }

    // 2. Mouse orbit controls when Right Mouse Button is held
    if mouse_buttons.pressed(MouseButton::Right) {
        for ev in mouse_motion.read() {
            if ev.delta.length_squared() > 1e-4 {
                editor.studio_yaw += ev.delta.x * 0.007;
                editor.studio_pitch = (editor.studio_pitch - ev.delta.y * 0.007).clamp(-0.85, 0.85);
            }
        }
    } else {
        mouse_motion.clear();
    }

    // 3. Mouse wheel zoom
    for ev in scroll_evts.read() {
        if ev.y.abs() > 1e-3 {
            editor.studio_distance = (editor.studio_distance - ev.y * 0.25).clamp(0.65, 5.0);
        }
    }

    // 4. Smoothly interpolate camera position and orientation in PlayerBody local space
    // In PlayerBody space, the character faces along local -Z.
    // The camera is placed in front along -Z relative to the focal point.
    let orbit_rot = Quat::from_euler(EulerRot::YXZ, editor.studio_yaw, editor.studio_pitch, 0.0);
    let target_local = Vec3::new(0.0, editor.studio_target_height, 0.0);
    let cam_offset = orbit_rot * Vec3::new(0.0, 0.0, -editor.studio_distance);
    let desired_cam_pos = target_local + cam_offset;
    let desired_cam_rot = BevyTransform::from_translation(desired_cam_pos)
        .looking_at(target_local, Vec3::Y)
        .rotation;

    let t = (14.0 * dt).min(1.0);
    head_transform.translation = head_transform.translation.lerp(desired_cam_pos, t);
    head_transform.rotation = head_transform.rotation.slerp(desired_cam_rot, t);
}

pub fn handle_character_editor_interactions(
    mut interaction_q: Query<(&Interaction, &EditorAction), (Changed<Interaction>, With<Button>)>,
    mut editor: ResMut<CharacterEditorState>,
    mut player_custom_q: Query<&mut PlayerCharacterCustomization, With<PlayerBody>>,
    mut player_anim_q: Query<&mut AnimationState, With<PlayerBody>>,
    mut toggle_evts: EventWriter<ToggleCharacterEditorEvent>,
) {
    for (interaction, action) in interaction_q.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let mut mark_dirty = false;

        match action {
            EditorAction::SetTab(tab) => {
                editor.current_tab = *tab;
                match tab {
                    EditorTab::FaceSculpt => {
                        editor.studio_distance = 1.15;
                        editor.studio_target_height = 0.38;
                        editor.studio_yaw = 0.0;
                        editor.studio_pitch = 0.0;
                    }
                    EditorTab::BodyAnatomy | EditorTab::WardrobeColors | EditorTab::AnimationStudio => {
                        editor.studio_distance = 2.80;
                        editor.studio_target_height = -0.15;
                        editor.studio_yaw = 0.0;
                        editor.studio_pitch = 0.0;
                    }
                }
            }
            EditorAction::PrevPreset => {
                let count = CHARACTER_PRESETS.len();
                let next_idx = (editor.preset_index + count - 1) % count;
                editor.load_preset(next_idx);
                mark_dirty = true;
            }
            EditorAction::NextPreset => {
                let count = CHARACTER_PRESETS.len();
                let next_idx = (editor.preset_index + 1) % count;
                editor.load_preset(next_idx);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceJawWidth(d) => {
                editor.face.jaw_width = (editor.face.jaw_width + d).clamp(0.05, 0.22);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceJawHeight(d) => {
                editor.face.jaw_height = (editor.face.jaw_height + d).clamp(-0.25, -0.06);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceCheekbones(d) => {
                editor.face.cheekbone_width = (editor.face.cheekbone_width + d).clamp(0.08, 0.22);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceNoseBridge(d) => {
                editor.face.nose_bridge_length = (editor.face.nose_bridge_length + d).clamp(0.05, 0.20);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceNoseTipZ(d) => {
                editor.face.nose_tip_z = (editor.face.nose_tip_z + d).clamp(0.02, 0.16);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceBrowRidge(d) => {
                editor.face.brow_ridge = (editor.face.brow_ridge + d).clamp(0.01, 0.10);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceChinForward(d) => {
                editor.face.chin_forward = (editor.face.chin_forward + d).clamp(0.01, 0.12);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceEyeDepth(d) => {
                editor.face.eye_depth = (editor.face.eye_depth + d).clamp(0.00, 0.06);
                mark_dirty = true;
            }
            EditorAction::AdjustTorsoWidth(d) => {
                editor.anatomy.torso_size.x = (editor.anatomy.torso_size.x + d).clamp(0.25, 0.80);
                mark_dirty = true;
            }
            EditorAction::AdjustTorsoHeight(d) => {
                editor.anatomy.torso_size.y = (editor.anatomy.torso_size.y + d).clamp(0.35, 0.90);
                mark_dirty = true;
            }
            EditorAction::AdjustShoulderWidth(d) => {
                editor.anatomy.shoulder_width_offset = (editor.anatomy.shoulder_width_offset + d).clamp(0.18, 0.50);
                mark_dirty = true;
            }
            EditorAction::AdjustHipWidth(d) => {
                editor.anatomy.hip_width_offset = (editor.anatomy.hip_width_offset + d).clamp(0.10, 0.35);
                mark_dirty = true;
            }
            EditorAction::AdjustArmLength(d) => {
                editor.anatomy.upper_arm_length = (editor.anatomy.upper_arm_length + d).clamp(0.18, 0.55);
                editor.anatomy.forearm_length = (editor.anatomy.forearm_length + d).clamp(0.16, 0.50);
                mark_dirty = true;
            }
            EditorAction::AdjustLegLength(d) => {
                editor.anatomy.upper_leg_length = (editor.anatomy.upper_leg_length + d).clamp(0.20, 0.60);
                editor.anatomy.lower_leg_length = (editor.anatomy.lower_leg_length + d).clamp(0.18, 0.55);
                mark_dirty = true;
            }
            EditorAction::AdjustLimbThickness(d) => {
                editor.anatomy.limb_thickness = (editor.anatomy.limb_thickness + d).clamp(0.06, 0.24);
                mark_dirty = true;
            }
            EditorAction::AdjustHeadScale(d) => {
                editor.anatomy.head_scale = (editor.anatomy.head_scale + d).clamp(0.60, 1.60);
                mark_dirty = true;
            }
            EditorAction::AdjustPauldronSize(d) => {
                editor.anatomy.pauldron_size = (editor.anatomy.pauldron_size + d).clamp(0.00, 0.50);
                mark_dirty = true;
            }
            EditorAction::CycleSkinColor => {
                let tones = [
                    Color::srgb(0.85, 0.68, 0.55), // Fair
                    Color::srgb(0.72, 0.52, 0.38), // Bronzed
                    Color::srgb(0.52, 0.36, 0.26), // Deep earthy
                    Color::srgb(0.55, 0.52, 0.65), // Elf twilight
                    Color::srgb(0.38, 0.52, 0.32), // Troll moss
                ];
                let cur = editor.anatomy.skin_color;
                let next_idx = tones.iter().position(|&c| c == cur).map(|i| (i + 1) % tones.len()).unwrap_or(0);
                editor.anatomy.skin_color = tones[next_idx];
                mark_dirty = true;
            }
            EditorAction::CycleClothColor => {
                let cloths = [
                    Color::srgb(0.22, 0.34, 0.48), // Navy
                    Color::srgb(0.55, 0.18, 0.15), // Crimson
                    Color::srgb(0.18, 0.12, 0.25), // Obsidian violet
                    Color::srgb(0.28, 0.42, 0.36), // Deep green
                    Color::srgb(0.38, 0.32, 0.24), // Rough leather
                ];
                let cur = editor.anatomy.cloth_color;
                let next_idx = cloths.iter().position(|&c| c == cur).map(|i| (i + 1) % cloths.len()).unwrap_or(0);
                editor.anatomy.cloth_color = cloths[next_idx];
                mark_dirty = true;
            }
            EditorAction::CycleArmorColor => {
                let armors = [
                    Color::srgb(0.45, 0.45, 0.48), // Steel
                    Color::srgb(0.72, 0.65, 0.38), // Brass
                    Color::srgb(0.32, 0.30, 0.34), // Black iron
                    Color::srgb(0.60, 0.40, 0.25), // Bronze
                ];
                let cur = editor.anatomy.armor_color;
                let next_idx = armors.iter().position(|&c| c == cur).map(|i| (i + 1) % armors.len()).unwrap_or(0);
                editor.anatomy.armor_color = armors[next_idx];
                mark_dirty = true;
            }
            EditorAction::ToggleShoulderSpikes => {
                editor.anatomy.has_shoulder_spikes = !editor.anatomy.has_shoulder_spikes;
                if editor.anatomy.pauldron_size < 0.05 {
                    editor.anatomy.pauldron_size = 0.18;
                }
                mark_dirty = true;
            }
            EditorAction::SetStudioMode(mode) => {
                editor.studio_preview_mode = *mode;
            }
            EditorAction::TriggerMeleeSwing => {
                if let Ok(mut anim) = player_anim_q.get_single_mut() {
                    anim.action = ActionState::MeleeSwing { timer: 0.0, duration: 0.55 };
                }
            }
            EditorAction::TriggerBowAim => {
                if let Ok(mut anim) = player_anim_q.get_single_mut() {
                    anim.action = ActionState::BowAim { draw_progress: 1.0, pitch: 0.0 };
                }
            }
            EditorAction::TriggerHitFlinch => {
                if let Ok(mut anim) = player_anim_q.get_single_mut() {
                    anim.hit_react_timer = 1.0;
                }
            }
            EditorAction::RandomizeAll => {
                editor.randomize();
                mark_dirty = true;
            }
            EditorAction::ResetCurrentPreset => {
                let idx = editor.preset_index;
                editor.load_preset(idx);
                mark_dirty = true;
            }
            EditorAction::ExportCode => {
                let code = editor.export_rust_code();
                info!("\n{}", code);
                println!("\n{}", code);
            }
            EditorAction::CloseEditor => {
                toggle_evts.send(ToggleCharacterEditorEvent);
            }
        }

        if mark_dirty {
            if let Ok(mut custom) = player_custom_q.get_single_mut() {
                custom.face = editor.face.clone();
                custom.anatomy = editor.anatomy.clone();
                custom.dirty = true;
            }
        }
    }
}

pub fn update_character_editor_display(
    editor: Res<CharacterEditorState>,
    mut display_q: Query<(&EditorValueDisplay, &mut Text)>,
) {
    if !editor.is_changed() || !editor.is_open {
        return;
    }

    for (field, mut text) in display_q.iter_mut() {
        match field.0.as_str() {
            "preset_name" => {
                let name = CHARACTER_PRESETS.get(editor.preset_index).map(|p| p.name).unwrap_or("Custom");
                text.sections[0].value = format!("Preset: {}", name);
            }
            "face_jaw_width" => text.sections[0].value = format!("{:.2}m", editor.face.jaw_width),
            "face_jaw_height" => text.sections[0].value = format!("{:.2}m", editor.face.jaw_height),
            "face_cheekbones" => text.sections[0].value = format!("{:.2}m", editor.face.cheekbone_width),
            "face_nose_bridge" => text.sections[0].value = format!("{:.2}m", editor.face.nose_bridge_length),
            "face_nose_tip_z" => text.sections[0].value = format!("{:.2}m", editor.face.nose_tip_z),
            "face_brow_ridge" => text.sections[0].value = format!("{:.2}m", editor.face.brow_ridge),
            "face_chin_forward" => text.sections[0].value = format!("{:.2}m", editor.face.chin_forward),
            "face_eye_depth" => text.sections[0].value = format!("{:.2}m", editor.face.eye_depth),
            "torso_width" => text.sections[0].value = format!("{:.2}m", editor.anatomy.torso_size.x),
            "torso_height" => text.sections[0].value = format!("{:.2}m", editor.anatomy.torso_size.y),
            "shoulder_width" => text.sections[0].value = format!("{:.2}m", editor.anatomy.shoulder_width_offset),
            "hip_width" => text.sections[0].value = format!("{:.2}m", editor.anatomy.hip_width_offset),
            "arm_length" => text.sections[0].value = format!("{:.2}m", editor.anatomy.upper_arm_length),
            "leg_length" => text.sections[0].value = format!("{:.2}m", editor.anatomy.upper_leg_length),
            "limb_thickness" => text.sections[0].value = format!("{:.2}m", editor.anatomy.limb_thickness),
            "head_scale" => text.sections[0].value = format!("{:.2}x", editor.anatomy.head_scale),
            "pauldron_size" => text.sections[0].value = format!("{:.2}m", editor.anatomy.pauldron_size),
            "skin_color_label" => {
                let srgba = editor.anatomy.skin_color.to_srgba();
                text.sections[0].value = format!("#{:02X}{:02X}{:02X}", (srgba.red * 255.0) as u8, (srgba.green * 255.0) as u8, (srgba.blue * 255.0) as u8);
            }
            "cloth_color_label" => {
                let srgba = editor.anatomy.cloth_color.to_srgba();
                text.sections[0].value = format!("#{:02X}{:02X}{:02X}", (srgba.red * 255.0) as u8, (srgba.green * 255.0) as u8, (srgba.blue * 255.0) as u8);
            }
            "armor_color_label" => {
                let srgba = editor.anatomy.armor_color.to_srgba();
                text.sections[0].value = format!("#{:02X}{:02X}{:02X}", (srgba.red * 255.0) as u8, (srgba.green * 255.0) as u8, (srgba.blue * 255.0) as u8);
            }
            "spikes_label" => {
                text.sections[0].value = if editor.anatomy.has_shoulder_spikes { "Active".into() } else { "None".into() };
            }
            _ => {}
        }
    }
}

/// Dynamically toggles tab panel containers based on the active tab, and highlights tab buttons.
pub fn update_editor_tab_visibility(
    editor: Res<CharacterEditorState>,
    mut tab_panel_q: Query<(&EditorTabPanel, &mut Style)>,
    mut tab_button_q: Query<(&EditorAction, &mut BorderColor, &mut BackgroundColor), With<Button>>,
) {
    if !editor.is_changed() {
        return;
    }

    // Toggle tab panels
    for (panel, mut style) in tab_panel_q.iter_mut() {
        style.display = if editor.is_open && panel.0 == editor.current_tab {
            Display::Flex
        } else {
            Display::None
        };
    }

    // Highlight active tab button
    for (action, mut border_color, mut bg_color) in tab_button_q.iter_mut() {
        if let EditorAction::SetTab(tab) = action {
            if *tab == editor.current_tab {
                *border_color = Color::srgb(0.96, 0.76, 0.25).into(); // Gold border
                *bg_color = Color::srgb(0.28, 0.32, 0.44).into();     // Active tab bg
            } else {
                *border_color = Color::srgb(0.40, 0.44, 0.52).into();
                *bg_color = Color::srgb(0.20, 0.22, 0.30).into();
            }
        }
    }
}

/// Rotates the local player model to face the direction of locomotion.
/// When aiming with a bow or performing a melee swing, immediately aligns with the crosshair direction (Quat::IDENTITY).
pub fn orient_character_model_to_locomotion_system(
    time: Res<Time>,
    player_q: Query<(&BevyTransform, &LinearVelocity, &AnimationState), With<PlayerBody>>,
    mut model_q: Query<(&Parent, &mut BevyTransform), (With<CharacterModelRoot>, Without<PlayerBody>)>,
) {
    let dt = time.delta_seconds();
    for (parent, mut model_transform) in model_q.iter_mut() {
        if let Ok((body_transform, linvel, anim_state)) = player_q.get(parent.get()) {
            let is_attacking = anim_state.action != ActionState::None;
            let target_rot = if is_attacking {
                Quat::IDENTITY
            } else {
                let local_vel = body_transform.rotation.inverse() * Vec3::new(linvel.x, 0.0, linvel.z);
                let horiz_speed_sq = local_vel.x * local_vel.x + local_vel.z * local_vel.z;
                if horiz_speed_sq > 0.08 {
                    let move_dir = local_vel.normalize();
                    // Face moving direction in local space
                    Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(move_dir.x, 0.0, move_dir.z))
                } else {
                    Quat::IDENTITY
                }
            };

            // Smoothly slerp local model facing
            let turn_speed = if is_attacking { 24.0 } else { 14.0 };
            let t = (turn_speed * dt).min(1.0);
            model_transform.rotation = model_transform.rotation.slerp(target_rot, t);
        }
    }
}

/// Mounts or unmounts the low-poly iron helmet on the player's JointType::Head pivot
/// based on the client's equipped armor state.
pub fn sync_character_equipped_helmet(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    equipped_armor: Res<ClientEquippedArmor>,
    head_joint_q: Query<(Entity, &JointType, &JointOwner)>,
    helmet_mesh_q: Query<(Entity, &Parent), With<EquippedHelmetMesh>>,
    player_q: Query<Entity, With<PlayerBody>>,
    studio_player_q: Query<Entity, With<PlayerCharacterCustomization>>,
    layers_q: Query<&RenderLayers>,
) {
    let valid_owners: Vec<Entity> = player_q.iter().chain(studio_player_q.iter()).collect();
    if valid_owners.is_empty() {
        return;
    }

    for (head_entity, joint, owner) in head_joint_q.iter() {
        if *joint != JointType::Head || !valid_owners.contains(&owner.0) {
            continue;
        }

        let existing_helmet = helmet_mesh_q.iter().find(|(_, p)| p.get() == head_entity);

        if equipped_armor.head.is_some() {
            if existing_helmet.is_none() {
                let helmet_mesh = meshes.add(create_low_poly_helmet());
                let helmet_mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.58, 0.62),
                    metallic: 0.85,
                    perceptual_roughness: 0.35,
                    cull_mode: None,
                    double_sided: true,
                    ..default()
                });
                let layer = layers_q.get(head_entity).cloned().unwrap_or(RenderLayers::from_layers(&[0, 1, 2]));

                commands.entity(head_entity).with_children(|head| {
                    head.spawn((
                        PbrBundle {
                            mesh: helmet_mesh,
                            material: helmet_mat,
                            transform: BevyTransform::IDENTITY,
                            ..default()
                        },
                        EquippedHelmetMesh,
                        layer,
                    ));
                });
                info!("Equipped 3D Helmet on head pivot ({:?})", head_entity);
            }
        } else if let Some((helmet_entity, _)) = existing_helmet {
            commands.entity(helmet_entity).despawn_recursive();
            info!("Unequipped 3D Helmet from head pivot ({:?})", head_entity);
        }
    }
}

// ----------------------------------------------------------------------------
// 15. CHARACTER CUSTOMIZER PLUGIN REGISTRATION
// ----------------------------------------------------------------------------

pub struct CharacterCustomizerPlugin;

impl Plugin for CharacterCustomizerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterEditorState>()
            .init_resource::<PhysicsWorld>()
            .add_systems(OnEnter(GameState::InGame), setup_character_editor_ui)
            .add_systems(FixedUpdate, step_physics_world_system)
            .add_systems(
                Update,
                (
                    toggle_character_editor_ui,
                    handle_character_editor_interactions,
                    update_character_editor_display,
                    update_editor_tab_visibility,
                    studio_camera_orbit_system,
                    live_character_update_system,
                    sync_character_equipped_helmet,
                    sync_player_animation_state,
                    procedural_animator_system,
                    orient_character_model_to_locomotion_system,
                )
                    .in_set(UpdateSet::Animation),
            );
    }
}

// ----------------------------------------------------------------------------
// 16. UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_face_mesh_generation_vertex_and_index_counts() {
        let profile = FaceProfile::default();
        let mesh = generate_custom_face(&profile);

        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());

        let pos_count = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len();
        assert!(pos_count > 10, "Face mesh must have faceted vertices");
    }

    #[test]
    fn test_character_presets_integrity() {
        assert_eq!(CHARACTER_PRESETS.len(), 5);
        for preset in CHARACTER_PRESETS {
            assert!(!preset.name.is_empty());
            assert!(preset.face.jaw_width > 0.0);
            assert!(preset.face.nose_tip_z > 0.0);
            assert!(preset.anatomy.torso_size.x > 0.0);
            assert!(preset.anatomy.torso_size.y > 0.0);
        }
    }

    #[test]
    fn test_procedural_pyramid_mesh() {
        let mesh = create_low_poly_pyramid();
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
    }

    #[test]
    fn test_export_rust_code_format() {
        let editor = CharacterEditorState::default();
        let code = editor.export_rust_code();
        assert!(code.contains("pub const CUSTOM_CHARACTER_PRESET: CharacterPreset"));
        assert!(code.contains("jaw_width:"));
        assert!(code.contains("torso_size:"));
    }

    #[test]
    fn test_physics_world_compound_skeleton_stepping() {
        let mut physics = PhysicsWorld::default();

        let root_rb = RigidBodyBuilder::kinematic_position_based().build();
        let root_handle = physics.rigid_body_set.insert(root_rb);
        let rb_comp = RbHandle(root_handle);

        let col = ColliderBuilder::cuboid(0.2, 0.3, 0.1);
        let col_handle = physics.collider_set.insert_with_parent(
            col,
            root_handle,
            &mut physics.rigid_body_set,
        );
        let col_comp = ColHandle(col_handle);

        assert_eq!(rb_comp.0, root_handle);
        assert_eq!(col_comp.0, col_handle);

        // Verify in-place hitbox update without dropping body
        if let Some(c) = physics.collider_set.get_mut(col_handle) {
            c.set_shape(SharedShape::cuboid(0.25, 0.35, 0.15));
        }

        physics.step();
        assert!(physics.rigid_body_set.get(root_handle).is_some());
    }

    #[test]
    fn test_low_poly_helmet_mesh_attributes() {
        let mesh = create_low_poly_helmet();
        assert_eq!(mesh.primitive_topology(), PrimitiveTopology::TriangleList);
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).expect("Missing positions");
        assert!(positions.len() >= 15);
        let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).expect("Missing flat normals");
        assert_eq!(positions.len(), normals.len());
    }
}
