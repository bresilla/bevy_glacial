//! `bevy_glacial` gizmo demo — a single accent-coloured cube on a
//! flat ground, manipulated with the upstream `transform-gizmo-bevy`
//! crate (translate + rotate + scale handles, full picking / drag /
//! draw built in).
//!
//! Mouse:
//!   * MMB-drag         pan the camera focus
//!   * LMB+RMB-drag     orbit yaw + pitch
//!   * Scroll           zoom (log-smoothed)
//!   * MMB × 2          snap focus to cursor's ground point
//!   * LMB-drag a handle  translate / rotate / scale the cube
//!
//! Keyboard (forwarded by the gizmo crate's hotkey resource):
//!   * G                translate-only
//!   * R                rotate-only
//!   * S                scale-only
//!   * X / Y / Z        constrain to that axis
//!   * Esc              clear mode override

use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy_glacial::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "bevy_glacial gizmo demo".into(),
                resolution: (1280u32, 800u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GlacialPlugin)
        .add_plugins(TransformGizmoPlugin)
        .insert_resource(GizmoOptions {
            hotkeys: Some(GizmoHotkeys::default()),
            ..default()
        })
        .init_resource::<GizmoAutoScale>()
        .add_systems(Startup, setup_scene)
        .add_systems(Update, auto_scale_gizmo_to_target)
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    grid_cfg: Res<GroundGrid>,
) {
    // Plain ground plane — a big flat cuboid, dark slate so the cube
    // and gizmo handles read against it without competing colour.
    commands.spawn((
        Name::new("Ground"),
        Mesh3d(meshes.add(Cuboid::new(200.0, 0.1, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.20, 0.22),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.05, 0.0),
    ));

    // LOD ground grid.
    spawn_circle_meshes(&mut commands, &mut meshes, &mut materials, &grid_cfg);

    // The subject of the gizmo: an accent-tinted 2 m cube.
    commands.spawn((
        Name::new("GizmoSubject"),
        Mesh3d(meshes.add(Cuboid::new(2.0, 2.0, 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.65, 0.54, 0.98),
            perceptual_roughness: 0.55,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.0, 0.0),
        GizmoTarget::default(),
    ));

    // Sun + ambient.
    let sun_shadow = CascadeShadowConfigBuilder {
        num_cascades: 1,
        minimum_distance: 0.1,
        maximum_distance: 100.0,
        first_cascade_far_bound: 100.0,
        overlap_proportion: 0.0,
    }
    .build();
    commands.spawn((
        Name::new("Sun"),
        Transform::from_xyz(5.0, 50.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: true,
            ..default()
        },
        sun_shadow,
    ));

    // Camera with the chase rig + the GizmoCamera marker so the
    // transform-gizmo plugin knows which view to draw against.
    let chase = ChaseCamera::default();
    let mut tr = Transform::default();
    apply_rig(&chase, &mut tr);
    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        tr,
        AmbientLight {
            color: Color::WHITE,
            brightness: 120.0,
            ..default()
        },
        chase,
        GizmoCamera,
    ));
}
