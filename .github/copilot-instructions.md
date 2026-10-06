# SpacetimeRTS - Copilot Instructions

All development on SpacetimeRTS is governed by the canonical instruction files in the workspace root:

- **Core Directives & Invariants:** [`AGENTS.md`](../AGENTS.md)
- **Deep Architecture & Rationale:** [`AI_RULES.md`](../AI_RULES.md)
- **Stylized Visual & Foliage Standards:** [`ART_RULES.md`](../ART_RULES.md)

## Core Guardrails
1. **SpacetimeDB Reducers:** Strictly deterministic. No OS clocks, threads, I/O, or SipHash `HashMap`/`HashSet`. Access database via `ctx.db.table_name()`, time via `ctx.timestamp`, and collections via `BTreeMap`/`BTreeSet`.
2. **Bevy 0.13 / Avian3D:** Avoid B0001 query panics; mutate `Transform` or `LinearVelocity` instead of querying components mutably alongside immutable system parameters.
3. **Frame & Rendering Budgets:** Maintain 60 FPS (16.67ms frame budget). Batch chunk grass into unified meshes per 16m chunk. Directional shadow maps max 2048x2048; cull dynamic shadows beyond 56m. Tag grass and clutter with `NotShadowCaster`.
4. **Mandatory Verification:** Ensure both crates compile cleanly and pass unit tests:
   ```bash
   cargo test -p backend
   cargo test -p client
   ```
