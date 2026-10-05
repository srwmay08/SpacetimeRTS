// ----------------------------------------------------------------------------
// NPC BUILDING TEMPLATE TESTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Validates pure template resolution, stability propagation across multi-room
// buildings, quarter-turn coordinate transforms, and grid snapping.

use backend::npc_building::{
    get_building_template, plan_building, rotate_offset, snap_to_grid,
    template_names, TemplatePiece, FOUNDATION_LIFT, GRID,
};

#[test]
fn test_template_registry_coverage() {
    let names = template_names();
    assert!(names.contains(&"hut"));
    assert!(names.contains(&"cottage"));
    assert!(names.contains(&"guardpost"));

    for name in names {
        let t = get_building_template(name);
        assert!(t.is_some(), "template '{}' must resolve", name);
        assert!(!t.unwrap().is_empty(), "template '{}' must have pieces", name);
    }
}

#[test]
fn test_rotate_offset_all_quadrants() {
    let (dx, dz) = (2.0, -1.0);

    // 0 quarter turns: identity
    assert_eq!(rotate_offset(dx, dz, 0), (2.0, -1.0));
    // 1 quarter turn (+90 deg about +Y): (z, -x) = (-1.0, -2.0)
    assert_eq!(rotate_offset(dx, dz, 1), (-1.0, -2.0));
    // 2 quarter turns (180 deg): (-x, -z) = (-2.0, 1.0)
    assert_eq!(rotate_offset(dx, dz, 2), (-2.0, 1.0));
    // 3 quarter turns (270 deg): (-z, x) = (1.0, 2.0)
    assert_eq!(rotate_offset(dx, dz, 3), (1.0, 2.0));
    // modulo wraps cleanly
    assert_eq!(rotate_offset(dx, dz, 4), (2.0, -1.0));
    assert_eq!(rotate_offset(dx, dz, 5), (-1.0, -2.0));
}

#[test]
fn test_snap_to_grid_aligns_to_foundation_size() {
    assert_eq!(snap_to_grid(0.0), 0.0);
    assert_eq!(snap_to_grid(1.9), 0.0);
    assert_eq!(snap_to_grid(2.1), GRID);
    assert_eq!(snap_to_grid(4.0), GRID);
    assert_eq!(snap_to_grid(-3.9), -GRID);
}

#[test]
fn test_plan_hut_template_structure_and_stability() {
    let hut = get_building_template("hut").expect("hut template");
    let flat_ground = |_x, _z| 10.0;

    let planned = plan_building(hut, 0.0, 0.0, 0, flat_ground).expect("planning hut succeeds");
    assert_eq!(planned.len(), 6);

    // Foundation
    let f = &planned[0];
    assert_eq!(f.piece_type, "Foundation");
    assert_eq!(f.parent, None);
    assert!(f.is_grounded);
    assert_eq!(f.stability, 100);
    assert!((f.y - (10.0 + FOUNDATION_LIFT)).abs() < 1.0e-5);

    // Wall-family pieces: Wall, Wall, Window, Door
    // Decay for all three is 20, so stability is 100 - 20 = 80.
    for p in &planned[1..5] {
        assert_eq!(p.parent, Some(0));
        assert!(!p.is_grounded);
        assert_eq!(p.stability, 80);
    }
    assert_eq!(planned[3].piece_type, "Window");
    assert_eq!(planned[4].piece_type, "Door");

    // Roof anchored to North wall (decay 30): 80 - 30 = 50.
    let roof = &planned[5];
    assert_eq!(roof.piece_type, "Roof");
    assert_eq!(roof.parent, Some(1));
    assert_eq!(roof.stability, 50);
}

#[test]
fn test_plan_cottage_multi_room_stability() {
    let cottage = get_building_template("cottage").expect("cottage template");
    let flat_ground = |_x, _z| 0.0;

    let planned = plan_building(cottage, 20.0, 40.0, 1, flat_ground).expect("cottage planning");
    assert_eq!(planned.len(), 10);

    // Two foundations, both grounded with 100 stability
    assert_eq!(planned[0].piece_type, "Foundation");
    assert_eq!(planned[1].piece_type, "Foundation");
    assert!(planned[0].is_grounded);
    assert!(planned[1].is_grounded);
    assert_eq!(planned[0].stability, 100);
    assert_eq!(planned[1].stability, 100);

    // Cottage contains both windows and a door
    let has_window = planned.iter().any(|p| p.piece_type == "Window");
    let has_door = planned.iter().any(|p| p.piece_type == "Door");
    assert!(has_window, "Cottage must include windows");
    assert!(has_door, "Cottage must include a door");

    // All pieces must maintain positive stability
    for p in &planned {
        assert!(p.stability > 0, "Piece {} has depleted stability", p.piece_type);
    }
}

#[test]
fn test_plan_rejects_floating_foundation_or_invalid_parent() {
    let flat_ground = |_x, _z| 0.0;

    // Non-foundation without a parent
    let bad_template = [TemplatePiece {
        piece_type: "Wall",
        parent: None,
        dx: 0.0, dy: 0.0, dz: 0.0, yaw_steps: 0,
    }];
    assert!(plan_building(&bad_template, 0.0, 0.0, 0, flat_ground).is_err());

    // Parent that doesn't precede the piece
    let forward_parent = [
        TemplatePiece { piece_type: "Foundation", parent: None, dx: 0.0, dy: 0.0, dz: 0.0, yaw_steps: 0 },
        TemplatePiece { piece_type: "Wall", parent: Some(2), dx: 0.0, dy: 0.0, dz: 0.0, yaw_steps: 0 },
        TemplatePiece { piece_type: "Wall", parent: Some(0), dx: 0.0, dy: 0.0, dz: 0.0, yaw_steps: 0 },
    ];
    assert!(plan_building(&forward_parent, 0.0, 0.0, 0, flat_ground).is_err());
}
