// ============================================================================
// File: client/src/building/mod.rs
// ============================================================================
// ----------------------------------------------------------------------------
// MODULAR BUILDING & LOW-POLY PROCEDURAL ARCHITECTURE ENGINE
// ----------------------------------------------------------------------------
// Re-exports modular piece definitions, procedural mesh builders, door kinematics,
// ghost preview holograms, SpacetimeDB structure synchronization, and structural collapse.

pub mod types;
pub mod meshes;
pub mod doors;
pub mod preview;
pub mod sync;
pub mod destruction;

pub use types::*;
#[allow(unused_imports)]
pub use meshes::*;
pub use doors::*;
pub use preview::*;
pub use sync::*;
pub use destruction::*;

// ----------------------------------------------------------------------------
// UNIT TESTS
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn test_modular_piece_properties_and_names() {
        assert_eq!(ModularPieceType::Foundation.name(), "Foundation");
        assert_eq!(ModularPieceType::Wall.name(), "Wall");
        assert_eq!(ModularPieceType::Window.name(), "Window");
        assert_eq!(ModularPieceType::Door.name(), "Door");
        assert_eq!(ModularPieceType::Workbench.name(), "Workbench");
        assert_eq!(ModularPieceType::Campfire.name(), "Campfire");
        assert_eq!(ModularPieceType::Floor.name(), "Floor");
        assert_eq!(ModularPieceType::Roof.name(), "Roof");
        assert_eq!(ModularPieceType::Ramp.name(), "Ramp");

        assert!(ModularPieceType::Foundation.wood_cost() > 0);
        assert!(ModularPieceType::Wall.wood_cost() > 0);
    }

    #[test]
    fn test_socket_compatibility_rules() {
        assert!(is_socket_compatible(ModularPieceType::Wall, "North"));
        assert!(is_socket_compatible(ModularPieceType::Wall, "WallTop"));
        assert!(!is_socket_compatible(ModularPieceType::Wall, "FoundationNorth"));

        assert!(is_socket_compatible(ModularPieceType::Foundation, "FoundationNorth"));
        assert!(!is_socket_compatible(ModularPieceType::Foundation, "WallTop"));

        assert!(is_socket_compatible(ModularPieceType::Floor, "Top"));
        assert!(is_socket_compatible(ModularPieceType::Ramp, "North"));
    }

    #[test]
    fn test_voxel_box_mesh_builder() {
        let b = BuildingBox {
            min: Vec3::new(-1.0, -1.0, -1.0),
            max: Vec3::new(1.0, 1.0, 1.0),
            color: [1.0, 0.5, 0.2, 1.0],
        };
        let mesh = build_building_mesh(&[b]);

        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert_eq!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len(), 24);
    }

    #[test]
    fn test_ramp_mesh_generation() {
        let mesh = create_ramp_mesh();
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
    }

    #[test]
    fn test_base_piece_name_resolution() {
        assert_eq!(base_piece_name("Human_Wall"), "Wall");
        assert_eq!(base_piece_name("Elf:Door"), "Door");
        assert_eq!(base_piece_name("Foundation"), "Foundation");
    }
}
