// ----------------------------------------------------------------------------
// CRAFTING & RECIPE SYSTEM INTEGRATION TESTS (SpacetimeDB v2.x / Rust 2024)
// ----------------------------------------------------------------------------
// Architectural Note: Tests the server-authoritative crafting pipeline.
// Verifies recipe ingredient verification, atomic resource consumption,
// multi-item batch outputs (e.g. 20x arrows), hand-crafting vs Workbench
// station proximity requirements, and item discoverability tracking.

use backend::{add_item, has_item, remove_item, Inventory, RecipeDefinition, RecipeIngredient};

/// Helper to simulate crafting a recipe against a mutable inventory
fn execute_craft(
    inv: &mut Inventory,
    recipe: &RecipeDefinition,
    has_workbench_nearby: bool,
) -> Result<(), &'static str> {
    // 1. Station proximity check
    if recipe.required_station != "None" && !has_workbench_nearby {
        return Err("Required crafting station not nearby");
    }

    // 2. Material verification
    for ing in &recipe.ingredients {
        if !has_item(inv, &ing.item_type, ing.count) {
            return Err("Missing required crafting materials");
        }
    }

    // 3. Material consumption
    for ing in &recipe.ingredients {
        remove_item(inv, &ing.item_type, ing.count);
    }

    // 4. Output delivery
    add_item(inv, &recipe.output_item, recipe.output_count);
    Ok(())
}

fn sample_recipes() -> Vec<RecipeDefinition> {
    vec![
        RecipeDefinition {
            recipe_id: "Hammer".to_string(),
            output_item: "Hammer".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 1 },
                RecipeIngredient { item_type: "LooseStone".to_string(), count: 1 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Stone Axe".to_string(),
            output_item: "Stone Axe".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 1 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 1 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Crude Bow".to_string(),
            output_item: "Crude Bow".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 10 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 4 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Flint Arrow".to_string(),
            output_item: "Flint Arrow".to_string(),
            output_count: 20,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 8 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Wood Arrow".to_string(),
            output_item: "Wood Arrow".to_string(),
            output_count: 20,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 8 },
            ],
        },
    ]
}

#[test]
fn test_craft_hammer_hand_recipe() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: Vec::new(),
        discovered_items: Vec::new(),
    };
    add_item(&mut inv, "Branch", 1);
    add_item(&mut inv, "LooseStone", 1);

    let recipes = sample_recipes();
    let hammer_recipe = recipes.iter().find(|r| r.recipe_id == "Hammer").unwrap();

    let result = execute_craft(&mut inv, hammer_recipe, false);
    assert!(result.is_ok());

    assert!(has_item(&inv, "Hammer", 1));
    assert!(!has_item(&inv, "Branch", 1));
    assert!(!has_item(&inv, "LooseStone", 1));
    assert!(inv.discovered_items.contains(&"Hammer".to_string()));
}

#[test]
fn test_craft_insufficient_materials_rejected() {
    let mut inv = Inventory {
        entity_id: 1,
        slots: Vec::new(),
        discovered_items: Vec::new(),
    };
    add_item(&mut inv, "Branch", 1);
    // Missing Flint

    let recipes = sample_recipes();
    let axe_recipe = recipes.iter().find(|r| r.recipe_id == "Stone Axe").unwrap();

    let result = execute_craft(&mut inv, axe_recipe, false);
    assert_eq!(result, Err("Missing required crafting materials"));

    // Inventory must remain intact
    assert!(has_item(&inv, "Branch", 1));
    assert!(!has_item(&inv, "Stone Axe", 1));
}

#[test]
fn test_craft_workbench_station_requirement() {
    // Architectural Note: Recipes specifying `required_station = "Workbench"`
    // must fail if player is not within 20m of an active workbench.
    let mut inv = Inventory {
        entity_id: 1,
        slots: Vec::new(),
        discovered_items: Vec::new(),
    };
    add_item(&mut inv, "Wood", 10);
    add_item(&mut inv, "Leather Scraps", 4);

    let recipes = sample_recipes();
    let bow_recipe = recipes.iter().find(|r| r.recipe_id == "Crude Bow").unwrap();

    // Crafting without nearby workbench fails
    let failed = execute_craft(&mut inv, bow_recipe, false);
    assert_eq!(failed, Err("Required crafting station not nearby"));
    assert!(!has_item(&inv, "Crude Bow", 1));

    // Crafting with workbench succeeds
    let succeeded = execute_craft(&mut inv, bow_recipe, true);
    assert!(succeeded.is_ok());
    assert!(has_item(&inv, "Crude Bow", 1));
    assert_eq!(has_item(&inv, "Wood", 1), false);
}

#[test]
fn test_craft_batch_item_output_yield() {
    // Architectural Note: Arrow crafting yields batches of 20 units per craft.
    let mut inv = Inventory {
        entity_id: 1,
        slots: Vec::new(),
        discovered_items: Vec::new(),
    };
    add_item(&mut inv, "Wood", 8);

    let recipes = sample_recipes();
    let arrow_recipe = recipes.iter().find(|r| r.recipe_id == "Wood Arrow").unwrap();

    let result = execute_craft(&mut inv, arrow_recipe, false);
    assert!(result.is_ok());

    assert!(has_item(&inv, "Wood Arrow", 20));
    assert!(!has_item(&inv, "Wood", 1));
}
