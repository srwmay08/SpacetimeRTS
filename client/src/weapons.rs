// ============================================================================
// File: client/src/weapons.rs
// ============================================================================
// ----------------------------------------------------------------------------
// WEAPONS MODULE & FIRST-PERSON VOXEL VIEWMODELS
// ----------------------------------------------------------------------------
// Implements first-person voxel models for Bows, Crossbows, Hand Crossbows,
// Revolvers, and Shotguns with authentic FPS mechanics:
// - Bow: Draw & hold charge, ballistic arrow trajectory scaling with charge.
// - Crossbow: Instant high-velocity piercing bolt, long mechanical crank reload.
// - Hand Crossbow: Fast tactical sidearm with rapid reload.
// - Revolver: Punchy 6-shot cylinder, crisp vertical recoil, single/double-action timing.
// - Shotgun: Pump-action 8-pellet buckshot spread cone with pump animation & falloff.

use bevy::prelude::{Transform as BevyTransform, *};
use bevy::render::view::RenderLayers;
use crate::components::*;
use crate::core::*;

use spacetime_rts_logic::HandSide;

// ----------------------------------------------------------------------------
// WEAPON ENUM & STATE
// ----------------------------------------------------------------------------

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct EquippedHandSide(pub HandSide);

impl Default for EquippedHandSide {
    fn default() -> Self {
        Self(HandSide::Right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeaponType {
    #[default]
    None,
    Bow,
    Crossbow,
    HandCrossbow,
    Revolver,
    Shotgun,
    SniperRifle,
    BouncyBombLauncher,
    Runestaff,
    Halberd,
    Longsword,
    Greatsword,
    Maul,
    Spear,
    Rapier,
    Warhammer,
    Club,
    Dagger,
    Handaxe,
    Cestus,
    KnuckleDuster,
    FryingPan,
    HolyMackerel,
    Hammer,
    Pickaxe,
    Torch,
    WoodenShield,
}

impl WeaponType {
    pub fn from_item_name(item: Option<&str>) -> Self {
        match item {
            Some("Crude Bow") | Some("Bow") | Some("Longbow") => Self::Bow,
            Some("Crossbow") => Self::Crossbow,
            Some("Hand Crossbow") => Self::HandCrossbow,
            Some("Revolver") => Self::Revolver,
            Some("Shotgun") => Self::Shotgun,
            Some("Sniper Rifle") => Self::SniperRifle,
            Some("Bouncy Bomb Launcher") => Self::BouncyBombLauncher,
            Some("Runestaff") => Self::Runestaff,
            Some("Halberd") => Self::Halberd,
            Some("Longsword") => Self::Longsword,
            Some("Greatsword") => Self::Greatsword,
            Some("Maul") => Self::Maul,
            Some("Spear") | Some("Flint Spear") => Self::Spear,
            Some("Rapier") => Self::Rapier,
            Some("Warhammer") => Self::Warhammer,
            Some("Club") => Self::Club,
            Some("Dagger") => Self::Dagger,
            Some("Handaxe") | Some("Stone Axe") => Self::Handaxe,
            Some("Cestus") => Self::Cestus,
            Some("Knuckle-Duster") => Self::KnuckleDuster,
            Some("Frying Pan") => Self::FryingPan,
            Some("Holy Mackerel") => Self::HolyMackerel,
            Some("Hammer") => Self::Hammer,
            Some("Pickaxe") => Self::Pickaxe,
            Some("Torch") => Self::Torch,
            Some("Wooden Shield") => Self::WoodenShield,
            _ => Self::None,
        }
    }

    #[allow(dead_code)]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "Unarmed",
            Self::Bow => "Bow",
            Self::Crossbow => "Heavy Crossbow",
            Self::HandCrossbow => "Hand Crossbow",
            Self::Revolver => "Six-Shooter Revolver",
            Self::Shotgun => "Pump Shotgun",
            Self::SniperRifle => "High-Velocity Sniper Rifle",
            Self::BouncyBombLauncher => "Bouncy Bomb Launcher",
            Self::Runestaff => "Arcane Runestaff",
            Self::Halberd => "Halberd Polearm",
            Self::Longsword => "Knight's Longsword",
            Self::Greatsword => "Zweihander Greatsword",
            Self::Maul => "Heavy Iron Maul",
            Self::Spear => "Hunting Spear",
            Self::Rapier => "Fencing Rapier",
            Self::Warhammer => "Flanged Warhammer",
            Self::Club => "Knotted War Club",
            Self::Dagger => "Stiletto Dagger",
            Self::Handaxe => "Bearded Handaxe",
            Self::Cestus => "Spiked Cestus",
            Self::KnuckleDuster => "Cast-Iron Knuckle-Duster",
            Self::FryingPan => "Cast-Iron Frying Pan",
            Self::HolyMackerel => "Holy Mackerel",
            Self::Hammer => "Construction Hammer",
            Self::Pickaxe => "Mining Pickaxe",
            Self::Torch => "Pitch Torch",
            Self::WoodenShield => "Reinforced Wooden Shield",
        }
    }

    pub fn is_ranged(&self) -> bool {
        matches!(
            self,
            Self::Bow
                | Self::Crossbow
                | Self::HandCrossbow
                | Self::Revolver
                | Self::Shotgun
                | Self::SniperRifle
                | Self::BouncyBombLauncher
                | Self::Runestaff
        )
    }

    pub fn is_melee(&self) -> bool {
        !self.is_ranged()
    }
}

#[derive(Resource)]
pub struct WeaponState {
    pub current_weapon: WeaponType,
    pub last_hand: HandSide,
    
    // Bow Mechanics
    pub bow_charge: f32, // 0.0 to 1.0
    pub bow_drawing: bool,

    // Crossbow Mechanics
    pub crossbow_loaded: bool,
    pub crossbow_reload_timer: Timer,

    // Hand Crossbow Mechanics
    pub hand_crossbow_loaded: bool,
    pub hand_crossbow_reload_timer: Timer,

    // Revolver Mechanics
    pub revolver_ammo: u32,
    pub revolver_max_ammo: u32,
    pub revolver_cooldown: Timer,
    pub revolver_reload_timer: Timer,
    pub revolver_is_reloading: bool,

    // Shotgun Mechanics
    pub shotgun_ammo: u32,
    pub shotgun_max_ammo: u32,
    pub shotgun_pump_timer: Timer,
    pub shotgun_is_pumping: bool,
    pub shotgun_reload_timer: Timer,
    pub shotgun_is_reloading: bool,

    // Viewmodel Physics (Recoil Kick & Sway)
    pub recoil_offset: Vec3,
    pub recoil_rot: Quat,
    pub sway_time: f32,
    pub dynamic_bloom: f32,
}

impl Default for WeaponState {
    fn default() -> Self {
        Self {
            current_weapon: WeaponType::None,
            last_hand: HandSide::Right,
            bow_charge: 0.0,
            bow_drawing: false,
            crossbow_loaded: true,
            crossbow_reload_timer: Timer::from_seconds(1.4, TimerMode::Once),
            hand_crossbow_loaded: true,
            hand_crossbow_reload_timer: Timer::from_seconds(0.65, TimerMode::Once),
            revolver_ammo: 6,
            revolver_max_ammo: 6,
            revolver_cooldown: Timer::from_seconds(0.28, TimerMode::Once),
            revolver_reload_timer: Timer::from_seconds(1.6, TimerMode::Once),
            revolver_is_reloading: false,
            shotgun_ammo: 4,
            shotgun_max_ammo: 4,
            shotgun_pump_timer: Timer::from_seconds(0.65, TimerMode::Once),
            shotgun_is_pumping: false,
            shotgun_reload_timer: Timer::from_seconds(1.8, TimerMode::Once),
            shotgun_is_reloading: false,
            recoil_offset: Vec3::ZERO,
            recoil_rot: Quat::IDENTITY,
            sway_time: 0.0,
            dynamic_bloom: 0.0,
        }
    }
}

// ----------------------------------------------------------------------------
// VIEWMODEL MARKER COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct ViewModelWeaponRoot;

#[derive(Component)]
pub struct ViewModelPumpSlide;

#[derive(Component)]
pub struct ViewModelCrossbowBolt;

#[derive(Component)]
pub struct ViewModelBowArrow;

#[derive(Component)]
pub struct WeaponHudText;

// ----------------------------------------------------------------------------
// HAND SWITCHING & VOXEL MODEL GENERATION FOR FIRST-PERSON VIEW
// ----------------------------------------------------------------------------

pub fn toggle_weapon_hand_system(
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    mut hand_side: ResMut<EquippedHandSide>,
    mut weapon_root_q: Query<&mut BevyTransform, With<ViewModelWeaponRoot>>,
) {
    if console.is_open {
        return;
    }

    if keys.just_pressed(KeyCode::KeyH) {
        hand_side.0 = match hand_side.0 {
            HandSide::Right => HandSide::Left,
            HandSide::Left => HandSide::Right,
        };
        info!("Toggled Primary Hand: {:?}", hand_side.0);
        for mut t in weapon_root_q.iter_mut() {
            t.translation.x = -t.translation.x;
        }
    }
}

pub fn spawn_or_update_view_model_weapon(
    mut commands: Commands,
    active_item: Res<ActiveEquippedItem>,
    mut weapon_state: ResMut<WeaponState>,
    hand_side: Res<EquippedHandSide>,
    camera_query: Query<Entity, With<FpsCamera>>,
    existing_weapon_q: Query<Entity, With<ViewModelWeaponRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let desired_weapon = WeaponType::from_item_name(active_item.0.as_deref());

    if desired_weapon == weapon_state.current_weapon && (desired_weapon == WeaponType::None || !existing_weapon_q.is_empty()) && weapon_state.last_hand == hand_side.0 {
        return;
    }

    // Despawn old weapon model
    for entity in existing_weapon_q.iter() {
        commands.entity(entity).despawn_recursive();
    }

    weapon_state.current_weapon = desired_weapon;
    weapon_state.last_hand = hand_side.0;

    let Ok(camera_entity) = camera_query.get_single() else {
        return;
    };

    if desired_weapon == WeaponType::None {
        return;
    }

    // Materials Palette (Authentic Voxel Palette)
    let wood_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.22, 0.12),
        perceptual_roughness: 0.85,
        ..default()
    });
    let wood_light = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.38, 0.22),
        perceptual_roughness: 0.8,
        ..default()
    });
    let iron_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.23, 0.25),
        metallic: 0.85,
        perceptual_roughness: 0.35,
        ..default()
    });
    let iron_bright = materials.add(StandardMaterial {
        base_color: Color::srgb(0.48, 0.50, 0.52),
        metallic: 0.7,
        perceptual_roughness: 0.4,
        ..default()
    });
    let brass_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.62, 0.22),
        metallic: 0.9,
        perceptual_roughness: 0.3,
        ..default()
    });
    let string_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.90, 0.82),
        unlit: true,
        ..default()
    });
    let red_fletch = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.15, 0.15),
        ..default()
    });
    let rune_cyan = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.85, 1.0),
        unlit: true,
        ..default()
    });
    let fire_orange = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.45, 0.05),
        unlit: true,
        ..default()
    });

    let mut default_pos = match desired_weapon {
        WeaponType::Bow => Vec3::new(0.22, -0.18, -0.42),
        WeaponType::Crossbow => Vec3::new(0.20, -0.22, -0.45),
        WeaponType::HandCrossbow => Vec3::new(0.22, -0.20, -0.40),
        WeaponType::Revolver => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::Shotgun => Vec3::new(0.22, -0.24, -0.46),
        WeaponType::SniperRifle => Vec3::new(0.22, -0.22, -0.48),
        WeaponType::BouncyBombLauncher => Vec3::new(0.24, -0.24, -0.44),
        WeaponType::Runestaff => Vec3::new(0.24, -0.26, -0.45),
        WeaponType::Halberd => Vec3::new(0.25, -0.24, -0.52),
        WeaponType::Longsword => Vec3::new(0.22, -0.20, -0.42),
        WeaponType::Greatsword => Vec3::new(0.24, -0.22, -0.48),
        WeaponType::Maul => Vec3::new(0.24, -0.22, -0.45),
        WeaponType::Spear => Vec3::new(0.24, -0.20, -0.50),
        WeaponType::Rapier => Vec3::new(0.22, -0.18, -0.42),
        WeaponType::Warhammer => Vec3::new(0.22, -0.20, -0.40),
        WeaponType::Club => Vec3::new(0.22, -0.22, -0.38),
        WeaponType::Dagger => Vec3::new(0.20, -0.18, -0.34),
        WeaponType::Handaxe => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::Cestus => Vec3::new(0.18, -0.18, -0.30),
        WeaponType::KnuckleDuster => Vec3::new(0.18, -0.18, -0.28),
        WeaponType::FryingPan => Vec3::new(0.24, -0.22, -0.40),
        WeaponType::HolyMackerel => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::Hammer => Vec3::new(0.20, -0.20, -0.36),
        WeaponType::Pickaxe => Vec3::new(0.22, -0.20, -0.40),
        WeaponType::Torch => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::WoodenShield => Vec3::new(-0.24, -0.18, -0.38),
        WeaponType::None => Vec3::ZERO,
    };

    if hand_side.0 == HandSide::Left && desired_weapon != WeaponType::WoodenShield {
        default_pos.x = -default_pos.x;
    } else if hand_side.0 == HandSide::Left && desired_weapon == WeaponType::WoodenShield {
        default_pos.x = 0.24;
    }

    let root_rot = if hand_side.0 == HandSide::Left {
        Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06)
    } else {
        Quat::IDENTITY
    };

    commands.entity(camera_entity).with_children(|parent| {
        let mut root = parent.spawn((
            SpatialBundle {
                transform: BevyTransform::from_translation(default_pos).with_rotation(root_rot),
                ..default()
            },
            ViewModelWeaponRoot,
            RenderLayers::layer(1),
        ));

        root.with_children(|builder| {
            match desired_weapon {
                WeaponType::Bow => {
                    // Central Handle Grip
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.04, 0.12, 0.04), Vec3::new(0.0, 0.0, 0.0));
                    // Upper Limb
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.035, 0.14, 0.035), Vec3::new(0.0, 0.12, -0.04));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.03, 0.14, 0.03), Vec3::new(0.0, 0.24, -0.10));
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.025, 0.08, 0.025), Vec3::new(0.0, 0.33, -0.14));
                    // Lower Limb
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.035, 0.14, 0.035), Vec3::new(0.0, -0.12, -0.04));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.03, 0.14, 0.03), Vec3::new(0.0, -0.24, -0.10));
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.025, 0.08, 0.025), Vec3::new(0.0, -0.33, -0.14));
                    // Bowstring
                    spawn_voxel_box(builder, &mut meshes, string_white.clone(), Vec3::new(0.005, 0.36, 0.005), Vec3::new(0.0, 0.17, -0.02));
                    spawn_voxel_box(builder, &mut meshes, string_white.clone(), Vec3::new(0.005, 0.36, 0.005), Vec3::new(0.0, -0.17, -0.02));
                    // Nocked Arrow
                    builder.spawn((
                        PbrBundle {
                            mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.015, 0.015, 0.52)),
                            material: wood_light.clone(),
                            transform: BevyTransform::from_xyz(-0.01, 0.02, -0.12),
                            ..default()
                        },
                        ViewModelBowArrow,
                        RenderLayers::layer(1),
                    )).with_children(|arrow| {
                        spawn_voxel_box(arrow, &mut meshes, iron_bright.clone(), Vec3::new(0.03, 0.01, 0.05), Vec3::new(0.0, 0.0, -0.27));
                        spawn_voxel_box(arrow, &mut meshes, red_fletch.clone(), Vec3::new(0.008, 0.04, 0.07), Vec3::new(0.0, 0.0, 0.22));
                    });
                }
                WeaponType::Crossbow => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.065, 0.075, 0.52), Vec3::new(0.0, 0.0, -0.12));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.055, 0.11, 0.16), Vec3::new(0.0, -0.04, 0.18));
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.045, 0.12, 0.06), Vec3::new(0.0, -0.09, 0.04));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.52, 0.035, 0.035), Vec3::new(0.0, 0.03, -0.32));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.12, 0.025, 0.08), Vec3::new(0.0, -0.01, -0.42));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.03, 0.015, 0.38), Vec3::new(0.0, 0.042, -0.14));
                    builder.spawn((
                        PbrBundle {
                            mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.018, 0.018, 0.30)),
                            material: wood_light.clone(),
                            transform: BevyTransform::from_xyz(0.0, 0.055, -0.16),
                            ..default()
                        },
                        ViewModelCrossbowBolt,
                        RenderLayers::layer(1),
                    )).with_children(|bolt| {
                        spawn_voxel_box(bolt, &mut meshes, iron_bright.clone(), Vec3::new(0.025, 0.025, 0.05), Vec3::new(0.0, 0.0, -0.16));
                        spawn_voxel_box(bolt, &mut meshes, red_fletch.clone(), Vec3::new(0.005, 0.035, 0.05), Vec3::new(0.0, 0.0, 0.12));
                    });
                }
                WeaponType::HandCrossbow => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.04, 0.11, 0.05), Vec3::new(0.0, -0.08, 0.05));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.045, 0.055, 0.32), Vec3::new(0.0, 0.0, -0.10));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.32, 0.025, 0.025), Vec3::new(0.0, 0.025, -0.22));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.02, 0.04, 0.03), Vec3::new(0.0, -0.04, -0.02));
                    builder.spawn((
                        PbrBundle {
                            mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.015, 0.015, 0.18)),
                            material: iron_bright.clone(),
                            transform: BevyTransform::from_xyz(0.0, 0.035, -0.12),
                            ..default()
                        },
                        ViewModelCrossbowBolt,
                        RenderLayers::layer(1),
                    ));
                }
                WeaponType::Revolver => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.042, 0.12, 0.06), Vec3::new(0.0, -0.08, 0.06));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.044, 0.075, 0.13), Vec3::new(0.0, 0.0, -0.02));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.02, 0.04, 0.05), Vec3::new(0.0, -0.055, -0.01));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.062, 0.062, 0.09), Vec3::new(0.0, 0.005, -0.06));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.04, 0.04, 0.01), Vec3::new(0.0, 0.005, -0.012));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.04, 0.045, 0.28), Vec3::new(0.0, 0.018, -0.24));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.015, 0.015, 0.28), Vec3::new(0.0, 0.044, -0.24));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.012, 0.02, 0.02), Vec3::new(0.0, 0.052, -0.36));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.018, 0.035, 0.03), Vec3::new(0.0, 0.045, 0.04));
                }
                WeaponType::Shotgun => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.055, 0.09, 0.24), Vec3::new(0.0, -0.04, 0.16));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.06, 0.08, 0.18), Vec3::new(0.0, 0.01, -0.04));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.01, 0.03, 0.05), Vec3::new(0.031, 0.02, -0.04));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.042, 0.042, 0.44), Vec3::new(0.0, 0.032, -0.34));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.038, 0.038, 0.38), Vec3::new(0.0, -0.008, -0.31));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.014, 0.016, 0.014), Vec3::new(0.0, 0.058, -0.54));
                    builder.spawn((
                        PbrBundle {
                            mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.054, 0.054, 0.14)),
                            material: wood_light.clone(),
                            transform: BevyTransform::from_xyz(0.0, -0.008, -0.26),
                            ..default()
                        },
                        ViewModelPumpSlide,
                        RenderLayers::layer(1),
                    ));
                }
                WeaponType::SniperRifle => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.05, 0.10, 0.28), Vec3::new(0.0, -0.05, 0.18));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.055, 0.08, 0.20), Vec3::new(0.0, 0.01, -0.05));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.038, 0.038, 0.65), Vec3::new(0.0, 0.025, -0.45));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.032, 0.032, 0.28), Vec3::new(0.0, 0.085, -0.10));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.038, 0.038, 0.02), Vec3::new(0.0, 0.085, -0.22));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.038, 0.038, 0.02), Vec3::new(0.0, 0.085, 0.02));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.06, 0.02, 0.12), Vec3::new(0.0, -0.02, -0.55));
                }
                WeaponType::BouncyBombLauncher => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.06, 0.11, 0.24), Vec3::new(0.0, -0.05, 0.16));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.055, 0.12, 0.12), Vec3::new(0.0, -0.06, 0.02));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.12, 0.12, 0.16), Vec3::new(0.0, 0.01, -0.12));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.125, 0.125, 0.02), Vec3::new(0.0, 0.01, -0.12));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.075, 0.075, 0.32), Vec3::new(0.0, 0.02, -0.34));
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.05, 0.07, 0.12), Vec3::new(0.0, -0.04, -0.32));
                }
                WeaponType::Runestaff => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.045, 0.045, 0.90), Vec3::new(0.0, 0.0, -0.15));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.055, 0.055, 0.04), Vec3::new(0.0, 0.0, -0.45));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.055, 0.055, 0.04), Vec3::new(0.0, 0.0, -0.20));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.08, 0.08, 0.08), Vec3::new(0.0, 0.0, -0.62));
                    spawn_voxel_box(builder, &mut meshes, rune_cyan.clone(), Vec3::new(0.065, 0.065, 0.12), Vec3::new(0.0, 0.0, -0.72));
                }
                WeaponType::Halberd => {
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.04, 0.04, 1.10), Vec3::new(0.0, 0.0, -0.20));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.055, 0.055, 0.24), Vec3::new(0.0, 0.0, -0.65));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.18, 0.02, 0.22), Vec3::new(0.11, 0.0, -0.72));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.12, 0.025, 0.06), Vec3::new(-0.08, 0.0, -0.72));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.03, 0.02, 0.26), Vec3::new(0.0, 0.0, -0.88));
                }
                WeaponType::Longsword => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.035, 0.035, 0.16), Vec3::new(0.0, 0.0, 0.08));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.05, 0.05, 0.04), Vec3::new(0.0, 0.0, 0.18));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.24, 0.025, 0.035), Vec3::new(0.0, 0.0, -0.02));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.06, 0.015, 0.68), Vec3::new(0.0, 0.0, -0.38));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.015, 0.018, 0.52), Vec3::new(0.0, 0.0, -0.32));
                }
                WeaponType::Greatsword => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.038, 0.038, 0.26), Vec3::new(0.0, 0.0, 0.15));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.06, 0.06, 0.05), Vec3::new(0.0, 0.0, 0.30));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.36, 0.03, 0.04), Vec3::new(0.0, 0.0, 0.0));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.07, 0.02, 0.14), Vec3::new(0.0, 0.0, -0.09));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.16, 0.02, 0.03), Vec3::new(0.0, 0.0, -0.17));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.08, 0.016, 0.85), Vec3::new(0.0, 0.0, -0.60));
                }
                WeaponType::Maul => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.045, 0.045, 0.75), Vec3::new(0.0, 0.0, -0.15));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.14, 0.14, 0.22), Vec3::new(0.0, 0.0, -0.55));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.13, 0.13, 0.02), Vec3::new(0.0, 0.0, -0.67));
                }
                WeaponType::Spear => {
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.032, 0.032, 1.05), Vec3::new(0.0, 0.0, -0.15));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.045, 0.045, 0.08), Vec3::new(0.0, 0.0, -0.68));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.065, 0.015, 0.26), Vec3::new(0.0, 0.0, -0.84));
                }
                WeaponType::Rapier => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.028, 0.028, 0.14), Vec3::new(0.0, 0.0, 0.08));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.14, 0.14, 0.10), Vec3::new(0.0, 0.0, 0.01));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.025, 0.025, 0.78), Vec3::new(0.0, 0.0, -0.42));
                }
                WeaponType::Warhammer => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.04, 0.04, 0.65), Vec3::new(0.0, 0.0, -0.12));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.09, 0.09, 0.12), Vec3::new(0.05, 0.0, -0.46));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.10, 0.03, 0.04), Vec3::new(-0.06, 0.0, -0.46));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.025, 0.025, 0.12), Vec3::new(0.0, 0.0, -0.56));
                }
                WeaponType::Club => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.05, 0.05, 0.35), Vec3::new(0.0, 0.0, 0.0));
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.095, 0.095, 0.28), Vec3::new(0.0, 0.0, -0.28));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.11, 0.11, 0.04), Vec3::new(0.0, 0.0, -0.28));
                }
                WeaponType::Dagger => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.032, 0.032, 0.12), Vec3::new(0.0, 0.0, 0.06));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.10, 0.025, 0.025), Vec3::new(0.0, 0.0, -0.01));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.042, 0.012, 0.30), Vec3::new(0.0, 0.0, -0.16));
                }
                WeaponType::Handaxe => {
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.036, 0.036, 0.48), Vec3::new(0.0, 0.0, -0.10));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.05, 0.05, 0.08), Vec3::new(0.0, 0.0, -0.32));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.14, 0.018, 0.16), Vec3::new(0.08, 0.0, -0.36));
                }
                WeaponType::Cestus => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.09, 0.09, 0.22), Vec3::new(0.0, 0.0, -0.02));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.10, 0.04, 0.06), Vec3::new(0.0, 0.04, -0.14));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.09, 0.02, 0.03), Vec3::new(0.0, 0.065, -0.14));
                }
                WeaponType::KnuckleDuster => {
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.08, 0.03, 0.025), Vec3::new(0.0, -0.02, -0.02));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.11, 0.05, 0.04), Vec3::new(0.0, 0.02, -0.06));
                }
                WeaponType::FryingPan => {
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.24, 0.02, 0.24), Vec3::new(0.0, 0.0, -0.22));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.26, 0.05, 0.02), Vec3::new(0.0, 0.02, -0.34));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.26, 0.05, 0.02), Vec3::new(0.0, 0.02, -0.10));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.02, 0.05, 0.24), Vec3::new(-0.12, 0.02, -0.22));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.02, 0.05, 0.24), Vec3::new(0.12, 0.02, -0.22));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.035, 0.025, 0.18), Vec3::new(0.0, 0.02, 0.0));
                }
                WeaponType::HolyMackerel => {
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.06, 0.12, 0.38), Vec3::new(0.0, 0.0, -0.16));
                    spawn_voxel_box(builder, &mut meshes, string_white.clone(), Vec3::new(0.055, 0.03, 0.36), Vec3::new(0.0, -0.05, -0.16));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.02, 0.14, 0.10), Vec3::new(0.0, 0.0, 0.08));
                    spawn_voxel_box(builder, &mut meshes, string_white.clone(), Vec3::new(0.05, 0.08, 0.12), Vec3::new(0.0, 0.0, 0.0));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.065, 0.025, 0.025), Vec3::new(0.0, 0.02, -0.30));
                }
                WeaponType::Hammer => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.035, 0.035, 0.40), Vec3::new(0.0, 0.0, -0.06));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.07, 0.07, 0.13), Vec3::new(0.0, 0.0, -0.24));
                }
                WeaponType::Pickaxe => {
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.038, 0.038, 0.52), Vec3::new(0.0, 0.0, -0.10));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.28, 0.04, 0.06), Vec3::new(0.0, 0.0, -0.34));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.32, 0.025, 0.03), Vec3::new(0.0, 0.0, -0.34));
                }
                WeaponType::Torch => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.038, 0.038, 0.46), Vec3::new(0.0, 0.0, -0.08));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.065, 0.065, 0.12), Vec3::new(0.0, 0.0, -0.28));
                    spawn_voxel_box(builder, &mut meshes, fire_orange.clone(), Vec3::new(0.09, 0.12, 0.09), Vec3::new(0.0, 0.04, -0.38));
                }
                WeaponType::WoodenShield => {
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.44, 0.44, 0.04), Vec3::new(0.0, 0.0, 0.0));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.46, 0.46, 0.02), Vec3::new(0.0, 0.0, -0.01));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.14, 0.14, 0.08), Vec3::new(0.0, 0.0, -0.04));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.18, 0.05, 0.03), Vec3::new(0.0, 0.0, 0.03));
                }
                WeaponType::None => {}
            }
        });
    });
}

fn spawn_voxel_box(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    size: Vec3,
    translation: Vec3,
) {
    parent.spawn((
        PbrBundle {
            mesh: meshes.add(bevy::math::primitives::Cuboid::new(size.x, size.y, size.z)),
            material,
            transform: BevyTransform::from_translation(translation),
            ..default()
        },
        RenderLayers::layer(1),
    ));
}

// ----------------------------------------------------------------------------
// VIEWMODEL ANIMATION, RECOIL & FPS WEAPON STATE UPDATE
// ----------------------------------------------------------------------------

pub fn animate_weapon_viewmodel(
    time: Res<Time>,
    mut weapon_state: ResMut<WeaponState>,
    hand_side: Res<EquippedHandSide>,
    mut root_q: Query<&mut BevyTransform, With<ViewModelWeaponRoot>>,
    mut pump_q: Query<&mut BevyTransform, (With<ViewModelPumpSlide>, Without<ViewModelWeaponRoot>)>,
    mut arrow_q: Query<&mut Visibility, (With<ViewModelBowArrow>, Without<ViewModelCrossbowBolt>)>,
    mut bolt_q: Query<&mut Visibility, (With<ViewModelCrossbowBolt>, Without<ViewModelBowArrow>)>,
) {
    let dt = time.delta_seconds();
    weapon_state.sway_time += dt;

    // 1. Ticking Reload & Action Timers
    if !weapon_state.crossbow_loaded {
        weapon_state.crossbow_reload_timer.tick(time.delta());
        if weapon_state.crossbow_reload_timer.just_finished() {
            weapon_state.crossbow_loaded = true;
            weapon_state.crossbow_reload_timer.reset();
        }
    }

    if !weapon_state.hand_crossbow_loaded {
        weapon_state.hand_crossbow_reload_timer.tick(time.delta());
        if weapon_state.hand_crossbow_reload_timer.just_finished() {
            weapon_state.hand_crossbow_loaded = true;
            weapon_state.hand_crossbow_reload_timer.reset();
        }
    }

    if weapon_state.revolver_is_reloading {
        weapon_state.revolver_reload_timer.tick(time.delta());
        if weapon_state.revolver_reload_timer.just_finished() {
            weapon_state.revolver_ammo = weapon_state.revolver_max_ammo;
            weapon_state.revolver_is_reloading = false;
            weapon_state.revolver_reload_timer.reset();
        }
    }
    weapon_state.revolver_cooldown.tick(time.delta());

    if weapon_state.shotgun_is_reloading {
        weapon_state.shotgun_reload_timer.tick(time.delta());
        if weapon_state.shotgun_reload_timer.just_finished() {
            weapon_state.shotgun_ammo = weapon_state.shotgun_max_ammo;
            weapon_state.shotgun_is_reloading = false;
            weapon_state.shotgun_reload_timer.reset();
        }
    }

    if weapon_state.shotgun_is_pumping {
        weapon_state.shotgun_pump_timer.tick(time.delta());
        let t = weapon_state.shotgun_pump_timer.fraction();
        let pump_slide_z = if t < 0.5 {
            -0.26 + (t * 2.0) * 0.08
        } else {
            -0.18 - ((t - 0.5) * 2.0) * 0.08
        };

        for mut pump_t in pump_q.iter_mut() {
            pump_t.translation.z = pump_slide_z;
        }

        if weapon_state.shotgun_pump_timer.just_finished() {
            weapon_state.shotgun_is_pumping = false;
            weapon_state.shotgun_pump_timer.reset();
        }
    }

    // 2. Bolt/Arrow Visibility
    for mut vis in bolt_q.iter_mut() {
        let is_visible = match weapon_state.current_weapon {
            WeaponType::Crossbow => weapon_state.crossbow_loaded,
            WeaponType::HandCrossbow => weapon_state.hand_crossbow_loaded,
            _ => true,
        };
        *vis = if is_visible { Visibility::Inherited } else { Visibility::Hidden };
    }

    for mut vis in arrow_q.iter_mut() {
        *vis = Visibility::Inherited;
    }

    // 3. Recoil Recovery (Exponential Decay) & Dynamic Bloom Decay
    weapon_state.recoil_offset = weapon_state.recoil_offset.lerp(Vec3::ZERO, (dt * 14.0).min(1.0));
    weapon_state.recoil_rot = weapon_state.recoil_rot.slerp(Quat::IDENTITY, (dt * 16.0).min(1.0));
    weapon_state.dynamic_bloom = (weapon_state.dynamic_bloom - dt * 28.0).max(0.0);
    if weapon_state.current_weapon == WeaponType::Bow && weapon_state.bow_drawing {
        weapon_state.dynamic_bloom = (1.0 - weapon_state.bow_charge) * 12.0;
    }

    // 4. Transform Animation on ViewModelWeaponRoot
    let Ok(mut root_t) = root_q.get_single_mut() else { return; };

    let mut default_pos = match weapon_state.current_weapon {
        WeaponType::Bow => Vec3::new(0.22, -0.18, -0.42),
        WeaponType::Crossbow => Vec3::new(0.20, -0.22, -0.45),
        WeaponType::HandCrossbow => Vec3::new(0.22, -0.20, -0.40),
        WeaponType::Revolver => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::Shotgun => Vec3::new(0.22, -0.24, -0.46),
        WeaponType::SniperRifle => Vec3::new(0.22, -0.22, -0.48),
        WeaponType::BouncyBombLauncher => Vec3::new(0.24, -0.24, -0.44),
        WeaponType::Runestaff => Vec3::new(0.24, -0.26, -0.45),
        WeaponType::Halberd => Vec3::new(0.25, -0.24, -0.52),
        WeaponType::Longsword => Vec3::new(0.22, -0.20, -0.42),
        WeaponType::Greatsword => Vec3::new(0.24, -0.22, -0.48),
        WeaponType::Maul => Vec3::new(0.24, -0.22, -0.45),
        WeaponType::Spear => Vec3::new(0.24, -0.20, -0.50),
        WeaponType::Rapier => Vec3::new(0.22, -0.18, -0.42),
        WeaponType::Warhammer => Vec3::new(0.22, -0.20, -0.40),
        WeaponType::Club => Vec3::new(0.22, -0.22, -0.38),
        WeaponType::Dagger => Vec3::new(0.20, -0.18, -0.34),
        WeaponType::Handaxe => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::Cestus => Vec3::new(0.18, -0.18, -0.30),
        WeaponType::KnuckleDuster => Vec3::new(0.18, -0.18, -0.28),
        WeaponType::FryingPan => Vec3::new(0.24, -0.22, -0.40),
        WeaponType::HolyMackerel => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::Hammer => Vec3::new(0.20, -0.20, -0.36),
        WeaponType::Pickaxe => Vec3::new(0.22, -0.20, -0.40),
        WeaponType::Torch => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::WoodenShield => Vec3::new(-0.24, -0.18, -0.38),
        WeaponType::None => Vec3::ZERO,
    };

    if hand_side.0 == HandSide::Left && weapon_state.current_weapon != WeaponType::WoodenShield {
        default_pos.x = -default_pos.x;
    } else if hand_side.0 == HandSide::Left && weapon_state.current_weapon == WeaponType::WoodenShield {
        default_pos.x = 0.24;
    }

    // Idle Breathing Sway
    let sway_x = (weapon_state.sway_time * 1.5).sin() * 0.003;
    let sway_y = (weapon_state.sway_time * 3.0).cos() * 0.002;
    let mut current_offset = default_pos + Vec3::new(sway_x, sway_y, 0.0) + weapon_state.recoil_offset;
    let mut current_rot = weapon_state.recoil_rot;

    // Bow ADS & Draw Stance
    if weapon_state.current_weapon == WeaponType::Bow && weapon_state.bow_drawing {
        let charge = weapon_state.bow_charge;
        current_offset += Vec3::new(-0.10 * charge, 0.05 * charge, 0.12 * charge);
        current_rot *= Quat::from_rotation_z(0.35 * charge) * Quat::from_rotation_x(0.12 * charge);
    }

    // Crossbow Cranking Stance
    if !weapon_state.crossbow_loaded && weapon_state.current_weapon == WeaponType::Crossbow {
        let t = weapon_state.crossbow_reload_timer.fraction();
        let dip = (t * std::f32::consts::PI).sin() * 0.06;
        current_offset.y -= dip;
        current_rot *= Quat::from_rotation_x(0.25 * dip);
    }

    root_t.translation = current_offset;
    root_t.rotation = current_rot;
}

// ----------------------------------------------------------------------------
// MANUAL RELOAD SYSTEM ('R' KEY)
// ----------------------------------------------------------------------------

pub fn weapon_reload_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut weapon_state: ResMut<WeaponState>,
    console: Res<ConsoleState>,
) {
    if console.is_open {
        return;
    }

    if keys.just_pressed(KeyCode::KeyR) {
        match weapon_state.current_weapon {
            WeaponType::Revolver => {
                if weapon_state.revolver_ammo < weapon_state.revolver_max_ammo && !weapon_state.revolver_is_reloading {
                    weapon_state.revolver_is_reloading = true;
                    weapon_state.revolver_reload_timer.reset();
                    weapon_state.recoil_rot *= Quat::from_rotation_z(0.3);
                }
            }
            WeaponType::Shotgun => {
                if weapon_state.shotgun_ammo < weapon_state.shotgun_max_ammo && !weapon_state.shotgun_is_reloading && !weapon_state.shotgun_is_pumping {
                    weapon_state.shotgun_is_reloading = true;
                    weapon_state.shotgun_reload_timer.reset();
                    weapon_state.recoil_offset += Vec3::new(0.0, -0.04, 0.04);
                }
            }
            WeaponType::Crossbow => {
                if !weapon_state.crossbow_loaded {
                    weapon_state.crossbow_reload_timer.reset();
                }
            }
            WeaponType::HandCrossbow => {
                if !weapon_state.hand_crossbow_loaded {
                    weapon_state.hand_crossbow_reload_timer.reset();
                }
            }
            _ => {}
        }
    }
}

// ----------------------------------------------------------------------------
// WEAPON HUD STATUS SYSTEM
// ----------------------------------------------------------------------------

pub fn update_weapon_hud(
    weapon_state: Res<WeaponState>,
    hand_side: Res<EquippedHandSide>,
    camera_mode: Res<State<CameraMode>>,
    mut text_q: Query<(&mut Visibility, &mut Text), With<WeaponHudText>>,
) {
    let Ok((mut vis, mut text)) = text_q.get_single_mut() else { return; };
    if *camera_mode.get() != CameraMode::FPS || weapon_state.current_weapon == WeaponType::None {
        *vis = Visibility::Hidden;
        return;
    }

    *vis = Visibility::Inherited;
    let hand_str = match hand_side.0 {
        HandSide::Right => "PRIMARY: RIGHT [H to Swap]",
        HandSide::Left => "PRIMARY: LEFT [H to Swap]",
    };

    let details = match weapon_state.current_weapon {
        WeaponType::Bow => {
            if weapon_state.bow_drawing {
                let pct = (weapon_state.bow_charge * 100.0) as u32;
                format!("BOW [DRAWING: {}%]\n[Release] Fire | [Right-Click] Cancel", pct)
            } else {
                "BOW [READY]\n[Hold Left-Click] Draw".to_string()
            }
        }
        WeaponType::Crossbow => {
            if weapon_state.crossbow_loaded {
                "HEAVY CROSSBOW [READY]\n[Left-Click] Fire Piercing Bolt".to_string()
            } else {
                let rem = weapon_state.crossbow_reload_timer.remaining_secs();
                format!("HEAVY CROSSBOW [CRANKING... {:.1}s]", rem)
            }
        }
        WeaponType::HandCrossbow => {
            if weapon_state.hand_crossbow_loaded {
                "HAND CROSSBOW [READY]\n[Left-Click] Fire Dart".to_string()
            } else {
                "HAND CROSSBOW [RELOADING...]".to_string()
            }
        }
        WeaponType::Revolver => {
            if weapon_state.revolver_is_reloading {
                "REVOLVER [RELOADING CYLINDER...]".to_string()
            } else {
                format!("REVOLVER [{} / {}]\n[R] Reload", weapon_state.revolver_ammo, weapon_state.revolver_max_ammo)
            }
        }
        WeaponType::Shotgun => {
            if weapon_state.shotgun_is_reloading {
                "SHOTGUN [RELOADING SHELLS...]".to_string()
            } else if weapon_state.shotgun_is_pumping {
                "SHOTGUN [CYCLING PUMP...]".to_string()
            } else {
                format!("SHOTGUN [{} / {}]\n[R] Reload", weapon_state.shotgun_ammo, weapon_state.shotgun_max_ammo)
            }
        }
        _ => {
            format!("{} [READY]\n[Left-Click] Attack", weapon_state.current_weapon.display_name())
        }
    };

    text.sections[0].value = format!("{}\n{}", hand_str, details);
}
