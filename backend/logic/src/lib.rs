//! Pure game logic for SpacetimeRTS.
//! This crate contains deterministic, side-effect-free game rules that can be
//! tested natively and shared between the WASM module and integration tests.

use noise::{NoiseFn, Perlin};

// ----------------------------------------------------------------------------
// INVENTORY
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct InventorySlot {
    pub item_type: String,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Inventory {
    pub entity_id: u64,
    pub slots: Vec<InventorySlot>,
}

impl Inventory {
    pub fn new(entity_id: u64) -> Self {
        Self {
            entity_id,
            slots: Vec::new(),
        }
    }

    pub fn add_item(&mut self, item_type: &str, mut amount: u32) {
        const MAX_STACK: u32 = 50;
        const MAX_SLOTS: usize = 16;

        // Try to stack with existing slots first
        for slot in self.slots.iter_mut() {
            if slot.item_type == item_type && slot.count < MAX_STACK {
                let space = MAX_STACK - slot.count;
                if amount <= space {
                    slot.count += amount;
                    amount = 0;
                    break;
                } else {
                    slot.count = MAX_STACK;
                    amount -= space;
                }
            }
        }

        // Create new slots for remaining amount
        while amount > 0 && self.slots.len() < MAX_SLOTS {
            let add_amt = amount.min(MAX_STACK);
            self.slots.push(InventorySlot {
                item_type: item_type.to_string(),
                count: add_amt,
            });
            amount -= add_amt;
        }
    }

    pub fn remove_item(&mut self, item_type: &str, mut amount: u32) -> bool {
        let total: u32 = self
            .slots
            .iter()
            .filter(|s| s.item_type == item_type)
            .map(|s| s.count)
            .sum();

        if total < amount {
            return false;
        }

        for slot in self.slots.iter_mut() {
            if slot.item_type == item_type {
                if slot.count >= amount {
                    slot.count -= amount;
                    break;
                } else {
                    amount -= slot.count;
                    slot.count = 0;
                }
            }
        }

        self.slots.retain(|s| s.count > 0);
        true
    }

    pub fn count_item(&self, item_type: &str) -> u32 {
        self.slots
            .iter()
            .filter(|s| s.item_type == item_type)
            .map(|s| s.count)
            .sum()
    }

    pub fn has_item(&self, item_type: &str, count: u32) -> bool {
        self.count_item(item_type) >= count
    }
}

// ----------------------------------------------------------------------------
// TERRAIN
// ----------------------------------------------------------------------------

pub fn get_terrain_height(x: f32, z: f32) -> f32 {
    let scale = 0.015;
    let base_height_amp = 18.0;
    let noise_elevation = Perlin::new(42);

    let nx = x as f64 * scale;
    let nz = z as f64 * scale;

    let mut elevation = noise_elevation.get([nx, nz]) * 0.6
        + noise_elevation.get([nx * 2.0, nz * 2.0]) * 0.3
        + noise_elevation.get([nx * 4.0, nz * 4.0]) * 0.1;

    elevation = (elevation + 1.0) * 0.5;
    elevation = elevation.max(0.001);

    let mut y = (elevation.powf(1.4)) as f32 * base_height_amp;

    // River channel
    let river_factor = (x * 0.04).cos().abs() * 3.5;
    if river_factor < 2.0 {
        y = (y - (2.0 - river_factor)).max(0.5);
    }

    // Lake basin at (-35, -35)
    let lake_dist = ((x + 35.0) * (x + 35.0) + (z + 35.0) * (z + 35.0)).sqrt();
    if lake_dist < 25.0 {
        let basin_depth = (1.0 - (lake_dist / 25.0)).max(0.0) * 4.0;
        y = (y - basin_depth).max(0.2);
    }

    if y.is_nan() {
        y = 0.5;
    }
    y
}

// ----------------------------------------------------------------------------
// RESOURCE NODES
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceNode {
    pub node_id: u64,
    pub node_type: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub health: u32,
    pub scale: f32,
    pub required_tool: String,
}

impl ResourceNode {
    pub fn new(node_id: u64, node_type: &str, x: f32, y: f32, z: f32, scale: f32) -> Self {
        let (health, required_tool) = match node_type {
            "Tree" => {
                let health = (3.0 * scale) as u32;
                let tool = if scale > 1.5 { "Axe" } else { "None" };
                (health, tool)
            }
            "Rock" => {
                let health = (4.0 * scale) as u32;
                (health, "None")
            }
            "Bush" => (1, "None"),
            _ => (1, "None"),
        };

        Self {
            node_id,
            node_type: node_type.to_string(),
            x,
            y,
            z,
            health,
            scale,
            required_tool: required_tool.to_string(),
        }
    }

    pub fn is_depleted(&self) -> bool {
        self.health == 0
    }

    pub fn harvest(&mut self) -> Option<(&str, u32)> {
        if self.is_depleted() {
            return None;
        }

        self.health = self.health.saturating_sub(1);

        if self.health == 0 {
            // Node destroyed — return full yield
            let (item, base) = match self.node_type.as_str() {
                "Tree" => ("Wood", 5),
                "Rock" => ("Ore", 3),
                _ => ("Wood", 1),
            };
            let amount = (base as f32 * self.scale).ceil() as u32;
            Some((item, amount))
        } else {
            // Partial harvest
            let item = match self.node_type.as_str() {
                "Tree" => "Wood",
                "Rock" => "Ore",
                _ => "Berry",
            };
            Some((item, 1))
        }
    }
}

// ----------------------------------------------------------------------------
// COMBAT
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick_id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Ray-sphere intersection for hitscan weapons.
/// Returns distance to hit if ray intersects sphere, None otherwise.
pub fn ray_sphere_intersect(
    origin: (f32, f32, f32),
    dir: (f32, f32, f32),
    center: (f32, f32, f32),
    radius: f32,
) -> Option<f32> {
    let ocx = origin.0 - center.0;
    let ocy = origin.1 - center.1;
    let ocz = origin.2 - center.2;

    let b = (ocx * dir.0 + ocy * dir.1 + ocz * dir.2) * 2.0;
    let c = ocx * ocx + ocy * ocy + ocz * ocz - (radius * radius);
    let discriminant = b * b - 4.0 * c;

    if discriminant > 0.0 {
        let dist = (-b - discriminant.sqrt()) / 2.0;
        if dist > 0.0 {
            Some(dist)
        } else {
            None
        }
    } else {
        None
    }
}

// ----------------------------------------------------------------------------
// BUILDING
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Structure {
    pub structure_id: u64,
    pub parent_id: Option<u64>,
    pub piece_type: String,
    pub stability: u32,
    pub is_grounded: bool,
}

impl Structure {
    pub fn foundation() -> Self {
        Self {
            structure_id: 0,
            parent_id: None,
            piece_type: "Foundation".to_string(),
            stability: 100,
            is_grounded: true,
        }
    }

    pub fn can_support(&self, decay_penalty: u32) -> bool {
        self.stability > decay_penalty
    }

    pub fn attach_child(&self, piece_type: &str, structure_id: u64) -> Option<Self> {
        let decay = match piece_type {
            "Wall" => 20,
            "Floor" => 25,
            "Roof" => 30,
            "Ramp" => 25,
            _ => return None,
        };

        if !self.can_support(decay) {
            return None;
        }

        Some(Self {
            structure_id,
            parent_id: Some(self.structure_id),
            piece_type: piece_type.to_string(),
            stability: self.stability - decay,
            is_grounded: false,
        })
    }
}

// ----------------------------------------------------------------------------
// MOVEMENT
// ----------------------------------------------------------------------------

/// Clamp movement delta to max speed (anti-speedhack).
pub fn clamp_movement_delta(
    delta_x: f32,
    delta_y: f32,
    delta_z: f32,
    max_speed: f32,
) -> (f32, f32, f32) {
    let magnitude_sq = delta_x * delta_x + delta_y * delta_y + delta_z * delta_z;
    let max_sq = max_speed * max_speed;

    if magnitude_sq > max_sq {
        let magnitude = magnitude_sq.sqrt();
        let scale = max_speed / magnitude;
        (delta_x * scale, delta_y * scale, delta_z * scale)
    } else {
        (delta_x, delta_y, delta_z)
    }
}

/// Check if a tick is stale (already processed).
pub fn is_stale_tick(tick_id: u64, last_processed: u64) -> bool {
    tick_id <= last_processed
}

// ----------------------------------------------------------------------------
// AI
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum AiState {
    Idle,
    MoveTo { x: f32, y: f32, z: f32 },
    Harvest(u64),
    Return(u64),
    Deposit(u64),
    AutoGather(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Peasant {
    pub entity_id: u64,
    pub owner_id: u64,
    pub state: AiState,
    pub carrying_item: String,
    pub carrying_amount: u32,
}

impl Peasant {
    pub fn new(entity_id: u64, owner_id: u64) -> Self {
        Self {
            entity_id,
            owner_id,
            state: AiState::Idle,
            carrying_item: "None".to_string(),
            carrying_amount: 0,
        }
    }

    pub fn is_carrying(&self) -> bool {
        self.carrying_amount > 0
    }

    pub fn is_carrying_max(&self, max: u32) -> bool {
        self.carrying_amount >= max
    }
}

// ----------------------------------------------------------------------------
// PRNG (deterministic random for resource spawning)
// ----------------------------------------------------------------------------

pub struct Prng {
    seed: u64,
}

impl Prng {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn next_f32(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed as u32 as f32) / (u32::MAX as f32)
    }

    pub fn next_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}
