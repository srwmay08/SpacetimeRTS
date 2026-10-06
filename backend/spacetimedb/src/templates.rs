// ----------------------------------------------------------------------------
// BUILDING TEMPLATES & FACTION BLUEPRINT STYLES (SpacetimeDB Reducers)
// ----------------------------------------------------------------------------
// Architectural Note: Re-exports canonical template definitions, factions, and grid
// offsets from spacetime-rts-logic. Defines authoritative SpacetimeDB reducers
// for stamping templates into the world as blueprints or instant structures.

use spacetimedb::{reducer, ReducerContext};
pub use spacetime_rts_logic::templates::*;

use crate::movement::player_session;

// ----------------------------------------------------------------------------
// REDUCERS FOR BLUEPRINT TEMPLATE SPAWNING
// ----------------------------------------------------------------------------

#[reducer]
pub fn spawn_template_blueprint(
    ctx: &ReducerContext,
    template_name: String,
    faction_name: String,
    origin_x: f32,
    origin_z: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let faction = Faction::from_str_name(&faction_name);
    let template_type = BuildingTemplateType::from_api_name(&template_name)
        .ok_or_else(|| format!("Unknown building template: {}", template_name))?;
    let template = template_type.to_template(faction);

    let ids = crate::npc_building::spawn_template_structures(
        ctx,
        &template,
        origin_x,
        origin_z,
        session.entity_id,
        true, // spawn as blueprints so peasants can construct them!
    )?;

    log::debug!(
        "Spawned {} blueprint pieces for template '{}' ({:?}) at ({:.1}, {:.1})",
        ids.len(), template_name, faction, origin_x, origin_z
    );
    Ok(())
}

#[reducer]
pub fn admin_spawn_template_instant(
    ctx: &ReducerContext,
    template_name: String,
    faction_name: String,
    origin_x: f32,
    origin_z: f32,
) -> Result<(), String> {
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or_else(|| "Unauthorized: No active session.".to_string())?;

    let faction = Faction::from_str_name(&faction_name);
    let template_type = BuildingTemplateType::from_api_name(&template_name)
        .ok_or_else(|| format!("Unknown building template: {}", template_name))?;
    let template = template_type.to_template(faction);

    let ids = crate::npc_building::spawn_template_structures(
        ctx,
        &template,
        origin_x,
        origin_z,
        session.entity_id,
        false, // completed instantly
    )?;

    log::debug!(
        "Admin spawned {} completed pieces for template '{}' ({:?}) at ({:.1}, {:.1})",
        ids.len(), template_name, faction, origin_x, origin_z
    );
    Ok(())
}
