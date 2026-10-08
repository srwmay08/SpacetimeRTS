<!-- ============================================================================ -->
<!-- SpacetimeRTS Comprehensive AI Rules, Architecture & Engineering Standards   -->
<!-- Source of Truth: AI_RULES.md (Rust 2024 / SpacetimeDB v2.7+ / Bevy 0.13)   -->
<!-- ============================================================================ -->

# SpacetimeRTS AI Architectural Blueprint & Engineering Rules

## 1. Project Context & High-Level Architecture
We are developing a multiplayer fantasy real-time strategy (RTS) and survival game. 
- **Backend:** SpacetimeDB v2.7+ server module written in Rust (2024 edition). All world simulation, transactions, combat verification, building stability, and player persistence run server-authoritatively inside WebAssembly sandbox reducers.
- **Client:** Bevy engine written in Rust, leveraging the official SpacetimeDB Rust SDK for real-time table replication, Avian3D (XPBD/Parry-based) for 3D client physics and character movement, and custom WGSL shaders for stylized low-poly PBR rendering.
- **Physics Architecture:** Avian3D runs exclusively on the Bevy client for responsive character prediction; standalone Rapier3D runs headless inside SpacetimeDB server WebAssembly reducers for authoritative hit verification and swept raycasts. Both share the exact same underlying collision kernel (Parry3D), with zero conflict.
- **Target Target:** Seamless, fluid 60 FPS (16.67ms frame budget) supporting up to 50 concurrent players on a shared 400m × 400m procedural world.

---

## 2. SpacetimeDB Strict Syntax & Database Directives (v2.x)
1. **Table Declarations & Attributes:**
   - Tables must use standard SpacetimeDB attributes: `#[spacetimedb::table(accessor = table_name, public)]`.
   - Keys and indexes must use official macros: `#[primary_key]`, `#[auto_inc]`, `#[unique]`, and `#[index(btree)]`.
   - Never use ad-hoc hash indexing inside SpacetimeDB modules; rely on BTree indexes.
2. **Data Models:**
   - All structs, enums, and sub-models stored within tables or reducer arguments must derive `SpacetimeType`.
3. **Reducer Signatures & Routing:**
   - Every reducer must take `ctx: &ReducerContext` as its very first parameter.
   - Lifecycle reducers must be properly registered: `#[spacetimedb::reducer(init)]`, `#[spacetimedb::reducer(client_connected)]`, `#[spacetimedb::reducer(client_disconnected)]`.
   - Database queries and mutations must route strictly through `ctx.db.table_name()` (e.g. `ctx.db.player().insert(...)`, `ctx.db.resource_node().node_id().find(...)`).
   - Reducers must never attempt to return arbitrary payloads to the client. State updates propagate strictly through table replication, and server logs use `log::info!`, `log::warn!`, or `log::error!`.

---

## 3. Reducer Determinism & Rollback Safety (Critical Guardrails)
SpacetimeDB guarantees multi-node transaction consistency and rollback safety by requiring all reducer executions to be strictly deterministic. Any desynchronization bug will corrupt database state silently at runtime.

1. **Strict Determinism (No OS Clocks or I/O):**
   - **Banned:** `std::time::SystemTime`, `std::time::Instant`, `std::thread`, `std::fs`, `std::net`, external HTTP requests.
   - **Mandatory:** Always read time strictly from `ctx.timestamp` (e.g. `ctx.timestamp.to_micros_since_unix_epoch()`).
2. **Deterministic Randomness Only:**
   - **Banned:** `rand::thread_rng()` or any OS entropy sources.
   - **Mandatory:** Always use deterministic pseudo-random number generators (PRNGs, such as Xorshift or PCG) seeded explicitly by `ctx.timestamp` or stable unique entity IDs.
3. **Strict Collection Iteration Ordering:**
   - **Banned:** `std::collections::HashMap` and `std::collections::HashSet`. Their standard SipHash uses randomized per-execution keys, creating nondeterministic iteration orders across nodes.
   - **Mandatory:** Always use `std::collections::BTreeMap` and `std::collections::BTreeSet` for state-affecting iteration.
4. **Statics & Global Linear Memory:**
   - Reducer rollbacks do not rewind WebAssembly global static variables. Never store authoritative game state in `static mut` or `lazy_static`.
   - Non-authoritative transient caches (e.g. spatial query grids) must be deterministically wiped and rebuilt every tick.

---

## 4. Prefer Engine & Library Built-ins Over Manual Workarounds
*The foundational rule for AI agents: "Homebrew your game, not your engine plumbing."*

1. **Avian3D & Bevy Physics Built-ins:**
   - Rely on Avian3D's built-in transform synchronization stages (`transform_to_position` in `PhysicsSet::Prepare` and `position_to_transform` in `PhysicsSet::Sync`).
   - **Never cause `B0001` aliased query panics:** Bevy panics if a system queries a component mutably while another system parameter queries it immutably. For example, Avian's `SpatialQuery` internally reads `&Position`; requesting `&mut Position` in the same system will crash the game. Query `&mut Transform` or `&mut LinearVelocity` instead.
   - Rely on Avian's built-in collision manifolds, contact resolvers, and gravity scales rather than writing manual per-frame height overrides that fight the physics solver.
2. **Bevy Engine Built-ins:**
   - Utilize standard schedules (`Update`, `FixedUpdate`, `PostUpdate`), change detection (`Changed<T>`, `Added<T>`), and system parameters (`ParamSet`, `Local`, `EventReader`) rather than hand-rolling manual dirty flags or synchronization loops.
   - Use standard window cursor grab modes (`CursorGrabMode::Locked`) and Bevy mouse motion events rather than manual OS-level cursor coordinate warping.
3. **SpacetimeDB SDK Built-ins:**
   - Always utilize official generated table accessors (`ctx.db.*`, `conn.db.*`), lifecycle callbacks (`on_insert`, `on_delete`, `on_update`), and subscription filters rather than inventing manual out-of-band networking or ad-hoc client caches.

---

## 5. Performance Budgets & Multiplayer Scalability (50 Players / 60 FPS)
All client systems must operate strictly within the **16.67ms (60 FPS)** frame budget:

1. **Entity Budgets (Client ECS):**
   - **Active Visible ECS Entities:** Maintain **$\approx 2,000 - 3,500$ entities** in the player's active view bubble. Exceeding 6,000 entities degrades Bevy's transform propagation and frustum culling.
   - **World Resource Nodes (Server DB):** Capped at **$\approx 8,000$ total nodes** across the 400m map (~3.5k trees, ~4.5k minerals/foraging). This guarantees fast client subscription sync (<180 KB snapshot) and microsecond spatial lookups.
   - **Distance Streaming:** Resource nodes must be streamed dynamically (e.g. loaded within 224m and unloaded beyond 240m with hysteresis).
2. **Shadow Caster Budget & Lighting Hierarchy:**
   - **Exclusive Directional Shadow Casters:** ONLY the two celestial directional lights (Host Star A and Companion Star B) may have `shadows_enabled: true`.
   - **Deep Rock Galactic (DRG) Performance Illusions for Local Lights:** ALL local point lights and spot lights (torches, campfires, glowing runes, projectiles, muzzle flashes, bioluminescent crystals) MUST set `shadows_enabled: false`. Dynamic illumination from local sources must be simulated using high-intensity HDR emissive materials (`LinearRgba > 2.0`), tight inverse-square attenuation radii (`range <= 16.0m`), and screen-space bloom illusions. Never spawn omnidirectional cubemap shadow passes.
   - **Near Distance Entity Cap:** Directional cascades must render at most **$\le 150$ shadow-casting entities** within near distance ($\le 56\text{m}$).
   - **Exemption Tags:** All distant foliage (>56m), ground clutter (loose rocks, flint, branches), ruby berries, and grass MUST be tagged with `NotShadowCaster` to prevent shadow cascade geometry explosion.
   - **Resolution Cap:** Directional light shadow map resolution is capped at **2048x2048** (never 4096) to preserve GPU fill-rate.
3. **Draw Calls & Chunk Batching:**
   - Draw calls per frame should stay within **300 - 700**.
   - Procedural grass and ground foliage must ALWAYS be batched into single unified meshes per 16m chunk attached to terrain entities. **Never spawn individual ECS entities for grass blades.**

---

## 6. Stylized Visual & Shading Standards
Match the established "modern retro" fantasy aesthetic (low-poly geometry with high-fidelity modern PBR lighting):
1. **Low-Poly Geometric Mesh Generators:**
   - Follow the BlendSwap #9440 faceted geometric aesthetic for trees, boulders, and props.
   - Replaced legacy dense micro-voxel grids with procedural flat-shaded polyhedral meshes (~95% fewer polygons, crisp directional facet lighting).
2. **Foliage & Grass Implementation:**
   - Grass blades use wide, fanned 4-blade star tufts (`blade_width: 0.13m`) fanned across quadrants for lush coverage with minimal geometry.
   - Use two-ring distance LOD: High LOD (200 tufts/chunk) within $\le 32\text{m}$, Low LOD (70 tufts/chunk) from $32\text{m} - 48\text{m}$, and $0$ geometry beyond 48m.
   - Enable `cull_mode: None, double_sided: true` on paper-thin foliage ribbons so they remain visible from all 360° camera angles and reflect celestial star light properly on backfaces.
   - Wind displacement must execute on the GPU via custom WGSL vertex shaders (`ExtendedMaterial`), never through CPU vertex buffer uploads.

---

## 7. Verification, Testing & Commit Standards
1. **Mandatory Verification:**
   - Every modification must compile without errors or warnings and pass all unit tests:
     ```bash
     cargo test -p backend
     cargo test -p client
     ```
2. **In-Line Architectural Rationale:**
   - Every non-trivial function, algorithm, or struct modification must have an architectural comment explaining *why* the decision was made, referencing multiplayer sync, physics stability, or GPU performance.
3. **Complete, Non-Truncated File Updates:**
   - **Strictly No Laziness:** Never output placeholders like `// ... existing code ...` or truncate files. Full, complete files must be emitted when editing or creating code.
4. **Git Commit & Push Protocol:**
   - When completing tasks or major architectural increments, proactively run `cargo test`, create a clean, descriptive git commit following Conventional Commits format, and push to `origin main`.