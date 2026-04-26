//! Local LOD ground grid — a stack of flat square grids of lines
//! centred on the chase-camera's focus.
//!
//! Each level uses a fixed decade spacing (1 m, 10 m, 100 m, 1 km);
//! the level whose cell size best matches the current view scale
//! fades in, neighbours fade down, and far levels disappear. Unlike
//! the old sphere-surface system, the meshes here are built once at
//! startup and just translate with the camera — there's no per-frame
//! vertex rebuild, so panning stays smooth.
//!
//! Fades:
//!   - Per-level Gaussian on `log(cam_dist / step)` — peaks at the
//!     level whose cell size is ~10× the camera distance.
//!   - Radial in the mesh: alpha drops with distance from centre, so
//!     the outer edge dissolves into the ground instead of ending in
//!     a hard square border.
//!   - Major-line boost every 10th line reads as a "chapter" tick
//!     without becoming noisy.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

use super::camera::ChaseCamera;

// ── User-visible settings ───────────────────────────────────────────

#[derive(Resource, Clone, Copy)]
pub struct GroundGrid {
    pub visible: bool,
    /// Base RGB + alpha. Alpha scales everything.
    pub color: Color,
}

impl Default for GroundGrid {
    fn default() -> Self {
        Self {
            visible: true,
            color: Color::srgba(80.0 / 255.0, 70.0 / 255.0, 70.0 / 255.0, 0.35),
        }
    }
}

// ── LOD levels ──────────────────────────────────────────────────────

/// Multiplicative spacing between adjacent LOD levels. `4.0` means
/// each level's cells are 4× the previous level's, and every 4th line
/// in a level lines up with a major line of the level above it.
pub const LEVEL_SCALE: f32 = 4.0;
/// Cell size of the finest level (metres). Each subsequent level is
/// ×[`LEVEL_SCALE`] this — set the base smaller for a denser grid,
/// larger for a coarser one.
const BASE_STEP: f32 = 0.5;
/// Cell size per level (metres). Geometric ×[`LEVEL_SCALE`] so
/// neighbouring levels stay tile-aligned.
pub const LEVEL_STEPS: [f32; 4] = [
    BASE_STEP,
    BASE_STEP * LEVEL_SCALE,
    BASE_STEP * LEVEL_SCALE * LEVEL_SCALE,
    BASE_STEP * LEVEL_SCALE * LEVEL_SCALE * LEVEL_SCALE,
];
/// Lines per side for each level — kept constant so the per-level
/// mesh cost stays flat. `LEVEL_HALF[i] = LINES_PER_SIDE * LEVEL_STEPS[i]`.
const LINES_PER_SIDE: f32 = 50.0;
/// Half-extent of each level's square (metres).
pub const LEVEL_HALF: [f32; 4] = [
    LINES_PER_SIDE * LEVEL_STEPS[0],
    LINES_PER_SIDE * LEVEL_STEPS[1],
    LINES_PER_SIDE * LEVEL_STEPS[2],
    LINES_PER_SIDE * LEVEL_STEPS[3],
];
/// Every Nth line is a major line (brighter alpha). Equal to
/// [`LEVEL_SCALE`] so a level's major lines coincide with the next
/// level's minor lines.
const MAJOR_EVERY: i32 = LEVEL_SCALE as i32;
/// Major-line alpha boost (multiplied against the base colour alpha)
/// so every `MAJOR_EVERY`-th line reads as a heavier "chapter" tick.
const MAJOR_BOOST: f32 = 3.5;
/// Fraction of the level's half-extent over which the radial fade
/// kicks in. `1.0` = fade smoothly from centre to edge (old
/// behaviour); `0.4` = lines stay full strength until the outer 40 %
/// of the radius and then drop off — outer lines stay readable for
/// longer.
const EDGE_FADE_FRAC: f32 = 0.4;
/// Grid rides this height above the tangent plane.
const GRID_Y: f32 = 0.05;

/// Peak fade at `log10(cam_dist / step) ≈ GAUSS_PEAK`. Set so a
/// level peaks when the camera is [`LEVEL_SCALE`]× its cell size
/// away — i.e. when the cells look like a comfortable fraction of
/// the view.
const GAUSS_PEAK: f32 = 0.602_06; // log10(4)
/// Bell width. Wider = adjacent levels linger longer before fading
/// out as the camera moves between their natural distances.
const GAUSS_WIDTH: f32 = 0.55;

// ── Components ──────────────────────────────────────────────────────

/// Two layers per LOD level — the cross-hatched line grid, and a
/// triangle-fan disc at every major intersection. Dots ride a
/// hair above lines so they read on top.
#[derive(Component, Copy, Clone, PartialEq, Eq, Debug)]
pub enum GridKind {
    Lines,
    Dots,
}

#[derive(Component)]
pub struct LocalGrid {
    pub level: u8,
    pub kind: GridKind,
    pub material: Handle<StandardMaterial>,
}

/// Dot-disc radius as a fraction of the cell step. ~2.4 % reads as
/// a fine tick at every intersection without becoming a bullet point.
const DOT_RADIUS_FRAC: f32 = 0.024;
/// Number of triangle-fan segments per dot. 8 is enough to read as
/// a circle at the size we draw them and keeps the vertex count
/// down — there's a dot at every minor intersection.
const DOT_SEGMENTS: u32 = 8;
/// Tiny Y offsets so dots paint on top of lines without z-fighting.
const LINES_Y: f32 = GRID_Y;
const DOTS_Y: f32 = GRID_Y + 0.002;


// ── Spawn ───────────────────────────────────────────────────────────

/// Spawn two entities per LOD level — the line cross-hatch and the
/// dots at major intersections. Name kept for back-compat with
/// `main::setup_scene`.
pub fn spawn_circle_meshes(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cfg: &GroundGrid,
) {
    let make_mat = |materials: &mut Assets<StandardMaterial>| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        })
    };

    for level in 0..LEVEL_STEPS.len() {
        let step = LEVEL_STEPS[level];
        let half = LEVEL_HALF[level];
        let is_top = level + 1 == LEVEL_STEPS.len();

        // Lines layer.
        let lines_mesh = meshes.add(build_level_mesh(cfg, step, half, is_top));
        let lines_mat = make_mat(materials);
        commands.spawn((
            Name::new(format!("LocalGrid[L{level}]:Lines")),
            LocalGrid {
                level: level as u8,
                kind: GridKind::Lines,
                material: lines_mat.clone(),
            },
            Transform::from_xyz(0.0, LINES_Y, 0.0),
            Mesh3d(lines_mesh),
            MeshMaterial3d(lines_mat),
            NotShadowCaster,
            Visibility::Visible,
        ));

        // Dots layer — small disc at every minor intersection
        // (skipping anything the parent level draws).
        let dots_mesh = meshes.add(build_dots_mesh(cfg, step, half, is_top));
        let dots_mat = make_mat(materials);
        commands.spawn((
            Name::new(format!("LocalGrid[L{level}]:Dots")),
            LocalGrid {
                level: level as u8,
                kind: GridKind::Dots,
                material: dots_mat.clone(),
            },
            Transform::from_xyz(0.0, DOTS_Y, 0.0),
            Mesh3d(dots_mesh),
            MeshMaterial3d(dots_mat),
            NotShadowCaster,
            Visibility::Visible,
        ));
    }
}

// ── Per-frame systems ──────────────────────────────────────────────

/// Camera-follow: slide every grid level with the chase-camera
/// focus, snapped to that level's minor step so lines stay
/// world-aligned. Also writes each level's fade to its material's
/// alpha — the level whose cell size matches the current zoom blends
/// in, the rest fade out.
pub fn build_grid_meshes(
    cameras: Query<&ChaseCamera>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cfg: Res<GroundGrid>,
    mut grids: Query<(&LocalGrid, &mut Transform, &mut Visibility)>,
) {
    let Ok(cam) = cameras.single() else { return };
    let cam_dist = cam.distance.max(0.1);

    for (grid, mut tr, mut vis) in grids.iter_mut() {
        let step = LEVEL_STEPS[grid.level as usize];
        // Snap to the *major* step so the mesh translates as a
        // rigid sheet — both lines and dots sit at the same world
        // positions every frame.
        let snap_step = step * MAJOR_EVERY as f32;
        tr.translation.x = (cam.focus.x / snap_step).round() * snap_step;
        tr.translation.y = match grid.kind {
            GridKind::Lines => LINES_Y,
            GridKind::Dots => DOTS_Y,
        };
        tr.translation.z = (cam.focus.z / snap_step).round() * snap_step;

        let fade = level_fade(cam_dist, step);
        let a = cfg.color.alpha() * fade;
        *vis = if cfg.visible && a > 0.005 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Some(m) = materials.get_mut(&grid.material) {
            let srgba = cfg.color.to_srgba();
            m.base_color = Color::srgba(srgba.red, srgba.green, srgba.blue, a);
        }
    }
}

/// When the UI changes grid colour, rebuild the tiny per-level line
/// meshes so the vertex-colour alpha pattern updates. Infrequent.
pub fn update_grid_alpha(
    cfg: Res<GroundGrid>,
    mut meshes: ResMut<Assets<Mesh>>,
    grids: Query<(&LocalGrid, &Mesh3d)>,
) {
    if !cfg.is_changed() {
        return;
    }
    for (grid, mesh_h) in grids.iter() {
        let step = LEVEL_STEPS[grid.level as usize];
        let half = LEVEL_HALF[grid.level as usize];
        let is_top = grid.level as usize + 1 == LEVEL_STEPS.len();
        if let Some(m) = meshes.get_mut(&mesh_h.0) {
            *m = match grid.kind {
                GridKind::Lines => build_level_mesh(&cfg, step, half, is_top),
                GridKind::Dots => build_dots_mesh(&cfg, step, half, is_top),
            };
        }
    }
}

// ── LOD fade ───────────────────────────────────────────────────────

fn level_fade(cam_dist: f32, step: f32) -> f32 {
    // Gaussian bell in log-space over the ratio `cam_dist / step`.
    // Peak at GAUSS_PEAK, width GAUSS_WIDTH. Near zero outside
    // ~2 × width from the peak.
    let log_r = (cam_dist / step).max(1e-3).log10();
    let z = (log_r - GAUSS_PEAK) / GAUSS_WIDTH;
    (-0.5 * z * z).exp()
}

// ── Mesh generation ────────────────────────────────────────────────

fn build_level_mesh(cfg: &GroundGrid, step: f32, half: f32, _is_top: bool) -> Mesh {
    let s = cfg.color.to_srgba();
    let base_rgba = [s.red, s.green, s.blue, s.alpha];

    let n = (half / step) as i32;
    let total_lines = (2 * n + 1) * 2;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity((total_lines * 2) as usize);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity((total_lines * 2) as usize);

    // Radial alpha + major-line boost. We deliberately do NOT skip
    // lines that the next-coarser LOD also draws — the GPU's
    // `LineList` primitive is 1 pixel wide, so two levels stamping
    // the same line just composite into a slightly brighter pixel
    // (invisible). Skipping introduces gaps as soon as the parent
    // level fades out.
    let line_color = |i: i32| -> [f32; 4] {
        let t = (i.abs() as f32) / (n as f32);
        let edge_fade = {
            // Stay at full strength until `1 - EDGE_FADE_FRAC` of the
            // radius, then smoothstep to 0 over the remaining outer
            // band. Outer lines persist much longer than the old
            // centre-to-edge smoothstep.
            let u = ((1.0 - t) / EDGE_FADE_FRAC).clamp(0.0, 1.0);
            u * u * (3.0 - 2.0 * u)
        };
        let major = i.rem_euclid(MAJOR_EVERY) == 0;
        let boost = if major { MAJOR_BOOST } else { 1.0 };
        [
            base_rgba[0],
            base_rgba[1],
            base_rgba[2],
            (base_rgba[3] * edge_fade * boost).clamp(0.0, 1.0),
        ]
    };

    // Lines running along +X (constant Z).
    for i in -n..=n {
        let z = i as f32 * step;
        let c = line_color(i);
        positions.push([-half, 0.0, z]);
        positions.push([half, 0.0, z]);
        colors.push(c);
        colors.push(c);
    }
    // Lines running along +Z (constant X).
    for i in -n..=n {
        let x = i as f32 * step;
        let c = line_color(i);
        positions.push([x, 0.0, -half]);
        positions.push([x, 0.0, half]);
        colors.push(c);
        colors.push(c);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::LineList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

/// Triangle-fan disc mesh — one disc at every minor intersection
/// inside the level's `half`-extent. Major intersections get a
/// brighter alpha (same `MAJOR_BOOST` as the line layer) so the
/// grid still has tick-mark structure.
fn build_dots_mesh(cfg: &GroundGrid, step: f32, half: f32, _is_top: bool) -> Mesh {
    let s = cfg.color.to_srgba();
    let base_rgba = [s.red, s.green, s.blue, s.alpha];

    let n = (half / step) as i32;
    let radius = step * DOT_RADIUS_FRAC;
    let segs = DOT_SEGMENTS;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for i in -n..=n {
        for j in -n..=n {
            // No major-boost on dots — dots from every LOD stack at
            // the world origin, and a 3.5× boost on each turns them
            // into a single bright disc. Major hierarchy lives on
            // the line layer.
            let t_axis = (i.abs().max(j.abs()) as f32) / (n as f32);
            let edge_fade = {
                let u = ((1.0 - t_axis) / EDGE_FADE_FRAC).clamp(0.0, 1.0);
                u * u * (3.0 - 2.0 * u)
            };
            let alpha = (base_rgba[3] * edge_fade).clamp(0.0, 1.0);
            let color = [base_rgba[0], base_rgba[1], base_rgba[2], alpha];

            let cx = i as f32 * step;
            let cz = j as f32 * step;

            // Shrink in world-space the further the dot sits from
            // the panel centre. Square the fade so the size drops
            // off faster than the alpha — outer dots read as fine
            // pin-pricks before they vanish.
            let dot_radius = radius * edge_fade * edge_fade;

            let centre_idx = positions.len() as u32;
            positions.push([cx, 0.0, cz]);
            colors.push(color);
            for k in 0..segs {
                let theta = (k as f32 / segs as f32) * std::f32::consts::TAU;
                let (sn, cs) = theta.sin_cos();
                positions.push([cx + cs * dot_radius, 0.0, cz + sn * dot_radius]);
                colors.push(color);
            }
            for k in 0..segs {
                let next = (k + 1) % segs;
                indices.push(centre_idx);
                indices.push(centre_idx + 1 + k);
                indices.push(centre_idx + 1 + next);
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    mesh
}
