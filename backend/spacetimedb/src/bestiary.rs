// ----------------------------------------------------------------------------
// AUTHORITATIVE SERVER BESTIARY: MONSTER ARCHETYPES & HITBOX PROFILES
// ----------------------------------------------------------------------------
// Architectural Note: Defines authoritative physics geometry, speeds, health,
// and default equipment loadouts for all creatures and mob types.
// Prevents hit registration discrepancies between client rendering and server raycasts.

use crate::ai::AiType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColliderShape {
    Cuboid,
    Capsule,
}

#[derive(Clone, Debug)]
pub struct MonsterArchetype {
    pub species_name: &'static str,
    pub ai_type: AiType,
    pub base_health: f32,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub aggro_range_sq: f32,
    pub flee_range_sq: f32,
    pub attack_range_sq: f32,
    pub attack_interval: f32,
    pub collider_shape: ColliderShape,
    /// For Cuboid: (half_x, half_y, half_z).
    /// For Capsule: (radius, half_height, 0.0).
    pub collider_half_extents: (f32, f32, f32),
    /// Y offset relative to transform.y (which is ground_y + 1.05)
    pub vertical_offset: f32,
    pub default_main_hand: &'static str,
    pub default_off_hand: &'static str,
    pub harvest_item: &'static str,
    pub harvest_amount: u32,
}

pub static BESTIARY: &[MonsterArchetype] = &[
    MonsterArchetype {
        species_name: "Boar",
        ai_type: AiType::Boar,
        base_health: 65.0,
        walk_speed: 2.0,
        run_speed: 6.0,
        aggro_range_sq: 225.0, // 15m
        flee_range_sq: 0.0,
        attack_range_sq: 6.25, // 2.5m
        attack_interval: 1.2,
        collider_shape: ColliderShape::Cuboid,
        collider_half_extents: (0.45, 0.48, 0.72), // 0.9m wide, 0.96m tall, 1.44m long
        vertical_offset: -0.58, // Vertically centered at ground_y + 0.47
        default_main_hand: "Boar Tusks",
        default_off_hand: "None",
        harvest_item: "Cooked Meat",
        harvest_amount: 2,
    },
    MonsterArchetype {
        species_name: "Deer",
        ai_type: AiType::Deer,
        base_health: 40.0,
        walk_speed: 2.5,
        run_speed: 7.5,
        aggro_range_sq: 400.0, // 20m detection
        flee_range_sq: 1600.0, // Flees until 40m away
        attack_range_sq: 4.0,
        attack_interval: 1.5,
        collider_shape: ColliderShape::Cuboid,
        collider_half_extents: (0.35, 0.90, 0.60), // 0.7m wide, 1.8m tall, 1.2m long
        vertical_offset: -0.15, // Vertically centered at ground_y + 0.90
        default_main_hand: "Deer Kick",
        default_off_hand: "None",
        harvest_item: "Cooked Meat",
        harvest_amount: 2,
    },
    MonsterArchetype {
        species_name: "Goblin",
        ai_type: AiType::Goblin,
        base_health: 50.0,
        walk_speed: 2.0,
        run_speed: 5.5,
        aggro_range_sq: 256.0, // 16m
        flee_range_sq: 0.0,
        attack_range_sq: 6.25,
        attack_interval: 1.0,
        collider_shape: ColliderShape::Capsule,
        collider_half_extents: (0.35, 0.40, 0.0), // radius 0.35, half-height 0.4
        vertical_offset: -0.35,
        default_main_hand: "1h Sword",
        default_off_hand: "None",
        harvest_item: "Iron Ore",
        harvest_amount: 3,
    },
    MonsterArchetype {
        species_name: "Peasant",
        ai_type: AiType::Peasant,
        base_health: 100.0,
        walk_speed: 2.0,
        run_speed: 5.0,
        aggro_range_sq: 100.0,
        flee_range_sq: 400.0,
        attack_range_sq: 6.25,
        attack_interval: 1.2,
        collider_shape: ColliderShape::Capsule,
        collider_half_extents: (0.40, 0.50, 0.0), // radius 0.4, half-height 0.5
        vertical_offset: -0.15,
        default_main_hand: "1h Axe",
        default_off_hand: "None",
        harvest_item: "Stone",
        harvest_amount: 5,
    },
    MonsterArchetype {
        species_name: "Friendly",
        ai_type: AiType::Friendly,
        base_health: 100.0,
        walk_speed: 2.0,
        run_speed: 5.0,
        aggro_range_sq: 100.0,
        flee_range_sq: 400.0,
        attack_range_sq: 6.25,
        attack_interval: 1.2,
        collider_shape: ColliderShape::Capsule,
        collider_half_extents: (0.40, 0.50, 0.0),
        vertical_offset: -0.15,
        default_main_hand: "1h Axe",
        default_off_hand: "None",
        harvest_item: "Stone",
        harvest_amount: 5,
    },
];

pub fn get_archetype_by_ai_type(ai_type: AiType) -> &'static MonsterArchetype {
    match ai_type {
        AiType::Boar => &BESTIARY[0],
        AiType::Deer => &BESTIARY[1],
        AiType::Goblin => &BESTIARY[2],
        AiType::Peasant => &BESTIARY[3],
        AiType::Friendly => &BESTIARY[4],
    }
}

pub fn get_archetype_by_name(name: &str) -> Option<&'static MonsterArchetype> {
    BESTIARY.iter().find(|m| m.species_name.eq_ignore_ascii_case(name.trim()))
}
