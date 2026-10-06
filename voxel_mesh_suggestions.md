Searched for "pub struct Prng"
Viewed lib.rs:2130-2160

Here is an elaboration on the cleanup options for [`client/src/voxel_mesh.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/voxel_mesh.rs), why the current structure causes confusion, and the trade-offs of each approach:

---

### The Underlying Architectural Problem

[`voxel_mesh.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/voxel_mesh.rs) currently suffers from **Three Unrelated Responsibilities** bundled into a file with a misleading name:

| Component in `voxel_mesh.rs` | What It Actually Is | Consumers | Why the Name is Misleading |
| :--- | :--- | :--- | :--- |
| **`Prng`** | Deterministic XorShift64 pseudo-random number generator | [`trees.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/trees.rs), [`props.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/props.rs), [`grass.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/grass.rs) | It is a pure math utility; it has nothing to do with meshes or voxels. |
| **`VoxelBox` & `build_voxel_mesh`** | Axis-Aligned Bounding Box (AABB) mesh generator | [`building.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/building.rs) (walls, roofs, stairs, crafting benches) | These are **architectural box composites** (cuboids with dimensions like $4.0 \times 0.2 \times 4.0\text{m}$), **not voxels**. There is no voxel grid or voxel volume. |
| **`MicroVoxelGrid`** | Discrete 3D rasterizer with exposed-face extraction | [`creatures.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/creatures.rs) (deer, boars, goblins, peasants) | This is the *only* actual voxel system in the file. |

---

### Option 1: Modular File Splitting & Semantic Renaming *(Recommended First Step)*

This option leaves all rendering algorithms identical but gives every piece its correct single responsibility and clear names.

1. **Extract `Prng` $\rightarrow$ [`client/src/prng.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/prng.rs) (or share with `spacetime_rts_logic`):**
   - Both the backend logic crate and client have near-identical XorShift64 implementations.
   - Moving this to a dedicated module allows `trees.rs`, `props.rs`, and `grass.rs` to import from `crate::prng::Prng` instead of importing random number generation from a mesh file.
2. **Move Box Meshing $\rightarrow$ [`client/src/building.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/building.rs):**
   - Rename `VoxelBox` $\rightarrow$ `BuildingBox` (or `BoxPrimitive`).
   - Rename `build_voxel_mesh` $\rightarrow$ `build_building_mesh`.
   - Place it directly in `building.rs` (or a helper submodule `building_mesh.rs`). This makes it immediately obvious that building pieces are composed of stylized wooden/stone planks and blocks.
3. **Rename `voxel_mesh.rs` $\rightarrow$ [`client/src/creature_mesh.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/creatures.rs):**
   - Keep `MicroVoxelGrid` isolated strictly to creature model generation.

* **Effort:** ~15 minutes.
* **Risk:** Zero (mechanical refactor, 100% test preserved).
* **Benefit:** Eliminates confusion over what is "voxel" vs "low-poly".

---

### Option 2: Migrate Creatures to Low-Poly Polyhedra *(Aesthetic & Performance Unification)*

This is the deeper, long-term architectural path aligned with **[AGENTS.md Directive #4](file:///Users/k/Documents/GameDev/SpacetimeRTS/AGENTS.md)**:

> *"Low-Poly Faceted Aesthetic: Follow the BlendSwap #9440 geometric aesthetic (procedural flat-shaded polyhedral meshes, ~95% fewer polygons than micro-voxels)."*

- **The Issue with Micro-Voxels for Creatures:**
  - A deer or peasant generated via [`MicroVoxelGrid`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/voxel_mesh.rs#L152) consists of thousands of tiny $0.022\text{m}$ ($2.2\text{cm}$) cubes.
  - Each cube has up to 6 exposed faces (2 triangles each). A single NPC can have 4,000–8,000 triangles and dense vertex buffers.
  - In a 50-player battle with 100+ peasants/mobs, micro-voxel creatures chew through the GPU vertex budget and shadow cascade fill rates.
- **The Solution:**
  - Generate creature limbs, torso, and heads using low-poly polyhedral cylinders and tapered prisms (similar to how [`trees.rs`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/trees.rs) generates faceted trunks and foliage).
  - A low-poly stylized deer would use **~180–300 triangles** instead of **~5,000 triangles**—an instant 95% geometry reduction that perfectly matches the world's trees, rocks, and terrain.
- **Result:** **`MicroVoxelGrid` and `voxel_mesh.rs` would be 100% deprecated and safely deleted.**

---

### Option 3: Greedy Meshing Optimization for `MicroVoxelGrid` *(If Keeping Voxel Creatures)*

If you prefer keeping the distinct micro-voxel aesthetic for creatures (resembling *Cube World* or *MagicaVoxel* characters):

- **Current Implementation:**
  - [`MicroVoxelGrid::build_mesh`](file:///Users/k/Documents/GameDev/SpacetimeRTS/client/src/voxel_mesh.rs#L300-L396) performs basic face culling (omits internal faces that have neighbor voxels), but emits **individual $1 \times 1$ quads** for every single exposed face.
  - Large flat surfaces (like a creature's torso or flank) generate dozens of unmerged tiny quads.
- **Optimization:**
  - Implement **2D slice Greedy Meshing**: scan adjacent coplanar voxel faces with matching vertex colors and merge them into larger rectangular quads.
  - This typically reduces creature vertex and triangle counts by **60% to 75%** without changing a single pixel of visual appearance.

---

### Comparison Matrix

| Approach | Architecture Impact | Performance Impact | Effort |
| :--- | :--- | :--- | :--- |
| **Option 1 (Split & Rename)** | High (decouples math, buildings, & creatures) | Neutral (code organization only) | Low (quick refactor) |
| **Option 2 (Polyhedral Creatures)** | Maximum (removes all voxels from engine, matches AGENTS.md) | Huge (~95% fewer creature vertices & shadows) | Medium-High (modeling procedural forms) |
| **Option 3 (Greedy Meshing)** | Keeps existing file structure, upgrades algorithm | Significant (~65% fewer creature vertices) | Medium (quad merging algorithm) |

### Recommended Action Plan
1. **Immediately / Near-Term:** Perform **Option 1** (move `Prng` to `prng.rs`, move `VoxelBox` to `building.rs`, and rename `voxel_mesh.rs` to `creature_mesh.rs`). This instantly clarifies the codebase.
2. **Longer-Term:** When scheduling creature visual updates, evaluate **Option 2** to unify the visual style with low-poly faceted meshes.