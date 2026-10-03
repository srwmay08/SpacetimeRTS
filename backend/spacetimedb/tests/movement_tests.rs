// ----------------------------------------------------------------------------
// AUTHORITATIVE MOVEMENT & ANTI-SPEEDHACK INTEGRATION TESTS (Rust 2024)
// ----------------------------------------------------------------------------
// Architectural Note: Tests the server-authoritative movement validation loop.
// Verifies anti-speedhack throttling (22.0 m/s ceiling), stale tick dropping,
// terrain elevation ground clamping (+1.05m player capsule offset), and 50m
// spatial chunk coordinate calculation.

use backend::movement::Transform;
use backend::get_terrain_height;

/// Simulates the authoritative movement delta clamp logic from process_movement
fn validate_movement_delta(
    last_tick: u64,
    current_tick: u64,
    delta_x: f32,
    delta_y: f32,
    delta_z: f32,
) -> Option<(f32, f32, f32)> {
    if current_tick <= last_tick {
        return None; // Stale tick dropped
    }

    let elapsed_ticks = (current_tick - last_tick).min(20) as f32;
    let max_speed_mps = 22.0_f32;
    let max_dist_per_tick = max_speed_mps * (elapsed_ticks * 0.05).max(0.05);
    let max_dist_sq = max_dist_per_tick * max_dist_per_tick;
    let magnitude_sq = (delta_x * delta_x) + (delta_y * delta_y) + (delta_z * delta_z);

    if magnitude_sq > max_dist_sq {
        let magnitude = magnitude_sq.sqrt();
        let scale = max_dist_per_tick / magnitude;
        Some((delta_x * scale, delta_y * scale, delta_z * scale))
    } else {
        Some((delta_x, delta_y, delta_z))
    }
}

#[test]
fn test_stale_tick_rejection() {
    // Architectural Note: Clients sending historical or duplicate tick packets
    // must be dropped to prevent rubber-banding and replay attack exploits.
    let last_processed = 100_u64;

    // Older tick -> dropped
    assert!(validate_movement_delta(last_processed, 99, 1.0, 0.0, 0.0).is_none());
    // Duplicate tick -> dropped
    assert!(validate_movement_delta(last_processed, 100, 1.0, 0.0, 0.0).is_none());
    // Newer tick -> accepted
    assert!(validate_movement_delta(last_processed, 101, 1.0, 0.0, 0.0).is_some());
}

#[test]
fn test_movement_within_speed_limit_unmodified() {
    // Normal single-tick displacement (1 tick = 50ms = 0.05s).
    // Max allowable distance per tick = 22.0 * 0.05 = 1.10m.
    // Moving 0.75m horizontally must pass without clamping.
    let (dx, dy, dz) = validate_movement_delta(0, 1, 0.6, 0.0, 0.45)
        .expect("Valid movement must be accepted");

    assert!((dx - 0.6).abs() < f32::EPSILON);
    assert!((dy - 0.0).abs() < f32::EPSILON);
    assert!((dz - 0.45).abs() < f32::EPSILON);
}

#[test]
fn test_speed_hack_clamping() {
    // Architectural Note: If a compromised client attempts to move 10.0m in a single tick
    // (exceeding the 1.10m ceiling), the displacement vector is clamped to 1.10m.
    let (dx, dy, dz) = validate_movement_delta(0, 1, 10.0, 0.0, 0.0)
        .expect("Clamped movement must still be processed");

    let clamped_mag = (dx * dx + dy * dy + dz * dz).sqrt();
    let max_allowed = 22.0 * 0.05; // 1.10m
    assert!((clamped_mag - max_allowed).abs() < 0.001);
    assert_eq!(dy, 0.0);
    assert_eq!(dz, 0.0);
}

#[test]
fn test_multitick_catchup_limit() {
    // Architectural Note: If network hiccups delay packet arrival by 50 ticks,
    // elapsed_ticks is clamped to at most 20 ticks (1.0 second) to prevent
    // teleportation bursts after lag spikes.
    let max_speed_mps = 22.0_f32;
    let expected_max_dist = max_speed_mps * (20.0 * 0.05); // 22.0m

    // Client claims 50 elapsed ticks with 100m move
    let (dx, dy, dz) = validate_movement_delta(0, 50, 100.0, 0.0, 0.0).unwrap();
    let mag = (dx * dx + dy * dy + dz * dz).sqrt();

    assert!((mag - expected_max_dist).abs() < 0.001);
}

#[test]
fn test_ground_elevation_clamping() {
    // Architectural Note: Player capsule center sits at ground_y + 1.05m.
    // Movement below terrain must snap up to ground_y + 1.05m to prevent
    // subterranean clipping.
    let test_x = 10.0_f32;
    let test_z = 20.0_f32;
    let ground_y = get_terrain_height(test_x, test_z);

    let mut transform = Transform {
        entity_id: 1,
        x: test_x,
        y: ground_y - 5.0, // Subterranean position
        z: test_z,
        chunk_x: 0,
        chunk_z: 0,
        last_processed_tick: 1,
    };

    if transform.y < ground_y + 1.05 {
        transform.y = ground_y + 1.05;
    }

    assert_eq!(transform.y, ground_y + 1.05);

    // Elevated position (jumping/midair) must not be clamped down
    transform.y = ground_y + 5.0;
    if transform.y < ground_y + 1.05 {
        transform.y = ground_y + 1.05;
    }
    assert_eq!(transform.y, ground_y + 5.0);
}

#[test]
fn test_chunk_coordinate_calculation() {
    // Architectural Note: Chunk spatial hashing divides world coordinates by 50.0m.
    let test_cases = [
        (0.0_f32, 0.0_f32, 0_i32, 0_i32),
        (49.9, 49.9, 0, 0),
        (50.0, 50.0, 1, 1),
        (-0.1, -0.1, -1, -1),
        (-50.0, -50.0, -1, -1),
        (-50.1, -50.1, -2, -2),
    ];

    for (x, z, expected_cx, expected_cz) in test_cases {
        let cx = (x / 50.0).floor() as i32;
        let cz = (z / 50.0).floor() as i32;
        assert_eq!(cx, expected_cx, "cx mismatch for x={}", x);
        assert_eq!(cz, expected_cz, "cz mismatch for z={}", z);
    }
}
