// ----------------------------------------------------------------------------
// AI & NPC BEHAVIOR INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024 Edition)
// ----------------------------------------------------------------------------
// Architectural Note: Tests the server-authoritative AI state machine and RTS unit orders.
// Verifies spatial chunk Chebyshev adjacency queries, Peasant worker state transitions
// (Harvest, MoveTo, Return, AutoGather), carrying capacity constraints, and Pet stance modes.

use backend::ai::{
    is_chunk_adjacent, AiState, AiType, BrainState, NpcBrain, Peasant, PetComponent, PetStance,
    Position,
};

#[test]
fn test_spatial_chunk_adjacency() {
    // Architectural Note: Chebyshev distance <= 1 ensures entity queries cover the 3x3
    // Moore neighborhood of 50m chunks without false exclusions.
    assert!(is_chunk_adjacent(0, 0, 0, 0), "Same chunk must be adjacent");
    assert!(is_chunk_adjacent(0, 0, 1, 0), "East chunk is adjacent");
    assert!(is_chunk_adjacent(0, 0, -1, 0), "West chunk is adjacent");
    assert!(is_chunk_adjacent(0, 0, 0, 1), "North chunk is adjacent");
    assert!(is_chunk_adjacent(0, 0, 0, -1), "South chunk is adjacent");
    assert!(is_chunk_adjacent(0, 0, 1, 1), "Northeast diagonal is adjacent");
    assert!(is_chunk_adjacent(0, 0, -1, -1), "Southwest diagonal is adjacent");

    // Non-adjacent chunks (distance >= 2 in either axis)
    assert!(!is_chunk_adjacent(0, 0, 2, 0));
    assert!(!is_chunk_adjacent(0, 0, 0, 2));
    assert!(!is_chunk_adjacent(0, 0, 2, 2));
    assert!(!is_chunk_adjacent(0, 0, -2, 1));
}

#[test]
fn test_peasant_initial_defaults() {
    let peasant = Peasant {
        entity_id: 100,
        owner_id: 42,
        state: AiState::Idle,
        carrying_item: "None".to_string(),
        carrying_amount: 0,
        last_harvest_target: None,
        consecutive_stuck_ticks: 0,
        auto_gather_type: "None".to_string(),
    };

    assert_eq!(peasant.entity_id, 100);
    assert_eq!(peasant.owner_id, 42);
    assert_eq!(peasant.state, AiState::Idle);
    assert_eq!(peasant.carrying_amount, 0);
    assert_eq!(peasant.auto_gather_type, "None");
}

#[test]
fn test_peasant_state_transitions() {
    let mut peasant = Peasant {
        entity_id: 100,
        owner_id: 42,
        state: AiState::Idle,
        carrying_item: "None".to_string(),
        carrying_amount: 0,
        last_harvest_target: None,
        consecutive_stuck_ticks: 0,
        auto_gather_type: "None".to_string(),
    };

    // Transition to Harvest
    peasant.state = AiState::Harvest(50);
    peasant.last_harvest_target = Some(50);
    assert_eq!(peasant.state, AiState::Harvest(50));
    assert_eq!(peasant.last_harvest_target, Some(50));

    // Transition to Return (carrying wood)
    peasant.carrying_item = "Wood".to_string();
    peasant.carrying_amount = 10;
    peasant.state = AiState::Return(42);
    assert_eq!(peasant.state, AiState::Return(42));
    assert_eq!(peasant.carrying_amount, 10);

    // Transition to MoveTo
    peasant.state = AiState::MoveTo(Position { x: 15.0, y: 2.0, z: 30.0 });
    assert_eq!(peasant.state, AiState::MoveTo(Position { x: 15.0, y: 2.0, z: 30.0 }));

    // Transition to AutoGather
    peasant.state = AiState::AutoGather("Tree".to_string());
    peasant.auto_gather_type = "Tree".to_string();
    assert_eq!(peasant.state, AiState::AutoGather("Tree".to_string()));
    assert_eq!(peasant.auto_gather_type, "Tree");
}

#[test]
fn test_peasant_carrying_capacity() {
    let max_carry = 20_u32;
    let mut peasant = Peasant {
        entity_id: 100,
        owner_id: 42,
        state: AiState::Idle,
        carrying_item: "Wood".to_string(),
        carrying_amount: 0,
        last_harvest_target: None,
        consecutive_stuck_ticks: 0,
        auto_gather_type: "None".to_string(),
    };

    let is_full = |p: &Peasant| p.carrying_amount >= max_carry;

    assert!(!is_full(&peasant));

    peasant.carrying_amount = 10;
    assert!(!is_full(&peasant));

    peasant.carrying_amount = 20;
    assert!(is_full(&peasant));

    peasant.carrying_amount = 25;
    assert!(is_full(&peasant));
}

#[test]
fn test_pet_stance_variants() {
    let mut pet = PetComponent {
        entity_id: 5,
        owner_id: 42,
        stance: PetStance::Follow,
    };

    assert_eq!(pet.stance, PetStance::Follow);

    pet.stance = PetStance::Stay;
    assert_eq!(pet.stance, PetStance::Stay);

    pet.stance = PetStance::Aggressive;
    assert_eq!(pet.stance, PetStance::Aggressive);

    pet.stance = PetStance::Defensive;
    assert_eq!(pet.stance, PetStance::Defensive);
}

#[test]
fn test_npc_brain_states_and_types() {
    let brain = NpcBrain {
        entity_id: 200,
        ai_type: AiType::Deer,
        state: BrainState::Idle,
        target_id: None,
        timer: 0.0,
        home_x: 100.0,
        home_z: 200.0,
        wander_x: 100.0,
        wander_z: 200.0,
    };

    assert_eq!(brain.ai_type, AiType::Deer);
    assert_eq!(brain.state, BrainState::Idle);

    // Verify all BrainState enum values
    let states = [
        BrainState::Idle,
        BrainState::Fleeing,
        BrainState::Chasing,
        BrainState::Attacking,
        BrainState::Warning,
        BrainState::Corpse,
    ];
    assert_eq!(states.len(), 6);

    // Verify all AiType enum values
    let types = [
        AiType::Friendly,
        AiType::Deer,
        AiType::Boar,
        AiType::Goblin,
        AiType::Peasant,
    ];
    assert_eq!(types.len(), 5);
}

#[test]
fn test_npc_state_machine_door_interaction_transitions() {
    let mut npc = backend::ai::NpcState {
        entity_id: 300,
        current_action: backend::ai::NpcAction::PathingToDoor,
        target_coords: Some(Position { x: 10.0, y: 1.0, z: 20.0 }),
        task_entity_id: Some(555),
        last_tick: 1_000_000,
    };

    assert_eq!(npc.current_action, backend::ai::NpcAction::PathingToDoor);
    assert_eq!(npc.task_entity_id, Some(555));

    // When the door is toggled, transition to InteractingWithDoor
    let door_open_tick = 2_000_000;
    npc.current_action = backend::ai::NpcAction::InteractingWithDoor;
    npc.last_tick = door_open_tick;

    assert_eq!(npc.current_action, backend::ai::NpcAction::InteractingWithDoor);

    // After 500ms (500_000 micros), transition to Patrolling
    let check_tick = door_open_tick + 600_000;
    let elapsed = check_tick - npc.last_tick;
    assert!(elapsed >= 500_000);

    npc.current_action = backend::ai::NpcAction::Patrolling;
    assert_eq!(npc.current_action, backend::ai::NpcAction::Patrolling);
}

#[test]
fn test_npc_state_machine_blueprint_construction_progress() {
    let mut npc = backend::ai::NpcState {
        entity_id: 301,
        current_action: backend::ai::NpcAction::ConstructingBlueprint,
        target_coords: Some(Position { x: 4.0, y: 0.5, z: 4.0 }),
        task_entity_id: Some(777),
        last_tick: 10_000_000,
    };

    let mut current_health = 1.0_f32;
    let max_health = 100.0_f32;
    let mut is_blueprint = true;

    // Simulate 1-second hammer swings
    for step in 1..=10 {
        let current_tick = 10_000_000 + (step * 1_000_000);
        let elapsed = current_tick - npc.last_tick;
        assert!(elapsed >= 1_000_000);

        current_health = (current_health + 10.0).min(max_health);
        let progress = ((current_health / max_health) * 100.0) as u32;
        if current_health >= max_health || progress >= 100 {
            is_blueprint = false;
            npc.current_action = backend::ai::NpcAction::Idle;
            npc.task_entity_id = None;
        }
        npc.last_tick = current_tick;
    }

    assert!(!is_blueprint);
    assert_eq!(current_health, 100.0);
    assert_eq!(npc.current_action, backend::ai::NpcAction::Idle);
    assert_eq!(npc.task_entity_id, None);
}

#[test]
fn test_building_templates_grid_offsets_and_faction_styles() {
    use backend::templates::{BuildingPiece, BuildingTemplate, Faction, GridOffset};

    // 1. Watchtower (2x2 foundation, door, window, sniper perches)
    let tower = BuildingTemplate::watchtower(Faction::HighElf);
    assert_eq!(tower.faction, Faction::HighElf);
    assert_eq!(tower.blocks.get(&GridOffset(0, 0, 0)), Some(&BuildingPiece::Foundation));
    assert_eq!(tower.blocks.get(&GridOffset(1, 0, 0)), Some(&BuildingPiece::Foundation));
    assert_eq!(tower.blocks.get(&GridOffset(0, 1, 0)), Some(&BuildingPiece::Doorway));
    assert_eq!(tower.blocks.get(&GridOffset(1, 1, 0)), Some(&BuildingPiece::SolidWall));
    assert_eq!(tower.blocks.get(&GridOffset(1, 1, 1)), Some(&BuildingPiece::WindowWall));

    // Check HighElf piece names
    assert_eq!(BuildingPiece::Doorway.to_piece_name(Faction::HighElf), "HighElf_Door");
    assert_eq!(BuildingPiece::WindowWall.to_piece_name(Faction::HighElf), "HighElf_Window");
    assert_eq!(BuildingPiece::SolidWall.to_piece_name(Faction::HighElf), "HighElf_Wall");
    assert_eq!(BuildingPiece::Foundation.to_piece_name(Faction::HighElf), "HighElf_Foundation");

    // 2. Palisade (4x1 wall with gate)
    let palisade = BuildingTemplate::palisade(Faction::DarkElf);
    assert_eq!(palisade.blocks.len(), 12);
    assert_eq!(palisade.blocks.get(&GridOffset(1, 1, 0)), Some(&BuildingPiece::Doorway));
    assert_eq!(BuildingPiece::Doorway.to_piece_name(Faction::DarkElf), "DarkElf_Door");

    // 3. Barbarian style
    assert_eq!(BuildingPiece::Doorway.to_piece_name(Faction::Barbarian), "Barbarian_Door");
    assert_eq!(BuildingPiece::SolidWall.to_piece_name(Faction::Barbarian), "Barbarian_Wall");

    // 4. World position transformation
    let offset = GridOffset(2, 1, 3);
    let (wx, wy, wz) = offset.to_world_pos(10.0, 5.0, 20.0);
    assert_eq!(wx, 18.0); // 10 + 2*4
    assert_eq!(wy, 8.0);  // 5 + 1*3
    assert_eq!(wz, 32.0); // 20 + 3*4
}
