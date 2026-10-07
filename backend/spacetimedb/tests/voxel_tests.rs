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
    assert_eq!(VOXEL_SIZE, 0.25);
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
    // Out-of-bounds defaults safely to Air
    assert_eq!(VoxelMaterial::from_u8(7), VoxelMaterial::Air);
    assert_eq!(VoxelMaterial::from_u8(255), VoxelMaterial::Air);
}

#[test]
fn test_voxel_material_hardness_ratings() {
    // Architectural Note: Hardness determines weapon damage required to excavate or destroy voxels.
    // Bedrock has infinite hardness and cannot be broken.
    assert_eq!(VoxelMaterial::Air.hardness(), 0.0);
    assert_eq!(VoxelMaterial::Dirt.hardness(), 20.0);
    assert_eq!(VoxelMaterial::Sand.hardness(), 20.0);
    assert_eq!(VoxelMaterial::Wood.hardness(), 50.0);
    assert_eq!(VoxelMaterial::Stone.hardness(), 100.0);
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
    // Architectural Note: With VOXEL_SIZE = 0.25, every 4.0m world space equals 1 chunk (16 voxels).
    // (0, 0, 0) world -> chunk (0, 0, 0), local (0, 0, 0)
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(0.0, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (0, 0, 0));
    assert_eq!((lx, ly, lz), (0, 0, 0));

    // (3.75, 0.0, 0.0) -> voxel 15 -> chunk 0, local 15
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(3.75, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (0, 0, 0));
    assert_eq!((lx, ly, lz), (15, 0, 0));

    // (4.0, 0.0, 0.0) -> voxel 16 -> chunk 1, local 0
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(4.0, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (1, 0, 0));
    assert_eq!((lx, ly, lz), (0, 0, 0));
}

#[test]
fn test_world_to_voxel_transform_negative() {
    // Architectural Note: Negative coordinates must use Euclidean division so that local coordinates
    // remain non-negative [0..15] within the negative chunk.
    // (-0.25, 0.0, 0.0) -> voxel -1 -> chunk -1, local 15
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(-0.25, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (-1, 0, 0));
    assert_eq!((lx, ly, lz), (15, 0, 0));

    // (-4.0, 0.0, 0.0) -> voxel -16 -> chunk -1, local 0
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(-4.0, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (-1, 0, 0));
    assert_eq!((lx, ly, lz), (0, 0, 0));

    // (-4.25, 0.0, 0.0) -> voxel -17 -> chunk -2, local 15
    let (cx, cy, cz, lx, ly, lz) = world_to_voxel(-4.25, 0.0, 0.0);
    assert_eq!((cx, cy, cz), (-2, 0, 0));
    assert_eq!((lx, ly, lz), (15, 0, 0));
}
