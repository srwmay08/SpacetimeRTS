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
    Wand,
    Orb,
    Halberd,
    Longsword,
    Greatsword,
    TwoHandAxe,
    Maul,
    Spear,
    Javelin,
    Trident,
    Rapier,
    Warhammer,
    Club,
    Dagger,
    Handaxe,
    TigerClaws,
    BlackJack,
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
            // Unarmed
            Some("Unarmed") => Self::None,

            // 1H Tiger Claws
            Some("1h Tiger Claws") | Some("Tiger Claws") => Self::TigerClaws,

            // 1H Black Jack
            Some("1h Black Jack") | Some("Black Jack") | Some("Blackjack") => Self::BlackJack,

            // 1H Sword
            Some("1h Sword") | Some("Sword") | Some("Longsword") | Some("Knight's Longsword") => Self::Longsword,

            // 1H Hammer
            Some("1h Hammer") | Some("Warhammer") | Some("Flanged Warhammer") => Self::Warhammer,

            // 1H Axe
            Some("1h Axe") | Some("Handaxe") | Some("Stone Axe") | Some("Bearded Handaxe") => Self::Handaxe,

            // 2H Sword
            Some("2h Sword") | Some("Greatsword") | Some("Two-Handed Sword") | Some("Zweihander Greatsword") => Self::Greatsword,

            // 2H Hammer
            Some("2h Hammer") | Some("Maul") | Some("Two-Handed Hammer") | Some("Heavy Iron Maul") => Self::Maul,

            // 2H Axe
            Some("2h Axe") | Some("Battleaxe") | Some("Greataxe") | Some("Two-Handed Axe") => Self::TwoHandAxe,

            // Polearms
            Some("Polearm Spear") | Some("Spear") | Some("Flint Spear") | Some("Hunting Spear") => Self::Spear,
            Some("Polearm Javelin - Thrown") | Some("Polearm Javelin") | Some("Javelin") | Some("Thrown Javelin") => Self::Javelin,
            Some("Polearm Trident") | Some("Trident") => Self::Trident,
            Some("Halberd") => Self::Halberd,

            // 1H Ranged
            Some("1h Ranged Hand Crossbow") | Some("Hand Crossbow") => Self::HandCrossbow,
            Some("1h Ranged Revolver") | Some("Revolver") => Self::Revolver,
            Some("1h Ranged Wand") | Some("Wand") | Some("Arcane Wand") => Self::Wand,
            Some("1h Ranged Orb") | Some("Orb") | Some("Mystic Orb") => Self::Orb,

            // 2H Ranged
            Some("2h Ranged Long Bow") | Some("Long Bow") | Some("Longbow") | Some("Crude Bow") | Some("Bow") => Self::Bow,
            Some("2h Ranged Shotgun") | Some("Shotgun") => Self::Shotgun,
            Some("2h Ranged Sniper Rifle") | Some("Sniper Rifle") => Self::SniperRifle,
            Some("2h Ranged Runestaff") | Some("Runestaff") => Self::Runestaff,
            Some("Crossbow") => Self::Crossbow,
            Some("Bouncy Bomb Launcher") => Self::BouncyBombLauncher,

            // Additional Melee & Tools
            Some("Rapier") => Self::Rapier,
            Some("Club") => Self::Club,
            Some("Dagger") => Self::Dagger,
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
            Self::TigerClaws => "1h Tiger Claws",
            Self::BlackJack => "1h Black Jack",
            Self::Longsword => "1h Sword",
            Self::Warhammer => "1h Hammer",
            Self::Handaxe => "1h Axe",
            Self::Greatsword => "2h Sword",
            Self::Maul => "2h Hammer",
            Self::TwoHandAxe => "2h Axe",
            Self::Spear => "Polearm Spear",
            Self::Javelin => "Polearm Javelin (Thrown)",
            Self::Trident => "Polearm Trident",
            Self::HandCrossbow => "1h Ranged Hand Crossbow",
            Self::Revolver => "1h Ranged Revolver",
            Self::Bow => "2h Ranged Long Bow",
            Self::Shotgun => "2h Ranged Shotgun",
            Self::SniperRifle => "2h Ranged Sniper Rifle",
            Self::Wand => "1h Ranged Wand",
            Self::Orb => "1h Ranged Orb",
            Self::Runestaff => "2h Ranged Runestaff",
            Self::Crossbow => "Heavy Crossbow",
            Self::Halberd => "Halberd Polearm",
            Self::BouncyBombLauncher => "Bouncy Bomb Launcher",
            Self::Rapier => "Fencing Rapier",
            Self::Club => "Knotted War Club",
            Self::Dagger => "Stiletto Dagger",
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
                | Self::Wand
                | Self::Orb
                | Self::Javelin
        )
    }

    pub fn is_melee(&self) -> bool {
        !self.is_ranged()
    }

    pub fn is_two_handed(&self) -> bool {
        matches!(
            self,
            Self::Bow
                | Self::Crossbow
                | Self::Shotgun
                | Self::SniperRifle
                | Self::BouncyBombLauncher
                | Self::Runestaff
                | Self::Halberd
                | Self::Greatsword
                | Self::TwoHandAxe
                | Self::Maul
                | Self::Spear
                | Self::Trident
        )
    }

    pub fn is_one_handed(&self) -> bool {
        !self.is_two_handed()
    }
}

#[derive(Resource)]
pub struct WeaponState {
    pub current_weapon: WeaponType,
    pub offhand_weapon: WeaponType,
    pub last_hand: HandSide,
    #[allow(dead_code)]
    pub offhand_last_hand: HandSide,
    
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
    pub offhand_recoil_offset: Vec3,
    pub offhand_recoil_rot: Quat,
    pub sway_time: f32,
    pub dynamic_bloom: f32,
}

impl Default for WeaponState {
    fn default() -> Self {
        Self {
            current_weapon: WeaponType::None,
            offhand_weapon: WeaponType::None,
            last_hand: HandSide::Right,
            offhand_last_hand: HandSide::Left,
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
            offhand_recoil_offset: Vec3::ZERO,
            offhand_recoil_rot: Quat::IDENTITY,
            sway_time: 0.0,
            dynamic_bloom: 0.0,
        }
    }
}

// ----------------------------------------------------------------------------
// VIEWMODEL MARKER COMPONENTS
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct ViewModelWeaponRoot {
    pub is_offhand: bool,
}

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

pub struct VoxelPalette {
    pub wood_dark: Handle<StandardMaterial>,
    pub wood_light: Handle<StandardMaterial>,
    pub iron_dark: Handle<StandardMaterial>,
    pub iron_bright: Handle<StandardMaterial>,
    pub brass_gold: Handle<StandardMaterial>,
    pub string_white: Handle<StandardMaterial>,
    pub red_fletch: Handle<StandardMaterial>,
    pub rune_cyan: Handle<StandardMaterial>,
    pub fire_orange: Handle<StandardMaterial>,
    pub skin_tone: Handle<StandardMaterial>,
    pub wrap_cloth: Handle<StandardMaterial>,
}

struct MeshDeref<'a>(&'a mut Assets<Mesh>);
impl<'a> std::ops::Deref for MeshDeref<'a> {
    type Target = Assets<Mesh>;
    fn deref(&self) -> &Self::Target { self.0 }
}
impl<'a> std::ops::DerefMut for MeshDeref<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target { self.0 }
}

pub fn get_default_weapon_pos(weapon: WeaponType, is_left: bool) -> Vec3 {
    let mut pos = match weapon {
        WeaponType::Bow => Vec3::new(0.22, -0.18, -0.42),
        WeaponType::Crossbow => Vec3::new(0.20, -0.22, -0.45),
        WeaponType::HandCrossbow => Vec3::new(0.22, -0.20, -0.40),
        WeaponType::Revolver => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::Shotgun => Vec3::new(0.22, -0.24, -0.46),
        WeaponType::SniperRifle => Vec3::new(0.22, -0.22, -0.48),
        WeaponType::BouncyBombLauncher => Vec3::new(0.24, -0.24, -0.44),
        WeaponType::Runestaff => Vec3::new(0.24, -0.26, -0.45),
        WeaponType::Wand => Vec3::new(0.20, -0.18, -0.38),
        WeaponType::Orb => Vec3::new(0.22, -0.20, -0.36),
        WeaponType::Halberd => Vec3::new(0.25, -0.24, -0.52),
        WeaponType::Longsword => Vec3::new(0.22, -0.20, -0.42),
        WeaponType::Greatsword => Vec3::new(0.24, -0.22, -0.48),
        WeaponType::TwoHandAxe => Vec3::new(0.24, -0.22, -0.48),
        WeaponType::Maul => Vec3::new(0.24, -0.22, -0.45),
        WeaponType::Spear => Vec3::new(0.24, -0.20, -0.50),
        WeaponType::Javelin => Vec3::new(0.22, -0.20, -0.46),
        WeaponType::Trident => Vec3::new(0.24, -0.22, -0.52),
        WeaponType::Rapier => Vec3::new(0.22, -0.18, -0.42),
        WeaponType::Warhammer => Vec3::new(0.22, -0.20, -0.40),
        WeaponType::Club => Vec3::new(0.22, -0.22, -0.38),
        WeaponType::BlackJack => Vec3::new(0.20, -0.20, -0.34),
        WeaponType::Dagger => Vec3::new(0.20, -0.18, -0.34),
        WeaponType::Handaxe => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::TigerClaws => Vec3::new(0.18, -0.18, -0.28),
        WeaponType::Cestus => Vec3::new(0.18, -0.18, -0.30),
        WeaponType::KnuckleDuster => Vec3::new(0.18, -0.18, -0.28),
        WeaponType::FryingPan => Vec3::new(0.24, -0.22, -0.40),
        WeaponType::HolyMackerel => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::Hammer => Vec3::new(0.20, -0.20, -0.36),
        WeaponType::Pickaxe => Vec3::new(0.22, -0.20, -0.40),
        WeaponType::Torch => Vec3::new(0.22, -0.20, -0.38),
        WeaponType::WoodenShield => Vec3::new(-0.24, -0.18, -0.38),
        WeaponType::None => Vec3::new(0.20, -0.20, -0.32), // Boxer guard default fist
    };

    if is_left {
        if weapon == WeaponType::WoodenShield {
            pos.x = -pos.x;
        } else {
            pos.x = -pos.x;
        }
    }
    pos
}

pub fn spawn_or_update_view_model_weapon(
    mut commands: Commands,
    active_item: Res<ActiveEquippedItem>,
    active_offhand: Res<ActiveOffHandItem>,
    mut weapon_state: ResMut<WeaponState>,
    hand_side: Res<EquippedHandSide>,
    camera_query: Query<Entity, With<FpsCamera>>,
    existing_weapon_q: Query<Entity, With<ViewModelWeaponRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let desired_main = WeaponType::from_item_name(active_item.0.as_deref());
    let desired_off = WeaponType::from_item_name(active_offhand.0.as_deref());

    let need_respawn = desired_main != weapon_state.current_weapon
        || desired_off != weapon_state.offhand_weapon
        || weapon_state.last_hand != hand_side.0
        || existing_weapon_q.is_empty();

    if !need_respawn {
        return;
    }

    // Despawn old weapon models
    for entity in existing_weapon_q.iter() {
        commands.entity(entity).despawn_recursive();
    }

    weapon_state.current_weapon = desired_main;
    weapon_state.offhand_weapon = desired_off;
    weapon_state.last_hand = hand_side.0;

    let Ok(camera_entity) = camera_query.get_single() else {
        return;
    };

    // Materials Palette (Authentic Voxel Palette)
    let palette = VoxelPalette {
        wood_dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.22, 0.12),
            perceptual_roughness: 0.85,
            ..default()
        }),
        wood_light: materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.38, 0.22),
            perceptual_roughness: 0.8,
            ..default()
        }),
        iron_dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.23, 0.25),
            metallic: 0.85,
            perceptual_roughness: 0.35,
            ..default()
        }),
        iron_bright: materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.50, 0.52),
            metallic: 0.7,
            perceptual_roughness: 0.4,
            ..default()
        }),
        brass_gold: materials.add(StandardMaterial {
            base_color: Color::srgb(0.78, 0.62, 0.22),
            metallic: 0.9,
            perceptual_roughness: 0.3,
            ..default()
        }),
        string_white: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.90, 0.82),
            unlit: true,
            ..default()
        }),
        red_fletch: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.15, 0.15),
            ..default()
        }),
        rune_cyan: materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.85, 1.0),
            unlit: true,
            ..default()
        }),
        fire_orange: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.45, 0.05),
            unlit: true,
            ..default()
        }),
        skin_tone: materials.add(StandardMaterial {
            base_color: Color::srgb(0.86, 0.68, 0.52),
            perceptual_roughness: 0.9,
            ..default()
        }),
        wrap_cloth: materials.add(StandardMaterial {
            base_color: Color::srgb(0.80, 0.78, 0.72),
            perceptual_roughness: 0.95,
            ..default()
        }),
    };

    commands.entity(camera_entity).with_children(|parent| {
        // 1. Main Hand Viewmodel Root (or Right Fist if Unarmed)
        let is_left = hand_side.0 == HandSide::Left;
        let main_pos = get_default_weapon_pos(desired_main, is_left);
        let main_rot = if is_left {
            Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06)
        } else {
            Quat::IDENTITY
        };

        parent.spawn((
            SpatialBundle {
                transform: BevyTransform::from_translation(main_pos).with_rotation(main_rot),
                ..default()
            },
            ViewModelWeaponRoot { is_offhand: false },
            RenderLayers::layer(1),
        )).with_children(|builder| {
            spawn_weapon_voxels(builder, desired_main, &mut meshes, &palette);
        });

        // 2. Off-Hand Viewmodel Root
        // Only spawn off-hand if:
        // - player is fully unarmed (both hands unarmed -> brawler stance with both fists)
        // - OR off-hand has an explicitly equipped weapon while main hand is 1-handed or None
        let should_spawn_offhand = (desired_main == WeaponType::None && desired_off == WeaponType::None)
            || (desired_off != WeaponType::None && (desired_main.is_one_handed() || desired_main == WeaponType::None));

        if should_spawn_offhand {
            let off_is_left = !is_left;
            let off_pos = get_default_weapon_pos(desired_off, off_is_left);
            let off_rot = if off_is_left {
                Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06)
            } else {
                Quat::from_rotation_y(-0.08) * Quat::from_rotation_z(-0.06)
            };

            parent.spawn((
                SpatialBundle {
                    transform: BevyTransform::from_translation(off_pos).with_rotation(off_rot),
                    ..default()
                },
                ViewModelWeaponRoot { is_offhand: true },
                RenderLayers::layer(1),
            )).with_children(|builder| {
                spawn_weapon_voxels(builder, desired_off, &mut meshes, &palette);
            });
        }
    });
}

fn spawn_weapon_voxels(
    builder: &mut ChildBuilder,
    desired_weapon: WeaponType,
    meshes: &mut Assets<Mesh>,
    palette: &VoxelPalette,
) {
    let mut meshes = MeshDeref(meshes);
    let wood_dark = palette.wood_dark.clone();
    let wood_light = palette.wood_light.clone();
    let iron_dark = palette.iron_dark.clone();
    let iron_bright = palette.iron_bright.clone();
    let brass_gold = palette.brass_gold.clone();
    let string_white = palette.string_white.clone();
    let red_fletch = palette.red_fletch.clone();
    let rune_cyan = palette.rune_cyan.clone();
    let fire_orange = palette.fire_orange.clone();
    let skin_tone = palette.skin_tone.clone();
    let wrap_cloth = palette.wrap_cloth.clone();

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
                WeaponType::TigerClaws => {
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.10, 0.05, 0.12), Vec3::new(0.0, -0.01, -0.04));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.08, 0.04, 0.08), Vec3::new(0.0, -0.03, 0.0));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.018, 0.03, 0.22), Vec3::new(0.0, 0.02, -0.16));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.018, 0.03, 0.19), Vec3::new(-0.035, 0.015, -0.14));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.018, 0.03, 0.19), Vec3::new(0.035, 0.015, -0.14));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.09, 0.02, 0.03), Vec3::new(0.0, 0.02, -0.06));
                }
                WeaponType::BlackJack => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.035, 0.035, 0.20), Vec3::new(0.0, 0.0, 0.06));
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.015, 0.04, 0.08), Vec3::new(0.0, -0.03, 0.18));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.065, 0.065, 0.16), Vec3::new(0.0, 0.0, -0.12));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.07, 0.07, 0.02), Vec3::new(0.0, 0.0, -0.12));
                }
                WeaponType::TwoHandAxe => {
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.04, 0.04, 0.95), Vec3::new(0.0, 0.0, -0.12));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.045, 0.045, 0.28), Vec3::new(0.0, 0.0, 0.15));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.06, 0.06, 0.14), Vec3::new(0.0, 0.0, -0.52));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.24, 0.02, 0.28), Vec3::new(0.14, 0.0, -0.52));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.20, 0.02, 0.24), Vec3::new(-0.12, 0.0, -0.52));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.025, 0.025, 0.14), Vec3::new(0.0, 0.0, -0.66));
                }
                WeaponType::Trident => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.036, 0.036, 1.15), Vec3::new(0.0, 0.0, -0.15));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.055, 0.055, 0.08), Vec3::new(0.0, 0.0, -0.66));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.26, 0.03, 0.03), Vec3::new(0.0, 0.0, -0.72));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.03, 0.015, 0.32), Vec3::new(0.0, 0.0, -0.88));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.025, 0.015, 0.28), Vec3::new(-0.11, 0.0, -0.86));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.025, 0.015, 0.28), Vec3::new(0.11, 0.0, -0.86));
                }
                WeaponType::Javelin => {
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.028, 0.028, 1.05), Vec3::new(0.0, 0.0, -0.10));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.035, 0.035, 0.18), Vec3::new(0.0, 0.0, 0.0));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.055, 0.015, 0.24), Vec3::new(0.0, 0.0, -0.70));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.032, 0.032, 0.08), Vec3::new(0.0, 0.0, 0.45));
                }
                WeaponType::Wand => {
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.022, 0.022, 0.38), Vec3::new(0.0, 0.0, -0.06));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.032, 0.032, 0.12), Vec3::new(0.0, 0.0, 0.06));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.042, 0.042, 0.03), Vec3::new(0.0, 0.0, -0.24));
                    spawn_voxel_box(builder, &mut meshes, rune_cyan.clone(), Vec3::new(0.036, 0.036, 0.08), Vec3::new(0.0, 0.0, -0.28));
                }
                WeaponType::Orb => {
                    spawn_voxel_box(builder, &mut meshes, rune_cyan.clone(), Vec3::new(0.09, 0.09, 0.09), Vec3::new(0.0, 0.0, -0.18));
                    spawn_voxel_box(builder, &mut meshes, brass_gold.clone(), Vec3::new(0.16, 0.015, 0.16), Vec3::new(0.0, 0.0, -0.18));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.015, 0.14, 0.14), Vec3::new(0.0, 0.0, -0.18));
                    spawn_voxel_box(builder, &mut meshes, fire_orange.clone(), Vec3::new(0.025, 0.025, 0.025), Vec3::new(0.08, 0.06, -0.18));
                }
                WeaponType::WoodenShield => {
                    spawn_voxel_box(builder, &mut meshes, wood_light.clone(), Vec3::new(0.44, 0.44, 0.04), Vec3::new(0.0, 0.0, 0.0));
                    spawn_voxel_box(builder, &mut meshes, iron_dark.clone(), Vec3::new(0.46, 0.46, 0.02), Vec3::new(0.0, 0.0, -0.01));
                    spawn_voxel_box(builder, &mut meshes, iron_bright.clone(), Vec3::new(0.14, 0.14, 0.08), Vec3::new(0.0, 0.0, -0.04));
                    spawn_voxel_box(builder, &mut meshes, wood_dark.clone(), Vec3::new(0.18, 0.05, 0.03), Vec3::new(0.0, 0.0, 0.03));
                }
                WeaponType::None => {
                    // Authentic Clenched Brawler Fist (Unarmed Default)
                    spawn_voxel_box(builder, &mut meshes, skin_tone.clone(), Vec3::new(0.065, 0.065, 0.14), Vec3::new(0.0, -0.01, 0.07));
                    spawn_voxel_box(builder, &mut meshes, wrap_cloth.clone(), Vec3::new(0.072, 0.072, 0.06), Vec3::new(0.0, -0.01, 0.02));
                    spawn_voxel_box(builder, &mut meshes, skin_tone.clone(), Vec3::new(0.075, 0.065, 0.07), Vec3::new(0.0, 0.0, -0.04));
                    spawn_voxel_box(builder, &mut meshes, skin_tone.clone(), Vec3::new(0.072, 0.035, 0.038), Vec3::new(0.0, -0.018, -0.07));
                    spawn_voxel_box(builder, &mut meshes, skin_tone.clone(), Vec3::new(0.028, 0.045, 0.035), Vec3::new(0.035, 0.01, -0.045));
                }
    }
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
    mut swing_state: ResMut<SwingState>,
    hand_side: Res<EquippedHandSide>,
    mut root_q: Query<(&ViewModelWeaponRoot, &mut BevyTransform)>,
    mut pump_q: Query<&mut BevyTransform, (With<ViewModelPumpSlide>, Without<ViewModelWeaponRoot>)>,
    mut arrow_q: Query<&mut Visibility, (With<ViewModelBowArrow>, Without<ViewModelCrossbowBolt>)>,
    mut bolt_q: Query<&mut Visibility, (With<ViewModelCrossbowBolt>, Without<ViewModelBowArrow>)>,
) {
    let dt = time.delta_seconds();
    weapon_state.sway_time += dt;

    // Melee swing timers ticking
    if swing_state.is_swinging {
        swing_state.timer.tick(time.delta());
        if swing_state.timer.just_finished() {
            swing_state.is_swinging = false;
        }
    }
    if swing_state.offhand_is_swinging {
        swing_state.offhand_timer.tick(time.delta());
        if swing_state.offhand_timer.just_finished() {
            swing_state.offhand_is_swinging = false;
        }
    }

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
    weapon_state.offhand_recoil_offset = weapon_state.offhand_recoil_offset.lerp(Vec3::ZERO, (dt * 14.0).min(1.0));
    weapon_state.offhand_recoil_rot = weapon_state.offhand_recoil_rot.slerp(Quat::IDENTITY, (dt * 16.0).min(1.0));
    weapon_state.dynamic_bloom = (weapon_state.dynamic_bloom - dt * 28.0).max(0.0);
    if weapon_state.current_weapon == WeaponType::Bow && weapon_state.bow_drawing {
        weapon_state.dynamic_bloom = (1.0 - weapon_state.bow_charge) * 12.0;
    }

    // 4. Transform Animation on ViewModelWeaponRoot
    for (root, mut root_t) in root_q.iter_mut() {
        if !root.is_offhand {
            let is_left = hand_side.0 == HandSide::Left;
            let default_pos = get_default_weapon_pos(weapon_state.current_weapon, is_left);

            // Idle Breathing Sway
            let sway_x = (weapon_state.sway_time * 1.5).sin() * 0.003;
            let sway_y = (weapon_state.sway_time * 3.0).cos() * 0.002;
            let mut current_offset = default_pos + Vec3::new(sway_x, sway_y, 0.0) + weapon_state.recoil_offset;
            let mut current_rot = weapon_state.recoil_rot;
            if is_left {
                current_rot = Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06) * current_rot;
            }

            // Main hand punch / swing lunge animation
            if swing_state.is_swinging {
                let t = swing_state.timer.fraction();
                let punch_forward = (t * std::f32::consts::PI).sin() * 0.16;
                current_offset.z -= punch_forward;
                current_offset.y += punch_forward * 0.15;
                current_rot *= Quat::from_rotation_x(punch_forward * 0.7);
            }

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
        } else {
            let off_is_left = hand_side.0 != HandSide::Left;
            let default_pos = get_default_weapon_pos(weapon_state.offhand_weapon, off_is_left);

            // Counter Sway
            let sway_x = ((weapon_state.sway_time + 1.2) * 1.5).sin() * 0.003;
            let sway_y = ((weapon_state.sway_time + 1.2) * 3.0).cos() * 0.002;
            let mut current_offset = default_pos + Vec3::new(sway_x, sway_y, 0.0) + weapon_state.offhand_recoil_offset;
            let mut current_rot = weapon_state.offhand_recoil_rot;
            if off_is_left {
                current_rot = Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06) * current_rot;
            } else {
                current_rot = Quat::from_rotation_y(-0.08) * Quat::from_rotation_z(-0.06) * current_rot;
            }

            // Off-hand punch / swing lunge animation
            if swing_state.offhand_is_swinging {
                let t = swing_state.offhand_timer.fraction();
                let punch_forward = (t * std::f32::consts::PI).sin() * 0.16;
                current_offset.z -= punch_forward;
                current_offset.y += punch_forward * 0.15;
                current_rot *= Quat::from_rotation_x(punch_forward * 0.7);
            }

            root_t.translation = current_offset;
            root_t.rotation = current_rot;
        }
    }
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
    if *camera_mode.get() != CameraMode::FPS {
        *vis = Visibility::Hidden;
        return;
    }

    *vis = Visibility::Inherited;
    let hand_str = match hand_side.0 {
        HandSide::Right => "PRIMARY: RIGHT [H to Swap]",
        HandSide::Left => "PRIMARY: LEFT [H to Swap]",
    };

    if weapon_state.current_weapon == WeaponType::None && weapon_state.offhand_weapon == WeaponType::None {
        text.sections[0].value = format!("{}\nUNARMED [BARE FISTS]\n[LMB] Right Jab | [RMB] Left Jab", hand_str);
        return;
    }

    let main_desc = format!("MAIN: {} [LMB]", weapon_state.current_weapon.display_name());
    let off_desc = if weapon_state.current_weapon.is_one_handed() || weapon_state.current_weapon == WeaponType::None {
        if weapon_state.offhand_weapon == WeaponType::None {
            "OFF: Bare Fist [RMB]".to_string()
        } else {
            format!("OFF: {} [RMB]", weapon_state.offhand_weapon.display_name())
        }
    } else {
        "TWO-HANDED [LMB Attack | Hold]".to_string()
    };

    text.sections[0].value = format!("{}\n{}\n{}", hand_str, main_desc, off_desc);
}

// ----------------------------------------------------------------------------
// AUTHORITATIVE PROJECTILE RENDERING LOOP
// ----------------------------------------------------------------------------

use std::collections::{BTreeMap, BTreeSet};
use spacetimedb_sdk::Table;
use crate::network::SpacetimeConnection;
use crate::module_bindings::active_projectile_table::ActiveProjectileTableAccess;
use crate::module_bindings::projectile_kind_type::ProjectileKind;

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
                // Shaft
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.015, 0.015, 0.52)),
                    material: wood_mat.clone(),
                    ..default()
                });
                // Flint arrowhead
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.03, 0.01, 0.05)),
                    material: iron_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, -0.27),
                    ..default()
                });
                // Red fletching fins
                builder.spawn(PbrBundle {
                    mesh: meshes.add(bevy::math::primitives::Cuboid::new(0.008, 0.04, 0.07)),
                    material: red_mat.clone(),
                    transform: BevyTransform::from_xyz(0.0, 0.0, 0.22),
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
