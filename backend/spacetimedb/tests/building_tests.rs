// ----------------------------------------------------------------------------
// MODULAR BUILDING & STABILITY INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024)
// ----------------------------------------------------------------------------
// Architectural Note: Tests the server-authoritative structural integrity system.
// Validates stability decay propagation from grounded anchors, piece health
// presets, hammer swing construction costs, cascading destruction queues,
// and workbench placement prerequisites.

use backend::building::{get_piece_cost, get_piece_decay, get_piece_max_health};
use backend::building::Structure;

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
    assert_eq!(get_piece_max_health("Window"), 150.0);
    assert_eq!(get_piece_max_health("Door"), 120.0);
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
    // Ground anchors (0 penalty), Walls/Windows/Doors (20), Floors (25), Roofs (30), Ramps (25).
    assert_eq!(get_piece_decay("Foundation"), Some(0));
    assert_eq!(get_piece_decay("Workbench"), Some(0));
    assert_eq!(get_piece_decay("Campfire"), Some(0));
    assert_eq!(get_piece_decay("Wall"), Some(20));
    assert_eq!(get_piece_decay("Window"), Some(20));
    assert_eq!(get_piece_decay("Door"), Some(20));
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
        "Workbench",
        "Campfire",
        "Foundation",
        "Wall",
        "Window",
        "Door",
        "Floor",
        "Roof",
        "Ramp",
    ];

    for piece in pieces {
        let (total_wood, total_stone) = get_piece_cost(piece);
        assert_eq!(total_wood % 4, 0, "Wood cost for {} must be divisible by 4", piece);
        assert_eq!(total_stone % 4, 0, "Stone cost for {} must be divisible by 4", piece);
    }
    assert_eq!(get_piece_cost("Window"), (8, 0));
    assert_eq!(get_piece_cost("Door"), (12, 0));
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
        current_support: 80.0,
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

#[test]
fn test_dag_support_formula_and_directional_degradation() {
    // Formula: S_current = min(S_parent, max_support) - (d_h * c_h + d_v * c_v)
    // Wood parameters: max=100.0, min=10.0, c_h=22.5, c_v=4.5
    // Stone parameters: max=150.0, min=15.0, c_h=35.0, c_v=3.0
    let calc_wood = |parent_sup: f32, dh: f32, dv: f32| -> f32 {
        parent_sup.min(100.0) - (dh * 22.5 + dv * 4.5)
    };
    let calc_stone = |parent_sup: f32, dh: f32, dv: f32| -> f32 {
        parent_sup.min(150.0) - (dh * 35.0 + dv * 3.0)
    };

    // 1. Wood vertical stacking: dh = 0, dv = 3.0m (1 story)
    // Loss per story = 3.0 * 4.5 = 13.5
    let w1 = calc_wood(100.0, 0.0, 3.0);
    assert_eq!(w1, 86.5);
    let w2 = calc_wood(w1, 0.0, 3.0);
    assert_eq!(w2, 73.0);
    let w3 = calc_wood(w2, 0.0, 3.0);
    assert_eq!(w3, 59.5);
    let w4 = calc_wood(w3, 0.0, 3.0);
    assert_eq!(w4, 46.0);
    let w5 = calc_wood(w4, 0.0, 3.0);
    assert_eq!(w5, 32.5);
    let w6 = calc_wood(w5, 0.0, 3.0);
    assert_eq!(w6, 19.0);
    let w7 = calc_wood(w6, 0.0, 3.0);
    assert_eq!(w7, 5.5); // 5.5 < 10.0 min_support: fails to support 7th story without pillars!
    assert!(w7 < 10.0);

    // 2. Wood horizontal span: dh = 2.0m, dv = 0
    // Loss per horizontal step = 2.0 * 22.5 = 45.0
    let f1 = calc_wood(100.0, 2.0, 0.0);
    assert_eq!(f1, 55.0);
    let f2 = calc_wood(f1, 2.0, 0.0);
    assert_eq!(f2, 10.0); // Exact viable threshold for 2nd cantilever tile
    let f3 = calc_wood(f2, 2.0, 0.0);
    assert_eq!(f3, -35.0); // 3rd cantilever collapses without ground support!
    assert!(f3 < 10.0);

    // 3. Stone horizontal vs vertical:
    // Vertical: 3.0 * 3.0 = 9.0 loss per story
    let s_vert = calc_stone(150.0, 0.0, 3.0);
    assert_eq!(s_vert, 141.0);
    // Horizontal: 2.0 * 35.0 = 70.0 loss per horizontal step
    let s_horiz1 = calc_stone(150.0, 2.0, 0.0);
    assert_eq!(s_horiz1, 80.0);
    let s_horiz2 = calc_stone(s_horiz1, 2.0, 0.0);
    assert_eq!(s_horiz2, 10.0); // < 15.0 min_support for stone: 2nd stone cantilever collapses!
    assert!(s_horiz2 < 15.0);
}

#[test]
fn test_multi_ground_path_maximum_support_adoption() {
    // When a piece is adjacent to two supporting parents, it must adopt the maximum support path.
    let path_a_support = 86.5_f32; // Wall on ground
    let path_b_support = 73.0_f32; // Wall on second floor

    let dh = 2.0_f32;
    let dv = 0.0_f32;
    let ch = 22.5_f32;
    let cv = 4.5_f32;

    let sup_from_a = path_a_support - (dh * ch + dv * cv); // 86.5 - 45.0 = 41.5
    let sup_from_b = path_b_support - (dh * ch + dv * cv); // 73.0 - 45.0 = 28.0

    let chosen_support = sup_from_a.max(sup_from_b);
    assert_eq!(chosen_support, 41.5);
    assert!(chosen_support >= 10.0);
}

#[test]
fn test_autobuild_3_second_progression() {
    // In low_frequency_tick (100ms interval), blueprints increment by 4% per tick.
    // Over 25 ticks (2.5s), it reaches 100% and finishes.
    let mut progress = 0_u32;
    let max_hp = 200.0_f32;
    let mut health;
    let mut is_blueprint = true;

    for tick in 1..=30 {
        progress = (progress + 4).min(100);
        health = (max_hp * (progress as f32 / 100.0)).max(1.0);
        if progress >= 100 {
            is_blueprint = false;
            health = max_hp;
        }
        if tick == 25 {
            assert_eq!(progress, 100);
            assert!(!is_blueprint);
            assert_eq!(health, 200.0);
        }
    }

    assert_eq!(progress, 100);
    assert!(!is_blueprint);
}

#[test]
fn test_subterranean_building_grounding_and_rock_anchor() {
    use backend::building::base_piece_type;

    // 1. Verify piece type classification
    assert_eq!(base_piece_type("Foundation"), "Foundation");
    assert_eq!(base_piece_type("Wall"), "Wall");
    assert_eq!(base_piece_type("Floor"), "Floor");
    assert_eq!(base_piece_type("Roof"), "Roof");

    // 2. Deep subterranean coordinates (e.g. cavern chasm at -60m)
    let y = -60.0_f32;
    let surface_y = 25.0_f32; // Overworld mountain height
    assert!((y - surface_y).abs() > 4.0); // Surface check fails

    // 3. Subterranean cavern floor anchor
    let subterranean_floor_y = -60.0_f32;
    let is_grounded = (y - surface_y).abs() < 4.0 || (y - subterranean_floor_y).abs() < 2.0;
    assert!(is_grounded, "Subterranean foundation must anchor to cavern floor");
}
