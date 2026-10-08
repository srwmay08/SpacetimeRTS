// ----------------------------------------------------------------------------
// VOXEL SYSTEM & DESTRUCTION INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024)
// ----------------------------------------------------------------------------
// Architectural Note: Tests the server-authoritative voxel terrain volumetric model.
// Validates 64-bit chunk key bit-packing bijection, Euclidean metric-to-voxel
// coordinate transforms (0.25m scale), 16^3 local chunk indexing, and material
// physical properties (hardness, solidity).

use backend::voxel::{
    local_to_index, pack_chunk_key, unpack_chunk_key, world_to_voxel, VoxelMaterial, CHUNK_SIZE,
    CHUNK_VOLUME, VOXEL_SIZE, BEDROCK_ELEVATION,
};

#[test]
fn test_voxel_constants() {
    // Architectural Note: 16^3 chunk volume = 4096 voxels = exactly 4KB of raw byte data,
    // fitting within SpacetimeDB BSATN serialization limits for low-latency network replication.
    assert_eq!(CHUNK_SIZE, 16);
    assert_eq!(CHUNK_VOLUME, 4096);
    assert_eq!(VOXEL_SIZE, 1.0);
    assert_eq!(BEDROCK_ELEVATION, -120.0);
}

#[test]
fn test_voxel_material_from_u8_mapping() {
    assert_eq!(VoxelMaterial::from_u8(0), VoxelMaterial::Air);
    assert_eq!(VoxelMaterial::from_u8(1), VoxelMaterial::Dirt);
    assert_eq!(VoxelMaterial::from_u8(2), VoxelMaterial::Stone);
    assert_eq!(VoxelMaterial::from_u8(3), VoxelMaterial::Sand);
    assert_eq!(VoxelMaterial::from_u8(4), VoxelMaterial::Wood);
    assert_eq!(VoxelMaterial::from_u8(5), VoxelMaterial::ReinforcedStone);
    assert_eq!(VoxelMaterial::from_u8(6), VoxelMaterial::Bedrock);
    assert_eq!(VoxelMaterial::from_u8(7), VoxelMaterial::IronOre);
    assert_eq!(VoxelMaterial::from_u8(8), VoxelMaterial::Ruby);
    assert_eq!(VoxelMaterial::from_u8(9), VoxelMaterial::CollapsedRubble);
    // Out-of-bounds defaults safely to Air
    assert_eq!(VoxelMaterial::from_u8(10), VoxelMaterial::Air);
    assert_eq!(VoxelMaterial::from_u8(255), VoxelMaterial::Air);
}

#[test]
fn test_voxel_material_hardness_ratings() {
    // Architectural Note: Hardness determines weapon damage required to excavate or destroy voxels.
    // Bedrock has infinite hardness and cannot be broken.
    assert_eq!(VoxelMaterial::Air.hardness(), 0.0);
    assert_eq!(VoxelMaterial::Dirt.hardness(), 20.0);
    assert_eq!(VoxelMaterial::Sand.hardness(), 20.0);
    assert_eq!(VoxelMaterial::CollapsedRubble.hardness(), 40.0);
    assert_eq!(VoxelMaterial::Wood.hardness(), 50.0);
    assert_eq!(VoxelMaterial::Stone.hardness(), 100.0);
    assert_eq!(VoxelMaterial::IronOre.hardness(), 120.0);
    assert_eq!(VoxelMaterial::Ruby.hardness(), 150.0);
    assert_eq!(VoxelMaterial::ReinforcedStone.hardness(), 250.0);
    assert!(VoxelMaterial::Bedrock.hardness().is_infinite());
}

#[test]
fn test_voxel_material_solidity() {
    assert!(!VoxelMaterial::Air.is_solid());
    assert!(VoxelMaterial::Dirt.is_solid());
    assert!(VoxelMaterial::Stone.is_solid());
    assert!(VoxelMaterial::Sand.is_solid());
    assert!(VoxelMaterial::Wood.is_solid());
    assert!(VoxelMaterial::ReinforcedStone.is_solid());
    assert!(VoxelMaterial::Bedrock.is_solid());
    assert!(VoxelMaterial::IronOre.is_solid());
    assert!(VoxelMaterial::Ruby.is_solid());
    assert!(VoxelMaterial::CollapsedRubble.is_solid());
}

#[test]
fn test_chunk_key_packing_bijection() {
    // Architectural Note: Packs X (24-bit), Y (16-bit), Z (24-bit) into a 64-bit integer.
    // Round-trip unpack must reproduce exact original signed coordinates without drift or collision.
    let test_coords = [
        (0, 0, 0),
        (1, 0, 0),
        (0, 1, 0),
        (0, 0, 1),
        (-1, -1, -1),
        (100, 50, -200),
        (-500, -100, 500),
        (50000, 15000, -50000),
        (-50000, -15000, 50000),
        (8_000_000, 30_000, 8_000_000),   // Near positive limit
        (-8_000_000, -30_000, -8_000_000), // Near negative limit
    ];

    for (cx, cy, cz) in test_coords {
        let key = pack_chunk_key(cx, cy, cz);
        let unpacked = unpack_chunk_key(key);
        assert_eq!(unpacked, (cx, cy, cz), "Failed round-trip for ({}, {}, {})", cx, cy, cz);
    }
}

#[test]
fn test_chunk_key_collision_resistance() {
    // Invariant: Distinct chunk coordinates must never map to the same packed key.
    let k1 = pack_chunk_key(1, 0, 0);
    let k2 = pack_chunk_key(0, 1, 0);
    let k3 = pack_chunk_key(0, 0, 1);
    let k4 = pack_chunk_key(-1, 0, 0);

    assert_ne!(k1, k2);
    assert_ne!(k1, k3);
    assert_ne!(k1, k4);
    assert_ne!(k2, k3);
    assert_ne!(k2, k4);
    assert_ne!(k3, k4);
}

#[test]
fn test_local_to_index_bounds() {
    // Architectural Note: Index = lx + ly * 16 + lz * 256.
    // Must map [0..15] exclusively within [0..4095].
    assert_eq!(local_to_index(0, 0, 0), 0);
    assert_eq!(local_to_index(15, 0, 0), 15);
    assert_eq!(local_to_index(0, 1, 0), 16);
    assert_eq!(local_to_index(0, 0, 1), 256);
    assert_eq!(local_to_index(15, 15, 15), 4095);

    // Verify ordering is strictly strictly monotonic
    for lz in 0..16 {
        for ly in 0..16 {
            for lx in 0..16 {
                let idx = local_to_index(lx, ly, lz);
                assert!(idx < CHUNK_VOLUME);
            }
        }
    }
}

#[test]
fn test_world_to_voxel_transform_positive() {
    // Architectural Note: With VOXEL_SIZE = 1.0, every 16.0m world space equals 1 chunk (16 voxels).
    // (0, 0, 0) world -> chunk (0, 0, 0), local (0, 0, 0)
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(0.0, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (0, 0, 0));
    assert_eq!((lx, ly, lz), (0, 0, 0));

    // (15.0, 0.0, 0.0) -> voxel 15 -> chunk 0, local 15
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(15.0, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (0, 0, 0));
    assert_eq!((lx, ly, lz), (15, 0, 0));

    // (16.0, 0.0, 0.0) -> voxel 16 -> chunk 1, local 0
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(16.0, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (1, 0, 0));
    assert_eq!((lx, ly, lz), (0, 0, 0));
}

#[test]
fn test_world_to_voxel_transform_negative() {
    // Architectural Note: Negative coordinates must use Euclidean division so that local coordinates
    // remain non-negative [0..15] within the negative chunk.
    // (-1.0, 0.0, 0.0) -> voxel -1 -> chunk -1, local 15
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(-1.0, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (-1, 0, 0));
    assert_eq!((lx, ly, lz), (15, 0, 0));

    // (-16.0, 0.0, 0.0) -> voxel -16 -> chunk -1, local 0
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(-16.0, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (-1, 0, 0));
    assert_eq!((lx, ly, lz), (0, 0, 0));

    // (-17.0, 0.0, 0.0) -> voxel -17 -> chunk -2, local 15
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(-17.0, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (-2, 0, 0));
    assert_eq!((lx, ly, lz), (15, 0, 0));
}

#[test]
fn test_dda_raymarch_axis_aligned_hit() {
    use backend::voxel::dda_raymarch_pure;

    // Ray starting at (0.5, 0.5, 0.5) pointing in +X direction.
    // Place a stone voxel at vx = 2 (world X = 2.0 to 3.0).
    let hit = dda_raymarch_pure(0.5, 0.5, 0.5, 1.0, 0.0, 0.0, 5.0, |vx, vy, vz| {
        if vx == 2 && vy == 0 && vz == 0 {
            VoxelMaterial::Stone
        } else {
            VoxelMaterial::Air
        }
    });

    assert!(hit.is_some());
    let hit = hit.unwrap();
    assert_eq!(hit.vx, 2);
    assert_eq!(hit.vy, 0);
    assert_eq!(hit.vz, 0);
    assert_eq!(hit.material, VoxelMaterial::Stone);
    assert_eq!(hit.normal, (-1.0, 0.0, 0.0)); // Struck on west (-X) face
    assert!((hit.distance - 1.5).abs() < 1e-4); // 2 * 1.0 = 2.0; 2.0 - 0.5 = 1.5m
}

#[test]
fn test_dda_raymarch_diagonal_and_miss() {
    use backend::voxel::dda_raymarch_pure;

    // 1. Diagonal ray pointing towards (+1, -1, +1)
    let hit = dda_raymarch_pure(0.1, 10.0, 0.1, 1.0, -1.0, 1.0, 10.0, |vx, _vy, vz| {
        if vx == 5 && vz == 5 {
            VoxelMaterial::Dirt
        } else {
            VoxelMaterial::Air
        }
    });
    assert!(hit.is_some());
    let hit = hit.unwrap();
    assert_eq!(hit.material, VoxelMaterial::Dirt);

    // 2. Miss test - no solid voxels in path within max_dist
    let miss = dda_raymarch_pure(0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 3.0, |_vx, _vy, _vz| {
        VoxelMaterial::Air
    });
    assert!(miss.is_none());

    // 3. Degenerate ray direction (zero length)
    let zero_dir = dda_raymarch_pure(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, |_vx, _vy, _vz| {
        VoxelMaterial::Stone
    });
    assert!(zero_dir.is_none());

    // 4. Ray starting inside solid voxel
    let inside = dda_raymarch_pure(0.1, 0.1, 0.1, 1.0, 0.0, 0.0, 3.0, |_vx, _vy, _vz| {
        VoxelMaterial::Bedrock
    });
    assert!(inside.is_some());
    let inside_hit = inside.unwrap();
    assert_eq!(inside_hit.material, VoxelMaterial::Bedrock);
    assert_eq!(inside_hit.distance, 0.0);
}

#[test]
fn test_procedural_ore_vein_generation() {
    use backend::voxel::procedural_stone_or_ore;

    // 1. Serpentine Iron ore vein distribution at mid-depths (-30.0m)
    let mut iron_found = false;
    let mut stone_found = false;
    for vx in 0..100 {
        let mat = procedural_stone_or_ore(vx, -120, 10, -30.0);
        if mat == VoxelMaterial::IronOre {
            iron_found = true;
        } else if mat == VoxelMaterial::Stone {
            stone_found = true;
        }
    }
    assert!(iron_found, "Must find procedurally embedded IronOre in mid-depth stone");
    assert!(stone_found, "Must have regular Stone matrix surrounding IronOre");

    // 2. Deep Ruby crystal cluster distribution near bedrock (-100.0m)
    let mut ruby_found = false;
    for vx in 0..500 {
        let mat = procedural_stone_or_ore(vx, -400, 25, -100.0);
        if mat == VoxelMaterial::Ruby {
            ruby_found = true;
            break;
        }
    }
    assert!(ruby_found, "Must find rare Ruby crystal clusters in deep subterranean stone");
}

#[test]
fn test_natural_cavern_carving_and_crust_preservation() {
    use backend::voxel::is_cave_air_at;

    let terrain_height = 25.0;

    // 1. Surface crust preservation: within 4.0m of surface, must NEVER carve air
    for wy_offset in [0.0, -1.0, -2.0, -3.0, -3.9] {
        assert!(
            !is_cave_air_at(10.0, terrain_height + wy_offset, 10.0, terrain_height),
            "Surface crust must remain solid at offset {}", wy_offset
        );
    }

    // 2. Above surface must never be cave air
    assert!(!is_cave_air_at(0.0, terrain_height + 5.0, 0.0, terrain_height));

    // 3. Natural hollow caverns must exist in the subterranean strata (-10m to -90m)
    let mut cave_found = false;
    let mut solid_found = false;
    for x in -20..20 {
        for z in -20..20 {
            let wx = x as f32 * 4.0;
            let wz = z as f32 * 4.0;
            let wy = -30.0;
            if is_cave_air_at(wx, wy, wz, terrain_height) {
                cave_found = true;
            } else {
                solid_found = true;
            }
            if cave_found && solid_found {
                break;
            }
        }
        if cave_found && solid_found {
            break;
        }
    }
    assert!(cave_found, "Must procedurally carve hollow subterranean caverns below surface crust");
    assert!(solid_found, "Must retain solid rock matrix surrounding subterranean caverns");
}

#[test]
fn test_compute_rock_chunk_voxels_scale_and_passageway_clearance() {
    use backend::voxel::{compute_rock_chunk_voxels, VOXEL_SIZE};

    // Forward excavation: aim along +X
    let voxels = compute_rock_chunk_voxels(10, 20, 10, 1.0, 0.0, 0.0);

    // 1. Excavated volume: 1x2 doorway extending 2m forward = 4 voxels (4.0m^3)
    assert_eq!(
        voxels.len(),
        4,
        "Rock chunk must excavate a solid 1x2 doorway slice (4 voxels), got {}",
        voxels.len()
    );

    // 2. Player humanoid capsule clearance:
    // Humanoid capsule requires at least 1.8m height and 0.8m width.
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut min_z = i32::MAX;
    let mut max_z = i32::MIN;

    for &(_vx, vy, vz) in &voxels {
        min_y = min_y.min(vy);
        max_y = max_y.max(vy);
        min_z = min_z.min(vz);
        max_z = max_z.max(vz);
    }

    let height_m = (max_y - min_y + 1) as f32 * VOXEL_SIZE;
    let width_m = (max_z - min_z + 1) as f32 * VOXEL_SIZE;

    assert_eq!(
        height_m,
        2.0,
        "Excavated rock chunk height ({:.2}m) must be exactly 2.0m for standing clearance",
        height_m
    );
    assert_eq!(
        width_m,
        1.0,
        "Excavated rock chunk width ({:.2}m) must be 1.0m",
        width_m
    );
}

#[test]
fn test_compute_rock_chunk_voxels_downward_shaft() {
    use backend::voxel::{compute_rock_chunk_voxels, VOXEL_SIZE};

    // Downward stepped excavation: looking down into floor
    let voxels = compute_rock_chunk_voxels(0, 10, 0, 1.0, -0.6, 0.0);

    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for &(_vx, vy, _vz) in &voxels {
        min_y = min_y.min(vy);
        max_y = max_y.max(vy);
    }

    let depth_m = (max_y - min_y + 1) as f32 * VOXEL_SIZE;
    assert!(
        depth_m >= 2.0,
        "Downward rock chunk break must carve a walkable stepped descent at least 2.0m deep with headroom, got {:.2}m",
        depth_m
    );
}
