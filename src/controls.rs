//! The egui control panel (data, models, playback, sweeps, display), the
//! chart panel, per-pane overlays, and keyboard shortcuts.

use std::collections::HashMap;

use bevaru_core::{LearningRate, LossKind, ModelKind, Status, Task, TrainerConfig};
use bevy::camera::CameraOutputMode;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_resource::BlendState;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle,
    PrimaryEguiContext, egui,
};

use crate::charts::{ChartSettings, Charts, WeightImages};
use crate::experiment::{
    DatasetChoice, Experiment, ExperimentLoader, ExperimentSpec, LoadExperiment, MAX_PANES,
    TrainSpace, View, fmt_num,
};
use crate::lobby::{AppScreen, LeaveExperience};
use crate::playback::{
    PaneViews, Playback, PlaybackCommand, Sweep, SweepCommand, SweepParam, SweepSpec,
};
use crate::scene::{CLASS_COLORS, FrameData, SceneSettings, UiInsets, pane_rects};

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        app.init_resource::<Draft>()
            .init_resource::<EguiImageTextures>()
            .add_message::<LeaveExperience>()
            .add_systems(Startup, spawn_ui_camera)
            .add_systems(EguiPrimaryContextPass, ui)
            .add_systems(
                Update,
                keyboard_shortcuts.run_if(resource_exists::<Experiment>),
            );
    }
}

/// egui gets its own full-window camera, drawn over the panes and seeing no
/// scene geometry. Left to itself, bevy_egui would attach to the first pane
/// camera and lay the UI out inside that pane's viewport.
fn spawn_ui_camera(mut commands: Commands, mut settings: ResMut<EguiGlobalSettings>) {
    settings.auto_create_primary_context = false;
    commands.spawn((
        PrimaryEguiContext,
        Camera2d,
        Camera {
            order: 100,
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        RenderLayers::none(),
    ));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataKind {
    SeparableBlobs,
    OverlappingBlobs,
    Regression,
    Iris,
    #[cfg(feature = "mnist")]
    Mnist,
}

impl DataKind {
    const ALL: &[DataKind] = &[
        DataKind::SeparableBlobs,
        DataKind::OverlappingBlobs,
        DataKind::Regression,
        DataKind::Iris,
        #[cfg(feature = "mnist")]
        DataKind::Mnist,
    ];

    fn name(self) -> &'static str {
        match self {
            DataKind::SeparableBlobs => "Blobs (separable)",
            DataKind::OverlappingBlobs => "Blobs (overlapping)",
            DataKind::Regression => "Regression with outliers",
            DataKind::Iris => "Iris",
            #[cfg(feature = "mnist")]
            DataKind::Mnist => "MNIST digits",
        }
    }

    fn feature_names(self) -> Vec<String> {
        match self {
            DataKind::SeparableBlobs | DataKind::OverlappingBlobs => vec!["x₁".into(), "x₂".into()],
            DataKind::Iris => bevaru_core::dataset::iris().feature_names,
            _ => Vec::new(),
        }
    }
}

const IRIS_CLASSES: [&str; 3] = ["setosa", "versicolor", "virginica"];

/// Unapplied edits to the data section.
#[derive(Resource, Debug, Clone)]
struct Draft {
    kind: DataKind,
    regression_dims: usize,
    outliers: f64,
    iris_positive: String,
    iris_negative: Option<String>,
    #[cfg_attr(not(feature = "mnist"), allow(dead_code))]
    mnist_digits: (u8, u8),
    #[cfg_attr(not(feature = "mnist"), allow(dead_code))]
    mnist_samples: usize,
    pca: bool,
    pca_dims: usize,
    features: Vec<usize>,
    train_space: TrainSpace,
    seed: u64,
    sweep: SweepSpec,
    synced_generation: u64,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            kind: DataKind::OverlappingBlobs,
            regression_dims: 1,
            outliers: 0.1,
            iris_positive: "versicolor".into(),
            iris_negative: Some("virginica".into()),
            mnist_digits: (3, 8),
            mnist_samples: 2000,
            pca: false,
            pca_dims: 2,
            features: vec![0, 1],
            train_space: TrainSpace::Full,
            seed: 1,
            sweep: SweepSpec::default(),
            synced_generation: 0,
        }
    }
}

impl Draft {
    fn sync_from(&mut self, spec: &ExperimentSpec) {
        match &spec.dataset {
            DatasetChoice::SeparableBlobs => self.kind = DataKind::SeparableBlobs,
            DatasetChoice::OverlappingBlobs => self.kind = DataKind::OverlappingBlobs,
            DatasetChoice::Regression { dims, outliers } => {
                self.kind = DataKind::Regression;
                self.regression_dims = *dims;
                self.outliers = *outliers;
            }
            DatasetChoice::Iris { positive, negative } => {
                self.kind = DataKind::Iris;
                self.iris_positive.clone_from(positive);
                self.iris_negative.clone_from(negative);
            }
            #[cfg(feature = "mnist")]
            DatasetChoice::Mnist {
                positive,
                negative,
                samples,
            } => {
                self.kind = DataKind::Mnist;
                self.mnist_digits = (*positive, *negative);
                self.mnist_samples = *samples;
            }
        }
        match &spec.view {
            View::Features(f) => {
                self.pca = false;
                self.features.clone_from(f);
            }
            View::Pca(k) => {
                self.pca = true;
                self.pca_dims = *k;
            }
        }
        self.train_space = spec.train_space;
        self.seed = spec.seed;
    }

    fn dataset(&self) -> DatasetChoice {
        match self.kind {
            DataKind::SeparableBlobs => DatasetChoice::SeparableBlobs,
            DataKind::OverlappingBlobs => DatasetChoice::OverlappingBlobs,
            DataKind::Regression => DatasetChoice::Regression {
                dims: self.regression_dims,
                outliers: self.outliers,
            },
            DataKind::Iris => DatasetChoice::Iris {
                positive: self.iris_positive.clone(),
                negative: self.iris_negative.clone(),
            },
            #[cfg(feature = "mnist")]
            DataKind::Mnist => DatasetChoice::Mnist {
                positive: self.mnist_digits.0,
                negative: self.mnist_digits.1,
                samples: self.mnist_samples,
            },
        }
    }

    /// The spec to load, keeping the current panes when the task is unchanged.
    fn spec(&self, current: Option<&ExperimentSpec>) -> ExperimentSpec {
        let dataset = self.dataset();
        let task = dataset.task();
        let panes = match current {
            Some(c) if c.dataset.task() == task => c.panes.clone(),
            _ => vec![default_config(task)],
        };
        let view = if self.pca {
            View::Pca(self.pca_dims)
        } else {
            View::Features(self.features.clone())
        };
        ExperimentSpec {
            view,
            train_space: self.train_space,
            seed: self.seed,
            panes,
            dataset,
        }
    }
}

fn default_config(task: Task) -> TrainerConfig {
    match task {
        Task::Classification => TrainerConfig::svm(1.0).expect("C = 1 is valid"),
        Task::Regression => TrainerConfig::regression(LossKind::Mse),
    }
}

/// A new configuration for `loss`, carrying over what still applies.
fn config_for_loss(old: &TrainerConfig, loss: LossKind) -> TrainerConfig {
    let model = ModelKind::for_loss(loss).unwrap_or(old.model);
    let base = match model {
        ModelKind::Svm => {
            TrainerConfig::svm(old.c().unwrap_or(1.0)).expect("C from a valid config")
        }
        ModelKind::LogisticRegression => TrainerConfig::logistic(),
        ModelKind::LinearRegression => TrainerConfig::regression(loss),
    };
    TrainerConfig {
        loss,
        loss_params: old.loss_params,
        max_steps: old.max_steps,
        batch_size: old.batch_size,
        seed: old.seed,
        ..base
    }
}

#[derive(SystemParam)]
struct Writers<'w> {
    playback: MessageWriter<'w, PlaybackCommand>,
    sweep: MessageWriter<'w, SweepCommand>,
    load: MessageWriter<'w, LoadExperiment>,
    frame: MessageWriter<'w, FrameData>,
    leave: MessageWriter<'w, LeaveExperience>,
}

#[derive(SystemParam)]
struct Settings<'w> {
    scene: ResMut<'w, SceneSettings>,
    charts: ResMut<'w, ChartSettings>,
    insets: ResMut<'w, UiInsets>,
    playback: ResMut<'w, Playback>,
    sweep: ResMut<'w, Sweep>,
}

#[derive(SystemParam)]
struct Views<'w> {
    panes: Res<'w, PaneViews>,
    charts: Res<'w, Charts>,
    weights: Res<'w, WeightImages>,
    loader: Res<'w, ExperimentLoader>,
    images: Res<'w, Assets<Image>>,
    /// Present when the app has a lobby.
    screen: Option<Res<'w, State<AppScreen>>>,
    hide_overlays: Option<Res<'w, crate::capture::HideOverlays>>,
}

/// egui textures registered for Bevy images (charts, weight images). Images
/// are registered weakly, so their memory is freed with the asset; this map
/// and bevy_egui's registration are released here when the image goes away,
/// so they don't accumulate across experiments.
#[derive(Resource, Default)]
pub struct EguiImageTextures(HashMap<AssetId<Image>, egui::TextureId>);

impl EguiImageTextures {
    fn get_or_add(
        &mut self,
        id: AssetId<Image>,
        add: impl FnOnce() -> egui::TextureId,
    ) -> egui::TextureId {
        *self.0.entry(id).or_insert_with(add)
    }

    /// Forget images that no longer exist; returns their ids for release.
    fn take_stale(&mut self, exists: impl Fn(AssetId<Image>) -> bool) -> Vec<AssetId<Image>> {
        let stale: Vec<_> = self.0.keys().copied().filter(|id| !exists(*id)).collect();
        for id in &stale {
            self.0.remove(id);
        }
        stale
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn ui(
    mut contexts: EguiContexts,
    experiment: Option<ResMut<Experiment>>,
    mut draft: ResMut<Draft>,
    mut settings: Settings,
    views: Views,
    mut out: Writers,
    window: Single<&Window, With<PrimaryWindow>>,
    mut textures: ResMut<EguiImageTextures>,
    mut pane_error: Local<Option<String>>,
) -> Result {
    for id in textures.take_stale(|id| views.images.contains(id)) {
        contexts.remove_image(id);
    }
    // With a lobby, the panels belong to running experiment experiences only;
    // the lobby, loading screen, and custom experiences draw their own UI.
    if views
        .screen
        .as_ref()
        .is_some_and(|s| *s.get() != AppScreen::Running)
        || (views.screen.is_some() && experiment.is_none())
    {
        *settings.insets = UiInsets::default();
        return Ok(());
    }
    let mut texture = |h: &Handle<Image>| {
        textures.get_or_add(h.id(), || {
            contexts.add_image(EguiTextureHandle::Weak(h.id()))
        })
    };
    let chart_tex = [
        (
            views.charts.classification.is_ready(),
            texture(&views.charts.classification.image),
        ),
        (
            views.charts.regression.is_ready(),
            texture(&views.charts.regression.image),
        ),
        (
            views.charts.training.is_ready(),
            texture(&views.charts.training.image),
        ),
    ];
    let weight_tex: Vec<egui::TextureId> = views.weights.images.iter().map(&mut texture).collect();
    let ctx = contexts.ctx_mut()?.clone();

    let mut experiment = experiment;
    if let Some(e) = experiment.as_deref()
        && draft.synced_generation != e.generation
    {
        draft.synced_generation = e.generation;
        draft.sync_from(&e.spec);
    }

    let mut root = egui::Ui::new(
        ctx.clone(),
        "bevaru-root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    let left = egui::Panel::left("bevaru-controls")
        .resizable(true)
        .default_size(310.0)
        .min_size(280.0)
        .show(&mut root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    if views.screen.is_some() && ui.button("◀ Lobby").on_hover_text("Esc").clicked()
                    {
                        out.leave.write(LeaveExperience);
                    }
                    ui.heading("bevaru");
                });
                if let Some(label) = &views.loader.loading {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Loading {label}…"));
                    });
                }
                if let Some(err) = &views.loader.error {
                    ui.colored_label(egui::Color32::from_rgb(200, 40, 40), err);
                }
                data_section(ui, &mut draft, experiment.as_deref(), &mut out);
                if let Some(e) = experiment.as_deref_mut() {
                    models_section(ui, e, &mut pane_error);
                    playback_section(
                        ui,
                        e,
                        &mut settings.playback,
                        &views.panes,
                        settings.sweep.active,
                        &mut out,
                    );
                    sweep_section(ui, e, &mut draft, &mut settings.sweep, &mut out);
                }
                display_section(ui, &mut settings.scene, &mut settings.playback, &mut out);
            });
        })
        .response
        .rect
        .width();

    let right = egui::Panel::right("bevaru-charts")
        .resizable(true)
        .default_size(420.0)
        .min_size(320.0)
        .show(&mut root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                charts_section(ui, &mut settings.charts, experiment.as_deref(), &chart_tex);
                if let Some(e) = experiment.as_deref() {
                    weights_section(ui, e, &views.weights, &weight_tex);
                }
            });
        })
        .response
        .rect
        .width();

    settings.insets.left = left;
    settings.insets.right = right;

    if let Some(e) = experiment
        .as_deref()
        .filter(|_| views.hide_overlays.is_none())
    {
        pane_overlays(
            &ctx,
            e,
            &views.panes,
            &settings.sweep,
            window.size(),
            *settings.insets,
        );
    }
    Ok(())
}

fn data_section(
    ui: &mut egui::Ui,
    draft: &mut Draft,
    experiment: Option<&Experiment>,
    out: &mut Writers,
) {
    egui::CollapsingHeader::new("Data").default_open(true).show(ui, |ui| {
        let before = draft.kind;
        egui::ComboBox::from_label("Dataset").selected_text(draft.kind.name()).show_ui(ui, |ui| {
            for &k in DataKind::ALL {
                ui.selectable_value(&mut draft.kind, k, k.name());
            }
        });
        if draft.kind != before {
            draft.pca = false;
            draft.features = vec![0, 1];
            draft.train_space = TrainSpace::Full;
            match draft.kind {
                DataKind::Iris => draft.features = vec![2, 3],
                #[cfg(feature = "mnist")]
                DataKind::Mnist => draft.pca = true,
                _ => {}
            }
        }
        match draft.kind {
            DataKind::Regression => {
                ui.horizontal(|ui| {
                    ui.label("Features");
                    ui.radio_value(&mut draft.regression_dims, 1, "1 (line)");
                    ui.radio_value(&mut draft.regression_dims, 2, "2 (plane)");
                });
                ui.add(egui::Slider::new(&mut draft.outliers, 0.0..=0.4).text("outlier fraction"));
            }
            DataKind::Iris => {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("iris-pos").selected_text(&draft.iris_positive).show_ui(ui, |ui| {
                        for c in IRIS_CLASSES {
                            ui.selectable_value(&mut draft.iris_positive, c.to_string(), c);
                        }
                    });
                    ui.label("vs");
                    let neg = draft.iris_negative.clone().unwrap_or_else(|| "rest".into());
                    egui::ComboBox::from_id_salt("iris-neg").selected_text(neg).show_ui(ui, |ui| {
                        for c in IRIS_CLASSES.iter().filter(|c| **c != draft.iris_positive) {
                            ui.selectable_value(&mut draft.iris_negative, Some(c.to_string()), *c);
                        }
                        ui.selectable_value(&mut draft.iris_negative, None, "rest");
                    });
                });
                if draft.iris_negative.as_deref() == Some(draft.iris_positive.as_str()) {
                    draft.iris_negative = None;
                }
            }
            #[cfg(feature = "mnist")]
            DataKind::Mnist => {
                ui.horizontal(|ui| {
                    ui.label("Digits");
                    ui.add(egui::DragValue::new(&mut draft.mnist_digits.0).range(0..=9));
                    ui.label("vs");
                    ui.add(egui::DragValue::new(&mut draft.mnist_digits.1).range(0..=9));
                });
                ui.add(egui::Slider::new(&mut draft.mnist_samples, 200..=6000).text("samples"));
                ui.small("First load downloads MNIST (~11 MB) and caches it.");
            }
            _ => {}
        }

        if draft.kind != DataKind::Regression {
            view_controls(ui, draft);
        }
        ui.horizontal(|ui| {
            ui.label("Seed");
            ui.add(egui::DragValue::new(&mut draft.seed));
        });
        #[cfg(feature = "mnist")]
        let same_digits = draft.kind == DataKind::Mnist && draft.mnist_digits.0 == draft.mnist_digits.1;
        #[cfg(not(feature = "mnist"))]
        let same_digits = false;
        if ui.add_enabled(!same_digits, egui::Button::new("Load")).clicked() {
            out.load.write(LoadExperiment(draft.spec(experiment.map(|e| &e.spec))));
        }

        if let Some(e) = experiment {
            ui.separator();
            ui.label(egui::RichText::new(&e.name).strong());
            ui.small(format!("{} samples · {} features", e.points.len(), e.data.x.ncols()));
            if let Some(ratio) = e.explained_variance() {
                let parts: Vec<String> = ratio.iter().map(|r| format!("{:.1}%", r * 100.0)).collect();
                ui.small(format!("PCA explained variance: {}", parts.join(", ")));
            }
            if !e.is_exact() {
                ui.small("Models train on every feature; the drawn boundary is its slice through the displayed plane (other features at their mean).");
            }
            legend(ui, e);
        }
    });
}

fn view_controls(ui: &mut egui::Ui, draft: &mut Draft) {
    let names = draft.kind.feature_names();
    ui.horizontal(|ui| {
        ui.label("View");
        if !names.is_empty() {
            ui.radio_value(&mut draft.pca, false, "Features");
        }
        ui.radio_value(&mut draft.pca, true, "PCA");
    });
    if draft.pca || names.is_empty() {
        draft.pca = true;
        ui.horizontal(|ui| {
            ui.radio_value(&mut draft.pca_dims, 2, "2-D");
            ui.radio_value(&mut draft.pca_dims, 3, "3-D");
        });
    } else {
        let axes = ["x", "y", "z"];
        for a in 0..draft.features.len() {
            ui.horizontal(|ui| {
                ui.label(axes[a]);
                let cur = draft.features[a].min(names.len() - 1);
                egui::ComboBox::from_id_salt(("feature", a))
                    .selected_text(&names[cur])
                    .show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut draft.features[a], i, n);
                        }
                    });
            });
        }
        if names.len() >= 3 {
            let mut three = draft.features.len() == 3;
            if ui.checkbox(&mut three, "3-D (third feature)").changed() {
                if three {
                    let next = (0..names.len())
                        .find(|i| !draft.features.contains(i))
                        .unwrap_or(0);
                    draft.features.push(next);
                } else {
                    draft.features.truncate(2);
                }
            }
        }
    }
    ui.horizontal(|ui| {
        ui.label("Train on");
        ui.radio_value(&mut draft.train_space, TrainSpace::Full, "all features");
        ui.radio_value(
            &mut draft.train_space,
            TrainSpace::Projected,
            "displayed axes",
        );
    });
}

fn legend(ui: &mut egui::Ui, e: &Experiment) {
    ui.horizontal_wrapped(|ui| {
        for (i, name) in e.class_names.iter().enumerate().take(3) {
            let c = CLASS_COLORS[i].to_srgba();
            let color = egui::Color32::from_rgb(
                (c.red * 255.0) as u8,
                (c.green * 255.0) as u8,
                (c.blue * 255.0) as u8,
            );
            marker(ui, i, color);
            ui.colored_label(color, name);
        }
        if e.task == Task::Classification {
            ui.small("· shaded by predicted class");
        }
    });
}

/// The class's marker shape (circle, triangle, square), as in the scene.
fn marker(ui: &mut egui::Ui, class: usize, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    let (c, r) = (rect.center(), 5.0);
    let painter = ui.painter();
    match class {
        0 => painter.circle_filled(c, r, color),
        1 => painter.add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(0.0, -r),
                c + egui::vec2(r, r * 0.8),
                c + egui::vec2(-r, r * 0.8),
            ],
            color,
            egui::Stroke::NONE,
        )),
        _ => painter.rect_filled(
            egui::Rect::from_center_size(c, egui::vec2(2.0 * r, 2.0 * r) * 0.85),
            0.0,
            color,
        ),
    };
}

fn log_slider(
    ui: &mut egui::Ui,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    text: &str,
) -> bool {
    ui.add(egui::Slider::new(value, range).logarithmic(true).text(text))
        .changed()
}

fn models_section(ui: &mut egui::Ui, e: &mut Experiment, error: &mut Option<String>) {
    egui::CollapsingHeader::new("Models")
        .default_open(true)
        .show(ui, |ui| {
            let mut remove = None;
            for i in 0..e.panes.len() {
                let current = e.panes[i].trainer.config().clone();
                let mut cfg = current.clone();
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("Pane {}", i + 1)).strong());
                        ui.label(cfg.model.name());
                        if e.panes.len() > 1
                            && ui
                                .small_button("Remove")
                                .on_hover_text("Remove pane")
                                .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                    let mut loss = cfg.loss;
                    egui::ComboBox::from_id_salt(("loss", i))
                        .selected_text(loss.name())
                        .show_ui(ui, |ui| {
                            for l in LossKind::for_task(e.task).filter(|l| l.is_trainable()) {
                                ui.selectable_value(&mut loss, l, l.name());
                            }
                        });
                    if loss != cfg.loss {
                        cfg = config_for_loss(&cfg, loss);
                    }
                    if cfg.model == ModelKind::Svm {
                        let mut c = cfg.c().unwrap_or(1.0);
                        if log_slider(ui, &mut c, 1e-3..=1e3, "C")
                            && let Ok(next) = cfg.clone().with_c(c)
                        {
                            cfg = next;
                        }
                    } else {
                        let mut on = cfg.lambda > 0.0;
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut on, "L2").changed() {
                                cfg.lambda = if on { 0.01 } else { 0.0 };
                            }
                            if on {
                                log_slider(ui, &mut cfg.lambda, 1e-4..=10.0, "λ");
                            }
                        });
                    }
                    match cfg.loss {
                        LossKind::Huber => {
                            let mut d = cfg.loss_params.huber_delta();
                            if log_slider(ui, &mut d, 0.05..=5.0, "Huber δ") {
                                cfg.loss_params = cfg
                                    .loss_params
                                    .with_huber_delta(d)
                                    .unwrap_or(cfg.loss_params);
                            }
                        }
                        LossKind::Hinge | LossKind::SquaredHinge => {
                            let mut m = cfg.loss_params.margin();
                            if ui
                                .add(egui::Slider::new(&mut m, 0.1..=3.0).text("margin"))
                                .changed()
                            {
                                cfg.loss_params =
                                    cfg.loss_params.with_margin(m).unwrap_or(cfg.loss_params);
                            }
                        }
                        _ => {}
                    }
                    egui::CollapsingHeader::new("Optimizer")
                        .id_salt(("optimizer", i))
                        .show(ui, |ui| {
                            let mut eta = cfg.learning_rate.initial();
                            if log_slider(ui, &mut eta, 1e-4..=2.0, "learning rate") {
                                cfg.learning_rate = match cfg.learning_rate {
                                    LearningRate::Constant(_) => LearningRate::Constant(eta),
                                    LearningRate::InverseDecay { initial, decay } => {
                                        LearningRate::InverseDecay {
                                            initial: eta,
                                            decay: decay * eta / initial,
                                        }
                                    }
                                };
                            }
                            let mut mini = cfg.batch_size.is_some();
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut mini, "mini-batch").changed() {
                                    cfg.batch_size = mini.then_some(16);
                                }
                                if let Some(b) = &mut cfg.batch_size {
                                    ui.add(egui::DragValue::new(b).range(1..=4096));
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("max steps");
                                ui.add(egui::DragValue::new(&mut cfg.max_steps).range(1..=200_000));
                            });
                        });
                });
                if cfg != current {
                    *error = e.update_pane(i, cfg).err();
                }
            }
            if let Some(i) = remove {
                e.remove_pane(i);
            }
            if e.panes.len() < MAX_PANES && ui.button("+ Compare (add pane)").clicked() {
                let cfg = e
                    .panes
                    .last()
                    .map(|p| p.trainer.config().clone())
                    .unwrap_or_else(|| default_config(e.task));
                *error = e.add_pane(cfg).err();
            }
            if let Some(err) = error {
                ui.colored_label(egui::Color32::from_rgb(200, 40, 40), err.as_str());
            }
        });
}

fn status_text(s: Option<Status>) -> &'static str {
    match s {
        Some(Status::Running) | None => "running",
        Some(Status::Converged) => "converged",
        Some(Status::BudgetExhausted) => "step budget reached",
        Some(Status::Diverged) => "diverged — lower the learning rate",
    }
}

fn playback_section(
    ui: &mut egui::Ui,
    e: &Experiment,
    playback: &mut Playback,
    views: &PaneViews,
    sweep_active: bool,
    out: &mut Writers,
) {
    egui::CollapsingHeader::new("Playback")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("⏮").on_hover_text("Reset (R)").clicked() {
                    out.playback.write(PlaybackCommand::Reset);
                }
                let label = if playback.playing {
                    "⏸ Pause"
                } else {
                    "▶ Play"
                };
                if ui.button(label).on_hover_text("Space").clicked() {
                    out.playback.write(PlaybackCommand::Toggle);
                }
                if ui.button("⏭ Step").on_hover_text("S").clicked() {
                    out.playback.write(PlaybackCommand::Step);
                }
            });
            ui.add(
                egui::Slider::new(&mut playback.steps_per_second, 1.0..=2000.0)
                    .logarithmic(true)
                    .text("steps / s"),
            );
            let max = e.max_step();
            let mut cursor = playback.cursor.min(max);
            if ui
                .add_enabled(
                    max > 0,
                    egui::Slider::new(&mut cursor, 0..=max.max(1)).text("step"),
                )
                .changed()
            {
                out.playback.write(PlaybackCommand::Seek(cursor));
            }
            for (i, v) in views.0.iter().enumerate().take(e.panes.len()) {
                ui.small(format!(
                    "Pane {}: step {} · objective {:.4} · {}",
                    i + 1,
                    v.step,
                    v.loss,
                    if sweep_active {
                        "sweep solution"
                    } else {
                        status_text(v.status)
                    }
                ));
            }
        });
}

fn sweep_section(
    ui: &mut egui::Ui,
    e: &Experiment,
    draft: &mut Draft,
    sweep: &mut Sweep,
    out: &mut Writers,
) {
    egui::CollapsingHeader::new("Hyperparameter sweep")
        .default_open(false)
        .show(ui, |ui| {
            let applicable: Vec<SweepParam> = SweepParam::ALL
                .into_iter()
                .filter(|p| {
                    e.panes
                        .iter()
                        .any(|pane| p.applies_to(pane.trainer.config()))
                })
                .collect();
            if !applicable.contains(&draft.sweep.param)
                && let Some(&p) = applicable.first()
            {
                draft.sweep.param = p;
            }
            let spec = &mut draft.sweep;
            let before = spec.param;
            egui::ComboBox::from_label("Parameter")
                .selected_text(spec.param.name())
                .show_ui(ui, |ui| {
                    for &p in &applicable {
                        ui.selectable_value(&mut spec.param, p, p.name());
                    }
                });
            if spec.param != before {
                (spec.from, spec.to, spec.log) = match spec.param {
                    SweepParam::C => (0.01, 100.0, true),
                    SweepParam::Lambda => (1e-4, 1.0, true),
                    SweepParam::HuberDelta => (0.1, 3.0, true),
                    SweepParam::Margin => (0.25, 2.5, false),
                    SweepParam::LearningRate => (1e-3, 0.5, true),
                };
            }
            ui.horizontal(|ui| {
                ui.label("from");
                ui.add(egui::DragValue::new(&mut spec.from).speed(0.01));
                ui.label("to");
                ui.add(egui::DragValue::new(&mut spec.to).speed(0.01));
                ui.checkbox(&mut spec.log, "log");
            });
            ui.add(egui::Slider::new(&mut spec.samples, 3..=40).text("values"));
            if sweep.secs_per_value <= 0.0 {
                sweep.secs_per_value = 0.6;
            }
            ui.add(egui::Slider::new(&mut sweep.secs_per_value, 0.1..=3.0).text("s per value"));
            let valid = spec.from.is_finite()
                && spec.to.is_finite()
                && (!spec.log || (spec.from > 0.0 && spec.to > 0.0));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        valid && !applicable.is_empty(),
                        egui::Button::new("Run sweep"),
                    )
                    .clicked()
                {
                    out.sweep.write(SweepCommand::Start(spec.clone()));
                }
                if sweep.active && ui.button("Stop").clicked() {
                    out.sweep.write(SweepCommand::Stop);
                }
            });
            let (done, total) = sweep.progress();
            if total > 0 {
                ui.add(
                    egui::ProgressBar::new(done as f32 / total as f32)
                        .text(format!("{done}/{total} solutions")),
                );
                ui.horizontal(|ui| {
                    if sweep.playing {
                        if ui.button("⏸").clicked() {
                            out.sweep.write(SweepCommand::Pause);
                        }
                    } else if ui.button("▶").clicked() {
                        out.sweep.write(SweepCommand::Play);
                    }
                    let mut idx = sweep.index;
                    let n = sweep.values.len().saturating_sub(1);
                    if ui
                        .add(egui::Slider::new(&mut idx, 0..=n).show_value(false))
                        .changed()
                    {
                        out.sweep.write(SweepCommand::Seek(idx));
                    }
                    if let Some(v) = sweep.current_value() {
                        ui.label(format!("{} = {}", sweep.spec.param.name(), fmt_num(v)));
                    }
                });
            }
        });
}

fn display_section(
    ui: &mut egui::Ui,
    scene: &mut SceneSettings,
    playback: &mut Playback,
    out: &mut Writers,
) {
    egui::CollapsingHeader::new("Display")
        .default_open(false)
        .show(ui, |ui| {
            ui.checkbox(&mut scene.show_margins, "SVM margins");
            ui.checkbox(&mut scene.show_support_vectors, "Support vectors");
            ui.checkbox(&mut scene.show_regions, "Decision regions");
            ui.checkbox(&mut scene.show_residuals, "Residuals");
            ui.checkbox(&mut scene.show_bounds, "Bounding box");
            ui.add(
                egui::Slider::new(&mut playback.transition_secs, 0.0..=1.5).text("transition (s)"),
            );
            if ui.button("Frame data (F)").clicked() {
                out.frame.write(FrameData);
            }
            ui.small("Drag to pan (2-D) or orbit (3-D), right-drag to pan, scroll to zoom.");
        });
}

fn charts_section(
    ui: &mut egui::Ui,
    settings: &mut ChartSettings,
    experiment: Option<&Experiment>,
    tex: &[(bool, egui::TextureId); 3],
) {
    let width = ui.available_width().min(settings.size.x as f32);
    let size = egui::vec2(
        width,
        width * settings.size.y as f32 / settings.size.x as f32,
    );
    ui.checkbox(&mut settings.show_loss_curves, "Loss curves");
    if settings.show_loss_curves {
        if let Some(e) = experiment {
            ui.horizontal_wrapped(|ui| {
                let all: Vec<LossKind> = LossKind::for_task(e.task).collect();
                let mut shown = if settings.overlay.is_empty() {
                    all.clone()
                } else {
                    settings.overlay.clone()
                };
                for l in &all {
                    let mut on = shown.contains(l);
                    if ui.checkbox(&mut on, l.name()).changed() {
                        if on {
                            shown.push(*l);
                        } else {
                            shown.retain(|x| x != l);
                        }
                        settings.overlay = if shown.is_empty() {
                            vec![*l]
                        } else {
                            shown.clone()
                        };
                    }
                }
            });
        }
        for &(ready, id) in &tex[..2] {
            if ready {
                ui.image(egui::load::SizedTexture::new(id, size));
            }
        }
    }
    ui.checkbox(&mut settings.show_training, "Training objective");
    if settings.show_training && tex[2].0 {
        ui.image(egui::load::SizedTexture::new(tex[2].1, size));
    }
}

fn weights_section(
    ui: &mut egui::Ui,
    e: &Experiment,
    weights: &WeightImages,
    tex: &[egui::TextureId],
) {
    if tex.is_empty() {
        return;
    }
    ui.separator();
    ui.label(egui::RichText::new("Learned weights").strong());
    ui.small(format!(
        "Each pixel's weight: toward {} where coloured like it, toward {} for the other colour.",
        e.class_names.first().map_or("", |s| s.as_str()),
        e.class_names.get(1).map_or("", |s| s.as_str())
    ));
    ui.horizontal_wrapped(|ui| {
        for (i, &id) in tex.iter().enumerate() {
            ui.vertical(|ui| {
                ui.image(egui::load::SizedTexture::new(id, egui::vec2(168.0, 168.0)));
                ui.small(format!(
                    "Pane {} · max |w| {}",
                    i + 1,
                    fmt_num(weights.max_abs.get(i).copied().unwrap_or(0.0))
                ));
            });
        }
    });
}

fn pane_overlays(
    ctx: &egui::Context,
    e: &Experiment,
    views: &PaneViews,
    sweep: &Sweep,
    window: Vec2,
    insets: UiInsets,
) {
    for (i, rect) in pane_rects(window, insets, e.panes.len())
        .into_iter()
        .enumerate()
    {
        let Some(view) = views.0.get(i) else { continue };
        egui::Area::new(egui::Id::new(("bevaru-pane", i)))
            .fixed_pos(egui::pos2(rect.min.x + 10.0, rect.min.y + 10.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 215))
                    .inner_margin(6.0)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.set_max_width(rect.width() - 30.0);
                        ui.label(
                            egui::RichText::new(e.pane_label(i))
                                .strong()
                                .color(egui::Color32::BLACK),
                        );
                        if let (true, Some(v)) = (sweep.active, sweep.current_value()) {
                            let applies = sweep.spec.param.applies_to(e.panes[i].trainer.config());
                            let text = if applies {
                                format!(
                                    "sweep: {} = {} (converged solution)",
                                    sweep.spec.param.name(),
                                    fmt_num(v)
                                )
                            } else {
                                format!("{} does not apply — held fixed", sweep.spec.param.name())
                            };
                            ui.label(
                                egui::RichText::new(text)
                                    .color(egui::Color32::from_rgb(0, 90, 150)),
                            );
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "step {} · objective {:.4}",
                                view.step, view.loss
                            ))
                            .color(egui::Color32::DARK_GRAY),
                        );
                        let axes = e.axis_names.join(" × ");
                        ui.small(egui::RichText::new(axes).color(egui::Color32::DARK_GRAY));
                        if !e.is_exact() {
                            ui.small(
                                egui::RichText::new("projection: boundary sliced at the data mean")
                                    .color(egui::Color32::DARK_GRAY),
                            );
                        }
                        if view.model.is_some() && view.shown.is_none() {
                            ui.label(
                                egui::RichText::new("no boundary (w = 0)")
                                    .color(egui::Color32::from_rgb(180, 60, 0)),
                            );
                        }
                    });
            });
    }
}

fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    egui: Option<Res<EguiWantsInput>>,
    mut playback: MessageWriter<PlaybackCommand>,
    mut frame: MessageWriter<FrameData>,
) {
    if egui.is_some_and(|e| e.wants_any_keyboard_input()) {
        return;
    }
    if keys.just_pressed(KeyCode::Space) {
        playback.write(PlaybackCommand::Toggle);
    }
    if keys.just_pressed(KeyCode::KeyS) {
        playback.write(PlaybackCommand::Step);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        playback.write(PlaybackCommand::Reset);
    }
    if keys.just_pressed(KeyCode::KeyF) {
        frame.write(FrameData);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_textures_are_released() {
        let (a, b) = (
            AssetId::<Image>::invalid(),
            AssetId::<Image>::from(bevy::asset::uuid::Uuid::from_u128(7)),
        );
        let mut t = EguiImageTextures::default();
        t.get_or_add(a, || egui::TextureId::User(1));
        t.get_or_add(b, || egui::TextureId::User(2));
        // Re-adding an existing image reuses its texture.
        assert_eq!(
            t.get_or_add(a, || egui::TextureId::User(99)),
            egui::TextureId::User(1)
        );
        let stale = t.take_stale(|id| id == a);
        assert_eq!(stale, vec![b]);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn loss_switch_picks_matching_model_and_keeps_settings() {
        let svm = TrainerConfig {
            max_steps: 777,
            ..TrainerConfig::svm(4.0).unwrap()
        };
        let logistic = config_for_loss(&svm, LossKind::Logistic);
        assert_eq!(logistic.model, ModelKind::LogisticRegression);
        assert_eq!(logistic.max_steps, 777);
        let back = config_for_loss(&logistic, LossKind::SquaredHinge);
        assert_eq!(back.model, ModelKind::Svm);
        let reg = config_for_loss(&TrainerConfig::regression(LossKind::Mse), LossKind::Huber);
        assert_eq!(
            (reg.model, reg.loss),
            (ModelKind::LinearRegression, LossKind::Huber)
        );
    }

    #[test]
    fn draft_keeps_panes_only_for_same_task() {
        let current = ExperimentSpec::default().compare(TrainerConfig::logistic());
        let mut draft = Draft::default();
        draft.sync_from(&current);
        assert_eq!(draft.spec(Some(&current)).panes.len(), 2);
        draft.kind = DataKind::Regression;
        let spec = draft.spec(Some(&current));
        assert_eq!(spec.panes, vec![default_config(Task::Regression)]);
    }
}
