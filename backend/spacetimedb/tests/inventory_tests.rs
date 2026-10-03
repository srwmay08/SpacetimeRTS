//! Integration tests for inventory management functions.
//! These tests verify the authoritative backend inventory logic used by SpacetimeDB reducers.

use backend::{add_item, has_item, remove_item, Inventory, InventorySlot};

fn helper_active_slot_count(inv: &Inventory) -> usize {
    inv.slots.iter().filter(|s| s.count > 0 && !s.item_type.is_empty()).count()
}

#[test]
fn test_add_item_to_empty_inventory() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![],
        discovered_items: vec![],
    };
    
    add_item(&mut inv, "Wood", 10);
    
    assert_eq!(inv.slots.len(), 16, "Backend ensures 16 fixed slots for player inventory");
    assert_eq!(helper_active_slot_count(&inv), 1);
    assert_eq!(inv.slots[0].item_type, "Wood");
    assert_eq!(inv.slots[0].count, 10);
    assert!(inv.discovered_items.contains(&"Wood".to_string()), "Item discovery should track Wood");
}

#[test]
fn test_add_item_stacks_existing() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![InventorySlot {
            item_type: "Wood".to_string(),
            count: 40,
        }],
        discovered_items: vec!["Wood".to_string()],
    };
    
    add_item(&mut inv, "Wood", 5);
    
    assert_eq!(inv.slots.len(), 16);
    assert_eq!(helper_active_slot_count(&inv), 1);
    assert_eq!(inv.slots[0].count, 45);
}

#[test]
fn test_add_item_creates_new_slot_when_stack_full() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![InventorySlot {
            item_type: "Wood".to_string(),
            count: 50, // Max stack
        }],
        discovered_items: vec!["Wood".to_string()],
    };
    
    add_item(&mut inv, "Wood", 10);
    
    assert_eq!(inv.slots.len(), 16);
    assert_eq!(helper_active_slot_count(&inv), 2);
    assert_eq!(inv.slots[0].count, 50);
    assert_eq!(inv.slots[1].item_type, "Wood");
    assert_eq!(inv.slots[1].count, 10);
}

#[test]
fn test_add_item_respects_max_slots() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![],
        discovered_items: vec![],
    };
    
    // Fill all 16 slots with 50 of each item
    for i in 0..16 {
        inv.slots.push(InventorySlot {
            item_type: format!("Item{}", i),
            count: 50,
        });
    }
    
    // Try to add more of a new item
    add_item(&mut inv, "Wood", 10);
    
    // Should not create 17th slot, length remains 16
    assert_eq!(inv.slots.len(), 16);
    assert!(!has_item(&inv, "Wood", 1), "Cannot add Wood when all 16 slots are full with other items");
}

#[test]
fn test_add_item_partial_fill_when_slots_full() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![],
        discovered_items: vec![],
    };
    
    // Fill 15 slots completely, 16th slot partially (45/50)
    for i in 0..15 {
        inv.slots.push(InventorySlot {
            item_type: format!("Item{}", i),
            count: 50,
        });
    }
    inv.slots.push(InventorySlot {
        item_type: "Wood".to_string(),
        count: 45,
    });
    
    // Add 10 wood — 5 should fit in existing slot, remaining 5 lost (no available empty slots)
    add_item(&mut inv, "Wood", 10);
    
    assert_eq!(inv.slots.len(), 16);
    assert_eq!(inv.slots[15].count, 50); // Filled to max
}

#[test]
fn test_remove_item_partial() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![InventorySlot {
            item_type: "Wood".to_string(),
            count: 30,
        }],
        discovered_items: vec!["Wood".to_string()],
    };
    
    let result = remove_item(&mut inv, "Wood", 10);
    
    assert!(result);
    assert_eq!(inv.slots[0].count, 20);
    assert_eq!(has_item(&inv, "Wood", 20), true);
}

#[test]
fn test_remove_item_exact() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![InventorySlot {
            item_type: "Wood".to_string(),
            count: 10,
        }],
        discovered_items: vec!["Wood".to_string()],
    };
    
    let result = remove_item(&mut inv, "Wood", 10);
    
    assert!(result);
    assert_eq!(inv.slots.len(), 16);
    assert_eq!(inv.slots[0].count, 0);
    assert!(inv.slots[0].item_type.is_empty(), "Item type cleared on depletion");
    assert_eq!(helper_active_slot_count(&inv), 0);
    assert!(!has_item(&inv, "Wood", 1));
}

#[test]
fn test_remove_item_insufficient() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![InventorySlot {
            item_type: "Wood".to_string(),
            count: 5,
        }],
        discovered_items: vec!["Wood".to_string()],
    };
    
    let result = remove_item(&mut inv, "Wood", 10);
    
    assert!(!result);
    assert_eq!(inv.slots[0].count, 5); // Unchanged
}

#[test]
fn test_remove_item_across_multiple_slots() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![
            InventorySlot {
                item_type: "Wood".to_string(),
                count: 30,
            },
            InventorySlot {
                item_type: "Wood".to_string(),
                count: 25,
            },
        ],
        discovered_items: vec!["Wood".to_string()],
    };
    
    let result = remove_item(&mut inv, "Wood", 40);
    
    assert!(result);
    assert_eq!(inv.slots[0].count, 0); // First slot drained
    assert!(inv.slots[0].item_type.is_empty());
    assert_eq!(inv.slots[1].count, 15); // Second slot has 15 remaining
    assert_eq!(helper_active_slot_count(&inv), 1);
    assert!(has_item(&inv, "Wood", 15));
}

#[test]
fn test_remove_item_cleans_empty_slots() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![
            InventorySlot {
                item_type: "Wood".to_string(),
                count: 10,
            },
            InventorySlot {
                item_type: "Ore".to_string(),
                count: 5,
            },
        ],
        discovered_items: vec!["Wood".to_string(), "Ore".to_string()],
    };
    
    remove_item(&mut inv, "Wood", 10);
    
    assert_eq!(inv.slots[0].count, 0);
    assert!(inv.slots[0].item_type.is_empty());
    assert_eq!(inv.slots[1].item_type, "Ore");
    assert_eq!(inv.slots[1].count, 5);
    assert_eq!(helper_active_slot_count(&inv), 1);
}
