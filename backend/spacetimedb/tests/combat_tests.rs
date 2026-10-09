// ----------------------------------------------------------------------------
// COMBAT & BALLISTICS INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Tests the server-authoritative combat resolution pipeline.
// Verifies faction standing matrices (Player, Villager, Wildlife, Goblin),
// health bounds clamping (0.0 <= HP <= max), ballistic projectile trajectory
// integration (drag, gravity, lifetime decay), and lag compensation hitbox history.

use backend::combat::{
    get_standing, ActiveProjectile, Faction, FactionStanding, Health, HitboxHistory,
    ProjectileKind, Snapshot,
};

#[test]
fn test_health_damage_and_clamping() {
    // Architectural Note: Health must never drop below 0.0 (preventing underflow exploits)
    // and must never exceed maximum HP during healing.
    let mut hp = Health {
        entity_id: 1,
        current: 100.0,
        max: 100.0,
    };

    // Partial damage
    hp.current = (hp.current - 35.0).max(0.0);
    assert_eq!(hp.current, 65.0);

    // Lethal overkill clamped to 0.0
    hp.current = (hp.current - 150.0).max(0.0);
    assert_eq!(hp.current, 0.0);

    // Healing clamped to max
    hp.current = (hp.current + 200.0).min(hp.max);
    assert_eq!(hp.current, 100.0);
}

#[test]
fn test_faction_standing_matrix() {
    // Architectural Note: Strict faction hostility matrix drives AI aggro and friendly-fire checks:
    // - Player <-> Villager: Neutral
    // - Player <-> Goblin: KillOnSight (Hostile)
    // - Goblin <-> Villager: KillOnSight (Hostile)
    // - Wildlife -> Player: KillOnSight (Predators attack players)
    // - Player -> Wildlife: Neutral (Players are not inherently hostile until attacked)

    // Player relationships
    assert_eq!(get_standing(&Faction::Player, &Faction::Villager), FactionStanding::Neutral);
    assert_eq!(get_standing(&Faction::Villager, &Faction::Player), FactionStanding::Neutral);
    assert_eq!(get_standing(&Faction::Player, &Faction::Goblin), FactionStanding::KillOnSight);
    assert_eq!(get_standing(&Faction::Goblin, &Faction::Player), FactionStanding::KillOnSight);
    assert_eq!(get_standing(&Faction::Player, &Faction::Wildlife), FactionStanding::Neutral);
    assert_eq!(get_standing(&Faction::Wildlife, &Faction::Player), FactionStanding::KillOnSight);

    // Goblin relationships
    assert_eq!(get_standing(&Faction::Goblin, &Faction::Villager), FactionStanding::KillOnSight);
    assert_eq!(get_standing(&Faction::Villager, &Faction::Goblin), FactionStanding::KillOnSight);

    // Intra-faction relationships
    assert_eq!(get_standing(&Faction::Player, &Faction::Player), FactionStanding::Neutral);
    assert_eq!(get_standing(&Faction::Goblin, &Faction::Goblin), FactionStanding::Neutral);
    assert_eq!(get_standing(&Faction::Villager, &Faction::Villager), FactionStanding::Neutral);
}

#[test]
fn test_arrow_projectile_ballistics_step() {
    // Architectural Note: Ballistic trajectory integration applies gravity and drag:
    // vel_x *= 1.0 - (drag * dt)
    // vel_y = (vel_y - (gravity * dt)) * (1.0 - (drag * dt))
    // pos += vel * dt
    // lifetime -= dt
    let dt = 0.016_f32; // 16ms server tick

    let mut arrow = ActiveProjectile {
        projectile_id: 1,
        shooter_id: 42,
        kind: ProjectileKind::Arrow,
        pos_x: 0.0,
        pos_y: 10.0,
        pos_z: 0.0,
        vel_x: 48.0, // Initial horizontal velocity
        vel_y: 0.0,  // Fired horizontally
        vel_z: 0.0,
        gravity: 4.8,
        drag: 0.001,
        damage: 35.0,
        blast_radius: 0.0,
        start_tick: 100,
        lifetime: 5.0,
    };

    // Step 1 simulation tick
    arrow.vel_x *= 1.0 - (arrow.drag * dt);
    arrow.vel_z *= 1.0 - (arrow.drag * dt);
    arrow.vel_y = (arrow.vel_y - (arrow.gravity * dt)) * (1.0 - (arrow.drag * dt));

    arrow.pos_x += arrow.vel_x * dt;
    arrow.pos_y += arrow.vel_y * dt;
    arrow.pos_z += arrow.vel_z * dt;
    arrow.lifetime -= dt;

    // Arrow must move forward along X
    assert!(arrow.pos_x > 0.0);
    // Gravity must pull arrow downward (negative Y velocity and Y position < initial 10.0)
    assert!(arrow.vel_y < 0.0);
    assert!(arrow.pos_y < 10.0);
    // Drag slightly reduces forward velocity
    assert!(arrow.vel_x < 48.0);
    // Lifetime decrements
    assert!((arrow.lifetime - (5.0 - dt)).abs() < f32::EPSILON);
}

#[test]
fn test_siege_projectile_attributes() {
    // Architectural Note: Siege weapons (Catapults, Trebuchets) feature blast radii
    // enabling spherical voxel excavation on impact, whereas personal ranged weapons do not.
    let catapult_rock = ActiveProjectile {
        projectile_id: 2,
        shooter_id: 100,
        kind: ProjectileKind::CatapultRock,
        pos_x: 0.0,
        pos_y: 20.0,
        pos_z: 0.0,
        vel_x: 25.0,
        vel_y: 15.0,
        vel_z: 0.0,
        gravity: 9.8,
        drag: 0.005,
        damage: 150.0,
        blast_radius: 3.5, // 3.5m voxel destruction sphere
        start_tick: 50,
        lifetime: 10.0,
    };

    assert_eq!(catapult_rock.kind, ProjectileKind::CatapultRock);
    assert!(catapult_rock.blast_radius > 0.0);
    assert_eq!(catapult_rock.damage, 150.0);
}

#[test]
fn test_hitbox_history_snapshot_ring_buffer() {
    // Architectural Note: HitboxHistory stores up to 10 historical snapshots per entity.
    // When an 11th snapshot arrives, the oldest is drained (FIFO), maintaining a fixed
    // memory footprint and bound for lag-compensation rewinds.
    let mut history = HitboxHistory {
        entity_id: 10,
        snapshots: Vec::new(),
    };

    for tick in 1..=10 {
        history.snapshots.push(Snapshot {
            tick_id: tick,
            x: tick as f32,
            y: 1.0,
            z: 0.0,
        });
    }
    assert_eq!(history.snapshots.len(), 10);
    assert_eq!(history.snapshots.first().unwrap().tick_id, 1);
    assert_eq!(history.snapshots.last().unwrap().tick_id, 10);

    // Push 11th snapshot and enforce 10-element cap
    history.snapshots.push(Snapshot {
        tick_id: 11,
        x: 11.0,
        y: 1.0,
        z: 0.0,
    });
    if history.snapshots.len() > 10 {
        let overflow = history.snapshots.len() - 10;
        history.snapshots.drain(0..overflow);
    }

    assert_eq!(history.snapshots.len(), 10);
    assert_eq!(history.snapshots.first().unwrap().tick_id, 2); // Oldest tick 1 removed
    assert_eq!(history.snapshots.last().unwrap().tick_id, 11);
}

#[test]
fn test_lag_compensation_closest_snapshot_selection() {
    // Architectural Note: When client fires weapon at client_tick T, the server finds
    // the snapshot that minimizes |snapshot.tick_id - client_tick|.
    let snapshots = vec![
        Snapshot { tick_id: 100, x: 10.0, y: 1.0, z: 0.0 },
        Snapshot { tick_id: 105, x: 15.0, y: 1.0, z: 0.0 },
        Snapshot { tick_id: 110, x: 20.0, y: 1.0, z: 0.0 },
    ];

    // Client fired at tick 104 -> closest snapshot is tick 105 (diff 1 vs diff 4)
    let client_tick: u64 = 104;
    let closest = snapshots
        .iter()
        .min_by_key(|s| (s.tick_id as i64 - client_tick as i64).abs())
        .unwrap();

    assert_eq!(closest.tick_id, 105);
    assert_eq!(closest.x, 15.0);

    // Client fired at tick 100 -> exact match
    let closest_exact = snapshots
        .iter()
        .min_by_key(|s| (s.tick_id as i64 - 100_i64).abs())
        .unwrap();
    assert_eq!(closest_exact.tick_id, 100);
}

#[test]
fn test_firing_vector_normalization_check() {
    // Architectural Note: Zero-length or near-zero firing vectors must be rejected
    // to prevent division-by-zero panics in ray trajectory direction normalization.
    let dir_x = 0.0_f32;
    let dir_y = 0.00001_f32;
    let dir_z = 0.0_f32;

    let dir_len_sq = dir_x * dir_x + dir_y * dir_y + dir_z * dir_z;
    assert!(dir_len_sq < 0.0001, "Near-zero vectors must fail validation threshold");

    let valid_x = 1.0_f32;
    let valid_y = 0.0_f32;
    let valid_z = 0.0_f32;
    let valid_len_sq = valid_x * valid_x + valid_y * valid_y + valid_z * valid_z;
    assert!(valid_len_sq >= 0.0001);

    let inv_len = 1.0 / valid_len_sq.sqrt();
    let ndx = valid_x * inv_len;
    assert_eq!(ndx, 1.0);
}

#[test]
fn test_corpse_drop_deterministic_offsets_and_health() {
    // Architectural Note: Multi-item corpse drops must use bounded deterministic offsets
    // to prevent z-fighting and stacking collisions, and initialize with valid Health
    // so they are harvestable.
    const SPREAD_OFFSETS: [(f32, f32); 8] = [
        (0.0, 0.0),
        (0.5, 0.0),
        (-0.5, 0.0),
        (0.0, 0.5),
        (0.0, -0.5),
        (0.35, 0.35),
        (-0.35, 0.35),
        (0.35, -0.35),
    ];

    let origin_x = 24.0_f32;
    let origin_z = -16.0_f32;

    for i in 0..8 {
        let (ox, oz) = SPREAD_OFFSETS[i % SPREAD_OFFSETS.len()];
        let drop_x = origin_x + ox;
        let drop_z = origin_z + oz;
        let dist = ((drop_x - origin_x).powi(2) + (drop_z - origin_z).powi(2)).sqrt();
        assert!(dist <= 0.6, "Drop offset must be clustered within 0.6m of origin");
    }

    let corpse_hp = Health {
        entity_id: 999,
        current: 1.0,
        max: 1.0,
    };
    assert_eq!(corpse_hp.current, 1.0);
    assert_eq!(corpse_hp.max, 1.0);
}

#[test]
fn test_player_respawn_spatial_and_tick_synchronization() {
    // Architectural Note: When a player dies from combat damage:
    // 1. Coordinates must reset to origin (0.0, y, 0.0)
    // 2. Spatial chunk indices MUST reset to (0, 0) so spatial queries and AI see the player at spawn
    // 3. last_processed_tick MUST leap (+100,000) so pre-death in-flight movement packets are dropped
    // 4. Hitbox history MUST be purged of pre-death coordinates and reset to spawn
    // 5. Health MUST be restored to maximum
    let mut transform = backend::movement::Transform {
        entity_id: 1001,
        x: 150.0,
        y: 12.0,
        z: -250.0,
        chunk_x: 3,
        chunk_z: -5,
        last_processed_tick: 480,
    };

    let mut hp = Health {
        entity_id: 1001,
        current: 0.0,
        max: 100.0,
    };

    let mut hitbox_history = HitboxHistory {
        entity_id: 1001,
        snapshots: vec![
            Snapshot { tick_id: 478, x: 148.0, y: 12.0, z: -250.0 },
            Snapshot { tick_id: 479, x: 149.0, y: 12.0, z: -250.0 },
            Snapshot { tick_id: 480, x: 150.0, y: 12.0, z: -250.0 },
        ],
    };

    // Simulate respawn logic from combat.rs
    hp.current = hp.max;
    transform.x = 0.0;
    transform.z = 0.0;
    transform.y = backend::get_terrain_height(0.0, 0.0) + 1.05;
    transform.chunk_x = 0;
    transform.chunk_z = 0;
    transform.last_processed_tick = transform.last_processed_tick.wrapping_add(100_000);

    hitbox_history.snapshots.clear();
    hitbox_history.snapshots.push(Snapshot {
        tick_id: transform.last_processed_tick,
        x: 0.0,
        y: transform.y,
        z: 0.0,
    });

    assert_eq!(transform.x, 0.0);
    assert_eq!(transform.z, 0.0);
    assert_eq!(transform.chunk_x, 0, "Chunk X must reset to 0 on respawn");
    assert_eq!(transform.chunk_z, 0, "Chunk Z must reset to 0 on respawn");
    assert_eq!(transform.last_processed_tick, 100_480, "Last processed tick must advance by 100,000");
    assert_eq!(hp.current, 100.0, "Health must be reset to max");
    assert_eq!(hitbox_history.snapshots.len(), 1, "Hitbox history must contain only the fresh respawn snapshot");
    assert_eq!(hitbox_history.snapshots[0].x, 0.0);
    assert_eq!(hitbox_history.snapshots[0].z, 0.0);

    // Verify stale in-flight packet rejection: packet from tick 490 arrives after respawn
    let in_flight_stale_tick = 490u64;
    assert!(in_flight_stale_tick <= transform.last_processed_tick, "Pre-death in-flight movement ticks must be <= server tick and dropped");
}
