use bevy::prelude::{Transform as BevyTransform, *};
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy::render::view::RenderLayers;
use avian3d::prelude::*;
use tracing::info; 

use crate::core::*;
use crate::components::*;
use crate::network::SpacetimeConnection;
use crate::module_bindings::set_camera_mode_reducer::set_camera_mode; 
use crate::module_bindings::set_interior_culling_reducer::set_interior_culling;
use crate::module_bindings::CameraModeType;

// ----------------------------------------------------------------------------
// MMO CHARACTER CAMERA SETTINGS & FREE-LOOK RESOURCE
// ----------------------------------------------------------------------------

#[derive(Resource, Debug, Clone)]
pub struct CharacterCameraSettings {
    /// Desired boom distance in meters (0.0 = pure 1st person, >0.0 = 3rd person over-shoulder).
    pub target_distance: f32,
    /// Smoothed current boom distance in meters.
    pub current_distance: f32,
    /// Minimum allowed distance (0.0).
    pub min_distance: f32,
    /// Maximum allowed distance (7.5m).
    pub max_distance: f32,
    /// Shoulder offset for over-the-shoulder framing (X=right, Y=up, Z=0).
    pub shoulder_offset: Vec3,
    /// Whether Free Look (Alt key) is actively held.
    pub is_free_looking: bool,
    /// Free look relative yaw offset (radians).
    pub free_look_yaw: f32,
    /// Free look relative pitch offset (radians).
    pub free_look_pitch: f32,
    /// Primary look pitch (radians).
    pub current_pitch: f32,
}

impl Default for CharacterCameraSettings {
    fn default() -> Self {
        Self {
            target_distance: 0.0,
            current_distance: 0.0,
            min_distance: 0.0,
            max_distance: 7.5,
            shoulder_offset: Vec3::new(0.35, 0.12, 0.0),
            is_free_looking: false,
            free_look_yaw: 0.0,
            free_look_pitch: 0.0,
            current_pitch: 0.0,
        }
    }
}

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
    console: Res<ConsoleState>,
    mut next_state: ResMut<NextState<CameraMode>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    player_query: Query<&BevyTransform, With<PlayerBody>>,
    mut rts_rig_query: Query<&mut BevyTransform, (With<RtsCameraRig>, Without<PlayerBody>)>,
    conn: Res<SpacetimeConnection>,
    mut culling_state: ResMut<NetworkCullingState>, 
    mut transition: ResMut<CameraTransitionState>,
) {
    if console.is_open {
        return;
    }

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
                
                culling_state.radius = 15;
                culling_state.needs_rebuild = true;
                info!("Camera Mode: RTS. Expanding network culling bounds to 15 chunks.");
            }
            CameraMode::RTS => {
                next_state.set(CameraMode::FPS);
                window.cursor.grab_mode = CursorGrabMode::Locked;
                window.cursor.visible = false;
                
                let _ = conn.db.reducers.set_camera_mode(CameraModeType::Fps);
                
                culling_state.radius = 2;
                culling_state.needs_rebuild = true;
                info!("Camera Mode: FPS. Contracting network culling bounds to 2 chunks.");
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
    console: Res<ConsoleState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut scroll_evts: EventReader<MouseWheel>,
    mut rig_query: Query<&mut BevyTransform, With<RtsCameraRig>>,
    mut child_camera_query: Query<&mut BevyTransform, (With<RtsCameraChild>, Without<RtsCameraRig>)>,
    transition: Res<CameraTransitionState>,
) {
    if transition.is_transitioning || console.is_open { return; }

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
    mut scroll_evts: EventReader<MouseWheel>,
    mut body_query: Query<(Entity, &mut BevyTransform), (With<PlayerBody>, Without<PlayerHead>)>,
    mut head_query: Query<(&mut BevyTransform, &mut RenderLayers), With<PlayerHead>>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>, 
    keys: Res<ButtonInput<KeyCode>>,
    console: Res<ConsoleState>,
    inv_query: Query<&Style, With<InventoryUiRoot>>,
    build_menu_query: Query<&Style, With<BuildMenuRoot>>,
    mut cam_settings: ResMut<CharacterCameraSettings>,
    time: Res<Time>,
    spatial_query: SpatialQuery,
) {
    let Ok(mut window) = window_query.get_single_mut() else { return; };
    let Ok((player_entity, mut body_transform)) = body_query.get_single_mut() else { return; };
    let Ok((mut head_transform, mut head_layers)) = head_query.get_single_mut() else { return; };

    let is_inventory_open = inv_query.get_single().map_or(false, |s| s.display != Display::None);
    let is_build_menu_open = build_menu_query.get_single().map_or(false, |s| s.display != Display::None);
    let ui_active = is_inventory_open || is_build_menu_open || console.is_open;

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
        let center = Vec2::new(window.width() / 2.0, window.height() / 2.0);
        window.set_cursor_position(Some(center));
    }
    if keys.just_pressed(KeyCode::Escape) {
        window.cursor.grab_mode = CursorGrabMode::None;
        window.cursor.visible = true;
    }

    if window.cursor.grab_mode == CursorGrabMode::Locked {
        let center = Vec2::new(window.width() / 2.0, window.height() / 2.0);
        if let Some(cursor_pos) = window.cursor_position() {
            if (cursor_pos - center).length_squared() > 64.0 * 64.0 {
                window.set_cursor_position(Some(center));
            }
        }

        let dt = time.delta_seconds();

        // 1. MMO Continuous Scroll-Wheel Zoom (0.0m = 1st person, >0.0m = 3rd person)
        for scroll in scroll_evts.read() {
            let zoom_step = 0.85;
            if scroll.y < 0.0 {
                cam_settings.target_distance = (cam_settings.target_distance + zoom_step).min(cam_settings.max_distance);
            } else if scroll.y > 0.0 {
                cam_settings.target_distance = (cam_settings.target_distance - zoom_step).max(cam_settings.min_distance);
                if cam_settings.target_distance < 0.35 {
                    cam_settings.target_distance = 0.0;
                }
            }
        }

        // Smooth camera boom distance
        cam_settings.current_distance += (cam_settings.target_distance - cam_settings.current_distance) * (18.0 * dt).min(1.0);
        if cam_settings.current_distance < 0.01 && cam_settings.target_distance == 0.0 {
            cam_settings.current_distance = 0.0;
        }

        // 2. Free-Look Mode (AltLeft or AltRight held)
        let alt_pressed = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
        cam_settings.is_free_looking = alt_pressed;

        for event in mouse_motion.read() {
            if event.delta.length_squared() < 1e-4 {
                continue;
            }
            if alt_pressed {
                // Free Look: Orbit camera around character without rotating player body
                cam_settings.free_look_yaw -= event.delta.x * 0.002;
                cam_settings.free_look_pitch = (cam_settings.free_look_pitch - event.delta.y * 0.002).clamp(-1.4, 1.4);
            } else {
                // Standard Look: Rotate body yaw and head pitch
                body_transform.rotate_y(-event.delta.x * 0.002);
                let min_pitch = -89.0_f32.to_radians();
                let max_pitch = 89.0_f32.to_radians();
                cam_settings.current_pitch = (cam_settings.current_pitch - event.delta.y * 0.002).clamp(min_pitch, max_pitch);
            }
        }

        if !alt_pressed {
            // Smoothly snap free-look offsets back to neutral
            cam_settings.free_look_yaw *= 1.0 - (16.0 * dt).min(1.0);
            cam_settings.free_look_pitch *= 1.0 - (16.0 * dt).min(1.0);
            if cam_settings.free_look_yaw.abs() < 1e-3 { cam_settings.free_look_yaw = 0.0; }
            if cam_settings.free_look_pitch.abs() < 1e-3 { cam_settings.free_look_pitch = 0.0; }
        }

        // 3. Compute Composite Rotation & Placement
        let effective_pitch = (cam_settings.current_pitch + cam_settings.free_look_pitch).clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());
        let look_rot = Quat::from_rotation_y(cam_settings.free_look_yaw) * Quat::from_rotation_x(effective_pitch);

        let head_base_local = Vec3::new(0.0, 0.5, 0.0);

        if cam_settings.current_distance <= 0.001 {
            // Pure First-Person: Camera pinned at eye level
            head_transform.translation = head_base_local;
            head_transform.rotation = look_rot;
        } else {
            // Third-Person Over-The-Shoulder Boom with Spring-Arm Raycast
            let shoulder_blend = (cam_settings.current_distance / 1.5).min(1.0);
            let shoulder = Vec3::new(
                cam_settings.shoulder_offset.x * shoulder_blend,
                cam_settings.shoulder_offset.y * shoulder_blend,
                cam_settings.current_distance,
            );
            let desired_local = head_base_local + look_rot * shoulder;

            let head_world = body_transform.transform_point(head_base_local);
            let desired_cam_world = body_transform.transform_point(desired_local);
            let cast_vec = desired_cam_world - head_world;
            let cast_dist = cast_vec.length();

            let final_cam_world = if cast_dist > 0.05 {
                let cast_dir = cast_vec / cast_dist;
                let filter = SpatialQueryFilter::from_excluded_entities([player_entity]);
                if let Ok(dir3) = Dir3::new(cast_dir) {
                    if let Some(hit) = spatial_query.cast_ray(head_world, dir3, cast_dist, true, filter) {
                        let safe_dist = (hit.time_of_impact - 0.22).max(0.0);
                        head_world + cast_dir * safe_dist
                    } else {
                        desired_cam_world
                    }
                } else {
                    desired_cam_world
                }
            } else {
                head_world
            };

            head_transform.translation = body_transform.compute_matrix().inverse().transform_point3(final_cam_world);
            head_transform.rotation = look_rot;
        }

        // 4. Dynamic RenderLayers: Layer [0, 1] in 1st person, Layer [0, 2] in 3rd person
        let target_layers = if cam_settings.current_distance < 0.25 {
            RenderLayers::from_layers(&[0, 1]) // World + FPS Viewmodels
        } else {
            RenderLayers::from_layers(&[0, 2]) // World + Full Character Body
        };

        if *head_layers != target_layers {
            *head_layers = target_layers;
        }
    }
}

/// Dynamically updates camera FOV for Aim-Down-Sights (ADS) zoom when drawing bows.
pub fn update_camera_fov(
    time: Res<Time>,
    weapon_state: Res<crate::weapons::WeaponState>,
    mut query: Query<&mut Projection, With<FpsCamera>>,
) {
    let Ok(mut projection) = query.get_single_mut() else { return; };
    if let Projection::Perspective(ref mut persp) = *projection {
        let base_fov = 65.0_f32.to_radians();
        let target_fov = if weapon_state.current_weapon == crate::weapons::WeaponType::Bow && weapon_state.bow_drawing {
            // ADS Zoom: Smoothly narrows FOV from 65° down to 50° at full draw
            base_fov - (weapon_state.bow_charge * 15.0_f32.to_radians())
        } else {
            base_fov
        };
        let lerp_speed = if weapon_state.bow_drawing { 10.0 } else { 16.0 };
        persp.fov += (target_fov - persp.fov) * (lerp_speed * time.delta_seconds()).min(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_camera_settings_defaults() {
        let settings = CharacterCameraSettings::default();
        assert_eq!(settings.target_distance, 0.0);
        assert_eq!(settings.current_distance, 0.0);
        assert_eq!(settings.min_distance, 0.0);
        assert!(settings.max_distance >= 5.0);
        assert_eq!(settings.shoulder_offset.x, 0.35);
        assert!(!settings.is_free_looking);
    }

    #[test]
    fn test_zoom_clamping_and_threshold_snapping() {
        let mut settings = CharacterCameraSettings::default();

        // Zoom out
        settings.target_distance = (settings.target_distance + 0.85).min(settings.max_distance);
        assert_eq!(settings.target_distance, 0.85);

        // Zoom out to max
        settings.target_distance = 15.0;
        settings.target_distance = settings.target_distance.clamp(settings.min_distance, settings.max_distance);
        assert_eq!(settings.target_distance, 7.5);

        // Zoom in below threshold snaps to 0.0
        settings.target_distance = 0.30;
        if settings.target_distance < 0.35 {
            settings.target_distance = 0.0;
        }
        assert_eq!(settings.target_distance, 0.0);
    }

    #[test]
    fn test_free_look_angle_clamping() {
        let mut settings = CharacterCameraSettings::default();
        settings.free_look_pitch = 2.5;
        settings.free_look_pitch = settings.free_look_pitch.clamp(-1.4, 1.4);
        assert_eq!(settings.free_look_pitch, 1.4);
    }

    #[test]
    fn test_camera_ads_fov_zoom() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(crate::weapons::WeaponState {
            current_weapon: crate::weapons::WeaponType::Bow,
            bow_charge: 1.0,
            bow_drawing: true,
            ..default()
        });

        let cam_id = app.world_mut().spawn((
            FpsCamera,
            Projection::Perspective(PerspectiveProjection {
                fov: 65.0_f32.to_radians(),
                ..default()
            }),
        )).id();

        app.add_systems(Update, update_camera_fov);
        app.update();
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
        app.update();

        let proj = app.world().get::<Projection>(cam_id).unwrap();
        if let Projection::Perspective(persp) = proj {
            assert!(persp.fov < 65.0_f32.to_radians());
        } else {
            panic!("Expected Perspective projection");
        }
    }
}