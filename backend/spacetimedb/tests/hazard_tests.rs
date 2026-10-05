// ----------------------------------------------------------------------------
// LETHAL FALL HAZARD & RUIN TESTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Validates the pure geometry and math behind the server-side fall sweep
// (point_in_fall_sweep, normalize_dir, ruin_position).

use backend::hazard::{
    normalize_dir, point_in_fall_sweep, ruin_position,
    TREE_FALL_DURATION_MS, TREE_HEIGHT, TREE_KILL_RADIUS,
};

#[test]
fn test_normalize_dir_handles_degenerate_vectors() {
    // Exact zero fallback -> +X
    let (dx, dz) = normalize_dir(0.0, 0.0);
    assert_eq!((dx, dz), (1.0, 0.0));

    // Near zero
    let (dx, dz) = normalize_dir(1.0e-8, -1.0e-8);
    assert_eq!((dx, dz), (1.0, 0.0));

    // Standard unit along +Z
    let (dx, dz) = normalize_dir(0.0, 5.0);
    assert!((dx).abs() < 1.0e-5);
    assert!((dz - 1.0).abs() < 1.0e-5);

    // Diagonal
    let (dx, dz) = normalize_dir(3.0, 4.0);
    assert!((dx - 0.6).abs() < 1.0e-5);
    assert!((dz - 0.8).abs() < 1.0e-5);
}

#[test]
fn test_point_in_fall_sweep_lethal_and_safe_zones() {
    // Tree rooted at origin, falling +Z, height 18m, radius 1.6m.
    let (ox, oy, oz) = (0.0, 0.0, 0.0);
    let (dir_x, dir_z) = (0.0, 1.0);
    let length = TREE_HEIGHT;
    let radius = TREE_KILL_RADIUS;

    // 1. Logger standing behind the stump is ALWAYS safe.
    assert!(!point_in_fall_sweep(0.0, 0.0, -1.5, ox, oy, oz, dir_x, dir_z, length, radius));
    assert!(!point_in_fall_sweep(0.0, 0.0, -0.1, ox, oy, oz, dir_x, dir_z, length, radius));

    // 2. Centerline hits along the trunk: mid-trunk and near the tip.
    assert!(point_in_fall_sweep(0.0, 0.0, 0.0, ox, oy, oz, dir_x, dir_z, length, radius));
    assert!(point_in_fall_sweep(0.0, 0.0, 9.0, ox, oy, oz, dir_x, dir_z, length, radius));
    assert!(point_in_fall_sweep(0.0, 0.0, 18.0, ox, oy, oz, dir_x, dir_z, length, radius));

    // 3. Past the tip is safe.
    assert!(!point_in_fall_sweep(0.0, 0.0, 18.2, ox, oy, oz, dir_x, dir_z, length, radius));

    // 4. Sideways tolerance: within radius is hit, beyond radius is safe.
    assert!(point_in_fall_sweep(1.5, 0.0, 9.0, ox, oy, oz, dir_x, dir_z, length, radius));
    assert!(point_in_fall_sweep(-1.5, 0.0, 9.0, ox, oy, oz, dir_x, dir_z, length, radius));
    assert!(!point_in_fall_sweep(1.7, 0.0, 9.0, ox, oy, oz, dir_x, dir_z, length, radius));
    assert!(!point_in_fall_sweep(-1.7, 0.0, 9.0, ox, oy, oz, dir_x, dir_z, length, radius));

    // 5. Vertical reach: jumping / slopes inside reach are hit; far overhead is safe.
    assert!(point_in_fall_sweep(0.0, 3.0, 9.0, ox, oy, oz, dir_x, dir_z, length, radius));
    assert!(point_in_fall_sweep(0.0, -3.0, 9.0, ox, oy, oz, dir_x, dir_z, length, radius));
    assert!(!point_in_fall_sweep(0.0, 10.0, 9.0, ox, oy, oz, dir_x, dir_z, length, radius));
}

#[test]
fn test_point_in_fall_sweep_diagonal_fall() {
    // Tree falling along (+X, +Z) diagonal: normalized dir is (S, S).
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let (ox, oy, oz) = (10.0, 0.0, 10.0);
    let (dir_x, dir_z) = (s, s);
    let length = 10.0;
    let radius = 1.0;

    // Point 5m along the diagonal: (10 + 5*s, 0, 10 + 5*s)
    let px = 10.0 + 5.0 * s;
    let pz = 10.0 + 5.0 * s;
    assert!(point_in_fall_sweep(px, 0.0, pz, ox, oy, oz, dir_x, dir_z, length, radius));

    // Behind origin along the diagonal is safe.
    let bx = 10.0 - 1.0 * s;
    let bz = 10.0 - 1.0 * s;
    assert!(!point_in_fall_sweep(bx, 0.0, bz, ox, oy, oz, dir_x, dir_z, length, radius));
}

#[test]
fn test_ruin_position_lands_near_stump() {
    let (rx, rz) = ruin_position(0.0, 0.0, 0.0, 1.0, 18.0);
    // At 30% of 18m = 5.4m along +Z
    assert!((rx).abs() < 1.0e-5);
    assert!((rz - 5.4).abs() < 1.0e-5);
}

#[test]
fn test_hazard_constants() {
    assert_eq!(TREE_FALL_DURATION_MS, 2500);
    assert!(TREE_HEIGHT > 10.0);
    assert!(TREE_KILL_RADIUS >= 1.0);
}
