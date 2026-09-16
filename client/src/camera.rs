use bevy::prelude::{Transform as BevyTransform, *};
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use tracing::{error, info}; // Architectural Note: Removed unused `warn` to keep build clean.
use spacetimedb_sdk::DbContext; // Architectural Note: Required in v2.x to access subscription_builder.

// Note: Assuming `module_bindings` is exposed at the crate root.
use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::module_bindings::set_camera_mode_reducer::set_camera_mode; // Architectural Note: Explicit v2 trait import.

// ----------------------------------------------------------------------------
// PERSPECTIVE TOGGLING & CULLING
// ----------------------------------------------------------------------------

/// Architectural Note: Handles dynamic spatial partitioning (network culling) per perspective.
/// Dispatches network event to server logging view transition and forcibly rebuilds the SpacetimeDB SQL Subscription.
/// In FPS Mode: Streams high-frequency micro-data in a tight 50m localized radius.
/// In RTS Mode: Expands the streaming radius to 500m to populate the macro view.
pub fn toggle_perspective(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<CameraMode>>,
    mut next_state: ResMut<NextState<CameraMode>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    mut rts_rig_query: Query<&mut BevyTransform, (With<RtsCameraRig>, Without<PlayerBody>)>,
    conn: Res<SpacetimeConnection>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        let Ok(mut window) = window_query.get_single_mut() else { return; };
        let Ok(player_transform) = player_query.get_single() else { return; };
        let Ok(mut rig_transform) = rts_rig_query.get_single_mut() else { return; };

        let px = player_transform.translation.x;
        let pz = player_transform.translation.z;

        match state.get() {
            CameraMode::FPS => {
                next_state.set(CameraMode::RTS);
                rig_transform.translation = player_transform.translation;
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
                
                // Authoritative camera state tracking for server-side metrics
                let _ = conn.db.reducers.set_camera_mode("RTS".to_string());
                
                let radius = 500.0;
                // Architectural Note: subscribe() now returns a SubscriptionHandle directly.
                let _handle = conn.db.subscription_builder().subscribe(vec![
                    "SELECT * FROM player".to_string(),
                    format!("SELECT * FROM transform WHERE x > {} AND x < {} AND z > {} AND z < {}", px - radius, px + radius, pz - radius, pz + radius),
                    "SELECT * FROM resource_stockpile".to_string(),
                    "SELECT * FROM ground_loot".to_string(),
                    "SELECT * FROM resource_node".to_string(),
                    "SELECT * FROM combat_event".to_string() 
                ]);
                info!("Expanded network culling bounds to 500m (RTS Mode)");
            }
            CameraMode::RTS => {
                next_state.set(CameraMode::FPS);
                window.cursor.grab_mode = CursorGrabMode::Locked;
                window.cursor.visible = false;
                
                let _ = conn.db.reducers.set_camera_mode("FPS".to_string());
                
                let radius = 50.0;
                // Architectural Note: subscribe() now returns a SubscriptionHandle directly.
                let _handle = conn.db.subscription_builder().subscribe(vec![
                    "SELECT * FROM player".to_string(),
                    format!("SELECT * FROM transform WHERE x > {} AND x < {} AND z > {} AND z < {}", px - radius, px + radius, pz - radius, pz + radius),
                    "SELECT * FROM resource_stockpile".to_string(),
                    "SELECT * FROM ground_loot".to_string(),
                    "SELECT * FROM resource_node".to_string(),
                    "SELECT * FROM combat_event".to_string() 
                ]);
                info!("Contracted network culling bounds to 50m (FPS Mode)");
            }
        }
    }
}

pub fn enable_fps_perspective(
    mut fps_cam: Query<&mut Camera, (With<FpsCamera>, Without<RtsCameraChild>)>,
    mut rts_cam: Query<&mut Camera, (With<RtsCameraChild>, Without<FpsCamera>)>,
) {
    info!("Transitioning to FPS Camera Mode");
    for mut cam in &mut fps_cam { cam.is_active = true; }
    for mut cam in &mut rts_cam { cam.is_active = false; }
}

pub fn enable_rts_perspective(
    mut fps_cam: Query<&mut Camera, (With<FpsCamera>, Without<RtsCameraChild>)>,
    mut rts_cam: Query<&mut Camera, (With<RtsCameraChild>, Without<FpsCamera>)>,
) {
    info!("Transitioning to RTS Camera Mode");
    for mut cam in &mut fps_cam { cam.is_active = false; }
    for mut cam in &mut rts_cam { cam.is_active = true; }
}

// ----------------------------------------------------------------------------
// CAMERA CONTROLLERS
// ----------------------------------------------------------------------------

/// Handles edge-panning, WASD movement, and scroll zooming for the RTS camera rig.
pub fn rts_camera_controller(
    keys: Res<ButtonInput<KeyCode>>, 
    time: Res<Time>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut scroll_evts: EventReader<MouseWheel>,
    mut rig_query: Query<&mut BevyTransform, With<RtsCameraRig>>,
    mut child_camera_query: Query<&mut BevyTransform, (With<RtsCameraChild>, Without<RtsCameraRig>)>,
) {
    let Ok(mut rig_transform) = rig_query.get_single_mut() else { return; };
    let Ok(mut cam_transform) = child_camera_query.get_single_mut() else { return; };
    let Ok(window) = window_query.get_single() else { return; };

    let pan_speed = 50.0;
    let mut move_dir = Vec3::ZERO;

    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) { move_dir.z -= 1.0; }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) { move_dir.z += 1.0; }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { move_dir.x += 1.0; }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) { move_dir.x -= 1.0; }

    if let Some(cursor_pos) = window.cursor_position() {
        let width = window.width();
        let height = window.height();
        let margin_x = width * 0.05;
        let margin_y = height * 0.05;

        if cursor_pos.x < margin_x { move_dir.x -= 1.0; }
        if cursor_pos.x > width - margin_x { move_dir.x += 1.0; }
        if cursor_pos.y < margin_y { move_dir.z -= 1.0; }
        if cursor_pos.y > height - margin_y { move_dir.z += 1.0; }
    }

    if move_dir != Vec3::ZERO {
        move_dir = move_dir.normalize();
        rig_transform.translation.x += move_dir.x * pan_speed * time.delta_seconds();
        rig_transform.translation.z += move_dir.z * pan_speed * time.delta_seconds();
    }

    // Force the rig base to stay on the ground plane
    rig_transform.translation.y = 0.0; 

    for evt in scroll_evts.read() {
        let zoom_delta = -evt.y * 5.0;
        cam_transform.translation.y = (cam_transform.translation.y + zoom_delta).clamp(10.0, 90.0);
        cam_transform.translation.z = (cam_transform.translation.z + zoom_delta * 0.6).clamp(5.0, 60.0);
    }
}

/// Handles raw mouse delta input to drive FPS look, locking vertical pitch to avoid flipping.
pub fn fps_look(
    mut mouse_motion: EventReader<MouseMotion>,
    mut body_query: Query<&mut BevyTransform, (With<PlayerBody>, Without<PlayerHead>)>,
    mut head_query: Query<&mut BevyTransform, With<PlayerHead>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>, 
    keys: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut window) = window_query.get_single_mut() else { return; };
    let Ok(mut body_transform) = body_query.get_single_mut() else { return; };
    let Ok(mut head_transform) = head_query.get_single_mut() else { return; };

    // Allows users to recapture the mouse if they escaped it during FPS mode.
    if mouse_buttons.just_pressed(MouseButton::Left) {
        window.cursor.grab_mode = CursorGrabMode::Locked;
        window.cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        window.cursor.grab_mode = CursorGrabMode::None;
        window.cursor.visible = true;
    }

    if window.cursor.grab_mode == CursorGrabMode::Locked {
        for event in mouse_motion.read() {
            // Apply yaw directly to the kinematic body to steer the character
            body_transform.rotate_y(-event.delta.x * 0.002);
            
            // Apply pitch independently to the camera head
            let (yaw, mut pitch, roll) = head_transform.rotation.to_euler(EulerRot::YXZ);
            let min_pitch = -89.0_f32.to_radians();
            let max_pitch = 89.0_f32.to_radians();
            
            pitch = (pitch - event.delta.y * 0.002).clamp(min_pitch, max_pitch); 
            head_transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
        }
    }
}