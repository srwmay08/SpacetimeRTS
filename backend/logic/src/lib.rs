//! Pure game logic for SpacetimeRTS.
//! This crate contains deterministic, side-effect-free game rules that can be
//! tested natively and shared between the WASM module and integration tests.

use std::collections::BTreeMap;
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
    pub max_capacity: usize,
}

impl Inventory {
    pub fn new(entity_id: u64) -> Self {
        Self {
            entity_id,
            slots: Vec::new(),
            discovered_items: Vec::new(),
            max_capacity: 16,
        }
    }

    pub fn max_slots(&self) -> usize {
        self.max_capacity.max(16)
    }

    pub fn set_capacity(&mut self, cap: usize) {
        self.max_capacity = cap.max(16);
    }

    pub fn add_item(&mut self, item_type: &str, mut amount: u32) {
        if item_type.is_empty() || amount == 0 {
            return;
        }

        if !self.discovered_items.iter().any(|d| d == item_type) {
            self.discovered_items.push(item_type.to_string());
        }

        const MAX_STACK: u32 = 50;
        let max_allowed_slots = self.max_slots();

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
        while amount > 0 && self.slots.len() < max_allowed_slots {
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
        RecipeDefinition::new("Hand Crossbow", "Hand Crossbow", 1, "Workbench", false, vec![("Wood", 8), ("Flint", 3), ("Leather Scraps", 2)]),
        RecipeDefinition::new("Revolver", "Revolver", 1, "Forge", true, vec![("Iron Ingot", 4), ("Wood", 2)]),
        RecipeDefinition::new("Shotgun", "Shotgun", 1, "Forge", true, vec![("Iron Ingot", 8), ("Wood", 4)]),
        RecipeDefinition::new("Sniper Rifle", "Sniper Rifle", 1, "Forge", true, vec![("Iron Ingot", 12), ("Wood", 6)]),
        RecipeDefinition::new("Runestaff", "Runestaff", 1, "Workbench", true, vec![("Elder Wood", 4), ("Resin", 4)]),
    ]
}

// ----------------------------------------------------------------------------
// WEAPON CLASSIFICATION, GRIPS & LOADOUTS
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponGrip {
    OneHanded,
    TwoHanded,
    Polearm,
    Versatile, // Can be wielded 1H or 2H
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponCategory {
    Edged,
    Pointed,
    Blunt,
    TwoHanded,
    Polearm,
    Brawling,
    Missile,
    Firearm,
    Runestaff,
    Comedic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageType {
    Slashing,
    Piercing,
    Bludgeoning,
    Ballistic,
    PelletSpread,
    ArcaneForce,
    FireSplash,
    CartoonBonk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandSlot {
    MainHand,
    OffHand,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectileProfile {
    pub kind: ProjectileKind,
    pub muzzle_velocity: f32, // m/s
    pub gravity: f32,          // m/s^2
    pub drag: f32,
    pub spread_radians: f32,
    pub pellet_count: u32,
    pub blast_radius: f32,
    pub is_slow_projectile: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponDef {
    pub id: String,
    pub name: String,
    pub grip: WeaponGrip,
    pub category: WeaponCategory,
    pub damage_type: DamageType,
    pub base_damage: f32,
    pub attack_speed: f32, // swings / shots per second
    pub reach_meters: f32,
    pub armor_penetration: f32, // 0.0 to 1.0 (percent of armor ignored)
    pub durability: u32,
    pub max_durability: u32,
    pub magazine_capacity: Option<u32>,
    pub current_ammo: u32,
    pub projectile_profile: Option<ProjectileProfile>,
    pub knockback_force: f32, // TF2 comedic launch impulse (m/s)
}

impl WeaponDef {
    pub fn is_two_handed(&self) -> bool {
        matches!(self.grip, WeaponGrip::TwoHanded | WeaponGrip::Polearm)
    }

    pub fn is_one_handed(&self) -> bool {
        matches!(self.grip, WeaponGrip::OneHanded | WeaponGrip::Versatile)
    }

    pub fn is_ranged(&self) -> bool {
        self.projectile_profile.is_some()
    }
}

// ----------------------------------------------------------------------------
// WOW & EQ CONTAINER BAGS & STATIC INVENTORY SCALING
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HandSide {
    #[default]
    Right,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContainerItemSize {
    Tiny,
    Small,
    Medium,
    Large,
    Giant,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BagContainerDef {
    pub id: String,
    pub name: String,
    pub capacity: usize,
    pub size_cap: ContainerItemSize,
    pub weight_reduction_pct: u32,
}

pub fn create_bag_container(name: &str) -> Option<BagContainerDef> {
    match name {
        "Small Pouch" => Some(BagContainerDef {
            id: "small_pouch".into(),
            name: "Small Pouch".into(),
            capacity: 4,
            size_cap: ContainerItemSize::Small,
            weight_reduction_pct: 0,
        }),
        "Leather Bag" => Some(BagContainerDef {
            id: "leather_bag".into(),
            name: "Leather Bag".into(),
            capacity: 6,
            size_cap: ContainerItemSize::Medium,
            weight_reduction_pct: 10,
        }),
        "Shralok Pack" => Some(BagContainerDef {
            id: "shralok_pack".into(),
            name: "Shralok Pack".into(),
            capacity: 8,
            size_cap: ContainerItemSize::Giant,
            weight_reduction_pct: 25,
        }),
        "Bag of Sewn Evil-Eye" => Some(BagContainerDef {
            id: "bag_of_sewn_evil_eye".into(),
            name: "Bag of Sewn Evil-Eye".into(),
            capacity: 8,
            size_cap: ContainerItemSize::Large,
            weight_reduction_pct: 70,
        }),
        "Traveler's Backpack" => Some(BagContainerDef {
            id: "travelers_backpack".into(),
            name: "Traveler's Backpack".into(),
            capacity: 12,
            size_cap: ContainerItemSize::Giant,
            weight_reduction_pct: 50,
        }),
        "Dragonflight Haversack" => Some(BagContainerDef {
            id: "dragonflight_haversack".into(),
            name: "Dragonflight Haversack".into(),
            capacity: 16,
            size_cap: ContainerItemSize::Giant,
            weight_reduction_pct: 100,
        }),
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// EQUIPPED LOADOUT & DUAL-WIELD VALIDATION
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EquippedLoadout {
    pub main_hand: Option<WeaponDef>,
    pub off_hand: Option<WeaponDef>,
    pub primary_hand: HandSide,
    pub head: Option<String>,
    pub chest: Option<String>,
    pub legs: Option<String>,
    pub feet: Option<String>,
    pub bag_slots: [Option<BagContainerDef>; 4],
}

impl EquippedLoadout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn equip(&mut self, slot: HandSlot, weapon: WeaponDef) -> Result<(), &'static str> {
        match slot {
            HandSlot::MainHand => {
                if weapon.is_two_handed() && self.off_hand.is_some() {
                    return Err("Cannot equip two-handed weapon while off-hand is occupied.");
                }
                self.main_hand = Some(weapon);
                Ok(())
            }
            HandSlot::OffHand => {
                if let Some(ref main) = self.main_hand {
                    if main.is_two_handed() {
                        return Err("Cannot equip off-hand item when main-hand weapon requires two hands.");
                    }
                }
                if weapon.is_two_handed() {
                    return Err("Two-handed weapons cannot be held in the off-hand.");
                }
                self.off_hand = Some(weapon);
                Ok(())
            }
        }
    }

    pub fn unequip(&mut self, slot: HandSlot) -> Option<WeaponDef> {
        match slot {
            HandSlot::MainHand => self.main_hand.take(),
            HandSlot::OffHand => self.off_hand.take(),
        }
    }

    pub fn switch_primary_hand(&mut self) {
        self.primary_hand = match self.primary_hand {
            HandSide::Right => HandSide::Left,
            HandSide::Left => HandSide::Right,
        };
    }

    pub fn swap_hands(&mut self) -> Result<(), &'static str> {
        if let Some(ref m) = self.main_hand {
            if m.is_two_handed() && self.off_hand.is_some() {
                return Err("Cannot swap two-handed weapon into occupied off-hand.");
            }
        }
        std::mem::swap(&mut self.main_hand, &mut self.off_hand);
        Ok(())
    }

    pub fn equip_bag(&mut self, slot: usize, bag: BagContainerDef) -> Result<(), &'static str> {
        if slot >= 4 {
            return Err("Invalid bag slot. Maximum 4 bag slots.");
        }
        self.bag_slots[slot] = Some(bag);
        Ok(())
    }

    pub fn unequip_bag(&mut self, slot: usize) -> Option<BagContainerDef> {
        if slot >= 4 {
            return None;
        }
        self.bag_slots[slot].take()
    }

    pub fn total_inventory_slots(&self) -> usize {
        16 + self.bag_slots.iter().flatten().map(|b| b.capacity).sum::<usize>()
    }

    pub fn is_dual_wielding(&self) -> bool {
        self.main_hand.is_some() && self.off_hand.is_some()
    }

    pub fn is_hybrid_melee_ranged(&self) -> bool {
        match (&self.main_hand, &self.off_hand) {
            (Some(m), Some(o)) => (m.is_ranged() && !o.is_ranged()) || (!m.is_ranged() && o.is_ranged()),
            _ => false,
        }
    }

    pub fn has_weapon_equipped(&self) -> bool {
        self.main_hand.is_some() || self.off_hand.is_some()
    }
}

// ----------------------------------------------------------------------------
// CLASSLESS PROGRESSION & WEAPON EXPERIENCE
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterCombatSkills {
    pub generic_physical: u32, // 1 to 100
    pub weapon_xp: BTreeMap<WeaponCategory, u32>,
}

impl Default for CharacterCombatSkills {
    fn default() -> Self {
        let mut xp = BTreeMap::new();
        xp.insert(WeaponCategory::Edged, 100);
        xp.insert(WeaponCategory::Pointed, 100);
        xp.insert(WeaponCategory::Blunt, 100);
        xp.insert(WeaponCategory::TwoHanded, 100);
        xp.insert(WeaponCategory::Polearm, 100);
        xp.insert(WeaponCategory::Brawling, 100);
        xp.insert(WeaponCategory::Missile, 100);
        xp.insert(WeaponCategory::Firearm, 100);
        xp.insert(WeaponCategory::Runestaff, 100);
        xp.insert(WeaponCategory::Comedic, 100);

        Self {
            generic_physical: 10,
            weapon_xp: xp,
        }
    }
}

impl CharacterCombatSkills {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_skill_level(&self, category: WeaponCategory) -> u32 {
        let total_xp = self.weapon_xp.get(&category).copied().unwrap_or(0);
        // Level calculation: 100 XP per level, min level 1, max 100
        (total_xp / 100).clamp(1, 100)
    }

    pub fn add_experience(&mut self, category: WeaponCategory, amount: u32) {
        let current = self.weapon_xp.entry(category).or_insert(0);
        *current = current.saturating_add(amount);

        // Physical skill naturally grows with combat exertion
        if *current % 500 == 0 && self.generic_physical < 100 {
            self.generic_physical += 1;
        }
    }

    pub fn dual_wield_penalty(&self) -> f32 {
        // Base penalty is 30% without training, reduced by physical skill
        let reduction = (self.generic_physical as f32 / 100.0) * 0.30;
        (0.30 - reduction).max(0.0)
    }

    pub fn damage_scaling_multiplier(&self, category: WeaponCategory) -> f32 {
        let level = self.get_skill_level(category);
        1.0 + (level as f32 * 0.01) // +1% damage per skill level
    }

    pub fn critical_strike_chance(&self, category: WeaponCategory) -> f32 {
        let level = self.get_skill_level(category);
        0.05 + (level as f32 * 0.003) // 5% base + up to 30% from mastery
    }
}

// ----------------------------------------------------------------------------
// COMBAT MANEUVERS, TECHNIQUES & SENSORY FEEDBACK VERBS
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombatManeuver {
    Thrust,
    Slash,
    Chop,
    DrawCut,
    Parry,
    Riposte,
    Feint,
    Lunge,
    Guard,
    Disarm,
    PommelStrike,
    HiltPunch,
    Bind,
    Dodge,
    Grapple,
    Bash,
    Flail,
    Throw,
    Stab,
    Pierce,
    Deflect,
    Counter,
    Cleave,
    Strike,
    Swing,
    OverheadSlash,
    UpwardSlash,
    SideStep,
    Charge,
    Sweep,
    Skewer,
    Rake,
    Shred,
    Puncture,
    Slice,
    Impale,
    Hack,
}

impl CombatManeuver {
    pub fn damage_multiplier(&self) -> f32 {
        match self {
            Self::Chop | Self::Cleave | Self::OverheadSlash => 1.45,
            Self::Impale | Self::Skewer | Self::Lunge => 1.35,
            Self::Thrust | Self::Stab | Self::Pierce | Self::Hack => 1.20,
            Self::Slash | Self::Slice | Self::Sweep => 1.0,
            Self::Riposte | Self::Counter => 1.50, // Counter-attack bonus
            Self::PommelStrike | Self::HiltPunch | Self::Bash => 0.65, // Stagger focus
            Self::DrawCut | Self::Rake | Self::Shred | Self::Puncture => 0.90,
            Self::Flail => 0.80, // Wild and erratic
            Self::Throw => 1.10,
            Self::Feint | Self::Parry | Self::Guard | Self::Bind | Self::Dodge | Self::SideStep | Self::Deflect => 0.0,
            _ => 1.0,
        }
    }

    pub fn armor_penetration_bonus(&self) -> f32 {
        match self {
            Self::Thrust | Self::Stab | Self::Pierce | Self::Skewer | Self::Impale => 0.35,
            Self::Chop | Self::OverheadSlash | Self::Bash => 0.20,
            _ => 0.0,
        }
    }

    pub fn stagger_potency(&self) -> f32 {
        match self {
            Self::Bash | Self::PommelStrike | Self::HiltPunch => 2.5,
            Self::Cleave | Self::Chop | Self::Charge => 1.8,
            Self::Sweep => 2.0,
            _ => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombatFeedbackVerb {
    // Acoustic & Vibration
    Hum,
    Whistle,
    Sing,
    Vibrate,
    Whine,
    Ring,
    Resonate,
    Echo,
    Clatter,
    Jar,
    // Comedic & Cartoon Acoustic Feedback
    Bonk,
    Clang,
    Whack,
    Kaboom,
    Boing,
    Yeet,
    // Sparks & Fluid dynamics
    Spit,
    Shower,
    Fountain,
    Spew,
    RainSparks,
    // Material & Durability Degradation
    Shatter,
    Crack,
    Crumble,
    Chip,
    Bend,
    Snap,
    Warp,
    Fold,
    Rust,
    Decay,
    Glow,
    Melt,
    Deform,
    Twist,
    // Kinematic reactions
    Bounce,
    Tumble,
    Drop,
    Fall,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CombatResolutionResult {
    pub hit_landed: bool,
    pub raw_damage: f32,
    pub final_damage: f32,
    pub armor_absorbed: f32,
    pub was_critical: bool,
    pub was_mini_crit: bool,
    pub was_parried: bool,
    pub was_deflected: bool,
    pub knockback_impulse: f32,
    pub comic_label: Option<String>,
    pub primary_verb: CombatFeedbackVerb,
    pub secondary_verb: Option<CombatFeedbackVerb>,
    pub durability_loss: u32,
    pub weapon_broken: bool,
}

pub fn resolve_weapon_attack(
    attacker_skills: &CharacterCombatSkills,
    weapon: &mut WeaponDef,
    maneuver: CombatManeuver,
    slot: HandSlot,
    is_dual_wielding: bool,
    target_armor: f32,
    target_has_runestaff: bool,
) -> CombatResolutionResult {
    // 1. Maneuver and skill damage scaling
    let skill_mult = attacker_skills.damage_scaling_multiplier(weapon.category);
    let maneuver_mult = maneuver.damage_multiplier();
    let dw_penalty = if is_dual_wielding && slot == HandSlot::OffHand {
        attacker_skills.dual_wield_penalty()
    } else {
        0.0
    };

    let base = weapon.base_damage * skill_mult * maneuver_mult * (1.0 - dw_penalty);

    // 2. Critical & Mini-Critical strike check (TF2 trope)
    let crit_chance = attacker_skills.critical_strike_chance(weapon.category);
    let was_crit = crit_chance > 0.25; // deterministic threshold check
    let was_mini_crit = !was_crit && crit_chance > 0.12;

    let damage_after_crit = if was_crit {
        base * 1.5
    } else if was_mini_crit {
        base * 1.35
    } else {
        base
    };

    // 3. Magical Runestaff Deflection check
    if target_has_runestaff && (weapon.is_ranged() || weapon.damage_type == DamageType::ArcaneForce) {
        return CombatResolutionResult {
            hit_landed: false,
            raw_damage: damage_after_crit,
            final_damage: 0.0,
            armor_absorbed: 0.0,
            was_critical: false,
            was_mini_crit: false,
            was_parried: false,
            was_deflected: true,
            knockback_impulse: 5.0,
            comic_label: Some("DEFLECTED!".to_string()),
            primary_verb: CombatFeedbackVerb::Resonate,
            secondary_verb: Some(CombatFeedbackVerb::Whine),
            durability_loss: 1,
            weapon_broken: false,
        };
    }

    // 4. Armor absorption with penetration
    let effective_armor_pen = (weapon.armor_penetration + maneuver.armor_penetration_bonus()).min(1.0);
    let effective_armor = target_armor * (1.0 - effective_armor_pen);
    let absorbed = effective_armor.min(damage_after_crit * 0.75);
    let final_dmg = (damage_after_crit - absorbed).max(1.0);

    // 5. Durability wear and feedback verb generation
    let mut wear = 1;
    let mut primary_verb = match weapon.damage_type {
        DamageType::Slashing => CombatFeedbackVerb::Clatter,
        DamageType::Piercing => CombatFeedbackVerb::Ring,
        DamageType::Bludgeoning => CombatFeedbackVerb::Jar,
        DamageType::Ballistic => CombatFeedbackVerb::RainSparks,
        DamageType::PelletSpread => CombatFeedbackVerb::Shower,
        DamageType::FireSplash => CombatFeedbackVerb::Melt,
        DamageType::ArcaneForce => CombatFeedbackVerb::Hum,
        DamageType::CartoonBonk => {
            if was_crit {
                CombatFeedbackVerb::Kaboom
            } else if weapon.name.contains("Pan") {
                CombatFeedbackVerb::Clang
            } else {
                CombatFeedbackVerb::Bonk
            }
        }
    };

    let mut secondary_verb = None;
    if target_armor > 40.0 {
        primary_verb = CombatFeedbackVerb::RainSparks;
        secondary_verb = Some(CombatFeedbackVerb::Clatter);
        wear += 1;
    }

    if weapon.durability > 0 {
        weapon.durability = weapon.durability.saturating_sub(wear);
    }
    let broken = weapon.durability == 0;
    if broken {
        primary_verb = CombatFeedbackVerb::Shatter;
        secondary_verb = Some(CombatFeedbackVerb::Snap);
    }

    // 6. Exaggerated Comedic Knockback Impulse & Comic Label
    let kb_mult = if was_crit {
        2.2
    } else if was_mini_crit {
        1.5
    } else {
        1.0
    };
    let knockback_impulse = weapon.knockback_force * kb_mult * maneuver.stagger_potency();
    if knockback_impulse > 75.0 && secondary_verb.is_none() {
        secondary_verb = Some(CombatFeedbackVerb::Yeet);
    }

    let comic_label = if was_crit {
        if weapon.name == "Frying Pan" {
            Some(format!("CRIT! CLANG! {:.0}", final_dmg))
        } else if weapon.damage_type == DamageType::CartoonBonk {
            Some(format!("CRIT! BONK! {:.0}", final_dmg))
        } else {
            Some(format!("CRIT! {:.0}", final_dmg))
        }
    } else if was_mini_crit {
        Some(format!("MINI-CRIT! {:.0}", final_dmg))
    } else if weapon.name == "Frying Pan" {
        Some("CLANG!".to_string())
    } else if weapon.damage_type == DamageType::CartoonBonk {
        Some("BONK!".to_string())
    } else {
        None
    };

    CombatResolutionResult {
        hit_landed: true,
        raw_damage: damage_after_crit,
        final_damage: final_dmg,
        armor_absorbed: absorbed,
        was_critical: was_crit,
        was_mini_crit,
        was_parried: false,
        was_deflected: false,
        knockback_impulse,
        comic_label,
        primary_verb,
        secondary_verb,
        durability_loss: wear,
        weapon_broken: broken,
    }
}

// ----------------------------------------------------------------------------
// WEAPON CATALOG FACTORY
// ----------------------------------------------------------------------------

pub fn create_weapon(name: &str) -> Option<WeaponDef> {
    match name {
        // One-Handed Edged Weapons
        "Longsword" => Some(WeaponDef {
            id: "longsword".into(),
            name: "Longsword".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Edged,
            damage_type: DamageType::Slashing,
            base_damage: 32.0,
            attack_speed: 1.3,
            reach_meters: 1.6,
            armor_penetration: 0.15,
            durability: 180,
            max_durability: 180,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 25.0,
        }),
        "Handaxe" => Some(WeaponDef {
            id: "handaxe".into(),
            name: "Handaxe".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Edged,
            damage_type: DamageType::Slashing,
            base_damage: 28.0,
            attack_speed: 1.4,
            reach_meters: 1.2,
            armor_penetration: 0.20,
            durability: 150,
            max_durability: 150,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 22.0,
        }),
        "Dagger" => Some(WeaponDef {
            id: "dagger".into(),
            name: "Dagger".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Edged,
            damage_type: DamageType::Piercing,
            base_damage: 18.0,
            attack_speed: 2.2,
            reach_meters: 0.8,
            armor_penetration: 0.30,
            durability: 120,
            max_durability: 120,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 10.0,
        }),

        // One-Handed Pointed Weapons
        "Rapier" => Some(WeaponDef {
            id: "rapier".into(),
            name: "Rapier".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Pointed,
            damage_type: DamageType::Piercing,
            base_damage: 26.0,
            attack_speed: 1.7,
            reach_meters: 1.8,
            armor_penetration: 0.40,
            durability: 130,
            max_durability: 130,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 15.0,
        }),

        // One-Handed Blunt Weapons
        "Warhammer" => Some(WeaponDef {
            id: "warhammer".into(),
            name: "Warhammer".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Blunt,
            damage_type: DamageType::Bludgeoning,
            base_damage: 34.0,
            attack_speed: 1.1,
            reach_meters: 1.3,
            armor_penetration: 0.50,
            durability: 200,
            max_durability: 200,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 42.0,
        }),
        "Club" => Some(WeaponDef {
            id: "club".into(),
            name: "Club".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Blunt,
            damage_type: DamageType::Bludgeoning,
            base_damage: 24.0,
            attack_speed: 1.2,
            reach_meters: 1.1,
            armor_penetration: 0.10,
            durability: 100,
            max_durability: 100,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 28.0,
        }),

        // Two-Handed Weapons
        "Greatsword" => Some(WeaponDef {
            id: "greatsword".into(),
            name: "Greatsword".into(),
            grip: WeaponGrip::TwoHanded,
            category: WeaponCategory::TwoHanded,
            damage_type: DamageType::Slashing,
            base_damage: 62.0,
            attack_speed: 0.85,
            reach_meters: 2.3,
            armor_penetration: 0.25,
            durability: 250,
            max_durability: 250,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 48.0,
        }),
        "Maul" => Some(WeaponDef {
            id: "maul".into(),
            name: "Maul".into(),
            grip: WeaponGrip::TwoHanded,
            category: WeaponCategory::TwoHanded,
            damage_type: DamageType::Bludgeoning,
            base_damage: 70.0,
            attack_speed: 0.70,
            reach_meters: 2.0,
            armor_penetration: 0.65,
            durability: 280,
            max_durability: 280,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 60.0,
        }),

        // Polearms
        "Halberd" => Some(WeaponDef {
            id: "halberd".into(),
            name: "Halberd".into(),
            grip: WeaponGrip::Polearm,
            category: WeaponCategory::Polearm,
            damage_type: DamageType::Slashing,
            base_damage: 54.0,
            attack_speed: 0.95,
            reach_meters: 3.2,
            armor_penetration: 0.35,
            durability: 220,
            max_durability: 220,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 38.0,
        }),
        "Spear" | "Flint Spear" => Some(WeaponDef {
            id: "spear".into(),
            name: "Spear".into(),
            grip: WeaponGrip::Polearm,
            category: WeaponCategory::Polearm,
            damage_type: DamageType::Piercing,
            base_damage: 42.0,
            attack_speed: 1.25,
            reach_meters: 3.5,
            armor_penetration: 0.45,
            durability: 190,
            max_durability: 190,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 22.0,
        }),

        // Brawling Weapons
        "Cestus" => Some(WeaponDef {
            id: "cestus".into(),
            name: "Cestus".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Brawling,
            damage_type: DamageType::Bludgeoning,
            base_damage: 16.0,
            attack_speed: 2.8,
            reach_meters: 0.6,
            armor_penetration: 0.10,
            durability: 160,
            max_durability: 160,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 15.0,
        }),
        "Knuckle-Duster" => Some(WeaponDef {
            id: "knuckle_duster".into(),
            name: "Knuckle-Duster".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Brawling,
            damage_type: DamageType::Bludgeoning,
            base_damage: 14.0,
            attack_speed: 3.0,
            reach_meters: 0.5,
            armor_penetration: 0.15,
            durability: 140,
            max_durability: 140,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 12.0,
        }),

        // Comedic / TF2 Style Weapons
        "Frying Pan" => Some(WeaponDef {
            id: "frying_pan".into(),
            name: "Frying Pan".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Comedic,
            damage_type: DamageType::CartoonBonk,
            base_damage: 45.0,
            attack_speed: 1.8,
            reach_meters: 1.2,
            armor_penetration: 0.15,
            durability: 200,
            max_durability: 200,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 65.0, // High comical launch impulse!
        }),
        "Holy Mackerel" => Some(WeaponDef {
            id: "holy_mackerel".into(),
            name: "Holy Mackerel".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Comedic,
            damage_type: DamageType::CartoonBonk,
            base_damage: 20.0,
            attack_speed: 3.2,
            reach_meters: 0.8,
            armor_penetration: 0.05,
            durability: 150,
            max_durability: 150,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: None,
            knockback_force: 35.0,
        }),
        "Bouncy Bomb Launcher" => Some(WeaponDef {
            id: "bouncy_bomb_launcher".into(),
            name: "Bouncy Bomb Launcher".into(),
            grip: WeaponGrip::TwoHanded,
            category: WeaponCategory::Comedic,
            damage_type: DamageType::FireSplash,
            base_damage: 85.0,
            attack_speed: 0.9,
            reach_meters: 50.0,
            armor_penetration: 0.40,
            durability: 180,
            max_durability: 180,
            magazine_capacity: Some(4),
            current_ammo: 4,
            projectile_profile: Some(ProjectileProfile {
                kind: ProjectileKind::BouncyBomb,
                muzzle_velocity: 40.0,
                gravity: 8.0,
                drag: 0.001,
                spread_radians: 0.02,
                pellet_count: 1,
                blast_radius: 5.0,
                is_slow_projectile: false,
            }),
            knockback_force: 80.0, // Massive explosive yeet!
        }),

        // 1H Ranged (Dual-wieldable)
        "Hand Crossbow" | "Crossbow" => Some(WeaponDef {
            id: "hand_crossbow".into(),
            name: "Hand Crossbow".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Missile,
            damage_type: DamageType::Piercing,
            base_damage: 28.0,
            attack_speed: 1.0,
            reach_meters: 30.0,
            armor_penetration: 0.25,
            durability: 110,
            max_durability: 110,
            magazine_capacity: Some(1),
            current_ammo: 1,
            projectile_profile: Some(ProjectileProfile {
                kind: ProjectileKind::HandCrossbowBolt,
                muzzle_velocity: 42.0,
                gravity: 5.5,
                drag: 0.001,
                spread_radians: 0.02,
                pellet_count: 1,
                blast_radius: 0.0,
                is_slow_projectile: false,
            }),
            knockback_force: 15.0,
        }),
        "Revolver" => Some(WeaponDef {
            id: "revolver".into(),
            name: "Revolver".into(),
            grip: WeaponGrip::OneHanded,
            category: WeaponCategory::Firearm,
            damage_type: DamageType::Ballistic,
            base_damage: 46.0,
            attack_speed: 2.0,
            reach_meters: 45.0,
            armor_penetration: 0.35,
            durability: 200,
            max_durability: 200,
            magazine_capacity: Some(6),
            current_ammo: 6,
            projectile_profile: Some(ProjectileProfile {
                kind: ProjectileKind::RevolverBullet,
                muzzle_velocity: 180.0,
                gravity: 2.0,
                drag: 0.0005,
                spread_radians: 0.035,
                pellet_count: 1,
                blast_radius: 0.0,
                is_slow_projectile: false,
            }),
            knockback_force: 28.0,
        }),

        // 2H Ranged Weapons (Heavy & Specialized)
        "Longbow" | "Crude Bow" | "Bow" => Some(WeaponDef {
            id: "longbow".into(),
            name: "Longbow".into(),
            grip: WeaponGrip::TwoHanded,
            category: WeaponCategory::Missile,
            damage_type: DamageType::Piercing,
            base_damage: 48.0,
            attack_speed: 0.9,
            reach_meters: 90.0,
            armor_penetration: 0.30,
            durability: 150,
            max_durability: 150,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: Some(ProjectileProfile {
                kind: ProjectileKind::Arrow,
                muzzle_velocity: 60.0,
                gravity: 4.8,
                drag: 0.001,
                spread_radians: 0.015,
                pellet_count: 1,
                blast_radius: 0.0,
                is_slow_projectile: false,
            }),
            knockback_force: 20.0,
        }),
        "Shotgun" => Some(WeaponDef {
            id: "shotgun".into(),
            name: "Shotgun".into(),
            grip: WeaponGrip::TwoHanded,
            category: WeaponCategory::Firearm,
            damage_type: DamageType::PelletSpread,
            base_damage: 18.0, // Per pellet (12 pellets = 216 max point-blank damage)
            attack_speed: 0.8,
            reach_meters: 25.0,
            armor_penetration: 0.15,
            durability: 180,
            max_durability: 180,
            magazine_capacity: Some(2),
            current_ammo: 2,
            projectile_profile: Some(ProjectileProfile {
                kind: ProjectileKind::ShotgunPellet,
                muzzle_velocity: 120.0,
                gravity: 3.5,
                drag: 0.004,
                spread_radians: 0.09, // Wide spread cone
                pellet_count: 12,
                blast_radius: 0.0,
                is_slow_projectile: false,
            }),
            knockback_force: 55.0,
        }),
        "Sniper Rifle" => Some(WeaponDef {
            id: "sniper_rifle".into(),
            name: "Sniper Rifle".into(),
            grip: WeaponGrip::TwoHanded,
            category: WeaponCategory::Firearm,
            damage_type: DamageType::Ballistic,
            base_damage: 160.0,
            attack_speed: 0.45,
            reach_meters: 250.0,
            armor_penetration: 0.80, // High penetration
            durability: 240,
            max_durability: 240,
            magazine_capacity: Some(4),
            current_ammo: 4,
            projectile_profile: Some(ProjectileProfile {
                kind: ProjectileKind::SniperBullet,
                muzzle_velocity: 820.0, // High-speed round
                gravity: 0.8,
                drag: 0.0001,
                spread_radians: 0.002, // Pinpoint precision
                pellet_count: 1,
                blast_radius: 0.0,
                is_slow_projectile: false,
            }),
            knockback_force: 45.0,
        }),

        // Runestaves & Magical Staves
        "Runestaff" => Some(WeaponDef {
            id: "runestaff".into(),
            name: "Runestaff".into(),
            grip: WeaponGrip::TwoHanded,
            category: WeaponCategory::Runestaff,
            damage_type: DamageType::Bludgeoning,
            base_damage: 22.0,
            attack_speed: 1.1,
            reach_meters: 2.2,
            armor_penetration: 0.10,
            durability: 260,
            max_durability: 260,
            magazine_capacity: None,
            current_ammo: 0,
            projectile_profile: Some(ProjectileProfile {
                kind: ProjectileKind::MagicMissile,
                muzzle_velocity: 55.0,
                gravity: 0.0,
                drag: 0.0,
                spread_radians: 0.005,
                pellet_count: 1,
                blast_radius: 0.0,
                is_slow_projectile: false,
            }),
            knockback_force: 25.0,
        }),

        _ => None,
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
    HandCrossbowBolt,
    RevolverBullet,
    ShotgunPellet,
    SniperBullet,
    CatapultRock,
    TrebuchetShell,
    BallistaSpear,
    MagicMissile,
    FireballBall,
    BouncyBomb,
    Rocket,
}

impl ProjectileKind {
    pub fn gravity(&self) -> f32 {
        match self {
            Self::Arrow => 9.81,
            Self::HandCrossbowBolt => 5.5,
            Self::RevolverBullet => 2.0,
            Self::ShotgunPellet => 3.5,
            Self::SniperBullet => 0.8,
            Self::CatapultRock => 15.0,
            Self::TrebuchetShell => 12.0,
            Self::BallistaSpear => 4.5,
            Self::MagicMissile => 0.0,
            Self::FireballBall => 1.2, // Slight floating arc
            Self::BouncyBomb => 8.0,
            Self::Rocket => 0.4, // Straight rocket trajectory
        }
    }

    pub fn base_speed(&self) -> f32 {
        match self {
            Self::Arrow => 45.0,
            Self::HandCrossbowBolt => 42.0,
            Self::RevolverBullet => 180.0,
            Self::ShotgunPellet => 120.0,
            Self::SniperBullet => 820.0,
            Self::CatapultRock => 25.0,
            Self::TrebuchetShell => 35.0,
            Self::BallistaSpear => 60.0,
            Self::MagicMissile => 55.0,
            Self::FireballBall => 18.0, // Slow moving projectile
            Self::BouncyBomb => 40.0,
            Self::Rocket => 75.0,
        }
    }

    pub fn base_damage(&self) -> f32 {
        match self {
            Self::Arrow => 25.0,
            Self::HandCrossbowBolt => 28.0,
            Self::RevolverBullet => 46.0,
            Self::ShotgunPellet => 18.0,
            Self::SniperBullet => 160.0,
            Self::CatapultRock => 120.0,
            Self::TrebuchetShell => 250.0,
            Self::BallistaSpear => 85.0,
            Self::MagicMissile => 35.0,
            Self::FireballBall => 110.0,
            Self::BouncyBomb => 95.0,
            Self::Rocket => 135.0,
        }
    }

    pub fn blast_radius(&self) -> f32 {
        match self {
            Self::FireballBall => 4.5,
            Self::CatapultRock => 3.5,
            Self::TrebuchetShell => 6.0,
            Self::BouncyBomb => 5.0,
            Self::Rocket => 4.0,
            _ => 0.0,
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
