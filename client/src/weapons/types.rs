// ============================================================================
// File: client/src/weapons/types.rs
// ============================================================================
// ----------------------------------------------------------------------------
// WEAPON DATA STRUCTURES, ENUMS, AND CLASSIFICATIONS
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use spacetime_rts_logic::HandSide;

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
        WeaponType::Longsword => Vec3::new(0.24, -0.16, -0.38),
        WeaponType::Greatsword => Vec3::new(0.24, -0.20, -0.46),
        WeaponType::TwoHandAxe => Vec3::new(0.24, -0.20, -0.46),
        WeaponType::Maul => Vec3::new(0.24, -0.20, -0.46),
        WeaponType::Spear => Vec3::new(0.24, -0.22, -0.50),
        WeaponType::Javelin => Vec3::new(0.22, -0.16, -0.40),
        WeaponType::Trident => Vec3::new(0.24, -0.22, -0.50),
        WeaponType::Rapier => Vec3::new(0.22, -0.16, -0.38),
        WeaponType::Warhammer => Vec3::new(0.24, -0.16, -0.38),
        WeaponType::Club => Vec3::new(0.22, -0.14, -0.34),
        WeaponType::Dagger => Vec3::new(0.20, -0.12, -0.30),
        WeaponType::Handaxe => Vec3::new(0.22, -0.14, -0.34),
        WeaponType::TigerClaws => Vec3::new(0.18, -0.10, -0.26),
        WeaponType::BlackJack => Vec3::new(0.20, -0.12, -0.30),
        WeaponType::Cestus => Vec3::new(0.18, -0.10, -0.26),
        WeaponType::KnuckleDuster => Vec3::new(0.18, -0.10, -0.26),
        WeaponType::FryingPan => Vec3::new(0.22, -0.14, -0.34),
        WeaponType::HolyMackerel => Vec3::new(0.22, -0.14, -0.34),
        WeaponType::Hammer => Vec3::new(0.22, -0.14, -0.32),
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
