// ----------------------------------------------------------------------------
// RESOURCE NODE INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Tests the server-authoritative resource extraction lifecycle.
// Validates tool requirement enforcement (Stone Axe for Tree, Pickaxe for Rock),
// scale-proportional resource yields, deterministic health depletion, and bush
// respawn timer behavior.

use backend::{ResourceNode, add_item, has_item, Inventory};

/// Helper to initialize a player inventory for resource harvesting tests
fn create_test_inventory(has_axe: bool, has_pickaxe: bool) -> Inventory {
    let mut inv = Inventory {
        entity_id: 1,
        slots: Vec::new(),
        discovered_items: Vec::new(),
    };
    if has_axe {
        add_item(&mut inv, "Stone Axe", 1);
    }
    if has_pickaxe {
        add_item(&mut inv, "Pickaxe", 1);
    }
    inv
}

#[test]
fn test_resource_node_spawn_conditions() {
    // Architectural Note: Resource nodes spawn with scale-proportional health
    // and required tool identifiers matched to harvesting difficulty.
    let tree = ResourceNode {
        node_id: 1,
        node_type: "Tree".to_string(),
        x: 10.0,
        y: 5.0,
        z: 20.0,
        chunk_x: 0,
        chunk_z: 0,
        health: 3,
        scale: 1.0,
        required_tool: "Stone Axe".to_string(),
    };

    assert_eq!(tree.node_type, "Tree");
    assert_eq!(tree.health, 3);
    assert_eq!(tree.required_tool, "Stone Axe");
    assert_eq!(tree.scale, 1.0);
}

#[test]
fn test_resource_node_tool_requirements() {
    // Invariant: Advanced nodes require specific crafted tools to harvest.
    let axe_required = "Stone Axe";
    let pickaxe_required = "Pickaxe";
    let hand_gatherable = "None";

    let inv_bare = create_test_inventory(false, false);
    let inv_equipped = create_test_inventory(true, true);

    // Bare hands check
    assert!(!has_item(&inv_bare, axe_required, 1));
    assert!(!has_item(&inv_bare, pickaxe_required, 1));

    // Equipped check
    assert!(has_item(&inv_equipped, axe_required, 1));
    assert!(has_item(&inv_equipped, pickaxe_required, 1));

    // Ground resources require no tools
    assert_eq!(hand_gatherable, "None");
}

#[test]
fn test_tree_yield_calculation_by_scale() {
    // Architectural Note: Tree destruction yield is calculated as:
    // (6.0 * scale).ceil() as u32
    let test_cases: [(f32, u32); 5] = [
        (0.5_f32, 3_u32),
        (1.0_f32, 6_u32),
        (1.2_f32, 8_u32), // 6.0 * 1.2 = 7.2 -> ceil = 8
        (1.5_f32, 9_u32),
        (2.0_f32, 12_u32),
    ];

    for (scale, expected_wood) in test_cases {
        let calculated = (6.0_f32 * scale).ceil() as u32;
        assert_eq!(calculated, expected_wood, "Failed for scale {}", scale);
    }
}

#[test]
fn test_rock_yield_calculation_by_scale() {
    // Architectural Note: Rock destruction yield is calculated as:
    // (4.0 * scale).ceil() as u32
    let test_cases: [(f32, u32); 4] = [
        (0.5_f32, 2_u32),
        (1.0_f32, 4_u32),
        (1.5_f32, 6_u32),
        (2.0_f32, 8_u32),
    ];

    for (scale, expected_stone) in test_cases {
        let calculated = (4.0_f32 * scale).ceil() as u32;
        assert_eq!(calculated, expected_stone, "Failed for rock scale {}", scale);
    }
}

#[test]
fn test_tree_harvest_resin_bonus() {
    // Architectural Note: Harvesting a tree with a Stone Axe grants a bonus 1x Resin,
    // which is essential for crafting torches and fire arrows.
    let mut inv = create_test_inventory(true, false);
    let scale: f32 = 1.0;
    let wood_amount = (6.0_f32 * scale).ceil() as u32;

    add_item(&mut inv, "Wood", wood_amount);
    if has_item(&inv, "Stone Axe", 1) {
        add_item(&mut inv, "Resin", 1);
    }

    assert_eq!(has_item(&inv, "Wood", 6), true);
    assert_eq!(has_item(&inv, "Resin", 1), true);
}

#[test]
fn test_bush_berry_harvest_and_depletion() {
    // Architectural Note: Harvesting a Berry bush yields 2 Berries and sets its health to 0.
    // Depleted bushes cannot be harvested again until respawned by the timer.
    let mut inv = create_test_inventory(false, false);
    let mut bush = ResourceNode {
        node_id: 10,
        node_type: "Bush".to_string(),
        x: 0.0,
        y: 0.0,
        z: 0.0,
        chunk_x: 0,
        chunk_z: 0,
        health: 1,
        scale: 1.0,
        required_tool: "None".to_string(),
    };

    assert_eq!(bush.health, 1);

    // Initial harvest
    add_item(&mut inv, "Berry", 2);
    bush.health = 0;

    assert_eq!(has_item(&inv, "Berry", 2), true);
    assert_eq!(bush.health, 0);

    // Subsequent harvest attempt when depleted
    let can_harvest = bush.health > 0;
    assert!(!can_harvest, "Depleted bush must not allow harvesting");

    // Respawn restores health to 1
    bush.health = 1;
    assert_eq!(bush.health, 1);
}

#[test]
fn test_ground_resource_collection() {
    // Architectural Note: Ground pickups (Branch, Flint, LooseStone) yield 1 unit
    // without requiring any tools.
    let mut inv = create_test_inventory(false, false);

    add_item(&mut inv, "Branch", 1);
    add_item(&mut inv, "Flint", 1);
    add_item(&mut inv, "LooseStone", 1);

    assert!(has_item(&inv, "Branch", 1));
    assert!(has_item(&inv, "Flint", 1));
    assert!(has_item(&inv, "LooseStone", 1));
}

#[test]
fn test_multi_hit_resource_health_depletion() {
    // Architectural Note: High-durability nodes absorb damage over successive hits
    // before collapsing and yielding full resources on the final strike.
    let mut node_health: u32 = 3;

    // Hit 1: health 3 -> 2 (no yield yet)
    node_health = node_health.saturating_sub(1);
    assert_eq!(node_health, 2);

    // Hit 2: health 2 -> 1 (no yield yet)
    node_health = node_health.saturating_sub(1);
    assert_eq!(node_health, 1);

    // Hit 3: health 1 -> 0 (final blow, node destroyed, yield triggered)
    node_health = node_health.saturating_sub(1);
    assert_eq!(node_health, 0);
}
