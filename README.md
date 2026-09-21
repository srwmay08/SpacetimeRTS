# SpacetimeRTS

A multiplayer fantasy RTS/survival hybrid prototype exploring [SpacetimeDB](https://spacetimedb.com) as a game backend — no traditional game server, just a database with WASM modules.

## What is this?

SpacetimeRTS is a proof-of-concept that tests whether SpacetimeDB can handle real-time multiplayer game state. The backend runs entirely inside the database as Rust WASM modules. The client is a Bevy engine app with Avian3D physics.

**Core idea:** What if your database *was* your game server?

## Current Features

- **Hybrid FPS/RTS perspective** — play first-person or pull back to command units
- **Real-time multiplayer state sync** — SpacetimeDB subscriptions replicate state to clients
- **Client-side prediction** — input buffers with server reconciliation
- **Server-side lag compensation** — 500ms rewind buffer for hit registration
- **AI peasants** — harvest, return, deposit loop with auto-gather commands
- **Resource gathering** — trees, rocks, bushes with tool requirements
- **Modular building** — foundations, walls, floors, roofs with stability decay
- **Spatial subscription culling** — clients only sync nearby chunks
- **Dual-rate server ticking** — 60Hz high-frequency + 10Hz low-frequency AI ticks

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | SpacetimeDB 2.10 (Rust WASM modules) |
| Client | Bevy 0.14 + Avian3D 0.1 |
| Netcode | SpacetimeDB Rust SDK |
| Physics | Avian3D (client-side prediction) |

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

The client connects to `http://localhost:3000` by default. A token is saved to `stdb_token.txt` for reconnection.

## Project Structure

```
SpacetimeRTS/
├── backend/
│   └── spacetimedb/          # SpacetimeDB WASM module
│       └── src/
│           ├── lib.rs        # Core tables, reducers, lifecycle
│           ├── movement.rs   # Player movement, anti-speedhack
│           ├── combat.rs     # Health, lag compensation, hitscan
│           ├── building.rs   # Structures, stability, collapse
│           └── ai.rs         # Peasant AI state machine
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

### Client Prediction

The client predicts movement locally and reconciles with server state. Input buffers track pending commands. The server echoes back `last_processed_tick` so the client can discard acknowledged inputs.

## Known Limitations

- **No persistence strategy** — module republish wipes data
- **No authentication** — anonymous identity only
- **No tests** — zero integration or unit tests
- **Hardcoded localhost** — no config for remote servers
- **Single-player tested** — multiplayer not verified with concurrent clients
- **No README screenshots** — needs gameplay GIFs

## Roadmap

- [ ] Verify multiplayer with concurrent clients
- [ ] Add integration tests for reducers
- [ ] Resource respawn for trees/rocks (only bushes respawn)
- [ ] Building placement validation (collision, slope)
- [ ] Enemy AI (combat, not just gathering)
- [ ] Day/night cycle
- [ ] Configurable server URI
- [ ] Authentication (SpacetimeAuth or OIDC)

## Contributing

This is a learning prototype. Contributions welcome, especially:
- **Testing** — integration tests, multiplayer verification
- **Documentation** — code comments, architecture decisions
- **Gameplay** — new features, balance tuning

## License

MIT (or whatever your brother chooses)

---

*Built with [SpacetimeDB](https://spacetimedb.com) and [Bevy](https://bevyengine.org)*
