// ============================================================================
// File: zone_editor.rs
// ============================================================================
// ----------------------------------------------------------------------------
// EVERQUEST WORLD MAP & WARCRAFT 3 MAP PAINTER / ZONE EDITOR SYSTEM
// ----------------------------------------------------------------------------
// Architectural Note: Provides an end-to-end interconnected multi-biome world
// map inspired by EverQuest (Antonica / Norrath zone atlas) coupled with a
// Warcraft 3-style RTS map painter tool for sculpting terrain elevation, painting
// biome textures, stamping micro-voxel doodads, spawning units, and placing
// zone-line boundary gateways.
//
// Complies with AI_RULES.md: Uses BTreeMap and BTreeSet for strictly deterministic
// iteration and memory stability.

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::window::PrimaryWindow;
use avian3d::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{OnceLock, RwLock};
use tracing::info;

use crate::core::*;
use crate::components::*;
use crate::trees::{create_voxel_oak_mesh, create_voxel_pine_mesh, create_voxel_dead_tree_mesh};
use crate::props::{create_voxel_rock_mesh, create_voxel_bush_mesh, create_voxel_flint_mesh};
use crate::creatures::{
    create_voxel_peasant_mesh, create_voxel_goblin_mesh, create_voxel_deer_mesh,
    create_voxel_boar_mesh,
};

// ----------------------------------------------------------------------------
// 1. BIOME DEFINITIONS & CONTINENTAL ZONE GRAPH
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiomeType {
    TemperatePlains,
    FrigidTundra,
    AridDesert,
    MurkySwamp,
    VolcanicWasteland,
    DarkForest,
}

impl BiomeType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::TemperatePlains => "Temperate Plains",
            Self::FrigidTundra => "Frigid Tundra",
            Self::AridDesert => "Arid Desert",
            Self::MurkySwamp => "Murky Swamp",
            Self::VolcanicWasteland => "Volcanic Wasteland",
            Self::DarkForest => "Dark Forest",
        }
    }

    pub fn primary_color(&self) -> [f32; 4] {
        match self {
            Self::TemperatePlains => [0.28, 0.64, 0.28, 1.0],   // Vibrant meadow green
            Self::FrigidTundra => [0.93, 0.96, 1.00, 1.0],      // Glacial snow white
            Self::AridDesert => [0.85, 0.74, 0.50, 1.0],        // Sun-baked dune sand
            Self::MurkySwamp => [0.22, 0.36, 0.20, 1.0],        // Stagnant moss muck
            Self::VolcanicWasteland => [0.18, 0.16, 0.16, 1.0], // Obsidian basalt
            Self::DarkForest => [0.14, 0.28, 0.16, 1.0],        // Deep twilight pine
        }
    }

    pub fn badge_color(&self) -> Color {
        match self {
            Self::TemperatePlains => Color::srgb(0.25, 0.72, 0.25),
            Self::FrigidTundra => Color::srgb(0.55, 0.82, 0.98),
            Self::AridDesert => Color::srgb(0.92, 0.78, 0.35),
            Self::MurkySwamp => Color::srgb(0.40, 0.55, 0.25),
            Self::VolcanicWasteland => Color::srgb(0.90, 0.35, 0.20),
            Self::DarkForest => Color::srgb(0.45, 0.30, 0.65),
        }
    }

    pub fn sky_ambient(&self) -> Color {
        match self {
            Self::TemperatePlains => Color::srgb(0.75, 0.84, 0.92),
            Self::FrigidTundra => Color::srgb(0.85, 0.92, 1.00),
            Self::AridDesert => Color::srgb(0.95, 0.85, 0.65),
            Self::MurkySwamp => Color::srgb(0.50, 0.60, 0.45),
            Self::VolcanicWasteland => Color::srgb(0.65, 0.35, 0.25),
            Self::DarkForest => Color::srgb(0.35, 0.30, 0.45),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ZoneDef {
    pub id: &'static str,
    pub name: &'static str,
    pub biome: BiomeType,
    pub world_center: Vec3,
    pub map_pos_normalized: Vec2, // Coordinates on EverQuest parchment map (0.0..1.0)
    pub danger_level: &'static str,
    pub description: &'static str,
    pub connections: &'static [&'static str],
}

pub const ZONE_CATALOG: &[ZoneDef] = &[
    ZoneDef {
        id: "karana_plains",
        name: "The Plains of Karana",
        biome: BiomeType::TemperatePlains,
        world_center: Vec3::new(0.0, 10.0, 0.0),
        map_pos_normalized: Vec2::new(0.35, 0.50),
        danger_level: "Level 1 - 10",
        description: "Expansive fertile grasslands watered by the winding river. Home to peaceful peasant hamlets and roaming deer.",
        connections: &["everfrost_peaks", "nektulos_forest", "oasis_of_ro"],
    },
    ZoneDef {
        id: "everfrost_peaks",
        name: "Everfrost Peaks",
        biome: BiomeType::FrigidTundra,
        world_center: Vec3::new(-150.0, 24.0, -150.0),
        map_pos_normalized: Vec2::new(0.20, 0.18),
        danger_level: "Level 10 - 20",
        description: "Glaciated alpine summits and blizzard winds. Only hardy mountain wildlife and frost goblins survive here.",
        connections: &["karana_plains", "lavastorm_ridge"],
    },
    ZoneDef {
        id: "nektulos_forest",
        name: "Nektulos Forest",
        biome: BiomeType::DarkForest,
        world_center: Vec3::new(140.0, 12.0, -100.0),
        map_pos_normalized: Vec2::new(0.70, 0.28),
        danger_level: "Level 5 - 15",
        description: "Dense pine groves cloaked in perpetual twilight shadows. Bandit camps and goblin outposts guard the trails.",
        connections: &["karana_plains", "innothule_swamp"],
    },
    ZoneDef {
        id: "lavastorm_ridge",
        name: "Lavastorm Ridge",
        biome: BiomeType::VolcanicWasteland,
        world_center: Vec3::new(-120.0, 32.0, 120.0),
        map_pos_normalized: Vec2::new(0.75, 0.12),
        danger_level: "Level 25 - 35",
        description: "Fractured obsidian volcanic plateaus steaming with magma vents and sulfur geysers. Highly perilous.",
        connections: &["everfrost_peaks", "innothule_swamp"],
    },
    ZoneDef {
        id: "innothule_swamp",
        name: "Innothule Swamp",
        biome: BiomeType::MurkySwamp,
        world_center: Vec3::new(80.0, 4.0, 150.0),
        map_pos_normalized: Vec2::new(0.50, 0.82),
        danger_level: "Level 15 - 25",
        description: "Muck-filled marshes and submerged ruins. Gnarled cypress timber and aggressive swamp raiders lurk in the reeds.",
        connections: &["nektulos_forest", "lavastorm_ridge", "oasis_of_ro"],
    },
    ZoneDef {
        id: "oasis_of_ro",
        name: "The Oasis of Marr",
        biome: BiomeType::AridDesert,
        world_center: Vec3::new(160.0, 8.0, 40.0),
        map_pos_normalized: Vec2::new(0.78, 0.62),
        danger_level: "Level 10 - 20",
        description: "Golden dunes surrounding a sacred desert oasis. Rich in sandstone outcroppings and flint deposits.",
        connections: &["karana_plains", "innothule_swamp"],
    },
];

// ----------------------------------------------------------------------------
// 2. GLOBAL DELTA BUFFERS (DETERMINISTIC THREAD-SAFE EDIT STATE)
// ----------------------------------------------------------------------------

static SCULPTED_HEIGHTS: OnceLock<RwLock<BTreeMap<(i32, i32), f32>>> = OnceLock::new();
static PAINTED_BIOMES: OnceLock<RwLock<BTreeMap<(i32, i32), [f32; 4]>>> = OnceLock::new();
static DIRTY_CHUNKS: OnceLock<RwLock<BTreeSet<(i32, i32)>>> = OnceLock::new();

#[inline]
fn get_sculpt_map() -> &'static RwLock<BTreeMap<(i32, i32), f32>> {
    SCULPTED_HEIGHTS.get_or_init(|| RwLock::new(BTreeMap::new()))
}

#[inline]
fn get_biome_map() -> &'static RwLock<BTreeMap<(i32, i32), [f32; 4]>> {
    PAINTED_BIOMES.get_or_init(|| RwLock::new(BTreeMap::new()))
}

#[inline]
fn get_dirty_chunks() -> &'static RwLock<BTreeSet<(i32, i32)>> {
    DIRTY_CHUNKS.get_or_init(|| RwLock::new(BTreeSet::new()))
}

/// Retrieves sculpted height offset for world coordinates (x, z).
pub fn get_sculpted_height_delta(x: f32, z: f32) -> f32 {
    let key = (x.round() as i32, z.round() as i32);
    if let Ok(map) = get_sculpt_map().read() {
        map.get(&key).copied().unwrap_or(0.0)
    } else {
        0.0
    }
}

/// Retrieves painted biome color override for world coordinates (x, z).
pub fn get_painted_biome_color(x: f32, z: f32) -> Option<[f32; 4]> {
    let key = (x.round() as i32, z.round() as i32);
    if let Ok(map) = get_biome_map().read() {
        map.get(&key).copied()
    } else {
        None
    }
}

/// Checks and consumes dirty status for chunk (cx, cz).
pub fn consume_chunk_dirty(cx: i32, cz: i32) -> bool {
    if let Ok(mut set) = get_dirty_chunks().write() {
        set.remove(&(cx, cz))
    } else {
        false
    }
}

pub fn mark_chunk_dirty(cx: i32, cz: i32) {
    if let Ok(mut set) = get_dirty_chunks().write() {
        set.insert((cx, cz));
    }
}

// ----------------------------------------------------------------------------
// 3. EDITOR ENUMS & RESOURCE STATE
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EditorToolCategory {
    #[default]
    TerrainSculpt,
    BiomePaint,
    Doodads,
    Units,
    ZoneLines,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SculptMode {
    #[default]
    Raise,
    Lower,
    Flatten,
    Smooth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DoodadChoice {
    #[default]
    AutumnOak,
    AlpinePine,
    DeadTree,
    GraniteBoulder,
    KnappedFlint,
    BerryBush,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnitChoice {
    #[default]
    PlayerStartLocation,
    PeasantWorker,
    ArmoredGoblin,
    WildStag,
    WildBoar,
}

#[derive(Resource)]
pub struct ZoneEditorState {
    pub is_editor_active: bool,
    pub is_world_map_open: bool,
    pub active_zone_id: &'static str,
    pub selected_zone_id: &'static str,
    
    // Tools
    pub tool_category: EditorToolCategory,
    pub sculpt_mode: SculptMode,
    pub paint_biome: BiomeType,
    pub doodad_choice: DoodadChoice,
    pub unit_choice: UnitChoice,
    pub target_zone_line: &'static str,
    pub player_team_index: u32,

    // Brush attributes
    pub brush_radius: f32,
    pub brush_strength: f32,
    pub target_flatten_height: f32,

    // Spatial cursor tracking
    pub cursor_hit_point: Option<Vec3>,
}

impl Default for ZoneEditorState {
    fn default() -> Self {
        Self {
            is_editor_active: false,
            is_world_map_open: false,
            active_zone_id: "karana_plains",
            selected_zone_id: "karana_plains",
            tool_category: EditorToolCategory::TerrainSculpt,
            sculpt_mode: SculptMode::Raise,
            paint_biome: BiomeType::TemperatePlains,
            doodad_choice: DoodadChoice::AutumnOak,
            unit_choice: UnitChoice::PlayerStartLocation,
            target_zone_line: "everfrost_peaks",
            player_team_index: 0,
            brush_radius: 6.0,
            brush_strength: 3.5,
            target_flatten_height: 12.0,
            cursor_hit_point: None,
        }
    }
}

// ----------------------------------------------------------------------------
// 4. ENTITY MARKERS
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct EditorBrushHologram;

#[derive(Component)]
pub struct EditorPaletteRoot;

#[derive(Component)]
pub struct WorldMapRoot;

#[derive(Component)]
pub struct StampedDoodad;

#[derive(Component)]
pub struct StampedUnit;

#[derive(Component)]
pub struct ZoneLineGateway {
    pub target_zone: &'static str,
}

#[derive(Component)]
pub struct WorldMapZoneButton {
    pub zone_id: &'static str,
}

#[derive(Component)]
pub struct WorldMapTravelButton;

#[derive(Component)]
pub struct WorldMapEditButton;

#[derive(Component)]
pub struct PaletteCategoryButton(pub EditorToolCategory);

#[derive(Component)]
pub struct PaletteSubModeButton(pub u32);

#[derive(Component)]
pub struct PaletteRadiusButton(pub f32);

#[derive(Component)]
pub struct WorldMapDetailText;

// ----------------------------------------------------------------------------
// 5. BRUSH SPATIAL RAYCASTING & SCULPTING SYSTEMS
// ----------------------------------------------------------------------------

pub fn update_editor_brush_cursor(
    mut editor: ResMut<ZoneEditorState>,
    camera_mode: Res<State<CameraMode>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    rts_cam_q: Query<(&Camera, &GlobalTransform), With<RtsCameraChild>>,
    spatial_query: SpatialQuery,
    mut brush_q: Query<(&mut BevyTransform, &mut Visibility), With<EditorBrushHologram>>,
) {
    if !editor.is_editor_active || *camera_mode.get() != CameraMode::RTS {
        for (_, mut vis) in brush_q.iter_mut() {
            *vis = Visibility::Hidden;
        }
        editor.cursor_hit_point = None;
        return;
    }

    let Ok(window) = window_query.get_single() else { return; };
    let Some(cursor_pos) = window.cursor_position() else {
        editor.cursor_hit_point = None;
        return;
    };

    let Ok((camera, cam_t)) = rts_cam_q.get_single() else { return; };
    let Some(ray) = camera.viewport_to_world(cam_t, cursor_pos) else { return; };

    let hit = spatial_query.cast_ray(
        ray.origin,
        ray.direction,
        1000.0,
        true,
        SpatialQueryFilter::from_mask([GameLayer::Terrain, GameLayer::Default]),
    );

    if let Some(hit_info) = hit {
        let hit_pos = ray.origin + ray.direction * hit_info.time_of_impact;
        editor.cursor_hit_point = Some(hit_pos);

        for (mut transform, mut vis) in brush_q.iter_mut() {
            transform.translation = hit_pos + Vec3::new(0.0, 0.15, 0.0);
            transform.scale = Vec3::new(editor.brush_radius, 1.0, editor.brush_radius);
            *vis = Visibility::Inherited;
        }
    } else {
        editor.cursor_hit_point = None;
        for (_, mut vis) in brush_q.iter_mut() {
            *vis = Visibility::Hidden;
        }
    }
}

pub fn handle_editor_brush_painting(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut editor: ResMut<ZoneEditorState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut default_doodad_mats: Local<Option<Handle<StandardMaterial>>>,
) {
    if !editor.is_editor_active || editor.is_world_map_open {
        return;
    }

    // Brush radius hotkeys [ and ]
    if keys.just_pressed(KeyCode::BracketLeft) {
        editor.brush_radius = (editor.brush_radius - 1.0).max(1.5);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        editor.brush_radius = (editor.brush_radius + 1.0).min(25.0);
    }

    let Some(hit_point) = editor.cursor_hit_point else { return; };

    // Continuous painting for Terrain Sculpting & Biome Painting
    if mouse_buttons.pressed(MouseButton::Left) {
        let dt = time.delta_seconds().min(0.05);
        let center = hit_point;
        let radius = editor.brush_radius;
        let radius_sq = radius * radius;
        let strength = editor.brush_strength * dt;

        let min_gx = ((center.x - radius).floor() as i32) - 1;
        let max_gx = ((center.x + radius).ceil() as i32) + 1;
        let min_gz = ((center.z - radius).floor() as i32) - 1;
        let max_gz = ((center.z + radius).ceil() as i32) + 1;

        match editor.tool_category {
            EditorToolCategory::TerrainSculpt => {
                if let Ok(mut sculpt_map) = get_sculpt_map().write() {
                    for gz in min_gz..=max_gz {
                        for gx in min_gx..=max_gx {
                            let wx = gx as f32;
                            let wz = gz as f32;
                            let dx = wx - center.x;
                            let dz = wz - center.z;
                            let dist_sq = dx * dx + dz * dz;
                            if dist_sq <= radius_sq {
                                let falloff = 1.0 - (dist_sq.sqrt() / radius).clamp(0.0, 1.0);
                                let delta = falloff * strength;
                                let key = (gx, gz);
                                let current_delta = sculpt_map.get(&key).copied().unwrap_or(0.0);

                                let new_delta = match editor.sculpt_mode {
                                    SculptMode::Raise => current_delta + delta,
                                    SculptMode::Lower => current_delta - delta,
                                    SculptMode::Flatten => {
                                        let current_total = crate::terrain::get_terrain_height(wx, wz);
                                        let diff = editor.target_flatten_height - current_total;
                                        current_delta + diff * delta.min(1.0)
                                    }
                                    SculptMode::Smooth => {
                                        let left = crate::terrain::get_terrain_height(wx - 1.0, wz);
                                        let right = crate::terrain::get_terrain_height(wx + 1.0, wz);
                                        let down = crate::terrain::get_terrain_height(wx, wz - 1.0);
                                        let up = crate::terrain::get_terrain_height(wx, wz + 1.0);
                                        let avg = (left + right + down + up) * 0.25;
                                        let cur = crate::terrain::get_terrain_height(wx, wz);
                                        current_delta + (avg - cur) * delta.min(1.0)
                                    }
                                };
                                sculpt_map.insert(key, new_delta);
                            }
                        }
                    }
                }

                // Invalidate affected 16m low-poly chunks
                let min_cx = (min_gx as f32 / 16.0).floor() as i32;
                let max_cx = (max_gx as f32 / 16.0).floor() as i32;
                let min_cz = (min_gz as f32 / 16.0).floor() as i32;
                let max_cz = (max_gz as f32 / 16.0).floor() as i32;
                for cz in min_cz..=max_cz {
                    for cx in min_cx..=max_cx {
                        mark_chunk_dirty(cx, cz);
                    }
                }
            }
            EditorToolCategory::BiomePaint => {
                let color = editor.paint_biome.primary_color();
                if let Ok(mut biome_map) = get_biome_map().write() {
                    for gz in min_gz..=max_gz {
                        for gx in min_gx..=max_gx {
                            let wx = gx as f32;
                            let wz = gz as f32;
                            let dx = wx - center.x;
                            let dz = wz - center.z;
                            if dx * dx + dz * dz <= radius_sq {
                                biome_map.insert((gx, gz), color);
                            }
                        }
                    }
                }

                let min_cx = (min_gx as f32 / 16.0).floor() as i32;
                let max_cx = (max_gx as f32 / 16.0).floor() as i32;
                let min_cz = (min_gz as f32 / 16.0).floor() as i32;
                let max_cz = (max_gz as f32 / 16.0).floor() as i32;
                for cz in min_cz..=max_cz {
                    for cx in min_cx..=max_cx {
                        mark_chunk_dirty(cx, cz);
                    }
                }
            }
            _ => {}
        }
    }

    // Discrete click-to-stamp for Doodads, Units, and Zone Lines
    if mouse_buttons.just_pressed(MouseButton::Left) {
        let white_mat = default_doodad_mats.get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.85,
                ..default()
            })
        }).clone();

        match editor.tool_category {
            EditorToolCategory::Doodads => {
                let (mesh, scale) = match editor.doodad_choice {
                    DoodadChoice::AutumnOak => (meshes.add(create_voxel_oak_mesh(1001)), 1.0),
                    DoodadChoice::AlpinePine => (meshes.add(create_voxel_pine_mesh(2002)), 1.1),
                    DoodadChoice::DeadTree => (meshes.add(create_voxel_dead_tree_mesh(3003)), 1.0),
                    DoodadChoice::GraniteBoulder => (meshes.add(create_voxel_rock_mesh()), 1.3),
                    DoodadChoice::KnappedFlint => (meshes.add(create_voxel_flint_mesh()), 1.0),
                    DoodadChoice::BerryBush => (meshes.add(create_voxel_bush_mesh()), 1.0),
                };

                let rot_y = (hit_point.x * 12.3 + hit_point.z * 45.6).sin() * std::f32::consts::PI;

                commands.spawn((
                    PbrBundle {
                        mesh,
                        material: white_mat,
                        transform: BevyTransform::from_translation(hit_point)
                            .with_rotation(Quat::from_rotation_y(rot_y))
                            .with_scale(Vec3::splat(scale)),
                        ..default()
                    },
                    RigidBody::Static,
                    Collider::cylinder(1.2 * scale, 0.4 * scale),
                    StampedDoodad,
                ));
                info!("Stamped doodad {:?} at {:?}", editor.doodad_choice, hit_point);
            }
            EditorToolCategory::Units => {
                let team_color = match editor.player_team_index {
                    0 => Color::srgb(0.1, 0.4, 0.9), // Player 1 Blue
                    1 => Color::srgb(0.0, 0.8, 0.8), // Player 2 Teal
                    2 => Color::srgb(0.6, 0.2, 0.8), // Player 3 Purple
                    _ => Color::srgb(0.9, 0.2, 0.2), // Neutral Hostile Red
                };

                match editor.unit_choice {
                    UnitChoice::PlayerStartLocation => {
                        // Stamp a glowing WC3-style starting location circle
                        let torus_mesh = meshes.add(bevy::math::primitives::Torus::new(2.4, 0.15));
                        let torus_mat = materials.add(StandardMaterial {
                            base_color: team_color,
                            emissive: LinearRgba::from(team_color),
                            unlit: true,
                            ..default()
                        });
                        commands.spawn((
                            PbrBundle {
                                mesh: torus_mesh,
                                material: torus_mat,
                                transform: BevyTransform::from_translation(hit_point + Vec3::new(0.0, 0.1, 0.0)),
                                ..default()
                            },
                            StampedUnit,
                        ));
                    }
                    UnitChoice::PeasantWorker => {
                        commands.spawn((
                            PbrBundle {
                                mesh: meshes.add(create_voxel_peasant_mesh()),
                                material: white_mat,
                                transform: BevyTransform::from_translation(hit_point + Vec3::new(0.0, 1.05, 0.0)),
                                ..default()
                            },
                            RigidBody::Dynamic,
                            Collider::cylinder(0.9, 0.35),
                            StampedUnit,
                        ));
                    }
                    UnitChoice::ArmoredGoblin => {
                        commands.spawn((
                            PbrBundle {
                                mesh: meshes.add(create_voxel_goblin_mesh()),
                                material: white_mat,
                                transform: BevyTransform::from_translation(hit_point + Vec3::new(0.0, 1.05, 0.0)),
                                ..default()
                            },
                            RigidBody::Dynamic,
                            Collider::cylinder(0.9, 0.35),
                            StampedUnit,
                        ));
                    }
                    UnitChoice::WildStag => {
                        commands.spawn((
                            PbrBundle {
                                mesh: meshes.add(create_voxel_deer_mesh()),
                                material: white_mat,
                                transform: BevyTransform::from_translation(hit_point + Vec3::new(0.0, 1.05, 0.0)),
                                ..default()
                            },
                            RigidBody::Dynamic,
                            Collider::cylinder(0.9, 0.5),
                            StampedUnit,
                        ));
                    }
                    UnitChoice::WildBoar => {
                        commands.spawn((
                            PbrBundle {
                                mesh: meshes.add(create_voxel_boar_mesh()),
                                material: white_mat,
                                transform: BevyTransform::from_translation(hit_point + Vec3::new(0.0, 0.8, 0.0)),
                                ..default()
                            },
                            RigidBody::Dynamic,
                            Collider::cylinder(0.8, 0.45),
                            StampedUnit,
                        ));
                    }
                }
                info!("Stamped unit {:?} at {:?}", editor.unit_choice, hit_point);
            }
            EditorToolCategory::ZoneLines => {
                // Stamp a glowing EverQuest portal gateway arch
                let portal_mat = materials.add(StandardMaterial {
                    base_color: Color::srgba(0.3, 0.85, 1.0, 0.8),
                    emissive: LinearRgba::new(1.5, 3.2, 4.0, 1.0),
                    unlit: true,
                    ..default()
                });
                let arch_mesh = meshes.add(bevy::math::primitives::Torus::new(3.5, 0.3));

                commands.spawn((
                    PbrBundle {
                        mesh: arch_mesh,
                        material: portal_mat,
                        transform: BevyTransform::from_translation(hit_point + Vec3::new(0.0, 3.2, 0.0))
                            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                        ..default()
                    },
                    ZoneLineGateway {
                        target_zone: editor.target_zone_line,
                    },
                ));
                info!("Stamped Zone Line Gateway to '{}' at {:?}", editor.target_zone_line, hit_point);
            }
            _ => {}
        }
    }
}

// ----------------------------------------------------------------------------
// 6. TOGGLE SYSTEMS FOR MAP PAINTER (F4) & EVERQUEST WORLD MAP (M)
// ----------------------------------------------------------------------------

pub fn toggle_zone_editor_and_world_map(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<ZoneEditorState>,
    mut next_camera: ResMut<NextState<CameraMode>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    mut palette_ui_q: Query<&mut Style, (With<EditorPaletteRoot>, Without<WorldMapRoot>)>,
    mut world_map_ui_q: Query<&mut Style, (With<WorldMapRoot>, Without<EditorPaletteRoot>)>,
) {
    let Ok(mut window) = window_q.get_single_mut() else { return; };

    // Toggle Warcraft 3 Zone Editor with F4
    if keys.just_pressed(KeyCode::F4) {
        editor.is_editor_active = !editor.is_editor_active;
        if editor.is_editor_active {
            next_camera.set(CameraMode::RTS);
            window.cursor.visible = true;
            window.cursor.grab_mode = bevy::window::CursorGrabMode::None;
            info!("Zone Editor: Activated. Switched to RTS overhead camera perspective.");
        }
    }

    // Toggle EverQuest World Map with M
    if keys.just_pressed(KeyCode::KeyM) {
        editor.is_world_map_open = !editor.is_world_map_open;
        if editor.is_world_map_open {
            window.cursor.visible = true;
            window.cursor.grab_mode = bevy::window::CursorGrabMode::None;
            info!("World Map: Opened EverQuest Continental Zone Atlas.");
        }
    }

    // Synchronize UI Styles
    for mut style in palette_ui_q.iter_mut() {
        style.display = if editor.is_editor_active && !editor.is_world_map_open {
            Display::Flex
        } else {
            Display::None
        };
    }

    for mut style in world_map_ui_q.iter_mut() {
        style.display = if editor.is_world_map_open {
            Display::Flex
        } else {
            Display::None
        };
    }
}

// ----------------------------------------------------------------------------
// 7. EVERQUEST WORLD MAP INTERACTIVE UI
// ----------------------------------------------------------------------------

pub fn handle_world_map_interactions(
    mut interaction_q: Query<(&Interaction, &WorldMapZoneButton), (Changed<Interaction>, With<Button>)>,
    travel_btn_q: Query<&Interaction, (Changed<Interaction>, With<WorldMapTravelButton>)>,
    edit_btn_q: Query<&Interaction, (Changed<Interaction>, With<WorldMapEditButton>)>,
    mut editor: ResMut<ZoneEditorState>,
    mut rts_rig_q: Query<&mut BevyTransform, With<RtsCameraRig>>,
    mut player_body_q: Query<&mut BevyTransform, (With<PlayerBody>, Without<RtsCameraRig>)>,
    mut next_camera: ResMut<NextState<CameraMode>>,
    mut detail_text_q: Query<&mut Text, With<WorldMapDetailText>>,
) {
    // 1. Zone selection click
    for (interaction, zone_btn) in interaction_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            editor.selected_zone_id = zone_btn.zone_id;
            info!("World Map: Selected zone '{}'", zone_btn.zone_id);

            if let Some(zone) = ZONE_CATALOG.iter().find(|z| z.id == zone_btn.zone_id) {
                let conn_str = zone.connections.join(", ");
                let details = format!(
                    "Zone: {}\nBiome: {}\nThreat: {}\n\n{}\n\nBorder Zone Connections:\n{}",
                    zone.name, zone.biome.name(), zone.danger_level, zone.description, conn_str
                );
                for mut text in detail_text_q.iter_mut() {
                    if let Some(section) = text.sections.get_mut(0) {
                        section.value = details.clone();
                    }
                }
            }
        }
    }

    // 2. Fast Travel Button Click
    for interaction in travel_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            if let Some(zone) = ZONE_CATALOG.iter().find(|z| z.id == editor.selected_zone_id) {
                editor.active_zone_id = zone.id;
                editor.is_world_map_open = false;

                // Move RTS Rig
                if let Ok(mut rig_t) = rts_rig_q.get_single_mut() {
                    rig_t.translation = zone.world_center;
                }
                // Move Player Body
                if let Ok(mut player_t) = player_body_q.get_single_mut() {
                    player_t.translation = zone.world_center;
                }
                info!("World Map: Fast traveled to '{}' at {:?}", zone.name, zone.world_center);
            }
        }
    }

    // 3. Edit in Map Painter Button Click
    for interaction in edit_btn_q.iter() {
        if *interaction == Interaction::Pressed {
            if let Some(zone) = ZONE_CATALOG.iter().find(|z| z.id == editor.selected_zone_id) {
                editor.active_zone_id = zone.id;
                editor.is_world_map_open = false;
                editor.is_editor_active = true;
                next_camera.set(CameraMode::RTS);

                if let Ok(mut rig_t) = rts_rig_q.get_single_mut() {
                    rig_t.translation = zone.world_center;
                }
                info!("World Map: Opened Zone Painter for '{}'", zone.name);
            }
        }
    }
}

// ----------------------------------------------------------------------------
// 8. WARCRAFT 3 TOOL PALETTE INTERACTIONS
// ----------------------------------------------------------------------------

pub fn handle_palette_interactions(
    mut cat_buttons: Query<(&Interaction, &PaletteCategoryButton), (Changed<Interaction>, With<Button>)>,
    mut sub_buttons: Query<(&Interaction, &PaletteSubModeButton), (Changed<Interaction>, With<Button>)>,
    mut rad_buttons: Query<(&Interaction, &PaletteRadiusButton), (Changed<Interaction>, With<Button>)>,
    mut editor: ResMut<ZoneEditorState>,
) {
    for (interaction, cat_btn) in cat_buttons.iter_mut() {
        if *interaction == Interaction::Pressed {
            editor.tool_category = cat_btn.0;
            info!("Zone Editor: Switched category to {:?}", cat_btn.0);
        }
    }

    for (interaction, sub_btn) in sub_buttons.iter_mut() {
        if *interaction == Interaction::Pressed {
            match editor.tool_category {
                EditorToolCategory::TerrainSculpt => {
                    editor.sculpt_mode = match sub_btn.0 {
                        0 => SculptMode::Raise,
                        1 => SculptMode::Lower,
                        2 => SculptMode::Flatten,
                        _ => SculptMode::Smooth,
                    };
                }
                EditorToolCategory::BiomePaint => {
                    editor.paint_biome = match sub_btn.0 {
                        0 => BiomeType::TemperatePlains,
                        1 => BiomeType::FrigidTundra,
                        2 => BiomeType::AridDesert,
                        3 => BiomeType::MurkySwamp,
                        4 => BiomeType::VolcanicWasteland,
                        _ => BiomeType::DarkForest,
                    };
                }
                EditorToolCategory::Doodads => {
                    editor.doodad_choice = match sub_btn.0 {
                        0 => DoodadChoice::AutumnOak,
                        1 => DoodadChoice::AlpinePine,
                        2 => DoodadChoice::DeadTree,
                        3 => DoodadChoice::GraniteBoulder,
                        4 => DoodadChoice::KnappedFlint,
                        _ => DoodadChoice::BerryBush,
                    };
                }
                EditorToolCategory::Units => {
                    editor.unit_choice = match sub_btn.0 {
                        0 => UnitChoice::PlayerStartLocation,
                        1 => UnitChoice::PeasantWorker,
                        2 => UnitChoice::ArmoredGoblin,
                        3 => UnitChoice::WildStag,
                        _ => UnitChoice::WildBoar,
                    };
                }
                EditorToolCategory::ZoneLines => {
                    editor.target_zone_line = match sub_btn.0 {
                        0 => "everfrost_peaks",
                        1 => "nektulos_forest",
                        2 => "lavastorm_ridge",
                        3 => "innothule_swamp",
                        4 => "oasis_of_ro",
                        _ => "karana_plains",
                    };
                }
            }
        }
    }

    for (interaction, rad_btn) in rad_buttons.iter_mut() {
        if *interaction == Interaction::Pressed {
            editor.brush_radius = (editor.brush_radius + rad_btn.0).clamp(1.5, 25.0);
            info!("Zone Editor: Brush radius adjusted to {:.1}m", editor.brush_radius);
        }
    }
}

// ----------------------------------------------------------------------------
// 9. INITIAL SETUP FOR BRUSH HOLOGRAM & RETRO UI PANELS
// ----------------------------------------------------------------------------

pub fn setup_zone_editor_visuals_and_ui(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // 1. 3D Brush Hologram
    let cylinder_mesh = meshes.add(bevy::math::primitives::Cylinder::new(1.0, 0.08));
    let brush_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 0.85, 1.0, 0.45),
        emissive: LinearRgba::new(0.4, 1.2, 1.8, 1.0),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands.spawn((
        PbrBundle {
            mesh: cylinder_mesh,
            material: brush_mat,
            transform: BevyTransform::from_xyz(0.0, -100.0, 0.0),
            visibility: Visibility::Hidden,
            ..default()
        },
        EditorBrushHologram,
    ));

    // 2. Warcraft 3 Floating Tool Palette UI
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                top: Val::Px(48.0),
                width: Val::Px(310.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(3.0)),
                display: Display::None, // Controlled by toggle system
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.08, 0.14, 0.09, 0.94)), // WC3 Forest Slate
            border_color: BorderColor(Color::srgb(0.60, 0.50, 0.30)), // Beveled brass
            ..default()
        },
        EditorPaletteRoot,
    )).with_children(|palette| {
        // Window Title Bar
        palette.spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(4.0)),
                margin: UiRect::bottom(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::srgb(0.12, 0.22, 0.15)),
            border_color: BorderColor(Color::srgb(0.70, 0.60, 0.35)),
            ..default()
        }).with_children(|title_bar| {
            title_bar.spawn(TextBundle::from_section(
                "TOOL PALETTE - ZONE PAINTER [F4]",
                TextStyle { font_size: 13.0, color: Color::srgb(1.0, 0.92, 0.65), ..default() },
            ));
        });

        // Category Tabs Row
        palette.spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            ..default()
        }).with_children(|tabs| {
            let cat_data = [
                ("Sculpt", EditorToolCategory::TerrainSculpt),
                ("Biome", EditorToolCategory::BiomePaint),
                ("Doodad", EditorToolCategory::Doodads),
                ("Unit", EditorToolCategory::Units),
                ("Portal", EditorToolCategory::ZoneLines),
            ];
            for (label, cat) in cat_data {
                tabs.spawn((
                    ButtonBundle {
                        style: Style {
                            padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgb(0.16, 0.28, 0.20)),
                        border_color: BorderColor(Color::srgb(0.70, 0.60, 0.35)),
                        ..default()
                    },
                    PaletteCategoryButton(cat),
                )).with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        label,
                        TextStyle { font_size: 11.0, color: Color::WHITE, ..default() },
                    ));
                });
            }
        });

        // Brush Settings Controls (Radius - / +)
        palette.spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(8.0)),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
            ..default()
        }).with_children(|rad_row| {
            rad_row.spawn(TextBundle::from_section(
                "Brush Radius [ / ]:",
                TextStyle { font_size: 11.0, color: Color::srgb(0.9, 0.85, 0.7), ..default() },
            ));

            rad_row.spawn((
                ButtonBundle {
                    style: Style { padding: UiRect::all(Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), ..default() },
                    background_color: BackgroundColor(Color::srgb(0.2, 0.3, 0.25)),
                    border_color: BorderColor(Color::srgb(0.6, 0.5, 0.3)),
                    ..default()
                },
                PaletteRadiusButton(-1.5),
            )).with_children(|b| {
                b.spawn(TextBundle::from_section("-1.5m", TextStyle { font_size: 10.0, color: Color::WHITE, ..default() }));
            });

            rad_row.spawn((
                ButtonBundle {
                    style: Style { padding: UiRect::all(Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), ..default() },
                    background_color: BackgroundColor(Color::srgb(0.2, 0.3, 0.25)),
                    border_color: BorderColor(Color::srgb(0.6, 0.5, 0.3)),
                    ..default()
                },
                PaletteRadiusButton(1.5),
            )).with_children(|b| {
                b.spawn(TextBundle::from_section("+1.5m", TextStyle { font_size: 10.0, color: Color::WHITE, ..default() }));
            });
        });

        // Sub-Mode Action Buttons Grid (4x2 grid of options)
        palette.spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            ..default()
        }).with_children(|grid| {
            let options = [
                ("1. Primary", 0),
                ("2. Secondary", 1),
                ("3. Alternate", 2),
                ("4. Special", 3),
            ];
            for (label, idx) in options {
                grid.spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Percent(48.0),
                            padding: UiRect::axes(Val::Px(4.0), Val::Px(5.0)),
                            margin: UiRect::bottom(Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgb(0.18, 0.25, 0.20)),
                        border_color: BorderColor(Color::srgb(0.55, 0.50, 0.30)),
                        ..default()
                    },
                    PaletteSubModeButton(idx),
                )).with_children(|b| {
                    b.spawn(TextBundle::from_section(
                        label,
                        TextStyle { font_size: 10.0, color: Color::srgb(0.95, 0.95, 0.85), ..default() },
                    ));
                });
            }
        });

        // Instructions Footer
        palette.spawn(TextBundle::from_section(
            "LMB: Paint/Stamp | [ / ]: Size\nM: World Map | F4: Exit",
            TextStyle { font_size: 10.0, color: Color::srgb(0.7, 0.75, 0.65), ..default() },
        ));
    });

    // 3. EverQuest Fullscreen Continental Zone Map UI
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Percent(8.0),
                right: Val::Percent(8.0),
                top: Val::Percent(6.0),
                bottom: Val::Percent(6.0),
                flex_direction: FlexDirection::Row,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(4.0)),
                display: Display::None, // Controlled by toggle system
                ..default()
            },
            background_color: BackgroundColor(Color::srgb(0.14, 0.12, 0.10)), // Aged Norrath Parchment
            border_color: BorderColor(Color::srgb(0.72, 0.58, 0.32)), // Gold inlay border
            ..default()
        },
        WorldMapRoot,
    )).with_children(|map_root| {
        // Left Column: Interactive Continent Zone Canvas
        map_root.spawn(NodeBundle {
            style: Style {
                width: Val::Percent(68.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.19, 0.16, 0.13, 0.95)),
            border_color: BorderColor(Color::srgb(0.55, 0.44, 0.25)),
            ..default()
        }).with_children(|canvas| {
            // Atlas Header
            canvas.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(12.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ..default()
            }).with_children(|h| {
                h.spawn(TextBundle::from_section(
                    "NORRATH / ANTONICA INTERCONNECTED ZONE MAP [M]",
                    TextStyle { font_size: 18.0, color: Color::srgb(0.95, 0.85, 0.60), ..default() },
                ));
            });

            // Interactive Zone Nodes Grid (6 Connected Zones)
            canvas.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::SpaceAround,
                    align_content: AlignContent::SpaceAround,
                    ..default()
                },
                ..default()
            }).with_children(|nodes_container| {
                for zone in ZONE_CATALOG {
                    nodes_container.spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Percent(45.0),
                                height: Val::Px(78.0),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::all(Val::Px(6.0)),
                                margin: UiRect::all(Val::Px(6.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                justify_content: JustifyContent::SpaceBetween,
                                ..default()
                            },
                            background_color: BackgroundColor(Color::srgb(0.24, 0.20, 0.16)),
                            border_color: BorderColor(zone.biome.badge_color()),
                            ..default()
                        },
                        WorldMapZoneButton { zone_id: zone.id },
                    )).with_children(|card| {
                        // Title & Biome Badge Row
                        card.spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            ..default()
                        }).with_children(|row| {
                            row.spawn(TextBundle::from_section(
                                zone.name,
                                TextStyle { font_size: 13.0, color: Color::srgb(1.0, 0.95, 0.80), ..default() },
                            ));
                            row.spawn(NodeBundle {
                                style: Style {
                                    padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                background_color: BackgroundColor(zone.biome.badge_color()),
                                ..default()
                            }).with_children(|badge| {
                                badge.spawn(TextBundle::from_section(
                                    zone.biome.name(),
                                    TextStyle { font_size: 9.0, color: Color::BLACK, ..default() },
                                ));
                            });
                        });

                        // Danger Level & Connections subtitle
                        let conn_text = format!("Links: {}", zone.connections.join(", "));
                        card.spawn(TextBundle::from_section(
                            format!("Threat: {} | {}", zone.danger_level, conn_text),
                            TextStyle { font_size: 9.5, color: Color::srgb(0.8, 0.75, 0.65), ..default() },
                        ));
                    });
                }
            });
        });

        // Right Column: Zone Inspection Drawer & Travel Actions
        map_root.spawn(NodeBundle {
            style: Style {
                width: Val::Percent(30.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                margin: UiRect::left(Val::Px(12.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.18, 0.15, 0.12, 0.95)),
            border_color: BorderColor(Color::srgb(0.55, 0.44, 0.25)),
            ..default()
        }).with_children(|drawer| {
            drawer.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ..default()
            }).with_children(|top| {
                top.spawn(TextBundle::from_section(
                    "ZONE DETAILS",
                    TextStyle { font_size: 14.0, color: Color::srgb(0.95, 0.85, 0.55), ..default() },
                ));

                top.spawn((
                    TextBundle::from_section(
                        "Click any zone to inspect topology, biomes, and travel routes.",
                        TextStyle { font_size: 11.0, color: Color::srgb(0.85, 0.80, 0.70), ..default() },
                    ).with_style(Style { margin: UiRect::top(Val::Px(8.0)), ..default() }),
                    WorldMapDetailText,
                ));
            });

            // Action Buttons
            drawer.spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ..default()
            }).with_children(|actions| {
                actions.spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(8.0)),
                            margin: UiRect::bottom(Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgb(0.20, 0.45, 0.25)),
                        border_color: BorderColor(Color::srgb(0.70, 0.60, 0.35)),
                        ..default()
                    },
                    WorldMapTravelButton,
                )).with_children(|b| {
                    b.spawn(TextBundle::from_section(
                        "Fast Travel / Enter Zone",
                        TextStyle { font_size: 12.0, color: Color::WHITE, ..default() },
                    ));
                });

                actions.spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgb(0.25, 0.35, 0.50)),
                        border_color: BorderColor(Color::srgb(0.70, 0.60, 0.35)),
                        ..default()
                    },
                    WorldMapEditButton,
                )).with_children(|b| {
                    b.spawn(TextBundle::from_section(
                        "Edit in Zone Painter (F4)",
                        TextStyle { font_size: 12.0, color: Color::WHITE, ..default() },
                    ));
                });
            });
        });
    });

    info!("Zone Editor & EverQuest World Map visual pipelines initialized.");
}

// ----------------------------------------------------------------------------
// 10. PLUGIN DEFINITION
// ----------------------------------------------------------------------------

pub struct ZoneEditorPlugin;

impl Plugin for ZoneEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ZoneEditorState>()
            .add_systems(OnEnter(GameState::InGame), setup_zone_editor_visuals_and_ui)
            .add_systems(
                Update,
                (
                    toggle_zone_editor_and_world_map,
                    update_editor_brush_cursor,
                    handle_editor_brush_painting,
                    handle_world_map_interactions,
                    handle_palette_interactions,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}
