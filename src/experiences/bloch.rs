//! "Bloch sphere: data encodings": one Bloch sphere per qubit, and the common
//! ways to encode classical data into qubits, animated as rotations.
//!
//! Two modes:
//! - **Gate by gate**: the encoding's circuit runs one gate at a time, each
//!   applied continuously as exp(−i·s·G), s from 0 to 1. The state's trail is
//!   drawn, and for a single-qubit gate its rotation axis and the circle the
//!   state turns on.
//! - **Sweep the data**: one feature sweeps its range and the encoded state
//!   follows, leaving the encoding's locus behind: a meridian for angle
//!   encoding, the equator for phase encoding, and so on.
//!
//! The math is in [`bevaru_core::quantum`]; this module only draws it.

use bevaru_core::quantum::{Bloch, Encoding, GatePath, gate_path, locus};
use bevy::gizmos::config::{GizmoConfigGroup, GizmoConfigStore, GizmoLineStyle};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use super::{ExperienceEntity, ExperienceStarted, ExperienceStopped, in_experience};
use crate::capture::HideOverlays;
use crate::formula::{Formula, FormulaTextures};
use crate::orbit::{OrbitPlugin, OrbitRig, OrbitView};

pub const ID: &str = "bloch-sphere";

/// Samples per gate along a gate path.
pub const SAMPLES_PER_GATE: usize = 48;
/// Samples along a sweep's locus.
pub const LOCUS_SAMPLES: usize = 240;
/// Seconds per gate, and per full sweep, at speed 1.
pub const SECONDS_PER_GATE: f32 = 1.4;
pub const SECONDS_PER_SWEEP: f32 = 5.0;
/// Seconds the finished state is held before the animation loops.
pub const HOLD_SECONDS: f32 = 1.2;
/// Trails and loci are drawn this far out (as a fraction of the radius), so
/// they sit just above the sphere's surface instead of depth-fighting it.
const PATH_LIFT: f32 = 1.012;
/// Distance between neighbouring spheres' centres.
const SPACING: f32 = 2.9;

const ARROW_COLORS: [Color; 2] = [Color::srgb(0.85, 0.33, 0.10), Color::srgb(0.10, 0.50, 0.70)];
const GUIDE: Color = Color::srgba(0.35, 0.38, 0.45, 0.55);
const AXIS_COLORS: [Color; 3] = [
    Color::srgb(0.80, 0.20, 0.20),
    Color::srgb(0.20, 0.60, 0.25),
    Color::srgb(0.20, 0.35, 0.85),
];
const ROTATION: Color = Color::srgb(0.55, 0.20, 0.70);

/// Guides: the equator, meridians, and axes.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct BlochGuideGizmos;

/// Trails and loci.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct BlochPathGizmos;

/// The current gate's rotation axis, dashed.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct BlochAxisGizmos;

pub struct BlochExperiencePlugin;

impl Plugin for BlochExperiencePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<OrbitPlugin>() {
            app.add_plugins(OrbitPlugin);
        }
        app.init_gizmo_group::<BlochGuideGizmos>()
            .init_gizmo_group::<BlochPathGizmos>()
            .init_gizmo_group::<BlochAxisGizmos>()
            .add_observer(start)
            .add_observer(stop)
            .add_systems(
                Update,
                (sync_config, advance, place_arrows, draw)
                    .chain()
                    .run_if(in_experience(ID)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (controls, labels).chain().run_if(in_experience(ID)),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Run the circuit one gate at a time.
    Gates,
    /// Sweep one feature across its range.
    Sweep,
}

/// What the student has set.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct BlochControls {
    pub encoding: Encoding,
    /// One value per feature of `encoding`.
    pub features: Vec<f64>,
    pub mode: Mode,
    /// The feature swept in [`Mode::Sweep`].
    pub sweep_feature: usize,
    pub playing: bool,
    pub speed: f32,
    pub trail: bool,
    pub labels: bool,
}

impl Default for BlochControls {
    fn default() -> Self {
        let encoding = Encoding::AngleRy;
        Self {
            encoding,
            features: encoding.defaults(),
            mode: Mode::Gates,
            sweep_feature: 0,
            playing: true,
            speed: 1.0,
            trail: true,
            labels: true,
        }
    }
}

impl BlochControls {
    /// The settings that determine the paths; a change restarts the animation.
    fn config(&self) -> (Encoding, Vec<f64>, Mode, usize) {
        (
            self.encoding,
            self.features.clone(),
            self.mode,
            self.sweep_feature,
        )
    }
}

/// The entities drawing one qubit.
#[derive(Debug, Clone, Copy)]
struct QubitEntities {
    shaft: Entity,
    tip: Entity,
    dot: Entity,
}

/// Marks a qubit's sphere.
#[derive(Component, Debug, Clone, Copy)]
pub struct BlochQubit(pub usize);

/// Shared meshes and materials; dropped when the experience stops.
#[derive(Resource, Debug)]
pub struct BlochAssets {
    sphere: Handle<Mesh>,
    shaft: Handle<Mesh>,
    tip: Handle<Mesh>,
    dot: Handle<Mesh>,
    glass: Handle<StandardMaterial>,
    arrows: [Handle<StandardMaterial>; 2],
}

/// The paths for the current settings, and where the animation is.
#[derive(Resource, Debug)]
pub struct BlochScene {
    shown: (Encoding, Vec<f64>, Mode, usize),
    pub path: GatePath,
    /// `locus[q]`: qubit q's encoded state as the swept feature moves.
    pub locus: Vec<Vec<Bloch>>,
    /// Position along the active path, in samples (0 = start).
    pub progress: f32,
    hold: f32,
    qubits: Vec<QubitEntities>,
}

impl BlochScene {
    fn compute(controls: &BlochControls) -> (GatePath, Vec<Vec<Bloch>>) {
        let e = controls.encoding;
        let path = gate_path(e, &controls.features, SAMPLES_PER_GATE);
        let feature = controls.sweep_feature.min(e.features().len() - 1);
        let locus = locus(e, &controls.features, feature, LOCUS_SAMPLES);
        (path, locus)
    }

    /// The path the animation follows in `mode`, per qubit.
    pub fn active(&self) -> &[Vec<Bloch>] {
        match self.shown.2 {
            Mode::Gates => &self.path.points,
            Mode::Sweep => &self.locus,
        }
    }

    /// The last valid `progress`.
    pub fn end(&self) -> f32 {
        (self.active()[0].len() - 1) as f32
    }

    pub fn at_end(&self) -> bool {
        self.progress >= self.end()
    }

    /// Each qubit's Bloch vector at the current progress, interpolated.
    pub fn current(&self) -> Vec<Bloch> {
        self.active()
            .iter()
            .map(|pts| sample(pts, self.progress))
            .collect()
    }

    /// The gate running at the current progress, in gate mode.
    pub fn current_gate(&self) -> Option<usize> {
        if self.shown.2 != Mode::Gates || self.path.gates.is_empty() {
            return None;
        }
        let s = self.path.samples_per_gate as f32;
        Some(((self.progress.max(1e-3) - 1e-3) / s).floor() as usize)
            .map(|g| g.min(self.path.gates.len() - 1))
    }

    /// The swept feature's value at the current progress, in sweep mode.
    pub fn swept_value(&self) -> Option<f64> {
        if self.shown.2 != Mode::Sweep {
            return None;
        }
        let f = self.shown.0.features()[self.shown.3];
        Some(f.min + (f.max - f.min) * (self.progress / self.end()) as f64)
    }
}

fn sample(points: &[Bloch], at: f32) -> Bloch {
    let at = at.clamp(0.0, (points.len() - 1) as f32);
    let i = at.floor() as usize;
    let j = (i + 1).min(points.len() - 1);
    let t = (at - i as f32) as f64;
    let (a, b) = (points[i], points[j]);
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
}

fn v(b: Bloch) -> Vec3 {
    Vec3::new(b[0] as f32, b[1] as f32, b[2] as f32)
}

/// A three-quarter view with |0⟩ up, framing `n` spheres.
pub fn home_view(n: usize) -> OrbitView {
    OrbitView {
        yaw: 0.55,
        pitch: 0.38,
        distance: 4.6 + 2.6 * (n as f32 - 1.0),
        target: Vec3::ZERO,
    }
}

/// Where qubit `q` of `n` sits: side by side across the screen at the home view.
pub fn sphere_centre(q: usize, n: usize) -> Vec3 {
    let yaw = home_view(n).yaw;
    let across = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
    across * SPACING * (q as f32 - (n as f32 - 1.0) / 2.0)
}

fn start(
    started: On<ExperienceStarted>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmos: Option<ResMut<GizmoConfigStore>>,
    capturing: Option<Res<HideOverlays>>,
) {
    if started.id != ID {
        return;
    }
    // Load the typesetting engine off the main thread, so the first formula
    // doesn't stall a frame.
    bevy::tasks::AsyncComputeTaskPool::get()
        .spawn(async { crate::formula::warm_up() })
        .detach();
    if let Some(store) = gizmos.as_deref_mut() {
        store.config_mut::<BlochGuideGizmos>().0.line.width = 1.5;
        store.config_mut::<BlochPathGizmos>().0.line.width = 5.0;
        let (axis, _) = store.config_mut::<BlochAxisGizmos>();
        axis.line.width = 1.5;
        axis.line.style = GizmoLineStyle::Dashed {
            gap_scale: 2.0,
            line_scale: 4.0,
        };
    }
    let assets = BlochAssets {
        sphere: meshes.add(Sphere::new(1.0).mesh().uv(48, 24)),
        shaft: meshes.add(Cylinder::new(0.028, 1.0)),
        tip: meshes.add(Cone {
            radius: 0.075,
            height: 0.2,
        }),
        dot: meshes.add(Sphere::new(0.05).mesh().uv(16, 8)),
        glass: materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.68, 0.90, 0.13),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.3,
            ..default()
        }),
        arrows: ARROW_COLORS.map(|c| {
            materials.add(StandardMaterial {
                base_color: c,
                perceptual_roughness: 0.5,
                ..default()
            })
        }),
    };
    // A thumbnail capture (overlays hidden from the start) gets a still
    // showcase: dense angle encoding partway through its second rotation,
    // with the axis, the circle it turns on, and the trail all visible.
    let showcase = capturing.is_some();
    let controls = if showcase {
        let encoding = Encoding::DenseAngle;
        BlochControls {
            encoding,
            features: vec![2.0, 2.2],
            playing: false,
            ..default()
        }
    } else {
        BlochControls::default()
    };
    let (path, locus) = BlochScene::compute(&controls);
    let n = controls.encoding.n_qubits();
    let qubits = (0..n)
        .map(|q| spawn_qubit(&mut commands, &assets, q, n))
        .collect();
    for (direction, illuminance) in [
        (Vec3::new(-0.6, -0.4, -1.0), 6000.0),
        (Vec3::new(0.7, 0.5, -0.3), 1800.0),
    ] {
        commands.spawn((
            ExperienceEntity,
            DirectionalLight {
                illuminance,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::default().looking_to(direction, Vec3::Z),
        ));
    }
    commands.spawn((
        ExperienceEntity,
        OrbitRig::bundle(home_view(n)),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::WHITE),
            ..default()
        },
        AmbientLight {
            brightness: 600.0,
            ..default()
        },
    ));
    commands.insert_resource(BlochScene {
        shown: controls.config(),
        path,
        locus,
        progress: if showcase {
            1.6 * SAMPLES_PER_GATE as f32
        } else {
            0.0
        },
        hold: 0.0,
        qubits,
    });
    commands.insert_resource(controls);
    commands.insert_resource(assets);
}

fn spawn_qubit(commands: &mut Commands, assets: &BlochAssets, q: usize, n: usize) -> QubitEntities {
    let centre = sphere_centre(q, n);
    commands.spawn((
        ExperienceEntity,
        BlochQubit(q),
        Mesh3d(assets.sphere.clone()),
        MeshMaterial3d(assets.glass.clone()),
        Transform::from_translation(centre),
        bevy::light::NotShadowCaster,
    ));
    let arrow = assets.arrows[q % 2].clone();
    let mut part = |mesh: &Handle<Mesh>| {
        commands
            .spawn((
                ExperienceEntity,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(arrow.clone()),
                Transform::from_translation(centre),
            ))
            .id()
    };
    QubitEntities {
        shaft: part(&assets.shaft),
        tip: part(&assets.tip),
        dot: part(&assets.dot),
    }
}

fn stop(stopped: On<ExperienceStopped>, mut commands: Commands) {
    if stopped.id != ID {
        return;
    }
    commands.remove_resource::<BlochControls>();
    commands.remove_resource::<BlochScene>();
    commands.remove_resource::<BlochAssets>();
}

/// Recompute the paths when the settings change, and restart from |0…0⟩.
fn sync_config(
    mut commands: Commands,
    controls: Option<ResMut<BlochControls>>,
    scene: Option<ResMut<BlochScene>>,
    assets: Option<Res<BlochAssets>>,
    spheres: Query<Entity, With<BlochQubit>>,
    mut rigs: Query<&mut OrbitRig, With<ExperienceEntity>>,
) {
    let (Some(mut controls), Some(mut scene), Some(assets)) = (controls, scene, assets) else {
        return;
    };
    let e = controls.encoding;
    if controls.features.len() != e.features().len() {
        controls.features = e.defaults();
    }
    if controls.sweep_feature >= e.features().len() {
        controls.sweep_feature = 0;
    }
    if scene.shown == controls.config() {
        return;
    }
    let n_before = scene.shown.0.n_qubits();
    let (path, locus) = BlochScene::compute(&controls);
    scene.path = path;
    scene.locus = locus;
    scene.shown = controls.config();
    scene.progress = 0.0;
    scene.hold = 0.0;
    let n = e.n_qubits();
    if n != n_before {
        for q in std::mem::take(&mut scene.qubits) {
            for entity in [q.shaft, q.tip, q.dot] {
                commands.entity(entity).despawn();
            }
        }
        for s in &spheres {
            commands.entity(s).despawn();
        }
        scene.qubits = (0..n)
            .map(|q| spawn_qubit(&mut commands, &assets, q, n))
            .collect();
        for mut rig in &mut rigs {
            rig.home = home_view(n);
            rig.view = rig.home;
        }
    }
}

/// Move along the active path; hold at the end, then loop.
fn advance(
    time: Res<Time>,
    controls: Option<Res<BlochControls>>,
    scene: Option<ResMut<BlochScene>>,
) {
    let (Some(controls), Some(mut scene)) = (controls, scene) else {
        return;
    };
    if !controls.playing {
        return;
    }
    let dt = time.delta_secs() * controls.speed;
    if scene.at_end() {
        scene.hold += dt;
        if scene.hold >= HOLD_SECONDS {
            scene.progress = 0.0;
            scene.hold = 0.0;
        }
        return;
    }
    let rate = match scene.shown.2 {
        Mode::Gates => SAMPLES_PER_GATE as f32 / SECONDS_PER_GATE,
        Mode::Sweep => LOCUS_SAMPLES as f32 / SECONDS_PER_SWEEP,
    };
    scene.progress = (scene.progress + dt * rate).min(scene.end());
}

/// Point each qubit's arrow at its current Bloch vector. A vector too short
/// to draw as an arrow (a maximally entangled qubit) shows as a dot.
fn place_arrows(
    scene: Option<Res<BlochScene>>,
    mut transforms: Query<(&mut Transform, &mut Visibility)>,
) {
    let Some(scene) = scene else {
        return;
    };
    let n = scene.qubits.len();
    for (q, (r, ents)) in scene.current().into_iter().zip(&scene.qubits).enumerate() {
        let centre = sphere_centre(q, n);
        let r = v(r);
        let len = r.length();
        let tip_h = 0.2;
        let visible = len > tip_h;
        let dir = if len > 1e-6 { r / len } else { Vec3::Z };
        let rot = Quat::from_rotation_arc(Vec3::Y, dir);
        let shaft_len = (len - tip_h).max(1e-3);
        if let Ok((mut t, mut vis)) = transforms.get_mut(ents.shaft) {
            *t = Transform::from_translation(centre + dir * shaft_len / 2.0)
                .with_rotation(rot)
                .with_scale(Vec3::new(1.0, shaft_len, 1.0));
            *vis = if visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if let Ok((mut t, mut vis)) = transforms.get_mut(ents.tip) {
            *t = Transform::from_translation(centre + dir * (shaft_len + tip_h / 2.0))
                .with_rotation(rot);
            *vis = if visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if let Ok((mut t, _)) = transforms.get_mut(ents.dot) {
            *t = Transform::from_translation(centre + r);
        }
    }
}

fn draw(
    controls: Option<Res<BlochControls>>,
    scene: Option<Res<BlochScene>>,
    mut guides: Gizmos<BlochGuideGizmos>,
    mut paths: Gizmos<BlochPathGizmos>,
    mut axes: Gizmos<BlochAxisGizmos>,
) {
    let (Some(controls), Some(scene)) = (controls, scene) else {
        return;
    };
    let n = scene.qubits.len();
    let quarter = std::f32::consts::FRAC_PI_2;
    for q in 0..n {
        let c = sphere_centre(q, n);
        // Equator (xy), and the x–z and y–z meridians.
        for rot in [
            Quat::IDENTITY,
            Quat::from_rotation_x(quarter),
            Quat::from_rotation_y(quarter),
        ] {
            guides
                .circle(Isometry3d::new(c, rot), 1.0, GUIDE)
                .resolution(96);
        }
        for (axis, color) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().zip(AXIS_COLORS) {
            guides.line(c - axis * 1.25, c + axis * 1.25, color);
        }

        let color = ARROW_COLORS[q % 2];
        let pts = &scene.active()[q];
        let upto = (scene.progress.floor() as usize).min(pts.len() - 1);
        // Lift paths just off the sphere so they don't depth-fight its surface.
        let lift = |b: &Bloch| c + v(*b) * PATH_LIFT;
        let mut traced: Vec<Vec3> = pts[..=upto].iter().map(lift).collect();
        traced.push(lift(&sample(pts, scene.progress)));
        match scene.shown.2 {
            Mode::Gates => {
                if controls.trail {
                    // Bright trace first: the faint full path coincides with
                    // it, and whichever is drawn second loses the depth test.
                    paths.linestrip(traced, color);
                    paths.linestrip(pts.iter().map(lift), color.with_alpha(0.18));
                }
                // The running gate's rotation: its axis, and the circle the
                // state turns on (perpendicular to the axis, through the
                // state where the gate began).
                if let Some(g) = scene.current_gate()
                    && scene.progress > 0.0
                    && let Some((gq, axis, _)) = scene.path.gates[g].bloch_rotation()
                    && gq == q
                {
                    let axis = v(axis).normalize();
                    axes.line(c - axis * 1.4, c + axis * 1.4, ROTATION);
                    let start = v(pts[g * scene.path.samples_per_gate]);
                    let along = start.dot(axis);
                    let radius = (start - axis * along).length();
                    if radius > 1e-3 {
                        axes.circle(
                            Isometry3d::new(
                                c + axis * along,
                                Quat::from_rotation_arc(Vec3::Z, axis),
                            ),
                            radius,
                            ROTATION.with_alpha(0.4),
                        )
                        .resolution(96);
                    }
                }
            }
            Mode::Sweep => {
                if controls.trail {
                    // Bright trace first: the faint full path coincides with
                    // it, and whichever is drawn second loses the depth test.
                    paths.linestrip(traced, color);
                    paths.linestrip(pts.iter().map(lift), color.with_alpha(0.25));
                }
            }
        }
    }
}

/// An encoding's state formula, typeset (Typst math) with a plain-text fallback.
pub fn encoding_formula(e: Encoding) -> Formula {
    let typst = match e {
        Encoding::Basis => "$ |b angle.r = X^b |0 angle.r $",
        Encoding::AngleRy => "$ R_Y (x)|0 angle.r = cos(x/2)|0 angle.r + sin(x/2)|1 angle.r $",
        Encoding::AngleRx => "$ R_X (x)|0 angle.r = cos(x/2)|0 angle.r - i sin(x/2)|1 angle.r $",
        Encoding::Phase => "$ P(x) H|0 angle.r = (|0 angle.r + e^(i x)|1 angle.r) / sqrt(2) $",
        Encoding::DenseAngle => {
            "$ R_Z (x_2) R_Y (x_1)|0 angle.r = cos(x_1/2)|0 angle.r + e^(i x_2) sin(x_1/2)|1 angle.r $"
        }
        Encoding::Amplitude => "$ (x_1|0 angle.r + x_2|1 angle.r) / norm(bold(x)) $",
        Encoding::Reuploading => "$ [R_Z (x) R_Y (x)]^3 |0 angle.r $",
        Encoding::ZzFeatureMap => {
            "$ e^(-i(pi - x_1)(pi - x_2) Z times.circle Z) (R_Z (2x_1) times.circle R_Z (2x_2)) (H times.circle H)|00 angle.r $"
        }
    };
    Formula::new(typst, e.formula())
}

/// The six basis-state labels: direction, Typst, and plain text.
const KETS: [(Vec3, &str, &str); 6] = [
    (Vec3::Z, "$|0 angle.r$", "|0⟩"),
    (Vec3::NEG_Z, "$|1 angle.r$", "|1⟩"),
    (Vec3::X, "$|+ angle.r$", "|+⟩"),
    (Vec3::NEG_X, "$|- angle.r$", "|−⟩"),
    (Vec3::Y, "$|+i angle.r$", "|+i⟩"),
    (Vec3::NEG_Y, "$|-i angle.r$", "|−i⟩"),
];

fn fmt_bloch(b: Bloch) -> String {
    format!("({:+.2}, {:+.2}, {:+.2})", b[0], b[1], b[2])
}

fn controls(
    mut contexts: EguiContexts,
    controls: Option<ResMut<BlochControls>>,
    scene: Option<ResMut<BlochScene>>,
    hide: Option<Res<HideOverlays>>,
    mut rigs: Query<&mut OrbitRig, With<ExperienceEntity>>,
    mut textures: Local<FormulaTextures>,
) -> Result {
    let (Some(mut controls), Some(mut scene)) = (controls, scene) else {
        return Ok(());
    };
    if hide.is_some() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut c = controls.clone();
    let mut restart = false;
    egui::Window::new("Bloch sphere: data encodings")
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .resizable(false)
        .default_width(330.0)
        .show(ctx, |ui| {
            egui::ComboBox::from_label("encoding")
                .selected_text(c.encoding.name())
                .width(220.0)
                .show_ui(ui, |ui| {
                    for e in Encoding::ALL {
                        let label = if e.n_qubits() > 1 {
                            format!("{} ({} qubits)", e.name(), e.n_qubits())
                        } else {
                            e.name().to_string()
                        };
                        ui.selectable_value(&mut c.encoding, e, label);
                    }
                });
            if c.encoding != controls.encoding {
                c.features = c.encoding.defaults();
                c.sweep_feature = 0;
            }
            let formula = encoding_formula(c.encoding);
            match textures.get(ui.ctx(), &formula, ui.ctx().pixels_per_point()) {
                Ok(texture) => {
                    // Typeset at 20 pt; shrink to fit the panel if wider.
                    let mut size = texture.size_vec2() / ui.ctx().pixels_per_point();
                    let room = ui.available_width() - 12.0;
                    if size.x > room {
                        size *= room / size.x;
                    }
                    // Typst ink is dark: show it on a light card, like the
                    // perceptron's formula tooltips.
                    egui::Frame::new()
                        .fill(egui::Color32::WHITE)
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.image(egui::load::SizedTexture::new(texture.id(), size));
                        });
                }
                // Hack (egui's monospace font) has ⟩ and ⊗; the proportional font does not.
                Err(_) => {
                    ui.label(egui::RichText::new(&formula.text).monospace());
                }
            }
            ui.small(c.encoding.shape());
            ui.separator();
            let features = c.encoding.features();
            for (value, f) in c.features.iter_mut().zip(features) {
                ui.add(
                    egui::Slider::new(value, f.min..=f.max)
                        .text(f.name)
                        .fixed_decimals(2),
                );
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut c.mode, Mode::Gates, "Gate by gate");
                ui.selectable_value(&mut c.mode, Mode::Sweep, "Sweep the data");
            });
            if c.mode == Mode::Sweep && features.len() > 1 {
                ui.horizontal(|ui| {
                    ui.label("sweep");
                    for (i, f) in features.iter().enumerate() {
                        ui.selectable_value(&mut c.sweep_feature, i, f.name);
                    }
                });
            }
            ui.horizontal(|ui| {
                if ui
                    .button(if c.playing { "⏸ Pause" } else { "▶ Play" })
                    .clicked()
                {
                    c.playing = !c.playing;
                }
                if ui.button("⟲ Restart").clicked() {
                    restart = true;
                }
                ui.add(
                    egui::Slider::new(&mut c.speed, 0.25..=4.0)
                        .text("speed")
                        .logarithmic(true),
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut c.trail,
                    if c.mode == Mode::Gates {
                        "trail"
                    } else {
                        "locus"
                    },
                );
                ui.checkbox(&mut c.labels, "labels");
            });
            ui.separator();
            let now = match (scene.current_gate(), scene.swept_value()) {
                (Some(g), _) if scene.progress > 0.0 => format!(
                    "gate {} of {}: {}",
                    g + 1,
                    scene.path.gates.len(),
                    scene.path.gates[g].label
                ),
                (Some(_), _) => {
                    format!("start: |{}⟩", "0".repeat(c.encoding.n_qubits()))
                }
                (None, Some(x)) => format!(
                    "{} = {x:.2}",
                    features[c.sweep_feature.min(features.len() - 1)].name
                ),
                _ => String::new(),
            };
            ui.label(egui::RichText::new(now).strong().monospace());
            for (q, b) in scene.current().into_iter().enumerate() {
                let len = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
                let who = if c.encoding.n_qubits() > 1 {
                    format!("qubit {}: ", q + 1)
                } else {
                    String::new()
                };
                let entangled = if len < 0.99 {
                    "  (inside the sphere: entangled)"
                } else {
                    ""
                };
                ui.small(format!(
                    "{who}r = {}, |r| = {len:.2}{entangled}",
                    fmt_bloch(b)
                ));
            }
            ui.separator();
            ui.small("Left-drag to orbit; right-drag to pan; scroll to zoom.");
            if ui.button("Reset view (F)").clicked() {
                for mut rig in &mut rigs {
                    rig.reset = true;
                }
            }
        });
    if restart {
        scene.progress = 0.0;
        scene.hold = 0.0;
    }
    if c != *controls {
        *controls = c;
    }
    Ok(())
}

/// Basis-state labels at the ends of each sphere's axes, and qubit names.
fn labels(
    mut contexts: EguiContexts,
    controls: Option<Res<BlochControls>>,
    scene: Option<Res<BlochScene>>,
    cameras: Query<(&Camera, &GlobalTransform), (With<OrbitRig>, With<ExperienceEntity>)>,
    mut textures: Local<FormulaTextures>,
) -> Result {
    let (Some(controls), Some(scene)) = (controls, scene) else {
        return Ok(());
    };
    if !controls.labels {
        return Ok(());
    }
    let Some((camera, cam)) = cameras.iter().next() else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?.clone();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("bloch-labels"),
    ));
    let ink = egui::Color32::from_rgb(35, 35, 45);
    let n = scene.qubits.len();
    for q in 0..n {
        let c = sphere_centre(q, n);
        let ppp = ctx.pixels_per_point();
        let screen = |at: Vec3| {
            camera
                .world_to_viewport(cam, at)
                .ok()
                .map(|p| egui::pos2(p.x, p.y))
        };
        let put = |at: Vec3, text: &str, size: f32| {
            if let Some(p) = screen(at) {
                painter.text(
                    p,
                    egui::Align2::CENTER_CENTER,
                    text,
                    // Hack (egui's monospace font) has ⟩; the proportional font does not.
                    egui::FontId::monospace(size),
                    ink,
                );
            }
        };
        for (dir, typst, text) in KETS {
            let at = c + dir * 1.42;
            match textures.get(&ctx, &Formula::new(typst, text), ppp) {
                Ok(texture) => {
                    if let Some(p) = screen(at) {
                        let rect = egui::Rect::from_center_size(p, texture.size_vec2() / ppp * 0.9);
                        painter.image(
                            texture.id(),
                            rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                }
                Err(_) => put(at, text, 19.0),
            }
        }
        if n > 1 {
            put(c + Vec3::Z * 1.75, &format!("qubit {}", q + 1), 18.0);
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "math"))]
mod tests {
    use super::*;

    #[test]
    fn every_formula_and_label_typesets() {
        for e in Encoding::ALL {
            let f = encoding_formula(e);
            crate::formula::typeset(&f.typst, 20.0)
                .unwrap_or_else(|err| panic!("{}: {err}", e.id()));
        }
        for (_, typst, _) in KETS {
            crate::formula::typeset(typst, 20.0).unwrap_or_else(|err| panic!("{typst}: {err}"));
        }
    }
}
