//! Integration tests for terrain generation functions.
//! These tests verify the deterministic noise-based terrain height calculation.

use backend::get_terrain_height;

#[test]
fn test_terrain_height_deterministic() {
    // Same input should always produce same output
    let h1 = get_terrain_height(10.0, 20.0);
    let h2 = get_terrain_height(10.0, 20.0);
    
    assert_eq!(h1, h2);
}

#[test]
fn test_terrain_height_always_positive() {
    // Test a grid of positions
    for x in -100..100 {
        for z in -100..100 {
            let h = get_terrain_height(x as f32, z as f32);
            assert!(h > 0.0, "Terrain height at ({}, {}) was {}", x, z, h);
        }
    }
}

#[test]
fn test_terrain_height_no_nan() {
    for x in -50..50 {
        for z in -50..50 {
            let h = get_terrain_height(x as f32, z as f32);
            assert!(!h.is_nan(), "NaN at ({}, {})", x, z);
            assert!(!h.is_infinite(), "Infinite at ({}, {})", x, z);
        }
    }
}

#[test]
fn test_terrain_height_varies() {
    // Different positions should produce different heights (not a flat plane)
    let h1 = get_terrain_height(0.0, 0.0);
    let h2 = get_terrain_height(50.0, 50.0);
    let h3 = get_terrain_height(-50.0, -50.0);
    
    // At least some variation (not all identical)
    assert!(h1 != h2 || h2 != h3 || h1 != h3, "Terrain appears flat");
}

#[test]
fn test_terrain_height_reasonable_range() {
    // Heights should be within expected bounds (0 to ~30 based on amplitude)
    for x in -200..200 {
        for z in -200..200 {
            let h = get_terrain_height(x as f32, z as f32);
            assert!(h >= 0.0 && h <= 35.0, "Height {} out of range at ({}, {})", h, x, z);
        }
    }
}

#[test]
fn test_terrain_height_lake_depression() {
    // The lake at (-35, -35) should create a depression
    let lake_center = get_terrain_height(-35.0, -35.0);
    let nearby = get_terrain_height(-20.0, -20.0);
    
    // Lake center should be lower than surrounding terrain
    assert!(lake_center < nearby, "Lake center ({}) should be lower than nearby ({})", lake_center, nearby);
}

#[test]
fn test_terrain_height_seeded_variance() {
    use backend::get_terrain_height_seeded;
    let h_seed42 = get_terrain_height_seeded(120.0, -85.0, 42);
    let h_seed99 = get_terrain_height_seeded(120.0, -85.0, 99);
    let h_seed1337 = get_terrain_height_seeded(120.0, -85.0, 1337);

    assert_ne!(h_seed42, h_seed99, "Different world seeds must generate different terrain heights");
    assert_ne!(h_seed42, h_seed1337, "Different world seeds must generate different terrain heights");
}

#[test]
fn test_apply_world_seed_updates_global_height() {
    use backend::apply_world_seed;
    apply_world_seed(42);
    let base_h = get_terrain_height(75.0, 120.0);

    apply_world_seed(9999);
    let new_h = get_terrain_height(75.0, 120.0);
    assert_ne!(base_h, new_h, "Changing active world seed must change global get_terrain_height output");

    // Resetting back to 42 restores original height
    apply_world_seed(42);
    let restored_h = get_terrain_height(75.0, 120.0);
    assert_eq!(base_h, restored_h, "Restoring world seed 42 must restore exact original height");
}

