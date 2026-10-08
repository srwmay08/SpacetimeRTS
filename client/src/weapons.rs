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
use crate::trees::LowPolyMeshBuilder;

use avian3d::prelude::LinearVelocity;
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

// ----------------------------------------------------------------------------
// WEAPON ARCHETYPE & TYPE DEFINITIONS
// ----------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponArchetype {
    Unarmed,
    OneHandBlade,
    TwoHandBlade,
    OneHandAxe,
    TwoHandAxe,
    OneHandBludgeon,
    TwoHandBludgeon,
    Polearm,
    Bow,
    Crossbow,
    MagicImplement,
    Shield,
    UtilityTool,
    /// Archived / horizontal prototype weapon variant retained for compatibility
    ArchivedVariant,
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
    /// Classifies the weapon into its high-level gameplay archetype.
    #[allow(dead_code)]
    pub fn archetype(&self) -> WeaponArchetype {
        match self {
            Self::None => WeaponArchetype::Unarmed,
            Self::Longsword | Self::Dagger | Self::Rapier => WeaponArchetype::OneHandBlade,
            Self::Greatsword => WeaponArchetype::TwoHandBlade,
            Self::Handaxe => WeaponArchetype::OneHandAxe,
            Self::TwoHandAxe => WeaponArchetype::TwoHandAxe,
            Self::Club | Self::Warhammer => WeaponArchetype::OneHandBludgeon,
            Self::Maul => WeaponArchetype::TwoHandBludgeon,
            Self::Spear | Self::Halberd | Self::Trident | Self::Javelin => WeaponArchetype::Polearm,
            Self::Bow => WeaponArchetype::Bow,
            Self::Crossbow | Self::HandCrossbow => WeaponArchetype::Crossbow,
            Self::Wand | Self::Runestaff | Self::Orb => WeaponArchetype::MagicImplement,
            Self::WoodenShield => WeaponArchetype::Shield,
            Self::Hammer | Self::Pickaxe | Self::Torch => WeaponArchetype::UtilityTool,
            _ => WeaponArchetype::ArchivedVariant,
        }
    }

    /// Indicates whether this weapon is part of the canonical vertical slice.
    #[allow(dead_code)]
    pub fn is_canonical_vertical_slice(&self) -> bool {
        !matches!(self.archetype(), WeaponArchetype::ArchivedVariant)
    }

    /// Parses item name through strongly-typed ItemKind with complete alias coverage.
    pub fn from_item_name(item: Option<&str>) -> Self {
        let Some(name) = item else { return Self::None; };
        crate::ui::types::ItemKind::from_name(name)
            .map(|k| {
                use crate::ui::types::ItemKindWeaponExt;
                k.to_weapon_type()
            })
            .unwrap_or(Self::None)
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
    pub bow_nock_timer: Timer,

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
        let mut nock_timer = Timer::from_seconds(0.32, TimerMode::Once);
        nock_timer.set_elapsed(std::time::Duration::from_millis(350));

        Self {
            current_weapon: WeaponType::None,
            offhand_weapon: WeaponType::None,
            last_hand: HandSide::Right,
            offhand_last_hand: HandSide::Left,
            bow_charge: 0.0,
            bow_drawing: false,
            bow_nock_timer: nock_timer,
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThirdPersonWeaponRoot {
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
    mut hand_evts: EventReader<crate::input::ToggleWeaponHandEvent>,
    mut hand_side: ResMut<EquippedHandSide>,
    mut weapon_root_q: Query<&mut BevyTransform, With<ViewModelWeaponRoot>>,
) {
    for _ in hand_evts.read() {
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
        WeaponType::Longsword => Vec3::new(0.24, -0.12, -0.34),
        WeaponType::Greatsword => Vec3::new(0.24, -0.14, -0.38),
        WeaponType::TwoHandAxe => Vec3::new(0.24, -0.14, -0.38),
        WeaponType::Maul => Vec3::new(0.24, -0.14, -0.36),
        WeaponType::Spear => Vec3::new(0.24, -0.14, -0.42),
        WeaponType::Javelin => Vec3::new(0.22, -0.14, -0.38),
        WeaponType::Trident => Vec3::new(0.24, -0.14, -0.44),
        WeaponType::Rapier => Vec3::new(0.22, -0.12, -0.34),
        WeaponType::Warhammer => Vec3::new(0.22, -0.14, -0.34),
        WeaponType::Club => Vec3::new(0.22, -0.14, -0.34),
        WeaponType::BlackJack => Vec3::new(0.20, -0.14, -0.30),
        WeaponType::Dagger => Vec3::new(0.20, -0.12, -0.30),
        WeaponType::Handaxe => Vec3::new(0.22, -0.14, -0.32),
        WeaponType::TigerClaws => Vec3::new(0.18, -0.14, -0.26),
        WeaponType::Cestus => Vec3::new(0.18, -0.14, -0.26),
        WeaponType::KnuckleDuster => Vec3::new(0.18, -0.14, -0.26),
        WeaponType::FryingPan => Vec3::new(0.24, -0.16, -0.34),
        WeaponType::HolyMackerel => Vec3::new(0.22, -0.14, -0.32),
        WeaponType::Hammer => Vec3::new(0.20, -0.14, -0.30),
        WeaponType::Pickaxe => Vec3::new(0.22, -0.14, -0.32),
        WeaponType::Torch => Vec3::new(0.22, -0.14, -0.32),
        WeaponType::WoodenShield => Vec3::new(0.24, -0.10, -0.32),
        WeaponType::None => Vec3::new(0.20, -0.14, -0.28), // Boxer guard default fist
    };

    if is_left {
        pos.x = -pos.x;
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
    player_query: Query<Entity, With<PlayerBody>>,
    existing_weapon_q: Query<Entity, With<ViewModelWeaponRoot>>,
    existing_tp_q: Query<Entity, With<ThirdPersonWeaponRoot>>,
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

    // Despawn old weapon models (both 1st-person viewmodels and 3rd-person body attachments)
    for entity in existing_weapon_q.iter() {
        commands.entity(entity).despawn_recursive();
    }
    for entity in existing_tp_q.iter() {
        commands.entity(entity).despawn_recursive();
    }

    weapon_state.current_weapon = desired_main;
    weapon_state.offhand_weapon = desired_off;
    weapon_state.last_hand = hand_side.0;

    let Ok(camera_entity) = camera_query.get_single() else {
        return;
    };

    let is_left = hand_side.0 == HandSide::Left;
    let should_spawn_offhand = (desired_main == WeaponType::None && desired_off == WeaponType::None)
        || (desired_off != WeaponType::None && (desired_main.is_one_handed() || desired_main == WeaponType::None));

    commands.entity(camera_entity).with_children(|parent| {
        // 1. Main Hand Viewmodel Root (or Right Fist if Unarmed)
        let main_pos = get_default_weapon_pos(desired_main, is_left);
        let main_rot = match desired_main {
            WeaponType::Longsword | WeaponType::Greatsword | WeaponType::Rapier => {
                Quat::from_rotation_x(0.48) * Quat::from_rotation_y(-0.25) * Quat::from_rotation_z(0.18)
            }
            _ => if is_left {
                Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06)
            } else {
                Quat::IDENTITY
            },
        };

        parent.spawn((
            SpatialBundle {
                transform: BevyTransform::from_translation(main_pos).with_rotation(main_rot),
                ..default()
            },
            ViewModelWeaponRoot { is_offhand: false },
            RenderLayers::layer(1),
        )).with_children(|builder| {
            spawn_weapon_voxels(builder, desired_main, &mut meshes, &mut materials, 1);
        });

        // 2. Off-Hand Viewmodel Root
        // Only spawn off-hand if:
        // - player is fully unarmed (both hands unarmed -> brawler stance with both fists)
        // - OR off-hand has an explicitly equipped weapon while main hand is 1-handed or None
        if should_spawn_offhand {
            let off_is_left = !is_left;
            let off_pos = get_default_weapon_pos(desired_off, off_is_left);
            let off_rot = match desired_off {
                WeaponType::WoodenShield => {
                    if off_is_left {
                        Quat::from_rotation_y(0.26) * Quat::from_rotation_x(0.16) * Quat::from_rotation_z(-0.08)
                    } else {
                        Quat::from_rotation_y(-0.26) * Quat::from_rotation_x(0.16) * Quat::from_rotation_z(0.08)
                    }
                }
                _ => if off_is_left {
                    Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06)
                } else {
                    Quat::from_rotation_y(-0.08) * Quat::from_rotation_z(-0.06)
                },
            };

            parent.spawn((
                SpatialBundle {
                    transform: BevyTransform::from_translation(off_pos).with_rotation(off_rot),
                    ..default()
                },
                ViewModelWeaponRoot { is_offhand: true },
                RenderLayers::layer(1),
            )).with_children(|builder| {
                spawn_weapon_voxels(builder, desired_off, &mut meshes, &mut materials, 1);
            });
        }
    });

    // 2. Spawn Third-Person Weapon Models on Player Body (RenderLayers::layer(2))
    if let Ok(player_entity) = player_query.get_single() {
        commands.entity(player_entity).with_children(|body| {
            // Main hand on player character (peasant right hand)
            let tp_main_pos = if is_left {
                Vec3::new(-0.28, -0.55, 0.08)
            } else {
                Vec3::new(0.28, -0.55, 0.08)
            };
            let tp_main_rot = match desired_main {
                WeaponType::Longsword | WeaponType::Greatsword | WeaponType::Rapier => {
                    Quat::from_rotation_x(-0.75)
                        * Quat::from_rotation_y(if is_left { -0.15 } else { 0.15 })
                        * Quat::from_rotation_z(if is_left { 0.25 } else { -0.25 })
                }
                _ => Quat::from_rotation_x(-0.4),
            };

            body.spawn((
                SpatialBundle {
                    transform: BevyTransform::from_translation(tp_main_pos).with_rotation(tp_main_rot),
                    ..default()
                },
                ThirdPersonWeaponRoot { is_offhand: false },
                RenderLayers::layer(2),
            )).with_children(|builder| {
                spawn_weapon_voxels(builder, desired_main, &mut meshes, &mut materials, 2);
            });

            // Off hand on player character (peasant left hand / shield)
            if should_spawn_offhand {
                let off_is_left = !is_left;
                let tp_off_pos = if off_is_left {
                    Vec3::new(-0.30, -0.52, 0.06)
                } else {
                    Vec3::new(0.30, -0.52, 0.06)
                };
                let tp_off_rot = match desired_off {
                    WeaponType::WoodenShield => {
                        if off_is_left {
                            Quat::from_rotation_y(1.57) * Quat::from_rotation_x(0.12)
                        } else {
                            Quat::from_rotation_y(-1.57) * Quat::from_rotation_x(0.12)
                        }
                    }
                    _ => Quat::IDENTITY,
                };

                body.spawn((
                    SpatialBundle {
                        transform: BevyTransform::from_translation(tp_off_pos).with_rotation(tp_off_rot),
                        ..default()
                    },
                    ThirdPersonWeaponRoot { is_offhand: true },
                    RenderLayers::layer(2),
                )).with_children(|builder| {
                    spawn_weapon_voxels(builder, desired_off, &mut meshes, &mut materials, 2);
                });
            }
        });
    }
}

pub const COL_WOOD_DARK: [f32; 4] = [0.35, 0.22, 0.12, 1.0];
pub const COL_WOOD_LIGHT: [f32; 4] = [0.55, 0.38, 0.22, 1.0];
pub const COL_IRON_DARK: [f32; 4] = [0.22, 0.23, 0.25, 1.0];
pub const COL_IRON_BRIGHT: [f32; 4] = [0.48, 0.50, 0.52, 1.0];
pub const COL_BRASS_GOLD: [f32; 4] = [0.78, 0.62, 0.22, 1.0];
pub const COL_STRING_WHITE: [f32; 4] = [0.92, 0.90, 0.82, 1.0];
pub const COL_RED_FLETCH: [f32; 4] = [0.85, 0.15, 0.15, 1.0];
pub const COL_RUNE_CYAN: [f32; 4] = [0.2, 0.85, 1.0, 1.0];
pub const COL_FIRE_ORANGE: [f32; 4] = [1.0, 0.45, 0.05, 1.0];
pub const COL_SKIN_TONE: [f32; 4] = [0.86, 0.68, 0.52, 1.0];
pub const COL_WRAP_CLOTH: [f32; 4] = [0.80, 0.78, 0.72, 1.0];

/// Builds a watertight low-poly nocked arrow mesh for first-person bows.
pub fn create_lowpoly_arrow_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    // Shaft
    builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.015, 0.015, 0.52), COL_WOOD_LIGHT);
    // Arrowhead
    builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.24), Vec3::new(0.0, 0.0, -0.30), 0.03, 0.002, 0.012, 0.002, COL_IRON_BRIGHT, true);
    // Fletching
    builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.22), Vec3::new(0.008, 0.04, 0.07), COL_RED_FLETCH);
    builder.build()
}

/// Builds a watertight low-poly bolt mesh for first-person crossbows.
pub fn create_lowpoly_bolt_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    // Shaft
    builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.018, 0.018, 0.30), COL_WOOD_LIGHT);
    // Piercing tip
    builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.14), Vec3::new(0.0, 0.0, -0.19), 0.025, 0.002, 0.025, 0.002, COL_IRON_BRIGHT, true);
    // Fletching
    builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.12), Vec3::new(0.005, 0.035, 0.05), COL_RED_FLETCH);
    builder.build()
}

/// Builds a watertight low-poly shotgun pump slide mesh.
pub fn create_lowpoly_pumpslide_mesh() -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();
    builder.add_box_center_size(Vec3::ZERO, Vec3::new(0.054, 0.054, 0.14), COL_WOOD_LIGHT);
    builder.build()
}

/// Constructs a watertight, unified low-poly weapon mesh with per-vertex shading.
pub fn create_lowpoly_weapon_mesh(weapon: WeaponType) -> Mesh {
    let mut builder = LowPolyMeshBuilder::new();

    match weapon {
        WeaponType::Bow => {
            // Central Handle Grip
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.04, 0.12, 0.04), COL_WOOD_DARK);
            // Upper Limb
            builder.add_box_center_size(Vec3::new(0.0, 0.12, -0.04), Vec3::new(0.035, 0.14, 0.035), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.24, -0.10), Vec3::new(0.03, 0.14, 0.03), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.33, -0.14), Vec3::new(0.025, 0.08, 0.025), COL_WOOD_LIGHT);
            // Lower Limb
            builder.add_box_center_size(Vec3::new(0.0, -0.12, -0.04), Vec3::new(0.035, 0.14, 0.035), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, -0.24, -0.10), Vec3::new(0.03, 0.14, 0.03), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.33, -0.14), Vec3::new(0.025, 0.08, 0.025), COL_WOOD_LIGHT);
            // Bowstring
            builder.add_box_center_size(Vec3::new(0.0, 0.17, -0.02), Vec3::new(0.005, 0.36, 0.005), COL_STRING_WHITE);
            builder.add_box_center_size(Vec3::new(0.0, -0.17, -0.02), Vec3::new(0.005, 0.36, 0.005), COL_STRING_WHITE);
        }
        WeaponType::Crossbow => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.065, 0.075, 0.52), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.04, 0.18), Vec3::new(0.055, 0.11, 0.16), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.09, 0.04), Vec3::new(0.045, 0.12, 0.06), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.03, -0.32), Vec3::new(0.52, 0.035, 0.035), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.01, -0.42), Vec3::new(0.12, 0.025, 0.08), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.042, -0.14), Vec3::new(0.03, 0.015, 0.38), COL_IRON_BRIGHT);
        }
        WeaponType::HandCrossbow => {
            builder.add_box_center_size(Vec3::new(0.0, -0.08, 0.05), Vec3::new(0.04, 0.11, 0.05), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.10), Vec3::new(0.045, 0.055, 0.32), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.025, -0.22), Vec3::new(0.32, 0.025, 0.025), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.04, -0.02), Vec3::new(0.02, 0.04, 0.03), COL_BRASS_GOLD);
        }
        WeaponType::Revolver => {
            builder.add_box_center_size(Vec3::new(0.0, -0.08, 0.06), Vec3::new(0.042, 0.12, 0.06), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.02), Vec3::new(0.044, 0.075, 0.13), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, -0.055, -0.01), Vec3::new(0.02, 0.04, 0.05), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.005, -0.06), Vec3::new(0.062, 0.062, 0.09), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.005, -0.012), Vec3::new(0.04, 0.04, 0.01), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.018, -0.24), Vec3::new(0.04, 0.045, 0.28), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.044, -0.24), Vec3::new(0.015, 0.015, 0.28), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.052, -0.36), Vec3::new(0.012, 0.02, 0.02), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.045, 0.04), Vec3::new(0.018, 0.035, 0.03), COL_IRON_DARK);
        }
        WeaponType::Shotgun => {
            builder.add_box_center_size(Vec3::new(0.0, -0.04, 0.16), Vec3::new(0.055, 0.09, 0.24), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.01, -0.04), Vec3::new(0.06, 0.08, 0.18), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.031, 0.02, -0.04), Vec3::new(0.01, 0.03, 0.05), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.032, -0.34), Vec3::new(0.042, 0.042, 0.44), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, -0.008, -0.31), Vec3::new(0.038, 0.038, 0.38), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.058, -0.54), Vec3::new(0.014, 0.016, 0.014), COL_BRASS_GOLD);
        }
        WeaponType::SniperRifle => {
            builder.add_box_center_size(Vec3::new(0.0, -0.05, 0.18), Vec3::new(0.05, 0.10, 0.28), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.01, -0.05), Vec3::new(0.055, 0.08, 0.20), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.025, -0.45), Vec3::new(0.038, 0.038, 0.65), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.085, -0.10), Vec3::new(0.032, 0.032, 0.28), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.085, -0.22), Vec3::new(0.038, 0.038, 0.02), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.085, 0.02), Vec3::new(0.038, 0.038, 0.02), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, -0.02, -0.55), Vec3::new(0.06, 0.02, 0.12), COL_IRON_DARK);
        }
        WeaponType::BouncyBombLauncher => {
            builder.add_box_center_size(Vec3::new(0.0, -0.05, 0.16), Vec3::new(0.06, 0.11, 0.24), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.06, 0.02), Vec3::new(0.055, 0.12, 0.12), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.01, -0.12), Vec3::new(0.12, 0.12, 0.16), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.01, -0.12), Vec3::new(0.125, 0.125, 0.02), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.34), Vec3::new(0.075, 0.075, 0.32), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, -0.04, -0.32), Vec3::new(0.05, 0.07, 0.12), COL_WOOD_LIGHT);
        }
        WeaponType::Runestaff => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.15), Vec3::new(0.045, 0.045, 0.90), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.45), Vec3::new(0.055, 0.055, 0.04), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.20), Vec3::new(0.055, 0.055, 0.04), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.62), Vec3::new(0.08, 0.08, 0.08), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.72), Vec3::new(0.065, 0.065, 0.12), COL_RUNE_CYAN);
        }
        WeaponType::Halberd => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.20), Vec3::new(0.04, 0.04, 1.10), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.65), Vec3::new(0.055, 0.055, 0.24), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.11, 0.0, -0.72), Vec3::new(0.18, 0.02, 0.22), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.08, 0.0, -0.72), Vec3::new(0.12, 0.025, 0.06), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.75), Vec3::new(0.0, 0.0, -1.02), 0.035, 0.005, 0.020, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Longsword => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.035, 0.035, 0.16), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.18), Vec3::new(0.05, 0.05, 0.04), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.02), Vec3::new(0.24, 0.025, 0.035), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.04), Vec3::new(0.0, 0.0, -0.72), 0.06, 0.015, 0.018, 0.004, COL_IRON_BRIGHT, true);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.32), Vec3::new(0.012, 0.019, 0.50), COL_IRON_DARK);
        }
        WeaponType::Greatsword => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.15), Vec3::new(0.038, 0.038, 0.26), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.30), Vec3::new(0.06, 0.06, 0.05), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.36, 0.03, 0.04), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.09), Vec3::new(0.07, 0.02, 0.14), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.17), Vec3::new(0.16, 0.02, 0.03), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.18), Vec3::new(0.0, 0.0, -1.02), 0.08, 0.02, 0.022, 0.005, COL_IRON_BRIGHT, true);
        }
        WeaponType::Maul => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.15), Vec3::new(0.045, 0.045, 0.75), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.55), Vec3::new(0.14, 0.14, 0.22), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.67), Vec3::new(0.13, 0.13, 0.02), COL_IRON_BRIGHT);
        }
        WeaponType::Spear => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.15), Vec3::new(0.032, 0.032, 1.05), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.68), Vec3::new(0.045, 0.045, 0.08), COL_BRASS_GOLD);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.72), Vec3::new(0.0, 0.0, -0.98), 0.065, 0.008, 0.018, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Rapier => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.028, 0.028, 0.14), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.01), Vec3::new(0.14, 0.14, 0.10), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.04), Vec3::new(0.0, 0.0, -0.82), 0.025, 0.004, 0.025, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Warhammer => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.04, 0.04, 0.65), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.05, 0.0, -0.46), Vec3::new(0.09, 0.09, 0.12), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.06, 0.0, -0.46), Vec3::new(0.10, 0.03, 0.04), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.56), Vec3::new(0.025, 0.025, 0.12), COL_IRON_BRIGHT);
        }
        WeaponType::Club => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.05, 0.05, 0.35), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.28), Vec3::new(0.095, 0.095, 0.28), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.28), Vec3::new(0.11, 0.11, 0.04), COL_IRON_BRIGHT);
        }
        WeaponType::Dagger => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.06), Vec3::new(0.032, 0.032, 0.12), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.01), Vec3::new(0.10, 0.025, 0.025), COL_BRASS_GOLD);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.02), Vec3::new(0.0, 0.0, -0.32), 0.042, 0.008, 0.014, 0.003, COL_IRON_BRIGHT, true);
        }
        WeaponType::Handaxe => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.10), Vec3::new(0.036, 0.036, 0.48), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.32), Vec3::new(0.05, 0.05, 0.08), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.08, 0.0, -0.36), Vec3::new(0.14, 0.018, 0.16), COL_IRON_BRIGHT);
        }
        WeaponType::Cestus => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.02), Vec3::new(0.09, 0.09, 0.22), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.04, -0.14), Vec3::new(0.10, 0.04, 0.06), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.065, -0.14), Vec3::new(0.09, 0.02, 0.03), COL_IRON_BRIGHT);
        }
        WeaponType::KnuckleDuster => {
            builder.add_box_center_size(Vec3::new(0.0, -0.02, -0.02), Vec3::new(0.08, 0.03, 0.025), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.06), Vec3::new(0.11, 0.05, 0.04), COL_IRON_BRIGHT);
        }
        WeaponType::FryingPan => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.22), Vec3::new(0.24, 0.02, 0.24), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.34), Vec3::new(0.26, 0.05, 0.02), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.10), Vec3::new(0.26, 0.05, 0.02), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.12, 0.02, -0.22), Vec3::new(0.02, 0.05, 0.24), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.12, 0.02, -0.22), Vec3::new(0.02, 0.05, 0.24), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, 0.0), Vec3::new(0.035, 0.025, 0.18), COL_IRON_DARK);
        }
        WeaponType::HolyMackerel => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.16), Vec3::new(0.06, 0.12, 0.38), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, -0.05, -0.16), Vec3::new(0.055, 0.03, 0.36), COL_STRING_WHITE);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.02, 0.14, 0.10), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.05, 0.08, 0.12), COL_STRING_WHITE);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.30), Vec3::new(0.065, 0.025, 0.025), COL_BRASS_GOLD);
        }
        WeaponType::Hammer => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.06), Vec3::new(0.035, 0.035, 0.40), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.24), Vec3::new(0.07, 0.07, 0.13), COL_IRON_BRIGHT);
        }
        WeaponType::Pickaxe => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.10), Vec3::new(0.038, 0.038, 0.52), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.34), Vec3::new(0.28, 0.04, 0.06), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.34), Vec3::new(0.32, 0.025, 0.03), COL_IRON_BRIGHT);
        }
        WeaponType::Torch => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.08), Vec3::new(0.038, 0.038, 0.46), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.28), Vec3::new(0.065, 0.065, 0.12), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.04, -0.38), Vec3::new(0.09, 0.12, 0.09), COL_FIRE_ORANGE);
        }
        WeaponType::TigerClaws => {
            builder.add_box_center_size(Vec3::new(0.0, -0.01, -0.04), Vec3::new(0.10, 0.05, 0.12), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.03, 0.0), Vec3::new(0.08, 0.04, 0.08), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.16), Vec3::new(0.018, 0.03, 0.22), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.035, 0.015, -0.14), Vec3::new(0.018, 0.03, 0.19), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.035, 0.015, -0.14), Vec3::new(0.018, 0.03, 0.19), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.02, -0.06), Vec3::new(0.09, 0.02, 0.03), COL_BRASS_GOLD);
        }
        WeaponType::BlackJack => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.06), Vec3::new(0.035, 0.035, 0.20), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, -0.03, 0.18), Vec3::new(0.015, 0.04, 0.08), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.065, 0.065, 0.16), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.07, 0.07, 0.02), COL_BRASS_GOLD);
        }
        WeaponType::TwoHandAxe => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.12), Vec3::new(0.04, 0.04, 0.95), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.15), Vec3::new(0.045, 0.045, 0.28), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.52), Vec3::new(0.06, 0.06, 0.14), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.14, 0.0, -0.52), Vec3::new(0.24, 0.02, 0.28), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(-0.12, 0.0, -0.52), Vec3::new(0.20, 0.02, 0.24), COL_IRON_BRIGHT);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.58), Vec3::new(0.0, 0.0, -0.74), 0.025, 0.005, 0.025, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Trident => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.15), Vec3::new(0.036, 0.036, 1.15), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.66), Vec3::new(0.055, 0.055, 0.08), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.72), Vec3::new(0.26, 0.03, 0.03), COL_IRON_DARK);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.72), Vec3::new(0.0, 0.0, -1.04), 0.030, 0.005, 0.018, 0.004, COL_IRON_BRIGHT, true);
            builder.add_diamond_blade(Vec3::new(-0.11, 0.0, -0.72), Vec3::new(-0.11, 0.0, -1.00), 0.025, 0.005, 0.015, 0.004, COL_IRON_BRIGHT, true);
            builder.add_diamond_blade(Vec3::new(0.11, 0.0, -0.72), Vec3::new(0.11, 0.0, -1.00), 0.025, 0.005, 0.015, 0.004, COL_IRON_BRIGHT, true);
        }
        WeaponType::Javelin => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.10), Vec3::new(0.028, 0.028, 1.05), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.035, 0.035, 0.18), COL_WOOD_DARK);
            builder.add_diamond_blade(Vec3::new(0.0, 0.0, -0.60), Vec3::new(0.0, 0.0, -0.84), 0.055, 0.008, 0.018, 0.004, COL_IRON_BRIGHT, true);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.45), Vec3::new(0.032, 0.032, 0.08), COL_BRASS_GOLD);
        }
        WeaponType::Wand => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.06), Vec3::new(0.022, 0.022, 0.38), COL_WOOD_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.06), Vec3::new(0.032, 0.032, 0.12), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.24), Vec3::new(0.042, 0.042, 0.03), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.28), Vec3::new(0.036, 0.036, 0.08), COL_RUNE_CYAN);
        }
        WeaponType::Orb => {
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.18), Vec3::new(0.09, 0.09, 0.09), COL_RUNE_CYAN);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.18), Vec3::new(0.16, 0.015, 0.16), COL_BRASS_GOLD);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.18), Vec3::new(0.015, 0.14, 0.14), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.08, 0.06, -0.18), Vec3::new(0.025, 0.025, 0.025), COL_FIRE_ORANGE);
        }
        WeaponType::WoodenShield => {
            // Compact combat buckler / targe: ~70% reduced screen obstruction
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.22, 0.24, 0.025), COL_WOOD_LIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.01), Vec3::new(0.24, 0.26, 0.015), COL_IRON_DARK);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.025), Vec3::new(0.07, 0.07, 0.04), COL_IRON_BRIGHT);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, 0.02), Vec3::new(0.09, 0.03, 0.02), COL_WOOD_DARK);
        }
        WeaponType::None => {
            // Authentic Clenched Brawler Fist (Unarmed Default for 1st person)
            builder.add_box_center_size(Vec3::new(0.0, -0.01, 0.07), Vec3::new(0.065, 0.065, 0.14), COL_SKIN_TONE);
            builder.add_box_center_size(Vec3::new(0.0, -0.01, 0.02), Vec3::new(0.072, 0.072, 0.06), COL_WRAP_CLOTH);
            builder.add_box_center_size(Vec3::new(0.0, 0.0, -0.04), Vec3::new(0.075, 0.065, 0.07), COL_SKIN_TONE);
            builder.add_box_center_size(Vec3::new(0.0, -0.018, -0.07), Vec3::new(0.072, 0.035, 0.038), COL_SKIN_TONE);
            builder.add_box_center_size(Vec3::new(0.035, 0.01, -0.045), Vec3::new(0.028, 0.045, 0.035), COL_SKIN_TONE);
        }
    }

    builder.build()
}

fn spawn_weapon_voxels(
    builder: &mut ChildBuilder,
    desired_weapon: WeaponType,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    render_layer: usize,
) {
    if desired_weapon == WeaponType::None && render_layer == 2 {
        // Third-person body model already has hands; no weapon attachment needed when unarmed.
        return;
    }

    let weapon_mesh = create_lowpoly_weapon_mesh(desired_weapon);
    let weapon_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.65,
        metallic: 0.35,
        cull_mode: None,
        ..default()
    });

    builder.spawn((
        PbrBundle {
            mesh: meshes.add(weapon_mesh),
            material: weapon_mat.clone(),
            ..default()
        },
        RenderLayers::layer(render_layer),
    ));

    // Dynamic animated accessories (only spawned on Viewmodel layer 1)
    if render_layer == 1 {
        match desired_weapon {
            WeaponType::Bow => {
                let arrow_mesh = create_lowpoly_arrow_mesh();
                builder.spawn((
                    PbrBundle {
                        mesh: meshes.add(arrow_mesh),
                        material: weapon_mat,
                        transform: BevyTransform::from_xyz(-0.01, 0.02, -0.12),
                        ..default()
                    },
                    ViewModelBowArrow,
                    RenderLayers::layer(1),
                ));
            }
            WeaponType::Crossbow => {
                let bolt_mesh = create_lowpoly_bolt_mesh();
                builder.spawn((
                    PbrBundle {
                        mesh: meshes.add(bolt_mesh),
                        material: weapon_mat,
                        transform: BevyTransform::from_xyz(0.0, 0.055, -0.16),
                        ..default()
                    },
                    ViewModelCrossbowBolt,
                    RenderLayers::layer(1),
                ));
            }
            WeaponType::HandCrossbow => {
                let bolt_mesh = create_lowpoly_bolt_mesh();
                builder.spawn((
                    PbrBundle {
                        mesh: meshes.add(bolt_mesh),
                        material: weapon_mat,
                        transform: BevyTransform::from_xyz(0.0, 0.035, -0.12),
                        ..default()
                    },
                    ViewModelCrossbowBolt,
                    RenderLayers::layer(1),
                ));
            }
            WeaponType::Shotgun => {
                let slide_mesh = create_lowpoly_pumpslide_mesh();
                builder.spawn((
                    PbrBundle {
                        mesh: meshes.add(slide_mesh),
                        material: weapon_mat,
                        transform: BevyTransform::from_xyz(0.0, -0.008, -0.26),
                        ..default()
                    },
                    ViewModelPumpSlide,
                    RenderLayers::layer(1),
                ));
            }
            _ => {}
        }
    }
}

/// Spawns an immediate procedural blade slash trail / crescent arc in front of the camera.
pub fn spawn_directional_slash_trail(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    origin: Vec3,
    cam_forward: Vec3,
    cam_right: Vec3,
    cam_up: Vec3,
    direction: MeleeSwingDirection,
) {
    let mut builder = LowPolyMeshBuilder::new();
    let num_segments = 12;

    match direction {
        MeleeSwingDirection::Right => {
            // Arc sweeping from upper-left to lower-right
            for i in 0..num_segments {
                let t0 = i as f32 / num_segments as f32;
                let t1 = (i + 1) as f32 / num_segments as f32;
                let angle0 = -0.9 + t0 * 1.8;
                let angle1 = -0.9 + t1 * 1.8;

                let r_inner = 0.55;
                let r_outer0 = 1.05 + 0.15 * (1.0 - (t0 - 0.5).abs() * 2.0);
                let r_outer1 = 1.05 + 0.15 * (1.0 - (t1 - 0.5).abs() * 2.0);

                let v0 = cam_right * (angle0.sin() * r_inner) + cam_up * (angle0.cos() * r_inner);
                let v1 = cam_right * (angle0.sin() * r_outer0) + cam_up * (angle0.cos() * r_outer0);
                let v2 = cam_right * (angle1.sin() * r_outer1) + cam_up * (angle1.cos() * r_outer1);
                let v3 = cam_right * (angle1.sin() * r_inner) + cam_up * (angle1.cos() * r_inner);

                let color = [0.8, 0.95, 1.0, 1.0 - (t0 - 0.5).abs() * 0.5];
                builder.add_flat_quad(v0, v1, v2, v3, color);
            }
        }
        MeleeSwingDirection::Left => {
            // Arc sweeping from upper-right to lower-left
            for i in 0..num_segments {
                let t0 = i as f32 / num_segments as f32;
                let t1 = (i + 1) as f32 / num_segments as f32;
                let angle0 = 0.9 - t0 * 1.8;
                let angle1 = 0.9 - t1 * 1.8;

                let r_inner = 0.55;
                let r_outer0 = 1.05 + 0.15 * (1.0 - (t0 - 0.5).abs() * 2.0);
                let r_outer1 = 1.05 + 0.15 * (1.0 - (t1 - 0.5).abs() * 2.0);

                let v0 = cam_right * (angle0.sin() * r_inner) + cam_up * (angle0.cos() * r_inner);
                let v1 = cam_right * (angle0.sin() * r_outer0) + cam_up * (angle0.cos() * r_outer0);
                let v2 = cam_right * (angle1.sin() * r_outer1) + cam_up * (angle1.cos() * r_outer1);
                let v3 = cam_right * (angle1.sin() * r_inner) + cam_up * (angle1.cos() * r_inner);

                let color = [0.8, 0.95, 1.0, 1.0 - (t0 - 0.5).abs() * 0.5];
                builder.add_flat_quad(v0, v1, v2, v3, color);
            }
        }
        MeleeSwingDirection::Overhead => {
            // Vertical downward cleave arc
            for i in 0..num_segments {
                let t0 = i as f32 / num_segments as f32;
                let t1 = (i + 1) as f32 / num_segments as f32;
                let angle0 = 1.1 - t0 * 2.2;
                let angle1 = 1.1 - t1 * 2.2;

                let r_inner = 0.50;
                let r_outer = 1.15;

                let v0 = cam_forward * (angle0.cos() * r_inner) + cam_up * (angle0.sin() * r_inner);
                let v1 = cam_forward * (angle0.cos() * r_outer) + cam_up * (angle0.sin() * r_outer);
                let v2 = cam_forward * (angle1.cos() * r_outer) + cam_up * (angle1.sin() * r_outer);
                let v3 = cam_forward * (angle1.cos() * r_inner) + cam_up * (angle1.sin() * r_inner);

                let color = [1.0, 0.85, 0.35, 1.0];
                builder.add_flat_quad(v0, v1, v2, v3, color);
            }
        }
        MeleeSwingDirection::Thrust => {
            // Focused linear aerodynamic thrust cone
            for i in 0..num_segments {
                let angle0 = (i as f32) * (std::f32::consts::TAU / num_segments as f32);
                let angle1 = ((i + 1) as f32) * (std::f32::consts::TAU / num_segments as f32);

                let p_tip = cam_forward * 1.5;
                let p_base0 = cam_forward * 0.3 + cam_right * (angle0.cos() * 0.18) + cam_up * (angle0.sin() * 0.18);
                let p_base1 = cam_forward * 0.3 + cam_right * (angle1.cos() * 0.18) + cam_up * (angle1.sin() * 0.18);

                let color = [0.85, 0.95, 1.0, 0.8];
                builder.add_flat_triangle(p_base0, p_tip, p_base1, color);
            }
        }
    }

    let trail_mesh = meshes.add(builder.build());
    let trail_mat = materials.add(StandardMaterial {
        base_color: match direction {
            MeleeSwingDirection::Overhead => Color::srgb(1.0, 0.85, 0.35),
            MeleeSwingDirection::Thrust => Color::srgb(0.9, 0.96, 1.0),
            _ => Color::srgb(0.75, 0.92, 1.0),
        },
        emissive: match direction {
            MeleeSwingDirection::Overhead => LinearRgba::rgb(4.0, 2.5, 0.6),
            MeleeSwingDirection::Thrust => LinearRgba::rgb(3.0, 4.0, 5.0),
            _ => LinearRgba::rgb(2.5, 4.0, 5.5),
        },
        unlit: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    commands.spawn((
        PbrBundle {
            mesh: trail_mesh,
            material: trail_mat,
            transform: BevyTransform::from_translation(origin + cam_forward * 0.45),
            ..default()
        },
        Particle { timer: Timer::from_seconds(0.14, TimerMode::Once) },
    ));
}

/// Computes procedural position and rotation offsets for directional 4-way melee attacks
/// (Right slash, Left backhand, Overhead cleave, Forward thrust) across Windup, Release, Recovery.
pub fn compute_directional_melee_transform(
    direction: MeleeSwingDirection,
    phase: MeleeAttackPhase,
    windup_t: f32,
    release_t: f32,
    recovery_t: f32,
) -> (Vec3, Quat) {
    match direction {
        MeleeSwingDirection::Right => {
            // Left-to-Right diagonal slash (mouse flick right)
            match phase {
                MeleeAttackPhase::Windup => {
                    let ease = windup_t * windup_t;
                    let pos = Vec3::new(-0.18 * ease, 0.06 * ease, 0.08 * ease);
                    let rot = Quat::from_rotation_z(-0.35 * ease)
                        * Quat::from_rotation_y(-0.45 * ease)
                        * Quat::from_rotation_x(0.20 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Release => {
                    let ease = release_t;
                    let pos = Vec3::new(
                        -0.18 + 0.42 * ease,
                        0.06 - 0.22 * ease,
                        0.08 - 0.26 * (ease * std::f32::consts::PI).sin(),
                    );
                    let rot = Quat::from_rotation_z(-0.35 + 0.80 * ease)
                        * Quat::from_rotation_y(-0.45 + 0.90 * ease)
                        * Quat::from_rotation_x(0.20 - 0.50 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Recovery => {
                    let ease = 1.0 - recovery_t;
                    let pos = Vec3::new(0.24 * ease, -0.16 * ease, -0.04 * ease);
                    let rot = Quat::from_rotation_z(0.45 * ease)
                        * Quat::from_rotation_y(0.45 * ease)
                        * Quat::from_rotation_x(-0.30 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Idle => (Vec3::ZERO, Quat::IDENTITY),
            }
        }
        MeleeSwingDirection::Left => {
            // Right-to-Left backhand slash (mouse flick left)
            match phase {
                MeleeAttackPhase::Windup => {
                    let ease = windup_t * windup_t;
                    let pos = Vec3::new(0.18 * ease, 0.08 * ease, 0.08 * ease);
                    let rot = Quat::from_rotation_z(0.35 * ease)
                        * Quat::from_rotation_y(0.50 * ease)
                        * Quat::from_rotation_x(0.20 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Release => {
                    let ease = release_t;
                    let pos = Vec3::new(
                        0.18 - 0.42 * ease,
                        0.08 - 0.22 * ease,
                        0.08 - 0.26 * (ease * std::f32::consts::PI).sin(),
                    );
                    let rot = Quat::from_rotation_z(0.35 - 0.80 * ease)
                        * Quat::from_rotation_y(0.50 - 0.95 * ease)
                        * Quat::from_rotation_x(0.20 - 0.45 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Recovery => {
                    let ease = 1.0 - recovery_t;
                    let pos = Vec3::new(-0.24 * ease, -0.14 * ease, -0.04 * ease);
                    let rot = Quat::from_rotation_z(-0.45 * ease)
                        * Quat::from_rotation_y(-0.45 * ease)
                        * Quat::from_rotation_x(-0.25 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Idle => (Vec3::ZERO, Quat::IDENTITY),
            }
        }
        MeleeSwingDirection::Overhead => {
            // Downward vertical cleave (mouse pull down)
            match phase {
                MeleeAttackPhase::Windup => {
                    let ease = windup_t * windup_t;
                    let pos = Vec3::new(0.02 * ease, 0.24 * ease, 0.06 * ease);
                    let rot = Quat::from_rotation_x(0.85 * ease) * Quat::from_rotation_z(-0.12 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Release => {
                    let ease = release_t;
                    let pos = Vec3::new(
                        0.02,
                        0.24 - 0.46 * ease,
                        0.06 - 0.24 * (ease * std::f32::consts::PI).sin(),
                    );
                    let rot = Quat::from_rotation_x(0.85 - 1.65 * ease) * Quat::from_rotation_z(-0.12 + 0.12 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Recovery => {
                    let ease = 1.0 - recovery_t;
                    let pos = Vec3::new(0.02 * ease, -0.22 * ease, -0.02 * ease);
                    let rot = Quat::from_rotation_x(-0.80 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Idle => (Vec3::ZERO, Quat::IDENTITY),
            }
        }
        MeleeSwingDirection::Thrust => {
            // Linear forward stab (mouse push up)
            match phase {
                MeleeAttackPhase::Windup => {
                    let ease = windup_t * windup_t;
                    let pos = Vec3::new(0.02 * ease, -0.04 * ease, 0.16 * ease);
                    let rot = Quat::from_rotation_y(0.12 * ease) * Quat::from_rotation_x(-0.08 * ease);
                    (pos, rot)
                }
                MeleeAttackPhase::Release => {
                    let ease = release_t;
                    let pos = Vec3::new(
                        0.02 * (1.0 - ease),
                        -0.04 + 0.06 * ease,
                        0.16 - 0.42 * ease,
                    );
                    let rot = Quat::from_rotation_y(0.12 * (1.0 - ease)) * Quat::from_rotation_x(-0.08 * (1.0 - ease));
                    (pos, rot)
                }
                MeleeAttackPhase::Recovery => {
                    let ease = 1.0 - recovery_t;
                    let pos = Vec3::new(0.0, 0.02 * ease, -0.26 * ease);
                    (pos, Quat::IDENTITY)
                }
                MeleeAttackPhase::Idle => (Vec3::ZERO, Quat::IDENTITY),
            }
        }
    }
}

// ----------------------------------------------------------------------------
// VIEWMODEL ANIMATION, RECOIL & FPS WEAPON STATE UPDATE
// ----------------------------------------------------------------------------

pub fn animate_weapon_viewmodel(
    time: Res<Time>,
    mut weapon_state: ResMut<WeaponState>,
    mut swing_state: ResMut<SwingState>,
    hand_side: Res<EquippedHandSide>,
    mut root_q: Query<(&ViewModelWeaponRoot, &mut BevyTransform), (Without<ViewModelBowArrow>, Without<ViewModelPumpSlide>)>,
    mut arrow_q: Query<(&mut Visibility, &mut BevyTransform), (With<ViewModelBowArrow>, Without<ViewModelWeaponRoot>, Without<ViewModelPumpSlide>, Without<ViewModelCrossbowBolt>)>,
    mut bolt_q: Query<&mut Visibility, (With<ViewModelCrossbowBolt>, Without<ViewModelBowArrow>)>,
) {
    let dt = time.delta_seconds();
    weapon_state.sway_time += dt;

    // Melee swing state machine & timers ticking
    match swing_state.phase {
        MeleeAttackPhase::Idle => {
            if swing_state.is_swinging {
                swing_state.timer.tick(time.delta());
                if swing_state.timer.just_finished() {
                    swing_state.is_swinging = false;
                }
            }
        }
        MeleeAttackPhase::Windup => {
            swing_state.windup_timer.tick(time.delta());
            if swing_state.windup_timer.just_finished() {
                swing_state.phase = MeleeAttackPhase::Release;
                swing_state.release_timer.reset();
            }
        }
        MeleeAttackPhase::Release => {
            swing_state.release_timer.tick(time.delta());
            if swing_state.release_timer.just_finished() {
                swing_state.phase = MeleeAttackPhase::Recovery;
                swing_state.recovery_timer.reset();
            }
        }
        MeleeAttackPhase::Recovery => {
            swing_state.recovery_timer.tick(time.delta());
            if swing_state.recovery_timer.just_finished() {
                swing_state.phase = MeleeAttackPhase::Idle;
                swing_state.is_swinging = false;
            }
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

    // Note: Legacy firearm (Revolver/Shotgun) reload & pump loops archived in docs/archive/legacy_firearms.rs

    // 2. Bolt/Arrow Visibility & Dynamic Arrow Pullback
    for mut vis in bolt_q.iter_mut() {
        let is_visible = match weapon_state.current_weapon {
            WeaponType::Crossbow => weapon_state.crossbow_loaded,
            WeaponType::HandCrossbow => weapon_state.hand_crossbow_loaded,
            _ => true,
        };
        *vis = if is_visible { Visibility::Inherited } else { Visibility::Hidden };
    }

    if !weapon_state.bow_nock_timer.finished() {
        weapon_state.bow_nock_timer.tick(time.delta());
    }

    for (mut vis, mut arrow_t) in arrow_q.iter_mut() {
        if weapon_state.current_weapon == WeaponType::Bow {
            // Hide nocked arrow immediately upon firing until nock timer finishes
            *vis = if weapon_state.bow_nock_timer.finished() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };

            // Dynamic arrow pullback along bow shelf:
            // Base rest: Vec3::new(-0.01, 0.02, -0.12).
            // When drawn: pulled backwards (+Z) by up to 0.20m towards the archer.
            let draw_offset = weapon_state.bow_charge * 0.20;
            arrow_t.translation = Vec3::new(-0.01, 0.02, -0.12 + draw_offset);
        } else {
            *vis = Visibility::Inherited;
        }
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

            // Idle ready guard posture for melee weapons
            let idle_base_rot = match weapon_state.current_weapon {
                WeaponType::Longsword | WeaponType::Greatsword | WeaponType::Rapier => {
                    Quat::from_rotation_x(0.48) * Quat::from_rotation_y(-0.25) * Quat::from_rotation_z(0.18)
                }
                WeaponType::Warhammer | WeaponType::Handaxe | WeaponType::Club => {
                    Quat::from_rotation_x(0.38) * Quat::from_rotation_y(-0.20)
                }
                WeaponType::Spear | WeaponType::Halberd => {
                    Quat::from_rotation_x(0.25) * Quat::from_rotation_y(-0.15)
                }
                _ => Quat::IDENTITY,
            };

            let mut current_rot = idle_base_rot * weapon_state.recoil_rot;
            if is_left {
                current_rot = Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06) * current_rot;
            }

            // Directional Melee Swing Animation (Mount & Blade / cRPG Style)
            if weapon_state.current_weapon.is_melee() && swing_state.phase != MeleeAttackPhase::Idle {
                let windup_t = swing_state.windup_timer.fraction();
                let release_t = swing_state.release_timer.fraction();
                let recovery_t = swing_state.recovery_timer.fraction();

                let (melee_offset, melee_rot) = compute_directional_melee_transform(
                    swing_state.direction,
                    swing_state.phase,
                    windup_t,
                    release_t,
                    recovery_t,
                );
                current_offset += melee_offset;
                current_rot = melee_rot * current_rot;
            } else if swing_state.is_swinging {
                // Main hand punch / fallback swing lunge animation
                let t = swing_state.timer.fraction();
                let punch_forward = (t * std::f32::consts::PI).sin() * 0.16;
                current_offset.z -= punch_forward;
                current_offset.y += punch_forward * 0.15;
                current_rot *= Quat::from_rotation_x(punch_forward * 0.7);
            }

            // Weapon Parry Guard (When blocking with melee weapon and no off-hand shield)
            if swing_state.is_blocking && weapon_state.offhand_weapon == WeaponType::None && weapon_state.current_weapon.is_melee() {
                let parry_pos = Vec3::new(-0.04, 0.04, -0.24);
                let parry_rot = Quat::from_rotation_z(-0.85) * Quat::from_rotation_x(0.35) * Quat::from_rotation_y(0.22);
                current_offset = current_offset.lerp(parry_pos, (dt * 18.0).min(1.0));
                current_rot = current_rot.slerp(parry_rot, (dt * 18.0).min(1.0));
            }

            // Bow ADS & Draw Stance
            if weapon_state.current_weapon == WeaponType::Bow && weapon_state.bow_drawing {
                let charge = weapon_state.bow_charge;
                current_offset += Vec3::new(-0.11 * charge, 0.06 * charge, 0.14 * charge);
                current_rot *= Quat::from_rotation_z(0.38 * charge) * Quat::from_rotation_x(0.14 * charge);
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

            let idle_offhand_rot = match weapon_state.offhand_weapon {
                WeaponType::WoodenShield => {
                    if off_is_left {
                        Quat::from_rotation_y(0.26) * Quat::from_rotation_x(0.16) * Quat::from_rotation_z(-0.08)
                    } else {
                        Quat::from_rotation_y(-0.26) * Quat::from_rotation_x(0.16) * Quat::from_rotation_z(0.08)
                    }
                }
                _ => Quat::IDENTITY,
            };

            let mut current_rot = idle_offhand_rot * weapon_state.offhand_recoil_rot;
            if off_is_left {
                current_rot = Quat::from_rotation_y(0.08) * Quat::from_rotation_z(0.06) * current_rot;
            } else {
                current_rot = Quat::from_rotation_y(-0.08) * Quat::from_rotation_z(-0.06) * current_rot;
            }

            // Shield Block Guard
            if swing_state.is_blocking && weapon_state.offhand_weapon == WeaponType::WoodenShield {
                let block_pos = Vec3::new(-0.06, -0.09, -0.24);
                let block_rot = Quat::from_rotation_y(-0.14) * Quat::from_rotation_x(0.14) * Quat::from_rotation_z(0.05);
                current_offset = current_offset.lerp(block_pos, (dt * 18.0).min(1.0));
                current_rot = current_rot.slerp(block_rot, (dt * 18.0).min(1.0));
            } else if swing_state.offhand_is_swinging {
                // Off-hand punch / swing lunge animation
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

/// Animates 3rd-person held weapon models (swords and shields) attached to the character body.
pub fn animate_third_person_weapons(
    time: Res<Time>,
    weapon_state: Res<WeaponState>,
    swing_state: Res<SwingState>,
    hand_side: Res<EquippedHandSide>,
    mut tp_root_q: Query<(&ThirdPersonWeaponRoot, &mut BevyTransform)>,
) {
    let dt = time.delta_seconds();
    let is_left = hand_side.0 == HandSide::Left;
    for (tp_root, mut tp_t) in tp_root_q.iter_mut() {
        if !tp_root.is_offhand {
            // Main-hand weapon (sword/mace/axe)
            let base_pos = if is_left { Vec3::new(-0.28, -0.55, 0.08) } else { Vec3::new(0.28, -0.55, 0.08) };
            let base_rot = match weapon_state.current_weapon {
                WeaponType::Longsword | WeaponType::Greatsword | WeaponType::Rapier => {
                    Quat::from_rotation_x(-0.75)
                        * Quat::from_rotation_y(if is_left { -0.15 } else { 0.15 })
                        * Quat::from_rotation_z(if is_left { 0.25 } else { -0.25 })
                }
                _ => Quat::from_rotation_x(-0.4),
            };

            if swing_state.is_swinging {
                let t = swing_state.timer.fraction();
                let swing_arc = (t * std::f32::consts::PI).sin() * 1.5;
                tp_t.translation = base_pos + Vec3::new(0.0, 0.08 * swing_arc, 0.18 * swing_arc);
                tp_t.rotation = base_rot * Quat::from_rotation_x(swing_arc);
            } else {
                tp_t.translation = tp_t.translation.lerp(base_pos, (dt * 12.0).min(1.0));
                tp_t.rotation = tp_t.rotation.slerp(base_rot, (dt * 12.0).min(1.0));
            }
        } else {
            // Off-hand weapon (shield)
            let off_is_left = !is_left;
            let idle_pos = if off_is_left { Vec3::new(-0.30, -0.52, 0.06) } else { Vec3::new(0.30, -0.52, 0.06) };
            let idle_rot = match weapon_state.offhand_weapon {
                WeaponType::WoodenShield => {
                    if off_is_left {
                        Quat::from_rotation_y(1.57) * Quat::from_rotation_x(0.12)
                    } else {
                        Quat::from_rotation_y(-1.57) * Quat::from_rotation_x(0.12)
                    }
                }
                _ => Quat::IDENTITY,
            };

            if swing_state.is_blocking && weapon_state.offhand_weapon == WeaponType::WoodenShield {
                // Raise shield in front of character's chest in 3rd person
                let block_pos = Vec3::new(-0.08, -0.42, 0.26);
                let block_rot = Quat::from_rotation_y(0.08) * Quat::from_rotation_x(0.1);
                tp_t.translation = tp_t.translation.lerp(block_pos, (dt * 18.0).min(1.0));
                tp_t.rotation = tp_t.rotation.slerp(block_rot, (dt * 18.0).min(1.0));
            } else if swing_state.offhand_is_swinging {
                let t = swing_state.offhand_timer.fraction();
                let punch_arc = (t * std::f32::consts::PI).sin() * 0.22;
                let punch_pos = idle_pos + Vec3::new(0.0, 0.05, punch_arc);
                tp_t.translation = tp_t.translation.lerp(punch_pos, (dt * 20.0).min(1.0));
                tp_t.rotation = tp_t.rotation.slerp(idle_rot * Quat::from_rotation_x(punch_arc), (dt * 20.0).min(1.0));
            } else {
                tp_t.translation = tp_t.translation.lerp(idle_pos, (dt * 12.0).min(1.0));
                tp_t.rotation = tp_t.rotation.slerp(idle_rot, (dt * 12.0).min(1.0));
            }
        }
    }
}

/// Traverses all descendants of `ThirdPersonWeaponRoot` and ensures their `RenderLayers`
/// is set to Layer 2 so they render in 3rd person / RTS view and never interfere with
/// 1st person viewmodels on Layer 1.
pub fn sync_third_person_weapon_render_layers(
    tp_roots: Query<&Children, With<ThirdPersonWeaponRoot>>,
    children_q: Query<&Children>,
    mut layers_q: Query<&mut RenderLayers>,
    mut commands: Commands,
) {
    for children in tp_roots.iter() {
        let mut stack: Vec<Entity> = children.iter().copied().collect();
        while let Some(entity) = stack.pop() {
            if let Ok(mut layers) = layers_q.get_mut(entity) {
                if *layers != RenderLayers::layer(2) {
                    *layers = RenderLayers::layer(2);
                }
            } else {
                commands.entity(entity).insert(RenderLayers::layer(2));
            }
            if let Ok(sub_children) = children_q.get(entity) {
                stack.extend(sub_children.iter().copied());
            }
        }
    }
}

// ----------------------------------------------------------------------------
// MANUAL RELOAD SYSTEM ('R' KEY)
// ----------------------------------------------------------------------------

pub fn weapon_reload_input_system(
    mut reload_evts: EventReader<crate::input::WeaponReloadEvent>,
    mut weapon_state: ResMut<WeaponState>,
) {
    for _ in reload_evts.read() {
        match weapon_state.current_weapon {
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
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    }

    if *vis != Visibility::Inherited {
        *vis = Visibility::Inherited;
    }

    // Only recompute and re-format strings when weapon state, equipped hand, or camera mode changes
    if !weapon_state.is_changed() && !hand_side.is_changed() && !camera_mode.is_changed() && !text.sections[0].value.is_empty() {
        return;
    }

    let hand_str = match hand_side.0 {
        HandSide::Right => "PRIMARY: RIGHT [H to Swap]",
        HandSide::Left => "PRIMARY: LEFT [H to Swap]",
    };

    let new_val = if weapon_state.current_weapon == WeaponType::None && weapon_state.offhand_weapon == WeaponType::None {
        format!("{}\nUNARMED [BARE FISTS]\n[LMB] Right Jab | [RMB] Left Jab", hand_str)
    } else {
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
        format!("{}\n{}\n{}", hand_str, main_desc, off_desc)
    };

    if text.sections[0].value != new_val {
        text.sections[0].value = new_val;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directional_melee_transforms_all_directions() {
        let directions = [
            MeleeSwingDirection::Right,
            MeleeSwingDirection::Left,
            MeleeSwingDirection::Overhead,
            MeleeSwingDirection::Thrust,
        ];
        let phases = [
            MeleeAttackPhase::Idle,
            MeleeAttackPhase::Windup,
            MeleeAttackPhase::Release,
            MeleeAttackPhase::Recovery,
        ];

        for &dir in &directions {
            for &phase in &phases {
                let (pos, rot) = compute_directional_melee_transform(dir, phase, 0.5, 0.5, 0.5);
                assert!(!pos.x.is_nan() && !pos.y.is_nan() && !pos.z.is_nan());
                assert!(!rot.x.is_nan() && !rot.y.is_nan() && !rot.z.is_nan() && !rot.w.is_nan());
                assert!(rot.is_normalized(), "Rotation must be normalized for {:?} {:?}", dir, phase);

                if phase == MeleeAttackPhase::Idle {
                    assert_eq!(pos, Vec3::ZERO);
                    assert_eq!(rot, Quat::IDENTITY);
                }
            }
        }
    }

    #[test]
    fn test_wooden_shield_position_mirroring() {
        let right_pos = get_default_weapon_pos(WeaponType::WoodenShield, false);
        let left_pos = get_default_weapon_pos(WeaponType::WoodenShield, true);

        assert_eq!(right_pos.x, 0.24);
        assert_eq!(left_pos.x, -0.24);
        assert_eq!(right_pos.y, left_pos.y);
        assert_eq!(right_pos.z, left_pos.z);
    }

    #[test]
    fn test_sword_and_board_archetype_properties() {
        assert!(WeaponType::Longsword.is_melee());
        assert!(WeaponType::Longsword.is_one_handed());
        assert!(!WeaponType::Longsword.is_two_handed());

        assert!(WeaponType::WoodenShield.is_one_handed());
        assert!(!WeaponType::WoodenShield.is_two_handed());
    }

    #[test]
    fn test_weapon_systems_ecs_disjointness_and_registration() {
        // Architectural Guard: Verifies that weapon systems have disjoint queries and do not cause B0001 panics
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();

        app.insert_resource(WeaponState::default());
        app.insert_resource(SwingState::default());
        app.insert_resource(EquippedHandSide(HandSide::Right));
        app.insert_resource(ActiveEquippedItem(Some("Longsword".to_string())));
        app.insert_resource(ActiveOffHandItem(Some("Wooden Shield".to_string())));

        app.add_systems(Update, (
            spawn_or_update_view_model_weapon,
            animate_weapon_viewmodel,
            animate_third_person_weapons,
            sync_third_person_weapon_render_layers,
        ));

        // Spawns camera and player body
        let _cam = app.world_mut().spawn((FpsCamera, BevyTransform::default())).id();
        let _player = app.world_mut().spawn((PlayerBody, BevyTransform::default())).id();

        // Run update - this will panic with B0001 if any query parameters conflict!
        app.update();

        assert_eq!(app.world().resource::<WeaponState>().current_weapon, WeaponType::Longsword);
        assert_eq!(app.world().resource::<WeaponState>().offhand_weapon, WeaponType::WoodenShield);
    }

    #[test]
    fn test_all_weapon_meshes_generation() {
        let weapons = [
            WeaponType::None,
            WeaponType::Bow,
            WeaponType::Crossbow,
            WeaponType::HandCrossbow,
            WeaponType::Revolver,
            WeaponType::Shotgun,
            WeaponType::SniperRifle,
            WeaponType::BouncyBombLauncher,
            WeaponType::Runestaff,
            WeaponType::Wand,
            WeaponType::Orb,
            WeaponType::Halberd,
            WeaponType::Longsword,
            WeaponType::Greatsword,
            WeaponType::TwoHandAxe,
            WeaponType::Maul,
            WeaponType::Spear,
            WeaponType::Javelin,
            WeaponType::Trident,
            WeaponType::Rapier,
            WeaponType::Warhammer,
            WeaponType::Club,
            WeaponType::Dagger,
            WeaponType::Handaxe,
            WeaponType::TigerClaws,
            WeaponType::BlackJack,
            WeaponType::Cestus,
            WeaponType::KnuckleDuster,
            WeaponType::FryingPan,
            WeaponType::HolyMackerel,
            WeaponType::Hammer,
            WeaponType::Pickaxe,
            WeaponType::Torch,
            WeaponType::WoodenShield,
        ];

        for w in weapons {
            let mesh = create_lowpoly_weapon_mesh(w);
            let pos_len = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len();
            let norm_len = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap().len();
            let col_len = mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap().len();
            assert_eq!(pos_len, norm_len);
            assert_eq!(pos_len, col_len);
            assert!(pos_len > 0, "Weapon {:?} mesh should not be empty", w);
        }

        let arrow = create_lowpoly_arrow_mesh();
        assert!(arrow.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len() > 0);

        let bolt = create_lowpoly_bolt_mesh();
        assert!(bolt.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len() > 0);

        let slide = create_lowpoly_pumpslide_mesh();
        assert!(slide.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len() > 0);
    }

    #[test]
    fn test_bow_arrow_pullback_and_projectile_trajectory() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(WeaponState {
            current_weapon: WeaponType::Bow,
            bow_charge: 0.75,
            bow_drawing: true,
            ..default()
        });
        app.insert_resource(SwingState::default());
        app.insert_resource(EquippedHandSide(HandSide::Right));

        let arrow_id = app.world_mut().spawn((
            ViewModelBowArrow,
            Visibility::Inherited,
            BevyTransform::from_xyz(-0.01, 0.02, -0.12),
        )).id();

        let proj_id = app.world_mut().spawn((
            ArrowProjectile,
            LinearVelocity(Vec3::new(30.0, -10.0, 0.0)),
            BevyTransform::default(),
        )).id();

        app.add_systems(Update, (
            animate_weapon_viewmodel,
            update_arrow_projectiles,
        ));

        app.update();

        // 1. Arrow should have shifted back along +Z proportional to bow_charge (0.75 * 0.20 = 0.15)
        let arrow_trans = app.world().get::<BevyTransform>(arrow_id).unwrap();
        assert!((arrow_trans.translation.z - (-0.12 + 0.75 * 0.20)).abs() < 1e-4);

        // 2. Projectile should have aligned forward with its velocity vector
        let proj_trans = app.world().get::<BevyTransform>(proj_id).unwrap();
        let expected_dir = Vec3::new(30.0, -10.0, 0.0).normalize();
        let actual_forward = proj_trans.forward().as_vec3();
        assert!((actual_forward.dot(expected_dir) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_weapon_archetypes_and_canonical_slice() {
        assert_eq!(WeaponType::Longsword.archetype(), WeaponArchetype::OneHandBlade);
        assert_eq!(WeaponType::Greatsword.archetype(), WeaponArchetype::TwoHandBlade);
        assert_eq!(WeaponType::Handaxe.archetype(), WeaponArchetype::OneHandAxe);
        assert_eq!(WeaponType::TwoHandAxe.archetype(), WeaponArchetype::TwoHandAxe);
        assert_eq!(WeaponType::Warhammer.archetype(), WeaponArchetype::OneHandBludgeon);
        assert_eq!(WeaponType::Maul.archetype(), WeaponArchetype::TwoHandBludgeon);
        assert_eq!(WeaponType::Spear.archetype(), WeaponArchetype::Polearm);
        assert_eq!(WeaponType::Bow.archetype(), WeaponArchetype::Bow);
        assert_eq!(WeaponType::Crossbow.archetype(), WeaponArchetype::Crossbow);
        assert_eq!(WeaponType::Wand.archetype(), WeaponArchetype::MagicImplement);
        assert_eq!(WeaponType::WoodenShield.archetype(), WeaponArchetype::Shield);
        assert_eq!(WeaponType::Hammer.archetype(), WeaponArchetype::UtilityTool);
        assert!(WeaponType::Longsword.is_canonical_vertical_slice());
        assert!(WeaponType::Bow.is_canonical_vertical_slice());
        assert!(!WeaponType::Revolver.is_canonical_vertical_slice());
        assert!(!WeaponType::Shotgun.is_canonical_vertical_slice());
    }
}
