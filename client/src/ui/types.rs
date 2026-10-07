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
    "Dragonflight Haversack",
    "Elder Wood",
    "Flint",
    "Flint Arrow",
    "Flint Spear",
    "Frying Pan",
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

// ----------------------------------------------------------------------------
// STRONGLY-TYPED EQUIPMENT SLOTS
// ----------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EquipmentSlot {
    MainHand,
    OffHand,
    Head,
    Chest,
    Legs,
    Feet,
}

#[allow(dead_code)]
impl EquipmentSlot {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MainHand => "MainHand",
            Self::OffHand => "OffHand",
            Self::Head => "Head",
            Self::Chest => "Chest",
            Self::Legs => "Legs",
            Self::Feet => "Feet",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "MainHand" | "main_hand" | "main" => Some(Self::MainHand),
            "OffHand" | "off_hand" | "off" => Some(Self::OffHand),
            "Head" | "head" | "helmet" => Some(Self::Head),
            "Chest" | "chest" | "chestplate" => Some(Self::Chest),
            "Legs" | "legs" | "greaves" => Some(Self::Legs),
            "Feet" | "feet" | "boots" => Some(Self::Feet),
            _ => None,
        }
    }

    pub fn is_hand(&self) -> bool {
        matches!(self, Self::MainHand | Self::OffHand)
    }

    pub fn is_armor(&self) -> bool {
        matches!(self, Self::Head | Self::Chest | Self::Legs | Self::Feet)
    }
}

// ----------------------------------------------------------------------------
// STRONGLY-TYPED ITEM KIND ENUM
// ----------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    // Core Medieval/Fantasy Weapons (Vertical Slice)
    Longsword,
    Greatsword,
    Handaxe,
    TwoHandAxe,
    Warhammer,
    Maul,
    Club,
    Dagger,
    Spear,
    Halberd,
    Bow,
    Crossbow,
    Wand,
    Runestaff,
    WoodenShield,
    Hammer,
    Pickaxe,
    Torch,

    // Ammunition
    WoodArrow,
    FlintArrow,
    CrossbowBolt,

    // Armor Pieces
    IronHelmet,
    IronChestplate,
    IronGreaves,
    IronBoots,

    // Containers & Bags
    SmallPouch,
    LeatherBag,
    TravelersBackpack,
    DragonflightHaversack,

    // Resources & Survival Materials
    Wood,
    ElderWood,
    Branch,
    Stone,
    LooseStone,
    Flint,
    Resin,
    IronIngot,
    LeatherScraps,
    Berry,
    CookedMeat,
    Honey,

    // Unarmed Natural State
    Unarmed,
}

#[allow(dead_code)]
impl ItemKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            // Weapons - Blades
            "1h Sword" | "Sword" | "Longsword" | "Knight's Longsword" => Some(Self::Longsword),
            "2h Sword" | "Greatsword" | "Two-Handed Sword" | "Zweihander Greatsword" => Some(Self::Greatsword),
            "Dagger" | "Stiletto Dagger" => Some(Self::Dagger),

            // Weapons - Axes
            "1h Axe" | "Handaxe" | "Stone Axe" | "Bearded Handaxe" => Some(Self::Handaxe),
            "2h Axe" | "Battleaxe" | "Greataxe" | "Two-Handed Axe" => Some(Self::TwoHandAxe),

            // Weapons - Bludgeons
            "Club" | "Knotted War Club" => Some(Self::Club),
            "1h Hammer" | "Warhammer" | "Flanged Warhammer" => Some(Self::Warhammer),
            "2h Hammer" | "Maul" | "Two-Handed Hammer" | "Heavy Iron Maul" => Some(Self::Maul),

            // Weapons - Polearms
            "Spear" | "Polearm Spear" | "Flint Spear" | "Hunting Spear" => Some(Self::Spear),
            "Halberd" | "Halberd Polearm" => Some(Self::Halberd),

            // Weapons - Ranged & Magic
            "Bow" | "Long Bow" | "Longbow" | "2h Ranged Long Bow" | "Crude Bow" => Some(Self::Bow),
            "Crossbow" | "Heavy Crossbow" => Some(Self::Crossbow),
            "Wand" | "1h Ranged Wand" | "Arcane Wand" => Some(Self::Wand),
            "Runestaff" | "2h Ranged Runestaff" => Some(Self::Runestaff),

            // Shields & Tools
            "Wooden Shield" | "Reinforced Wooden Shield" => Some(Self::WoodenShield),
            "Hammer" | "Construction Hammer" => Some(Self::Hammer),
            "Pickaxe" | "Mining Pickaxe" => Some(Self::Pickaxe),
            "Torch" | "Pitch Torch" => Some(Self::Torch),

            // Ammunition
            "Wood Arrow" => Some(Self::WoodArrow),
            "Flint Arrow" => Some(Self::FlintArrow),
            "Crossbow Bolt" => Some(Self::CrossbowBolt),

            // Armor
            "Iron Helmet" => Some(Self::IronHelmet),
            "Iron Chestplate" => Some(Self::IronChestplate),
            "Iron Greaves" => Some(Self::IronGreaves),
            "Iron Boots" => Some(Self::IronBoots),

            // Bags
            "Small Pouch" => Some(Self::SmallPouch),
            "Leather Bag" => Some(Self::LeatherBag),
            "Traveler's Backpack" => Some(Self::TravelersBackpack),
            "Dragonflight Haversack" | "Bag of Sewn Evil-Eye" | "Shralok Pack" => Some(Self::DragonflightHaversack),

            // Resources
            "Wood" => Some(Self::Wood),
            "Elder Wood" => Some(Self::ElderWood),
            "Branch" => Some(Self::Branch),
            "Stone" => Some(Self::Stone),
            "LooseStone" => Some(Self::LooseStone),
            "Flint" => Some(Self::Flint),
            "Resin" => Some(Self::Resin),
            "Iron Ingot" => Some(Self::IronIngot),
            "Leather Scraps" => Some(Self::LeatherScraps),
            "Berry" => Some(Self::Berry),
            "Cooked Meat" => Some(Self::CookedMeat),
            "Honey" => Some(Self::Honey),

            // Unarmed
            "Unarmed" | "None" => Some(Self::Unarmed),

            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Longsword => "Longsword",
            Self::Greatsword => "Greatsword",
            Self::Handaxe => "Handaxe",
            Self::TwoHandAxe => "TwoHandAxe",
            Self::Warhammer => "Warhammer",
            Self::Maul => "Maul",
            Self::Club => "Club",
            Self::Dagger => "Dagger",
            Self::Spear => "Spear",
            Self::Halberd => "Halberd",
            Self::Bow => "Bow",
            Self::Crossbow => "Crossbow",
            Self::Wand => "Wand",
            Self::Runestaff => "Runestaff",
            Self::WoodenShield => "Wooden Shield",
            Self::Hammer => "Hammer",
            Self::Pickaxe => "Pickaxe",
            Self::Torch => "Torch",
            Self::WoodArrow => "Wood Arrow",
            Self::FlintArrow => "Flint Arrow",
            Self::CrossbowBolt => "Crossbow Bolt",
            Self::IronHelmet => "Iron Helmet",
            Self::IronChestplate => "Iron Chestplate",
            Self::IronGreaves => "Iron Greaves",
            Self::IronBoots => "Iron Boots",
            Self::SmallPouch => "Small Pouch",
            Self::LeatherBag => "Leather Bag",
            Self::TravelersBackpack => "Traveler's Backpack",
            Self::DragonflightHaversack => "Dragonflight Haversack",
            Self::Wood => "Wood",
            Self::ElderWood => "Elder Wood",
            Self::Branch => "Branch",
            Self::Stone => "Stone",
            Self::LooseStone => "LooseStone",
            Self::Flint => "Flint",
            Self::Resin => "Resin",
            Self::IronIngot => "Iron Ingot",
            Self::LeatherScraps => "Leather Scraps",
            Self::Berry => "Berry",
            Self::CookedMeat => "Cooked Meat",
            Self::Honey => "Honey",
            Self::Unarmed => "Unarmed",
        }
    }

    pub fn to_weapon_type(&self) -> crate::weapons::WeaponType {
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
            Self::Wand => crate::weapons::WeaponType::Wand,
            Self::Runestaff => crate::weapons::WeaponType::Runestaff,
            Self::WoodenShield => crate::weapons::WeaponType::WoodenShield,
            Self::Hammer => crate::weapons::WeaponType::Hammer,
            Self::Pickaxe => crate::weapons::WeaponType::Pickaxe,
            Self::Torch => crate::weapons::WeaponType::Torch,
            Self::Unarmed => crate::weapons::WeaponType::None,
            _ => crate::weapons::WeaponType::None,
        }
    }

    pub fn is_two_handed(&self) -> bool {
        matches!(
            self,
            Self::Greatsword
                | Self::TwoHandAxe
                | Self::Maul
                | Self::Spear
                | Self::Halberd
                | Self::Bow
                | Self::Crossbow
                | Self::Runestaff
        )
    }

    pub fn is_bag(&self) -> bool {
        matches!(
            self,
            Self::SmallPouch
                | Self::LeatherBag
                | Self::TravelersBackpack
                | Self::DragonflightHaversack
        )
    }

    pub fn is_ammo(&self) -> bool {
        matches!(self, Self::WoodArrow | Self::FlintArrow | Self::CrossbowBolt)
    }

    pub fn is_armor(&self) -> bool {
        matches!(
            self,
            Self::IronHelmet | Self::IronChestplate | Self::IronGreaves | Self::IronBoots
        )
    }

    pub fn is_tool(&self) -> bool {
        matches!(self, Self::Hammer | Self::Pickaxe | Self::Torch)
    }
}

#[derive(Component)]
pub struct WorkbenchHeaderStatus;

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientEquippedBags {
    pub bags: [Option<BagContainerDef>; 4],
}

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

