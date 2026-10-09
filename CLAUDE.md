# SpacetimeRTS - Claude Guidelines

All architectural standards, determinism guardrails, engine conventions, and performance budgets are defined canonically in the project root:

- **Core Directives & Invariants:** [`AGENTS.md`](./AGENTS.md)
- **Deep Architecture & Rationale:** [`AI_RULES.md`](./AI_RULES.md)
- **Visual & Foliage Standards:** [`ART_RULES.md`](./ART_RULES.md)

## Key Invariants
1. **SpacetimeDB Reducers:** All reducers take `ctx: &ReducerContext`. Access tables strictly via `ctx.db.table_name()`. Zero nondeterminism: no OS clocks (`SystemTime`, `Instant`), no OS entropy (`rand::thread_rng()`), no I/O, no threads. Use `ctx.timestamp` and `BTreeMap`/`BTreeSet`.
2. **Physics & Engine Standards:** Rely on Bevy 0.13 and unified Parry3D / Rapier3D physics. Avian3D is strictly banned. Use Parry for direct narrow-phase geometry and Rapier for scene queries and kinematic movement. Never write manual height overrides that fight the solver.
3. **Avoid Bevy B0001 Panics:** Never query components mutably if built-in system parameters in the same system access them immutably.
4. **Foliage & Rendering Budgets:** Target 60 FPS (16.67ms). Batch terrain grass into unified chunk meshes (never spawn individual ECS grass entities). Execute wind displacement in GPU WGSL shaders (`ExtendedMaterial`).
5. **Mandatory Verification:** Ensure both crates compile cleanly and pass all unit tests:
   ```bash
   cargo test -p backend
   cargo test -p client
   ```
