// ----------------------------------------------------------------------------
// BUILDING TEMPLATES & FACTION BLUEPRINT STYLES (Bevy Client)
// ----------------------------------------------------------------------------
// Architectural Note: Provides deterministic multi-story building templates
// (watchtowers, palisades, cottages, settlements) for client-side build mode previews,
// blueprint placement dispatch, and RTS commander orders.

use std::collections::BTreeMap;
use crate::components::BuildingFaction;
use crate::building::ModularPieceType;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash)]
pub enum Faction {
    #[default]
    Human,
    HighElf,
    DarkElf,
    Barbarian,
}

impl From<BuildingFaction> for Faction {
    fn from(bf: BuildingFaction) -> Self {
        match bf {
            BuildingFaction::Human => Faction::Human,
            BuildingFaction::HighElf => Faction::HighElf,
            BuildingFaction::DarkElf => Faction::DarkElf,
            BuildingFaction::Barbarian => Faction::Barbarian,
        }
    }
}

impl From<Faction> for BuildingFaction {
    fn from(f: Faction) -> Self {
        match f {
            Faction::Human => BuildingFaction::Human,
            Faction::HighElf => BuildingFaction::HighElf,
            Faction::DarkElf => BuildingFaction::DarkElf,
            Faction::Barbarian => BuildingFaction::Barbarian,
        }
    }
}

/// 3D integer grid offset for modular architectural pieces.
/// In SpacetimeRTS, standard grid pitch is 4.0m on X/Z and 3.0m per vertical storey.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct GridOffset(pub i32, pub i32, pub i32);

impl GridOffset {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self(x, y, z)
    }

    /// Converts integer grid coordinates into world-space meters relative to origin.
    pub fn to_world_pos(&self, origin_x: f32, origin_y: f32, origin_z: f32) -> (f32, f32, f32) {
        let wx = origin_x + (self.0 as f32) * 4.0;
        let wy = origin_y + (self.1 as f32) * 3.0;
        let wz = origin_z + (self.2 as f32) * 4.0;
        (wx, wy, wz)
    }
}

/// Modular building piece type in blueprint templates.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum BuildingPiece {
    Foundation,
    SolidWall,
    WindowWall,
    Doorway,
    Floor,
    Roof,
    Ramp,
}

impl BuildingPiece {
    pub fn to_piece_name(&self, faction: Faction) -> &'static str {
        match (faction, self) {
            (Faction::Human, BuildingPiece::Foundation) => "Foundation",
            (Faction::Human, BuildingPiece::SolidWall) => "Wall",
            (Faction::Human, BuildingPiece::WindowWall) => "Window",
            (Faction::Human, BuildingPiece::Doorway) => "Door",
            (Faction::Human, BuildingPiece::Floor) => "Floor",
            (Faction::Human, BuildingPiece::Roof) => "Roof",
            (Faction::Human, BuildingPiece::Ramp) => "Ramp",

            (Faction::HighElf, BuildingPiece::Foundation) => "HighElf_Foundation",
            (Faction::HighElf, BuildingPiece::SolidWall) => "HighElf_Wall",
            (Faction::HighElf, BuildingPiece::WindowWall) => "HighElf_Window",
            (Faction::HighElf, BuildingPiece::Doorway) => "HighElf_Door",
            (Faction::HighElf, BuildingPiece::Floor) => "HighElf_Floor",
            (Faction::HighElf, BuildingPiece::Roof) => "HighElf_Roof",
            (Faction::HighElf, BuildingPiece::Ramp) => "HighElf_Ramp",

            (Faction::DarkElf, BuildingPiece::Foundation) => "DarkElf_Foundation",
            (Faction::DarkElf, BuildingPiece::SolidWall) => "DarkElf_Wall",
            (Faction::DarkElf, BuildingPiece::WindowWall) => "DarkElf_Window",
            (Faction::DarkElf, BuildingPiece::Doorway) => "DarkElf_Door",
            (Faction::DarkElf, BuildingPiece::Floor) => "DarkElf_Floor",
            (Faction::DarkElf, BuildingPiece::Roof) => "DarkElf_Roof",
            (Faction::DarkElf, BuildingPiece::Ramp) => "DarkElf_Ramp",

            (Faction::Barbarian, BuildingPiece::Foundation) => "Barbarian_Foundation",
            (Faction::Barbarian, BuildingPiece::SolidWall) => "Barbarian_Wall",
            (Faction::Barbarian, BuildingPiece::WindowWall) => "Barbarian_Window",
            (Faction::Barbarian, BuildingPiece::Doorway) => "Barbarian_Door",
            (Faction::Barbarian, BuildingPiece::Floor) => "Barbarian_Floor",
            (Faction::Barbarian, BuildingPiece::Roof) => "Barbarian_Roof",
            (Faction::Barbarian, BuildingPiece::Ramp) => "Barbarian_Ramp",
        }
    }
}

/// A multi-story template definition composed of grid offsets mapped to modular pieces.
/// Uses BTreeMap for strict deterministic ordering across all server and client platforms.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingTemplate {
    pub blocks: BTreeMap<GridOffset, BuildingPiece>,
    pub faction: Faction,
}

impl BuildingTemplate {
    pub fn new(faction: Faction) -> Self {
        Self {
            blocks: BTreeMap::new(),
            faction,
        }
    }

    /// Multi-story watchtower with 2x2 foundation, ground-floor doorway and walls,
    /// and second-story sniper perches / window walls.
    pub fn watchtower(faction: Faction) -> Self {
        let mut blocks = BTreeMap::new();

        // Base Foundation 2x2
        for x in 0..2 {
            for z in 0..2 {
                blocks.insert(GridOffset(x, 0, z), BuildingPiece::Foundation);
            }
        }

        // Ground Floor Walls with a Doorway
        blocks.insert(GridOffset(0, 1, 0), BuildingPiece::Doorway);
        blocks.insert(GridOffset(1, 1, 0), BuildingPiece::SolidWall);
        blocks.insert(GridOffset(0, 1, 1), BuildingPiece::SolidWall);
        blocks.insert(GridOffset(1, 1, 1), BuildingPiece::WindowWall);

        // Second Story (Sniper perches)
        for x in 0..2 {
            for z in 0..2 {
                blocks.insert(GridOffset(x, 2, z), BuildingPiece::WindowWall);
            }
        }

        // Upper Roof Battlements
        for x in 0..2 {
            for z in 0..2 {
                blocks.insert(GridOffset(x, 3, z), BuildingPiece::Roof);
            }
        }

        Self { blocks, faction }
    }

    /// Palisade wall segment with a central gateway and reinforced battlements.
    pub fn palisade(faction: Faction) -> Self {
        let mut blocks = BTreeMap::new();

        // 4x1 foundation run
        for x in 0..4 {
            blocks.insert(GridOffset(x, 0, 0), BuildingPiece::Foundation);
        }

        // Central doorway flanked by solid walls
        blocks.insert(GridOffset(0, 1, 0), BuildingPiece::SolidWall);
        blocks.insert(GridOffset(1, 1, 0), BuildingPiece::Doorway);
        blocks.insert(GridOffset(2, 1, 0), BuildingPiece::SolidWall);
        blocks.insert(GridOffset(3, 1, 0), BuildingPiece::SolidWall);

        // Battlement walkways above
        for x in 0..4 {
            blocks.insert(GridOffset(x, 2, 0), BuildingPiece::WindowWall);
        }

        Self { blocks, faction }
    }

    /// Two-room residential cottage with doorway and window openings.
    pub fn cottage(faction: Faction) -> Self {
        let mut blocks = BTreeMap::new();

        // 2x1 foundation
        blocks.insert(GridOffset(0, 0, 0), BuildingPiece::Foundation);
        blocks.insert(GridOffset(1, 0, 0), BuildingPiece::Foundation);

        // Ground walls, doorway, and windows
        blocks.insert(GridOffset(0, 1, 0), BuildingPiece::Doorway);
        blocks.insert(GridOffset(1, 1, 0), BuildingPiece::SolidWall);
        blocks.insert(GridOffset(0, 1, 1), BuildingPiece::WindowWall);
        blocks.insert(GridOffset(1, 1, 1), BuildingPiece::WindowWall);

        // Roof covers
        blocks.insert(GridOffset(0, 2, 0), BuildingPiece::Roof);
        blocks.insert(GridOffset(1, 2, 0), BuildingPiece::Roof);

        Self { blocks, faction }
    }

    /// Stamped settlement outpost consisting of a watchtower, palisades, and cottage.
    pub fn settlement(faction: Faction) -> Self {
        let mut blocks = BTreeMap::new();
        let tower = Self::watchtower(faction);
        for (offset, piece) in tower.blocks {
            blocks.insert(offset, piece);
        }
        let wall = Self::palisade(faction);
        for (offset, piece) in wall.blocks {
            blocks.insert(GridOffset(offset.0 + 2, offset.1, offset.2), piece);
        }
        Self { blocks, faction }
    }
}

impl From<BuildingPiece> for ModularPieceType {
    fn from(piece: BuildingPiece) -> Self {
        match piece {
            BuildingPiece::Foundation => ModularPieceType::Foundation,
            BuildingPiece::SolidWall => ModularPieceType::Wall,
            BuildingPiece::WindowWall => ModularPieceType::Window,
            BuildingPiece::Doorway => ModularPieceType::Door,
            BuildingPiece::Floor => ModularPieceType::Floor,
            BuildingPiece::Roof => ModularPieceType::Roof,
            BuildingPiece::Ramp => ModularPieceType::Ramp,
        }
    }
}

/// Identifiers for multi-story blueprint templates supported in build mode and RTS construction.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default)]
pub enum BuildingTemplateType {
    #[default]
    Watchtower,
    Palisade,
    Cottage,
    Settlement,
}

impl BuildingTemplateType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Watchtower => "Watchtower (2x2 Multi-story)",
            Self::Palisade => "Palisade Gate & Wall",
            Self::Cottage => "Residential Cottage",
            Self::Settlement => "Settlement Outpost",
        }
    }

    pub fn to_api_name(&self) -> &'static str {
        match self {
            Self::Watchtower => "watchtower",
            Self::Palisade => "palisade",
            Self::Cottage => "cottage",
            Self::Settlement => "settlement",
        }
    }

    pub fn wood_cost(&self) -> u32 {
        match self {
            Self::Watchtower => 120,
            Self::Palisade => 90,
            Self::Cottage => 70,
            Self::Settlement => 260,
        }
    }

    pub fn to_template(&self, faction: Faction) -> BuildingTemplate {
        match self {
            Self::Watchtower => BuildingTemplate::watchtower(faction),
            Self::Palisade => BuildingTemplate::palisade(faction),
            Self::Cottage => BuildingTemplate::cottage(faction),
            Self::Settlement => BuildingTemplate::settlement(faction),
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Watchtower => Self::Palisade,
            Self::Palisade => Self::Cottage,
            Self::Cottage => Self::Settlement,
            Self::Settlement => Self::Watchtower,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_building_piece_to_modular_piece_type() {
        assert_eq!(ModularPieceType::from(BuildingPiece::Foundation), ModularPieceType::Foundation);
        assert_eq!(ModularPieceType::from(BuildingPiece::SolidWall), ModularPieceType::Wall);
        assert_eq!(ModularPieceType::from(BuildingPiece::WindowWall), ModularPieceType::Window);
        assert_eq!(ModularPieceType::from(BuildingPiece::Doorway), ModularPieceType::Door);
        assert_eq!(ModularPieceType::from(BuildingPiece::Floor), ModularPieceType::Floor);
        assert_eq!(ModularPieceType::from(BuildingPiece::Roof), ModularPieceType::Roof);
        assert_eq!(ModularPieceType::from(BuildingPiece::Ramp), ModularPieceType::Ramp);
    }

    #[test]
    fn test_building_template_type_cycle_and_metadata() {
        let t = BuildingTemplateType::Watchtower;
        assert_eq!(t.to_api_name(), "watchtower");
        assert_eq!(t.wood_cost(), 120);

        let t2 = t.next();
        assert_eq!(t2, BuildingTemplateType::Palisade);
        assert_eq!(t2.to_api_name(), "palisade");
        assert_eq!(t2.wood_cost(), 90);

        let t3 = t2.next();
        assert_eq!(t3, BuildingTemplateType::Cottage);
        assert_eq!(t3.to_api_name(), "cottage");
        assert_eq!(t3.wood_cost(), 70);

        let t4 = t3.next();
        assert_eq!(t4, BuildingTemplateType::Settlement);
        assert_eq!(t4.to_api_name(), "settlement");
        assert_eq!(t4.wood_cost(), 260);

        let t5 = t4.next();
        assert_eq!(t5, BuildingTemplateType::Watchtower);
    }

    #[test]
    fn test_template_block_counts_and_offsets() {
        let tower = BuildingTemplate::watchtower(Faction::HighElf);
        // Base 2x2 = 4, ground walls = 4, second floor = 4, roof = 4 -> 16 blocks total
        assert_eq!(tower.blocks.len(), 16);
        assert_eq!(tower.faction, Faction::HighElf);
        assert_eq!(tower.blocks.get(&GridOffset(0, 1, 0)), Some(&BuildingPiece::Doorway));
        assert_eq!(tower.blocks.get(&GridOffset(1, 1, 1)), Some(&BuildingPiece::WindowWall));

        let palisade = BuildingTemplate::palisade(Faction::Human);
        // Base 4x1 = 4, walls = 4, battlement = 4 -> 12 blocks total
        assert_eq!(palisade.blocks.len(), 12);
        assert_eq!(palisade.blocks.get(&GridOffset(1, 1, 0)), Some(&BuildingPiece::Doorway));

        let cottage = BuildingTemplate::cottage(Faction::DarkElf);
        // Base 2x1 = 2, walls = 4, roof = 2 -> 8 blocks total
        assert_eq!(cottage.blocks.len(), 8);
        assert_eq!(cottage.blocks.get(&GridOffset(0, 1, 0)), Some(&BuildingPiece::Doorway));

        let settlement = BuildingTemplate::settlement(Faction::Barbarian);
        // Tower (16) + Palisade (12) offset = 28 blocks
        assert_eq!(settlement.blocks.len(), 28);
    }

    #[test]
    fn test_faction_bidirectional_conversion() {
        assert_eq!(Faction::from(BuildingFaction::Human), Faction::Human);
        assert_eq!(Faction::from(BuildingFaction::HighElf), Faction::HighElf);
        assert_eq!(Faction::from(BuildingFaction::DarkElf), Faction::DarkElf);
        assert_eq!(Faction::from(BuildingFaction::Barbarian), Faction::Barbarian);

        assert_eq!(BuildingFaction::from(Faction::Human), BuildingFaction::Human);
        assert_eq!(BuildingFaction::from(Faction::HighElf), BuildingFaction::HighElf);
        assert_eq!(BuildingFaction::from(Faction::DarkElf), BuildingFaction::DarkElf);
        assert_eq!(BuildingFaction::from(Faction::Barbarian), BuildingFaction::Barbarian);
    }

    #[test]
    fn test_grid_offset_world_conversion() {
        let offset = GridOffset(2, 3, -1);
        let (wx, wy, wz) = offset.to_world_pos(10.0, 0.0, 20.0);
        assert_eq!(wx, 10.0 + 8.0);
        assert_eq!(wy, 0.0 + 9.0);
        assert_eq!(wz, 20.0 - 4.0);
    }
}

