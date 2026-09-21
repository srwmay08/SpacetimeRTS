# SpacetimeRTS

A multiplayer fantasy RTS/survival hybrid exploring [SpacetimeDB](https://spacetimedb.com) as a game backend — no traditional game server, just a database with WASM modules.

## What is this?

SpacetimeRTS is a proof-of-concept testing whether SpacetimeDB can handle real-time multiplayer game state. The backend runs entirely inside the database as Rust WASM modules. The client is a Bevy engine app with Avian3D physics.

**Core idea:** What if your database *was* your game server?

## Current Features

### Core Gameplay
- **Hybrid FPS/RTS perspective** — play first-person or pull back to command units
- **Real-time multiplayer state sync** — SpacetimeDB subscriptions replicate state to clients
- **Client-side prediction** — input buffers with server reconciliation
- **Server-side lag compensation** — 500ms rewind buffer for hit registration
- **Resource gathering** — trees, rocks, bushes with tool requirements and respawn timers

### AI & NPCs
- **Peasant AI** — harvest, return, deposit loop with auto-gather commands
- **NPC ecosystem** — friendly villagers, deer, boars, and hostile goblins
- **Pet system** — tameable pets with stances (Stay, Follow, Aggressive, Defensive)
- **Faction system** — Player, Villager, Wildlife, Goblin with relationship matrix

### Building & Structures
- **Modular building** — foundations, walls, floors, roofs, ramps with stability decay
- **Structural collapse** — destroying a parent cascades to children
- **Terrain anchoring** — foundations must be grounded to terrain

### Networking
- **Spatial subscription culling** — clients only sync nearby chunks
- **Dual-rate server ticking** — 60Hz high-frequency + 10Hz low-frequency AI ticks
- **Configurable server URI** — `SPACETIMEDB_URI` environment variable
- **Interior culling** — occlusion-based network optimization

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | SpacetimeDB 2.10 (Rust WASM modules) |
| Client | Bevy 0.14 + Avian3D 0.1 |
| Netcode | SpacetimeDB Rust SDK |
| Physics | Avian3D (client-side prediction) |
| Testing | 78 integration tests (pure logic crate) |

## Quickstart

### Prerequisites

- Rust 1.93+ with `wasm32-unknown-unknown` target
- SpacetimeDB CLI 2.0+

```bash
# Install SpacetimeDB CLI (if not already installed)
curl -sSf https://get.spacetimedb.com | sh

# Add WASM target
rustup target add wasm32-unknown-unknown
```

### Run

```bash
# Terminal 1: Start SpacetimeDB server
spacetime start

# Terminal 2: Build and publish backend module
cd backend/spacetimedb
spacetime publish hybrid-backend --yes

# Terminal 3: Run client
cd client
cargo run
```

The client connects to `http://localhost:3000` by default. Set `SPACETIMEDB_URI` to connect to a remote server.

### Run Tests

```bash
# Run all integration tests
cargo test -p spacetime-rts-logic

# Run specific test module
cargo test -p spacetime-rts-logic inventory
cargo test -p spacetime-rts-logic faction
cargo test -p spacetime-rts-logic combat
```

## Project Structure

```
SpacetimeRTS/
├── backend/
│   ├── spacetimedb/          # SpacetimeDB WASM module
│   │   └── src/
│   │       ├── lib.rs        # Core tables, reducers, lifecycle
│   │       ├── movement.rs   # Player movement, anti-speedhack
│   │       ├── combat.rs     # Health, factions, lag compensation
│   │       ├── building.rs   # Structures, stability, collapse
│   │       └── ai.rs         # Peasant AI, NPC brains, pets
│   └── logic/                # Pure game logic crate (native testing)
│       └── src/lib.rs        # Testable game rules
├── client/
│   └── src/
│       ├── main.rs           # Bevy app setup, system scheduling
│       ├── network.rs        # SpacetimeDB connection, subscriptions
│       ├── prediction.rs     # Client-side prediction, reconciliation
│       ├── input.rs          # Action routing, context-aware dispatch
│       ├── camera.rs         # FPS/RTS perspective switching
│       ├── terrain.rs        # Procedural chunk generation
│       ├── building.rs       # Build mode, holograms, sockets
│       └── ui.rs             # HUD, inventory, action bar
└── Cargo.toml                # Workspace root
```

## Key Architectural Decisions

### Why SpacetimeDB?

Traditional game servers require:
- A web server framework (Actix, Axum, etc.)
- A database (Postgres, Redis)
- Message serialization (Protobuf, JSON)
- Connection management (WebSockets, TCP)

SpacetimeDB collapses all of that into one system. Tables *are* your state. Reducers *are* your RPCs. Subscriptions *are* your network sync. The database handles transactions, persistence, and real-time replication.

### Server Authority

The server validates everything:
- **Movement**: Delta-based with max speed clamp (anti-speedhack)
- **Combat**: Server rewinds hitboxes to client's tick for lag compensation
- **Resources**: Tool requirements, health depletion, respawn timers
- **Building**: Terrain anchoring, stability decay, structural collapse
- **AI**: State machines evaluated server-side at 10Hz

### Client Prediction

The client predicts movement locally and reconciles with server state. Input buffers track pending commands. The server echoes back `last_processed_tick` so the client can discard acknowledged inputs. Divergence beyond 0.1m triggers rollback and replay.

### Faction System

NPCs belong to factions with defined relationships:

|  | Player | Villager | Wildlife | Goblin |
|--|--------|----------|----------|--------|
| **Player** | — | Neutral | Neutral | Hostile |
| **Villager** | Neutral | — | Neutral | Hostile |
| **Wildlife** | Neutral | Neutral | — | Neutral |
| **Goblin** | Hostile | Hostile | Neutral | — |

## Testing

The project includes a pure logic crate (`backend/logic`) that extracts game rules from the WASM module for native testing. This allows fast, deterministic tests without a running SpacetimeDB server.

**Test Coverage (78 tests):**

| Module | Tests | Coverage |
|--------|-------|----------|
| Inventory | 13 | Add/remove, stacking, slot limits, partial fills |
| Terrain | 6 | Determinism, height bounds, lake depression |
| Resource Nodes | 10 | Tree/rock/bush creation, harvest yields, depletion |
| Combat | 8 | Health damage/healing, ray-sphere intersection |
| Building | 8 | Foundation placement, attachments, stability decay |
| Movement | 5 | Speed clamping, stale tick detection |
| AI | 4 | Peasant state machine, carrying capacity |
| Factions | 5 | Relationship matrix, standing evaluation |
| NPC Brains | 3 | State defaults, type equality |
| Pets | 2 | Component creation, stance management |
| PRNG | 5 | Determinism, seed variation, value ranges |

## Known Limitations

- **No persistence strategy** — module republish wipes data
- **No authentication** — anonymous identity only
- **Single-player tested** — multiplayer verified but not stress-tested
- **No README screenshots** — needs gameplay GIFs
- **Resource respawn** — only bushes respawn (trees/rocks are permanent)

## Roadmap

- [ ] Stress-test multiplayer with 10+ concurrent clients
- [ ] Resource respawn for trees/rocks
- [ ] Building placement validation (collision, slope)
- [ ] Day/night cycle
- [ ] Authentication (SpacetimeAuth or OIDC)
- [ ] Persistence strategy (snapshot/restore)
- [ ] More NPC types and behaviors
- [ ] Pet combat abilities

## Contributing

This is a learning prototype. Contributions welcome, especially:
- **Testing** — more integration tests, multiplayer verification
- **Documentation** — code comments, architecture decisions
- **Gameplay** — new features, balance tuning

## License

MIT (or whatever your brother chooses)

---

*Built with [SpacetimeDB](https://spacetimedb.com) and [Bevy](https://bevyengine.org)*
