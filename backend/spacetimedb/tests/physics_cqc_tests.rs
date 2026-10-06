// ----------------------------------------------------------------------------
// CLOSE QUARTERS COMBAT & ADAPTIVE CCD PHYSICS TESTS (Rapier3D / Parry3D)
// ----------------------------------------------------------------------------
// Architectural Note: Validates the core geometric routines used for CQC:
// 1. Capsule-to-capsule contact manifold calculation for body presence separation.
// 2. Swept blade volume time-of-impact (ToI) calculation for melee cleaves.
// 3. Mathematical anti-tunneling verification for fast projectile steps.

use backend::physics::{compute_capsule_contact, check_blade_sweep_toi};

#[test]
fn test_capsule_contact_penetration_and_separation() {
    // Two player capsules: half-height 0.5, radius 0.4
    // Position 1: (0.0, 0.0, 0.0)
    // Position 2: (0.5, 0.0, 0.0) -> Distance between centers is 0.5m.
    // Sum of radii is 0.4 + 0.4 = 0.8m.
    // Expected penetration depth: 0.8m - 0.5m = 0.3m (signed distance approx -0.3m).
    let pos1 = (0.0, 0.0, 0.0);
    let pos2 = (0.5, 0.0, 0.0);

    let contact = compute_capsule_contact(pos1, 0.5, 0.4, pos2, 0.5, 0.4, 0.0);
    assert!(contact.is_some(), "Overlapping capsules must generate a contact manifold");

    let (normal, dist) = contact.unwrap();
    assert!(dist < 0.0, "Overlapping capsules must produce negative signed distance (penetration)");
    assert!((dist.abs() - 0.3).abs() < 0.05, "Penetration depth must be ~0.3m, got {}", dist.abs());

    // Normal must point along the X axis separating the capsules
    assert!(normal.0.abs() > 0.9, "Contact normal must align with the displacement axis");
}

#[test]
fn test_capsule_contact_separated_no_penetration() {
    // Two capsules separated by 2.0 meters (far greater than 0.8m combined radius)
    let pos1 = (0.0, 0.0, 0.0);
    let pos2 = (2.0, 0.0, 0.0);

    // With prediction distance 2.0m, Parry computes distance between separated capsules
    let contact = compute_capsule_contact(pos1, 0.5, 0.4, pos2, 0.5, 0.4, 2.0);
    assert!(contact.is_some());
    let (_normal, dist) = contact.unwrap();
    assert!(dist > 0.0, "Separated capsules must have positive distance, got {}", dist);
    assert!((dist - 1.2).abs() < 0.05, "Separation distance should be 2.0 - 0.8 = 1.2m");
}

#[test]
fn test_swept_blade_volume_hit_detection() {
    // Greatsword blade: half-height 0.75m, radius 0.15m (1.5m blade length)
    // Blade starting at (0.0, 0.0, 0.0), sweeping along X by 2.0m
    // Target capsule at (1.0, 0.0, 0.0), half-height 0.5m, radius 0.4m
    let blade_pos = (0.0, 0.0, 0.0);
    let blade_vel = (2.0, 0.0, 0.0);
    let target_pos = (1.0, 0.0, 0.0);

    let toi = check_blade_sweep_toi(
        blade_pos, blade_vel, 0.75, 0.15,
        target_pos, 0.5, 0.4,
        1.0,
    );

    assert!(toi.is_some(), "Swept blade must intersect target capsule along its swing path");
    let t = toi.unwrap();
    assert!(t > 0.0 && t <= 1.0, "Time of impact must be within normalized interval [0, 1], got {}", t);
}

#[test]
fn test_swept_blade_volume_miss_detection() {
    // Blade sweeping in the opposite direction (-X), target is at (+X)
    let blade_pos = (0.0, 0.0, 0.0);
    let blade_vel = (-2.0, 0.0, 0.0);
    let target_pos = (1.0, 0.0, 0.0);

    let toi = check_blade_sweep_toi(
        blade_pos, blade_vel, 0.75, 0.15,
        target_pos, 0.5, 0.4,
        1.0,
    );

    assert!(toi.is_none(), "Blade sweeping away from target must not register a hit");
}
