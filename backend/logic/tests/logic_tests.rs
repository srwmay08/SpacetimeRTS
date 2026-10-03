//! Comprehensive integration tests for SpacetimeRTS pure game logic.
//! Run with: cargo test -p spacetime-rts-logic

use spacetime_rts_logic::*;

// ============================================================================
// INVENTORY TESTS
// ============================================================================

mod inventory {
    use super::*;

    #[test]
    fn add_to_empty_inventory() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 10);
        assert_eq!(inv.slots.len(), 1);
        assert_eq!(inv.slots[0].item_type, "Wood");
        assert_eq!(inv.slots[0].count, 10);
        assert!(inv.is_discovered("Wood"));
    }

    #[test]
    fn add_stacks_existing() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 40);
        inv.add_item("Wood", 5);
        assert_eq!(inv.slots.len(), 1);
        assert_eq!(inv.slots[0].count, 45);
    }

    #[test]
    fn add_creates_new_slot_when_stack_full() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 50); // Max stack
        inv.add_item("Wood", 10);
        assert_eq!(inv.slots.len(), 2);
        assert_eq!(inv.slots[0].count, 50);
        assert_eq!(inv.slots[1].count, 10);
    }

    #[test]
    fn add_respects_max_slots() {
        let mut inv = Inventory::new(1);
        for i in 0..16 {
            inv.add_item(&format!("Item{}", i), 50);
        }
        inv.add_item("Wood", 10);
        assert_eq!(inv.slots.len(), 16);
    }

    #[test]
    fn add_partial_fill_when_slots_full() {
        let mut inv = Inventory::new(1);
        for i in 0..15 {
            inv.add_item(&format!("Item{}", i), 50);
        }
        inv.add_item("Wood", 45);
        inv.add_item("Wood", 10);
        assert_eq!(inv.slots.len(), 16);
        assert_eq!(inv.slots[15].count, 50);
    }

    #[test]
    fn remove_partial() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 30);
        assert!(inv.remove_item("Wood", 10));
        assert_eq!(inv.slots[0].count, 20);
    }

    #[test]
    fn remove_exact() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 10);
        assert!(inv.remove_item("Wood", 10));
        assert_eq!(inv.slots.len(), 0);
    }

    #[test]
    fn remove_insufficient() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 5);
        assert!(!inv.remove_item("Wood", 10));
        assert_eq!(inv.slots[0].count, 5);
    }

    #[test]
    fn remove_across_multiple_slots() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 50);
        inv.add_item("Wood", 25);
        assert!(inv.remove_item("Wood", 40));
        assert_eq!(inv.slots.len(), 2);
        assert_eq!(inv.count_item("Wood"), 35);
    }

    #[test]
    fn remove_cleans_empty_slots() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 10);
        inv.add_item("Ore", 5);
        inv.remove_item("Wood", 10);
        assert_eq!(inv.slots.len(), 1);
        assert_eq!(inv.slots[0].item_type, "Ore");
    }

    #[test]
    fn count_item() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 30);
        inv.add_item("Wood", 20);
        inv.add_item("Ore", 5);
        assert_eq!(inv.count_item("Wood"), 50);
        assert_eq!(inv.count_item("Ore"), 5);
        assert_eq!(inv.count_item("Stone"), 0);
    }

    #[test]
    fn has_item() {
        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 30);
        assert!(inv.has_item("Wood", 30));
        assert!(inv.has_item("Wood", 20));
        assert!(!inv.has_item("Wood", 31));
        assert!(!inv.has_item("Ore", 1));
    }

    #[test]
    fn item_discovery_tracking() {
        let mut inv = Inventory::new(1);
        assert!(!inv.is_discovered("Branch"));
        inv.add_item("Branch", 3);
        assert!(inv.is_discovered("Branch"));
        assert!(!inv.is_discovered("Flint"));
    }
}

// ============================================================================
// CRAFTING & RECIPES TESTS
// ============================================================================

mod crafting {
    use super::*;

    #[test]
    fn canonical_recipes_exist() {
        let recipes = get_canonical_recipes();
        assert!(recipes.len() >= 10);
        assert!(recipes.iter().any(|r| r.recipe_id == "Hammer"));
        assert!(recipes.iter().any(|r| r.recipe_id == "Stone Axe"));
        assert!(recipes.iter().any(|r| r.recipe_id == "Crude Bow"));
        assert!(recipes.iter().any(|r| r.recipe_id == "Hand Crossbow"));
        assert!(recipes.iter().any(|r| r.recipe_id == "Revolver"));
        assert!(recipes.iter().any(|r| r.recipe_id == "Shotgun"));
        assert!(recipes.iter().any(|r| r.recipe_id == "Sniper Rifle"));
        assert!(recipes.iter().any(|r| r.recipe_id == "Runestaff"));
    }

    #[test]
    fn craft_hand_recipe_success() {
        let recipes = get_canonical_recipes();
        let hammer_recipe = recipes.iter().find(|r| r.recipe_id == "Hammer").unwrap();

        let mut inv = Inventory::new(1);
        inv.add_item("Branch", 1);
        inv.add_item("LooseStone", 1);

        assert!(hammer_recipe.can_craft(&inv, None, false).is_ok());
        let res = hammer_recipe.craft(&mut inv, None, false);
        assert!(res.is_ok());

        assert_eq!(inv.count_item("Branch"), 0);
        assert_eq!(inv.count_item("LooseStone"), 0);
        assert_eq!(inv.count_item("Hammer"), 1);
    }

    #[test]
    fn craft_hand_recipe_missing_ingredients() {
        let recipes = get_canonical_recipes();
        let axe_recipe = recipes.iter().find(|r| r.recipe_id == "Stone Axe").unwrap();

        let mut inv = Inventory::new(1);
        inv.add_item("Branch", 1); // Missing Flint!

        assert!(axe_recipe.can_craft(&inv, None, false).is_err());
        let res = axe_recipe.craft(&mut inv, None, false);
        assert!(res.is_err());
        assert_eq!(inv.count_item("Branch"), 1);
    }

    #[test]
    fn craft_workbench_recipe_fails_without_station() {
        let recipes = get_canonical_recipes();
        let bow_recipe = recipes.iter().find(|r| r.recipe_id == "Crude Bow").unwrap();

        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 10);
        inv.add_item("Leather Scraps", 4);

        assert_eq!(
            bow_recipe.can_craft(&inv, None, false),
            Err("Missing required crafting station")
        );
    }

    #[test]
    fn craft_workbench_recipe_succeeds_with_station() {
        let recipes = get_canonical_recipes();
        let bow_recipe = recipes.iter().find(|r| r.recipe_id == "Crude Bow").unwrap();

        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 10);
        inv.add_item("Leather Scraps", 4);

        let res = bow_recipe.craft(&mut inv, Some("Workbench"), false);
        assert!(res.is_ok());
        assert_eq!(inv.count_item("Wood"), 0);
        assert_eq!(inv.count_item("Leather Scraps"), 0);
        assert_eq!(inv.count_item("Crude Bow"), 1);
    }

    #[test]
    fn craft_yields_multiple_items() {
        let recipes = get_canonical_recipes();
        let arrow_recipe = recipes.iter().find(|r| r.recipe_id == "Wood Arrow").unwrap();

        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 8);

        let res = arrow_recipe.craft(&mut inv, None, false);
        assert!(res.is_ok());
        assert_eq!(inv.count_item("Wood"), 0);
        assert_eq!(inv.count_item("Wood Arrow"), 20);
    }

    #[test]
    fn craft_shelter_requirement() {
        let custom_recipe = RecipeDefinition::new(
            "Enchanted Staff",
            "Enchanted Staff",
            1,
            "Workbench",
            true,
            vec![("Wood", 5)],
        );

        let mut inv = Inventory::new(1);
        inv.add_item("Wood", 5);

        assert_eq!(
            custom_recipe.can_craft(&inv, Some("Workbench"), false),
            Err("Requires roof/shelter")
        );
        assert!(custom_recipe.can_craft(&inv, Some("Workbench"), true).is_ok());
    }
}

// ============================================================================
// WEAPONS SYSTEM & DUAL-WIELDING LOADOUT TESTS
// ============================================================================

mod weapon_system {
    use super::*;

    #[test]
    fn weapon_catalog_categories_and_grips() {
        let longsword = create_weapon("Longsword").unwrap();
        assert_eq!(longsword.category, WeaponCategory::Edged);
        assert_eq!(longsword.grip, WeaponGrip::OneHanded);
        assert!(longsword.is_one_handed());
        assert!(!longsword.is_two_handed());

        let rapier = create_weapon("Rapier").unwrap();
        assert_eq!(rapier.category, WeaponCategory::Pointed);
        assert_eq!(rapier.grip, WeaponGrip::OneHanded);

        let warhammer = create_weapon("Warhammer").unwrap();
        assert_eq!(warhammer.category, WeaponCategory::Blunt);
        assert_eq!(warhammer.grip, WeaponGrip::OneHanded);

        let greatsword = create_weapon("Greatsword").unwrap();
        assert_eq!(greatsword.category, WeaponCategory::TwoHanded);
        assert_eq!(greatsword.grip, WeaponGrip::TwoHanded);
        assert!(greatsword.is_two_handed());

        let halberd = create_weapon("Halberd").unwrap();
        assert_eq!(halberd.category, WeaponCategory::Polearm);
        assert_eq!(halberd.grip, WeaponGrip::Polearm);
        assert!(halberd.reach_meters > 3.0); // Reach weapon

        let cestus = create_weapon("Cestus").unwrap();
        assert_eq!(cestus.category, WeaponCategory::Brawling);
        assert!(cestus.attack_speed >= 2.5); // High speed brawling
    }

    #[test]
    fn dual_wielding_two_one_handed_melee() {
        let mut loadout = EquippedLoadout::new();
        let sword = create_weapon("Longsword").unwrap();
        let dagger = create_weapon("Dagger").unwrap();

        assert!(loadout.equip(HandSlot::MainHand, sword).is_ok());
        assert!(loadout.equip(HandSlot::OffHand, dagger).is_ok());
        assert!(loadout.is_dual_wielding());
        assert!(!loadout.is_hybrid_melee_ranged());
    }

    #[test]
    fn dual_wielding_cross_category_sword_and_revolver() {
        let mut loadout = EquippedLoadout::new();
        let sword = create_weapon("Longsword").unwrap();
        let revolver = create_weapon("Revolver").unwrap();

        // Sword in main-hand, revolver in off-hand
        assert!(loadout.equip(HandSlot::MainHand, sword).is_ok());
        assert!(loadout.equip(HandSlot::OffHand, revolver).is_ok());
        assert!(loadout.is_dual_wielding());
        assert!(loadout.is_hybrid_melee_ranged());
    }

    #[test]
    fn dual_wielding_axe_and_hand_crossbow() {
        let mut loadout = EquippedLoadout::new();
        let axe = create_weapon("Handaxe").unwrap();
        let hand_crossbow = create_weapon("Hand Crossbow").unwrap();

        assert!(loadout.equip(HandSlot::MainHand, axe).is_ok());
        assert!(loadout.equip(HandSlot::OffHand, hand_crossbow).is_ok());
        assert!(loadout.is_dual_wielding());
        assert!(loadout.is_hybrid_melee_ranged());
    }

    #[test]
    fn dual_wielding_dual_ranged_gunslinger() {
        let mut loadout = EquippedLoadout::new();
        let rev1 = create_weapon("Revolver").unwrap();
        let rev2 = create_weapon("Revolver").unwrap();

        assert!(loadout.equip(HandSlot::MainHand, rev1).is_ok());
        assert!(loadout.equip(HandSlot::OffHand, rev2).is_ok());
        assert!(loadout.is_dual_wielding());
        assert!(!loadout.is_hybrid_melee_ranged()); // Both are ranged
    }

    #[test]
    fn cannot_equip_two_handed_in_offhand() {
        let mut loadout = EquippedLoadout::new();
        let sword = create_weapon("Longsword").unwrap();
        let greatsword = create_weapon("Greatsword").unwrap();

        assert!(loadout.equip(HandSlot::MainHand, sword).is_ok());
        let res = loadout.equip(HandSlot::OffHand, greatsword);
        assert_eq!(res, Err("Two-handed weapons cannot be held in the off-hand."));
    }

    #[test]
    fn cannot_equip_two_handed_mainhand_while_offhand_occupied() {
        let mut loadout = EquippedLoadout::new();
        let dagger = create_weapon("Dagger").unwrap();
        let sniper = create_weapon("Sniper Rifle").unwrap();

        assert!(loadout.equip(HandSlot::OffHand, dagger).is_ok());
        let res = loadout.equip(HandSlot::MainHand, sniper);
        assert_eq!(res, Err("Cannot equip two-handed weapon while off-hand is occupied."));
    }

    #[test]
    fn cannot_equip_offhand_when_mainhand_is_two_handed() {
        let mut loadout = EquippedLoadout::new();
        let halberd = create_weapon("Halberd").unwrap();
        let dagger = create_weapon("Dagger").unwrap();

        assert!(loadout.equip(HandSlot::MainHand, halberd).is_ok());
        let res = loadout.equip(HandSlot::OffHand, dagger);
        assert_eq!(res, Err("Cannot equip off-hand item when main-hand weapon requires two hands."));
    }
}

// ============================================================================
// RANGED WEAPONS & BALLISTICS BALANCE TESTS
// ============================================================================

mod ranged_balance {
    use super::*;

    #[test]
    fn one_handed_vs_two_handed_ranged_distinction() {
        let revolver = create_weapon("Revolver").unwrap();
        let hand_crossbow = create_weapon("Hand Crossbow").unwrap();
        let sniper = create_weapon("Sniper Rifle").unwrap();
        let shotgun = create_weapon("Shotgun").unwrap();
        let longbow = create_weapon("Longbow").unwrap();

        // 1H Ranged are dual-wieldable with compact reach
        assert_eq!(revolver.grip, WeaponGrip::OneHanded);
        assert_eq!(hand_crossbow.grip, WeaponGrip::OneHanded);
        assert!(revolver.reach_meters <= 50.0);
        assert!(hand_crossbow.reach_meters <= 35.0);

        // 2H Ranged are heavy with high range or specialized spreads
        assert_eq!(sniper.grip, WeaponGrip::TwoHanded);
        assert_eq!(shotgun.grip, WeaponGrip::TwoHanded);
        assert_eq!(longbow.grip, WeaponGrip::TwoHanded);
        assert!(sniper.reach_meters >= 200.0);
        assert!(longbow.reach_meters >= 80.0);
    }

    #[test]
    fn sniper_rifle_high_velocity_and_penetration() {
        let sniper = create_weapon("Sniper Rifle").unwrap();
        let profile = sniper.projectile_profile.as_ref().unwrap();

        assert!(profile.muzzle_velocity >= 800.0, "Sniper velocity must be near-instant/hyper-speed");
        assert!(profile.gravity < 1.0, "Sniper trajectory has minimal gravity drop");
        assert!(sniper.armor_penetration >= 0.75, "Sniper ignores most armor");
        assert_eq!(sniper.base_damage, 160.0);
    }

    #[test]
    fn shotgun_pellet_spread_and_close_quarters_power() {
        let shotgun = create_weapon("Shotgun").unwrap();
        let profile = shotgun.projectile_profile.as_ref().unwrap();

        assert_eq!(profile.pellet_count, 12, "Shotgun fires 12 spread pellets");
        assert!(profile.spread_radians > 0.05, "Cone dispersion");
        let total_point_blank_potential = shotgun.base_damage * profile.pellet_count as f32;
        assert_eq!(total_point_blank_potential, 216.0, "Point blank shotgun damage is devastating");
    }

    #[test]
    fn slow_moving_projectile_spells() {
        let fireball = ProjectileKind::FireballBall;
        let magic_missile = ProjectileKind::MagicMissile;

        // Fireball is slow with wide area-of-effect blast
        assert!(fireball.base_speed() <= 20.0, "Fireball is a slow, dodgeable projectile");
        assert!(fireball.blast_radius() >= 4.0, "Fireball has explosive AoE blast");

        // Magic missile is fast, direct, zero-g
        assert!(magic_missile.base_speed() > fireball.base_speed());
        assert_eq!(magic_missile.gravity(), 0.0, "Magic missile travels straight");
        assert_eq!(magic_missile.blast_radius(), 0.0, "Magic missile is single-target");
    }

    #[test]
    fn runestaff_magical_deflection() {
        let runestaff = create_weapon("Runestaff").unwrap();
        assert_eq!(runestaff.category, WeaponCategory::Runestaff);
        assert_eq!(runestaff.grip, WeaponGrip::TwoHanded);

        let skills = CharacterCombatSkills::new();
        let mut arrow_weapon = create_weapon("Longbow").unwrap();

        // Target with Runestaff deflecting incoming arrow
        let result = resolve_weapon_attack(
            &skills,
            &mut arrow_weapon,
            CombatManeuver::Strike,
            HandSlot::MainHand,
            false,
            0.0,
            true, // target has runestaff!
        );

        assert!(!result.hit_landed);
        assert!(result.was_deflected);
        assert_eq!(result.primary_verb, CombatFeedbackVerb::Resonate);
        assert_eq!(result.secondary_verb, Some(CombatFeedbackVerb::Whine));
    }
}

// ============================================================================
// CLASSLESS SKILLS & PROGRESSION TESTS
// ============================================================================

mod classless_progression {
    use super::*;

    #[test]
    fn skill_scaling_and_experience_gain() {
        let mut skills = CharacterCombatSkills::new();
        assert_eq!(skills.get_skill_level(WeaponCategory::Edged), 1);
        assert_eq!(skills.damage_scaling_multiplier(WeaponCategory::Edged), 1.01);

        // Train Edged weapons
        skills.add_experience(WeaponCategory::Edged, 2000);
        assert_eq!(skills.get_skill_level(WeaponCategory::Edged), 21);
        assert!((skills.damage_scaling_multiplier(WeaponCategory::Edged) - 1.21).abs() < 0.001);

        // Train Firearm without any class locks
        skills.add_experience(WeaponCategory::Firearm, 3500);
        assert_eq!(skills.get_skill_level(WeaponCategory::Firearm), 36);
        assert!((skills.damage_scaling_multiplier(WeaponCategory::Firearm) - 1.36).abs() < 0.001);
    }

    #[test]
    fn dual_wield_penalty_mitigated_by_generic_physical_skill() {
        let mut skills = CharacterCombatSkills::new();
        skills.generic_physical = 0;
        assert_eq!(skills.dual_wield_penalty(), 0.30); // 30% penalty untrained

        skills.generic_physical = 50;
        assert!((skills.dual_wield_penalty() - 0.15).abs() < 0.001); // 15% penalty half-trained

        skills.generic_physical = 100;
        assert_eq!(skills.dual_wield_penalty(), 0.0); // 0% penalty fully mastered
    }
}

// ============================================================================
// COMBAT MANEUVERS & SENSORY FEEDBACK VERBS TESTS
// ============================================================================

mod maneuvers_and_feedback {
    use super::*;

    #[test]
    fn maneuver_multipliers() {
        assert_eq!(CombatManeuver::Chop.damage_multiplier(), 1.45);
        assert_eq!(CombatManeuver::Riposte.damage_multiplier(), 1.50);
        assert_eq!(CombatManeuver::Thrust.damage_multiplier(), 1.20);
        assert_eq!(CombatManeuver::Slash.damage_multiplier(), 1.0);
        assert_eq!(CombatManeuver::PommelStrike.damage_multiplier(), 0.65);
        assert_eq!(CombatManeuver::Parry.damage_multiplier(), 0.0);
    }

    #[test]
    fn thrust_armor_penetration_bonus() {
        assert_eq!(CombatManeuver::Thrust.armor_penetration_bonus(), 0.35);
        assert_eq!(CombatManeuver::Skewer.armor_penetration_bonus(), 0.35);
        assert_eq!(CombatManeuver::Slash.armor_penetration_bonus(), 0.0);
    }

    #[test]
    fn sensory_feedback_and_durability_verbs() {
        let skills = CharacterCombatSkills::new();
        let mut sword = create_weapon("Longsword").unwrap();

        // Strike heavily armored target generates RainSparks and Clatter
        let result = resolve_weapon_attack(
            &skills,
            &mut sword,
            CombatManeuver::Slash,
            HandSlot::MainHand,
            false,
            60.0, // High armor
            false,
        );

        assert!(result.hit_landed);
        assert_eq!(result.primary_verb, CombatFeedbackVerb::RainSparks);
        assert_eq!(result.secondary_verb, Some(CombatFeedbackVerb::Clatter));
        assert!(result.durability_loss >= 2);
    }

    #[test]
    fn weapon_shattering_on_durability_depletion() {
        let skills = CharacterCombatSkills::new();
        let mut fragile_dagger = create_weapon("Dagger").unwrap();
        fragile_dagger.durability = 1;

        let result = resolve_weapon_attack(
            &skills,
            &mut fragile_dagger,
            CombatManeuver::Chop,
            HandSlot::MainHand,
            false,
            50.0,
            false,
        );

        assert!(result.weapon_broken);
        assert_eq!(result.primary_verb, CombatFeedbackVerb::Shatter);
        assert_eq!(result.secondary_verb, Some(CombatFeedbackVerb::Snap));
    }
}

// ============================================================================
// TERRAIN TESTS
// ============================================================================

mod terrain {
    use super::*;

    #[test]
    fn deterministic() {
        let h1 = get_terrain_height(10.0, 20.0);
        let h2 = get_terrain_height(10.0, 20.0);
        assert_eq!(h1, h2);
    }

    #[test]
    fn always_positive() {
        for x in -100..100 {
            for z in -100..100 {
                let h = get_terrain_height(x as f32, z as f32);
                assert!(h > 0.0, "Height at ({}, {}) was {}", x, z, h);
            }
        }
    }

    #[test]
    fn no_nan_or_infinite() {
        for x in -50..50 {
            for z in -50..50 {
                let h = get_terrain_height(x as f32, z as f32);
                assert!(!h.is_nan(), "NaN at ({}, {})", x, z);
                assert!(!h.is_infinite(), "Infinite at ({}, {})", x, z);
            }
        }
    }

    #[test]
    fn varies() {
        let h1 = get_terrain_height(0.0, 0.0);
        let h2 = get_terrain_height(50.0, 50.0);
        let h3 = get_terrain_height(-50.0, -50.0);
        assert!(h1 != h2 || h2 != h3 || h1 != h3, "Terrain appears flat");
    }

    #[test]
    fn reasonable_range() {
        for x in -200..200 {
            for z in -200..200 {
                let h = get_terrain_height(x as f32, z as f32);
                assert!(h >= 0.0 && h <= 35.0, "Height {} out of range at ({}, {})", h, x, z);
            }
        }
    }

    #[test]
    fn lake_depression() {
        let lake_center = get_terrain_height(-35.0, -35.0);
        let nearby = get_terrain_height(-20.0, -20.0);
        assert!(lake_center < nearby, "Lake center ({}) should be lower than nearby ({})", lake_center, nearby);
    }

    #[test]
    fn chunk_coordinate_quantization() {
        assert_eq!(world_to_chunk_coord(0.0, 50.0), 0);
        assert_eq!(world_to_chunk_coord(49.9, 50.0), 0);
        assert_eq!(world_to_chunk_coord(50.0, 50.0), 1);
        assert_eq!(world_to_chunk_coord(-0.1, 50.0), -1);
        assert_eq!(world_to_chunk_coord(-50.0, 50.0), -1);
        assert_eq!(world_to_chunk_coord(-50.1, 50.0), -2);
    }
}

// ============================================================================
// RESOURCE NODE TESTS
// ============================================================================

mod resource_node {
    use super::*;

    #[test]
    fn tree_creation() {
        let tree = ResourceNode::new(1, "Tree", 10.0, 5.0, 10.0, 2.0);
        assert_eq!(tree.node_type, "Tree");
        assert_eq!(tree.health, 6);
        assert_eq!(tree.required_tool, "Stone Axe");
    }

    #[test]
    fn small_tree_no_axe_required() {
        let tree = ResourceNode::new(1, "Tree", 10.0, 5.0, 10.0, 1.0);
        assert_eq!(tree.required_tool, "None");
    }

    #[test]
    fn rock_creation() {
        let rock = ResourceNode::new(2, "Rock", 20.0, 15.0, 20.0, 1.5);
        assert_eq!(rock.node_type, "Rock");
        assert_eq!(rock.health, 6);
        assert_eq!(rock.required_tool, "Pickaxe");
    }

    #[test]
    fn bush_creation() {
        let bush = ResourceNode::new(3, "Bush", 5.0, 2.0, 5.0, 1.0);
        assert_eq!(bush.node_type, "Bush");
        assert_eq!(bush.health, 1);
        assert_eq!(bush.required_tool, "None");
    }

    #[test]
    fn branch_and_flint_nodes() {
        let branch = ResourceNode::new(4, "Branch", 1.0, 1.0, 1.0, 1.0);
        assert_eq!(branch.health, 1);
        assert_eq!(branch.required_tool, "None");

        let flint = ResourceNode::new(5, "Flint", 2.0, 1.0, 2.0, 1.0);
        assert_eq!(flint.health, 1);
        assert_eq!(flint.required_tool, "None");
    }

    #[test]
    fn is_depleted() {
        let mut node = ResourceNode::new(1, "Bush", 0.0, 0.0, 0.0, 1.0);
        assert!(!node.is_depleted());
        node.health = 0;
        assert!(node.is_depleted());
    }

    #[test]
    fn harvest_bush_single_hit() {
        let mut bush = ResourceNode::new(1, "Bush", 0.0, 0.0, 0.0, 1.0);
        let result = bush.harvest();
        assert!(result.is_some());
        let (item, amount) = result.unwrap();
        assert_eq!(item, "Wood");
        assert_eq!(amount, 1);
        assert!(bush.is_depleted());
    }

    #[test]
    fn harvest_tree_partial() {
        let mut tree = ResourceNode::new(1, "Tree", 0.0, 0.0, 0.0, 2.0);
        let result = tree.harvest();
        assert!(result.is_some());
        let (item, amount) = result.unwrap();
        assert_eq!(item, "Wood");
        assert_eq!(amount, 1);
        assert!(!tree.is_depleted());
        assert_eq!(tree.health, 5);
    }

    #[test]
    fn harvest_tree_full() {
        let mut tree = ResourceNode::new(1, "Tree", 0.0, 0.0, 0.0, 1.0);
        tree.health = 1;
        
        let result = tree.harvest();
        assert!(result.is_some());
        let (item, amount) = result.unwrap();
        assert_eq!(item, "Wood");
        assert_eq!(amount, 6);
        assert!(tree.is_depleted());
    }

    #[test]
    fn harvest_rock_full() {
        let mut rock = ResourceNode::new(1, "Rock", 0.0, 0.0, 0.0, 2.0);
        rock.health = 1;
        
        let result = rock.harvest();
        assert!(result.is_some());
        let (item, amount) = result.unwrap();
        assert_eq!(item, "Stone");
        assert_eq!(amount, 8);
        assert!(rock.is_depleted());
    }

    #[test]
    fn harvest_depleted_returns_none() {
        let mut bush = ResourceNode::new(1, "Bush", 0.0, 0.0, 0.0, 1.0);
        bush.harvest();
        let result = bush.harvest();
        assert!(result.is_none());
    }
}

// ============================================================================
// COMBAT & BALLISTICS TESTS
// ============================================================================

mod combat {
    use super::*;

    #[test]
    fn health_new() {
        let hp = Health::new(100.0);
        assert_eq!(hp.current, 100.0);
        assert_eq!(hp.max, 100.0);
        assert!(!hp.is_dead());
    }

    #[test]
    fn health_damage() {
        let mut hp = Health::new(100.0);
        hp.damage(30.0);
        assert_eq!(hp.current, 70.0);
        assert!(!hp.is_dead());
    }

    #[test]
    fn health_damage_clamps_to_zero() {
        let mut hp = Health::new(100.0);
        hp.damage(150.0);
        assert_eq!(hp.current, 0.0);
        assert!(hp.is_dead());
    }

    #[test]
    fn health_heal() {
        let mut hp = Health::new(100.0);
        hp.damage(50.0);
        hp.heal(20.0);
        assert_eq!(hp.current, 70.0);
    }

    #[test]
    fn health_heal_clamps_to_max() {
        let mut hp = Health::new(100.0);
        hp.heal(50.0);
        assert_eq!(hp.current, 100.0);
    }

    #[test]
    fn ray_sphere_hit() {
        let hit = ray_sphere_intersect(
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 1.0),
            (0.0, 0.0, 5.0),
            1.0,
        );
        assert!(hit.is_some());
        assert_eq!(hit.unwrap(), 4.0);
    }

    #[test]
    fn ray_sphere_miss() {
        let hit = ray_sphere_intersect(
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 1.0),
            (10.0, 0.0, 5.0),
            1.0,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn ray_sphere_behind() {
        let hit = ray_sphere_intersect(
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 1.0),
            (0.0, 0.0, -5.0),
            1.0,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn ray_sphere_grazing() {
        let hit = ray_sphere_intersect(
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 1.0),
            (1.5, 0.0, 5.0),
            1.0,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn snapshot_interpolation_exact() {
        let s1 = Snapshot { tick_id: 10, x: 0.0, y: 0.0, z: 0.0 };
        let s2 = Snapshot { tick_id: 20, x: 10.0, y: 0.0, z: 20.0 };
        let pos = interpolate_snapshot(&s1, &s2, 10).unwrap();
        assert_eq!(pos, (0.0, 0.0, 0.0));
    }

    #[test]
    fn snapshot_interpolation_midpoint() {
        let s1 = Snapshot { tick_id: 10, x: 0.0, y: 2.0, z: 0.0 };
        let s2 = Snapshot { tick_id: 20, x: 10.0, y: 4.0, z: 20.0 };
        let pos = interpolate_snapshot(&s1, &s2, 15).unwrap();
        assert_eq!(pos, (5.0, 3.0, 10.0));
    }

    #[test]
    fn snapshot_interpolation_out_of_bounds() {
        let s1 = Snapshot { tick_id: 10, x: 0.0, y: 0.0, z: 0.0 };
        let s2 = Snapshot { tick_id: 20, x: 10.0, y: 0.0, z: 20.0 };
        assert!(interpolate_snapshot(&s1, &s2, 5).is_none());
        assert!(interpolate_snapshot(&s1, &s2, 25).is_none());
    }

    #[test]
    fn projectile_flight_simulation_arrow() {
        let pos = (0.0, 10.0, 0.0);
        let vel = (0.0, 0.0, 45.0);
        let dt = 0.1;
        let (new_pos, new_vel) = simulate_projectile_step(pos, vel, ProjectileKind::Arrow, dt);

        assert!((new_pos.2 - 4.5).abs() < 0.001);
        assert!(new_pos.1 < 10.0, "Gravity drops Y pos");
        assert!(new_vel.1 < 0.0, "Gravity pulls downward");
        assert_eq!(new_vel.2, 45.0);
    }

    #[test]
    fn projectile_magic_missile_zero_gravity() {
        let pos = (0.0, 10.0, 0.0);
        let vel = (0.0, 0.0, 30.0);
        let dt = 0.5;
        let (new_pos, new_vel) = simulate_projectile_step(pos, vel, ProjectileKind::MagicMissile, dt);

        assert_eq!(new_pos.1, 10.0, "Magic missiles travel horizontally without gravity drop");
        assert_eq!(new_vel.1, 0.0);
        assert_eq!(new_pos.2, 15.0);
    }

    #[test]
    fn projectile_properties() {
        assert_eq!(ProjectileKind::Arrow.base_damage(), 25.0);
        assert_eq!(ProjectileKind::TrebuchetShell.base_damage(), 250.0);
        assert!(ProjectileKind::BallistaSpear.base_speed() > ProjectileKind::Arrow.base_speed());
    }
}

// ============================================================================
// BUILDING & DURABILITY TESTS
// ============================================================================

mod building {
    use super::*;

    #[test]
    fn foundation_creation() {
        let f = Structure::foundation();
        assert_eq!(f.piece_type, "Foundation");
        assert_eq!(f.stability, 100);
        assert!(f.is_grounded);
        assert!(f.parent_id.is_none());
        assert_eq!(f.max_health, 400.0);
        assert_eq!(f.current_health, 400.0);
        assert!(!f.is_blueprint);
    }

    #[test]
    fn attach_wall_to_foundation() {
        let f = Structure::foundation();
        let wall = f.attach_child("Wall", 1).unwrap();
        assert_eq!(wall.piece_type, "Wall");
        assert_eq!(wall.stability, 80);
        assert!(!wall.is_grounded);
        assert_eq!(wall.parent_id, Some(f.structure_id));
        assert_eq!(wall.max_health, 200.0);
    }

    #[test]
    fn attach_floor_to_foundation() {
        let f = Structure::foundation();
        let floor = f.attach_child("Floor", 1).unwrap();
        assert_eq!(floor.stability, 75);
    }

    #[test]
    fn attach_roof_to_foundation() {
        let f = Structure::foundation();
        let roof = f.attach_child("Roof", 1).unwrap();
        assert_eq!(roof.stability, 70);
    }

    #[test]
    fn attach_ramp_to_foundation() {
        let f = Structure::foundation();
        let ramp = f.attach_child("Ramp", 1).unwrap();
        assert_eq!(ramp.stability, 75);
    }

    #[test]
    fn chain_structures() {
        let f = Structure::foundation();
        let wall = f.attach_child("Wall", 1).unwrap();
        let floor = wall.attach_child("Floor", 2).unwrap();
        assert_eq!(floor.stability, 55);
    }

    #[test]
    fn cannot_attach_to_weak_structure() {
        let mut f = Structure::foundation();
        f.stability = 15;
        assert!(f.attach_child("Wall", 1).is_none());
    }

    #[test]
    fn cannot_attach_unknown_piece() {
        let f = Structure::foundation();
        assert!(f.attach_child("Unknown", 1).is_none());
    }

    #[test]
    fn can_support_check() {
        let f = Structure::foundation();
        assert!(f.can_support(20));
        assert!(f.can_support(99));
        assert!(!f.can_support(100));
        assert!(!f.can_support(101));
    }

    #[test]
    fn blueprint_construction_progression() {
        let mut bp = Structure::new_blueprint(10, None, "Wall", 80, false);
        assert!(bp.is_blueprint);
        assert_eq!(bp.construction_progress, 0);

        assert_eq!(bp.repair(50.0), 0.0);

        let finished = bp.contribute_construction(50);
        assert!(!finished);
        assert_eq!(bp.construction_progress, 50);
        assert_eq!(bp.current_health, 100.0);

        let finished2 = bp.contribute_construction(50);
        assert!(finished2);
        assert!(!bp.is_blueprint);
        assert_eq!(bp.construction_progress, 100);
        assert_eq!(bp.current_health, 200.0);
    }

    #[test]
    fn physical_structure_damage_and_repair() {
        let mut wall = Structure::foundation().attach_child("Wall", 1).unwrap();
        assert_eq!(wall.current_health, 200.0);

        let destroyed = wall.damage(50.0);
        assert!(!destroyed);
        assert_eq!(wall.current_health, 150.0);

        let healed = wall.repair(30.0);
        assert_eq!(healed, 30.0);
        assert_eq!(wall.current_health, 180.0);

        let overhealed = wall.repair(50.0);
        assert_eq!(overhealed, 20.0);
        assert_eq!(wall.current_health, 200.0);
    }

    #[test]
    fn structure_destruction_threshold() {
        let mut wall = Structure::foundation().attach_child("Wall", 1).unwrap();
        let destroyed = wall.damage(250.0);
        assert!(destroyed);
        assert_eq!(wall.current_health, 0.0);
    }
}

// ============================================================================
// MOVEMENT TESTS
// ============================================================================

mod movement {
    use super::*;

    #[test]
    fn clamp_within_limit() {
        let (dx, dy, _dz) = clamp_movement_delta(3.0, 4.0, 0.0, 5.0);
        assert_eq!(dx, 3.0);
        assert_eq!(dy, 4.0);
    }

    #[test]
    fn clamp_exceeds_limit() {
        let (dx, dy, dz) = clamp_movement_delta(6.0, 8.0, 0.0, 5.0);
        let mag = (dx * dx + dy * dy + dz * dz).sqrt();
        assert!((mag - 5.0).abs() < 0.001);
    }

    #[test]
    fn clamp_exactly_at_limit() {
        let (dx, dy, _dz) = clamp_movement_delta(3.0, 4.0, 0.0, 5.0);
        assert_eq!(dx, 3.0);
        assert_eq!(dy, 4.0);
    }

    #[test]
    fn clamp_zero_movement() {
        let (dx, dy, dz) = clamp_movement_delta(0.0, 0.0, 0.0, 5.0);
        assert_eq!(dx, 0.0);
        assert_eq!(dy, 0.0);
        assert_eq!(dz, 0.0);
    }

    #[test]
    fn stale_tick_detection() {
        assert!(is_stale_tick(5, 10));
        assert!(is_stale_tick(10, 10));
        assert!(!is_stale_tick(11, 10));
    }
}

// ============================================================================
// AI & NPC BEHAVIOR TESTS
// ============================================================================

mod ai {
    use super::*;

    #[test]
    fn peasant_new() {
        let p = Peasant::new(1, 100);
        assert_eq!(p.entity_id, 1);
        assert_eq!(p.owner_id, 100);
        assert_eq!(p.state, AiState::Idle);
        assert_eq!(p.carrying_item, "None");
        assert_eq!(p.carrying_amount, 0);
    }

    #[test]
    fn peasant_is_carrying() {
        let mut p = Peasant::new(1, 100);
        assert!(!p.is_carrying());
        p.carrying_amount = 1;
        assert!(p.is_carrying());
    }

    #[test]
    fn peasant_is_carrying_max() {
        let mut p = Peasant::new(1, 100);
        assert!(!p.is_carrying_max(10));
        p.carrying_amount = 10;
        assert!(p.is_carrying_max(10));
        p.carrying_amount = 11;
        assert!(p.is_carrying_max(10));
    }

    #[test]
    fn peasant_stuck_tick_counting() {
        let mut p = Peasant::new(1, 100);
        for _ in 0..9 {
            assert!(!p.note_stuck_tick());
        }
        assert!(p.note_stuck_tick());
        p.reset_stuck_ticks();
        assert_eq!(p.consecutive_stuck_ticks, 0);
    }

    #[test]
    fn ai_state_equality() {
        assert_eq!(AiState::Idle, AiState::Idle);
        assert_ne!(AiState::Idle, AiState::Harvest(1));
        assert_eq!(AiState::Harvest(1), AiState::Harvest(1));
        assert_ne!(AiState::Harvest(1), AiState::Harvest(2));
    }
}

// ============================================================================
// PRNG TESTS
// ============================================================================

mod prng {
    use super::*;

    #[test]
    fn deterministic() {
        let mut p1 = Prng::new(42);
        let mut p2 = Prng::new(42);
        for _ in 0..100 {
            assert_eq!(p1.next_f32(), p2.next_f32());
        }
    }

    #[test]
    fn different_seeds_different_sequences() {
        let mut p1 = Prng::new(42);
        let mut p2 = Prng::new(43);
        let seq1: Vec<f32> = (0..10).map(|_| p1.next_f32()).collect();
        let seq2: Vec<f32> = (0..10).map(|_| p2.next_f32()).collect();
        assert_ne!(seq1, seq2);
    }

    #[test]
    fn next_f32_in_range() {
        let mut p = Prng::new(12345);
        for _ in 0..1000 {
            let v = p.next_f32();
            assert!(v >= 0.0 && v <= 1.0);
        }
    }

    #[test]
    fn next_range() {
        let mut p = Prng::new(999);
        for _ in 0..1000 {
            let v = p.next_range(-10.0, 10.0);
            assert!(v >= -10.0 && v <= 10.0);
        }
    }
}

// ============================================================================
// FACTION & PET TESTS
// ============================================================================

mod faction {
    use super::*;

    #[test]
    fn player_villager_neutral() {
        assert_eq!(get_standing(&Faction::Player, &Faction::Villager), FactionStanding::Neutral);
        assert_eq!(get_standing(&Faction::Villager, &Faction::Player), FactionStanding::Neutral);
    }

    #[test]
    fn player_goblin_hostile() {
        assert_eq!(get_standing(&Faction::Player, &Faction::Goblin), FactionStanding::KillOnSight);
        assert_eq!(get_standing(&Faction::Goblin, &Faction::Player), FactionStanding::KillOnSight);
    }

    #[test]
    fn goblin_villager_hostile() {
        assert_eq!(get_standing(&Faction::Goblin, &Faction::Villager), FactionStanding::KillOnSight);
        assert_eq!(get_standing(&Faction::Villager, &Faction::Goblin), FactionStanding::KillOnSight);
    }

    #[test]
    fn wildlife_neutral_to_all() {
        assert_eq!(get_standing(&Faction::Wildlife, &Faction::Player), FactionStanding::Neutral);
        assert_eq!(get_standing(&Faction::Player, &Faction::Wildlife), FactionStanding::Neutral);
        assert_eq!(get_standing(&Faction::Wildlife, &Faction::Villager), FactionStanding::Neutral);
        assert_eq!(get_standing(&Faction::Villager, &Faction::Wildlife), FactionStanding::Neutral);
    }

    #[test]
    fn same_faction_neutral() {
        assert_eq!(get_standing(&Faction::Player, &Faction::Player), FactionStanding::Neutral);
        assert_eq!(get_standing(&Faction::Goblin, &Faction::Goblin), FactionStanding::Neutral);
    }
}

mod pet {
    use super::*;

    #[test]
    fn new_pet_defaults() {
        let pet = PetComponent::new(1, 100);
        assert_eq!(pet.entity_id, 1);
        assert_eq!(pet.owner_id, 100);
        assert_eq!(pet.stance, PetStance::Follow);
    }

    #[test]
    fn pet_stance_combat_targeting() {
        let mut pet = PetComponent::new(1, 100);

        pet.stance = PetStance::Follow;
        assert!(!pet.should_attack_target(FactionStanding::KillOnSight, false));
        assert!(!pet.should_attack_target(FactionStanding::KillOnSight, true));

        pet.stance = PetStance::Aggressive;
        assert!(pet.should_attack_target(FactionStanding::KillOnSight, false));
        assert!(!pet.should_attack_target(FactionStanding::Neutral, false));

        pet.stance = PetStance::Defensive;
        assert!(!pet.should_attack_target(FactionStanding::KillOnSight, false));
        assert!(pet.should_attack_target(FactionStanding::KillOnSight, true));
        assert!(pet.should_attack_target(FactionStanding::Neutral, true));
    }
}

// ============================================================================
// DAY / NIGHT CYCLE TESTS
// ============================================================================

mod day_night {
    use super::*;

    #[test]
    fn advance_time_normal() {
        let t = advance_time_of_day(8.0, 2.5);
        assert!((t - 10.5).abs() < 0.001);
    }

    #[test]
    fn advance_time_wraparound_midnight() {
        let t = advance_time_of_day(23.0, 2.0);
        assert!((t - 1.0).abs() < 0.001);
    }

    #[test]
    fn advance_time_multiple_cycles() {
        let t = advance_time_of_day(0.0, 50.0);
        assert!((t - 2.0).abs() < 0.001);
    }

    #[test]
    fn daylight_schedule() {
        assert!(is_daylight(8.0));
        assert!(is_daylight(12.0));
        assert!(is_daylight(19.9));
        assert!(!is_daylight(20.0));
        assert!(!is_daylight(23.5));
        assert!(!is_daylight(4.0));
    }
}
