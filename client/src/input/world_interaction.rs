// ============================================================================
// File: client/src/input/world_interaction.rs
// ============================================================================
// ----------------------------------------------------------------------------
// WORLD INTERACTION PROMPT & IDENTIFIER RESOLUTION HELPERS
// ----------------------------------------------------------------------------

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::components::*;
use crate::core::GameLayer;
use crate::network::SpacetimeConnection;
use crate::module_bindings::resource_node_table::ResourceNodeTableAccess;
use crate::module_bindings::structure_table::StructureTableAccess;
use crate::module_bindings::door_state_table::DoorStateTableAccess;

pub(crate) fn resolve_node_id(entity: Entity, node_q: &Query<&ResourceNodeItem>, parent_q: &Query<&Parent>) -> Option<u64> {
    if let Ok(node) = node_q.get(entity) {
        return Some(node.node_id);
    }
    if let Ok(parent) = parent_q.get(entity) {
        if let Ok(node) = node_q.get(parent.get()) {
            return Some(node.node_id);
        }
    }
    None
}

pub(crate) fn resolve_structure_id(
    entity: Entity,
    structure_q: &Query<&NetworkStructure>,
    door_q: &Query<&Door>,
    parent_q: &Query<&Parent>,
) -> Option<u64> {
    if let Ok(door) = door_q.get(entity) {
        return Some(door.structure_id);
    }
    if let Ok(st) = structure_q.get(entity) {
        return Some(st.structure_id);
    }
    if let Ok(parent) = parent_q.get(entity) {
        if let Ok(door) = door_q.get(parent.get()) {
            return Some(door.structure_id);
        }
        if let Ok(st) = structure_q.get(parent.get()) {
            return Some(st.structure_id);
        }
    }
    None
}

pub fn update_interaction_prompt(
    conn: Res<SpacetimeConnection>,
    active_item: Res<ActiveEquippedItem>,
    camera_query: Query<&GlobalTransform, With<FpsCamera>>,
    player_query: Query<Entity, With<PlayerBody>>,
    spatial_query: SpatialQuery,
    node_query: Query<&ResourceNodeItem>,
    parent_query: Query<&Parent>,
    structure_query: Query<&NetworkStructure>,
    door_query: Query<&Door>,
    ruin_query: Query<&HarvestableRuin>,
    mut prompt_query: Query<(&mut Text, &mut Visibility), With<InteractionPromptText>>,
) {
    let Ok(cam_transform) = camera_query.get_single() else { return; };
    let Ok(player_entity) = player_query.get_single() else { return; };
    let Ok((mut text, mut vis)) = prompt_query.get_single_mut() else { return; };

    let origin = cam_transform.translation();
    let dir = cam_transform.forward();

    let hit = spatial_query.cast_ray(
        origin, dir.into(), 7.0, true,
        SpatialQueryFilter::from_mask([GameLayer::Environment, GameLayer::Default])
            .with_excluded_entities([player_entity]),
    );

    if let Some(hit_data) = hit {
        if let Some(node_id) = resolve_node_id(hit_data.entity, &node_query, &parent_query) {
            if let Some(node) = conn.db.db.resource_node().node_id().find(&node_id) {
                let prompt = match node.node_type.as_str() {
                    "Bush" => {
                        if node.health > 0 { "[E] Pick Berries" } else { "Berries Depleted" }
                    }
                    "Branch" => "[E] Pick up Branch",
                    "Flint" => "[E] Pick up Flint",
                    "LooseStone" => "[E] Pick up Stone",
                    "Tree" => "Tree (Left-click with Stone Axe)",
                    "FallenLog" => "[E] Chop Fallen Log",
                    "Rubble" => "[E] Mine Rubble",
                    "Rock" => "Rock (Left-click with Pickaxe)",
                    _ => "[E] Gather",
                };

                text.sections[0].value = prompt.to_string();
                *vis = Visibility::Inherited;
                return;
            }
        } else if let Ok(ruin) = ruin_query.get(hit_data.entity) {
            text.sections[0].value = format!("[E] Mine {} (Yield: {})", ruin.node_type, ruin.yield_amount);
            *vis = Visibility::Inherited;
            return;
        } else if let Some(struct_id) = resolve_structure_id(hit_data.entity, &structure_query, &door_query, &parent_query) {
            if let Some(s) = conn.db.db.structure().structure_id().find(&struct_id) {
                let is_hammer = active_item.0.as_deref() == Some("Hammer");
                if s.piece_type == "Door" && !s.is_blueprint {
                    let is_open = conn.db.db.door_state().structure_id().find(&struct_id).map_or(false, |d| d.is_open);
                    text.sections[0].value = if is_open {
                        "[E] Close Door".to_string()
                    } else {
                        "[E] Open Door".to_string()
                    };
                } else if s.piece_type == "Workbench" && !s.is_blueprint {
                    text.sections[0].value = "[E] Open Workbench".to_string();
                } else if s.is_blueprint {
                    text.sections[0].value = format!("Autobuilding {} ({}%)...", s.piece_type, s.construction_progress);
                } else if s.current_health < s.max_health {
                    text.sections[0].value = if is_hammer {
                        format!("[E] Repair Structure ({:.0}/{:.0} HP)", s.current_health, s.max_health)
                    } else {
                        format!("Damaged ({:.0}/{:.0} HP) - Equip Hammer", s.current_health, s.max_health)
                    };
                } else {
                    text.sections[0].value = format!("{} ({:.0}/{:.0} HP)", s.piece_type, s.current_health, s.max_health);
                }
                *vis = Visibility::Inherited;
                return;
            }
        }
    }

    text.sections[0].value = "".to_string();
    *vis = Visibility::Hidden;
}
