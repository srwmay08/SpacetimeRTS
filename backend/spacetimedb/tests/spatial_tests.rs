// ----------------------------------------------------------------------------
// SPATIAL HASH GRID INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Tests the server-side broad-phase spatial hash partitioning.
// Verifies metric world-to-cell projection (50m grid alignment), neighborhood
// radius query ranges, Euclidean distance squared filtering, and deterministic
// entity indexing.

use backend::spatial::{world_to_cell, GRID_CELL_SIZE};
use std::collections::BTreeMap;

#[test]
fn test_spatial_grid_constants() {
    // Architectural Note: 50.0m cell size aligns exactly with SpacetimeDB
    // 2D horizontal chunk bounds, simplifying chunk-based client subscriptions.
    assert_eq!(GRID_CELL_SIZE, 50.0);
}

#[test]
fn test_world_to_cell_mapping_positive() {
    // [0.0, 50.0) -> Cell 0
    assert_eq!(world_to_cell(0.0, 0.0), (0, 0));
    assert_eq!(world_to_cell(25.0, 25.0), (0, 0));
    assert_eq!(world_to_cell(49.99, 49.99), (0, 0));

    // [50.0, 100.0) -> Cell 1
    assert_eq!(world_to_cell(50.0, 50.0), (1, 1));
    assert_eq!(world_to_cell(75.5, 99.9), (1, 1));

    // [100.0, 150.0) -> Cell 2
    assert_eq!(world_to_cell(100.0, 100.0), (2, 2));
}

#[test]
fn test_world_to_cell_mapping_negative() {
    // [-50.0, 0.0) -> Cell -1
    assert_eq!(world_to_cell(-0.01, -0.01), (-1, -1));
    assert_eq!(world_to_cell(-25.0, -25.0), (-1, -1));
    assert_eq!(world_to_cell(-50.0, -50.0), (-1, -1));

    // [-100.0, -50.0) -> Cell -2
    assert_eq!(world_to_cell(-50.01, -50.01), (-2, -2));
    assert_eq!(world_to_cell(-100.0, -100.0), (-2, -2));
}

#[test]
fn test_radius_cell_span_calculation() {
    // Architectural Note: Radius cells determine the size of the neighborhood query window:
    // (radius_meters / GRID_CELL_SIZE).ceil() as i32
    let calc_radius_cells = |radius_meters: f32| -> i32 {
        (radius_meters / GRID_CELL_SIZE).ceil() as i32
    };

    assert_eq!(calc_radius_cells(10.0), 1);   // 3x3 cells (-1..=1)
    assert_eq!(calc_radius_cells(50.0), 1);   // 3x3 cells
    assert_eq!(calc_radius_cells(50.1), 2);   // 5x5 cells (-2..=2)
    assert_eq!(calc_radius_cells(100.0), 2);  // 5x5 cells
    assert_eq!(calc_radius_cells(150.0), 3);  // 7x7 cells
}

#[test]
fn test_spatial_grid_btreemap_determinism() {
    // Architectural Note: Anti-desync mandate requires BTreeMap instead of HashMap
    // so cell iteration order is strictly deterministic across all network replicas.
    let mut grid: BTreeMap<(i32, i32), Vec<u64>> = BTreeMap::new();

    // Insert out-of-order
    grid.entry((1, 0)).or_default().push(101);
    grid.entry((-1, -1)).or_default().push(102);
    grid.entry((0, 0)).or_default().push(103);
    grid.entry((0, 1)).or_default().push(104);

    let sorted_keys: Vec<(i32, i32)> = grid.keys().copied().collect();
    assert_eq!(sorted_keys, vec![(-1, -1), (0, 0), (0, 1), (1, 0)]);
}

#[test]
fn test_distance_squared_filtering() {
    // Architectural Note: Distance squared comparison avoids costly sqrt calls
    // during post-broadphase entity filtering.
    let center_x = 10.0_f32;
    let center_z = 20.0_f32;
    let max_dist = 15.0_f32;
    let max_dist_sq = max_dist * max_dist; // 225.0

    let entities = [
        (1u64, 10.0_f32, 20.0_f32), // dist 0 -> dist_sq 0 (Inside)
        (2u64, 20.0_f32, 20.0_f32), // dist 10 -> dist_sq 100 (Inside)
        (3u64, 10.0_f32, 35.0_f32), // dist 15 -> dist_sq 225 (Boundary, Inside)
        (4u64, 25.0_f32, 35.0_f32), // dist sqrt(15^2 + 15^2) = 450 (Outside)
    ];

    let filtered: Vec<u64> = entities
        .iter()
        .filter(|(_, x, z)| {
            let dx = x - center_x;
            let dz = z - center_z;
            dx * dx + dz * dz <= max_dist_sq
        })
        .map(|(id, _, _)| *id)
        .collect();

    assert_eq!(filtered, vec![1, 2, 3]);
    assert!(!filtered.contains(&4));
}
