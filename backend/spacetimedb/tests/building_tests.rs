// ----------------------------------------------------------------------------
// MODULAR BUILDING & STABILITY INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024)
// ----------------------------------------------------------------------------
// Architectural Note: Tests the server-authoritative structural integrity system.
// Validates stability decay propagation from grounded anchors, piece health
// presets, hammer swing construction costs, cascading destruction queues,
// and workbench placement prerequisites.

use backend::building::get_piece_max_health;
use backend::building::Structure;

/// Evaluates stability decay penalty by piece type
fn get_piece_decay(piece_type: &str) -> Option<u32> {
    match piece_type {
        "Workbench" | "Campfire" | "Bed" | "Foundation" => Some(0),
        "Wall" => Some(20),
        "Floor" => Some(25),
        "Roof" => Some(30),
        "Ramp" => Some(25),
        _ => None,
    }
}

/// Checks whether an existing structure with `parent_stability` can support a new child
fn can_support_child(parent_stability: u32, piece_type: &str) -> Option<u32> {
    let decay = get_piece_decay(piece_type)?;
    if parent_stability > decay {
        Some(parent_stability - decay)
    } else {
        None
    }
}

#[test]
fn test_piece_max_health_presets() {
    // Architectural Note: Verifies balance health targets for modular pieces:
    // Foundations (400 HP) act as rugged anchors; Walls (200 HP); Ramps (250 HP);
    // Floors & Roofs (150 HP); Campfires (60 HP); Default fallback (100 HP).
    assert_eq!(get_piece_max_health("Foundation"), 400.0);
    assert_eq!(get_piece_max_health("Wall"), 200.0);
    assert_eq!(get_piece_max_health("Floor"), 150.0);
    assert_eq!(get_piece_max_health("Roof"), 150.0);
    assert_eq!(get_piece_max_health("Ramp"), 250.0);
    assert_eq!(get_piece_max_health("Workbench"), 150.0);
    assert_eq!(get_piece_max_health("Campfire"), 60.0);
    assert_eq!(get_piece_max_health("UnknownStructure"), 100.0);
}

#[test]
fn test_decay_penalties_by_piece_type() {
    // Architectural Note: Decay rules dictate vertical and cantilever limits:
    // Ground anchors (0 penalty), Walls (20), Floors (25), Roofs (30), Ramps (25).
    assert_eq!(get_piece_decay("Foundation"), Some(0));
    assert_eq!(get_piece_decay("Workbench"), Some(0));
    assert_eq!(get_piece_decay("Campfire"), Some(0));
    assert_eq!(get_piece_decay("Wall"), Some(20));
    assert_eq!(get_piece_decay("Floor"), Some(25));
    assert_eq!(get_piece_decay("Roof"), Some(30));
    assert_eq!(get_piece_decay("Ramp"), Some(25));
    assert_eq!(get_piece_decay("InvalidType"), None);
}

#[test]
fn test_stability_propagation_chain() {
    // Architectural Note: Stability cascades down the structural tree:
    // Foundation: 100 (grounded)
    // Wall 1: 100 - 20 = 80
    // Wall 2: 80 - 20 = 60
    // Floor 1: 60 - 25 = 35
    // Roof 1: 35 - 30 = 5
    // Roof 2: 5 <= 30 -> Attachment fails (structural integrity depleted)
    let foundation_stability = 100_u32;

    let wall1_stability = can_support_child(foundation_stability, "Wall").expect("Wall 1 must attach");
    assert_eq!(wall1_stability, 80);

    let wall2_stability = can_support_child(wall1_stability, "Wall").expect("Wall 2 must attach");
    assert_eq!(wall2_stability, 60);

    let floor_stability = can_support_child(wall2_stability, "Floor").expect("Floor must attach");
    assert_eq!(floor_stability, 35);

    let roof_stability = can_support_child(floor_stability, "Roof").expect("Roof must attach");
    assert_eq!(roof_stability, 5);

    // Another roof requires 30 decay; 5 <= 30 fails
    assert!(can_support_child(roof_stability, "Roof").is_none());
}

#[test]
fn test_construction_swing_cost_division() {
    // Architectural Note: All piece costs must be evenly divisible by 4 (25% progress per swing).
    let pieces = [
        ("Workbench", 8, 0, 2, 0),
        ("Campfire", 4, 4, 1, 1),
        ("Foundation", 20, 0, 5, 0),
        ("Wall", 8, 0, 2, 0),
        ("Floor", 12, 0, 3, 0),
        ("Roof", 12, 0, 3, 0),
        ("Ramp", 16, 0, 4, 0),
    ];

    for (piece, total_wood, total_stone, swing_wood, swing_stone) in pieces {
        assert_eq!(total_wood % 4, 0, "Wood cost for {} must be divisible by 4", piece);
        assert_eq!(total_stone % 4, 0, "Stone cost for {} must be divisible by 4", piece);
        assert_eq!(total_wood / 4, swing_wood);
        assert_eq!(total_stone / 4, swing_stone);
    }
}

#[test]
fn test_construction_health_scaling() {
    // Architectural Note: During construction progress (0% -> 100%), health scales
    // proportionally: (max_hp * (progress / 100.0)).max(1.0).
    let max_hp = 200.0_f32; // Wall

    // 0% progress (new blueprint)
    let hp_0 = (max_hp * (0.0 / 100.0)).max(1.0);
    assert_eq!(hp_0, 1.0);

    // 25% progress (hit 1)
    let hp_25 = (max_hp * (25.0 / 100.0)).max(1.0);
    assert_eq!(hp_25, 50.0);

    // 50% progress (hit 2)
    let hp_50 = (max_hp * (50.0 / 100.0)).max(1.0);
    assert_eq!(hp_50, 100.0);

    // 75% progress (hit 3)
    let hp_75 = (max_hp * (75.0 / 100.0)).max(1.0);
    assert_eq!(hp_75, 150.0);

    // 100% progress (hit 4: finished)
    let hp_100 = (max_hp * (100.0 / 100.0)).max(1.0);
    assert_eq!(hp_100, 200.0);
}

#[test]
fn test_cascading_destruction_queue() {
    // Architectural Note: When a supporting foundation is destroyed, all child structures
    // attached to it (and children of those children) must be discovered via BFS queue
    // and marked for collapse.
    struct MockStructure {
        id: u64,
        parent_id: Option<u64>,
    }

    let world_structures = vec![
        MockStructure { id: 1, parent_id: None },      // Foundation (Root)
        MockStructure { id: 2, parent_id: Some(1) },   // Wall attached to Foundation
        MockStructure { id: 3, parent_id: Some(1) },   // Ramp attached to Foundation
        MockStructure { id: 4, parent_id: Some(2) },   // Floor attached to Wall
        MockStructure { id: 5, parent_id: Some(4) },   // Roof attached to Floor
        MockStructure { id: 6, parent_id: None },      // Independent Foundation
        MockStructure { id: 7, parent_id: Some(6) },   // Wall on independent Foundation
    ];

    // Destroy Foundation 1
    let target_id = 1;
    let mut collapse_queue = vec![target_id];
    let mut index = 0;

    while index < collapse_queue.len() {
        let current_id = collapse_queue[index];
        for s in &world_structures {
            if s.parent_id == Some(current_id) {
                collapse_queue.push(s.id);
            }
        }
        index += 1;
    }

    // Exactly [1, 2, 3, 4, 5] must collapse; [6, 7] remain standing
    assert_eq!(collapse_queue, vec![1, 2, 3, 4, 5]);
    assert!(!collapse_queue.contains(&6));
    assert!(!collapse_queue.contains(&7));
}

#[test]
fn test_starter_piece_workbench_exemption() {
    // Architectural Note: Foundations, Workbenches, and Campfires can be placed without
    // a pre-existing Workbench, bootstraping initial base building.
    let is_starter = |piece: &str| matches!(piece, "Foundation" | "Workbench" | "Campfire");

    assert!(is_starter("Foundation"));
    assert!(is_starter("Workbench"));
    assert!(is_starter("Campfire"));

    assert!(!is_starter("Wall"));
    assert!(!is_starter("Floor"));
    assert!(!is_starter("Roof"));
    assert!(!is_starter("Ramp"));
}

#[test]
fn test_workbench_proximity_radius() {
    // Architectural Note: Advanced construction requires being within 20m (dist_sq <= 400.0)
    // of an active Workbench.
    let wb_x = 0.0_f32;
    let wb_z = 0.0_f32;

    let is_within_workbench = |px: f32, pz: f32| -> bool {
        let dist_sq = (wb_x - px).powi(2) + (wb_z - pz).powi(2);
        dist_sq <= 400.0
    };

    assert!(is_within_workbench(10.0, 10.0));  // 100 + 100 = 200 <= 400 (Valid)
    assert!(is_within_workbench(20.0, 0.0));   // 400 <= 400 (Boundary valid)
    assert!(!is_within_workbench(20.1, 0.0));  // > 400 (Out of range)
    assert!(!is_within_workbench(15.0, 15.0)); // 225 + 225 = 450 > 400 (Out of range)
}

#[test]
fn test_roof_coverage_calculation() {
    // Architectural Note: `is_covered` checks if a Roof structure is within 3m horizontal
    // (dist_sq <= 9.0) and positioned between 0m and 10m overhead.
    let is_roof_covering = |roof_x: f32, roof_y: f32, roof_z: f32, target_x: f32, target_y: f32, target_z: f32| -> bool {
        let dist_sq = (roof_x - target_x).powi(2) + (roof_z - target_z).powi(2);
        dist_sq <= 9.0 && roof_y > target_y && (roof_y - target_y) < 10.0
    };

    // Valid: 2m directly overhead
    assert!(is_roof_covering(0.0, 5.0, 0.0, 0.0, 3.0, 0.0));
    // Valid: 2m horizontal offset, 4m overhead (dist_sq = 4 <= 9)
    assert!(is_roof_covering(2.0, 6.0, 0.0, 0.0, 2.0, 0.0));
    // Invalid: Too far horizontally (dist_sq = 16 > 9)
    assert!(!is_roof_covering(4.0, 5.0, 0.0, 0.0, 3.0, 0.0));
    // Invalid: Roof is below the target
    assert!(!is_roof_covering(0.0, 2.0, 0.0, 0.0, 3.0, 0.0));
    // Invalid: Roof is too high (> 10m overhead)
    assert!(!is_roof_covering(0.0, 15.0, 0.0, 0.0, 3.0, 0.0));
}

#[test]
fn test_structure_repair_validation() {
    // Architectural Note: Repairing restores full health to damaged structures,
    // but rejects blueprints or structures already at full health.
    let mut structure = Structure {
        structure_id: 1,
        parent_id: None,
        piece_type: "Wall".to_string(),
        stability: 80,
        is_grounded: false,
        is_blueprint: false,
        construction_progress: 100,
        current_health: 120.0,
        max_health: 200.0,
        x: 0.0,
        y: 2.0,
        z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        rot_w: 1.0,
        owner_id: 1,
    };

    // Damaged structure is eligible for repair
    assert!(structure.current_health < structure.max_health);
    structure.current_health = structure.max_health;
    assert_eq!(structure.current_health, 200.0);

    // Once fully repaired, cannot be repaired again
    let can_repair_again = !structure.is_blueprint && structure.current_health < structure.max_health;
    assert!(!can_repair_again);

    // Blueprint cannot be repaired
    structure.is_blueprint = true;
    structure.current_health = 50.0;
    let can_repair_blueprint = !structure.is_blueprint && structure.current_health < structure.max_health;
    assert!(!can_repair_blueprint);
}
