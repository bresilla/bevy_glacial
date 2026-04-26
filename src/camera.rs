//! Free orbit camera — astrocraft-style mouse navigation.
//!
//! Bindings:
//!   Scroll wheel                — zoom (logarithmic, smoothed)
//!   Middle-click + drag         — pan (translate focus in world XZ plane)
//!   Left + Right pressed + drag — orbit (yaw + pitch)
//!   Double middle-click         — snap focus to cursor's world-point
//!
//! No automatic follow is wired in here — this crate is project-
//! agnostic. If your app needs follow / cinematic flight, attach a
//! sibling component on the camera entity and run a system that
//! writes to [`ChaseCamera`] before [`chase_camera_control`] runs.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Attach this to a `Camera3d` entity. Fully user-driven. Project-
/// agnostic — no fly-target / vehicle / scene-graph references.
#[derive(Component, Clone, Debug)]
pub struct ChaseCamera {
    /// World-space focus point.
    pub focus: Vec3,
    /// Orbit angle around world +Y, radians. 0 looks along +Z.
    pub yaw: f32,
    /// Elevation above the horizon, radians.
    pub elevation: f32,
    pub distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub pan_sensitivity: f32,
    pub orbit_speed: f32,
    /// Exponential zoom coefficient — 0.05 = 5 % per scroll line.
    pub zoom_step: f64,
    /// Smoothing for zoom (exponential toward target distance).
    pub zoom_smoothing: f64,
    /// Time of the last middle-click — used to detect double-clicks
    /// for the focus-snap gesture.
    pub last_middle_click_secs: f32,
}

impl Default for ChaseCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            yaw: 0.0,
            elevation: 25f32.to_radians(),
            distance: 14.0,
            min_distance: 3.0,
            max_distance: 120.0,
            pan_sensitivity: 0.0012,
            orbit_speed: 0.005,
            zoom_step: 0.05,
            zoom_smoothing: 6.0,
            last_middle_click_secs: -10.0,
        }
    }
}

/// Handles pan (middle drag), orbit (L+R drag), and double-middle-
/// click re-centring (ray-casts the cursor to the ground plane).
pub fn chase_camera_control(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    bevy_cameras: Query<(&Camera, &GlobalTransform)>,
    mut pan_anchor: Local<Option<Vec2>>,
    mut orbit_anchor: Local<Option<Vec2>>,
    mut cameras: Query<(&mut ChaseCamera, &mut Transform)>,
) {
    let middle_pressed = mouse_buttons.pressed(MouseButton::Middle);
    let left_pressed = mouse_buttons.pressed(MouseButton::Left);
    let right_pressed = mouse_buttons.pressed(MouseButton::Right);
    let both_lr = left_pressed && right_pressed;

    if !middle_pressed {
        *pan_anchor = None;
    }
    if !both_lr {
        *orbit_anchor = None;
    }

    let cursor_position = primary_window.single().ok().and_then(|w| w.cursor_position());

    // --- Pan: middle-click drag ---
    let mut pan_delta = Vec2::ZERO;
    if middle_pressed {
        if let Some(pos) = cursor_position {
            if let Some(anchor) = *pan_anchor {
                pan_delta = pos - anchor;
            }
            *pan_anchor = Some(pos);
        }
    }

    // --- Orbit: left+right click drag ---
    let mut orbit_delta = Vec2::ZERO;
    if both_lr {
        if let Some(pos) = cursor_position {
            if orbit_anchor.is_none() {
                *orbit_anchor = Some(pos);
            }
            if let Some(anchor) = *orbit_anchor {
                orbit_delta = pos - anchor;
            }
            *orbit_anchor = Some(pos);
        }
    }

    let now = time.elapsed_secs();

    for (mut cam, mut tr) in &mut cameras {
        // Double-middle-click → re-centre focus on cursor-to-ground point.
        if mouse_buttons.just_pressed(MouseButton::Middle) {
            let is_double = now - cam.last_middle_click_secs < 0.35;
            cam.last_middle_click_secs = now;
            if is_double {
                if let (Some(cursor), Ok((camera, cam_tr))) =
                    (cursor_position, bevy_cameras.single())
                {
                    if let Some(hit) = cursor_ray_to_ground(camera, cam_tr, cursor) {
                        cam.focus = hit;
                    }
                }
            }
        }

        // Pan → slide focus in world XZ plane, aligned to current yaw.
        if pan_delta != Vec2::ZERO {
            let pan_speed = cam.distance * cam.pan_sensitivity;
            let forward = Vec3::new(cam.yaw.sin(), 0.0, cam.yaw.cos());
            let right = Vec3::new(forward.z, 0.0, -forward.x);
            cam.focus += (-right * pan_delta.x - forward * pan_delta.y) * pan_speed;
        }

        // Orbit.
        if orbit_delta != Vec2::ZERO {
            cam.yaw -= orbit_delta.x * cam.orbit_speed;
            cam.elevation += orbit_delta.y * cam.orbit_speed;
            cam.elevation = cam.elevation.clamp(
                5f32.to_radians(),
                89f32.to_radians(),
            );
        }

        apply_rig(&cam, &mut tr);
    }
}

/// Scroll-wheel zoom — logarithmic with exponential smoothing.
/// Skips when Ctrl is held: that gesture is reserved for callers
/// that want to use Ctrl+scroll for a different action (e.g.
/// rotating a ghost during drag-to-place).
pub fn chase_camera_zoom(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    mut zoom_target: Local<Option<f64>>,
    mut cameras: Query<(&mut ChaseCamera, &mut Transform)>,
) {
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        wheel.read().for_each(drop);
        return;
    }
    let mut scroll_delta = 0.0_f64;
    for event in wheel.read() {
        scroll_delta += match event.unit {
            MouseScrollUnit::Line => event.y as f64,
            MouseScrollUnit::Pixel => event.y as f64 / 32.0,
        };
    }

    let Ok((mut cam, mut tr)) = cameras.single_mut() else { return };

    let target = zoom_target.get_or_insert(cam.distance as f64);
    let min = cam.min_distance as f64;
    let max = cam.max_distance as f64;

    if scroll_delta != 0.0 {
        let log_target = target.max(0.1).log10();
        let new_log = log_target - scroll_delta * cam.zoom_step;
        *target = 10f64.powf(new_log).clamp(min, max);
    }

    let dt = time.delta_secs_f64();
    let log_current = (cam.distance as f64).max(0.1).ln();
    let log_target = target.max(0.1).ln();
    let log_diff = log_target - log_current;
    if log_diff.abs() > 1e-4 {
        let new_log = log_current + log_diff * (cam.zoom_smoothing * dt).min(0.9);
        cam.distance = new_log.exp() as f32;
        apply_rig(&cam, &mut tr);
    } else if log_diff.abs() > 1e-5 {
        cam.distance = *target as f32;
        apply_rig(&cam, &mut tr);
    }
}

/// Set the camera's world-space pose from the rig state
/// (focus + yaw + elevation + distance). Public so external
/// systems (e.g. a fly-target animation in the host app) can
/// reuse the same maths.
pub fn apply_rig(cam: &ChaseCamera, tr: &mut Transform) {
    let horizontal = cam.distance * cam.elevation.cos();
    let vertical = cam.distance * cam.elevation.sin();
    let offset = Vec3::new(
        horizontal * cam.yaw.sin(),
        vertical,
        horizontal * cam.yaw.cos(),
    );
    let cam_world = cam.focus + offset;
    *tr = Transform::from_translation(cam_world).looking_at(cam.focus, Vec3::Y);
}

/// Cast a ray from the camera through `cursor` and intersect it
/// with the y = 0 ground plane. Used by the double-middle-click
/// focus-snap gesture; also exposed publicly so callers can do
/// their own viewport-to-ground picking.
pub fn cursor_ray_to_ground(
    camera: &Camera,
    cam_tr: &GlobalTransform,
    cursor: Vec2,
) -> Option<Vec3> {
    let ray = camera.viewport_to_world(cam_tr, cursor).ok()?;
    let origin = ray.origin;
    let direction = *ray.direction;
    if direction.y.abs() < 1e-6 {
        return None;
    }
    let t = -origin.y / direction.y;
    if t < 0.0 {
        return None;
    }
    Some(origin + direction * t)
}
