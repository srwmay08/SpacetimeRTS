use spacetimedb::{table, reducer, ReducerContext, SpacetimeType, Table};
use crate::movement::player_session;
use crate::resource_stockpile; // Architectural Note: Explicit v2.x table accessor trait import.

/// Defines a localized snap point on a modular piece for server-side validation.
#[derive(SpacetimeType, Clone, Debug)]
pub struct SocketDef {
    pub name: String,
    pub offset_x: f32,
    pub offset_y: f32,
    pub offset_z: f32,
}

/// Authoritative database table storing all placed player structures.
#[table(accessor = structure, public)]
#[derive(Clone)]
pub struct Structure {
    #[primary_key] #[auto_inc]
    pub structure_id: u64,
    pub piece_type: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub rot_w: f32,
    pub owner_id: u64,
}

/// Authoritative reducer to validate material costs, verify placement, and persist structures.
#[reducer]
pub fn place_structure(
    ctx: &ReducerContext,
    piece_type: String,
    x: f32, y: f32, z: f32,
    rot_x: f32, rot_y: f32, rot_z: f32, rot_w: f32,
) -> Result<(), String> {
    // Authenticate builder session
    let session = ctx.db.player_session().identity().find(ctx.sender())
        .ok_or("Unauthorized: No active session")?;

    // Verify and deduct resource costs from stockpile
    let mut stockpile = ctx.db.resource_stockpile().entity_id().find(session.entity_id)
        .ok_or("Stockpile not found")?;

    let wood_cost = match piece_type.as_str() {
        "Foundation" => 20,
        "Wall" => 10,
        "Floor" => 15,
        "Roof" => 15,
        "Ramp" => 20,
        _ => return Err(format!("Unknown piece type: {}", piece_type)),
    };

    if stockpile.wood < wood_cost {
        return Err("Not enough wood in stockpile to build structure".to_string());
    }

    stockpile.wood -= wood_cost;
    ctx.db.resource_stockpile().entity_id().update(stockpile);

    // Insert authoritative structure record
    ctx.db.structure().insert(Structure {
        structure_id: 0,
        piece_type: piece_type.clone(),
        x, y, z,
        rot_x, rot_y, rot_z, rot_w,
        owner_id: session.entity_id,
    });

    log::info!("Player {} successfully placed structure type: {}", session.entity_id, piece_type);
    Ok(())
}