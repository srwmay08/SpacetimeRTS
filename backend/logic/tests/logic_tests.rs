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
        // 50 + 25 = 75, remove 40 = 35 remaining
        // First slot: 50 - 40 = 10, second slot: 25 (unchanged)
        // But remove_item iterates and drains: slot1 50->10 (removed 40), done
        // So we have 10 + 25 = 35 across 2 slots
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
        assert_eq!(tree.health, 6); // 3.0 * 2.0
        assert_eq!(tree.required_tool, "Axe"); // scale > 1.5
    }

    #[test]
    fn small_tree_no_axe_required() {
        let tree = ResourceNode::new(1, "Tree", 10.0, 5.0, 10.0, 1.0);
        assert_eq!(tree.required_tool, "None"); // scale <= 1.5
    }

    #[test]
    fn rock_creation() {
        let rock = ResourceNode::new(2, "Rock", 20.0, 15.0, 20.0, 1.5);
        assert_eq!(rock.node_type, "Rock");
        assert_eq!(rock.health, 6); // 4.0 * 1.5
        assert_eq!(rock.required_tool, "None");
    }

    #[test]
    fn bush_creation() {
        let bush = ResourceNode::new(3, "Bush", 5.0, 2.0, 5.0, 1.0);
        assert_eq!(bush.node_type, "Bush");
        assert_eq!(bush.health, 1);
        assert_eq!(bush.required_tool, "None");
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
        // Bush falls through to the "_" match arm, returning "Wood"
        // This is the actual behavior in lib.rs — bushes give "Wood" on final hit
        assert_eq!(item, "Wood");
        assert_eq!(amount, 1); // 1 * 1.0
        assert!(bush.is_depleted());
    }

    #[test]
    fn harvest_tree_partial() {
        let mut tree = ResourceNode::new(1, "Tree", 0.0, 0.0, 0.0, 2.0);
        let result = tree.harvest();
        assert!(result.is_some());
        let (item, amount) = result.unwrap();
        assert_eq!(item, "Wood");
        assert_eq!(amount, 1); // Partial harvest gives 1
        assert!(!tree.is_depleted());
        assert_eq!(tree.health, 5);
    }

    #[test]
    fn harvest_tree_full() {
        let mut tree = ResourceNode::new(1, "Tree", 0.0, 0.0, 0.0, 1.0);
        tree.health = 1; // Set to 1 hit remaining
        
        let result = tree.harvest();
        assert!(result.is_some());
        let (item, amount) = result.unwrap();
        assert_eq!(item, "Wood");
        assert_eq!(amount, 5); // Full yield: 5 * 1.0
        assert!(tree.is_depleted());
    }

    #[test]
    fn harvest_rock_full() {
        let mut rock = ResourceNode::new(1, "Rock", 0.0, 0.0, 0.0, 2.0);
        rock.health = 1;
        
        let result = rock.harvest();
        assert!(result.is_some());
        let (item, amount) = result.unwrap();
        assert_eq!(item, "Ore");
        assert_eq!(amount, 6); // Full yield: 3 * 2.0
        assert!(rock.is_depleted());
    }

    #[test]
    fn harvest_depleted_returns_none() {
        let mut bush = ResourceNode::new(1, "Bush", 0.0, 0.0, 0.0, 1.0);
        bush.harvest(); // Depletes it
        let result = bush.harvest();
        assert!(result.is_none());
    }
}

// ============================================================================
// COMBAT TESTS
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
        // Ray from origin going +Z, sphere at (0, 0, 5) with radius 1
        let hit = ray_sphere_intersect(
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 1.0),
            (0.0, 0.0, 5.0),
            1.0,
        );
        assert!(hit.is_some());
        assert_eq!(hit.unwrap(), 4.0); // Distance to near surface
    }

    #[test]
    fn ray_sphere_miss() {
        // Ray from origin going +Z, sphere at (10, 0, 5) — way off to the side
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
        // Ray going +Z, sphere behind at -5
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
        // Ray that just barely misses (sphere at edge of radius)
        let hit = ray_sphere_intersect(
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 1.0),
            (1.5, 0.0, 5.0), // 1.5 units off-center, radius 1
            1.0,
        );
        assert!(hit.is_none());
    }
}

// ============================================================================
// BUILDING TESTS
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
    }

    #[test]
    fn attach_wall_to_foundation() {
        let f = Structure::foundation();
        let wall = f.attach_child("Wall", 1).unwrap();
        assert_eq!(wall.piece_type, "Wall");
        assert_eq!(wall.stability, 80); // 100 - 20
        assert!(!wall.is_grounded);
        assert_eq!(wall.parent_id, Some(f.structure_id));
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
        assert!(f.can_support(20)); // 100 > 20
        assert!(f.can_support(99)); // 100 > 99
        assert!(!f.can_support(100)); // 100 > 100 is false
        assert!(!f.can_support(101)); // 100 > 101 is false
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
        // Original magnitude: 10, clamped to 5
        let mag = (dx * dx + dy * dy + dz * dz).sqrt();
        assert!((mag - 5.0).abs() < 0.001);
    }

    #[test]
    fn clamp_exactly_at_limit() {
        let (dx, dy, dz) = clamp_movement_delta(3.0, 4.0, 0.0, 5.0);
        // Magnitude is exactly 5, should not change
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
        assert!(is_stale_tick(5, 10)); // 5 <= 10, stale
        assert!(is_stale_tick(10, 10)); // 10 <= 10, stale
        assert!(!is_stale_tick(11, 10)); // 11 > 10, fresh
    }
}

// ============================================================================
// AI TESTS
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
        assert!(!p.is_carrying_max(10)); // 0 >= 10 is false
        p.carrying_amount = 10;
        assert!(p.is_carrying_max(10)); // 10 >= 10 is true
        p.carrying_amount = 11;
        assert!(p.is_carrying_max(10)); // 11 >= 10 is true
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
// FACTION TESTS
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
        // Same faction relationships default to Neutral
        assert_eq!(get_standing(&Faction::Player, &Faction::Player), FactionStanding::Neutral);
        assert_eq!(get_standing(&Faction::Goblin, &Faction::Goblin), FactionStanding::Neutral);
    }
}

// ============================================================================
// NPC BRAIN TESTS
// ============================================================================

mod npc_brain {
    use super::*;

    #[test]
    fn new_brain_defaults() {
        let brain = NpcBrain::new(1, AiType::Deer, 10.0, 20.0);
        assert_eq!(brain.entity_id, 1);
        assert_eq!(brain.ai_type, AiType::Deer);
        assert_eq!(brain.state, BrainState::Idle);
        assert_eq!(brain.target_id, None);
        assert_eq!(brain.timer, 0.0);
        assert_eq!(brain.home_x, 10.0);
        assert_eq!(brain.home_z, 20.0);
        assert_eq!(brain.wander_x, 10.0);
        assert_eq!(brain.wander_z, 20.0);
    }

    #[test]
    fn ai_type_equality() {
        assert_eq!(AiType::Deer, AiType::Deer);
        assert_ne!(AiType::Deer, AiType::Boar);
        assert_ne!(AiType::Goblin, AiType::Friendly);
    }

    #[test]
    fn brain_state_equality() {
        assert_eq!(BrainState::Idle, BrainState::Idle);
        assert_ne!(BrainState::Idle, BrainState::Fleeing);
        assert_ne!(BrainState::Chasing, BrainState::Attacking);
    }
}

// ============================================================================
// PET TESTS
// ============================================================================

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
    fn pet_stance_equality() {
        assert_eq!(PetStance::Stay, PetStance::Stay);
        assert_ne!(PetStance::Stay, PetStance::Follow);
        assert_ne!(PetStance::Aggressive, PetStance::Defensive);
    }
}

// ============================================================================
// STRUCTURE BUILDING TESTS (Extended)
// ============================================================================

mod structure_building {
    use super::*;

    #[test]
    fn place_foundation_grounded() {
        // Foundation with no parent, anchored to terrain
        let s = Structure {
            structure_id: 1,
            parent_id: None,
            piece_type: "Foundation".to_string(),
            stability: 100,
            is_grounded: true,
        };
        assert!(s.is_grounded);
        assert_eq!(s.stability, 100);
    }

    #[test]
    fn attach_wall_to_foundation() {
        let f = Structure::foundation();
        let wall = f.attach_child("Wall", 2).unwrap();
        assert_eq!(wall.piece_type, "Wall");
        assert_eq!(wall.stability, 80);
        assert!(!wall.is_grounded);
        assert_eq!(wall.parent_id, Some(f.structure_id));
    }

    #[test]
    fn attach_floor_to_foundation() {
        let f = Structure::foundation();
        let floor = f.attach_child("Floor", 2).unwrap();
        assert_eq!(floor.stability, 75);
    }

    #[test]
    fn attach_roof_to_foundation() {
        let f = Structure::foundation();
        let roof = f.attach_child("Roof", 2).unwrap();
        assert_eq!(roof.stability, 70);
    }

    #[test]
    fn attach_ramp_to_foundation() {
        let f = Structure::foundation();
        let ramp = f.attach_child("Ramp", 2).unwrap();
        assert_eq!(ramp.stability, 75);
    }

    #[test]
    fn chain_structures() {
        let f = Structure::foundation();
        let wall = f.attach_child("Wall", 2).unwrap();
        let floor = wall.attach_child("Floor", 3).unwrap();
        assert_eq!(floor.stability, 55); // 80 - 25
    }

    #[test]
    fn cannot_attach_to_weak_structure() {
        let mut f = Structure::foundation();
        f.stability = 15;
        assert!(f.attach_child("Wall", 2).is_none());
    }

    #[test]
    fn cannot_attach_unknown_piece() {
        let f = Structure::foundation();
        assert!(f.attach_child("Unknown", 2).is_none());
    }

    #[test]
    fn can_support_check() {
        let f = Structure::foundation();
        assert!(f.can_support(20));
        assert!(f.can_support(99));
        assert!(!f.can_support(100));
        assert!(!f.can_support(101));
    }
}
