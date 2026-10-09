# SpacetimeRTS: Unified Server & Developer Console Guide

This document is the definitive operational reference for launching the SpacetimeDB backend, compiling the client with optimal flags, and executing in-game developer console commands for debugging, gameplay testing, and real-time graphics/lighting diagnostics.

---

## 1. SpacetimeDB Server Commands

The backend WebAssembly module is located in `backend/spacetimedb`.

### 1.1 Fresh Server Launch (Clean Database & Full Reset)
Use this command whenever you modify table schemas, reducers, or wish to test from a clean world state with regenerated resource nodes:
```bash
# Clear previous database state and start a fresh server instance
spacetime start --clear-database
```
*Alternatively, in another terminal publish the module:*
```bash
# Publish module to local database named "spacetimerts"
cd backend/spacetimedb
spacetime publish spacetimerts --clear-database
```

### 1.2 Standard Server Launch (Persisting World State)
```bash
spacetime start
```
```bash
# Publish module update while retaining existing player and world data
cd backend/spacetimedb
spacetime publish spacetimerts
```

### 1.3 Generating Client Module Bindings
If you modify types or tables in `backend/spacetimedb/src/lib.rs`:
```bash
spacetime generate --lang rust --out-dir client/src/module_bindings --module-path backend/spacetimedb
```

### 1.4 Setting or Randomizing World Seed
The world terrain, resource nodes, caves, and ravines are generated deterministically from the authoritative `world_seed`:
```bash
# Generate a new randomized world seed on the server
spacetime call spacetimerts admin_randomize_world_seed

# Set an explicit world seed (e.g. 1337 or 42)
spacetime call spacetimerts admin_set_world_seed 1337
```
When invoked, SpacetimeDB updates `global_state.world_seed`, respawns resource nodes, and automatically broadcasts the update to all connected Bevy clients, which rebuild terrain and colliders in real time.


---

## 2. Client Launch Commands

The client is built on Bevy 0.13 with Avian3D physics and WGSL shaders.

### 2.1 Snappy High-Performance Dev Launch (Recommended)
Runs with release optimizations on dependencies (configured in `Cargo.toml`) while compiling game code quickly:
```bash
cargo run -p client
```

### 2.2 Full Release Build (Locked 60+ FPS Production Mode)
Maximizes SIMD vectorization, Avian3D solver throughput, and GPU pipeline performance:
```bash
cargo run --release -p client
```

### 2.3 Headless or Dedicated Tests
```bash
cargo test -p client
cargo test -p backend
```

---

## 3. Developer In-Game Console

### 3.1 Opening the Console & Auto-Completion
- **Toggle Console**: Press the **`~`** (Tilde / Grave) key or **F1**.
- **Auto-Completion**: Type any partial command or item name and press **`Tab`** to cycle matching options.
- **Command History**: Use **Up Arrow** ($\uparrow$) and **Down Arrow** ($\downarrow$) to recall previous commands.

---

### 3.2 Lighting & Dual-Star Diagnostics

These commands allow you to isolate illumination sources, resolve shadow conflicts, and inspect atmospheric ephemeris.

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `light solo a` | — | **Isolates Host Star A**: Disables Star B (0 lx) to verify single-source shadows and uniform lit face angles. |
| `light solo b` | — | **Isolates Companion Star B**: Disables Star A (0 lx) to inspect warm dwarf amber illumination. |
| `light dual` | — | **Restores Dual Stars**: Re-enables both Host Star A and Companion Star B. |
| `light shadows` | `<a\|b\|both> <on\|off>` | Toggles directional shadow casting individually per star. |
| `light` / `light status` | — | Prints current star lux levels, shadow states, and active directional flags. |
| `star` | `<a\|b> <lux>` | Manually overrides extraterrestrial illuminance (e.g. `star a 75000`, `star b 40000`). |
| `star` | `<a\|b> color <hex\|name>` | Overrides star spectral tint (e.g. `star b color ff5400`, `star b color blaze`, `star b color reset`). |
| `dualshadows` | — | Activates high-contrast dual-sun preset with distinct white/blaze-orange penumbras. |
| `ambient` | `<lux\|reset>` | Adjusts ambient fill light floor (e.g. `ambient 200` for deeper shadows, `ambient reset` for auto). |
| `time` / `settime` | `<0-24>` | Sets in-game world clock (e.g. `time 12.0` for High Noon, `time 18.2` for Sunset). |
| `day` / `noon` | — | Instantly jumps to High Noon (12:00) with overhead solar contact. |
| `night` / `midnight` | — | Instantly jumps to Deep Midnight (00:00) with active cosmic starfield and auroras. |
| `sunset` / `dusk` / `dawn` | — | Jumps directly to atmospheric twilight palette transitions (18.2h, 19.3h, 05.8h). |
| `palette` | — | Displays the active S-type circumstellar atmospheric hex color palette. |
| `timescale` / `speed` | `<mult>` | Speeds up or slows down celestial orbits (e.g. `timescale 10.0`). |
| `weather` | `<clear\|haze\|aurora\|rain>` | Sets atmospheric scattering and weather preset. |

---

### 3.3 Foliage & Tree Material Diagnostics

These commands isolate geometry, face normals, and material shading bugs (such as dark or flat black polygons).

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `foliage unlit` | `[on\|off]` | **Bypasses PBR Lighting & Shadows**: Renders foliage unlit. If dark faces turn into rich colors, the geometry and vertex colors are sound and the problem is shadow acne or lighting. |
| `foliage cull` | `<none\|back>` | **Toggles Face Culling**: `none` enables two-sided ribbons/leaves; `back` enforces strict CCW outward faces. If leaves vanish or darken under `back`, vertex normal winding was inverted. |
| `foliage white` | — | **Raw Vertex Colors**: Sets material base color to pure `Color::WHITE` to display baked mesh vertex colors without squaring or darkening. |
| `foliage tint` | `[on\|off]` | Toggles seasonal palette multiplier on tree materials. |
| `foliage` / `tree` | — | Prints active foliage material settings (unlit state, culling mode, seasonal tint). |

---

### 3.4 Cascaded Shadow Map (CSM) Tuning

Eliminates shadow acne (black self-shadowing patterns) and harsh cascade boundary seams between near and far trees.

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `csm bias` | `<depth_bias> <normal_bias>` | Adjusts directional light shadow biases live (e.g. `csm bias 0.03 2.0`). Increase slightly if flat faces self-shadow into black. |
| `csm dist` | `<max_dist> [first_cascade_bound]` | Adjusts maximum shadow draw distance and near cascade threshold (e.g. `csm dist 160 18`). |
| `csm min` | `<min_dist>` | Adjusts the near cascade clip distance in meters (e.g. `csm min 0.5`). Prevents near-plane shadow acne. |
| `csm` / `csm status` | — | Displays active cascade counts, distance bounds, and depth/normal biases for both stars. |

---

### 3.5 Player, Equipment & Combat Commands

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `giveitem` / `give` | `<Item> [amt]` | Grants items to player inventory. Supports Tab auto-fill (e.g. `giveitem Longsword 1`, `giveitem Wood 50`). |
| `tp` / `teleport` | `<x> <z>` | Instantly teleports player to world coordinate (e.g. `tp 0 0`, `tp 120 -80`). |
| `heal` | `[amt]` | Restores player health (default: 100 HP). |
| `god` | — | Enables invulnerability (sets health to 99,999 HP). |
| `equip` | `[main\|off] <weapon>` | Equips specified weapon to main-hand or off-hand slot. |
| `dual` | `<main_weapon> <off_weapon>` | Equips a dual-wield weapon loadout (e.g. `dual Longsword Axe`, `dual Revolver Revolver`). |
| `clearinv` / `clear` | — | Wipes all player inventory slots clean. |
| `abilities` | — | Displays tactical abilities guide (Dash `[Q]`, Smoke `[C]`, Intel `[E]`, Grav-Lift `[F]`). |

---

### 3.6 NPC, Mob & World Spawning

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `spawn` | `<deer\|boar\|goblin\|peasant> [amt]` | Spawns creatures or peasants around the player. |
| `spawnbuilding` | `<hut\|cottage\|guardpost> [rot (0-3)] [x] [z]` | Dispatches NPC building construction. |
| `killall` | — | Destroys all active non-player entities in the server. |
| `nuke` / `blast` | `[radius]` | Triggers a destructive spherical voxel demolition blast. |
| `seed` | `[number\|random]` | Views current world seed, configures an explicit seed, or generates a new random seed. |

---

### 3.7 Display, Performance & Diagnostics

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `f3` / `fps` / `diag` | `[on\|off\|toggle]` | Toggles the real-time F3 telemetry HUD (FPS, frame time, entities, player coordinates). |
| `maxfps` / `fpslimit` | `<fps\|0\|off>` | Caps framerate or unbinds it (e.g. `maxfps 60`, `maxfps 144`, `maxfps 0` for uncapped). |
| `vsync` | `<on\|off\|immediate\|mailbox>` | Toggles or configures presentation sync mode. |
| `res` / `resolution` | `<w h \| preset>` | Changes window resolution (e.g. `res 1080p`, `res 1440p`, `res 1920 1080`). |
| `fullscreen` / `windowed` | — | Toggles between borderless fullscreen and windowed display mode. |
| `hud` / `togglehud` | `[on\|off\|toggle]` | Toggles the upper celestial clock and weather HUD pill. |

---

## 4. Keybinding Quick Reference

| Key | Context | Action |
| :---: | :--- | :--- |
| **`~`** or **`F1`** | Global | Toggle Developer Console |
| **`Tab`** | In Console | Auto-complete command or item name |
| **`F3`** | In-Game | Toggle Performance Telemetry Overlay (FPS, entities, coordinate) |
| **`F6`** | In-Game | Open Weapon & Spell Tuner Workbench |
| **`F7`** | In-Game | Open Interactive Crosshair Tuner GUI |
| **`F8`** | In-Game | Quick-cycle Noon / Midnight celestial lighting |
| **`F9`** | In-Game | Quick-cycle Atmospheric Weather presets |
| **`F10`** | In-Game | Toggle Celestial Clock HUD pill |
| **`K`** | In-Game | Open Spellbook & Ability Grimoire |
| **`L`** | In-Game | Open Valheim-style Character Skills Sheet |
| **`[`** / **`]`** | In-Game | Scrub world time backward / forward by 1.0 hour |
| **`-`** / **`=`** | In-Game | Halve / double celestial orbit timescale |
| **`Q`** | Combat | Tactical Ability: Phase Dash |
| **`C`** | Combat | Tactical Ability: Smoke Veil |
| **`E`** | Combat | Tactical Ability: Intel Dart |
| **`F`** | Combat | Tactical Ability: Grav-Lift |
