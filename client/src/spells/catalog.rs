// ============================================================================
// File: client/src/spells/catalog.rs
// ============================================================================
// ----------------------------------------------------------------------------
// SPELL & ABILITY CATALOG DEFINITIONS
// ----------------------------------------------------------------------------

use bevy::prelude::*;
use spacetime_rts_logic::TacticalAbilityKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellCategory {
    Tactical,
    Elemental,
    Arcane,
    Restoration,
}

impl SpellCategory {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Tactical => "Tactical",
            Self::Elemental => "Elemental",
            Self::Arcane => "Arcane",
            Self::Restoration => "Restoration",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Tactical => Color::srgb(0.2, 0.8, 0.8),
            Self::Elemental => Color::srgb(1.0, 0.45, 0.2),
            Self::Arcane => Color::srgb(0.7, 0.4, 0.95),
            Self::Restoration => Color::srgb(0.3, 0.9, 0.4),
        }
    }
}

/// Strongly-typed identifier for all spells and tactical abilities.
/// Eliminates stringly-typed hotbars and enables zero-allocation slotting and matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpellId {
    EntanglingRoots,
    Moonfire,
    TeleportMoonglade,
    Thorns,
    Wrath,
    PhaseDash,
    SmokeVeil,
    IntelDart,
    GravLift,
    Fireball,
    MagicMissile,
    FrostNova,
    MinorHealing,
    Blink,
    ChainLightning,
    Stoneskin,
    Starfall,
    SpiritFamiliar,
    WarCry,
    ShadowCloak,
    SolarFlare,
}

impl SpellId {
    pub const ALL: &'static [SpellId] = &[
        SpellId::EntanglingRoots,
        SpellId::Moonfire,
        SpellId::TeleportMoonglade,
        SpellId::Thorns,
        SpellId::Wrath,
        SpellId::PhaseDash,
        SpellId::SmokeVeil,
        SpellId::IntelDart,
        SpellId::GravLift,
        SpellId::Fireball,
        SpellId::MagicMissile,
        SpellId::FrostNova,
        SpellId::MinorHealing,
        SpellId::Blink,
        SpellId::ChainLightning,
        SpellId::Stoneskin,
        SpellId::Starfall,
        SpellId::SpiritFamiliar,
        SpellId::WarCry,
        SpellId::ShadowCloak,
        SpellId::SolarFlare,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EntanglingRoots => "entangling_roots",
            Self::Moonfire => "moonfire",
            Self::TeleportMoonglade => "teleport_moonglade",
            Self::Thorns => "thorns",
            Self::Wrath => "wrath",
            Self::PhaseDash => "phase_dash",
            Self::SmokeVeil => "smoke_veil",
            Self::IntelDart => "intel_dart",
            Self::GravLift => "grav_lift",
            Self::Fireball => "fireball",
            Self::MagicMissile => "magic_missile",
            Self::FrostNova => "frost_nova",
            Self::MinorHealing => "minor_healing",
            Self::Blink => "blink",
            Self::ChainLightning => "chain_lightning",
            Self::Stoneskin => "stoneskin",
            Self::Starfall => "starfall",
            Self::SpiritFamiliar => "spirit_familiar",
            Self::WarCry => "war_cry",
            Self::ShadowCloak => "shadow_cloak",
            Self::SolarFlare => "solar_flare",
        }
    }

    pub fn from_id_or_name(id_or_name: &str) -> Option<Self> {
        let trimmed = id_or_name.trim();
        for &spell in Self::ALL {
            let def = spell.def();
            if def.id.eq_ignore_ascii_case(trimmed) || def.name.eq_ignore_ascii_case(trimmed) {
                return Some(spell);
            }
        }
        None
    }

    pub fn to_tactical_ability(&self) -> Option<TacticalAbilityKind> {
        match self {
            Self::PhaseDash => Some(TacticalAbilityKind::PhaseDash),
            Self::SmokeVeil => Some(TacticalAbilityKind::SmokeVeil),
            Self::IntelDart => Some(TacticalAbilityKind::IntelDart),
            Self::GravLift => Some(TacticalAbilityKind::GravLift),
            _ => None,
        }
    }

    #[inline]
    pub fn def(&self) -> &'static SpellDef {
        get_spell(*self)
    }
}

#[derive(Clone, Debug)]
pub struct SpellDef {
    pub spell_id: SpellId,
    pub id: &'static str,
    pub name: &'static str,
    pub rank: &'static str,
    pub category: SpellCategory,
    pub icon: &'static str,
    pub description: &'static str,
    pub cooldown_seconds: f32,
    pub color: Color,
}

pub const SPELL_CATALOG: &[SpellDef] = &[
    SpellDef {
        spell_id: SpellId::EntanglingRoots,
        id: "entangling_roots",
        name: "Entangling Roots",
        rank: "Rank 1",
        category: SpellCategory::Restoration,
        icon: "ROOT",
        description: "Roots the target in place, inflicting nature damage over time.",
        cooldown_seconds: 12.0,
        color: Color::srgb(0.4, 0.85, 0.3),
    },
    SpellDef {
        spell_id: SpellId::Moonfire,
        id: "moonfire",
        name: "Moonfire",
        rank: "Rank 2",
        category: SpellCategory::Arcane,
        icon: "MOON",
        description: "Strikes the target with lunar radiance, inflicting direct and ongoing arcane burn.",
        cooldown_seconds: 3.0,
        color: Color::srgb(0.6, 0.8, 1.0),
    },
    SpellDef {
        spell_id: SpellId::TeleportMoonglade,
        id: "teleport_moonglade",
        name: "Teleport: Moonglade",
        rank: "Rank 1",
        category: SpellCategory::Arcane,
        icon: "TELE",
        description: "Transports the caster across the mystic ley lines to Moonglade.",
        cooldown_seconds: 60.0,
        color: Color::srgb(0.5, 0.9, 0.8),
    },
    SpellDef {
        spell_id: SpellId::Thorns,
        id: "thorns",
        name: "Thorns",
        rank: "Rank 1",
        category: SpellCategory::Restoration,
        icon: "THRN",
        description: "Sprouts protective briars, dealing return nature damage to attackers.",
        cooldown_seconds: 20.0,
        color: Color::srgb(0.35, 0.75, 0.3),
    },
    SpellDef {
        spell_id: SpellId::Wrath,
        id: "wrath",
        name: "Wrath",
        rank: "Rank 2",
        category: SpellCategory::Elemental,
        icon: "WRTH",
        description: "Hurls a bolt of solar fury at the target, striking with searing solar force.",
        cooldown_seconds: 2.0,
        color: Color::srgb(0.95, 0.85, 0.25),
    },
    SpellDef {
        spell_id: SpellId::PhaseDash,
        id: "phase_dash",
        name: "Phase Dash",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "DASH",
        description: "Surges 32m/s forward or strafe-direction with aerodynamic slipstream.",
        cooldown_seconds: 6.0,
        color: Color::srgb(0.0, 0.9, 0.9),
    },
    SpellDef {
        spell_id: SpellId::SmokeVeil,
        id: "smoke_veil",
        name: "Smoke Veil",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "SMOK",
        description: "Deploys a dense volumetric smoke cloud obscuring lines of sight for 8s.",
        cooldown_seconds: 18.0,
        color: Color::srgb(0.65, 0.70, 0.75),
    },
    SpellDef {
        spell_id: SpellId::IntelDart,
        id: "intel_dart",
        name: "Intel Dart",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "INTL",
        description: "Fires a high-velocity sonar ping dart revealing hidden entities.",
        cooldown_seconds: 22.0,
        color: Color::srgb(1.0, 0.85, 0.2),
    },
    SpellDef {
        spell_id: SpellId::GravLift,
        id: "grav_lift",
        name: "Grav-Lift",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "LIFT",
        description: "Projects an anti-gravity vertical repulsion column propelling player upward.",
        cooldown_seconds: 15.0,
        color: Color::srgb(0.3, 0.95, 0.45),
    },
    SpellDef {
        spell_id: SpellId::Fireball,
        id: "fireball",
        name: "Fireball",
        rank: "Rank 3",
        category: SpellCategory::Elemental,
        icon: "FIRE",
        description: "Launches a slow, heavy incendiary sphere detonating with 4.5m blast radius.",
        cooldown_seconds: 4.0,
        color: Color::srgb(1.0, 0.35, 0.1),
    },
    SpellDef {
        spell_id: SpellId::MagicMissile,
        id: "magic_missile",
        name: "Magic Missile",
        rank: "Rank 2",
        category: SpellCategory::Arcane,
        icon: "MISS",
        description: "Discharges a rapid 55m/s zero-gravity arcane projectile.",
        cooldown_seconds: 1.5,
        color: Color::srgb(0.75, 0.3, 0.95),
    },
    SpellDef {
        spell_id: SpellId::FrostNova,
        id: "frost_nova",
        name: "Frost Nova",
        rank: "Rank 1",
        category: SpellCategory::Elemental,
        icon: "NOVA",
        description: "Radial freezing explosion chilling all nearby hostile creatures.",
        cooldown_seconds: 8.0,
        color: Color::srgb(0.4, 0.8, 1.0),
    },
    SpellDef {
        spell_id: SpellId::MinorHealing,
        id: "minor_healing",
        name: "Rejuvenation",
        rank: "Rank 2",
        category: SpellCategory::Restoration,
        icon: "HEAL",
        description: "Restores vital health with a rejuvenating surge of golden radiance.",
        cooldown_seconds: 10.0,
        color: Color::srgb(0.3, 1.0, 0.5),
    },
    SpellDef {
        spell_id: SpellId::Blink,
        id: "blink",
        name: "Blink",
        rank: "Rank 1",
        category: SpellCategory::Arcane,
        icon: "BLNK",
        description: "Instantly teleports 12m forward across the physical plane.",
        cooldown_seconds: 12.0,
        color: Color::srgb(0.85, 0.4, 0.9),
    },
    SpellDef {
        spell_id: SpellId::ChainLightning,
        id: "chain_lightning",
        name: "Chain Lightning",
        rank: "Rank 1",
        category: SpellCategory::Elemental,
        icon: "LGHT",
        description: "Discharges crackling high-voltage lightning arcing through targets.",
        cooldown_seconds: 9.0,
        color: Color::srgb(0.95, 0.9, 0.3),
    },
    SpellDef {
        spell_id: SpellId::Stoneskin,
        id: "stoneskin",
        name: "Barkskin",
        rank: "Rank 1",
        category: SpellCategory::Restoration,
        icon: "BARK",
        description: "Hardens flesh into earthen armor, absorbing incoming impact damage.",
        cooldown_seconds: 25.0,
        color: Color::srgb(0.75, 0.6, 0.4),
    },
    SpellDef {
        spell_id: SpellId::Starfall,
        id: "starfall",
        name: "Starfall",
        rank: "Rank 1",
        category: SpellCategory::Arcane,
        icon: "STAR",
        description: "Calls down celestial radiant sparks from the binary star system.",
        cooldown_seconds: 30.0,
        color: Color::srgb(0.95, 0.8, 1.0),
    },
    SpellDef {
        spell_id: SpellId::SpiritFamiliar,
        id: "spirit_familiar",
        name: "Bear Form",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "BEAR",
        description: "Transforms into a fierce beast, increasing health and melee durability.",
        cooldown_seconds: 40.0,
        color: Color::srgb(0.4, 0.9, 0.85),
    },
    SpellDef {
        spell_id: SpellId::WarCry,
        id: "war_cry",
        name: "Demoralizing Roar",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "ROAR",
        description: "Thunderous acoustic roar delivering kinetic knockback to nearby foes.",
        cooldown_seconds: 20.0,
        color: Color::srgb(0.9, 0.3, 0.3),
    },
    SpellDef {
        spell_id: SpellId::ShadowCloak,
        id: "shadow_cloak",
        name: "Prowl",
        rank: "Rank 1",
        category: SpellCategory::Tactical,
        icon: "PROW",
        description: "Envelops the player in shadows, reducing visual aggro profile.",
        cooldown_seconds: 35.0,
        color: Color::srgb(0.45, 0.4, 0.55),
    },
    SpellDef {
        spell_id: SpellId::SolarFlare,
        id: "solar_flare",
        name: "Sunfire",
        rank: "Rank 1",
        category: SpellCategory::Elemental,
        icon: "SUNF",
        description: "Emits a blistering beam of concentrated stellar radiation.",
        cooldown_seconds: 16.0,
        color: Color::srgb(1.0, 0.6, 0.15),
    },
];

pub fn get_spell(id: SpellId) -> &'static SpellDef {
    SPELL_CATALOG.iter().find(|s| s.spell_id == id).unwrap_or(&SPELL_CATALOG[0])
}

pub fn get_spell_by_id(id: &str) -> Option<&'static SpellDef> {
    SpellId::from_id_or_name(id).map(|s| s.def())
}

pub fn get_filtered_spells(category: Option<SpellCategory>, rank_filter: bool) -> Vec<&'static SpellDef> {
    SPELL_CATALOG.iter().filter(|s| {
        if let Some(cat) = category {
            if s.category != cat {
                return false;
            }
        }
        if rank_filter {
            s.rank.contains('1')
        } else {
            true
        }
    }).collect()
}
