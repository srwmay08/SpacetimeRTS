# SpacetimeRTS

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![SpacetimeDB](https://img.shields.io/badge/SpacetimeDB-v2.10-purple.svg)](https://spacetimedb.com)
[![Bevy](https://img.shields.io/badge/bevy-0.14-blue.svg)](https://bevyengine.org/)
[![Tests](https://img.shields.io/badge/tests-109%20passed-brightgreen.svg)]()
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

A multiplayer fantasy RTS/survival hybrid exploring **[SpacetimeDB](https://spacetimedb.com)** as a real-time game backend — no traditional game server or networking middleware, just an in-memory relational database running transactional WebAssembly modules.

---

## 🎮 Core Premise: The Database *Is* The Server

In conventional multiplayer architectures, developers maintain a web framework (Actix, Axum), an external database (PostgreSQL, Redis), a serialization protocol (Protobuf, FlatBuffers), and custom WebSocket/UDP connection brokers.

**SpacetimeRTS** replaces that entire stack with SpacetimeDB:
- **Tables *are* your state:** Entities, inventories, structures, and projectiles are relational rows.
- **Reducers *are* your RPCs:** Client inputs invoke deterministic, transactional functions in WebAssembly linear memory.
- **Subscriptions *are* your netcode:** Clients register SQL queries (with spatial chunk filtering) to receive real-time row diffs automatically.

```
┌────────────────────────────────────────────────────────┐
│                      Bevy Client                       │
│  - First-Person Action (WASD, Mouse, Tool Swings)      │
│  - Tactical RTS Commander View (Waypoints, Selection)  │
│  - Avian3D Physics & Client Prediction Buffer          │
└───────────────────────────▲────────────────────────────┘
                            │ WebSocket SQL Subscriptions
                            │ & Direct Reducer Calls
┌───────────────────────────▼────────────────────────────┐
│                  SpacetimeDB Backend                   │
│  - Authoritative WASM Module (Rust 2024 Edition)       │
│  - Dual-Rate Simulation: 60Hz Physics + 10Hz AI        │
│  - Lag Compensation Rewind Buffer (500ms)              │
│  - Valheim-Style Crafting, Building & Voxel Mining     │
└────────────────────────────────────────────────────────┘
```

---

## 🌟 Gameplay & Feature Catalog

### 1. Hybrid FPS / RTS Perspective
- **First-Person Survival:** Walk the world, swing stone axes and pickaxes, hunt wildlife, and place modular structures.
- **RTS Commander Mode:** Pull the camera back smoothly into an overhead tactical view to marquee-select peasants, issue waypoint orders, and coordinate base expansion.

### 2. Valheim-Style Survival & Crafting
- **Harvestable Resource Nodes:** Trees, Rocks, Flint, LooseStone, Branches, and Bushes scale dynamically with required tools (e.g., Stone Axe required for mature trees, Pickaxe for boulders).
- **Item Discovery:** Acquiring new raw materials automatically unlocks associated blueprints in the player's knowledge table (`discovered_items`).
- **10+ Authoritative Crafting Recipes:**
  - *Hand Tools & Weapons:* Hammer, Stone Axe, Pickaxe, Club, Torch, Wood Arrows.
  - *Workbench Stations:* Crude Bow, Flint Arrows, Wooden Shield, Flint Spear.
  - *Shelter Validation:* Recipes enforce proximity to active Workbenches and roof coverage (`requires_roof`).

### 3. Modular Building & Physics Stability
- **Structural Integrity Model:** Starter pieces (Foundations, Workbenches, Campfires) anchor directly to terrain with 100% stability. Child pieces decay stability hierarchically:
  $$\text{Wall: } -20\% \quad|\quad \text{Floor / Ramp: } -25\% \quad|\quad \text{Roof: } -30\%$$
- **Cascading Collapse:** If an anchor foundation or parent piece is destroyed, all unsupported child structures collapse automatically.
- **Blueprint & Repair Loop:** Structures place as translucent ghost blueprints. Players contribute construction progress using wood/stone and their Hammer. Damaged physical buildings can be repaired back to full health.

### 4. Authoritative Combat & Ballistics
- **Ballistic Simulation:** Projectiles (`ActiveProjectile`) simulate full ballistic trajectories under gravity, drag, and speed:
  - *Arrows* ($45\,\text{m/s}$, $9.81\,\text{m/s}^2$ gravity)
  - *Catapult Rocks* ($25\,\text{m/s}$, $15\,\text{m/s}^2$ gravity, area damage)
  - *Trebuchet Shells* ($35\,\text{m/s}$, $12\,\text{m/s}^2$ gravity, anti-fortification)
  - *Ballista Spears* ($60\,\text{m/s}$, high-velocity armor piercing)
  - *Magic Missiles* ($30\,\text{m/s}$, straight-line zero gravity)
- **Lag-Compensated Hit Registration:** The server maintains a 500ms `HitboxHistory` snapshot buffer, rewinding entity positions to the client's timestamp to perform anti-cheat validated swept intersection tests.

### 5. NPC Ecosystem & Autonomous Population
- **Population Manager:** Server automatically guarantees an active 16-creature perimeter around the player spawn, dynamically maintaining all mob archetypes:
  - 🦌 **Deer:** Passive herbivore, flees when startled.
  - 🐗 **Boars:** Defensive wildlife, retaliates when provoked.
  - 👺 **Goblins:** Hostile raiders that attack players and villagers on sight.
  - 🧑‍🌾 **Peasants:** Controllable friendly units capable of autonomous gather, return, and deposit resource loops.
- **Faction Diplomacy Matrix:**

| Faction | Player | Villager | Wildlife | Goblin |
|:---|:---:|:---:|:---:|:---:|
| **Player** | — | Neutral | Neutral | **KillOnSight** |
| **Villager** | Neutral | — | Neutral | **KillOnSight** |
| **Wildlife** | Neutral | Neutral | — | Neutral |
| **Goblin** | **KillOnSight** | **KillOnSight** | Neutral | — |

- **Pet System:** Companion creatures with customizable tactical stances:
  - `Stay`: Hold position unconditionally.
  - `Follow`: Track owner displacement.
  - `Defensive`: Only engage targets that deal damage to the owner.
  - `Aggressive`: Seek and destroy any `KillOnSight` hostile entering detection radius.

### 6. Procedural Terrain & 3D Voxel World
- Multi-octave deterministic Perlin heightmap featuring natural river canyons and a lake basin at $(-35, -35)$.
- 3D voxel chunk grid allowing terrain digging and filling.

---

## 🛠️ Tech Stack

| Layer | Component | Notes |
|:---|:---|:---|
| **Backend** | SpacetimeDB 2.10 | In-database Rust WASM module (`backend/spacetimedb`) |
| **Client** | Bevy 0.14 | Rust entity-component-system game engine (`client`) |
| **Physics** | Avian3D 0.1 / Rapier3D | Client prediction & authoritative server collision |
| **Pure Logic** | `spacetime-rts-logic` | Deterministic, headless game rules crate (`backend/logic`) |
| **Noise** | `noise` 0.9 | Deterministic procedural terrain generation |
| **Tests** | **109 Automated Tests** | 90 headless logic tests + 19 backend SpacetimeDB integration tests |

---

## ⚡ Quickstart

### Prerequisites

1. **Rust 1.93+** with the WebAssembly target:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```
2. **SpacetimeDB CLI 2.0+**:
   ```bash
   # Linux / macOS / WSL
   curl -sSf https://get.spacetimedb.com | sh

   # Windows (PowerShell)
   iwr -useb https://get.spacetimedb.com | iex
   ```

### 1. Launch SpacetimeDB Server
```bash
spacetime start
```
*SpacetimeDB will start locally on `http://127.0.0.1:3000`.*

### 2. Publish Backend WASM Module
```bash
cd backend/spacetimedb
spacetime publish hybrid-backend --yes
```
*To clear database state and republish from scratch:*
```bash
spacetime publish hybrid-backend --delete-data=always --yes
```

### 3. Run the Game Client
```bash
cd client
cargo run
```
*By default, the client connects to `http://localhost:3000`. To target a remote server, set:*
```bash
$env:SPACETIMEDB_URI="http://remote-server:3000"  # PowerShell
export SPACETIMEDB_URI="http://remote-server:3000" # Bash
cargo run
```

---

## 🧪 Testing Suite

SpacetimeRTS employs a decoupled testing architecture:
- **`backend/logic` (`spacetime-rts-logic`):** Completely side-effect-free, headless game logic extracted from the database. Runs in **~0.2 seconds** without needing SpacetimeDB or GPU drivers.
- **`backend/spacetimedb` (`backend`):** Tests database inventory pre-allocation, simulation determinism, and terrain height integration.

### Run All Pure Logic Tests (90 Tests)
```bash
cargo test -p spacetime-rts-logic
```

**Coverage Breakdown:**
- 📦 **Inventory & Discovery (13 tests):** Add/remove, stacking (50/slot), 16-slot limits, partial drains, discovery flags.
- 🔨 **Crafting & Recipes (7 tests):** Hand tools (Hammer, Axe, Pickaxe), Workbench requirements (Bow, Shield), shelter/roof validation (`requires_roof`), yield multipliers.
- 🌲 **Resource Nodes (13 tests):** Scale multipliers, tool requirements (Stone Axe for trees, Pickaxe for rocks), branch/flint forage, health depletion.
- 🏹 **Combat & Ballistics (16 tests):** Health clamp/heal, ray-sphere hitscan, snapshot interpolation for lag compensation, projectile trajectories with gravity vs zero-g magic missiles.
- 🏰 **Building & Blueprints (12 tests):** Grounding, stability decay, blueprint progression (0% $\to$ 100%), repair with Hammer, damage/collapse thresholds.
- 🏃 **Movement & Anti-Cheat (5 tests):** Speed clamping delta bounds, stale tick rejection.
- 🤖 **AI & Peasants (5 tests):** Carrying capacity, state equality, stuck counter and reset.
- 🐾 **Faction & Pet Diplomacy (7 tests):** Standing matrix (Ally, Neutral, KillOnSight), Pet stances (Aggressive, Defensive, Follow).
- 🎲 **Deterministic PRNG (4 tests):** Seed reproducibility, floating-point range bounds.
- ☀️ **Day / Night Cycle (4 tests):** 24-hour wrap-around, morning/noon/dusk/night thresholds.
- 🗺️ **Terrain & Spatial Grid (7 tests):** Determinism, lake basin depression, chunk quantization.

### Run Database Backend Tests (19 Tests)
```bash
cargo test -p backend
```
- Tests fixed 16-slot inventory table semantics, item removal clearing, and terrain height determinism.

---

## 📂 Project Structure

```
SpacetimeRTS/
├── backend/
│   ├── spacetimedb/              # Authoritative SpacetimeDB WASM Module
│   │   ├── src/
│   │   │   ├── lib.rs            # Core schemas, lifecycle hooks (init, connect)
│   │   │   ├── building.rs       # Blueprints, stability decay, repair, collapse
│   │   │   ├── combat.rs         # Ballistic simulation, lag compensation, hitboxes
│   │   │   ├── ai.rs             # Peasant state machines, mob brains, pet stances
│   │   │   ├── movement.rs       # Transform sync, anti-speedhack validation
│   │   │   ├── spatial.rs        # Broadphase grid queries & chunk indexing
│   │   │   ├── voxel.rs          # 3D voxel chunk manipulation
│   │   │   └── physics.rs        # Authoritative colliders & raycasting
│   │   └── tests/                # Backend database integration tests (19 tests)
│   │
│   └── logic/                    # Pure Game Logic Crate (Headless, Native)
│       ├── src/lib.rs            # Deterministic, testable game rules
│       └── tests/logic_tests.rs  # Fast integration test suite (90 tests)
│
├── client/                       # Bevy 0.14 Game Client
│   ├── src/
│   │   ├── main.rs               # App initialization & SystemSets
│   │   ├── network.rs            # SpacetimeDB connection & SQL subscriptions
│   │   ├── prediction.rs         # Client-side input prediction & reconciliation
│   │   ├── input.rs              # Action event dispatch (WASD, hotbar, clicks)
│   │   ├── camera.rs             # FPS and RTS camera controllers
│   │   ├── building.rs           # Hologram blueprints, snap sockets
│   │   ├── terrain.rs            # Procedural terrain meshing
│   │   ├── ui.rs                 # HUD, health, inventory hotbar, console
│   │   └── module_bindings/      # Generated SpacetimeDB client bindings
│   └── Cargo.toml
│
├── Cargo.toml                    # Workspace root & optimized profiles
└── README.md
```

---

## 🗺️ Roadmap

- [x] SpacetimeDB authoritative server module & lifecycle reducers
- [x] Client-side movement prediction & server reconciliation
- [x] Server-side lag compensation with 500ms rewind buffer
- [x] Modular structural building with physics stability decay & cascading collapse
- [x] Valheim-style crafting, discovery, and workstation requirements
- [x] Ballistic projectile simulation (Arrows, Catapult, Trebuchet, Ballista, Magic)
- [x] Autonomous 16-mob ecosystem & faction diplomacy matrix
- [x] Peasant resource harvesting automation & pet stance system
- [x] 109 automated tests spanning pure logic and backend database layers
- [ ] Multi-client load & stress test (16+ concurrent sessions)
- [ ] Save/snapshot database persistence script across republishes
- [ ] Authentication integration (SpacetimeAuth / OIDC)
- [ ] Animated mesh skinning for wildlife and peasants

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
