<!-- Automatically discovered project instructions for Antigravity AI agents -->
<!-- Source of truth: AI_RULES.md -->

# SpacetimeRTS AI Rules & Guidelines

Please refer to and strictly follow [AI_RULES.md](file:///Users/k/Documents/GameDev/SpacetimeRTS/AI_RULES.md) for all architectural, coding, determinism, and engine conventions in this repository.

## Core Directives Summary
1. **SpacetimeDB Strict Syntax (v2.x):** Reducer context routing, SpacetimeType models, btree indexes.
2. **Rust Best Practices & Reducer Determinism:** No OS clocks, no thread_rng, no HashMaps (use BTreeMap/BTreeSet), no I/O inside reducers.
3. **Prefer Engine & Library Built-ins Over Manual Synchronization / Workarounds:**
   - **Avian3D & Bevy Physics:** Rely on built-in transform synchronization (`transform_to_position` / `position_to_transform`), collision solvers, and gravity scale. Never mutate components that built-in system parameters like `SpatialQuery` access immutably (avoids `B0001` runtime panics).
   - **Bevy Engine:** Use built-in schedules, change detection, system params, and standard window/input events rather than manual per-frame warping or custom sync loops.
   - **SpacetimeDB SDK:** Use generated table accessors, connection callbacks, and lifecycle hooks rather than ad-hoc state duplication.
   - **General Rule:** Always prefer first-party engine/library built-ins over custom manual overrides or low-level workarounds.
4. **Documentation & GitHub Commit Summaries:** In-line rationale for architectural decisions and structured commit summaries.
5. **Output Formatting:** Complete, non-truncated file updates.
