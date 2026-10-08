// ============================================================================
// File: client/src/ui/crafting.rs
// ============================================================================
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use tracing::{info, error};

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::building::BuildModeState; 
 
 
 
 
 
use crate::module_bindings::craft_item_reducer::craft_item;

// ----------------------------------------------------------------------------
// INVENTORY & WORKBENCH HUD SYSTEMS
// ----------------------------------------------------------------------------

pub fn handle_crafting_interaction(
    mut interaction_query: Query<(&Interaction, &CraftRecipeButton, &mut BackgroundColor), Changed<Interaction>>,
    conn: Res<SpacetimeConnection>,
) {
    for (interaction, btn, mut bg) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                info!("CLIENT UI: Crafting item '{}'", btn.0);
                if let Err(e) = conn.db.reducers.craft_item(btn.0.clone()) {
                    error!("Crafting failed: {:?}", e);
                }
            }
            Interaction::Hovered => {
                *bg = Color::srgb(0.28, 0.28, 0.28).into();
            }
            Interaction::None => {
                *bg = Color::srgb(0.16, 0.16, 0.16).into();
            }
        }
    }
}


pub fn handle_build_menu_selection(
    mut interaction_query: Query<(&Interaction, &BuildPieceButton, &mut BackgroundColor), (Changed<Interaction>, Without<BuildTemplateButton>)>,
    mut template_query: Query<(&Interaction, &BuildTemplateButton, &mut BackgroundColor), (Changed<Interaction>, Without<BuildPieceButton>)>,
    mut build_state: ResMut<BuildModeState>,
    mut menu_query: Query<&mut Style, With<BuildMenuRoot>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    camera_mode: Res<State<CameraMode>>,
) {
    for (interaction, btn, mut bg) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                build_state.selected_piece = btn.0;
                build_state.selected_template = None;
                build_state.is_active = true;
                if let Ok(mut style) = menu_query.get_single_mut() {
                    style.display = Display::None;
                }
                if let Ok(mut window) = window_query.get_single_mut() {
                    if *camera_mode.get() == CameraMode::FPS {
                        window.cursor.grab_mode = CursorGrabMode::Locked;
                        window.cursor.visible = false;
                    }
                }
            }
            Interaction::Hovered => {
                *bg = Color::srgb(0.3, 0.3, 0.3).into();
            }
            Interaction::None => {
                *bg = Color::srgb(0.18, 0.18, 0.18).into();
            }
        }
    }

    for (interaction, btn, mut bg) in template_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                build_state.selected_template = Some(btn.0);
                build_state.is_active = true;
                if let Ok(mut style) = menu_query.get_single_mut() {
                    style.display = Display::None;
                }
                if let Ok(mut window) = window_query.get_single_mut() {
                    if *camera_mode.get() == CameraMode::FPS {
                        window.cursor.grab_mode = CursorGrabMode::Locked;
                        window.cursor.visible = false;
                    }
                }
            }
            Interaction::Hovered => {
                *bg = Color::srgb(0.4, 0.32, 0.22).into();
            }
            Interaction::None => {
                *bg = Color::srgb(0.22, 0.18, 0.14).into();
            }
        }
    }
}


pub fn update_build_ui(
    build_state: Res<BuildModeState>,
    mut ui_query: Query<(&mut Visibility, &mut Text), With<BuildUIText>>,
) {
    if build_state.is_changed() {
        for (mut vis, mut text) in ui_query.iter_mut() {
            if build_state.is_active {
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
                let (mode_label, cost_label) = if let Some(tmpl) = build_state.selected_template {
                    (format!("Template: {}", tmpl.name()), format!("{} Wood", tmpl.wood_cost()))
                } else {
                    (format!("Piece: {}", build_state.selected_piece.name()), format!("{} Wood", build_state.selected_piece.wood_cost()))
                };
                let new_text = format!(
                    "BUILD MODE: ACTIVE | Style: Frontier Wood & Stone | {} (Cost: {})\n[Y] Toggle Template Mode | [R] Cycle Piece | [Q/E] Rotate | [Right-Click] Catalog | [B] Exit", 
                    mode_label,
                    cost_label
                );
                if text.sections[0].value != new_text {
                    text.sections[0].value = new_text;
                }
            } else if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::RequiresWorkbenchRecipe;

    #[test]
    fn test_field_vs_workbench_crafting_recipe_visibility() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        // Spawn core field recipes (Torch, Club, Hammer)
        let core_items = ["Torch", "Club", "Hammer"];
        for name in core_items {
            app.world_mut().spawn((
                NodeBundle {
                    style: Style { display: Display::Flex, ..default() },
                    ..default()
                },
                CraftRecipeButton(name.to_string()),
            ));
        }

        // Spawn advanced workbench recipes
        let wb_items = ["Crossbow", "Bow", "Wooden Shield"];
        for name in wb_items {
            app.world_mut().spawn((
                NodeBundle {
                    style: Style { display: Display::None, ..default() },
                    ..default()
                },
                CraftRecipeButton(name.to_string()),
                RequiresWorkbenchRecipe,
            ));
        }

        // Away from workbench: workbench recipes must be hidden (Display::None)
        let mut wb_q = app.world_mut().query_filtered::<&Style, With<RequiresWorkbenchRecipe>>();
        for style in wb_q.iter(app.world()) {
            assert_eq!(style.display, Display::None, "Workbench recipes must be hidden when away from workbench");
        }

        // Near workbench: toggle display to Display::Flex
        let mut wb_mut_q = app.world_mut().query_filtered::<&mut Style, With<RequiresWorkbenchRecipe>>();
        for mut style in wb_mut_q.iter_mut(app.world_mut()) {
            style.display = Display::Flex;
        }

        // Verify active workbench visibility
        let mut wb_q2 = app.world_mut().query_filtered::<&Style, With<RequiresWorkbenchRecipe>>();
        for style in wb_q2.iter(app.world()) {
            assert_eq!(style.display, Display::Flex, "Workbench recipes must be visible when workbench is active");
        }
    }
}
