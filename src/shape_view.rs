//! Rendering for the 3-D loss shapes of [`bevaru_core::shapes`]: the surface,
//! its axes and colour legend, the clip level, the linked 2-D slice, and a
//! probe that reads the loss and its gradient under the pointer.
//!
//! Insert a [`ShapePlot`] resource and call [`spawn_shape_scene`]; the systems
//! of [`ShapeViewPlugin`] run while the resource exists. Replacing or mutating
//! the resource updates the surface in place.

use bevaru_core::shapes::{ShapeError, ShapeParams, ShapeView, SurfaceGrid};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::loss_surface::{BOX_HEIGHT, BOX_SIDE, GridMap, HeightColors, grid_mesh};
use crate::orbit::{OrbitPlugin, OrbitRig, OrbitView};

pub struct ShapeViewPlugin;

impl Plugin for ShapeViewPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<OrbitPlugin>() {
            app.add_plugins(OrbitPlugin);
        }
        app.init_resource::<ShapeOverlays>()
            .init_resource::<Probe>()
            .add_systems(
                Update,
                (sync_scene, update_probe, draw_gizmos)
                    .chain()
                    .run_if(resource_exists::<ShapePlot>),
            )
            .add_systems(
                EguiPrimaryContextPass,
                overlay_labels.run_if(
                    resource_exists::<ShapePlot>
                        .and_then(not(resource_exists::<crate::capture::HideOverlays>)),
                ),
            );
    }
}

/// The view on screen: what was sampled, and how it is placed and coloured.
#[derive(Resource, Debug, Clone)]
pub struct ShapePlot {
    pub view: ShapeView,
    pub params: ShapeParams,
    pub grid: SurfaceGrid,
    pub map: GridMap,
    pub colors: HeightColors,
}

impl ShapePlot {
    pub fn sample(
        view: ShapeView,
        params: ShapeParams,
        resolution: usize,
    ) -> Result<Self, ShapeError> {
        Ok(Self::from_grid(
            view,
            params,
            view.sample(&params, resolution)?,
        ))
    }

    /// Place an already-sampled grid (e.g. one sampled off the main thread).
    pub fn from_grid(view: ShapeView, params: ShapeParams, grid: SurfaceGrid) -> Self {
        let map = if view == ShapeView::ThreeClassProbabilities {
            GridMap::ternary(&grid)
        } else {
            GridMap::fit(&grid)
        };
        let colors = HeightColors::coolwarm(grid.min, grid.max);
        Self {
            view,
            params,
            grid,
            map,
            colors,
        }
    }

    pub fn mesh(&self) -> Result<Mesh, &'static str> {
        grid_mesh(&self.grid, &self.map, &self.colors)
    }

    /// The drawn height at inputs `(x, y)`: the exact loss, capped like the
    /// grid. `None` off the surface.
    pub fn surface_value(&self, x: f64, y: f64) -> Option<f64> {
        let (gx, gy) = (&self.grid.x, &self.grid.y);
        let inside = |v: f64, lo: f64, hi: f64| v >= lo - 1e-9 && v <= hi + 1e-9;
        if !(inside(x, gx.min, gx.max) && inside(y, gy.min, gy.max)) {
            return None;
        }
        let v = self.view.value(x, y, &self.params)?;
        Some(self.grid.cap.map_or(v, |cap| v.min(cap)))
    }

    /// Half a grid cell in each input: how close to a kink counts as on it.
    fn half_cell(&self) -> (f64, f64) {
        let half = |a: &bevaru_core::shapes::Axis| (a.max - a.min) / (a.n - 1) as f64 / 2.0;
        (half(&self.grid.x), half(&self.grid.y))
    }

    pub fn is_clipped(&self) -> bool {
        self.grid.clipped.iter().any(|&c| c)
    }

    /// The highlighted 2-D slice as a world polyline on the surface.
    pub fn slice_world(&self, n: usize) -> Option<Vec<Vec3>> {
        let points = self.view.slice_points(&self.params, n)?;
        Some(
            points
                .into_iter()
                .filter_map(|[x, y, _, _]| {
                    self.surface_value(x, y).map(|v| self.map.world(x, y, v))
                })
                .collect(),
        )
    }

    /// The camera's home view for this placement.
    pub fn home_view() -> OrbitView {
        OrbitView {
            yaw: -1.1,
            pitch: 0.5,
            distance: 27.0,
            target: Vec3::new(0.0, 0.0, BOX_HEIGHT * 0.35),
        }
    }
}

/// Which overlays are drawn.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ShapeOverlays {
    pub slice: bool,
    pub legend: bool,
    pub ticks: bool,
    pub probe: bool,
    pub clip: bool,
}

impl Default for ShapeOverlays {
    fn default() -> Self {
        Self {
            slice: true,
            legend: true,
            ticks: true,
            probe: true,
            clip: true,
        }
    }
}

#[derive(Component)]
pub struct ShapeSurface;

#[derive(Component)]
pub struct ShapeClipPlane;

/// Spawn the surface, the clip plane, lights, and an orbiting camera for the
/// current [`ShapePlot`], each with `marker` (e.g. `ExperienceEntity`).
pub fn spawn_shape_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    plot: &ShapePlot,
    marker: impl Bundle + Clone,
) -> Result<(), &'static str> {
    commands.spawn((
        marker.clone(),
        ShapeSurface,
        Mesh3d(meshes.add(plot.mesh()?)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
    ));
    commands.spawn((
        marker.clone(),
        ShapeClipPlane,
        Mesh3d(meshes.add(Rectangle::new(BOX_SIDE, BOX_SIDE))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.75, 0.1, 0.1, 0.16),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));
    for (direction, illuminance) in [
        (Vec3::new(-0.4, -0.6, -1.0), 6000.0),
        (Vec3::new(0.7, 0.5, -0.4), 2500.0),
    ] {
        commands.spawn((
            marker.clone(),
            DirectionalLight {
                illuminance,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::default().looking_to(direction, Vec3::Z),
        ));
    }
    commands.spawn((
        marker,
        OrbitRig::bundle(ShapePlot::home_view()),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.97, 0.97, 0.98)),
            ..default()
        },
        AmbientLight {
            brightness: 900.0,
            ..default()
        },
    ));
    Ok(())
}

fn sync_scene(
    plot: Res<ShapePlot>,
    overlays: Res<ShapeOverlays>,
    surface: Query<&Mesh3d, With<ShapeSurface>>,
    mut clip: Query<(&mut Transform, &mut Visibility), With<ShapeClipPlane>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !(plot.is_changed() || overlays.is_changed()) {
        return;
    }
    if plot.is_changed() {
        match plot.mesh() {
            Ok(mesh) => {
                for handle in &surface {
                    if let Some(mut current) = meshes.get_mut(&handle.0) {
                        *current = mesh.clone();
                    }
                }
            }
            Err(e) => error!(
                "bevaru: could not build the {} surface: {e}",
                plot.view.id()
            ),
        }
    }
    for (mut transform, mut visibility) in &mut clip {
        // The square plane would overhang the probability triangle; that view
        // gets a triangular outline from `draw_gizmos` instead.
        let square = plot.view != ShapeView::ThreeClassProbabilities;
        match plot
            .grid
            .cap
            .filter(|_| square && overlays.clip && plot.is_clipped())
        {
            Some(cap) => {
                *transform = Transform::from_xyz(0.0, 0.0, plot.map.height(cap));
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

// ---------------------------------------------------------------------------
// Probe

/// What the probe reads at a point: computed from the core loss functions at
/// the exact inputs, not interpolated from the mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeReading {
    pub x: f64,
    pub y: f64,
    pub loss: f64,
    /// −∇L in input coordinates, or `None` where the gradient is zero.
    pub downhill: Option<(f64, f64)>,
    /// Which subgradient is shown at a kink, or why there is no gradient.
    pub note: Option<&'static str>,
}

/// Read the loss and its gradient at `(x, y)`. `tolerance` is how close to a
/// kink counts as on it, per input (half a grid cell).
pub fn probe_reading(
    view: ShapeView,
    params: &ShapeParams,
    x: f64,
    y: f64,
    tolerance: (f64, f64),
) -> Option<ProbeReading> {
    let loss = view.value(x, y, params)?;
    let (gx, gy) = view.gradient(x, y, params)?;
    let flat = gx.abs() < 1e-12 && gy.abs() < 1e-12;
    let downhill = (!flat && gx.is_finite() && gy.is_finite()).then_some((-gx, -gy));
    let note = kink_note(view, params, x, y, tolerance).or(match (flat, view) {
        (true, ShapeView::TwoScoresZeroOne) => {
            Some("No gradient here: 0-1 loss is flat everywhere except the cliff.")
        }
        (true, _) => Some("No gradient here: the loss is flat."),
        (false, _) => None,
    });
    Some(ProbeReading {
        x,
        y,
        loss,
        downhill,
        note,
    })
}

/// At a kink, say which subgradient the library uses there.
fn kink_note(
    view: ShapeView,
    params: &ShapeParams,
    x: f64,
    y: f64,
    (tx, ty): (f64, f64),
) -> Option<&'static str> {
    let m = params.loss.margin();
    let tol = tx.max(ty);
    use ShapeView::*;
    match view {
        PredictionMae if (y - x).abs() <= tol => {
            Some("On the crease ŷ = y: the library's subgradient here is 0.")
        }
        TwoScoresHinge if ((x - y) - m).abs() <= tol => Some(
            "At the hinge corner: the library uses −1 below the margin and 0 from the margin on.",
        ),
        HingeMargin if (x - y).abs() <= tol => Some(
            "At the hinge corner m = μ: the library uses −1 below the margin and 0 from it on.",
        ),
        ThreeClassHingeWestonWatkins if (m + x).abs() <= tx || (m + y).abs() <= ty => {
            Some("On a hinge corner: a rival exactly at the margin contributes 0 to the gradient.")
        }
        ThreeClassHingeCrammerSinger if (x - y).abs() <= tol && m + x.max(y) > 0.0 => {
            Some("Where z₂ = z₃ the worst rival switches; the library picks z₂.")
        }
        ThreeClassHingeCrammerSinger if (m + x.max(y)).abs() <= tol => {
            Some("On the hinge corner: the library uses 0 once the worst rival is at the margin.")
        }
        _ => None,
    }
}

/// Where a ray first meets the surface, as grid inputs. Marches the exact
/// (capped) loss as a heightfield, then refines the crossing by bisection.
pub fn march(plot: &ShapePlot, ray: Ray3d) -> Option<(f64, f64)> {
    let below = |p: Vec3| -> Option<bool> {
        let (x, y) = plot.map.inputs(p.truncate());
        plot.surface_value(x, y).map(|v| p.z <= plot.map.height(v))
    };
    let dir = *ray.direction;
    let reach = 4.0 * (BOX_SIDE + BOX_HEIGHT) + ray.origin.length();
    let steps = 1200;
    let step = reach / steps as f32;
    let mut previous = 0.0;
    for i in 1..=steps {
        let t = i as f32 * step;
        if below(ray.origin + dir * t) == Some(true) {
            let (mut lo, mut hi) = (previous, t);
            for _ in 0..30 {
                let mid = 0.5 * (lo + hi);
                if below(ray.origin + dir * mid) == Some(true) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Some(plot.map.inputs((ray.origin + dir * hi).truncate()));
        }
        previous = t;
    }
    None
}

/// The probe under the pointer, if any.
#[derive(Resource, Default, Debug, Clone)]
pub struct Probe {
    pub reading: Option<ProbeReading>,
    /// Where the pointer meets the surface, in world coordinates.
    pub at: Option<Vec3>,
}

fn update_probe(
    plot: Res<ShapePlot>,
    overlays: Res<ShapeOverlays>,
    mut probe: ResMut<Probe>,
    cameras: Query<(&Camera, &GlobalTransform), With<OrbitRig>>,
    windows: Query<&Window>,
    egui: Option<Res<bevy_egui::input::EguiWantsInput>>,
) {
    let hit = || {
        if !overlays.probe || egui.as_ref().is_some_and(|e| e.wants_any_pointer_input()) {
            return None;
        }
        let cursor = windows.iter().find_map(Window::cursor_position)?;
        let (camera, transform) = cameras.iter().next()?;
        let ray = camera.viewport_to_world(transform, cursor).ok()?;
        let (x, y) = march(&plot, ray)?;
        let reading = probe_reading(plot.view, &plot.params, x, y, plot.half_cell())?;
        let at = plot.map.world(x, y, plot.surface_value(x, y)?);
        Some((reading, at))
    };
    let (reading, at) = hit().unzip();
    if probe.reading != reading {
        probe.reading = reading;
        probe.at = at;
    }
}

/// The downhill arrow's end, on the surface: `length` world units along
/// steepest descent in the ground plane, lifted to the surface height there.
pub fn arrow_end(plot: &ShapePlot, reading: &ProbeReading, length: f32) -> Option<Vec3> {
    let (dx, dy) = reading.downhill?;
    // −∇ is a covector in inputs; steepest descent on the ground is J⁻ᵀ·(−∇).
    let j = Mat2::from_cols(plot.map.ex, plot.map.ey);
    let ground_dir =
        (j.inverse().transpose() * Vec2::new(dx as f32, dy as f32)).normalize_or_zero();
    if ground_dir == Vec2::ZERO {
        return None;
    }
    let start = plot.map.ground(reading.x, reading.y);
    let mut l = length;
    while l > 0.05 {
        let (x, y) = plot.map.inputs(start + ground_dir * l);
        if let Some(v) = plot.surface_value(x, y) {
            return Some(plot.map.world(x, y, v));
        }
        l *= 0.5;
    }
    None
}

// ---------------------------------------------------------------------------
// Axes, slice, and labels

/// Round tick values covering `[min, max]`, about `target` of them.
pub fn nice_ticks(min: f64, max: f64, target: usize) -> Vec<f64> {
    if !(min.is_finite() && max.is_finite()) || max <= min {
        return vec![min];
    }
    let raw = (max - min) / target.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|k| k * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let first = (min / step).ceil() as i64;
    let last = (max / step).floor() as i64;
    (first..=last).map(|i| i as f64 * step).collect()
}

fn format_tick(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

const AXIS: Color = Color::srgb(0.25, 0.25, 0.3);
const SLICE: Color = Color::srgb(0.95, 0.65, 0.05);
const ARROW: Color = Color::srgb(0.05, 0.6, 0.2);

fn draw_gizmos(
    plot: Res<ShapePlot>,
    overlays: Res<ShapeOverlays>,
    probe: Res<Probe>,
    mut gizmos: Gizmos,
) {
    let (gx, gy) = (&plot.grid.x, &plot.grid.y);
    let map = &plot.map;
    let ternary = plot.view == ShapeView::ThreeClassProbabilities;
    // Base outline and the vertical height axis.
    let base: Vec<Vec3> = if ternary {
        let [a, b, c] = GridMap::ternary_corners();
        vec![a.extend(0.0), b.extend(0.0), c.extend(0.0), a.extend(0.0)]
    } else {
        [
            (gx.min, gy.min),
            (gx.max, gy.min),
            (gx.max, gy.max),
            (gx.min, gy.max),
            (gx.min, gy.min),
        ]
        .into_iter()
        .map(|(x, y)| map.ground(x, y).extend(0.0))
        .collect()
    };
    gizmos.linestrip(base, AXIS);
    let corner = height_axis_foot(&plot);
    gizmos.line(corner.extend(0.0), corner.extend(BOX_HEIGHT), AXIS);
    if overlays.ticks {
        for v in nice_ticks(plot.grid.min, plot.grid.max, 5) {
            let h = map.height(v);
            gizmos.line(
                corner.extend(h),
                (corner + Vec2::new(-0.25, 0.25)).extend(h),
                AXIS,
            );
        }
        if !ternary {
            for x in nice_ticks(gx.min, gx.max, 6) {
                let p = map.ground(x, gy.min);
                gizmos.line(
                    p.extend(0.0),
                    (p - map.ey.normalize() * 0.25).extend(0.0),
                    AXIS,
                );
            }
            for y in nice_ticks(gy.min, gy.max, 6) {
                let p = map.ground(gx.max, y);
                gizmos.line(
                    p.extend(0.0),
                    (p + map.ex.normalize() * 0.25).extend(0.0),
                    AXIS,
                );
            }
        }
    }
    if overlays.slice
        && let Some(curve) = plot.slice_world(161)
        && curve.len() >= 2
    {
        let (a, b) = (curve[0], curve[curve.len() - 1]);
        let lift = Vec3::Z * 0.03;
        gizmos.linestrip(curve.iter().map(|p| *p + lift), SLICE);
        // The slice plane: a vertical rectangle through the curve.
        let (a0, b0) = (a.with_z(0.0), b.with_z(0.0));
        let top = Vec3::Z * BOX_HEIGHT;
        gizmos.linestrip([a0, b0, b0 + top, a0 + top, a0], SLICE.with_alpha(0.45));
    }
    if overlays.clip
        && let Some(cap) = plot.grid.cap.filter(|_| plot.is_clipped())
    {
        let h = map.height(cap);
        let color = Color::srgba(0.75, 0.1, 0.1, 0.6);
        if ternary {
            let [a, b, c] = GridMap::ternary_corners();
            gizmos.linestrip([a, b, c, a].map(|p| p.extend(h)), color);
        } else {
            gizmos.rect(
                Isometry3d::from_translation(Vec3::new(0.0, 0.0, h)),
                Vec2::splat(BOX_SIDE),
                color,
            );
        }
    }
    if let (Some(reading), Some(at)) = (&probe.reading, probe.at) {
        gizmos.sphere(Isometry3d::from_translation(at), 0.08, ARROW);
        if let Some(end) = arrow_end(&plot, reading, 1.6) {
            gizmos.arrow(at + Vec3::Z * 0.05, end + Vec3::Z * 0.05, ARROW);
        }
    }
}

/// The ground point the height axis stands on: the back-left corner.
fn height_axis_foot(plot: &ShapePlot) -> Vec2 {
    if plot.view == ShapeView::ThreeClassProbabilities {
        GridMap::ternary_corners()[1]
    } else {
        plot.map.ground(plot.grid.x.min, plot.grid.y.max)
    }
}

fn overlay_labels(
    mut contexts: EguiContexts,
    plot: Res<ShapePlot>,
    overlays: Res<ShapeOverlays>,
    probe: Res<Probe>,
    cameras: Query<(&Camera, &GlobalTransform), With<OrbitRig>>,
) -> Result {
    let Some((camera, cam)) = cameras.iter().next() else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?.clone();
    let clip = camera
        .logical_viewport_rect()
        .map(|r| {
            egui::Rect::from_min_max(egui::pos2(r.min.x, r.min.y), egui::pos2(r.max.x, r.max.y))
        })
        .unwrap_or_else(|| ctx.viewport_rect());
    let painter = ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("shape-labels"),
        ))
        .with_clip_rect(clip);
    let project = |p: Vec3| {
        camera
            .world_to_viewport(cam, p)
            .ok()
            .map(|v| egui::pos2(v.x, v.y))
    };
    let ink = egui::Color32::from_rgb(50, 50, 60);
    let small = egui::FontId::proportional(12.0);
    // Centred on the projected point, but shifted to stay inside the view so
    // a label never runs under a side panel.
    let label = |p: Vec3, text: String, font: egui::FontId| {
        if let Some(pos) = project(p) {
            let galley = painter.layout_no_wrap(text, font, ink);
            let half = galley.size() / 2.0;
            let inner = clip.shrink(4.0);
            let x = pos.x.clamp(
                inner.min.x + half.x,
                (inner.max.x - half.x).max(inner.min.x + half.x),
            );
            let y = pos.y.clamp(
                inner.min.y + half.y,
                (inner.max.y - half.y).max(inner.min.y + half.y),
            );
            painter.galley(egui::pos2(x, y) - half, galley, ink);
        }
    };
    let (gx, gy) = (&plot.grid.x, &plot.grid.y);
    let map = &plot.map;
    let title = egui::FontId::proportional(14.0);

    if plot.view == ShapeView::ThreeClassProbabilities {
        let names = ["class 1 (true)", "class 2", "class 3"];
        for (corner, name) in GridMap::ternary_corners().into_iter().zip(names) {
            label((corner * 1.12).extend(0.0), name.into(), title.clone());
        }
    } else {
        let out_y = -map.ey.normalize();
        let out_x = map.ex.normalize();
        if overlays.ticks {
            for x in nice_ticks(gx.min, gx.max, 6) {
                label(
                    (map.ground(x, gy.min) + out_y * 0.6).extend(0.0),
                    format_tick(x),
                    small.clone(),
                );
            }
            for y in nice_ticks(gy.min, gy.max, 6) {
                label(
                    (map.ground(gx.max, y) + out_x * 0.6).extend(0.0),
                    format_tick(y),
                    small.clone(),
                );
            }
        }
        let mid_x = map.ground((gx.min + gx.max) / 2.0, gy.min) + out_y * 2.2;
        let mid_y = map.ground(gx.max, (gy.min + gy.max) / 2.0) + out_x * 3.0;
        label(
            mid_x.extend(0.0),
            format!("{} — {}", gx.symbol, gx.name),
            title.clone(),
        );
        label(
            mid_y.extend(0.0),
            format!("{} — {}", gy.symbol, gy.name),
            title.clone(),
        );
    }
    let foot = height_axis_foot(&plot);
    if overlays.ticks {
        for v in nice_ticks(plot.grid.min, plot.grid.max, 5) {
            label(
                (foot + Vec2::new(-0.7, 0.7)).extend(map.height(v)),
                format_tick(v),
                small.clone(),
            );
        }
    }
    label(
        foot.extend(BOX_HEIGHT + 0.6),
        plot.grid.height_label.clone(),
        title.clone(),
    );
    if overlays.clip
        && let Some(cap) = plot.grid.cap.filter(|_| plot.is_clipped())
    {
        let corner = if plot.view == ShapeView::ThreeClassProbabilities {
            GridMap::ternary_corners()[2]
        } else {
            Vec2::new(BOX_SIDE / 2.0, -BOX_SIDE / 2.0)
        }
        .extend(map.height(cap));
        label(
            corner + Vec3::Z * 0.4,
            format!("clipped at {}", format_tick(cap)),
            small.clone(),
        );
    }
    if overlays.slice
        && let Some(curve) = plot.slice_world(3)
        && let Some(end) = curve.last()
        && let Some(pos) = project(*end + Vec3::Z * 0.5)
    {
        painter.text(
            pos,
            egui::Align2::CENTER_BOTTOM,
            "2-D curve",
            small.clone(),
            egui::Color32::from_rgb(200, 130, 0),
        );
    }

    // Legend and probe readout, pinned to the viewport's bottom-right.
    let mut y = clip.max.y - 12.0;
    if overlays.legend {
        let (w, h) = (180.0, 12.0);
        let rect = egui::Rect::from_min_size(
            egui::pos2(clip.max.x - w - 16.0, y - h - 16.0),
            egui::vec2(w, h),
        );
        let steps = 48;
        for i in 0..steps {
            let t = i as f64 / (steps - 1) as f64;
            let c = plot.colors.at_fraction(t).to_srgba().to_u8_array();
            let x0 = rect.min.x + w * i as f32 / steps as f32;
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(x0, rect.min.y),
                    egui::vec2(w / steps as f32 + 0.5, h),
                ),
                0.0,
                egui::Color32::from_rgb(c[0], c[1], c[2]),
            );
        }
        let top = if plot.is_clipped() { "≥ " } else { "" };
        painter.text(
            rect.left_bottom() + egui::vec2(0.0, 2.0),
            egui::Align2::LEFT_TOP,
            format_tick(plot.grid.min),
            small.clone(),
            ink,
        );
        painter.text(
            rect.right_bottom() + egui::vec2(0.0, 2.0),
            egui::Align2::RIGHT_TOP,
            format!("{top}{}", format_tick(plot.grid.max)),
            small.clone(),
            ink,
        );
        painter.text(
            rect.center_top() - egui::vec2(0.0, 2.0),
            egui::Align2::CENTER_BOTTOM,
            &plot.grid.height_label,
            small.clone(),
            ink,
        );
        y = rect.min.y - 24.0;
    }
    if let Some(r) = &probe.reading {
        let mut lines = vec![
            format!("{} = {:.3}   {} = {:.3}", gx.symbol, r.x, gy.symbol, r.y),
            format!("loss = {:.4}", r.loss),
        ];
        if let Some((dx, dy)) = r.downhill {
            lines.push(format!("−∇L = ({dx:.3}, {dy:.3})"));
        }
        if let Some(note) = r.note {
            lines.push(note.into());
        }
        painter.text(
            egui::pos2(clip.max.x - 16.0, y),
            egui::Align2::RIGHT_BOTTOM,
            lines.join("\n"),
            egui::FontId::proportional(13.0),
            ink,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::loss_curve_chart;
    use bevaru_core::loss::LossParams;

    fn plot(view: ShapeView) -> ShapePlot {
        ShapePlot::sample(view, ShapeParams::default(), 81).unwrap()
    }

    #[test]
    fn slices_equal_the_2d_chart_data() {
        let params = ShapeParams::default();
        for view in ShapeView::ALL {
            let Some(_) = view.slice() else { continue };
            for &loss in &view.losses() {
                let chart =
                    loss_curve_chart(loss.task(), &[loss], &LossParams::default(), &[], 3.0);
                let series = &chart.series[0];
                for (&arg, &expected) in series.x.iter().zip(&series.y) {
                    let (x, y) = view.slice_input(arg, &params).unwrap();
                    let Some(value) = view.value(x, y, &params) else {
                        continue;
                    };
                    assert!(
                        (value - expected).abs() < 1e-9,
                        "{} at {arg}: {value} vs {expected}",
                        view.id()
                    );
                }
            }
        }
    }

    #[test]
    fn probe_on_mse_points_toward_the_diagonal() {
        let r = probe_reading(
            ShapeView::PredictionMse,
            &ShapeParams::default(),
            0.0,
            2.0,
            (0.04, 0.04),
        )
        .unwrap();
        assert_eq!(r.loss, 4.0);
        let (dx, dy) = r.downhill.unwrap();
        // A small step downhill brings ŷ − y closer to zero.
        assert!(((2.0 + 0.01 * dy) - (0.0 + 0.01 * dx)).abs() < 2.0);
        assert!(r.note.is_none());
        let p = plot(ShapeView::PredictionMse);
        let start = p.map.ground(0.0, 2.0);
        let end = arrow_end(&p, &r, 1.0).unwrap();
        let (x, y) = p.map.inputs(end.truncate());
        assert!((y - x).abs() < 2.0, "arrow ends nearer the diagonal");
        assert!(end.truncate().distance(start) > 0.5);
    }

    #[test]
    fn flat_regions_have_no_gradient() {
        let params = ShapeParams::default();
        let tol = (0.05, 0.05);
        let hinge = probe_reading(ShapeView::TwoScoresHinge, &params, 3.0, 0.0, tol).unwrap();
        assert_eq!(hinge.downhill, None);
        assert!(hinge.note.unwrap().starts_with("No gradient here"));
        for (x, y) in [(1.0, -2.0), (-1.0, 2.0)] {
            let z = probe_reading(ShapeView::TwoScoresZeroOne, &params, x, y, tol).unwrap();
            assert_eq!(z.downhill, None);
            assert!(z.note.unwrap().starts_with("No gradient here"));
        }
    }

    #[test]
    fn kinks_name_their_subgradient() {
        let params = ShapeParams::default();
        let mae = probe_reading(ShapeView::PredictionMae, &params, 1.0, 1.0, (0.04, 0.04)).unwrap();
        assert!(mae.note.unwrap().contains("subgradient here is 0"));
        let hinge =
            probe_reading(ShapeView::TwoScoresHinge, &params, 1.0, 0.0, (0.05, 0.05)).unwrap();
        assert!(hinge.note.unwrap().contains("−1 below the margin"));
        let away =
            probe_reading(ShapeView::TwoScoresHinge, &params, -1.0, 0.0, (0.05, 0.05)).unwrap();
        assert!(away.note.is_none() && away.downhill.is_some());
    }

    #[test]
    fn the_probe_ray_finds_the_surface() {
        let p = plot(ShapeView::PredictionMse);
        // Straight down onto (y, ŷ) = (1, −0.5).
        let ground = p.map.ground(1.0, -0.5);
        let ray = Ray3d::new(ground.extend(50.0), Dir3::NEG_Z);
        let (x, y) = march(&p, ray).unwrap();
        assert!((x - 1.0).abs() < 1e-3 && (y + 0.5).abs() < 1e-3);
        // A ray that misses the box.
        let miss = Ray3d::new(Vec3::new(40.0, 40.0, 50.0), Dir3::Z);
        assert!(march(&p, miss).is_none());
        // Oblique, from the home camera toward the centre.
        let eye = ShapePlot::home_view().transform().translation;
        let ray = Ray3d::new(eye, Dir3::new(Vec3::new(0.0, 0.0, 1.0) - eye).unwrap());
        let (x, y) = march(&p, ray).unwrap();
        let v = p.surface_value(x, y).unwrap();
        let hit = p.map.world(x, y, v);
        let along = (hit - eye)
            .normalize()
            .dot((Vec3::new(0.0, 0.0, 1.0) - eye).normalize());
        assert!(along > 0.9999, "the hit lies on the ray");
    }

    #[test]
    fn slice_curve_lies_on_the_surface_and_clip_is_reported() {
        let p = plot(ShapeView::CrossEntropy);
        assert!(p.is_clipped());
        let curve = p.slice_world(41).unwrap();
        assert_eq!(curve.len(), 41);
        assert!(
            curve
                .iter()
                .all(|v| v.z >= -1e-4 && v.z <= BOX_HEIGHT + 1e-4)
        );
        assert!(plot(ShapeView::ThreeClassSoftmax).slice_world(41).is_none());
        assert!(!plot(ShapeView::PredictionMse).is_clipped());
    }

    #[test]
    fn ticks_are_round_and_inside() {
        assert_eq!(
            nice_ticks(-3.0, 3.0, 6),
            vec![-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0]
        );
        let t = nice_ticks(0.0, 8.0, 5);
        assert_eq!(t, vec![0.0, 2.0, 4.0, 6.0, 8.0]);
        assert_eq!(format_tick(2.5), "2.5");
        assert_eq!(format_tick(-0.0), "0");
    }
}
