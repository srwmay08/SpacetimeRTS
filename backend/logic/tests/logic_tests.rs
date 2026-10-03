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
        // Fill all 16 slots with different items
        for i in 0..16 {
            inv.add_item(&format!("Item{}", i), 50);
        }
        // Try to add one more
        inv.add_item("Wood", 10);
        assert_eq!(inv.slots.len(), 16);
    }

    #[test]
    fn add_partial_fill_when_slots_full() {
        let mut inv = Inventory::new(1);
        // Fill 15 slots
        for i in 0..15 {
            inv.add_item(&format!("Item{}", i), 50);
        }
        // 16th slot partially filled
        inv.add_item("Wood", 45);
        // Add 10 more wood — 5 should fit, 5 lost
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

        // No station provided
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
            true, // Requires roof!
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
        let vel = (0.0, 0.0, 45.0); // 45 m/s forward
        let dt = 0.1;
        let (new_pos, new_vel) = simulate_projectile_step(pos, vel, ProjectileKind::Arrow, dt);

        assert!((new_pos.2 - 4.5).abs() < 0.001); // 45 * 0.1
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
        assert_eq!(wall.stability, 80); // 100 - 20
        assert!(!wall.is_grounded);
        assert_eq!(wall.parent_id, Some(f.structure_id));
        assert_eq!(wall.max_health, 200.0);
    }

    #[test]
    fn attach_floor_to_foundation() {
        let f = Structure::foundation();
        let floor = f.attach_child("Floor", 1).unwrap();
        assert_eq!(floor.stability, 75); // 100 - 25
    }

    #[test]
    fn attach_roof_to_foundation() {
        let f = Structure::foundation();
        let roof = f.attach_child("Roof", 1).unwrap();
        assert_eq!(roof.stability, 70); // 100 - 30
    }

    #[test]
    fn attach_ramp_to_foundation() {
        let f = Structure::foundation();
        let ramp = f.attach_child("Ramp", 1).unwrap();
        assert_eq!(ramp.stability, 75); // 100 - 25
    }

    #[test]
    fn chain_structures() {
        let f = Structure::foundation();
        let wall = f.attach_child("Wall", 1).unwrap();
        let floor = wall.attach_child("Floor", 2).unwrap();
        assert_eq!(floor.stability, 55); // 80 - 25
    }

    #[test]
    fn cannot_attach_to_weak_structure() {
        let mut f = Structure::foundation();
        f.stability = 15; // Too weak for wall (needs > 20)
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

        // Cannot repair blueprint
        assert_eq!(bp.repair(50.0), 0.0);

        // Contribute 50%
        let finished = bp.contribute_construction(50);
        assert!(!finished);
        assert_eq!(bp.construction_progress, 50);
        assert_eq!(bp.current_health, 100.0); // 50% of 200

        // Finish remaining 50%
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

        // Cannot overheal beyond max_health
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
        // 10th consecutive stuck tick triggers unstuck routine
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

        // Follow stance doesn't auto-attack
        pet.stance = PetStance::Follow;
        assert!(!pet.should_attack_target(FactionStanding::KillOnSight, false));
        assert!(!pet.should_attack_target(FactionStanding::KillOnSight, true));

        // Aggressive attacks any KillOnSight enemy
        pet.stance = PetStance::Aggressive;
        assert!(pet.should_attack_target(FactionStanding::KillOnSight, false));
        assert!(!pet.should_attack_target(FactionStanding::Neutral, false));

        // Defensive only attacks enemies that attack the owner
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
        let t = advance_time_of_day(0.0, 50.0); // 2 full cycles + 2 hours
        assert!((t - 2.0).abs() < 0.001);
    }

    #[test]
    fn daylight_schedule() {
        assert!(is_daylight(8.0));  // Morning
        assert!(is_daylight(12.0)); // Noon
        assert!(is_daylight(19.9)); // Dusk
        assert!(!is_daylight(20.0)); // Night
        assert!(!is_daylight(23.5)); // Midnight
        assert!(!is_daylight(4.0));  // Pre-dawn
    }
}
