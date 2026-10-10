// ============================================================================
// File: client/src/ui/types.rs
// ============================================================================
use bevy::prelude::*;
use spacetime_rts_logic::BagContainerDef;

pub const CANONICAL_ITEMS: &[&str] = &[
    "1h Axe",
    "1h Black Jack",
    "1h Hammer",
    "1h Ranged Hand Crossbow",
    "1h Ranged Orb",
    "1h Ranged Revolver",
    "1h Ranged Wand",
    "1h Sword",
    "1h Tiger Claws",
    "2h Axe",
    "2h Hammer",
    "2h Ranged Long Bow",
    "2h Ranged Runestaff",
    "2h Ranged Shotgun",
    "2h Ranged Sniper Rifle",
    "2h Sword",
    "Bag of Sewn Evil-Eye",
    "Ballista",
    "Battering Ram",
    "Berry",
    "Black Jack",
    "Bouncy Bomb Ammo",
    "Bouncy Bomb Launcher",
    "Bow",
    "Branch",
    "Catapult",
    "Cestus",
    "Club",
    "Cooked Meat",
    "Crossbow",
    "Crossbow Bolt",
    "Crude Bow",
    "Dagger",
    "Dragon Helm",
    "Dragonflight Haversack",
    "Elder Wood",
    "Flint",
    "Flint Arrow",
    "Flint Spear",
    "Frying Pan",
    "Golden Dragon Helm",
    "Greataxe",
    "Greatsword",
    "Halberd",
    "Hammer",
    "Hand Crossbow",
    "Handaxe",
    "Holy Mackerel",
    "Honey",
    "Iron Boots",
    "Iron Chestplate",
    "Iron Greaves",
    "Iron Helmet",
    "Iron Ingot",
    "Javelin",
    "Knuckle-Duster",
    "Leather Bag",
    "Leather Scraps",
    "Long Bow",
    "Longbow",
    "Longsword",
    "LooseStone",
    "Maul",
    "Orb",
    "Pickaxe",
    "Polearm Javelin - Thrown",
    "Polearm Spear",
    "Polearm Trident",
    "Rapier",
    "Resin",
    "Revolver",
    "Revolver Ammo",
    "Runestaff",
    "Shotgun",
    "Shotgun Shell",
    "Shralok Pack",
    "Small Pouch",
    "Sniper Ammo",
    "Sniper Rifle",
    "Spear",
    "Stone",
    "Stone Axe",
    "Tiger Claws",
    "Torch",
    "Traveler's Backpack",
    "Trebuchet",
    "Trident",
    "Unarmed",
    "Wand",
    "Warhammer",
    "Wood",
    "Wood Arrow",
    "Wooden Shield",
];

#[allow(unused_imports)]
pub use spacetime_rts_logic::{
    CombatEventType, EquipmentSlot, ItemKind, ResourceNodeType, StructurePieceType, ToolKind,
};

pub trait ItemKindWeaponExt {
    fn to_weapon_type(&self) -> crate::weapons::WeaponType;
}

impl ItemKindWeaponExt for ItemKind {
    fn to_weapon_type(&self) -> crate::weapons::WeaponType {
        match self {
            Self::Longsword => crate::weapons::WeaponType::Longsword,
            Self::Greatsword => crate::weapons::WeaponType::Greatsword,
            Self::Handaxe => crate::weapons::WeaponType::Handaxe,
            Self::TwoHandAxe => crate::weapons::WeaponType::TwoHandAxe,
            Self::Warhammer => crate::weapons::WeaponType::Warhammer,
            Self::Maul => crate::weapons::WeaponType::Maul,
            Self::Club => crate::weapons::WeaponType::Club,
            Self::Dagger => crate::weapons::WeaponType::Dagger,
            Self::Spear => crate::weapons::WeaponType::Spear,
            Self::Halberd => crate::weapons::WeaponType::Halberd,
            Self::Bow => crate::weapons::WeaponType::Bow,
            Self::Crossbow => crate::weapons::WeaponType::Crossbow,
            Self::HandCrossbow => crate::weapons::WeaponType::HandCrossbow,
            Self::Revolver => crate::weapons::WeaponType::Revolver,
            Self::Shotgun => crate::weapons::WeaponType::Shotgun,
            Self::SniperRifle => crate::weapons::WeaponType::SniperRifle,
            Self::BouncyBombLauncher => crate::weapons::WeaponType::BouncyBombLauncher,
            Self::Wand => crate::weapons::WeaponType::Wand,
            Self::Orb => crate::weapons::WeaponType::Orb,
            Self::Runestaff => crate::weapons::WeaponType::Runestaff,
            Self::WoodenShield => crate::weapons::WeaponType::WoodenShield,
            Self::Hammer => crate::weapons::WeaponType::Hammer,
            Self::Pickaxe => crate::weapons::WeaponType::Pickaxe,
            Self::Torch => crate::weapons::WeaponType::Torch,
            Self::Javelin => crate::weapons::WeaponType::Javelin,
            Self::Trident => crate::weapons::WeaponType::Trident,
            Self::Rapier => crate::weapons::WeaponType::Rapier,
            Self::TigerClaws => crate::weapons::WeaponType::TigerClaws,
            Self::BlackJack => crate::weapons::WeaponType::BlackJack,
            Self::Cestus => crate::weapons::WeaponType::Cestus,
            Self::KnuckleDuster => crate::weapons::WeaponType::KnuckleDuster,
            Self::FryingPan => crate::weapons::WeaponType::FryingPan,
            Self::HolyMackerel => crate::weapons::WeaponType::HolyMackerel,
            _ => crate::weapons::WeaponType::None,
        }
    }
}

#[derive(Component)]
pub struct WorkbenchHeaderStatus;

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientEquippedBags {
    pub bags: [Option<BagContainerDef>; 4],
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientEquippedArmor {
    pub head: Option<String>,
    pub chest: Option<String>,
    pub legs: Option<String>,
    pub feet: Option<String>,
}

#[derive(Component)] pub struct PaperdollHeadSlot;
#[derive(Component)] pub struct PaperdollHeadText;
#[derive(Component)] pub struct PaperdollUnequipHeadButton;
#[derive(Component)] pub struct PaperdollMainHandSlot;
#[derive(Component)] pub struct PaperdollOffHandSlot;
#[derive(Component)] pub struct PaperdollMainHandText;
#[derive(Component)] pub struct PaperdollOffHandText;
#[derive(Component)] pub struct PaperdollPrimaryHandText;
#[derive(Component)] pub struct PaperdollPrimaryHandButton;
#[derive(Component)] pub struct PaperdollUnequipMainButton;
#[derive(Component)] pub struct PaperdollUnequipOffButton;
#[derive(Component, Clone, Copy, Debug)] pub struct PaperdollBagSlotIndex(pub usize);
#[derive(Component, Clone, Copy, Debug)] pub struct PaperdollBagText(pub usize);
#[derive(Component, Clone, Copy, Debug)] pub struct PaperdollBagTooltip(pub usize);
#[derive(Component)] pub struct InventoryCapacityHeader;
#[derive(Component)] pub struct CelestialHudRoot;
#[derive(Component)] pub struct CelestialHudText;
#[derive(Component)] pub struct DiagnosticOverlayRoot;
#[derive(Component)] pub struct DiagnosticOverlayText;
#[derive(Component)] pub struct RequiresWorkbenchRecipe;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equipment_slot_properties() {
        assert_eq!(EquipmentSlot::from_str("MainHand"), Some(EquipmentSlot::MainHand));
        assert_eq!(EquipmentSlot::from_str("off"), Some(EquipmentSlot::OffHand));
        assert!(EquipmentSlot::MainHand.is_hand());
        assert!(EquipmentSlot::OffHand.is_hand());
        assert!(!EquipmentSlot::Head.is_hand());
        assert!(EquipmentSlot::Head.is_armor());
        assert_eq!(EquipmentSlot::MainHand.as_str(), "MainHand");
    }

    #[test]
    fn test_item_kind_properties() {
        let bow = ItemKind::from_name("Crude Bow").unwrap();
        assert_eq!(bow, ItemKind::Bow);
        assert!(bow.is_two_handed());
        assert!(!bow.is_armor());
        assert_eq!(bow.as_str(), "Bow");

        let bag = ItemKind::from_name("Leather Bag").unwrap();
        assert!(bag.is_bag());

        let arrow = ItemKind::from_name("Flint Arrow").unwrap();
        assert!(arrow.is_ammo());

        let armor = ItemKind::from_name("Iron Chestplate").unwrap();
        assert!(armor.is_armor());

        let hammer = ItemKind::from_name("Hammer").unwrap();
        assert!(hammer.is_tool());
    }
}

