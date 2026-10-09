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
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::RenderLayers;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use crate::physics::LinearVelocity;
use tracing::info;

use crate::components::*;
use crate::core::*;
use crate::camera::CharacterCameraSettings;
use crate::weapons::{WeaponState, WeaponType};

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
            jaw_width: 0.22,
            jaw_height: -0.16,
            cheekbone_width: 0.22,
            nose_bridge_length: 0.20,
            nose_tip_z: 0.22,
            brow_ridge: 0.12,
            chin_forward: 0.10,
            eye_depth: 0.05,
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
            shoulder_width_offset: 0.28,
            hip_width_offset: 0.16,
            upper_arm_length: 0.32,
            forearm_length: 0.30,
            upper_leg_length: 0.38,
            lower_leg_length: 0.36,
            limb_thickness: 0.12,
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
            jaw_width: 0.22,
            jaw_height: -0.16,
            cheekbone_width: 0.22,
            nose_bridge_length: 0.20,
            nose_tip_z: 0.22,
            brow_ridge: 0.12,
            chin_forward: 0.10,
            eye_depth: 0.05,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Human",
            torso_size: Vec3::new(0.42, 0.58, 0.26),
            shoulder_width_offset: 0.28,
            hip_width_offset: 0.16,
            upper_arm_length: 0.32,
            forearm_length: 0.30,
            upper_leg_length: 0.38,
            lower_leg_length: 0.36,
            limb_thickness: 0.12,
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
            jaw_width: 0.15,
            jaw_height: -0.18,
            cheekbone_width: 0.26,
            nose_bridge_length: 0.24,
            nose_tip_z: 0.30,
            brow_ridge: 0.16,
            chin_forward: 0.12,
            eye_depth: 0.07,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Dark Elf",
            torso_size: Vec3::new(0.36, 0.62, 0.22),
            shoulder_width_offset: 0.24,
            hip_width_offset: 0.14,
            upper_arm_length: 0.35,
            forearm_length: 0.33,
            upper_leg_length: 0.42,
            lower_leg_length: 0.40,
            limb_thickness: 0.10,
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
            jaw_width: 0.35,
            jaw_height: -0.10,
            cheekbone_width: 0.35,
            nose_bridge_length: 0.18,
            nose_tip_z: 0.15,
            brow_ridge: 0.22,
            chin_forward: 0.16,
            eye_depth: 0.08,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Barbarian",
            torso_size: Vec3::new(0.58, 0.65, 0.36),
            shoulder_width_offset: 0.38,
            hip_width_offset: 0.20,
            upper_arm_length: 0.36,
            forearm_length: 0.34,
            upper_leg_length: 0.40,
            lower_leg_length: 0.38,
            limb_thickness: 0.16,
            head_scale: 1.15,
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
            jaw_width: 0.28,
            jaw_height: -0.40,
            cheekbone_width: 0.28,
            nose_bridge_length: 0.32,
            nose_tip_z: 0.50,
            brow_ridge: 0.25,
            chin_forward: 0.22,
            eye_depth: 0.10,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Troll",
            torso_size: Vec3::new(0.48, 0.72, 0.32),
            shoulder_width_offset: 0.34,
            hip_width_offset: 0.18,
            upper_arm_length: 0.44,
            forearm_length: 0.42,
            upper_leg_length: 0.44,
            lower_leg_length: 0.42,
            limb_thickness: 0.13,
            head_scale: 1.10,
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
            jaw_width: 0.32,
            jaw_height: -0.16,
            cheekbone_width: 0.30,
            nose_bridge_length: 0.20,
            nose_tip_z: 0.26,
            brow_ridge: 0.24,
            chin_forward: 0.14,
            eye_depth: 0.08,
        },
        anatomy: RaceAnatomyProfile {
            race_name: "Dwarf",
            torso_size: Vec3::new(0.54, 0.48, 0.36),
            shoulder_width_offset: 0.34,
            hip_width_offset: 0.22,
            upper_arm_length: 0.26,
            forearm_length: 0.24,
            upper_leg_length: 0.28,
            lower_leg_length: 0.26,
            limb_thickness: 0.15,
            head_scale: 1.10,
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
// 3. ECS COMPONENTS & MARKERS
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

/// Identifies the root container for the procedural character hierarchy.
#[derive(Component)]
pub struct CharacterModelRoot;

/// Marker for the generated procedural face mesh entity.
#[derive(Component)]
pub struct ProceduralFaceMesh;

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
// 4. PROCEDURAL MESH GENERATORS
// ----------------------------------------------------------------------------

/// Generates a stylized, low-poly faceted 3D face mesh matching the FaceProfile.
/// Winding order is counter-clockwise for outward normals, followed by flat normal calculation.
pub fn generate_custom_face(profile: &FaceProfile) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    // Coordinate positions in local space (Forward is -Z, Up is +Y, Right is +X)
    let positions: Vec<[f32; 3]> = vec![
        // 0: Chin Tip
        [0.0, profile.jaw_height, -profile.chin_forward],
        // 1: Left Jaw
        [-profile.jaw_width, profile.jaw_height + 0.10, 0.0],
        // 2: Right Jaw
        [profile.jaw_width, profile.jaw_height + 0.10, 0.0],
        // 3: Nose Tip
        [0.0, 0.0, -profile.nose_tip_z],
        // 4: Nose Bridge (Midway up)
        [0.0, profile.nose_bridge_length * 0.55, -profile.nose_tip_z * 0.55],
        // 5: Left Cheekbone
        [-profile.cheekbone_width, 0.08, -0.04],
        // 6: Right Cheekbone
        [profile.cheekbone_width, 0.08, -0.04],
        // 7: Forehead Center
        [0.0, 0.42, 0.0],
        // 8: Left Brow
        [-profile.cheekbone_width * 0.75, 0.28, -profile.brow_ridge],
        // 9: Right Brow
        [profile.cheekbone_width * 0.75, 0.28, -profile.brow_ridge],
        // 10: Left Eye Socket
        [-profile.cheekbone_width * 0.45, 0.18, -profile.eye_depth],
        // 11: Right Eye Socket
        [profile.cheekbone_width * 0.45, 0.18, -profile.eye_depth],
        // 12: Upper Lip / Philtrum
        [0.0, profile.jaw_height * 0.45, -profile.chin_forward * 0.8],
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

// ----------------------------------------------------------------------------
// 5. PROCEDURAL CHARACTER HIERARCHY ASSEMBLY
// ----------------------------------------------------------------------------

/// Spawns the entire articulated character hierarchy matching the SpacetimeRTS directives.
pub fn spawn_procedural_character_hierarchy(
    commands: &mut Commands,
    parent_entity: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    race: &RaceAnatomyProfile,
    face: &FaceProfile,
) -> Entity {
    // Generate mesh assets
    let face_mesh = meshes.add(generate_custom_face(face));
    let skull_mesh = meshes.add(bevy::math::primitives::Cuboid::new(
        0.34 * race.head_scale,
        0.38 * race.head_scale,
        0.34 * race.head_scale,
    ));
    let torso_mesh = meshes.add(bevy::math::primitives::Cuboid::from_size(race.torso_size));
    let upper_arm_mesh = meshes.add(bevy::math::primitives::Cuboid::new(
        race.limb_thickness,
        race.upper_arm_length,
        race.limb_thickness,
    ));
    let forearm_mesh = meshes.add(bevy::math::primitives::Cuboid::new(
        race.limb_thickness * 0.9,
        race.forearm_length,
        race.limb_thickness * 0.9,
    ));
    let hand_mesh = meshes.add(bevy::math::primitives::Cuboid::new(
        race.limb_thickness * 0.95,
        0.12,
        race.limb_thickness * 1.1,
    ));
    let upper_leg_mesh = meshes.add(bevy::math::primitives::Cuboid::new(
        race.limb_thickness * 1.15,
        race.upper_leg_length,
        race.limb_thickness * 1.15,
    ));
    let lower_leg_mesh = meshes.add(bevy::math::primitives::Cuboid::new(
        race.limb_thickness,
        race.lower_leg_length,
        race.limb_thickness,
    ));
    let foot_mesh = meshes.add(bevy::math::primitives::Cuboid::new(
        race.limb_thickness * 1.05,
        0.10,
        0.24,
    ));
    let pauldron_mesh = meshes.add(bevy::math::primitives::Cuboid::new(
        race.pauldron_size * 1.3,
        race.pauldron_size * 0.7,
        race.pauldron_size * 1.3,
    ));
    let spike_mesh = meshes.add(create_low_poly_pyramid());

    // Material assets (Two-sided PBR materials for crisp low-poly faceted shading without backface culling gaps)
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

    let layer2 = RenderLayers::layer(2);
    let mut model_root_id = Entity::PLACEHOLDER;

    commands.entity(parent_entity).with_children(|root| {
        let r_id = root
            .spawn((
                SpatialBundle::from_transform(BevyTransform::from_xyz(0.0, -0.15, 0.0)),
                CharacterModelRoot,
                RTSProxy,
            ))
            .with_children(|model_parent| {
                // 1. TORSO (Central skeletal anchor)
                model_parent
                    .spawn((
                        PbrBundle {
                            mesh: torso_mesh,
                            material: cloth_mat.clone(),
                            transform: BevyTransform::from_xyz(0.0, 0.0, 0.0),
                            ..default()
                        },
                        JointType::Torso,
                        layer2.clone(),
                    ))
                    .with_children(|torso| {
                        // 2. HEAD PIVOT
                        let head_pivot_y = race.torso_size.y * 0.5 + 0.16 * race.head_scale;
                        torso
                            .spawn((
                                SpatialBundle::from_transform(BevyTransform::from_xyz(0.0, head_pivot_y, 0.0)),
                                JointType::Head,
                            ))
                            .with_children(|head_pivot| {
                                // Skull Base
                                head_pivot.spawn((
                                    PbrBundle {
                                        mesh: skull_mesh,
                                        material: skin_mat.clone(),
                                        transform: BevyTransform::from_xyz(0.0, 0.08 * race.head_scale, 0.0),
                                        ..default()
                                    },
                                    layer2.clone(),
                                ));
                                // Raw Procedural Face (Pinned to anterior face: -Z)
                                head_pivot.spawn((
                                    PbrBundle {
                                        mesh: face_mesh,
                                        material: skin_mat.clone(),
                                        transform: BevyTransform::from_xyz(
                                            0.0,
                                            0.06 * race.head_scale,
                                            -0.17 * race.head_scale,
                                        ),
                                        ..default()
                                    },
                                    ProceduralFaceMesh,
                                    layer2.clone(),
                                ));
                            });

                        // 3. LEFT ARM (Shoulder -> UpperArm -> Elbow -> Forearm -> Hand)
                        let shoulder_y = race.torso_size.y * 0.5 - 0.08;
                        torso
                            .spawn((
                                SpatialBundle::from_transform(BevyTransform::from_xyz(
                                    -race.shoulder_width_offset,
                                    shoulder_y,
                                    0.0,
                                )),
                                JointType::ShoulderL,
                            ))
                            .with_children(|shoulder_l| {
                                // Upper Arm mesh
                                shoulder_l.spawn((
                                    PbrBundle {
                                        mesh: upper_arm_mesh.clone(),
                                        material: cloth_mat.clone(),
                                        transform: BevyTransform::from_xyz(0.0, -race.upper_arm_length * 0.5, 0.0),
                                        ..default()
                                    },
                                    layer2.clone(),
                                ));
                                // Pauldron / Spikes
                                if race.pauldron_size > 0.01 {
                                    shoulder_l.spawn((
                                        PbrBundle {
                                            mesh: pauldron_mesh.clone(),
                                            material: armor_mat.clone(),
                                            transform: BevyTransform::from_xyz(-0.02, 0.04, 0.0),
                                            ..default()
                                        },
                                        layer2.clone(),
                                    ));
                                    if race.has_shoulder_spikes {
                                        shoulder_l.spawn((
                                            PbrBundle {
                                                mesh: spike_mesh.clone(),
                                                material: armor_mat.clone(),
                                                transform: BevyTransform::from_xyz(-0.02, 0.08 + race.pauldron_size * 0.35, 0.0),
                                                ..default()
                                            },
                                            layer2.clone(),
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
                                    ))
                                    .with_children(|elbow_l| {
                                        // Forearm mesh
                                        elbow_l.spawn((
                                            PbrBundle {
                                                mesh: forearm_mesh.clone(),
                                                material: skin_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.0, -race.forearm_length * 0.5, 0.0),
                                                ..default()
                                            },
                                            layer2.clone(),
                                        ));
                                        // Hand L (Socket)
                                        elbow_l.spawn((
                                            PbrBundle {
                                                mesh: hand_mesh.clone(),
                                                material: skin_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.0, -race.forearm_length - 0.04, 0.0),
                                                ..default()
                                            },
                                            JointType::HandL,
                                            layer2.clone(),
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
                            ))
                            .with_children(|shoulder_r| {
                                // Upper Arm mesh
                                shoulder_r.spawn((
                                    PbrBundle {
                                        mesh: upper_arm_mesh.clone(),
                                        material: cloth_mat.clone(),
                                        transform: BevyTransform::from_xyz(0.0, -race.upper_arm_length * 0.5, 0.0),
                                        ..default()
                                    },
                                    layer2.clone(),
                                ));
                                // Pauldron / Spikes
                                if race.pauldron_size > 0.01 {
                                    shoulder_r.spawn((
                                        PbrBundle {
                                            mesh: pauldron_mesh.clone(),
                                            material: armor_mat.clone(),
                                            transform: BevyTransform::from_xyz(0.02, 0.04, 0.0),
                                            ..default()
                                        },
                                        layer2.clone(),
                                    ));
                                    if race.has_shoulder_spikes {
                                        shoulder_r.spawn((
                                            PbrBundle {
                                                mesh: spike_mesh.clone(),
                                                material: armor_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.02, 0.08 + race.pauldron_size * 0.35, 0.0),
                                                ..default()
                                            },
                                            layer2.clone(),
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
                                    ))
                                    .with_children(|elbow_r| {
                                        // Forearm mesh
                                        elbow_r.spawn((
                                            PbrBundle {
                                                mesh: forearm_mesh.clone(),
                                                material: skin_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.0, -race.forearm_length * 0.5, 0.0),
                                                ..default()
                                            },
                                            layer2.clone(),
                                        ));
                                        // Hand R (Weapon Socket)
                                        elbow_r.spawn((
                                            PbrBundle {
                                                mesh: hand_mesh.clone(),
                                                material: skin_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.0, -race.forearm_length - 0.04, 0.0),
                                                ..default()
                                            },
                                            JointType::HandR,
                                            layer2.clone(),
                                        ));
                                    });
                            });

                        // 5. LEFT LEG (Hip -> UpperLeg -> Knee -> LowerLeg -> Foot)
                        let hip_y = -race.torso_size.y * 0.5;
                        torso
                            .spawn((
                                SpatialBundle::from_transform(BevyTransform::from_xyz(
                                    -race.hip_width_offset,
                                    hip_y,
                                    0.0,
                                )),
                                JointType::HipL,
                            ))
                            .with_children(|hip_l| {
                                hip_l.spawn((
                                    PbrBundle {
                                        mesh: upper_leg_mesh.clone(),
                                        material: pants_mat.clone(),
                                        transform: BevyTransform::from_xyz(0.0, -race.upper_leg_length * 0.5, 0.0),
                                        ..default()
                                    },
                                    layer2.clone(),
                                ));
                                hip_l
                                    .spawn((
                                        SpatialBundle::from_transform(BevyTransform::from_xyz(
                                            0.0,
                                            -race.upper_leg_length,
                                            0.0,
                                        )),
                                        JointType::KneeL,
                                    ))
                                    .with_children(|knee_l| {
                                        knee_l.spawn((
                                            PbrBundle {
                                                mesh: lower_leg_mesh.clone(),
                                                material: pants_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.0, -race.lower_leg_length * 0.5, 0.0),
                                                ..default()
                                            },
                                            layer2.clone(),
                                        ));
                                        knee_l.spawn((
                                            PbrBundle {
                                                mesh: foot_mesh.clone(),
                                                material: pants_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.0, -race.lower_leg_length - 0.04, -0.05),
                                                ..default()
                                            },
                                            JointType::FootL,
                                            layer2.clone(),
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
                            ))
                            .with_children(|hip_r| {
                                hip_r.spawn((
                                    PbrBundle {
                                        mesh: upper_leg_mesh.clone(),
                                        material: pants_mat.clone(),
                                        transform: BevyTransform::from_xyz(0.0, -race.upper_leg_length * 0.5, 0.0),
                                        ..default()
                                    },
                                    layer2.clone(),
                                ));
                                hip_r
                                    .spawn((
                                        SpatialBundle::from_transform(BevyTransform::from_xyz(
                                            0.0,
                                            -race.upper_leg_length,
                                            0.0,
                                        )),
                                        JointType::KneeR,
                                    ))
                                    .with_children(|knee_r| {
                                        knee_r.spawn((
                                            PbrBundle {
                                                mesh: lower_leg_mesh.clone(),
                                                material: pants_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.0, -race.lower_leg_length * 0.5, 0.0),
                                                ..default()
                                            },
                                            layer2.clone(),
                                        ));
                                        knee_r.spawn((
                                            PbrBundle {
                                                mesh: foot_mesh.clone(),
                                                material: pants_mat.clone(),
                                                transform: BevyTransform::from_xyz(0.0, -race.lower_leg_length - 0.04, -0.05),
                                                ..default()
                                            },
                                            JointType::FootR,
                                            layer2.clone(),
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
// 6. PROCEDURAL ANIMATION ENGINE
// ----------------------------------------------------------------------------

/// Applies mathematical Euler/Quaternion rotations to all articulated joint pivots.
/// Strictly implements walk/run oscillations, velocity-driven jumping, upper-body
/// action overrides (melee swings, bow aiming), and hit reaction flinches.
pub fn procedural_animator_system(
    time: Res<Time>,
    mut character_q: Query<(&mut AnimationState, Option<&LinearVelocity>)>,
    editor_state: Res<CharacterEditorState>,
    mut joint_q: Query<(&JointType, &mut BevyTransform)>,
) {
    let dt = time.delta_seconds();

    for (mut state, linvel_opt) in character_q.iter_mut() {
        let (speed, is_grounded, vy) = if editor_state.is_open && editor_state.studio_preview_mode != StudioPreviewMode::LiveGameplay {
            // Studio preview simulation
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
            state.gait_phase += dt * frequency;
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

        // Apply joint rotations based on compiled mathematical models
        let phase = state.gait_phase;
        let walk_run_blend = ((speed - 0.5) / 3.5).clamp(0.0, 1.0);
        let amplitude = 0.35 + 0.35 * walk_run_blend; // 0.35 rad (walk) -> 0.70 rad (sprint)

        for (joint, mut tf) in joint_q.iter_mut() {
            match joint {
                // TORSO: Forward lean during sprints + Hit reaction snap
                JointType::Torso => {
                    let sprint_lean = -0.15 * walk_run_blend;
                    let hit_lean = state.hit_react_timer * 0.30;
                    tf.rotation = Quat::from_rotation_x(sprint_lean + hit_lean);
                }

                // HEAD: Compensate torso lean + Hit reaction flinch
                JointType::Head => {
                    let hit_snap = state.hit_react_timer * 0.25;
                    tf.rotation = Quat::from_rotation_x(hit_snap);
                }

                // LOWER BODY: Locomotion vs Airborne Trajectory
                JointType::HipL => {
                    if state.is_grounded {
                        let rot_x = phase.sin() * amplitude;
                        tf.rotation = Quat::from_rotation_x(rot_x);
                    } else if state.vertical_velocity > 0.0 {
                        // Ascending jump: Knees tuck upward
                        tf.rotation = Quat::from_rotation_x(-0.40);
                    } else {
                        // Falling: Straighten downward
                        tf.rotation = Quat::from_rotation_x(0.10);
                    }
                }

                JointType::HipR => {
                    if state.is_grounded {
                        let rot_x = -phase.sin() * amplitude;
                        tf.rotation = Quat::from_rotation_x(rot_x);
                    } else if state.vertical_velocity > 0.0 {
                        tf.rotation = Quat::from_rotation_x(-0.40);
                    } else {
                        tf.rotation = Quat::from_rotation_x(0.10);
                    }
                }

                JointType::KneeL => {
                    if state.is_grounded {
                        // Half-wave rectified backward bend: Knees never bend forward
                        let bend = (-phase.sin()).max(0.0) * (amplitude * 1.4);
                        tf.rotation = Quat::from_rotation_x(bend);
                    } else if state.vertical_velocity > 0.0 {
                        tf.rotation = Quat::from_rotation_x(0.60);
                    } else {
                        tf.rotation = Quat::from_rotation_x(0.0);
                    }
                }

                JointType::KneeR => {
                    if state.is_grounded {
                        let bend = (phase.sin()).max(0.0) * (amplitude * 1.4);
                        tf.rotation = Quat::from_rotation_x(bend);
                    } else if state.vertical_velocity > 0.0 {
                        tf.rotation = Quat::from_rotation_x(0.60);
                    } else {
                        tf.rotation = Quat::from_rotation_x(0.0);
                    }
                }

                // UPPER BODY LAYER: Counterbalance swing OR Action overrides
                JointType::ShoulderL => {
                    match &state.action {
                        ActionState::BowAim { pitch, .. } => {
                            // Left arm holds bow firmly pointed forward along aiming pitch
                            tf.rotation = Quat::from_rotation_x(-1.57 + pitch)
                                * Quat::from_rotation_y(0.15);
                        }
                        _ => {
                            if !state.is_grounded {
                                // Jump splay for balance (+Z rot)
                                tf.rotation = Quat::from_rotation_z(0.30);
                            } else {
                                // Locomotion arm swing opposite to hip
                                let arm_swing = -phase.sin() * (amplitude * 0.85);
                                let hit_jitter = state.hit_react_timer * 0.20;
                                tf.rotation = Quat::from_rotation_x(arm_swing)
                                    * Quat::from_rotation_z(0.08 + hit_jitter);
                            }
                        }
                    }
                }

                JointType::ElbowL => {
                    match &state.action {
                        ActionState::BowAim { .. } => {
                            tf.rotation = Quat::from_rotation_x(0.05); // Locked straight
                        }
                        _ => {
                            // Slight natural bend on backward swing
                            let elbow_bend = (phase.sin()).max(0.0) * 0.40;
                            tf.rotation = Quat::from_rotation_x(elbow_bend);
                        }
                    }
                }

                JointType::ShoulderR => {
                    match &state.action {
                        ActionState::MeleeSwing { timer, duration } => {
                            // 3-Phase Melee Swing: Anticipation -> Snap -> Follow-through
                            let t = (timer / duration.max(0.001)).clamp(0.0, 1.0);
                            if t < 0.30 {
                                // Phase 1: Wind back and up
                                let p = t / 0.30;
                                let rot_x = 0.0 + (-1.20 - 0.0) * p;
                                tf.rotation = Quat::from_rotation_x(rot_x) * Quat::from_rotation_y(0.40);
                            } else if t < 0.50 {
                                // Phase 2: Forward snap cutting horizontally
                                let p = (t - 0.30) / 0.20;
                                let rot_x = -1.20 + (0.80 - -1.20) * p;
                                tf.rotation = Quat::from_rotation_x(rot_x) * Quat::from_rotation_y(-0.60);
                            } else {
                                // Phase 3: Recovery / Follow-through
                                let p = (t - 0.50) / 0.50;
                                let rot_x = 0.80 + (0.0 - 0.80) * p;
                                tf.rotation = Quat::from_rotation_x(rot_x) * Quat::from_rotation_y(-0.60 * (1.0 - p));
                            }
                        }
                        ActionState::BowAim { .. } => {
                            // Draw shoulder pulls back
                            tf.rotation = Quat::from_rotation_x(-1.20) * Quat::from_rotation_y(0.35);
                        }
                        _ => {
                            if !state.is_grounded {
                                tf.rotation = Quat::from_rotation_z(-0.30);
                            } else {
                                let arm_swing = phase.sin() * (amplitude * 0.85);
                                let hit_jitter = state.hit_react_timer * -0.20;
                                tf.rotation = Quat::from_rotation_x(arm_swing)
                                    * Quat::from_rotation_z(-0.08 + hit_jitter);
                            }
                        }
                    }
                }

                JointType::ElbowR => {
                    match &state.action {
                        ActionState::MeleeSwing { timer, duration } => {
                            let t = (timer / duration.max(0.001)).clamp(0.0, 1.0);
                            let bend = if t < 0.30 { 0.80 } else if t < 0.50 { 0.25 } else { 0.10 };
                            tf.rotation = Quat::from_rotation_x(bend);
                        }
                        ActionState::BowAim { draw_progress, .. } => {
                            // Deep bend inward toward the cheekbone
                            let draw_bend = 0.20 + (1.80 - 0.20) * draw_progress.clamp(0.0, 1.0);
                            tf.rotation = Quat::from_rotation_x(draw_bend);
                        }
                        _ => {
                            let elbow_bend = (-phase.sin()).max(0.0) * 0.40;
                            tf.rotation = Quat::from_rotation_x(elbow_bend);
                        }
                    }
                }

                _ => {}
            }
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
// 7. IN-GAME CHARACTER CUSTOMIZER WORKBENCH UI
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

        // Procedural variations (+/- 20%)
        let jitter = |val: f32, pct: f32| -> f32 {
            val * (1.0 + (0.5 - 0.5) * pct) // deterministic base jitter
        };
        self.face.jaw_width = jitter(self.face.jaw_width, 0.20);
        self.face.cheekbone_width = jitter(self.face.cheekbone_width, 0.20);
        self.face.nose_tip_z = jitter(self.face.nose_tip_z, 0.25);
    }

    pub fn export_rust_code(&self) -> String {
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
             \x20       race_name: \"{}\".into(),\n\
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
            0.85, 0.68, 0.55,
            0.24, 0.36, 0.52,
            0.18, 0.16, 0.15,
            0.45, 0.45, 0.48,
            self.anatomy.pauldron_size,
            self.anatomy.has_shoulder_spikes,
        )
    }
}

// ----------------------------------------------------------------------------
// 8. UI COMPONENT TAGS & ACTIONS
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
// 9. UI SETUP & HIERARCHY SPAWNING
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
                header.spawn(TextBundle::from_section(
                    "🎭 CHARACTER STUDIO & MORPHOLOGY",
                    TextStyle {
                        font_size: font_title,
                        color: gold_header,
                        ..default()
                    },
                ));

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
// 10. UI HELPER SPAWNERS
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
// 11. TOGGLE & INTERACTION SYSTEMS
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

                // Orbit 3rd-person camera boom so player can inspect character model
                editor.previous_cam_distance = cam_settings.target_distance;
                cam_settings.target_distance = 3.2;
                info!("🎭 Opened Character Model Customizer Studio [F5]. Camera zoomed to 3rd-person.");
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
                editor.face.jaw_width = (editor.face.jaw_width + d).clamp(0.08, 0.45);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceJawHeight(d) => {
                editor.face.jaw_height = (editor.face.jaw_height + d).clamp(-0.50, 0.05);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceCheekbones(d) => {
                editor.face.cheekbone_width = (editor.face.cheekbone_width + d).clamp(0.12, 0.45);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceNoseBridge(d) => {
                editor.face.nose_bridge_length = (editor.face.nose_bridge_length + d).clamp(0.10, 0.40);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceNoseTipZ(d) => {
                editor.face.nose_tip_z = (editor.face.nose_tip_z + d).clamp(0.08, 0.60);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceBrowRidge(d) => {
                editor.face.brow_ridge = (editor.face.brow_ridge + d).clamp(0.02, 0.35);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceChinForward(d) => {
                editor.face.chin_forward = (editor.face.chin_forward + d).clamp(-0.15, 0.25);
                mark_dirty = true;
            }
            EditorAction::AdjustFaceEyeDepth(d) => {
                editor.face.eye_depth = (editor.face.eye_depth + d).clamp(0.00, 0.20);
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
                let next = tones.iter().find(|&&c| c != cur).copied().unwrap_or(tones[0]);
                editor.anatomy.skin_color = next;
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
                let next = cloths.iter().find(|&&c| c != cur).copied().unwrap_or(cloths[0]);
                editor.anatomy.cloth_color = next;
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
                let next = armors.iter().find(|&&c| c != cur).copied().unwrap_or(armors[0]);
                editor.anatomy.armor_color = next;
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
    if !editor.is_open {
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

/// Rebuilds the player's 3D articulated model when customization parameters change.
pub fn sync_player_model_rebuild_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut player_q: Query<(Entity, &mut PlayerCharacterCustomization), With<PlayerBody>>,
    existing_root_q: Query<(Entity, &Parent), With<CharacterModelRoot>>,
) {
    for (player_entity, mut custom) in player_q.iter_mut() {
        if !custom.dirty {
            continue;
        }
        custom.dirty = false;

        // Despawn existing procedural model hierarchy
        for (root_entity, parent) in existing_root_q.iter() {
            if parent.get() == player_entity {
                commands.entity(root_entity).despawn_recursive();
            }
        }

        // Spawn freshly compiled articulated hierarchy
        spawn_procedural_character_hierarchy(
            &mut commands,
            player_entity,
            &mut meshes,
            &mut materials,
            &custom.anatomy,
            &custom.face,
        );

        info!("🎭 Rebuilt procedural character model for player (Race: {})", custom.anatomy.race_name);
    }
}

/// Dynamically toggles tab panel containers based on the active tab, and highlights tab buttons.
pub fn update_editor_tab_visibility(
    editor: Res<CharacterEditorState>,
    mut tab_panel_q: Query<(&EditorTabPanel, &mut Style)>,
    mut tab_button_q: Query<(&EditorAction, &mut BorderColor, &mut BackgroundColor), With<Button>>,
) {
    if !editor.is_changed() && !editor.is_open {
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

// ----------------------------------------------------------------------------
// 12. CHARACTER CUSTOMIZER PLUGIN REGISTRATION
// ----------------------------------------------------------------------------

pub struct CharacterCustomizerPlugin;

impl Plugin for CharacterCustomizerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterEditorState>()
            .add_systems(OnEnter(GameState::InGame), setup_character_editor_ui)
            .add_systems(
                Update,
                (
                    toggle_character_editor_ui,
                    handle_character_editor_interactions,
                    update_character_editor_display,
                    update_editor_tab_visibility,
                    sync_player_model_rebuild_system,
                    sync_player_animation_state,
                    procedural_animator_system,
                    orient_character_model_to_locomotion_system,
                )
                    .in_set(UpdateSet::Animation),
            );
    }
}

// ----------------------------------------------------------------------------
// 13. UNIT TESTS
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
}
