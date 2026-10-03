# SpacetimeRTS Backend Integration Tests

This directory contains deterministic integration tests for the SpacetimeDB backend server module.

## Approach

We test pure functions, spatial indexers, mathematical algorithms, and state machines directly without requiring a running SpacetimeDB server. This provides ultra-fast, deterministic verification of core game rules, rollback safety, and anti-cheat constraints.

For full end-to-end testing with a live database, use the SpacetimeDB CLI or run the client against a local server instance.

## Running Tests

Run all backend integration tests:
```bash
cargo test -p backend
```

Run a specific test suite:
```bash
cargo test -p backend --test inventory_tests
cargo test -p backend --test combat_tests
cargo test -p backend --test building_tests
cargo test -p backend --test voxel_tests
```

## Test Suites & Coverage

### 1. Inventory Management (`inventory_tests.rs`)
- `test_ensure_inventory_capacity_pads_to_16_slots` — Padded 16-slot array allocation
- `test_add_item_to_empty_inventory` — Adding items to slot 0 with 16-slot padding
- `test_add_item_tracks_discovered_items` — Discovery list tracking for recipe unlocks
- `test_add_item_empty_or_zero_ignored` — Input sanitization for empty strings or 0 amounts
- `test_add_item_stacks_existing` — Stacking existing items up to 50 max stack
- `test_add_item_creates_new_slot_when_stack_full` — Overflowing stacks into new slots
- `test_add_item_respects_max_slots` — Rejection when all 16 slots are full
- `test_add_item_partial_fill_when_slots_full` — Top-off behavior on nearly full inventories
- `test_has_item_queries` — Multi-slot quantity validation
- `test_remove_item_partial` — Partial stack reduction
- `test_remove_item_exact` — Clearing slot data while maintaining 16-slot capacity
- `test_remove_item_insufficient` — Atomic rollback when insufficient items exist
- `test_remove_item_across_multiple_slots` — Multi-stack drain traversal
- `test_remove_item_clears_slot_data_while_preserving_16_slots` — Empty slot metadata reset

### 2. Resource Node Mechanics (`resource_tests.rs`)
- `test_resource_node_spawn_conditions` — Node initialization with scale and tools
- `test_resource_node_tool_requirements` — Stone Axe (Tree) and Pickaxe (Rock) enforcement
- `test_tree_yield_calculation_by_scale` — `ceil(6.0 * scale)` Wood yield formula
- `test_rock_yield_calculation_by_scale` — `ceil(4.0 * scale)` Stone yield formula
- `test_tree_harvest_resin_bonus` — Bonus Resin drop when using Stone Axe
- `test_bush_berry_harvest_and_depletion` — Berry harvesting and health depletion
- `test_ground_resource_collection` — Hand gathering for Branches, Flint, LooseStone
- `test_multi_hit_resource_health_depletion` — Progressive durability damage

### 3. Combat & Ballistics (`combat_tests.rs`)
- `test_health_damage_and_clamping` — Damage clamping to 0.0 and heal clamping to max
- `test_faction_standing_matrix` — Player, Villager, Wildlife, and Goblin hostility
- `test_arrow_projectile_ballistics_step` — Trajectory simulation with drag and gravity
- `test_siege_projectile_attributes` — Catapult and Trebuchet blast radii for voxel deformation
- `test_hitbox_history_snapshot_ring_buffer` — 10-snapshot history capacity enforcement
- `test_lag_compensation_closest_snapshot_selection` — Finding closest client tick snapshot
- `test_firing_vector_normalization_check` — Near-zero vector rejection

### 4. Modular Building & Stability (`building_tests.rs`)
- `test_piece_max_health_presets` — Foundation (400), Wall (200), Ramp (250), Floor/Roof (150)
- `test_decay_penalties_by_piece_type` — Ground (0), Wall (20), Floor (25), Roof (30), Ramp (25)
- `test_stability_propagation_chain` — Stability decay chain and structural failure thresholds
- `test_construction_swing_cost_division` — 4-swing cost divisibility (25% per swing)
- `test_construction_health_scaling` — Health scaling proportionally with construction progress
- `test_cascading_destruction_queue` — BFS cascade destruction of dependent child structures
- `test_starter_piece_workbench_exemption` — Foundation, Workbench, Campfire placement rules
- `test_workbench_proximity_radius` — 20m proximity requirement for advanced pieces
- `test_roof_coverage_calculation` — 3D bounding check for roof weather/rain protection
- `test_structure_repair_validation` — Repairing damaged structures vs blueprint rejection

### 5. Voxel Volumetric World (`voxel_tests.rs`)
- `test_voxel_constants` — 16^3 chunk dimensions (4KB) and 0.25m metric voxel scale
- `test_voxel_material_from_u8_mapping` — Byte-to-material enum mapping
- `test_voxel_material_hardness_ratings` — Material hardness ratings and Bedrock infinity
- `test_voxel_material_solidity` — Solid vs Air collision classification
- `test_chunk_key_packing_bijection` — 64-bit bijective coordinate packing round-trip
- `test_chunk_key_collision_resistance` — Distinct coordinates producing unique keys
- `test_local_to_index_bounds` — Contiguous 0..4095 array indexing
- `test_world_to_voxel_transform_positive` — Positive coordinate metric-to-voxel mapping
- `test_world_to_voxel_transform_negative` — Euclidean division for negative coordinate voxel mapping

### 6. Spatial Hash Grid (`spatial_tests.rs`)
- `test_spatial_grid_constants` — 50.0m cell size alignment with chunks
- `test_world_to_cell_mapping_positive` — World-to-cell projection for positive coordinates
- `test_world_to_cell_mapping_negative` — World-to-cell projection for negative coordinates
- `test_radius_cell_span_calculation` — Query window cell span calculations
- `test_spatial_grid_btreemap_determinism` — Deterministic BTreeMap ordering across nodes
- `test_distance_squared_filtering` — Precise Euclidean distance squared pruning

### 7. Authoritative Movement & Anti-Cheat (`movement_tests.rs`)
- `test_stale_tick_rejection` — Historical or duplicate client tick rejection
- `test_movement_within_speed_limit_unmodified` — Legitimate player movement acceptance
- `test_speed_hack_clamping` — Throttling excessive deltas to 22.0 m/s ceiling
- `test_multitick_catchup_limit` — 20-tick clamp on network lag burst catchups
- `test_ground_elevation_clamping` — Snapping subterranean positions to terrain + 1.05m
- `test_chunk_coordinate_calculation` — Continuous position to 50m chunk indexing

### 8. Crafting & Recipes (`crafting_tests.rs`)
- `test_craft_hammer_hand_recipe` — Hand-crafting recipes requiring no station
- `test_craft_insufficient_materials_rejected` — Atomic failure on missing ingredients
- `test_craft_workbench_station_requirement` — Workbench proximity gating
- `test_craft_batch_item_output_yield` — Multi-item outputs (e.g. 20x arrows)

### 9. AI & NPC Behaviors (`ai_tests.rs`)
- `test_spatial_chunk_adjacency` — Chebyshev distance <= 1 chunk proximity queries
- `test_peasant_initial_defaults` — Default state, owner, and gather attributes
- `test_peasant_state_transitions` — Transitions between MoveTo, Harvest, Return, AutoGather
- `test_peasant_carrying_capacity` — Maximum carry threshold validation
- `test_pet_stance_variants` — Follow, Stay, Aggressive, Defensive pet stances
- `test_npc_brain_states_and_types` — Threat states (Fleeing, Chasing, Attacking) and creature types

### 10. Deterministic PRNG (`prng_tests.rs`)
- `test_prng_determinism` — Identical pseudo-random sequences for identical seeds
- `test_prng_seed_mutation` — Internal state advancement on each query
- `test_prng_value_range` — Strict `[0.0, 1.0]` floating point boundary enforcement
- `test_different_seeds_divergent_sequences` — Divergence across different seeds
- `test_prng_distribution_buckets` — Uniformity sanity check across quartiles

### 11. Terrain Generation (`terrain_tests.rs`)
- `test_terrain_height_deterministic` — Identical input yields identical height
- `test_terrain_height_always_positive` — Continuous ground level above water/zero
- `test_terrain_height_no_nan` — Protection against NaN or Inf floating point errors
- `test_terrain_height_varies` — Non-flat organic landscape evaluation
- `test_terrain_height_reasonable_range` — Elevation bounds within expected amplitudes
- `test_terrain_height_lake_depression` — Procedural lake basin elevation depression

### 12. Simulation Regression (`simulation_tests.rs`)
- `test_terrain_height_determinism` — Frame-to-frame noise consistency
- `test_inventory_add_remove_atomic` — Multi-item add and drain atomicity
- `test_stability_decay_chain` — 4-piece vertical structural stability cascade
