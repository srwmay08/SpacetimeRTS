// ----------------------------------------------------------------------------
// INVENTORY SYSTEM INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Validates the server-authoritative 16-slot padded inventory model.
// SpacetimeDB serializes fixed-capacity collections efficiently, avoiding dynamic allocation
// reallocations across reducers. Tests verify discoverability tracking, max-stack limits (50),
// slot preservation during item removal, and multi-slot stacking/drain atomicity.

use backend::{add_item, ensure_inventory_capacity, has_item, remove_item, Inventory, InventorySlot};

/// Helper function to count non-empty active slots in an inventory
fn helper_active_slot_count(inv: &Inventory) -> usize {
    inv.slots.iter().filter(|s| s.count > 0 && !s.item_type.is_empty()).count()
}

/// Helper function to construct a fresh, empty test inventory for entity_id.
/// Automatically initializes the `discovered_items` table column added in schema v2.
fn create_empty_inventory(entity_id: u64) -> Inventory {
    Inventory {
        entity_id,
        slots: Vec::new(),
        discovered_items: Vec::new(),
    }
}

#[test]
fn test_ensure_inventory_capacity_pads_to_16_slots() {
    // Architectural Note: Pre-allocating exactly 16 slots stabilizes SpacetimeDB BSATN
    // serialized row length and prevents out-of-bounds indexing in client drag-and-drop slots.
    let mut inv = create_empty_inventory(1);
    assert_eq!(inv.slots.len(), 0);

    ensure_inventory_capacity(&mut inv);
    assert_eq!(inv.slots.len(), 16);
    for slot in &inv.slots {
        assert_eq!(slot.item_type, "");
        assert_eq!(slot.count, 0);
    }

    // Repeated calls must be idempotent
    ensure_inventory_capacity(&mut inv);
    assert_eq!(inv.slots.len(), 16);
}

#[test]
fn test_add_item_to_empty_inventory() {
    // Architectural Note: Adding items into an unpadded or empty inventory must pad to 16 slots
    // while populating slot 0 with the added resource.
    let mut inv = create_empty_inventory(1);

    add_item(&mut inv, "Wood", 10);

    assert_eq!(inv.slots.len(), 16, "Backend ensures 16 fixed slots for player inventory");
    assert_eq!(helper_active_slot_count(&inv), 1);
    assert_eq!(inv.slots[0].item_type, "Wood");
    assert_eq!(inv.slots[0].count, 10);
    assert!(inv.discovered_items.contains(&"Wood".to_string()), "Item discovery should track Wood");
}

#[test]
fn test_add_item_tracks_discovered_items() {
    // Architectural Note: Discovered items table informs the client tech tree & crafting menu
    // of which recipes the player is eligible to craft without scanning historical event logs.
    let mut inv = create_empty_inventory(1);

    add_item(&mut inv, "Wood", 5);
    add_item(&mut inv, "Stone", 2);
    add_item(&mut inv, "Flint", 1);

    assert_eq!(inv.discovered_items.len(), 3);
    assert!(inv.discovered_items.contains(&"Wood".to_string()));
    assert!(inv.discovered_items.contains(&"Stone".to_string()));
    assert!(inv.discovered_items.contains(&"Flint".to_string()));

    // Adding existing item must not duplicate discovery entry
    add_item(&mut inv, "Wood", 10);
    assert_eq!(inv.discovered_items.len(), 3);
}

#[test]
fn test_add_item_empty_or_zero_ignored() {
    // Invariant: Non-existent item types or 0 counts must not mutate inventory or discoverability.
    let mut inv = create_empty_inventory(1);

    add_item(&mut inv, "", 10);
    assert!(inv.slots.is_empty());
    assert!(inv.discovered_items.is_empty());

    add_item(&mut inv, "Wood", 0);
    assert!(inv.slots.is_empty());
    assert!(inv.discovered_items.is_empty());
}

#[test]
fn test_add_item_stacks_existing() {
    // Architectural Note: Items stack up to a maximum threshold of 50 units before spilling
    // into adjacent slots, preserving precious inventory space.
    let mut inv = create_empty_inventory(1);
    add_item(&mut inv, "Wood", 40);
    add_item(&mut inv, "Wood", 5);

    assert_eq!(inv.slots.len(), 16);
    assert_eq!(helper_active_slot_count(&inv), 1);
    assert_eq!(inv.slots[0].item_type, "Wood");
    assert_eq!(inv.slots[0].count, 45);
    assert_eq!(inv.slots[1].count, 0);
}

#[test]
fn test_add_item_creates_new_slot_when_stack_full() {
    // Overflowing a max stack of 50 must place remaining units in the next available empty slot.
    let mut inv = create_empty_inventory(1);
    add_item(&mut inv, "Wood", 40);
    add_item(&mut inv, "Wood", 20); // 40 + 20 = 50 in slot 0, 10 in slot 1

    assert_eq!(inv.slots.len(), 16);
    assert_eq!(helper_active_slot_count(&inv), 2);
    assert_eq!(inv.slots[0].item_type, "Wood");
    assert_eq!(inv.slots[0].count, 50);
    assert_eq!(inv.slots[1].item_type, "Wood");
    assert_eq!(inv.slots[1].count, 10);
}

#[test]
fn test_add_item_respects_max_slots() {
    // Architectural Note: When all 16 slots are completely filled to max stack (50),
    // additional items must be rejected without exceeding the 16-slot boundary or corrupting state.
    let mut inv = create_empty_inventory(1);

    // Fill all 16 slots with 50 units each
    for i in 0..16 {
        inv.slots.push(InventorySlot {
            item_type: format!("Item{}", i),
            count: 50,
        });
    }
    assert_eq!(inv.slots.len(), 16);

    // Attempt adding a 17th unique item
    add_item(&mut inv, "ExtraItem", 10);

    assert_eq!(inv.slots.len(), 16);
    assert!(!has_item(&inv, "ExtraItem", 1), "Cannot add ExtraItem when all 16 slots are full");
}

#[test]
fn test_add_item_partial_fill_when_slots_full() {
    // Architectural Note: If slots 0-14 are full and slot 15 has room for 5 units,
    // adding 10 units must top off slot 15 to 50, discarding the remaining 5 due to full inventory.
    let mut inv = create_empty_inventory(1);

    for i in 0..15 {
        add_item(&mut inv, &format!("Item{}", i), 50);
    }
    add_item(&mut inv, "Wood", 45); // Slot 15 has 45 Wood

    add_item(&mut inv, "Wood", 10); // 5 fits in slot 15, remaining 5 cannot fit

    assert_eq!(inv.slots.len(), 16);
    assert_eq!(inv.slots[15].item_type, "Wood");
    assert_eq!(inv.slots[15].count, 50);
    assert_eq!(has_item(&inv, "Wood", 50), true);
    assert_eq!(has_item(&inv, "Wood", 51), false);
}

#[test]
fn test_has_item_queries() {
    let mut inv = create_empty_inventory(1);
    add_item(&mut inv, "Wood", 30);
    add_item(&mut inv, "Wood", 25); // Split across 2 slots: 50 in slot 0, 5 in slot 1

    assert!(has_item(&inv, "Wood", 0));
    assert!(has_item(&inv, "Wood", 30));
    assert!(has_item(&inv, "Wood", 55));
    assert!(!has_item(&inv, "Wood", 56));
    assert!(!has_item(&inv, "Stone", 1));
}

#[test]
fn test_remove_item_partial() {
    let mut inv = create_empty_inventory(1);
    add_item(&mut inv, "Wood", 30);

    let result = remove_item(&mut inv, "Wood", 10);

    assert!(result);
    assert_eq!(inv.slots[0].count, 20);
    assert_eq!(inv.slots[0].item_type, "Wood");
    assert_eq!(has_item(&inv, "Wood", 20), true);
}

#[test]
fn test_remove_item_exact() {
    // Architectural Note: Removing all items in a slot empties it (`item_type` cleared, `count` = 0)
    // while keeping the slot index allocated in the 16-slot array.
    let mut inv = create_empty_inventory(1);
    add_item(&mut inv, "Wood", 10);

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
    // Invariant: If player has fewer items than requested, removal fails atomically
    // without altering any inventory slots.
    let mut inv = create_empty_inventory(1);
    add_item(&mut inv, "Wood", 5);

    let result = remove_item(&mut inv, "Wood", 10);

    assert!(!result);
    assert_eq!(inv.slots[0].count, 5);
    assert_eq!(inv.slots[0].item_type, "Wood");
}

#[test]
fn test_remove_item_across_multiple_slots() {
    // Architectural Note: Draining resources across multiple stacks must consume greedily
    // from earlier slots before proceeding to subsequent slots.
    let mut inv = create_empty_inventory(1);
    add_item(&mut inv, "Wood", 50); // Slot 0: 50
    add_item(&mut inv, "Wood", 25); // Slot 1: 25

    let result = remove_item(&mut inv, "Wood", 60);

    assert!(result);
    // Slot 0 completely drained and cleared
    assert_eq!(inv.slots[0].count, 0);
    assert!(inv.slots[0].item_type.is_empty());
    // Slot 1 reduced from 25 to 15
    assert_eq!(inv.slots[1].count, 15);
    assert_eq!(inv.slots[1].item_type, "Wood");
    assert_eq!(helper_active_slot_count(&inv), 1);
    assert!(has_item(&inv, "Wood", 15));
}

#[test]
fn test_remove_item_clears_slot_data_while_preserving_16_slots() {
    let mut inv = create_empty_inventory(1);
    add_item(&mut inv, "Wood", 10);
    add_item(&mut inv, "Stone", 5);

    assert!(remove_item(&mut inv, "Wood", 10));

    assert_eq!(inv.slots.len(), 16);
    assert_eq!(inv.slots[0].count, 0);
    assert!(inv.slots[0].item_type.is_empty());
    assert_eq!(inv.slots[1].item_type, "Stone");
    assert_eq!(inv.slots[1].count, 5);
    assert_eq!(helper_active_slot_count(&inv), 1);
}
