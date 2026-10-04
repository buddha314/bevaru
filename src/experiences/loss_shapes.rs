//! "Loss shapes in 3D": every loss as a surface over two quantities students
//! already know, with its caption, its linked 2-D curve, and a probe.
//!
//! The rendering lives in [`crate::shape_view`]; this module is the custom
//! experience around it: the controls, off-thread re-sampling when they
//! change, and the 2-D loss-curve chart beside the surface.

use bevaru_core::shapes::{
    DEFAULT_RESOLUTION, ShapeError, ShapeFamily, ShapeParams, ShapeView, Slice, SurfaceGrid,
};
use bevaru_core::{LossParams, Task as LossTask};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, EguiTextureHandle, egui};

use super::layout::SideInsets;
use super::{ExperienceEntity, ExperienceStarted, ExperienceStopped, in_experience};
use crate::capture::HideOverlays;
use crate::charts::{LineChart, loss_curve_chart};
use crate::orbit::OrbitRig;
use crate::shape_view::{Probe, ShapeOverlays, ShapePlot, ShapeViewPlugin, spawn_shape_scene};

pub const ID: &str = "loss-shapes";

/// Size of the 2-D loss-curve chart beside the surface.
const CHART_SIZE: UVec2 = UVec2::new(420, 270);

pub struct LossShapesExperiencePlugin;

impl Plugin for LossShapesExperiencePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<ShapeViewPlugin>() {
            app.add_plugins(ShapeViewPlugin);
        }
        app.add_observer(start)
            .add_observer(stop)
            .add_systems(
                Update,
                (request_sample, collect_sample, request_chart, collect_chart)
                    .chain()
                    .run_if(in_experience(ID)),
            )
            .add_systems(EguiPrimaryContextPass, ui.run_if(in_experience(ID)));
    }
}

/// What the student has chosen. A change re-samples the surface.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ShapeControls {
    pub view: ShapeView,
    pub params: ShapeParams,
    pub resolution: usize,
}

impl Default for ShapeControls {
    fn default() -> Self {
        Self {
            view: ShapeView::DEFAULT,
            params: ShapeParams::default(),
            resolution: DEFAULT_RESOLUTION,
        }
    }
}

impl ShapeControls {
    fn sample(&self) -> Result<SurfaceGrid, ShapeError> {
        self.view.sample(&self.params, self.resolution)
    }
}

/// The surface being sampled off the main thread, and what is on screen.
#[derive(Resource, Default)]
pub struct ShapeSampling {
    pending: Option<(ShapeControls, Task<Result<SurfaceGrid, ShapeError>>)>,
    /// The controls the current [`ShapePlot`] (or `error`) reflects.
    pub shown: Option<ShapeControls>,
    pub error: Option<String>,
}

impl ShapeSampling {
    pub fn is_sampling(&self) -> bool {
        self.pending.is_some()
    }
}

/// The 2-D loss curve, rendered by ruviz off the main thread.
#[derive(Resource)]
pub struct ShapeChart {
    pub image: Handle<Image>,
    shown: Option<LineChart>,
    pending: Option<(LineChart, Task<Result<ruviz::core::Image, String>>)>,
}

fn blank_chart() -> Image {
    Image::new_fill(
        Extent3d {
            width: CHART_SIZE.x,
            height: CHART_SIZE.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn start(
    started: On<ExperienceStarted>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if started.id != ID {
        return;
    }
    let controls = ShapeControls::default();
    // The first surface is sampled here (milliseconds) so the experience
    // opens on it; later ones are sampled off the main thread.
    let plot = match controls.sample() {
        Ok(grid) => ShapePlot::from_grid(controls.view, controls.params, grid),
        Err(e) => {
            error!("bevaru: could not sample {}: {e}", controls.view.id());
            return;
        }
    };
    if let Err(e) = spawn_shape_scene(
        &mut commands,
        &mut meshes,
        &mut materials,
        &plot,
        ExperienceEntity,
    ) {
        error!(
            "bevaru: could not build the {} surface: {e}",
            plot.view.id()
        );
        return;
    }
    commands.insert_resource(plot);
    commands.insert_resource(ShapeSampling {
        shown: Some(controls.clone()),
        ..default()
    });
    commands.insert_resource(controls);
    commands.insert_resource(ShapeChart {
        image: images.add(blank_chart()),
        shown: None,
        pending: None,
    });
    commands.insert_resource(ShapeOverlays::default());
    commands.insert_resource(Probe::default());
}

fn stop(
    stopped: On<ExperienceStopped>,
    mut commands: Commands,
    chart: Option<Res<ShapeChart>>,
    mut images: ResMut<Assets<Image>>,
) {
    if stopped.id != ID {
        return;
    }
    if let Some(chart) = chart {
        images.remove(&chart.image);
    }
    commands.remove_resource::<ShapeChart>();
    commands.remove_resource::<ShapePlot>();
    commands.remove_resource::<ShapeSampling>();
    commands.remove_resource::<ShapeControls>();
    commands.insert_resource(Probe::default());
}

/// Start sampling when the controls differ from what is shown. One sample at
/// a time: while a slider is dragged, each finished sample starts the next,
/// so the surface follows the slider.
fn request_sample(controls: Res<ShapeControls>, mut sampling: ResMut<ShapeSampling>) {
    if sampling.pending.is_some() || sampling.shown.as_ref() == Some(&*controls) {
        return;
    }
    let wanted = controls.clone();
    let job = wanted.clone();
    sampling.pending = Some((
        wanted,
        AsyncComputeTaskPool::get().spawn(async move { job.sample() }),
    ));
}

fn collect_sample(mut sampling: ResMut<ShapeSampling>, mut plot: ResMut<ShapePlot>) {
    let Some((_, task)) = sampling.pending.as_mut() else {
        return;
    };
    let Some(result) = check_ready(task) else {
        return;
    };
    let (controls, _) = sampling.pending.take().expect("checked above");
    match result {
        Ok(grid) => {
            *plot = ShapePlot::from_grid(controls.view, controls.params, grid);
            sampling.error = None;
        }
        Err(e) => sampling.error = Some(e.to_string()),
    }
    sampling.shown = Some(controls);
}

/// The 2-D loss curve for a view: the chart its slice equals, or, for the
/// three-class views, the binary loss they reduce to.
pub fn view_chart(view: ShapeView, params: &LossParams) -> LineChart {
    let losses = view.losses();
    let task = losses
        .first()
        .map_or(LossTask::Classification, |l| l.task());
    let range = match task {
        LossTask::Regression => 3.0,
        LossTask::Classification => 4.0,
    };
    loss_curve_chart(task, &losses, params, &losses, range)
}

fn request_chart(plot: Res<ShapePlot>, mut chart: ResMut<ShapeChart>) {
    let wanted = view_chart(plot.view, &plot.params.loss);
    let busy = chart.pending.is_some();
    if busy || chart.shown.as_ref() == Some(&wanted) {
        return;
    }
    let job = wanted.clone();
    chart.pending = Some((
        wanted,
        AsyncComputeTaskPool::get().spawn(async move { job.render(CHART_SIZE) }),
    ));
}

fn collect_chart(mut chart: ResMut<ShapeChart>, mut images: ResMut<Assets<Image>>) {
    let Some((_, task)) = chart.pending.as_mut() else {
        return;
    };
    let Some(result) = check_ready(task) else {
        return;
    };
    let (drawn, _) = chart.pending.take().expect("checked above");
    match result {
        Ok(img) => {
            let new = Image::new(
                Extent3d {
                    width: img.width,
                    height: img.height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                img.pixels,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            if let Some(mut existing) = images.get_mut(&chart.image) {
                *existing = new;
            }
        }
        // Don't retry the same chart every frame.
        Err(e) => warn!("bevaru: loss-curve chart failed: {e}"),
    }
    chart.shown = Some(drawn);
}

// ---------------------------------------------------------------------------
// Controls

/// Families in catalogue order.
fn families() -> Vec<ShapeFamily> {
    let mut out: Vec<ShapeFamily> = Vec::new();
    for v in ShapeView::ALL {
        if !out.contains(&v.family()) {
            out.push(v.family());
        }
    }
    out
}

/// The view's name within its family, for the loss picker.
pub fn short_name(view: ShapeView) -> &'static str {
    use ShapeView::*;
    match view {
        PredictionMse => "MSE",
        PredictionMae => "MAE",
        PredictionHuber => "Huber",
        CrossEntropy => "Binary cross-entropy",
        TwoScoresHinge => "Hinge",
        TwoScoresSquaredHinge => "Squared hinge",
        TwoScoresLogistic => "Logistic (softmax)",
        TwoScoresZeroOne => "0-1",
        HuberDelta => "Huber over δ",
        HingeMargin => "Hinge over μ",
        SquaredHingeMargin => "Squared hinge over μ",
        ThreeClassProbabilities => "Cross-entropy on the triangle",
        ThreeClassSoftmax => "Softmax cross-entropy",
        ThreeClassHingeWestonWatkins => "Weston–Watkins hinge",
        ThreeClassHingeCrammerSinger => "Crammer–Singer hinge",
    }
}

/// Where the highlighted 2-D curve lies on the surface.
fn slice_note(view: ShapeView) -> &'static str {
    match view.slice() {
        Some(Slice::ZeroTruth) => {
            "Highlighted on the surface: the slice y = 0, where the prediction ŷ is the residual."
        }
        Some(Slice::ZeroOtherScore) => {
            "Highlighted on the surface: the slice z_other = 0, where z_correct is the margin."
        }
        Some(Slice::CertainTruth) => {
            "Highlighted on the surface: the slice p = 1, plotted here against the margin m = ln(q / (1 − q))."
        }
        Some(Slice::HyperparameterRow) => {
            "Highlighted on the surface: the row at the slider's value."
        }
        None => {
            "Three-class losses have no 2-D curve. This is the binary loss each reduces to when one rival scores far below."
        }
    }
}

/// Switch view, keeping the hyperparameters; the KL toggle only applies to
/// cross-entropy.
fn select(controls: &mut ShapeControls, view: ShapeView) {
    controls.view = view;
    if view != ShapeView::CrossEntropy {
        controls.params.entropy_removed = false;
    }
}

#[allow(clippy::too_many_arguments)]
fn ui(
    mut contexts: EguiContexts,
    mut controls: ResMut<ShapeControls>,
    sampling: Option<Res<ShapeSampling>>,
    mut overlays: ResMut<ShapeOverlays>,
    chart: Option<Res<ShapeChart>>,
    mut insets: ResMut<SideInsets>,
    mut rigs: Query<&mut OrbitRig, With<ExperienceEntity>>,
    hide: Option<Res<HideOverlays>>,
    mut texture: Local<Option<(AssetId<Image>, egui::TextureId)>>,
) -> Result {
    if hide.is_some() {
        // Thumbnail capture: the surface alone, filling the window.
        SideInsets::set(&mut insets, SideInsets::default());
        return Ok(());
    }
    let chart_id = chart.as_ref().map(|c| c.image.id());
    if texture.map(|(id, _)| id) != chart_id {
        if let Some((old, _)) = texture.take() {
            contexts.remove_image(old);
        }
        *texture = chart_id.map(|id| (id, contexts.add_image(EguiTextureHandle::Weak(id))));
    }
    let ctx = contexts.ctx_mut()?.clone();
    let mut root = egui::Ui::new(
        ctx.clone(),
        "loss-shapes-root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    let left = egui::Panel::left("loss-shapes-controls")
        .default_size(300.0)
        .show(&mut root, |ui| {
            // Room for the lobby's "◀ Lobby" button.
            ui.add_space(36.0);
            ui.heading("Loss shapes in 3D");
            ui.separator();
            let current = controls.view;
            egui::ComboBox::from_label("Family")
                .selected_text(current.family().name())
                .show_ui(ui, |ui| {
                    for family in families() {
                        if ui
                            .selectable_label(current.family() == family, family.name())
                            .clicked()
                            && current.family() != family
                            && let Some(first) =
                                ShapeView::ALL.into_iter().find(|v| v.family() == family)
                        {
                            select(&mut controls, first);
                        }
                    }
                });
            egui::ComboBox::from_label("Loss")
                .selected_text(short_name(current))
                .show_ui(ui, |ui| {
                    for view in ShapeView::ALL
                        .into_iter()
                        .filter(|v| v.family() == current.family())
                    {
                        if ui
                            .selectable_label(current == view, short_name(view))
                            .clicked()
                            && current != view
                        {
                            select(&mut controls, view);
                        }
                    }
                });

            let hyper = controls.view.hyperparameters();
            if hyper.contains(&"huber_delta") {
                let mut delta = controls.params.loss.huber_delta();
                if ui
                    .add(egui::Slider::new(&mut delta, 0.1..=3.0).text("Huber δ"))
                    .changed()
                    && let Ok(p) = controls.params.loss.with_huber_delta(delta)
                {
                    controls.params.loss = p;
                }
            }
            if hyper.contains(&"margin") {
                let mut margin = controls.params.loss.margin();
                if ui
                    .add(egui::Slider::new(&mut margin, 0.1..=3.0).text("margin μ"))
                    .changed()
                    && let Ok(p) = controls.params.loss.with_margin(margin)
                {
                    controls.params.loss = p;
                }
            }
            if controls.view == ShapeView::CrossEntropy {
                let mut kl = controls.params.entropy_removed;
                if ui
                    .checkbox(&mut kl, "Remove the entropy H(p) (KL divergence)")
                    .changed()
                {
                    controls.params.entropy_removed = kl;
                }
            }
            let mut resolution = controls.resolution;
            if ui
                .add(
                    egui::Slider::new(&mut resolution, 21..=161)
                        .step_by(2.0)
                        .text("resolution"),
                )
                .changed()
            {
                // Odd, so the diagonal and zero lines fall on samples.
                controls.resolution = resolution | 1;
            }

            ui.separator();
            ui.label("Show");
            let o = &mut *overlays;
            ui.checkbox(&mut o.slice, "2-D slice");
            ui.checkbox(&mut o.legend, "colour legend");
            ui.checkbox(&mut o.ticks, "axis ticks");
            ui.checkbox(&mut o.clip, "clip level");
            ui.checkbox(&mut o.probe, "probe (point at the surface)");

            ui.separator();
            if let Some(s) = &sampling {
                if s.is_sampling() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("sampling…");
                    });
                }
                if let Some(e) = &s.error {
                    ui.colored_label(egui::Color32::RED, e);
                }
            }
            ui.small("Left-drag to orbit; right-drag to pan; scroll to zoom.");
            if ui.button("Reset view (F)").clicked() {
                for mut rig in &mut rigs {
                    rig.reset = true;
                }
            }
        });

    let view = controls.view;
    let right = egui::Panel::right("loss-shapes-caption")
        .default_size(CHART_SIZE.x as f32 + 24.0)
        .show(&mut root, |ui| {
            ui.add_space(8.0);
            ui.heading(view.title());
            ui.add_space(4.0);
            ui.label(view.caption());
            ui.separator();
            ui.strong("The 2-D curve");
            if let Some((_, id)) = *texture {
                let width = ui.available_width().min(CHART_SIZE.x as f32);
                let size = egui::vec2(width, width * CHART_SIZE.y as f32 / CHART_SIZE.x as f32);
                ui.image(egui::load::SizedTexture::new(id, size));
            }
            ui.small(slice_note(view));
        });

    SideInsets::set(
        &mut insets,
        SideInsets {
            left: left.response.rect.width(),
            right: right.response.rect.width(),
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_and_view_is_reachable_from_the_controls() {
        let fams = families();
        assert_eq!(fams.len(), 5);
        for view in ShapeView::ALL {
            assert!(fams.contains(&view.family()));
            assert!(!short_name(view).is_empty());
            assert!(!slice_note(view).is_empty());
        }
    }

    #[test]
    fn leaving_cross_entropy_turns_kl_off() {
        let mut c = ShapeControls::default();
        c.params.entropy_removed = true;
        assert!(c.sample().is_ok());
        select(&mut c, ShapeView::TwoScoresHinge);
        assert!(!c.params.entropy_removed);
        assert!(c.sample().is_ok());
    }

    #[test]
    fn every_view_has_a_2d_chart_with_its_losses() {
        for view in ShapeView::ALL {
            let chart = view_chart(view, &LossParams::default());
            let names: Vec<&str> = chart.series.iter().map(|s| s.label.as_str()).collect();
            let expected: Vec<&str> = view.losses().iter().map(|l| l.name()).collect();
            assert_eq!(names, expected, "{}", view.id());
        }
    }
}
