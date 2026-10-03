//! Pure game logic for SpacetimeRTS.
//! This crate contains deterministic, side-effect-free game rules that can be
//! tested natively and shared between the WASM module and integration tests.

use noise::{NoiseFn, Perlin};

// ----------------------------------------------------------------------------
// INVENTORY & ITEM DISCOVERY
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
    pub discovered_items: Vec<String>,
}

impl Inventory {
    pub fn new(entity_id: u64) -> Self {
        Self {
            entity_id,
            slots: Vec::new(),
            discovered_items: Vec::new(),
        }
    }

    pub fn add_item(&mut self, item_type: &str, mut amount: u32) {
        if item_type.is_empty() || amount == 0 {
            return;
        }

        if !self.discovered_items.iter().any(|d| d == item_type) {
            self.discovered_items.push(item_type.to_string());
        }

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

    pub fn is_discovered(&self, item_type: &str) -> bool {
        self.discovered_items.iter().any(|d| d == item_type)
    }
}

// ----------------------------------------------------------------------------
// RECIPES & CRAFTING
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct RecipeIngredient {
    pub item_type: String,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecipeDefinition {
    pub recipe_id: String,
    pub output_item: String,
    pub output_count: u32,
    pub required_station: String,
    pub requires_roof: bool,
    pub ingredients: Vec<RecipeIngredient>,
}

impl RecipeDefinition {
    pub fn new(
        recipe_id: &str,
        output_item: &str,
        output_count: u32,
        required_station: &str,
        requires_roof: bool,
        ingredients: Vec<(&str, u32)>,
    ) -> Self {
        Self {
            recipe_id: recipe_id.to_string(),
            output_item: output_item.to_string(),
            output_count,
            required_station: required_station.to_string(),
            requires_roof,
            ingredients: ingredients
                .into_iter()
                .map(|(item, count)| RecipeIngredient {
                    item_type: item.to_string(),
                    count,
                })
                .collect(),
        }
    }

    pub fn can_craft(
        &self,
        inv: &Inventory,
        nearby_station: Option<&str>,
        is_under_roof: bool,
    ) -> Result<(), &'static str> {
        if self.requires_roof && !is_under_roof {
            return Err("Requires roof/shelter");
        }

        if self.required_station != "None" {
            match nearby_station {
                Some(station) if station == self.required_station => {}
                _ => return Err("Missing required crafting station"),
            }
        }

        for ing in &self.ingredients {
            if !inv.has_item(&ing.item_type, ing.count) {
                return Err("Missing required ingredients");
            }
        }

        Ok(())
    }

    pub fn craft(
        &self,
        inv: &mut Inventory,
        nearby_station: Option<&str>,
        is_under_roof: bool,
    ) -> Result<(), &'static str> {
        self.can_craft(inv, nearby_station, is_under_roof)?;

        for ing in &self.ingredients {
            inv.remove_item(&ing.item_type, ing.count);
        }

        inv.add_item(&self.output_item, self.output_count);
        Ok(())
    }
}

pub fn get_canonical_recipes() -> Vec<RecipeDefinition> {
    vec![
        RecipeDefinition::new("Hammer", "Hammer", 1, "None", false, vec![("Branch", 1), ("LooseStone", 1)]),
        RecipeDefinition::new("Stone Axe", "Stone Axe", 1, "None", false, vec![("Branch", 1), ("Flint", 1)]),
        RecipeDefinition::new("Pickaxe", "Pickaxe", 1, "None", false, vec![("Branch", 2), ("Flint", 2)]),
        RecipeDefinition::new("Club", "Club", 1, "None", false, vec![("Branch", 2)]),
        RecipeDefinition::new("Torch", "Torch", 1, "None", false, vec![("Branch", 1), ("Resin", 1)]),
        RecipeDefinition::new("Crude Bow", "Crude Bow", 1, "Workbench", false, vec![("Wood", 10), ("Leather Scraps", 4)]),
        RecipeDefinition::new("Flint Arrow", "Flint Arrow", 20, "Workbench", false, vec![("Wood", 8), ("Flint", 2)]),
        RecipeDefinition::new("Wood Arrow", "Wood Arrow", 20, "None", false, vec![("Wood", 8)]),
        RecipeDefinition::new("Wooden Shield", "Wooden Shield", 1, "Workbench", false, vec![("Wood", 10), ("Leather Scraps", 2)]),
        RecipeDefinition::new("Flint Spear", "Flint Spear", 1, "Workbench", false, vec![("Wood", 6), ("Flint", 2), ("Leather Scraps", 2)]),
    ]
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

pub fn world_to_chunk_coord(pos: f32, chunk_size: f32) -> i32 {
    (pos / chunk_size).floor() as i32
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
                let tool = if scale > 1.2 { "Stone Axe" } else { "None" };
                (health, tool)
            }
            "Rock" => {
                let health = (4.0 * scale) as u32;
                (health, "Pickaxe")
            }
            "Bush" => (1, "None"),
            "Branch" => (1, "None"),
            "Flint" => (1, "None"),
            "LooseStone" => (1, "None"),
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
            let (item, base) = match self.node_type.as_str() {
                "Tree" => ("Wood", 6),
                "Rock" => ("Stone", 4),
                "Branch" => ("Branch", 1),
                "Flint" => ("Flint", 1),
                "LooseStone" => ("LooseStone", 1),
                _ => ("Wood", 1),
            };
            let amount = (base as f32 * self.scale).ceil() as u32;
            Some((item, amount))
        } else {
            let item = match self.node_type.as_str() {
                "Tree" => "Wood",
                "Rock" => "Stone",
                _ => "Berry",
            };
            Some((item, 1))
        }
    }
}

// ----------------------------------------------------------------------------
// COMBAT & BALLISTICS
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

/// Linear interpolation between two snapshots for lag compensation hit registration.
pub fn interpolate_snapshot(
    s1: &Snapshot,
    s2: &Snapshot,
    target_tick: u64,
) -> Option<(f32, f32, f32)> {
    if target_tick < s1.tick_id || target_tick > s2.tick_id {
        return None;
    }
    if s1.tick_id == s2.tick_id {
        return Some((s1.x, s1.y, s1.z));
    }
    let t = (target_tick - s1.tick_id) as f32 / (s2.tick_id - s1.tick_id) as f32;
    Some((
        s1.x + (s2.x - s1.x) * t,
        s1.y + (s2.y - s1.y) * t,
        s1.z + (s2.z - s1.z) * t,
    ))
}

/// Ray-sphere intersection for hitscan weapons and projectile sweep checks.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileKind {
    Arrow,
    CatapultRock,
    TrebuchetShell,
    BallistaSpear,
    MagicMissile,
}

impl ProjectileKind {
    pub fn gravity(&self) -> f32 {
        match self {
            Self::Arrow => 9.81,
            Self::CatapultRock => 15.0,
            Self::TrebuchetShell => 12.0,
            Self::BallistaSpear => 4.5,
            Self::MagicMissile => 0.0,
        }
    }

    pub fn base_speed(&self) -> f32 {
        match self {
            Self::Arrow => 45.0,
            Self::CatapultRock => 25.0,
            Self::TrebuchetShell => 35.0,
            Self::BallistaSpear => 60.0,
            Self::MagicMissile => 30.0,
        }
    }

    pub fn base_damage(&self) -> f32 {
        match self {
            Self::Arrow => 25.0,
            Self::CatapultRock => 120.0,
            Self::TrebuchetShell => 250.0,
            Self::BallistaSpear => 85.0,
            Self::MagicMissile => 40.0,
        }
    }
}

pub fn simulate_projectile_step(
    pos: (f32, f32, f32),
    vel: (f32, f32, f32),
    kind: ProjectileKind,
    dt: f32,
) -> ((f32, f32, f32), (f32, f32, f32)) {
    let new_vel = (
        vel.0,
        vel.1 - kind.gravity() * dt,
        vel.2,
    );
    let new_pos = (
        pos.0 + new_vel.0 * dt,
        pos.1 + new_vel.1 * dt,
        pos.2 + new_vel.2 * dt,
    );
    (new_pos, new_vel)
}

// ----------------------------------------------------------------------------
// BUILDING & DURABILITY
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Structure {
    pub structure_id: u64,
    pub parent_id: Option<u64>,
    pub piece_type: String,
    pub stability: u32,
    pub is_grounded: bool,
    pub is_blueprint: bool,
    pub construction_progress: u32,
    pub current_health: f32,
    pub max_health: f32,
}

impl Structure {
    pub fn piece_max_health(piece_type: &str) -> f32 {
        match piece_type {
            "Foundation" => 400.0,
            "Wall" => 200.0,
            "Floor" => 150.0,
            "Roof" => 150.0,
            "Ramp" => 250.0,
            "Workbench" => 150.0,
            "Campfire" => 60.0,
            _ => 100.0,
        }
    }

    pub fn foundation() -> Self {
        Self {
            structure_id: 0,
            parent_id: None,
            piece_type: "Foundation".to_string(),
            stability: 100,
            is_grounded: true,
            is_blueprint: false,
            construction_progress: 100,
            current_health: 400.0,
            max_health: 400.0,
        }
    }

    pub fn new_blueprint(structure_id: u64, parent_id: Option<u64>, piece_type: &str, stability: u32, is_grounded: bool) -> Self {
        let max_hp = Self::piece_max_health(piece_type);
        Self {
            structure_id,
            parent_id,
            piece_type: piece_type.to_string(),
            stability,
            is_grounded,
            is_blueprint: true,
            construction_progress: 0,
            current_health: 1.0,
            max_health: max_hp,
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

        let max_hp = Self::piece_max_health(piece_type);
        Some(Self {
            structure_id,
            parent_id: Some(self.structure_id),
            piece_type: piece_type.to_string(),
            stability: self.stability - decay,
            is_grounded: false,
            is_blueprint: false,
            construction_progress: 100,
            current_health: max_hp,
            max_health: max_hp,
        })
    }

    pub fn contribute_construction(&mut self, percent: u32) -> bool {
        if !self.is_blueprint {
            return false;
        }
        self.construction_progress = (self.construction_progress + percent).min(100);
        if self.construction_progress >= 100 {
            self.is_blueprint = false;
            self.current_health = self.max_health;
            true
        } else {
            self.current_health = (self.max_health * (self.construction_progress as f32 / 100.0)).max(1.0);
            false
        }
    }

    pub fn repair(&mut self, amount: f32) -> f32 {
        if self.is_blueprint {
            return 0.0;
        }
        let missing = self.max_health - self.current_health;
        let actual_repair = missing.min(amount);
        self.current_health += actual_repair;
        actual_repair
    }

    pub fn damage(&mut self, amount: f32) -> bool {
        self.current_health = (self.current_health - amount).max(0.0);
        self.current_health <= 0.0
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
// AI & FACTIONS
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
pub enum AiType {
    Friendly,
    Deer,
    Boar,
    Goblin,
    Peasant,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrainState {
    Idle,
    Fleeing,
    Chasing,
    Attacking,
    Warning,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PetStance {
    Stay,
    Follow,
    Aggressive,
    Defensive,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Faction {
    Player,
    Villager,
    Wildlife,
    Goblin,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FactionStanding {
    Ally,
    Neutral,
    KillOnSight,
}

/// Evaluate faction relationships.
pub fn get_standing(a: &Faction, b: &Faction) -> FactionStanding {
    match (a, b) {
        (Faction::Player, Faction::Villager) => FactionStanding::Neutral,
        (Faction::Villager, Faction::Player) => FactionStanding::Neutral,
        (Faction::Player, Faction::Goblin) => FactionStanding::KillOnSight,
        (Faction::Goblin, Faction::Player) => FactionStanding::KillOnSight,
        (Faction::Goblin, Faction::Villager) => FactionStanding::KillOnSight,
        (Faction::Villager, Faction::Goblin) => FactionStanding::KillOnSight,
        (Faction::Villager, Faction::Wildlife) => FactionStanding::Neutral,
        (Faction::Wildlife, Faction::Villager) => FactionStanding::Neutral,
        (Faction::Player, Faction::Wildlife) => FactionStanding::Neutral,
        (Faction::Wildlife, Faction::Player) => FactionStanding::Neutral,
        _ => FactionStanding::Neutral,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Peasant {
    pub entity_id: u64,
    pub owner_id: u64,
    pub state: AiState,
    pub carrying_item: String,
    pub carrying_amount: u32,
    pub last_harvest_target: Option<u64>,
    pub consecutive_stuck_ticks: u32,
    pub auto_gather_type: String,
}

impl Peasant {
    pub fn new(entity_id: u64, owner_id: u64) -> Self {
        Self {
            entity_id,
            owner_id,
            state: AiState::Idle,
            carrying_item: "None".to_string(),
            carrying_amount: 0,
            last_harvest_target: None,
            consecutive_stuck_ticks: 0,
            auto_gather_type: "None".to_string(),
        }
    }

    pub fn is_carrying(&self) -> bool {
        self.carrying_amount > 0
    }

    pub fn is_carrying_max(&self, max: u32) -> bool {
        self.carrying_amount >= max
    }

    pub fn note_stuck_tick(&mut self) -> bool {
        self.consecutive_stuck_ticks += 1;
        self.consecutive_stuck_ticks >= 10 // Flag stuck after 10 ticks
    }

    pub fn reset_stuck_ticks(&mut self) {
        self.consecutive_stuck_ticks = 0;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NpcBrain {
    pub entity_id: u64,
    pub ai_type: AiType,
    pub state: BrainState,
    pub target_id: Option<u64>,
    pub timer: f32,
    pub home_x: f32,
    pub home_z: f32,
    pub wander_x: f32,
    pub wander_z: f32,
}

impl NpcBrain {
    pub fn new(entity_id: u64, ai_type: AiType, home_x: f32, home_z: f32) -> Self {
        Self {
            entity_id,
            ai_type,
            state: BrainState::Idle,
            target_id: None,
            timer: 0.0,
            home_x,
            home_z,
            wander_x: home_x,
            wander_z: home_z,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PetComponent {
    pub entity_id: u64,
    pub owner_id: u64,
    pub stance: PetStance,
}

impl PetComponent {
    pub fn new(entity_id: u64, owner_id: u64) -> Self {
        Self {
            entity_id,
            owner_id,
            stance: PetStance::Follow,
        }
    }

    pub fn should_attack_target(&self, target_standing: FactionStanding, is_attacking_owner: bool) -> bool {
        match self.stance {
            PetStance::Aggressive => target_standing == FactionStanding::KillOnSight,
            PetStance::Defensive => is_attacking_owner,
            PetStance::Stay | PetStance::Follow => false,
        }
    }
}

// ----------------------------------------------------------------------------
// DAY / NIGHT CYCLE & WORLD STATE
// ----------------------------------------------------------------------------

pub fn advance_time_of_day(current_time: f32, dt_hours: f32) -> f32 {
    let mut t = current_time + dt_hours;
    while t >= 24.0 {
        t -= 24.0;
    }
    while t < 0.0 {
        t += 24.0;
    }
    t
}

pub fn is_daylight(time_of_day: f32) -> bool {
    time_of_day >= 6.0 && time_of_day < 20.0
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
