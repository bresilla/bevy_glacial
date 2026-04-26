//! The common import line for apps building on top of `bevy_glacial`.
//!
//! ```ignore
//! use bevy::prelude::*;
//! use bevy_glacial::prelude::*;
//! ```

pub use crate::{
    axis_gizmo::{
        draw_axis_gizmos, draw_axis_triad, draw_axis_triad_with_colors, AxisGizmo,
        AxisGizmoPlugin, DEFAULT_AXIS_COLORS,
    },
    camera::{
        apply_rig, chase_camera_control, chase_camera_zoom, cursor_ray_to_ground, ChaseCamera,
        ChaseCameraPlugin,
    },
    follow::{follow_camera_target, FollowCameraPlugin, FollowTarget},
    grid::{
        build_grid_meshes, spawn_circle_meshes, update_grid_alpha, GridKind, GroundGrid,
        GroundGridPlugin, LocalGrid, LEVEL_HALF, LEVEL_STEPS,
    },
    selection_ring::{
        SelectionRing, SelectionRingEntity, SelectionRingExtension, SelectionRingMaterial,
        SelectionRingPlugin, SelectionRingSettings,
    },
    GlacialPlugins,
};

// Vendored transform-gizmo passthrough.
pub use crate::gizmo::{
    auto_scale_gizmo_to_target, EnumSet, GizmoAutoScale, GizmoCamera, GizmoHotkeys, GizmoMode,
    GizmoOptions, GizmoOrientation, GizmoTarget, GizmoVisuals, TransformGizmoPlugin,
};
