# Close Quarters Combat (CQC) Physics System Architecture

Developing a **Close Quarters Combat (CQC) Physics System** for both player movement and fast projectile dynamics requires bridging two libraries:

* **`parry.rs`**: The pure computational geometry engine (bounding volumes, GJK / EPA distance algorithms, ray casts, shape-casts, and contact manifolds).
* **`rapier.rs`**: The higher-level spatial simulation engine built *on top of Parry* (`QueryPipeline`, broad-phase BVH trees, `ColliderSet`, `RigidBodySet`, and the Kinematic Character Controller).

In **SpacetimeRTS**, this system bridges both ends of the wire:
1. **Server (SpacetimeDB):** Authoritative hit verification, anti-tunneling swept projectile steps, and body-separation pushback inside deterministic reducers using `rapier3d`.
2. **Client (Bevy + Avian3D):** Zero-latency client prediction, weapon swing trail colliders, and smooth camera/character responsiveness.

---

## System Architecture Overview

```
               [Melee Swing / Lunge]                        [Point-Blank Projectile]
                         │                                              │
                         ▼                                              ▼
            Parry `cast_shape` (Sweep)                     Parry / Rapier CCD (Sweep)
         Swept Capsule along Swing Arc                   Swept Sphere along Velocity Delta
                         │                                              │
                         ├──────────────────────┬───────────────────────┤
                         ▼                                              ▼
               [Player Collision]                               [Shield / Wall]
            Body Pushback & Poise Stagger                     Ricochet or Deflection
       (Capsule-to-Capsule Separation)                   (Contact Normal Reflection)
```

---

## Phase 1: Player Movement in CQC (Body Presence & Sweeps)

In close-quarters combat, the two most common physics failures are **player phasing** (enemies walking through each other during swings) and **wall clipping** (players glitching through walls during dodge rolls or lunges).

### 1. Kinematic Swept Character Movement (`cast_shape`)
Rather than teleporting character positions (`pos += vel * dt`), CQC movement requires **sweeping the player’s cylinder or capsule** through space before moving:

```
[Start Position] ──────(Swept Capsule Vector)──────► [Obstacle / Wall]
                                                           ▲
                                                     Hit at ToI = 0.65
                                                           │
                                             Slide along Tangent Vector
```

* **The Algorithm:**
  1. Take the candidate movement delta $\vec{d} = \vec{v} \cdot \Delta t$.
  2. Cast the player capsule shape using Rapier’s `QueryPipeline::cast_shape`.
  3. If a collision occurs at Time of Impact $\tau \in [0, 1]$:
     * Advance the player by $\tau \cdot \vec{d} \cdot (1 - \epsilon)$ (stopping just short of the surface).
     * Project the remaining distance $(1 - \tau) \cdot \vec{d}$ onto the contact plane tangent:
       $$\vec{d}_{\text{slide}} = \vec{d}_{\text{remaining}} - (\vec{d}_{\text{remaining}} \cdot \vec{n}) \vec{n}$$
     * Execute a second, smaller sweep along $\vec{d}_{\text{slide}}$ to slide naturally along walls.

### 2. Body-to-Body Separation & Pushback (Poise & Mass)
When two players swing at each other, their capsules must resist interpenetration without violent physics explosions:
* Use Parry’s `contact(&shape_a, &pos_a, &shape_b, &pos_b, prediction_dist)`.
* If penetration depth $\delta > 0$, calculate separation impulse:
  $$\vec{p} = \vec{n} \cdot \delta \cdot \frac{m_{\text{other}}}{m_{\text{self}} + m_{\text{other}}}$$
* In SpacetimeDB’s high-frequency tick, apply this pushback symmetrically so heavier armor/poise pushes lighter targets backward.

---

## Phase 2: Melee Hitboxes via Swept Arcs (No Zero-Width Raycasts)

A classic RTS/action flaw is testing melee hits using a simple forward raycast. A dagger or greatsword swing does not travel like a laser—it sweeps an arc through volume over several frames.

```
 Frame N (Windup)          Frame N+1 (Release)
      [Hand]                     [Hand]
        \                          /
         \  (Swept Capsule Volume) /
          \ ◄───────────────────► /
         [Tip]                   [Tip]
```

### The Parry Swept Arc Implementation
Instead of spawning hundreds of tiny colliders along the blade, test the weapon volume between the previous frame’s blade segment $[H_0, T_0]$ and the current frame’s segment $[H_1, T_1]$:

```rust
use parry3d::query::{details::IntersectionCompositeShapeShapeBestFirstVisitor, Ray, PointQuery};
use parry3d::shape::Capsule;
use parry3d::math::{Isometry, Point, Vector};

pub fn check_melee_blade_sweep(
    prev_tip: Vec3, curr_tip: Vec3,
    prev_hilt: Vec3, curr_hilt: Vec3,
    target_capsule: &Capsule,
    target_iso: &Isometry<f32>,
) -> bool {
    // 1. Represent the blade as a capsule from hilt to tip
    let blade_len = (curr_tip - curr_hilt).length();
    let blade_capsule = Capsule::new_y(blade_len * 0.5, 0.12); // 12cm blade thickness
    
    let blade_mid = (curr_tip + curr_hilt) * 0.5;
    let blade_iso = Isometry::translation(blade_mid.x, blade_mid.y, blade_mid.z);

    // 2. Perform swept shape query between the blade and target player capsule
    let sweep_vector = Vector::new(
        curr_tip.x - prev_tip.x,
        curr_tip.y - prev_tip.y,
        curr_tip.z - prev_tip.z,
    );

    // 3. Parry Time of Impact calculation detects hits anywhere along the swing path
    if let Ok(Some(toi)) = parry3d::query::time_of_impact(
        &blade_iso,
        &sweep_vector,
        &blade_capsule,
        target_iso,
        &Vector::zeros(),
        target_capsule,
        1.0, // Max ToI normalized to this frame tick
        0.0,
    ) {
        return toi.toi <= 1.0;
    }
    false
}
```

---

## Phase 3: Close Quarters Projectile Physics (Eliminating Tunneling)

In CQC, firearms (shotguns, revolvers) and crossbows suffer from the **tunneling problem**:
* A bullet moving at $250\text{ m/s}$ travels **$4.16\text{ meters}$ in a single $16.6\text{ms}$ tick**.
* A player capsule is only $0.8\text{m}$ wide.
* A discrete step can completely skip past a player standing $1.5\text{m}$ in front of the barrel!

```
[Muzzle] ──── 1.5m ────► [Player Target (0.8m)] ──── 2.5m ────► [New Position]
  Tick N                                                          Tick N+1
                      (DISCRETE STEP TUNNELED THROUGH!)
```

### The Continuous Collision Detection (CCD) Solution
In `backend/spacetimedb/src/physics.rs`, extend the projectile step to use **Swept Sphere CCD**:

```rust
use rapier3d::prelude::*;

pub fn step_projectile_ccd(
    start: Point<f32>,
    velocity: Vector<f32>,
    dt: f32,
    projectile_radius: f32,
    shooter_id: u64,
    cache: &(ColliderSet, QueryPipeline),
) -> ProjectileStepResult {
    let (colliders, query_pipeline) = cache;
    let delta = velocity * dt;
    let distance = delta.magnitude();
    let dir = delta / distance.max(1e-6);

    let sphere = Ball::new(projectile_radius);
    let shape_iso = Isometry::translation(start.x, start.y, start.z);

    let predicate = |_, collider: &Collider| {
        let id = (collider.user_data & 0xFFFFFFFFFFFFFFFF) as u64;
        id != shooter_id
    };
    let filter = QueryFilter::default().predicate(&predicate);

    // Rapier QueryPipeline Sweeps the sphere through space
    if let Some((handle, hit)) = query_pipeline.cast_shape(
        &RigidBodySet::new(),
        colliders,
        &shape_iso,
        &dir,
        &sphere,
        distance,
        true,
        filter,
    ) {
        let hit_collider = &colliders[handle];
        let hit_entity = hit_collider.user_data;
        let hit_pos = start + dir * hit.toi;
        let normal = hit.normal1; // Contact normal for ricochets

        ProjectileStepResult::Impact {
            entity_id: hit_entity,
            hit_position: hit_pos,
            normal,
        }
    } else {
        ProjectileStepResult::Moved {
            new_position: start + delta,
        }
    }
}
```

---

## Phase 4: Shield Blocks, Deflections & Ricochets

When a projectile or melee weapon impacts an entity, the contact normal $\vec{n}$ determines the physical outcome:

1. **Ricochet / Deflection Angle:**
   $$\vec{v}_{\text{reflected}} = \vec{v}_{\text{incident}} - 2 (\vec{v}_{\text{incident}} \cdot \vec{n}) \vec{n}$$
2. **Glancing Blow vs. Direct Hit:**
   * Calculate angle of incidence: $\cos \theta = -\frac{\vec{v} \cdot \vec{n}}{\|\vec{v}\|}$.
   * If $\theta > 65^\circ$ (steep angle on plate armor or stone): apply **$0.25\times$ damage** and spawn a deflected spark particle.
3. **Shield Block Cone:**
   * If the defender holds a shield, compare the attack vector to the defender's facing vector $\vec{f}$:
     $$\text{is\_blocked} = (\vec{v}_{\text{attack}} \cdot \vec{f}) < -0.5 \quad (\approx 120^\circ \text{ frontal arc})$$

---

## Phase 5: Implementation Roadmap for SpacetimeRTS

| Step | Subsystem | Action Items | Files Involved |
| :---: | :--- | :--- | :--- |
| **1** | **Backend Physics Pipeline** | Add `cast_shape` and `swept_capsule` helpers alongside existing `cast_ray` in `QueryPipeline`. | `backend/spacetimedb/src/physics.rs` |
| **2** | **Ballistic CCD Tick** | Upgrade `ActiveProjectile` simulation in `process_projectiles_tick` to use swept sphere testing instead of discrete jumps. | `backend/spacetimedb/src/combat.rs` |
| **3** | **CQC Player Separation** | In `process_movement`, query neighboring dynamic capsules in the spatial grid and push back penetrating players. | `backend/spacetimedb/src/movement.rs` |
| **4** | **Swept Melee Arc Validation** | Replace single raycast checks in `swing_tool` with a 2-point hilt/tip swept arc check matched against `HitboxHistory`. | `backend/spacetimedb/src/combat.rs` |
| **5** | **Client Prediction & Audio** | Leverage Avian3D's client-side collision events for immediate sword clangs, sparks, and blood impact gibs. | `client/src/weapons.rs`, `client/src/audio_feedback.rs` |
