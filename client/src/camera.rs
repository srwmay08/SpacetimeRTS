use bevy::prelude::{Transform as BevyTransform, *};
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use tracing::info; 

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::module_bindings::set_camera_mode_reducer::set_camera_mode; 
use crate::module_bindings::set_interior_culling_reducer::set_interior_culling;
use crate::module_bindings::CameraModeType;

// ----------------------------------------------------------------------------
// CAMERA BLENDING RESOURCE
// ----------------------------------------------------------------------------
#[derive(Resource)]
pub struct CameraTransitionState {
    pub is_transitioning: bool,
    pub timer: Timer,
    pub start_pos: Vec3,
    pub target_pos: Vec3,
}

impl Default for CameraTransitionState {
    fn default() -> Self {
        Self {
            is_transitioning: false,
            timer: Timer::from_seconds(0.35, TimerMode::Once),
            start_pos: Vec3::ZERO,
            target_pos: Vec3::ZERO,
        }
    }
}

// ----------------------------------------------------------------------------
// PERSPECTIVE TOGGLING & CULLING
// ----------------------------------------------------------------------------

pub fn toggle_perspective(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<CameraMode>>,
    mut next_state: ResMut<NextState<CameraMode>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    mut rts_rig_query: Query<&mut BevyTransform, (With<RtsCameraRig>, Without<PlayerBody>)>,
    conn: Res<SpacetimeConnection>,
    mut culling_state: ResMut<NetworkCullingState>, 
    mut transition: ResMut<CameraTransitionState>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        let Ok(mut window) = window_query.get_single_mut() else { return; };
        let Ok(player_transform) = player_query.get_single() else { return; };
        let Ok(mut rig_transform) = rts_rig_query.get_single_mut() else { return; };

        match state.get() {
            CameraMode::FPS => {
                next_state.set(CameraMode::RTS);
                transition.is_transitioning = true;
                transition.timer.reset();
                transition.start_pos = player_transform.translation;
                transition.target_pos = player_transform.translation + Vec3::new(0.0, 40.0, 25.0);

                rig_transform.translation = player_transform.translation;
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
                
                let _ = conn.db.reducers.set_camera_mode(CameraModeType::Rts);
                
                culling_state.radius = 10;
                culling_state.needs_rebuild = true;
                info!("Camera Mode: RTS. Expanding network culling bounds to 10 chunks.");
            }
            CameraMode::RTS => {
                next_state.set(CameraMode::FPS);
                window.cursor.grab_mode = CursorGrabMode::Locked;
                window.cursor.visible = false;
                
                let _ = conn.db.reducers.set_camera_mode(CameraModeType::Fps);
                
                culling_state.radius = 1;
                culling_state.needs_rebuild = true;
                info!("Camera Mode: FPS. Contracting network culling bounds to 1 chunk.");
            }
        }
    }
}

pub fn update_camera_transition(
    time: Res<Time>,
    mut transition: ResMut<CameraTransitionState>,
    mut rts_cam: Query<&mut BevyTransform, With<RtsCameraChild>>,
) {
    if transition.is_transitioning {
        transition.timer.tick(time.delta());
        let t = transition.timer.fraction();
        let ease = 1.0 - (1.0 - t).powi(3);

        if let Ok(mut cam_t) = rts_cam.get_single_mut() {
            cam_t.translation = transition.start_pos.lerp(transition.target_pos, ease);
        }

        if transition.timer.just_finished() {
            transition.is_transitioning = false;
        }
    }
}

// Architectural Note: Clean Camera Mode Transition Hooks.
// Hides 2D floating health bar overlays instantly when entering FPS perspective.
pub fn enable_fps_perspective(
    mut fps_cam: Query<&mut Camera, (With<FpsCamera>, Without<RtsCameraChild>)>,
    mut rts_cam: Query<&mut Camera, (With<RtsCameraChild>, Without<FpsCamera>)>,
    mut health_bars: Query<&mut Visibility, With<HealthBarUI>>,
    mut marquee: Query<&mut Visibility, (With<MarqueeUI>, Without<HealthBarUI>)>,
) {
    info!("Transitioning to FPS Camera Mode");
    for mut cam in &mut fps_cam { cam.is_active = true; }
    for mut cam in &mut rts_cam { cam.is_active = false; }
    for mut vis in &mut health_bars { *vis = Visibility::Hidden; }
    for mut vis in &mut marquee { *vis = Visibility::Hidden; }
}

pub fn enable_rts_perspective(
    mut fps_cam: Query<&mut Camera, (With<FpsCamera>, Without<RtsCameraChild>)>,
    mut rts_cam: Query<&mut Camera, (With<RtsCameraChild>, Without<FpsCamera>)>,
) {
    info!("Transitioning to RTS Camera Mode");
    for mut cam in &mut fps_cam { cam.is_active = false; }
    for mut cam in &mut rts_cam { cam.is_active = true; }
}

pub fn interior_occlusion_culling_system(
    camera_query: Query<&GlobalTransform, With<FpsCamera>>,
    volume_query: Query<&BaseInteriorVolume>,
    mut interior_props: Query<&mut Visibility, With<InteriorProp>>,
    mut culling_state: ResMut<NetworkCullingState>,
    conn: Res<SpacetimeConnection>,
    camera_mode: Res<State<CameraMode>>,
) {
    if *camera_mode.get() != CameraMode::FPS { return; }

    let Ok(cam_transform) = camera_query.get_single() else { return; };
    let pos = cam_transform.translation();

    let mut is_inside_any = false;
    for volume in volume_query.iter() {
        if pos.x >= volume.min.x && pos.x <= volume.max.x &&
           pos.y >= volume.min.y && pos.y <= volume.max.y &&
           pos.z >= volume.min.z && pos.z <= volume.max.z {
            is_inside_any = true;
            break;
        }
    }

    if culling_state.in_interior != is_inside_any {
        culling_state.in_interior = is_inside_any;
        culling_state.needs_rebuild = true; 

        let _ = conn.db.reducers.set_interior_culling(is_inside_any);

        for mut vis in interior_props.iter_mut() {
            *vis = if is_inside_any { Visibility::Inherited } else { Visibility::Hidden };
        }

        if is_inside_any {
            info!("Entered interior AABB volume. Rendering interiors, halting external network macro-data.");
        } else {
            info!("Exited interior AABB volume. Culling interiors, flushing network macro-data.");
        }
    }
}

// ----------------------------------------------------------------------------
// CAMERA CONTROLLERS
// ----------------------------------------------------------------------------

pub fn rts_camera_controller(
    keys: Res<ButtonInput<KeyCode>>, 
    time: Res<Time>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut scroll_evts: EventReader<MouseWheel>,
    mut rig_query: Query<&mut BevyTransform, With<RtsCameraRig>>,
    mut child_camera_query: Query<&mut BevyTransform, (With<RtsCameraChild>, Without<RtsCameraRig>)>,
    transition: Res<CameraTransitionState>,
) {
    if transition.is_transitioning { return; }

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
        
        let margin_x = 5.0;
        let margin_y = 5.0;

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

    rig_transform.translation.y = 0.0; 

    for evt in scroll_evts.read() {
        let zoom_delta = -evt.y * 5.0;
        cam_transform.translation.y = (cam_transform.translation.y + zoom_delta).clamp(10.0, 90.0);
        cam_transform.translation.z = (cam_transform.translation.z + zoom_delta * 0.6).clamp(5.0, 60.0);
    }
}

pub fn fps_look(
    mut mouse_motion: EventReader<MouseMotion>,
    mut body_query: Query<&mut BevyTransform, (With<PlayerBody>, Without<PlayerHead>)>,
    mut head_query: Query<&mut BevyTransform, With<PlayerHead>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>, 
    keys: Res<ButtonInput<KeyCode>>,
    inv_query: Query<&Style, With<InventoryUiRoot>>,
    build_menu_query: Query<&Style, With<BuildMenuRoot>>,
) {
    let Ok(mut window) = window_query.get_single_mut() else { return; };
    let Ok(mut body_transform) = body_query.get_single_mut() else { return; };
    let Ok(mut head_transform) = head_query.get_single_mut() else { return; };

    let is_inventory_open = inv_query.get_single().map_or(false, |s| s.display != Display::None);
    let is_build_menu_open = build_menu_query.get_single().map_or(false, |s| s.display != Display::None);
    let ui_active = is_inventory_open || is_build_menu_open;

    if ui_active {
        if window.cursor.grab_mode != CursorGrabMode::None {
            window.cursor.grab_mode = CursorGrabMode::None;
            window.cursor.visible = true;
        }
        return;
    }

    if mouse_buttons.just_pressed(MouseButton::Left) {
        window.cursor.grab_mode = CursorGrabMode::Locked;
        window.cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        window.cursor.grab_mode = CursorGrabMode::None;
        window.cursor.visible = true;
    }

    if window.cursor.grab_mode == CursorGrabMode::Locked {
        let center_x = window.width() / 2.0;
        let center_y = window.height() / 2.0;
        window.set_cursor_position(Some(Vec2::new(center_x, center_y)));

        for event in mouse_motion.read() {
            body_transform.rotate_y(-event.delta.x * 0.002);
            
            let mut current_pitch = head_transform.rotation.to_euler(EulerRot::YXZ).1;
            let min_pitch = -89.0_f32.to_radians();
            let max_pitch = 89.0_f32.to_radians();
            
            current_pitch = (current_pitch - event.delta.y * 0.002).clamp(min_pitch, max_pitch); 
            head_transform.rotation = Quat::from_rotation_x(current_pitch);
        }
    }
}