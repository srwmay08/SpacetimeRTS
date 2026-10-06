<!-- ============================================================================ -->
<!-- SpacetimeRTS Core Directives for Autonomous AI Agents                      -->
<!-- Automatically ingested by Antigravity / AI Coding Assistants              -->
<!-- Definitive Reference: AI_RULES.md (Rust 2024 / SpacetimeDB v2 / Bevy 0.13) -->
<!-- ============================================================================ -->

# SpacetimeRTS Core Agent Instructions

You are pair-programming on **SpacetimeRTS**, a 50-player multiplayer fantasy RTS & survival game built in Rust (2024 Edition) with **SpacetimeDB v2.7+** (server-authoritative WebAssembly reducers) and **Bevy 0.13** (client engine with Avian3D physics and custom WGSL shaders).

You must strictly obey the following project expectations on every turn. Refer to [AI_RULES.md](file:///Users/k/Documents/GameDev/SpacetimeRTS/AI_RULES.md) for deep technical rationale and architecture specifications.

---

## 1. SpacetimeDB Syntax & Reducer Determinism (Non-Negotiable)
- **Strict Context Routing:** All reducers take `ctx: &ReducerContext` as parameter 1. Access DB strictly via `ctx.db.table_name()`. Custom structs in tables must derive `SpacetimeType`.
- **Zero Nondeterminism (Banned):** NEVER call OS clocks (`SystemTime`, `Instant`), OS entropy (`rand::thread_rng()`), I/O (`std::fs`, `std::net`), or threads inside reducers.
- **Mandatory Replacements:** Use `ctx.timestamp` for time, deterministic PRNGs seeded by timestamp/IDs for randomness, and `BTreeMap`/`BTreeSet` for collections (standard `HashMap`/`HashSet` SipHash iteration order is randomized and will desync nodes).

## 2. Prefer Engine & Library Built-ins Over Custom Workarounds
*"Homebrew your game, not your engine plumbing."*
- **Avian3D & Bevy Physics:** Rely on Avian's transform synchronization (`transform_to_position` / `position_to_transform`), collision solvers, and gravity scale. Never write manual height overrides that fight the solver.
- **Prevent Bevy B0001 Panics:** Never query components mutably if built-in system parameters in the same system access them immutably (e.g. `SpatialQuery` reads `&Position`; requesting `&mut Position` panics the engine). Mutate `Transform` or `LinearVelocity` instead.
- **Bevy Engine:** Rely on built-in schedules (`Update`, `FixedUpdate`), change detection (`Changed<T>`, `Added<T>`), and window events (`CursorGrabMode::Locked`) rather than manual per-frame polling loops or cursor warping.
- **SpacetimeDB SDK:** Always use generated table accessors (`conn.db.*`), lifecycle hooks (`on_insert`, `on_delete`), and connection callbacks.

## 3. Strict 60 FPS & 50-Player Performance Budgets
- **Frame Budget:** 16.67ms (60 FPS) target across all gameplay and physics systems.
- **Client ECS Entities:** Keep active visible entities in the player bubble around **2,000 – 3,500**. Use distance streaming for world nodes (load within 224m, unload past 240m).
- **Shadow Map & Fill-Rate Preservation:** Directional light shadow maps must not exceed **2048x2048**. Cull dynamic tree/bush shadows beyond **56m**. Tag grass, berries, and small ground clutter with `NotShadowCaster`.
- **World Node Cap:** Keep total world resource nodes in the database around **~8,000** (~3.5k trees, ~4.5k minerals/foraging).

## 4. Stylized Visual & Foliage Standards
- **Low-Poly Faceted Aesthetic:** Follow the BlendSwap #9440 geometric aesthetic (procedural flat-shaded polyhedral meshes, ~95% fewer polygons than micro-voxels).
- **Batch Foliage by Chunk:** Grass and ground foliage must ALWAYS be batched into unified meshes per 16m terrain chunk. **Never spawn individual ECS entities for grass blades.**
- **GPU Wind Shaders:** Execute foliage wind displacement in the WGSL vertex shader (`ExtendedMaterial`), never via CPU vertex uploads.
- **Two-Sided Foliage:** Use `cull_mode: None, double_sided: true` on flat foliage ribbons for 360° visibility and accurate celestial PBR shading on backfaces.

## 5. Mandatory Verification & Testing
- Before completing any task, you must proactively verify that both crates compile cleanly with zero warnings and pass all unit tests:
  ```bash
  cargo test -p backend
  cargo test -p client
  ```

## 6. Output Integrity & Git Protocol
- **No Laziness / No Truncation:** Always output complete, ready-to-compile files from top to bottom. Placeholders like `// ... existing code ...` are strictly forbidden.
- **In-Line Architectural Rationale:** Document the *why* for all non-obvious algorithms and structures.
- **Commit & Push:** Make clean commits adhering to Conventional Commits and keep `origin main` synchronized.
