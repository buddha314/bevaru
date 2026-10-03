//! The rendered scene: one camera per comparison pane, data markers, decision
//! regions, boundaries, margins, support vectors, residuals, and the 3-D
//! decision plane.
//!
//! Each pane's content lives at its own far-off world offset and has its own
//! camera viewport, so panes never see each other's geometry.

use bevaru_core::{ModelKind, Task};
use bevy::asset::RenderAssetUsages;
use bevy::camera::{ScalingMode, Viewport};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;

use crate::experiment::{Experiment, UnloadExperiment};
use crate::geometry::Boundary;
use crate::playback::PaneViews;

/// Distance between panes' world-space content.
pub const PANE_SPACING: f32 = 10_000.0;

pub fn pane_offset(pane: usize) -> Vec3 {
    Vec3::X * pane as f32 * PANE_SPACING
}

/// Okabe–Ito colours: distinguishable with every common colour-vision deficiency.
pub const CLASS_COLORS: [Color; 3] = [
    Color::srgb(0.0, 0.447, 0.698),
    Color::srgb(0.835, 0.369, 0.0),
    Color::srgb(0.0, 0.62, 0.451),
];

pub const BACKGROUND: Color = Color::srgb(0.965, 0.965, 0.957);
const BOUNDARY_COLOR: Color = Color::srgb(0.1, 0.1, 0.12);
const MARGIN_COLOR: Color = Color::srgb(0.35, 0.35, 0.38);
const SUPPORT_COLOR: Color = Color::srgb(0.902, 0.624, 0.0);
const RESIDUAL_COLOR: Color = Color::srgba(0.835, 0.369, 0.0, 0.7);
const BOUNDS_COLOR: Color = Color::srgb(0.82, 0.82, 0.8);

/// World-space marker radius; data is fitted into roughly ±5.
const MARKER_RADIUS: f32 = 0.11;
const REGION_RESOLUTION: u32 = 160;

#[derive(Resource, Debug, Clone)]
pub struct SceneSettings {
    pub show_margins: bool,
    pub show_support_vectors: bool,
    pub show_regions: bool,
    pub show_residuals: bool,
    pub show_bounds: bool,
}

impl Default for SceneSettings {
    fn default() -> Self {
        Self {
            show_margins: true,
            show_support_vectors: true,
            show_regions: true,
            show_residuals: true,
            show_bounds: true,
        }
    }
}

/// Screen space taken by UI panels, in logical pixels; panes share the rest.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct UiInsets {
    pub left: f32,
    pub right: f32,
}

/// Logical-pixel rectangles of each pane in a window of `size`.
pub fn pane_rects(size: Vec2, insets: UiInsets, panes: usize) -> Vec<Rect> {
    let n = panes.max(1) as f32;
    let width = (size.x - insets.left - insets.right).max(1.0) / n;
    (0..panes)
        .map(|i| {
            let x0 = insets.left + i as f32 * width;
            Rect::new(x0, 0.0, x0 + width, size.y)
        })
        .collect()
}

/// Fit every pane's camera to its data.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct FrameData;

/// Dashed lines for margins.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct MarginGizmos;

/// Everything the scene spawns for an experiment; despawned on unload.
#[derive(Component)]
pub(crate) struct SceneEntity;

#[derive(Debug, Clone, Copy)]
struct CameraRig {
    /// Pane-local look-at point.
    target: Vec3,
    /// 3-D: distance from target.
    distance: f32,
    yaw: f32,
    pitch: f32,
    /// 2-D: visible world height.
    ortho_height: f32,
    needs_fit: bool,
}

#[derive(Component)]
pub struct PaneCamera {
    pub pane: usize,
    rig: CameraRig,
}

#[derive(Component)]
struct RegionQuad {
    pane: usize,
    image: Handle<Image>,
    drawn: Option<Option<Boundary>>,
}

#[derive(Component)]
struct DecisionPlane {
    pane: usize,
    mesh: Handle<Mesh>,
}

#[derive(Resource)]
struct Markers {
    flat: [Handle<Mesh>; 3],
    solid: [Handle<Mesh>; 3],
    flat_materials: [Handle<StandardMaterial>; 3],
    solid_materials: [Handle<StandardMaterial>; 3],
}

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneSettings>()
            .init_resource::<UiInsets>()
            .add_message::<FrameData>()
            .add_message::<UnloadExperiment>()
            .init_resource::<BuiltScene>()
            .init_gizmo_group::<MarginGizmos>()
            .insert_resource(ClearColor(BACKGROUND))
            .add_systems(Startup, setup)
            .add_systems(PreUpdate, unload_scene)
            .add_systems(
                Update,
                (
                    rebuild_scene,
                    (frame_data, camera_input).chain(),
                    layout_cameras,
                    update_regions,
                    update_planes,
                    draw_overlays,
                )
                    .chain()
                    .after(crate::playback::PlaybackSystems)
                    .run_if(resource_exists::<Experiment>),
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmos: ResMut<GizmoConfigStore>,
) {
    let (lines, _) = gizmos.config_mut::<DefaultGizmoConfigGroup>();
    lines.line.width = 2.5;
    let (margins, _) = gizmos.config_mut::<MarginGizmos>();
    margins.line.width = 2.0;
    margins.line.style = GizmoLineStyle::Dashed {
        gap_scale: 2.0,
        line_scale: 4.0,
    };

    let r = MARKER_RADIUS;
    let material = |m: &mut Assets<StandardMaterial>, color: Color, unlit: bool| {
        m.add(StandardMaterial {
            base_color: color,
            unlit,
            perceptual_roughness: 0.6,
            ..default()
        })
    };
    commands.insert_resource(Markers {
        flat: [
            meshes.add(Circle::new(r)),
            meshes.add(RegularPolygon::new(r * 1.35, 3)),
            meshes.add(Rectangle::new(r * 1.7, r * 1.7)),
        ],
        solid: [
            meshes.add(Sphere::new(r).mesh().ico(2).expect("valid subdivision")),
            meshes.add(Cone::new(r, r * 2.2)),
            meshes.add(Cuboid::new(r * 1.6, r * 1.6, r * 1.6)),
        ],
        flat_materials: CLASS_COLORS.map(|c| material(&mut materials, c, true)),
        solid_materials: CLASS_COLORS.map(|c| material(&mut materials, c, false)),
    });
    commands.insert_resource(GlobalAmbientLight {
        brightness: 600.0,
        ..default()
    });
}

/// The `(generation, pane count)` the scene was last built for.
#[derive(Resource, Default)]
struct BuiltScene(Option<(u64, usize)>);

/// Despawn every pane's cameras, lights, markers, regions, and planes. Their
/// meshes, materials, and region images are owned only by these entities,
/// so the assets are freed with them.
fn unload_scene(
    mut commands: Commands,
    mut requests: MessageReader<UnloadExperiment>,
    mut built: ResMut<BuiltScene>,
    entities: Query<Entity, With<SceneEntity>>,
) {
    if requests.read().count() == 0 {
        return;
    }
    built.0 = None;
    for e in &entities {
        commands.entity(e).despawn();
    }
}

fn rebuild_scene(
    mut commands: Commands,
    experiment: Res<Experiment>,
    markers: Res<Markers>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    old: Query<Entity, With<SceneEntity>>,
    mut built: ResMut<BuiltScene>,
) {
    let key = (experiment.generation, experiment.panes.len());
    if built.0 == Some(key) {
        return;
    }
    built.0 = Some(key);
    for e in &old {
        commands.entity(e).despawn();
    }
    let (lo, hi) = experiment.bounds;
    let is_3d = experiment.dims == 3;

    for pane in 0..experiment.panes.len() {
        let offset = pane_offset(pane);
        let rig = CameraRig {
            target: (lo + hi) / 2.0,
            distance: 20.0,
            yaw: -2.2,
            pitch: 0.45,
            ortho_height: 12.0,
            needs_fit: true,
        };
        let projection = if is_3d {
            Projection::Perspective(PerspectiveProjection::default())
        } else {
            Projection::from(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: rig.ortho_height,
                },
                ..OrthographicProjection::default_3d()
            })
        };
        commands.spawn((
            SceneEntity,
            PaneCamera { pane, rig },
            Camera3d::default(),
            Camera {
                order: pane as isize,
                ..default()
            },
            projection,
            Transform::from_translation(offset + Vec3::Z * 100.0),
        ));

        for (i, p) in experiment.points.iter().enumerate() {
            let class = experiment.classes[i].min(2);
            let (mesh, material) = if is_3d {
                (
                    markers.solid[class].clone(),
                    markers.solid_materials[class].clone(),
                )
            } else {
                (
                    markers.flat[class].clone(),
                    markers.flat_materials[class].clone(),
                )
            };
            commands.spawn((
                SceneEntity,
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(offset + *p),
            ));
        }

        if is_3d {
            commands.spawn((
                SceneEntity,
                DirectionalLight {
                    illuminance: 4000.0,
                    ..default()
                },
                Transform::from_translation(offset + Vec3::new(4.0, -6.0, 10.0))
                    .looking_at(offset, Vec3::Z),
            ));
            let mesh = meshes.add(Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            ));
            let material = materials.add(StandardMaterial {
                base_color: Color::srgba(0.45, 0.45, 0.5, 0.35),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            commands.spawn((
                SceneEntity,
                DecisionPlane {
                    pane,
                    mesh: mesh.clone(),
                },
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(offset),
            ));
        } else if experiment.task == Task::Classification {
            let image = images.add(Image::new_fill(
                Extent3d {
                    width: REGION_RESOLUTION,
                    height: REGION_RESOLUTION,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[0, 0, 0, 0],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ));
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(image.clone()),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            });
            let size = (hi - lo).truncate();
            commands.spawn((
                SceneEntity,
                RegionQuad {
                    pane,
                    image,
                    drawn: None,
                },
                Mesh3d(meshes.add(Rectangle::new(size.x, size.y))),
                MeshMaterial3d(material),
                Transform::from_translation(offset + ((lo + hi) / 2.0).with_z(-1.0)),
            ));
        }
    }
}

fn frame_data(mut requests: MessageReader<FrameData>, mut cameras: Query<&mut PaneCamera>) {
    if requests.read().count() > 0 {
        for mut cam in &mut cameras {
            cam.rig.needs_fit = true;
        }
    }
}

/// Assign each pane camera its viewport and apply its rig.
fn layout_cameras(
    window: Single<&Window, With<PrimaryWindow>>,
    insets: Res<UiInsets>,
    experiment: Res<Experiment>,
    mut cameras: Query<(
        &mut PaneCamera,
        &mut Camera,
        &mut Transform,
        &mut Projection,
    )>,
) {
    let rects = pane_rects(window.size(), *insets, experiment.panes.len());
    let scale = window.scale_factor();
    let (lo, hi) = experiment.bounds;
    for (mut cam, mut camera, mut transform, mut projection) in &mut cameras {
        let Some(rect) = rects.get(cam.pane) else {
            continue;
        };
        let size = (rect.size() * scale).as_uvec2().max(UVec2::ONE);
        let pos = (rect.min * scale).as_uvec2();
        let physical = window.physical_size();
        // A viewport must lie inside the window or the frame fails to render.
        if pos.x + size.x > physical.x || pos.y + size.y > physical.y {
            continue;
        }
        camera.viewport = Some(Viewport {
            physical_position: pos,
            physical_size: size,
            ..default()
        });
        let aspect = rect.width() / rect.height().max(1.0);
        let offset = pane_offset(cam.pane);
        let rig = &mut cam.rig;
        if rig.needs_fit {
            rig.needs_fit = false;
            rig.target = (lo + hi) / 2.0;
            let extent = hi - lo;
            rig.ortho_height = extent.y.max(extent.x / aspect) * 1.15;
            rig.distance = extent.length() * 0.5 / (std::f32::consts::FRAC_PI_8).tan() * 1.15;
        }
        match &mut *projection {
            Projection::Orthographic(o) => {
                o.scaling_mode = ScalingMode::FixedVertical {
                    viewport_height: rig.ortho_height,
                };
                *transform = Transform::from_translation(offset + rig.target.with_z(100.0));
            }
            _ => {
                let dir = Vec3::new(
                    rig.pitch.cos() * rig.yaw.cos(),
                    rig.pitch.cos() * rig.yaw.sin(),
                    rig.pitch.sin(),
                );
                *transform = Transform::from_translation(offset + rig.target + dir * rig.distance)
                    .looking_at(offset + rig.target, Vec3::Z);
            }
        }
    }
}

/// 2-D: drag to pan, scroll to zoom. 3-D: left-drag to orbit, right-drag to
/// pan, scroll to zoom. Ignored while the pointer is over the UI.
fn camera_input(
    window: Single<&Window, With<PrimaryWindow>>,
    insets: Res<UiInsets>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    egui: Option<Res<EguiWantsInput>>,
    mut active: Local<Option<usize>>,
    mut cameras: Query<(&mut PaneCamera, &Projection)>,
) {
    if egui.is_some_and(|e| e.wants_any_pointer_input()) {
        *active = None;
        return;
    }
    let rects = pane_rects(window.size(), *insets, cameras.iter().len());
    let hovered = window
        .cursor_position()
        .and_then(|c| rects.iter().position(|r| r.contains(c)));
    let dragging =
        buttons.any_pressed([MouseButton::Left, MouseButton::Right, MouseButton::Middle]);
    if buttons.any_just_pressed([MouseButton::Left, MouseButton::Right, MouseButton::Middle]) {
        *active = hovered;
    } else if !dragging {
        *active = None;
    }
    let target = if dragging { *active } else { hovered };
    let Some(pane) = target else { return };
    let Some((mut cam, projection)) = cameras.iter_mut().find(|(c, _)| c.pane == pane) else {
        return;
    };
    let height = rects[pane].height().max(1.0);
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    let zoom = 0.9f32.powf(lines);
    let rig = &mut cam.rig;
    match projection {
        Projection::Orthographic(_) => {
            rig.ortho_height = (rig.ortho_height * zoom).clamp(0.05, 500.0);
            if dragging {
                let d = motion.delta * rig.ortho_height / height;
                rig.target += Vec3::new(-d.x, d.y, 0.0);
            }
        }
        _ => {
            rig.distance = (rig.distance * zoom).clamp(0.5, 500.0);
            if buttons.pressed(MouseButton::Left) {
                rig.yaw -= motion.delta.x * 0.008;
                rig.pitch = (rig.pitch + motion.delta.y * 0.008).clamp(-1.5, 1.5);
            } else if dragging {
                let right = Vec3::new(-rig.yaw.sin(), rig.yaw.cos(), 0.0);
                let up = Vec3::Z;
                let k = rig.distance / height;
                rig.target += (-motion.delta.x * right + motion.delta.y * up) * k;
            }
        }
    }
}

/// Shade each 2-D pane by predicted class, alpha proportional to confidence.
fn update_regions(
    experiment: Res<Experiment>,
    views: Res<PaneViews>,
    settings: Res<SceneSettings>,
    mut images: ResMut<Assets<Image>>,
    mut quads: Query<(&mut RegionQuad, &mut Visibility)>,
) {
    let (lo, hi) = experiment.bounds;
    for (mut quad, mut visibility) in &mut quads {
        *visibility = if settings.show_regions {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let Some(view) = views.0.get(quad.pane) else {
            continue;
        };
        if !settings.show_regions || quad.drawn == Some(view.shown) {
            continue;
        }
        quad.drawn = Some(view.shown);
        let Some(mut image) = images.get_mut(&quad.image) else {
            continue;
        };
        let logistic =
            experiment.panes[quad.pane].trainer.config().model == ModelKind::LogisticRegression;
        let pixels = region_pixels(view.shown, lo.truncate(), hi.truncate(), logistic);
        image.data = Some(pixels);
    }
}

fn region_pixels(boundary: Option<Boundary>, lo: Vec2, hi: Vec2, logistic: bool) -> Vec<u8> {
    let n = REGION_RESOLUTION as usize;
    let mut px = vec![0u8; n * n * 4];
    let Some(b) = boundary else { return px };
    let colors = [CLASS_COLORS[0].to_srgba(), CLASS_COLORS[1].to_srgba()];
    for row in 0..n {
        // Image row 0 is the top edge of the quad.
        let y = hi.y - (row as f32 + 0.5) / n as f32 * (hi.y - lo.y);
        for col in 0..n {
            let x = lo.x + (col as f32 + 0.5) / n as f32 * (hi.x - lo.x);
            let f = b.eval(Vec3::new(x, y, 0.0));
            let confidence = if logistic {
                (2.0 * bevaru_core::loss::sigmoid(f as f64) - 1.0).abs() as f32
            } else {
                f.abs().tanh()
            };
            let c = if f >= 0.0 { colors[0] } else { colors[1] };
            let i = (row * n + col) * 4;
            px[i] = (c.red * 255.0) as u8;
            px[i + 1] = (c.green * 255.0) as u8;
            px[i + 2] = (c.blue * 255.0) as u8;
            px[i + 3] = ((0.08 + 0.27 * confidence) * 255.0) as u8;
        }
    }
    px
}

/// Rebuild each 3-D pane's decision-plane polygon from its shown boundary.
fn update_planes(
    experiment: Res<Experiment>,
    views: Res<PaneViews>,
    mut meshes: ResMut<Assets<Mesh>>,
    planes: Query<&DecisionPlane>,
) {
    let (lo, hi) = experiment.bounds;
    for plane in &planes {
        let polygon = views
            .0
            .get(plane.pane)
            .and_then(|v| v.shown)
            .map(|b| b.clip_to_box(0.0, lo, hi))
            .unwrap_or_default();
        let Some(mut mesh) = meshes.get_mut(&plane.mesh) else {
            continue;
        };
        let normal = views
            .0
            .get(plane.pane)
            .and_then(|v| v.shown)
            .map_or(Vec3::Z, |b| b.normal);
        let indices: Vec<u32> = (1..polygon.len().saturating_sub(1) as u32)
            .flat_map(|i| [0, i, i + 1])
            .collect();
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            vec![normal.to_array(); polygon.len()],
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            polygon.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        );
        mesh.insert_indices(Indices::U32(indices));
    }
}

fn draw_overlays(
    experiment: Res<Experiment>,
    views: Res<PaneViews>,
    settings: Res<SceneSettings>,
    mut gizmos: Gizmos,
    mut margin_gizmos: Gizmos<MarginGizmos>,
) {
    let (lo, hi) = experiment.bounds;
    let is_3d = experiment.dims == 3;
    for (pane, view) in views.0.iter().enumerate().take(experiment.panes.len()) {
        let offset = pane_offset(pane);
        let is_svm = experiment.panes[pane].trainer.config().model == ModelKind::Svm;

        if settings.show_bounds {
            if is_3d {
                gizmos.cube(
                    Transform::from_translation(offset + (lo + hi) / 2.0).with_scale(hi - lo),
                    BOUNDS_COLOR,
                );
            } else {
                let c = offset + ((lo + hi) / 2.0).with_z(-0.5);
                gizmos.rect(
                    Isometry3d::from_translation(c),
                    (hi - lo).truncate(),
                    BOUNDS_COLOR,
                );
            }
        }

        if let Some(b) = view.shown {
            let levels: &[f32] = if is_svm && settings.show_margins {
                &[0.0, 1.0, -1.0]
            } else {
                &[0.0]
            };
            for &level in levels {
                if is_3d {
                    let poly = b.clip_to_box(level, lo, hi);
                    let strip = poly.iter().chain(poly.first()).map(|p| offset + *p);
                    if level == 0.0 {
                        gizmos.linestrip(strip, BOUNDARY_COLOR);
                    } else {
                        margin_gizmos.linestrip(strip, MARGIN_COLOR);
                    }
                } else if let Some((p, q)) = b.clip_to_rect(level, lo.truncate(), hi.truncate()) {
                    let (p, q) = (offset + p.extend(0.02), offset + q.extend(0.02));
                    if level == 0.0 {
                        gizmos.line(p, q, BOUNDARY_COLOR);
                    } else {
                        margin_gizmos.line(p, q, MARGIN_COLOR);
                    }
                }
            }
        }

        if is_svm && settings.show_support_vectors {
            for &i in &view.support_vectors {
                let p = offset + experiment.points[i] + Vec3::Z * 0.03;
                if is_3d {
                    gizmos.sphere(p, MARKER_RADIUS * 2.0, SUPPORT_COLOR);
                } else {
                    gizmos.circle(p, MARKER_RADIUS * 1.9, SUPPORT_COLOR);
                }
            }
        }

        if settings.show_residuals
            && let Some(model) = &view.model
        {
            for i in 0..experiment.points.len() {
                if let Some(fit) = experiment.fitted_point(model, i) {
                    gizmos.line(offset + experiment.points[i], offset + fit, RESIDUAL_COLOR);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panes_split_the_space_between_panels() {
        let rects = pane_rects(
            Vec2::new(1000.0, 600.0),
            UiInsets {
                left: 200.0,
                right: 200.0,
            },
            2,
        );
        assert_eq!(rects[0], Rect::new(200.0, 0.0, 500.0, 600.0));
        assert_eq!(rects[1], Rect::new(500.0, 0.0, 800.0, 600.0));
    }

    #[test]
    fn regions_shade_by_side_and_confidence() {
        // Boundary x = 0: right half positive (class 0, blue), left negative.
        let b = Boundary::from_linear(Vec3::X, 0.0);
        let px = region_pixels(b, Vec2::splat(-5.0), Vec2::splat(5.0), false);
        let n = REGION_RESOLUTION as usize;
        let at = |row: usize, col: usize| &px[(row * n + col) * 4..(row * n + col) * 4 + 4];
        let blue = CLASS_COLORS[0].to_srgba();
        assert_eq!(at(n / 2, n - 1)[2], (blue.blue * 255.0) as u8);
        assert_ne!(at(n / 2, 0)[0], at(n / 2, n - 1)[0]);
        // Further from the boundary is more opaque.
        assert!(at(n / 2, n - 1)[3] > at(n / 2, n / 2)[3]);
        assert!(
            region_pixels(None, Vec2::ZERO, Vec2::ONE, false)
                .iter()
                .all(|&v| v == 0)
        );
    }
}
