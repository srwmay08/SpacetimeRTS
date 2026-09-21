//! Integration tests for inventory management functions.
//! These tests verify the core inventory logic used by reducers.

use backend::{add_item, remove_item, Inventory, InventorySlot};

#[test]
fn test_add_item_to_empty_inventory() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![],
    };
    
    add_item(&mut inv, "Wood", 10);
    
    assert_eq!(inv.slots.len(), 1);
    assert_eq!(inv.slots[0].item_type, "Wood");
    assert_eq!(inv.slots[0].count, 10);
}

#[test]
fn test_add_item_stacks_existing() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![InventorySlot {
            item_type: "Wood".to_string(),
            count: 40,
        }],
    };
    
    add_item(&mut inv, "Wood", 5);
    
    assert_eq!(inv.slots.len(), 1);
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
    };
    
    add_item(&mut inv, "Wood", 10);
    
    assert_eq!(inv.slots.len(), 2);
    assert_eq!(inv.slots[0].count, 50);
    assert_eq!(inv.slots[1].count, 10);
}

#[test]
fn test_add_item_respects_max_slots() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![],
    };
    
    // Fill all 16 slots
    for i in 0..16 {
        inv.slots.push(InventorySlot {
            item_type: format!("Item{}", i),
            count: 50,
        });
    }
    
    // Try to add more
    add_item(&mut inv, "Wood", 10);
    
    // Should not create 17th slot
    assert_eq!(inv.slots.len(), 16);
}

#[test]
fn test_add_item_partial_fill_when_slots_full() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![],
    };
    
    // Fill 15 slots completely, 16th slot partially
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
    
    // Add 10 wood — 5 should fit in existing slot, 5 should be lost (no more slots)
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
    };
    
    let result = remove_item(&mut inv, "Wood", 10);
    
    assert!(result);
    assert_eq!(inv.slots[0].count, 20);
}

#[test]
fn test_remove_item_exact() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![InventorySlot {
            item_type: "Wood".to_string(),
            count: 10,
        }],
    };
    
    let result = remove_item(&mut inv, "Wood", 10);
    
    assert!(result);
    assert_eq!(inv.slots.len(), 0); // Slot cleaned up
}

#[test]
fn test_remove_item_insufficient() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: vec![InventorySlot {
            item_type: "Wood".to_string(),
            count: 5,
        }],
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
    };
    
    let result = remove_item(&mut inv, "Wood", 40);
    
    assert!(result);
    assert_eq!(inv.slots.len(), 1);
    assert_eq!(inv.slots[0].count, 15);
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
    };
    
    remove_item(&mut inv, "Wood", 10);
    
    assert_eq!(inv.slots.len(), 1);
    assert_eq!(inv.slots[0].item_type, "Ore");
}
