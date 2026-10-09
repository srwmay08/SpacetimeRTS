// ============================================================================
// File: backend/spacetimedb/src/crafting.rs
// ============================================================================
// Authoritative crafting recipes, station prerequisites, and craft_item reducer.

use spacetimedb::{table, reducer, ReducerContext, Table, SpacetimeType};
use crate::movement::player_session;
use crate::movement::transform;
use crate::building::structure;
use crate::player_inventory::{inventory, has_item, remove_item, add_item};

#[derive(SpacetimeType, Clone, Debug, PartialEq, Eq)]
pub struct RecipeIngredient {
    pub item_type: String,
    pub count: u32,
}

#[table(accessor = recipe_definition, public)]
#[derive(Clone)]
pub struct RecipeDefinition {
    #[primary_key]
    pub recipe_id: String,
    pub output_item: String,
    pub output_count: u32,
    pub required_station: String,
    pub requires_roof: bool,
    pub ingredients: Vec<RecipeIngredient>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftingStation {
    None,
    Workbench,
    Forge,
}

impl CraftingStation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Workbench => "Workbench",
            Self::Forge => "Forge",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Workbench" => Self::Workbench,
            "Forge" => Self::Forge,
            _ => Self::None,
        }
    }
}

pub fn seed_authoritative_recipes(ctx: &ReducerContext) {
    let recipes = [
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
            recipe_id: "Pickaxe".to_string(),
            output_item: "Pickaxe".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 2 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Club".to_string(),
            output_item: "Club".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Torch".to_string(),
            output_item: "Torch".to_string(),
            output_count: 1,
            required_station: "None".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Branch".to_string(), count: 1 },
                RecipeIngredient { item_type: "Resin".to_string(), count: 1 },
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
        RecipeDefinition {
            recipe_id: "Wooden Shield".to_string(),
            output_item: "Wooden Shield".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 10 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Flint Spear".to_string(),
            output_item: "Flint Spear".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 5 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Bow".to_string(),
            output_item: "Bow".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 12 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 4 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Crossbow".to_string(),
            output_item: "Crossbow".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 15 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 5 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 4 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Hand Crossbow".to_string(),
            output_item: "Hand Crossbow".to_string(),
            output_count: 1,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 8 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 3 },
                RecipeIngredient { item_type: "Leather Scraps".to_string(), count: 2 },
            ],
        },
        RecipeDefinition {
            recipe_id: "Crossbow Bolt".to_string(),
            output_item: "Crossbow Bolt".to_string(),
            output_count: 15,
            required_station: "Workbench".to_string(),
            requires_roof: false,
            ingredients: vec![
                RecipeIngredient { item_type: "Wood".to_string(), count: 6 },
                RecipeIngredient { item_type: "Flint".to_string(), count: 3 },
            ],
        },
    ];

    for recipe in recipes {
        ctx.db.recipe_definition().insert(recipe);
    }
}

#[reducer]
pub fn craft_item(ctx: &ReducerContext, recipe_id: String) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let recipe = ctx.db.recipe_definition().recipe_id().find(&recipe_id)
        .ok_or_else(|| format!("Unknown crafting recipe: {}", recipe_id))?;

    let mut inv = ctx.db.inventory().entity_id().find(session.entity_id)
        .ok_or_else(|| "Inventory not found.".to_string())?;

    let player_transform = ctx.db.transform().entity_id().find(session.entity_id)
        .ok_or_else(|| "Transform not found.".to_string())?;

    let station = CraftingStation::from_str(&recipe.required_station);
    if station != CraftingStation::None {
        let station_name = station.as_str();
        let mut valid_station = false;
        for s in ctx.db.structure().iter().filter(|s| s.piece_type == station_name && !s.is_blueprint) {
            let dist_sq = (s.x - player_transform.x).powi(2) + (s.z - player_transform.z).powi(2);
            if dist_sq <= 400.0 {
                if !recipe.requires_roof || crate::building::is_covered(ctx, s.x, s.y, s.z) {
                    valid_station = true;
                    break;
                }
            }
        }
        if !valid_station {
            return Err(format!(
                "Crafting '{}' requires an active constructed {} nearby.",
                recipe.output_item, station_name
            ));
        }
    }

    for ing in &recipe.ingredients {
        if !has_item(&inv, &ing.item_type, ing.count) {
            return Err(format!("Missing materials: {}x {}.", ing.count, ing.item_type));
        }
    }

    for ing in &recipe.ingredients {
        remove_item(&mut inv, &ing.item_type, ing.count);
    }

    add_item(&mut inv, &recipe.output_item, recipe.output_count);
    ctx.db.inventory().entity_id().update(inv);

    log::debug!(
        "Player {} authoritatively crafted {}x {}",
        session.entity_id, recipe.output_count, recipe.output_item
    );
    Ok(())
}
