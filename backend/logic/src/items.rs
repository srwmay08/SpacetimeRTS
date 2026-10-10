// ----------------------------------------------------------------------------
// AUTHORITATIVE SHARED ITEMS & EQUIPMENT TYPE SYSTEM
// ----------------------------------------------------------------------------
// Architectural Note: Provides strongly-typed enums for items, equipment slots,
// tools, and resource node types shared across the WASM backend, native client,
// and simulation tests. Replaces brittle stringly-typed literals and prevents
// network desyncs.

// ----------------------------------------------------------------------------
// 1. EQUIPMENT SLOTS
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EquipmentSlot {
    MainHand,
    OffHand,
    Head,
    Chest,
    Legs,
    Feet,
}

impl EquipmentSlot {
    pub const fn as_str(&self) -> &'static str {
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
        let clean = s.trim();
        if clean.eq_ignore_ascii_case("MainHand")
            || clean.eq_ignore_ascii_case("main_hand")
            || clean.eq_ignore_ascii_case("main")
            || clean.eq_ignore_ascii_case("right")
        {
            Some(Self::MainHand)
        } else if clean.eq_ignore_ascii_case("OffHand")
            || clean.eq_ignore_ascii_case("off_hand")
            || clean.eq_ignore_ascii_case("off")
            || clean.eq_ignore_ascii_case("left")
        {
            Some(Self::OffHand)
        } else if clean.eq_ignore_ascii_case("Head") || clean.eq_ignore_ascii_case("helmet") {
            Some(Self::Head)
        } else if clean.eq_ignore_ascii_case("Chest") || clean.eq_ignore_ascii_case("chestplate") {
            Some(Self::Chest)
        } else if clean.eq_ignore_ascii_case("Legs") || clean.eq_ignore_ascii_case("greaves") {
            Some(Self::Legs)
        } else if clean.eq_ignore_ascii_case("Feet") || clean.eq_ignore_ascii_case("boots") {
            Some(Self::Feet)
        } else {
            None
        }
    }

    pub const fn is_hand(&self) -> bool {
        matches!(self, Self::MainHand | Self::OffHand)
    }

    pub const fn is_armor(&self) -> bool {
        matches!(self, Self::Head | Self::Chest | Self::Legs | Self::Feet)
    }
}

// ----------------------------------------------------------------------------
// 2. STRONGLY-TYPED ITEM KIND ENUM
// ----------------------------------------------------------------------------

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

    // Extended / Firearms / Comedic / Experimental
    HandCrossbow,
    Revolver,
    Shotgun,
    SniperRifle,
    BouncyBombLauncher,
    Orb,
    Javelin,
    Trident,
    Rapier,
    TigerClaws,
    BlackJack,
    Cestus,
    KnuckleDuster,
    FryingPan,
    HolyMackerel,

    // Ammunition
    WoodArrow,
    FlintArrow,
    CrossbowBolt,
    RevolverAmmo,
    ShotgunShell,
    SniperAmmo,
    BouncyBombAmmo,

    // Armor Pieces
    IronHelmet,
    DragonHelm,
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
    IronOre,
    IronIngot,
    Ruby,
    LeatherScraps,
    Berry,
    CookedMeat,
    Honey,

    // Siege Engines
    Ballista,
    Catapult,
    Trebuchet,
    BatteringRam,

    // Unarmed Natural State
    Unarmed,
}

impl ItemKind {
    /// Resolves canonical and alias item strings into strongly-typed ItemKind.
    pub fn from_name(name: &str) -> Option<Self> {
        let clean = name.trim();
        match clean {
            // Weapons - Blades
            "1h Sword" | "Sword" | "Longsword" | "Knight's Longsword" => Some(Self::Longsword),
            "2h Sword" | "Greatsword" | "Two-Handed Sword" | "Zweihander Greatsword" => Some(Self::Greatsword),
            "Dagger" | "Stiletto Dagger" => Some(Self::Dagger),
            "Rapier" => Some(Self::Rapier),

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
            "Polearm Javelin - Thrown" | "Polearm Javelin" | "Javelin" | "Thrown Javelin" => Some(Self::Javelin),
            "Polearm Trident" | "Trident" => Some(Self::Trident),

            // Weapons - Ranged & Magic
            "Bow" | "Long Bow" | "Longbow" | "2h Ranged Long Bow" | "Crude Bow" => Some(Self::Bow),
            "Crossbow" | "Heavy Crossbow" => Some(Self::Crossbow),
            "1h Ranged Hand Crossbow" | "Hand Crossbow" => Some(Self::HandCrossbow),
            "1h Ranged Revolver" | "Revolver" => Some(Self::Revolver),
            "2h Ranged Shotgun" | "Shotgun" => Some(Self::Shotgun),
            "2h Ranged Sniper Rifle" | "Sniper Rifle" => Some(Self::SniperRifle),
            "Bouncy Bomb Launcher" => Some(Self::BouncyBombLauncher),
            "1h Ranged Wand" | "Wand" | "Arcane Wand" => Some(Self::Wand),
            "1h Ranged Orb" | "Orb" => Some(Self::Orb),
            "2h Ranged Runestaff" | "Runestaff" => Some(Self::Runestaff),

            // Shields & Tools
            "Wooden Shield" | "Reinforced Wooden Shield" => Some(Self::WoodenShield),
            "Hammer" | "Construction Hammer" => Some(Self::Hammer),
            "Pickaxe" | "Mining Pickaxe" => Some(Self::Pickaxe),
            "Torch" | "Pitch Torch" => Some(Self::Torch),

            // Comedic / Brawling
            "1h Tiger Claws" | "Tiger Claws" => Some(Self::TigerClaws),
            "1h Black Jack" | "Black Jack" | "Blackjack" => Some(Self::BlackJack),
            "Cestus" => Some(Self::Cestus),
            "Knuckle-Duster" | "Knuckle Duster" => Some(Self::KnuckleDuster),
            "Frying Pan" => Some(Self::FryingPan),
            "Holy Mackerel" => Some(Self::HolyMackerel),

            // Ammunition
            "Wood Arrow" => Some(Self::WoodArrow),
            "Flint Arrow" => Some(Self::FlintArrow),
            "Crossbow Bolt" => Some(Self::CrossbowBolt),
            "Revolver Ammo" => Some(Self::RevolverAmmo),
            "Shotgun Shell" => Some(Self::ShotgunShell),
            "Sniper Ammo" => Some(Self::SniperAmmo),
            "Bouncy Bomb Ammo" => Some(Self::BouncyBombAmmo),

            // Armor
            "Dragon Helm" | "Golden Dragon Helm" | "Golden Dragon Helmet" | "Dragon Helmet" | "Winged Dragon Helm" | "Helm of the Dragon" => Some(Self::DragonHelm),
            "Iron Helmet" => Some(Self::IronHelmet),
            "Iron Chestplate" => Some(Self::IronChestplate),
            "Iron Greaves" => Some(Self::IronGreaves),
            "Iron Boots" => Some(Self::IronBoots),

            // Containers & Bags
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
            "Iron Ore" | "IronOre" => Some(Self::IronOre),
            "Iron Ingot" => Some(Self::IronIngot),
            "Ruby" => Some(Self::Ruby),
            "Leather Scraps" => Some(Self::LeatherScraps),
            "Berry" => Some(Self::Berry),
            "Cooked Meat" => Some(Self::CookedMeat),
            "Honey" => Some(Self::Honey),

            // Siege Engines
            "Ballista" => Some(Self::Ballista),
            "Catapult" => Some(Self::Catapult),
            "Trebuchet" => Some(Self::Trebuchet),
            "Battering Ram" => Some(Self::BatteringRam),

            // Unarmed
            "Unarmed" | "None" | "" => Some(Self::Unarmed),

            _ => None,
        }
    }

    pub const fn as_str(&self) -> &'static str {
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
            Self::HandCrossbow => "Hand Crossbow",
            Self::Revolver => "Revolver",
            Self::Shotgun => "Shotgun",
            Self::SniperRifle => "Sniper Rifle",
            Self::BouncyBombLauncher => "Bouncy Bomb Launcher",
            Self::Orb => "Orb",
            Self::Javelin => "Javelin",
            Self::Trident => "Trident",
            Self::Rapier => "Rapier",
            Self::TigerClaws => "Tiger Claws",
            Self::BlackJack => "Black Jack",
            Self::Cestus => "Cestus",
            Self::KnuckleDuster => "Knuckle-Duster",
            Self::FryingPan => "Frying Pan",
            Self::HolyMackerel => "Holy Mackerel",
            Self::WoodArrow => "Wood Arrow",
            Self::FlintArrow => "Flint Arrow",
            Self::CrossbowBolt => "Crossbow Bolt",
            Self::RevolverAmmo => "Revolver Ammo",
            Self::ShotgunShell => "Shotgun Shell",
            Self::SniperAmmo => "Sniper Ammo",
            Self::BouncyBombAmmo => "Bouncy Bomb Ammo",
            Self::DragonHelm => "Dragon Helm",
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
            Self::IronOre => "IronOre",
            Self::IronIngot => "Iron Ingot",
            Self::Ruby => "Ruby",
            Self::LeatherScraps => "Leather Scraps",
            Self::Berry => "Berry",
            Self::CookedMeat => "Cooked Meat",
            Self::Honey => "Honey",
            Self::Ballista => "Ballista",
            Self::Catapult => "Catapult",
            Self::Trebuchet => "Trebuchet",
            Self::BatteringRam => "Battering Ram",
            Self::Unarmed => "Unarmed",
        }
    }

    pub const fn is_two_handed(&self) -> bool {
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
                | Self::Shotgun
                | Self::SniperRifle
                | Self::BouncyBombLauncher
                | Self::BatteringRam
        )
    }

    pub const fn is_offhand_capable(&self) -> bool {
        !self.is_two_handed()
    }

    pub const fn is_bag(&self) -> bool {
        matches!(
            self,
            Self::SmallPouch
                | Self::LeatherBag
                | Self::TravelersBackpack
                | Self::DragonflightHaversack
        )
    }

    pub const fn is_ammo(&self) -> bool {
        matches!(
            self,
            Self::WoodArrow
                | Self::FlintArrow
                | Self::CrossbowBolt
                | Self::RevolverAmmo
                | Self::ShotgunShell
                | Self::SniperAmmo
                | Self::BouncyBombAmmo
        )
    }

    pub const fn is_armor(&self) -> bool {
        matches!(
            self,
            Self::IronHelmet | Self::DragonHelm | Self::IronChestplate | Self::IronGreaves | Self::IronBoots
        )
    }

    pub const fn equipment_slot(&self) -> Option<EquipmentSlot> {
        match self {
            Self::IronHelmet | Self::DragonHelm => Some(EquipmentSlot::Head),
            Self::IronChestplate => Some(EquipmentSlot::Chest),
            Self::IronGreaves => Some(EquipmentSlot::Legs),
            Self::IronBoots => Some(EquipmentSlot::Feet),
            k if k.is_weapon() || k.is_tool() => Some(EquipmentSlot::MainHand),
            _ => None,
        }
    }

    pub const fn is_tool(&self) -> bool {
        matches!(self, Self::Hammer | Self::Pickaxe | Self::Torch | Self::Handaxe)
    }

    pub const fn is_weapon(&self) -> bool {
        !self.is_armor() && !self.is_bag() && !self.is_ammo() && !self.is_resource()
    }

    pub const fn is_resource(&self) -> bool {
        matches!(
            self,
            Self::Wood
                | Self::ElderWood
                | Self::Branch
                | Self::Stone
                | Self::LooseStone
                | Self::Flint
                | Self::Resin
                | Self::IronOre
                | Self::IronIngot
                | Self::Ruby
                | Self::LeatherScraps
                | Self::Berry
                | Self::CookedMeat
                | Self::Honey
        )
    }
}

// ----------------------------------------------------------------------------
// 3. RESOURCE NODE TYPES & INTERACTION METADATA
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceNodeType {
    Bush,
    Branch,
    Flint,
    LooseStone,
    Tree,
    FallenLog,
    Rock,
    Rubble,
    CollapsedRubble,
    IronOre,
    Ruby,
}

impl ResourceNodeType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Bush => "Bush",
            Self::Branch => "Branch",
            Self::Flint => "Flint",
            Self::LooseStone => "LooseStone",
            Self::Tree => "Tree",
            Self::FallenLog => "FallenLog",
            Self::Rock => "Rock",
            Self::Rubble => "Rubble",
            Self::CollapsedRubble => "CollapsedRubble",
            Self::IronOre => "Ore:Iron",
            Self::Ruby => "Gem:Ruby",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "Bush" => Some(Self::Bush),
            "Branch" => Some(Self::Branch),
            "Flint" => Some(Self::Flint),
            "LooseStone" => Some(Self::LooseStone),
            "Tree" => Some(Self::Tree),
            "FallenLog" => Some(Self::FallenLog),
            "Rock" => Some(Self::Rock),
            "Rubble" => Some(Self::Rubble),
            "CollapsedRubble" => Some(Self::CollapsedRubble),
            "Ore:Iron" | "IronOre" => Some(Self::IronOre),
            "Gem:Ruby" | "Ruby" => Some(Self::Ruby),
            _ => None,
        }
    }

    pub const fn required_tool(&self) -> Option<ToolKind> {
        match self {
            Self::Tree => Some(ToolKind::StoneAxe),
            Self::Rock => Some(ToolKind::Pickaxe),
            Self::IronOre | Self::Ruby => Some(ToolKind::Pickaxe),
            _ => None,
        }
    }

    pub const fn default_drop(&self) -> (ItemKind, u32) {
        match self {
            Self::Bush => (ItemKind::Berry, 2),
            Self::Branch => (ItemKind::Branch, 1),
            Self::Flint => (ItemKind::Flint, 1),
            Self::LooseStone => (ItemKind::LooseStone, 1),
            Self::Tree | Self::FallenLog => (ItemKind::Wood, 6),
            Self::Rock | Self::Rubble => (ItemKind::Stone, 4),
            Self::CollapsedRubble => (ItemKind::LooseStone, 4),
            Self::IronOre => (ItemKind::IronOre, 3),
            Self::Ruby => (ItemKind::Ruby, 1),
        }
    }

    pub fn interaction_prompt(&self, health: u32) -> &'static str {
        match self {
            Self::Bush => {
                if health > 0 { "[E] Pick Berries" } else { "Berries Depleted" }
            }
            Self::Branch => "[E] Pick up Branch",
            Self::Flint => "[E] Pick up Flint",
            Self::LooseStone => "[E] Pick up Stone",
            Self::Tree => "Tree (Left-click with Stone Axe)",
            Self::FallenLog => "[E] Chop Fallen Log",
            Self::Rubble => "[E] Mine Rubble",
            Self::CollapsedRubble => "[E] Mine Collapsed Rubble",
            Self::Rock => "Rock (Left-click with Pickaxe)",
            Self::IronOre => "Iron Vein (Left-click with Pickaxe)",
            Self::Ruby => "Ruby Crystal (Left-click with Pickaxe)",
        }
    }
}

// ----------------------------------------------------------------------------
// 4. TOOL KINDS
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolKind {
    StoneAxe,
    Pickaxe,
    Hammer,
}

impl ToolKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::StoneAxe => "Stone Axe",
            Self::Pickaxe => "Pickaxe",
            Self::Hammer => "Hammer",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "Stone Axe" | "StoneAxe" | "Axe" => Some(Self::StoneAxe),
            "Pickaxe" | "Pick" => Some(Self::Pickaxe),
            "Hammer" | "Construction Hammer" => Some(Self::Hammer),
            _ => None,
        }
    }

    pub fn is_satisfied_by(&self, item: ItemKind) -> bool {
        match self {
            Self::StoneAxe => matches!(item, ItemKind::Handaxe),
            Self::Pickaxe => matches!(item, ItemKind::Pickaxe),
            Self::Hammer => matches!(item, ItemKind::Hammer),
        }
    }
}

// ----------------------------------------------------------------------------
// 5. STRUCTURE PIECE TYPES
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StructurePieceType {
    Foundation,
    Wall,
    Floor,
    Door,
    Ramp,
    Roof,
    Window,
    Workbench,
}

impl StructurePieceType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Foundation => "Foundation",
            Self::Wall => "Wall",
            Self::Floor => "Floor",
            Self::Door => "Door",
            Self::Ramp => "Ramp",
            Self::Roof => "Roof",
            Self::Window => "Window",
            Self::Workbench => "Workbench",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "Foundation" => Some(Self::Foundation),
            "Wall" => Some(Self::Wall),
            "Floor" => Some(Self::Floor),
            "Door" => Some(Self::Door),
            "Ramp" => Some(Self::Ramp),
            "Roof" => Some(Self::Roof),
            "Window" => Some(Self::Window),
            "Workbench" => Some(Self::Workbench),
            _ => None,
        }
    }
}

// ----------------------------------------------------------------------------
// 6. COMBAT EVENT TYPES
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombatEventType {
    HitPlayer,
    HitTree,
    HitRock,
    HitBush,
    HitRubble,
    HitIronOre,
    HitRuby,
    DeflectBedrock,
}

impl CombatEventType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::HitPlayer => "HitPlayer",
            Self::HitTree => "HitTree",
            Self::HitRock => "HitRock",
            Self::HitBush => "HitBush",
            Self::HitRubble => "HitRubble",
            Self::HitIronOre => "HitIronOre",
            Self::HitRuby => "HitRuby",
            Self::DeflectBedrock => "DeflectBedrock",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "HitPlayer" => Some(Self::HitPlayer),
            "HitTree" => Some(Self::HitTree),
            "HitRock" => Some(Self::HitRock),
            "HitBush" => Some(Self::HitBush),
            "HitRubble" => Some(Self::HitRubble),
            "HitIronOre" => Some(Self::HitIronOre),
            "HitRuby" => Some(Self::HitRuby),
            "DeflectBedrock" => Some(Self::DeflectBedrock),
            _ => None,
        }
    }
}

// ----------------------------------------------------------------------------
// 7. UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equipment_slot_conversions_and_predicates() {
        assert_eq!(EquipmentSlot::from_str("MainHand"), Some(EquipmentSlot::MainHand));
        assert_eq!(EquipmentSlot::from_str("main"), Some(EquipmentSlot::MainHand));
        assert_eq!(EquipmentSlot::from_str("OffHand"), Some(EquipmentSlot::OffHand));
        assert_eq!(EquipmentSlot::from_str("off"), Some(EquipmentSlot::OffHand));
        assert_eq!(EquipmentSlot::from_str("head"), Some(EquipmentSlot::Head));
        assert_eq!(EquipmentSlot::from_str("chestplate"), Some(EquipmentSlot::Chest));

        assert!(EquipmentSlot::MainHand.is_hand());
        assert!(EquipmentSlot::OffHand.is_hand());
        assert!(!EquipmentSlot::Head.is_hand());
        assert!(EquipmentSlot::Head.is_armor());
        assert!(EquipmentSlot::Chest.is_armor());
        assert_eq!(EquipmentSlot::MainHand.as_str(), "MainHand");
    }

    #[test]
    fn test_item_kind_conversions_and_predicates() {
        let bow = ItemKind::from_name("Crude Bow").unwrap();
        assert_eq!(bow, ItemKind::Bow);
        assert!(bow.is_two_handed());
        assert!(!bow.is_offhand_capable());
        assert!(bow.is_weapon());
        assert!(!bow.is_armor());
        assert_eq!(bow.as_str(), "Bow");

        let longsword = ItemKind::from_name("Knight's Longsword").unwrap();
        assert_eq!(longsword, ItemKind::Longsword);
        assert!(!longsword.is_two_handed());
        assert!(longsword.is_offhand_capable());

        let bag = ItemKind::from_name("Leather Bag").unwrap();
        assert!(bag.is_bag());

        let arrow = ItemKind::from_name("Flint Arrow").unwrap();
        assert!(arrow.is_ammo());

        let hammer = ItemKind::from_name("Hammer").unwrap();
        assert!(hammer.is_tool());

        let wood = ItemKind::from_name("Wood").unwrap();
        assert!(wood.is_resource());

        let dragon_helm = ItemKind::from_name("Dragon Helm").unwrap();
        assert_eq!(dragon_helm, ItemKind::DragonHelm);
        assert!(dragon_helm.is_armor());
        assert_eq!(dragon_helm.equipment_slot(), Some(EquipmentSlot::Head));
        assert_eq!(dragon_helm.as_str(), "Dragon Helm");

        let unarmed = ItemKind::from_name("None").unwrap();
        assert_eq!(unarmed, ItemKind::Unarmed);
    }

    #[test]
    fn test_resource_node_type_tool_and_prompts() {
        let tree = ResourceNodeType::from_str("Tree").unwrap();
        assert_eq!(tree.required_tool(), Some(ToolKind::StoneAxe));
        assert_eq!(tree.default_drop(), (ItemKind::Wood, 6));

        let bush = ResourceNodeType::from_str("Bush").unwrap();
        assert_eq!(bush.required_tool(), None);
        assert_eq!(bush.default_drop(), (ItemKind::Berry, 2));
        assert_eq!(bush.interaction_prompt(1), "[E] Pick Berries");
        assert_eq!(bush.interaction_prompt(0), "Berries Depleted");

        let rock = ResourceNodeType::from_str("Rock").unwrap();
        assert_eq!(rock.required_tool(), Some(ToolKind::Pickaxe));

        let ore = ResourceNodeType::from_str("Ore:Iron").unwrap();
        assert_eq!(ore, ResourceNodeType::IronOre);
    }

    #[test]
    fn test_structure_piece_type_parsing() {
        assert_eq!(StructurePieceType::from_str("Foundation"), Some(StructurePieceType::Foundation));
        assert_eq!(StructurePieceType::from_str("Door"), Some(StructurePieceType::Door));
        assert_eq!(StructurePieceType::from_str("Workbench"), Some(StructurePieceType::Workbench));
        assert_eq!(StructurePieceType::Workbench.as_str(), "Workbench");
    }
}
