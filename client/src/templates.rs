// ----------------------------------------------------------------------------
// BUILDING TEMPLATES & FACTION BLUEPRINT STYLES (Bevy Client)
// ----------------------------------------------------------------------------
// Architectural Note: Re-exports canonical building templates and types from
// spacetime-rts-logic. Exposes bidirectional conversions between client Bevy types
// (BuildingFaction, ModularPieceType) and canonical game logic types (Faction, BuildingPiece).

pub use spacetime_rts_logic::templates::*;

use crate::components::BuildingFaction;
use crate::building::ModularPieceType;

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
