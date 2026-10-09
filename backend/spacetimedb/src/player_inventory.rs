// ============================================================================
// File: backend/spacetimedb/src/player_inventory.rs
// ============================================================================
// Authoritative player inventory tables, 16-slot capacity utilities,
// item stacking rules (50/slot), slot swap reducers, and drop reducers.

use spacetimedb::{table, reducer, ReducerContext, Table, SpacetimeType};
use crate::movement::{player_session, transform};
use crate::ai::harvestable_corpse;
use crate::combat::{health, faction_component};
use crate::harvesting::get_terrain_height;

#[derive(SpacetimeType, Clone, Debug, PartialEq, Eq)]
pub struct InventorySlot {
    pub item_type: String,
    pub count: u32,
}

#[table(accessor = inventory, public)]
#[derive(Clone)]
pub struct Inventory {
    #[primary_key] pub entity_id: u64,
    pub slots: Vec<InventorySlot>,
    pub discovered_items: Vec<String>,
}

// ----------------------------------------------------------------------------
// INVENTORY UTILITIES
// ----------------------------------------------------------------------------

pub fn ensure_inventory_capacity(inventory: &mut Inventory) {
    while inventory.slots.len() < 16 {
        inventory.slots.push(InventorySlot {
            item_type: String::new(),
            count: 0,
        });
    }
}

pub fn add_item(inventory: &mut Inventory, item_type: &str, mut amount: u32) {
    if item_type.is_empty() || amount == 0 {
        return;
    }

    if !inventory.discovered_items.iter().any(|d| d == item_type) {
        inventory.discovered_items.push(item_type.to_string());
    }

    ensure_inventory_capacity(inventory);

    for slot in inventory.slots.iter_mut() {
        if slot.item_type == item_type && slot.count > 0 && slot.count < 50 {
            let space = 50 - slot.count;
            if amount <= space {
                slot.count += amount;
                amount = 0;
                break;
            } else {
                slot.count = 50;
                amount -= space;
            }
        }
    }

    if amount > 0 {
        for slot in inventory.slots.iter_mut() {
            if slot.count == 0 || slot.item_type.is_empty() {
                let add_amt = amount.min(50);
                slot.item_type = item_type.to_string();
                slot.count = add_amt;
                amount -= add_amt;
                if amount == 0 {
                    break;
                }
            }
        }
    }
}

pub fn remove_item(inventory: &mut Inventory, item_type: &str, mut amount: u32) -> bool {
    ensure_inventory_capacity(inventory);

    let total: u32 = inventory.slots.iter()
        .filter(|s| s.item_type == item_type && s.count > 0)
        .map(|s| s.count)
        .sum();

    if total < amount {
        return false;
    }

    for slot in inventory.slots.iter_mut() {
        if slot.item_type == item_type && slot.count > 0 {
            if slot.count >= amount {
                slot.count -= amount;
                if slot.count == 0 {
                    slot.item_type.clear();
                }
                break;
            } else {
                amount -= slot.count;
                slot.count = 0;
                slot.item_type.clear();
            }
        }
    }
    true
}

pub fn has_item(inventory: &Inventory, item_type: &str, amount: u32) -> bool {
    let total: u32 = inventory.slots.iter()
        .filter(|s| s.item_type == item_type && s.count > 0)
        .map(|s| s.count)
        .sum();
    total >= amount
}

// ----------------------------------------------------------------------------
// INVENTORY INTERACTION & REORGANIZATION REDUCERS
// ----------------------------------------------------------------------------

#[reducer]
pub fn swap_inventory_slots(ctx: &ReducerContext, from_slot: u32, to_slot: u32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    if from_slot >= 16 || to_slot >= 16 {
        return Err("Slot index out of bounds.".to_string());
    }

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    ensure_inventory_capacity(&mut inv);

    let from_idx = from_slot as usize;
    let to_idx = to_slot as usize;

    if from_idx == to_idx {
        return Ok(());
    }

    if inv.slots[from_idx].item_type == inv.slots[to_idx].item_type
        && !inv.slots[from_idx].item_type.is_empty()
        && inv.slots[from_idx].count > 0
    {
        let current_to = inv.slots[to_idx].count;
        let available = 50u32.saturating_sub(current_to);
        if available > 0 {
            let move_amt = inv.slots[from_idx].count.min(available);
            inv.slots[to_idx].count += move_amt;
            inv.slots[from_idx].count -= move_amt;
            if inv.slots[from_idx].count == 0 {
                inv.slots[from_idx].item_type.clear();
            }
            ctx.db.inventory().entity_id().update(inv);
            return Ok(());
        }
    }

    inv.slots.swap(from_idx, to_idx);
    ctx.db.inventory().entity_id().update(inv);
    Ok(())
}

#[reducer]
pub fn drop_inventory_item(ctx: &ReducerContext, slot_index: u32, mut amount: u32) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    if slot_index >= 16 {
        return Err("Slot index out of bounds.".to_string());
    }

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    ensure_inventory_capacity(&mut inv);

    let slot_idx = slot_index as usize;
    if inv.slots[slot_idx].count == 0 || inv.slots[slot_idx].item_type.is_empty() {
        return Err("Slot is empty.".to_string());
    }

    let item_type = inv.slots[slot_idx].item_type.clone();
    if amount == 0 || amount > inv.slots[slot_idx].count {
        amount = inv.slots[slot_idx].count;
    }

    inv.slots[slot_idx].count -= amount;
    if inv.slots[slot_idx].count == 0 {
        inv.slots[slot_idx].item_type.clear();
    }
    ctx.db.inventory().entity_id().update(inv);

    let transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or_else(|| "Player transform missing.".to_string())?;

    let drop_id = ((ctx.timestamp.to_micros_since_unix_epoch() as u64) << 16)
        ^ (session.entity_id.wrapping_add(slot_index as u64 + 7777));

    let drop_x = transform.x + 1.8;
    let drop_z = transform.z + 1.8;
    let surface_y = get_terrain_height(drop_x, drop_z);
    let floor_y = if transform.y < surface_y - 2.0 {
        crate::voxel::find_ground_surface_below(ctx, drop_x, transform.y + 0.5, drop_z)
    } else {
        surface_y
    };
    let drop_y = floor_y + 0.35;

    ctx.db.harvestable_corpse().insert(crate::ai::HarvestableCorpse {
        entity_id: drop_id,
        loot_item: item_type.clone(),
        amount,
    });

    ctx.db.transform().insert(crate::movement::Transform {
        entity_id: drop_id,
        x: drop_x,
        y: drop_y,
        z: drop_z,
        chunk_x: (drop_x / 50.0).floor() as i32,
        chunk_z: (drop_z / 50.0).floor() as i32,
        last_processed_tick: 0,
    });

    ctx.db.faction_component().insert(crate::combat::FactionComponent {
        entity_id: drop_id,
        faction: crate::combat::Faction::Wildlife,
    });

    ctx.db.health().insert(crate::combat::Health {
        entity_id: drop_id,
        current: 1.0,
        max: 1.0,
    });

    log::debug!("Player {} dropped {}x '{}' into world.", session.entity_id, amount, item_type);
    Ok(())
}
