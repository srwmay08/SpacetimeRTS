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

