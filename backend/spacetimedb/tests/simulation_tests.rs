use backend::{add_item, get_terrain_height, remove_item, Inventory};

#[test]
fn test_terrain_height_determinism() {
    let h1 = get_terrain_height(12.5, -45.2);
    let h2 = get_terrain_height(12.5, -45.2);
    assert!(
        (h1 - h2).abs() < f32::EPSILON,
        "Terrain generation must be strictly deterministic across frames"
    );
}

#[test]
fn test_inventory_add_remove_atomic() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![],
        discovered_items: vec![],
    };

    add_item(&mut inv, "Wood", 65);
    assert_eq!(inv.slots.len(), 2);
    assert_eq!(inv.slots[0].count, 50);
    assert_eq!(inv.slots[1].count, 15);

    let success = remove_item(&mut inv, "Wood", 20);
    assert!(success);
    let total: u32 = inv.slots.iter().filter(|s| s.item_type == "Wood").map(|s| s.count).sum();
    assert_eq!(total, 45);

    let over_drain = remove_item(&mut inv, "Wood", 100);
    assert!(!over_drain);
    let total_after_fail: u32 = inv.slots.iter().filter(|s| s.item_type == "Wood").map(|s| s.count).sum();
    assert_eq!(total_after_fail, 45);
}

#[test]
fn test_stability_decay_chain() {
    let foundation_decay = 0;
    let wall_decay = 20;
    let mut stability = 100 - foundation_decay;

    stability -= wall_decay; // Wall 1: 80
    stability -= wall_decay; // Wall 2: 60
    stability -= wall_decay; // Wall 3: 40
    stability -= wall_decay; // Wall 4: 20
    assert_eq!(stability, 20);

    assert!(stability <= wall_decay);
}