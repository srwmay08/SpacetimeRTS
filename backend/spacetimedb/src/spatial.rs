// ----------------------------------------------------------------------------
// SPATIAL HASH GRID (Server-Side Broad-Phase Optimization)
// ----------------------------------------------------------------------------
// Architectural Note: Provides O(1) spatial queries for entity lookups.
// Replaces full table scans with cell-based hashing for projectile hit detection,
// AI target finding, and proximity checks. Grid cell size matches chunk size (50m).

use spacetimedb::{ReducerContext, Table};
use std::collections::BTreeMap;
use std::sync::RwLock;
use crate::movement::transform;
use crate::building::structure;

/// Spatial grid cell size in meters (matches chunk size for alignment)
pub const GRID_CELL_SIZE: f32 = 50.0;

/// Spatial hash grid: (cell_x, cell_y, cell_z) -> Vec<entity_id>
/// Thread-safe with RwLock for concurrent read access during ticks
static SPATIAL_GRID: RwLock<Option<BTreeMap<(i32, i32, i32), Vec<u64>>>> = RwLock::new(None);

/// Initialize the spatial grid (called once at module load)
fn get_grid() -> std::sync::RwLockReadGuard<'static, Option<BTreeMap<(i32, i32, i32), Vec<u64>>>> {
    SPATIAL_GRID.read().unwrap()
}

/// Get mutable access to initialize the grid
fn get_grid_mut() -> std::sync::RwLockWriteGuard<'static, Option<BTreeMap<(i32, i32, i32), Vec<u64>>>> {
    SPATIAL_GRID.write().unwrap()
}

/// Convert world position to grid cell coordinates (2D projection)
#[inline]
pub fn world_to_cell(x: f32, z: f32) -> (i32, i32) {
    (
        (x / GRID_CELL_SIZE).floor() as i32,
        (z / GRID_CELL_SIZE).floor() as i32,
    )
}

/// Convert world position to 3D grid cell coordinates
#[inline]
pub fn world_to_cell_3d(x: f32, y: f32, z: f32) -> (i32, i32, i32) {
    (
        (x / GRID_CELL_SIZE).floor() as i32,
        (y / GRID_CELL_SIZE).floor() as i32,
        (z / GRID_CELL_SIZE).floor() as i32,
    )
}

/// Rebuild the spatial grid from all transform records in full 3D space
/// Called at the start of each tick to ensure consistency
pub fn rebuild_spatial_grid(ctx: &ReducerContext) {
    let mut grid = BTreeMap::new();
    
    // Index all entities with transforms
    for t in ctx.db.transform().iter() {
        let cell = world_to_cell_3d(t.x, t.y, t.z);
        grid.entry(cell).or_insert_with(Vec::new).push(t.entity_id);
    }
    
    // Also index structures for collision queries
    for s in ctx.db.structure().iter() {
        let cell = world_to_cell_3d(s.x, s.y, s.z);
        grid.entry(cell).or_insert_with(Vec::new).push(s.structure_id);
    }
    
    *get_grid_mut() = Some(grid);
}

/// Get all entity IDs in a 3D cell and its 26 neighbors (3x3x3 neighborhood)
pub fn get_nearby_entities_3d(x: f32, y: f32, z: f32, radius_meters: f32) -> Vec<u64> {
    let grid_guard = get_grid();
    let Some(grid) = grid_guard.as_ref() else {
        return Vec::new();
    };
    
    let center_cell = world_to_cell_3d(x, y, z);
    let radius_cells = (radius_meters / GRID_CELL_SIZE).ceil() as i32;
    
    let mut result = Vec::new();
    
    for dx in -radius_cells..=radius_cells {
        for dy in -radius_cells..=radius_cells {
            for dz in -radius_cells..=radius_cells {
                let cell = (center_cell.0 + dx, center_cell.1 + dy, center_cell.2 + dz);
                if let Some(entities) = grid.get(&cell) {
                    result.extend(entities.iter().copied());
                }
            }
        }
    }
    
    result
}

/// Backwards-compatible 2D query across surface neighborhood
pub fn get_nearby_entities(x: f32, z: f32, radius_meters: f32) -> Vec<u64> {
    get_nearby_entities_3d(x, 0.0, z, radius_meters)
}

/// Get entities in same 3D cell only (for very close range checks)
pub fn get_entities_in_cell(x: f32, z: f32) -> Vec<u64> {
    let grid_guard = get_grid();
    let Some(grid) = grid_guard.as_ref() else {
        return Vec::new();
    };
    
    let cell = world_to_cell_3d(x, 0.0, z);
    grid.get(&cell).cloned().unwrap_or_default()
}

/// Filter entities by 3D distance from point (for precise radius checks after broad-phase)
pub fn filter_by_distance_3d(
    ctx: &ReducerContext,
    entity_ids: Vec<u64>,
    x: f32,
    y: f32,
    z: f32,
    max_dist_sq: f32,
) -> Vec<u64> {
    entity_ids
        .into_iter()
        .filter(|&id| {
            if let Some(t) = ctx.db.transform().entity_id().find(id) {
                let dx = t.x - x;
                let dy = t.y - y;
                let dz = t.z - z;
                dx * dx + dy * dy + dz * dz <= max_dist_sq
            } else {
                false
            }
        })
        .collect()
}

/// Filter entities by 2D distance from point (backwards-compatible)
pub fn filter_by_distance(
    ctx: &ReducerContext,
    entity_ids: Vec<u64>,
    x: f32,
    z: f32,
    max_dist_sq: f32,
) -> Vec<u64> {
    entity_ids
        .into_iter()
        .filter(|&id| {
            if let Some(t) = ctx.db.transform().entity_id().find(id) {
                let dx = t.x - x;
                let dz = t.z - z;
                dx * dx + dz * dz <= max_dist_sq
            } else {
                false
            }
        })
        .collect()
}
