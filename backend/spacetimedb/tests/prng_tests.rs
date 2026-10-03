// ----------------------------------------------------------------------------
// DETERMINISTIC PRNG INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Validates the server-authoritative Xorshift PRNG algorithm.
// Strictly conforms to Rule 2.1 (Deterministic Randomness Only): PRNG must be
// seeded solely by stable reducer context inputs (timestamp or entity IDs),
// never using thread_rng() or OS entropy, to guarantee zero multi-node desyncs.

use backend::prng;

#[test]
fn test_prng_determinism() {
    let mut seed1: u64 = 123456789;
    let mut seed2: u64 = 123456789;

    for _ in 0..100 {
        let r1 = prng(&mut seed1);
        let r2 = prng(&mut seed2);
        assert_eq!(r1, r2, "PRNG output must be identical for identical seeds");
    }
}

#[test]
fn test_prng_seed_mutation() {
    let mut seed: u64 = 42;
    let initial_seed = seed;

    let _val = prng(&mut seed);
    assert_ne!(seed, initial_seed, "Seed must mutate after PRNG call");
}

#[test]
fn test_prng_value_range() {
    let mut seed: u64 = 987654321;

    for _ in 0..1000 {
        let val = prng(&mut seed);
        assert!(val >= 0.0 && val <= 1.0, "PRNG output {} out of [0.0, 1.0]", val);
        assert!(!val.is_nan());
        assert!(!val.is_infinite());
    }
}

#[test]
fn test_different_seeds_divergent_sequences() {
    let mut seed1: u64 = 11111;
    let mut seed2: u64 = 22222;

    let seq1: Vec<f32> = (0..20).map(|_| prng(&mut seed1)).collect();
    let seq2: Vec<f32> = (0..20).map(|_| prng(&mut seed2)).collect();

    assert_ne!(seq1, seq2, "Different initial seeds must produce different sequences");
}

#[test]
fn test_prng_distribution_buckets() {
    // Architectural Note: Basic uniformity sanity check ensuring values span
    // across all four quartiles: [0, 0.25), [0.25, 0.5), [0.5, 0.75), [0.75, 1.0].
    let mut seed: u64 = 555555;
    let mut buckets = [0u32; 4];

    for _ in 0..1000 {
        let val = prng(&mut seed);
        let bucket_idx = ((val * 4.0).floor() as usize).min(3);
        buckets[bucket_idx] += 1;
    }

    for (i, count) in buckets.iter().enumerate() {
        assert!(*count > 100, "Quartile {} received too few samples: {}", i, count);
    }
}
