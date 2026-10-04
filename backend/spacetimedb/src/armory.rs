// ----------------------------------------------------------------------------
// AUTHORITATIVE SERVER ARMORY: WEAPON DEFINITIONS & COMBAT REGISTRY
// ----------------------------------------------------------------------------
// Architectural Note: Provides the single source of truth for all weapon attributes
// across both players and NPCs. Prevents client/server stat drift, magic numbers,
// and redundant weapon parsing.

use crate::combat::ProjectileKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponCategory {
    Brawling,
    Edged,
    Blunt,
    Pointed,
    Polearm,
    TwoHanded,
    Missile,
    Firearm,
    Runestaff,
}

#[derive(Clone, Debug)]
pub struct WeaponDefinition {
    pub name: &'static str,
    pub category: WeaponCategory,
    pub is_two_handed: bool,
    pub is_offhand_capable: bool,
    pub base_damage: f32,
    pub attack_range: f32,
    pub cooldown_secs: f32,
    pub is_hitscan: bool,
    pub projectile_kind: Option<ProjectileKind>,
    pub projectile_speed: f32,
    pub gravity: f32,
    pub drag: f32,
    pub skill_category: &'static str,
    pub xp_yield: u32,
}

pub static WEAPONS: &[WeaponDefinition] = &[
    // --- Unarmed & Natural ---
    WeaponDefinition {
        name: "Unarmed",
        category: WeaponCategory::Brawling,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 12.0,
        attack_range: 2.0,
        cooldown_secs: 0.35,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Brawling",
        xp_yield: 10,
    },
    WeaponDefinition {
        name: "Boar Tusks",
        category: WeaponCategory::Pointed,
        is_two_handed: false,
        is_offhand_capable: false,
        base_damage: 18.0,
        attack_range: 2.5,
        cooldown_secs: 1.2,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Pointed",
        xp_yield: 0,
    },
    WeaponDefinition {
        name: "Deer Kick",
        category: WeaponCategory::Blunt,
        is_two_handed: false,
        is_offhand_capable: false,
        base_damage: 10.0,
        attack_range: 2.0,
        cooldown_secs: 1.5,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Blunt",
        xp_yield: 0,
    },

    // --- 1H Melee ---
    WeaponDefinition {
        name: "1h Tiger Claws",
        category: WeaponCategory::Brawling,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 20.0,
        attack_range: 2.2,
        cooldown_secs: 0.30,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Brawling",
        xp_yield: 15,
    },
    WeaponDefinition {
        name: "1h Black Jack",
        category: WeaponCategory::Blunt,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 22.0,
        attack_range: 2.1,
        cooldown_secs: 0.40,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Blunt",
        xp_yield: 15,
    },
    WeaponDefinition {
        name: "1h Sword",
        category: WeaponCategory::Edged,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 26.0,
        attack_range: 2.6,
        cooldown_secs: 0.45,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Edged",
        xp_yield: 15,
    },
    WeaponDefinition {
        name: "1h Hammer",
        category: WeaponCategory::Blunt,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 24.0,
        attack_range: 2.3,
        cooldown_secs: 0.50,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Blunt",
        xp_yield: 15,
    },
    WeaponDefinition {
        name: "1h Axe",
        category: WeaponCategory::Edged,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 25.0,
        attack_range: 2.4,
        cooldown_secs: 0.48,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Edged",
        xp_yield: 15,
    },

    // --- 2H Melee ---
    WeaponDefinition {
        name: "2h Sword",
        category: WeaponCategory::TwoHanded,
        is_two_handed: true,
        is_offhand_capable: false,
        base_damage: 48.0,
        attack_range: 3.2,
        cooldown_secs: 0.75,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "TwoHanded",
        xp_yield: 25,
    },
    WeaponDefinition {
        name: "2h Hammer",
        category: WeaponCategory::TwoHanded,
        is_two_handed: true,
        is_offhand_capable: false,
        base_damage: 52.0,
        attack_range: 3.0,
        cooldown_secs: 0.85,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "TwoHanded",
        xp_yield: 25,
    },
    WeaponDefinition {
        name: "2h Axe",
        category: WeaponCategory::TwoHanded,
        is_two_handed: true,
        is_offhand_capable: false,
        base_damage: 50.0,
        attack_range: 3.1,
        cooldown_secs: 0.80,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "TwoHanded",
        xp_yield: 25,
    },

    // --- Polearms ---
    WeaponDefinition {
        name: "Polearm Spear",
        category: WeaponCategory::Polearm,
        is_two_handed: true,
        is_offhand_capable: false,
        base_damage: 34.0,
        attack_range: 4.2,
        cooldown_secs: 0.55,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Polearm",
        xp_yield: 20,
    },
    WeaponDefinition {
        name: "Polearm Javelin",
        category: WeaponCategory::Polearm,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 45.0,
        attack_range: 50.0,
        cooldown_secs: 0.90,
        is_hitscan: false,
        projectile_kind: Some(ProjectileKind::BallistaSpear),
        projectile_speed: 40.0,
        gravity: 6.0,
        drag: 0.002,
        skill_category: "Polearm",
        xp_yield: 25,
    },
    WeaponDefinition {
        name: "Polearm Trident",
        category: WeaponCategory::Polearm,
        is_two_handed: true,
        is_offhand_capable: false,
        base_damage: 36.0,
        attack_range: 4.0,
        cooldown_secs: 0.60,
        is_hitscan: true,
        projectile_kind: None,
        projectile_speed: 0.0,
        gravity: 0.0,
        drag: 0.0,
        skill_category: "Polearm",
        xp_yield: 20,
    },

    // --- Ranged Firearms & Missiles ---
    WeaponDefinition {
        name: "1h Ranged Hand Crossbow",
        category: WeaponCategory::Missile,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 28.0,
        attack_range: 60.0,
        cooldown_secs: 0.65,
        is_hitscan: false,
        projectile_kind: Some(ProjectileKind::HandCrossbowBolt),
        projectile_speed: 42.0,
        gravity: 5.5,
        drag: 0.001,
        skill_category: "Missile",
        xp_yield: 20,
    },
    WeaponDefinition {
        name: "1h Ranged Revolver",
        category: WeaponCategory::Firearm,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 46.0,
        attack_range: 80.0,
        cooldown_secs: 0.28,
        is_hitscan: true,
        projectile_kind: Some(ProjectileKind::RevolverBullet),
        projectile_speed: 180.0,
        gravity: 2.0,
        drag: 0.0005,
        skill_category: "Firearm",
        xp_yield: 25,
    },
    WeaponDefinition {
        name: "2h Ranged Long Bow",
        category: WeaponCategory::Missile,
        is_two_handed: true,
        is_offhand_capable: false,
        base_damage: 35.0,
        attack_range: 85.0,
        cooldown_secs: 0.70,
        is_hitscan: false,
        projectile_kind: Some(ProjectileKind::Arrow),
        projectile_speed: 48.0,
        gravity: 4.8,
        drag: 0.001,
        skill_category: "Missile",
        xp_yield: 20,
    },
    WeaponDefinition {
        name: "2h Ranged Shotgun",
        category: WeaponCategory::Firearm,
        is_two_handed: true,
        is_offhand_capable: false,
        base_damage: 18.0, // per pellet
        attack_range: 40.0,
        cooldown_secs: 0.90,
        is_hitscan: true,
        projectile_kind: Some(ProjectileKind::ShotgunPellet),
        projectile_speed: 120.0,
        gravity: 3.5,
        drag: 0.004,
        skill_category: "Firearm",
        xp_yield: 30,
    },
    WeaponDefinition {
        name: "2h Ranged Sniper Rifle",
        category: WeaponCategory::Firearm,
        is_two_handed: true,
        is_offhand_capable: false,
        base_damage: 95.0,
        attack_range: 250.0,
        cooldown_secs: 1.40,
        is_hitscan: true,
        projectile_kind: Some(ProjectileKind::SniperBullet),
        projectile_speed: 300.0,
        gravity: 0.8,
        drag: 0.0002,
        skill_category: "Firearm",
        xp_yield: 40,
    },

    // --- Magic Focus & Cast ---
    WeaponDefinition {
        name: "1h Ranged Wand",
        category: WeaponCategory::Runestaff,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 24.0,
        attack_range: 50.0,
        cooldown_secs: 0.35,
        is_hitscan: false,
        projectile_kind: Some(ProjectileKind::MagicMissile),
        projectile_speed: 65.0,
        gravity: 0.8,
        drag: 0.0005,
        skill_category: "Runestaff",
        xp_yield: 20,
    },
    WeaponDefinition {
        name: "1h Ranged Orb",
        category: WeaponCategory::Runestaff,
        is_two_handed: false,
        is_offhand_capable: true,
        base_damage: 20.0,
        attack_range: 40.0,
        cooldown_secs: 0.50,
        is_hitscan: false,
        projectile_kind: Some(ProjectileKind::MagicMissile),
        projectile_speed: 35.0,
        gravity: 0.0,
        drag: 0.0002,
        skill_category: "Runestaff",
        xp_yield: 20,
    },
    WeaponDefinition {
        name: "2h Ranged Runestaff",
        category: WeaponCategory::Runestaff,
        is_two_handed: true,
        is_offhand_capable: false,
        base_damage: 38.0,
        attack_range: 70.0,
        cooldown_secs: 0.60,
        is_hitscan: false,
        projectile_kind: Some(ProjectileKind::MagicMissile),
        projectile_speed: 55.0,
        gravity: 0.4,
        drag: 0.0003,
        skill_category: "Runestaff",
        xp_yield: 25,
    },
];

pub fn get_weapon_def(raw_name: &str) -> &'static WeaponDefinition {
    let normalized = raw_name.trim();

    // Direct match
    if let Some(def) = WEAPONS.iter().find(|w| w.name.eq_ignore_ascii_case(normalized)) {
        return def;
    }

    // Keyword matching for aliases (e.g. "Revolver" -> "1h Ranged Revolver", "Stone Axe" -> "1h Axe")
    for def in WEAPONS {
        if normalized.contains(def.name) || def.name.contains(normalized) {
            return def;
        }
    }

    // Specific aliases
    if normalized.contains("Revolver") {
        return get_weapon_def("1h Ranged Revolver");
    }
    if normalized.contains("Shotgun") {
        return get_weapon_def("2h Ranged Shotgun");
    }
    if normalized.contains("Sniper") {
        return get_weapon_def("2h Ranged Sniper Rifle");
    }
    if normalized.contains("Crossbow") {
        return get_weapon_def("1h Ranged Hand Crossbow");
    }
    if normalized.contains("Bow") {
        return get_weapon_def("2h Ranged Long Bow");
    }
    if normalized.contains("Wand") {
        return get_weapon_def("1h Ranged Wand");
    }
    if normalized.contains("Orb") {
        return get_weapon_def("1h Ranged Orb");
    }
    if normalized.contains("Runestaff") || normalized.contains("Staff") {
        return get_weapon_def("2h Ranged Runestaff");
    }
    if normalized.contains("Sword") {
        return get_weapon_def("1h Sword");
    }
    if normalized.contains("Axe") {
        return get_weapon_def("1h Axe");
    }
    if normalized.contains("Hammer") {
        return get_weapon_def("1h Hammer");
    }
    if normalized.contains("Spear") {
        return get_weapon_def("Polearm Spear");
    }
    if normalized.contains("Javelin") {
        return get_weapon_def("Polearm Javelin");
    }
    if normalized.contains("Trident") {
        return get_weapon_def("Polearm Trident");
    }
    if normalized.contains("Claw") {
        return get_weapon_def("1h Tiger Claws");
    }
    if normalized.contains("Black Jack") || normalized.contains("Blackjack") {
        return get_weapon_def("1h Black Jack");
    }

    // Default fallback to Unarmed
    &WEAPONS[0]
}
