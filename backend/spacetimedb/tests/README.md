# SpacetimeRTS Integration Tests

This directory contains integration tests for the SpacetimeDB backend module.

## Approach

We test the module's pure functions and business logic directly, without requiring a running SpacetimeDB server. This gives us fast, deterministic tests for the core game rules.

For full end-to-end testing with a live database, use the SpacetimeDB CLI or run the client against a local server.

## Running Tests

```bash
cd backend/spacetimedb
cargo test
```

## Test Coverage

### Inventory Management (`inventory_tests.rs`)
- `test_add_item_to_empty_inventory` — Adding items to empty slots
- `test_add_item_stacks_existing` — Stacking items up to max (50)
- `test_add_item_creates_new_slot_when_full` — Creating new slots when stack is full
- `test_add_item_respects_max_slots` — Respecting 16-slot inventory limit
- `test_remove_item_partial` — Removing partial stack amounts
- `test_remove_item_exact` — Removing exact amount
- `test_remove_item_insufficient` — Failing when not enough items
- `test_remove_item_cleans_empty_slots` — Cleaning up empty slots after removal

### Terrain Generation (`terrain_tests.rs`)
- `test_terrain_height_deterministic` — Same input produces same output
- `test_terrain_height_always_positive` — Never returns negative/zero
- `test_terrain_height_no_nan` — Never returns NaN
- `test_terrain_height_varies` — Different positions produce different heights

### Resource Node Logic (`resource_tests.rs`)
- `test_resource_node_spawn_conditions` — Valid spawn conditions
- `test_resource_node_tool_requirements` — Tool requirements by type

### Combat Logic (`combat_tests.rs`)
- `test_health_damage_clamping` — Health never goes below 0
- `test_hitbox_sphere_intersection` — Ray-sphere hit detection math

## Future: Full Integration Tests

For tests that require a running SpacetimeDB instance:

1. Start local server: `spacetime start`
2. Publish module: `spacetime publish test-db --yes`
3. Use `spacetime call` to invoke reducers
4. Use `spacetime sql` to verify database state

Example:
```bash
spacetime call test-db client_connected
spacetime sql test-db "SELECT * FROM player"
```

This can be scripted for CI/CD pipelines.
