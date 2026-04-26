//! # bevy_glacial — reusable 3D-scene niceties for Bevy editor apps.
//!
//! Project-agnostic Bevy primitives that pair well with
//! [`bevy_frost`](https://github.com/bresilla/bevy_frost): a
//! mouse-driven orbit camera, a self-fading LOD ground grid, a
//! selection-ring shader, and a transform-gizmo (translate / rotate /
//! scale handles) inlined from the upstream
//! [`transform-gizmo`](https://github.com/urholaukkarinen/transform-gizmo)
//! project by **Urho Laukkarinen** (MIT or Apache-2.0; full text at
//! `LICENSE-MIT.transform-gizmo` / `LICENSE-APACHE.transform-gizmo`).
//!
//! ## Shape
//!
//! * [`camera`] — [`ChaseCamera`] component + control / zoom systems.
//! * [`grid`] — [`GroundGrid`] resource + LOD line / dot meshes that
//!   follow the camera's focus.
//! * [`selection_ring`] — extended-material flat ring that attaches to
//!   a target entity.
//! * [`gizmo`] — vendored transform-gizmo: add [`TransformGizmoPlugin`]
//!   and tag your camera with [`GizmoCamera`]; attach [`GizmoTarget`]
//!   to whichever entity you want manipulable.
//!
//! ## Getting started
//!
//! ```ignore
//! use bevy::prelude::*;
//! use bevy_glacial::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(GlacialPlugin)
//!         .add_plugins(TransformGizmoPlugin)
//!         .run();
//! }
//! ```

pub mod camera;
pub mod gizmo;
pub mod grid;
pub mod prelude;
pub mod selection_ring;

pub use camera::{
    apply_rig, chase_camera_control, chase_camera_zoom, cursor_ray_to_ground, ChaseCamera,
};
pub use grid::{
    build_grid_meshes, spawn_circle_meshes, update_grid_alpha, GridKind, GroundGrid, LocalGrid,
    LEVEL_HALF, LEVEL_STEPS,
};
pub use selection_ring::{
    SelectionRing, SelectionRingEntity, SelectionRingExtension, SelectionRingMaterial,
    SelectionRingPlugin, SelectionRingSettings,
};

pub use gizmo::{
    auto_scale_gizmo_to_target, EnumSet, GizmoAutoScale, GizmoCamera, GizmoHotkeys, GizmoMode,
    GizmoOptions, GizmoOrientation, GizmoTarget, GizmoVisuals, TransformGizmoPlugin,
};

use bevy::prelude::*;

/// Camera + grid bundle. Does **not** include the transform-gizmo
/// plugin — add [`TransformGizmoPlugin`] separately if you want handles.
pub struct GlacialPlugin;

impl Plugin for GlacialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<grid::GroundGrid>().add_systems(
            Update,
            (
                camera::chase_camera_control,
                camera::chase_camera_zoom,
                grid::build_grid_meshes,
                grid::update_grid_alpha,
            ),
        );
    }
}
