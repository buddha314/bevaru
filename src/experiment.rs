//! An experiment: one dataset, how it is displayed, and one trainer per
//! comparison pane. Built off the main thread, since MNIST may download and
//! PCA on 784-D data takes a moment.

use std::sync::Arc;

use bevaru_core::dataset::{self, BinaryTask, Dataset};
use bevaru_core::nalgebra::{DMatrix, DVector};
use bevaru_core::{
    LinearModel, ModelKind, Projection, ProjectionKind, Task, Trainer, TrainerConfig, TrainingData,
};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task as BevyTask, futures::check_ready};

use crate::geometry::Boundary;

/// Which data to load.
#[derive(Debug, Clone, PartialEq)]
pub enum DatasetChoice {
    SeparableBlobs,
    OverlappingBlobs,
    /// Synthetic linear regression with 1 or 2 features.
    Regression {
        dims: usize,
        outliers: f64,
    },
    /// Iris reduced to a binary task; `negative: None` is one-vs-rest.
    Iris {
        positive: String,
        negative: Option<String>,
    },
    /// Two MNIST digits, subsampled.
    #[cfg(feature = "mnist")]
    Mnist {
        positive: u8,
        negative: u8,
        samples: usize,
    },
}

impl DatasetChoice {
    pub fn task(&self) -> Task {
        match self {
            DatasetChoice::Regression { .. } => Task::Regression,
            _ => Task::Classification,
        }
    }

    pub fn label(&self) -> String {
        match self {
            DatasetChoice::SeparableBlobs => "Blobs (separable)".into(),
            DatasetChoice::OverlappingBlobs => "Blobs (overlapping)".into(),
            DatasetChoice::Regression { dims, outliers } => {
                format!("Regression {dims}-D, {:.0}% outliers", outliers * 100.0)
            }
            DatasetChoice::Iris { positive, negative } => {
                format!(
                    "Iris: {positive} vs {}",
                    negative.as_deref().unwrap_or("rest")
                )
            }
            #[cfg(feature = "mnist")]
            DatasetChoice::Mnist {
                positive, negative, ..
            } => format!("MNIST: {positive} vs {negative}"),
        }
    }

    /// The natural display for this data.
    pub fn default_view(&self) -> View {
        match self {
            DatasetChoice::Iris { .. } => View::Features(vec![2, 3]),
            #[cfg(feature = "mnist")]
            DatasetChoice::Mnist { .. } => View::Pca(2),
            _ => View::Features(vec![0, 1]),
        }
    }
}

/// How classification data is projected for display. Regression data is
/// always shown as its features plus the target on the last axis.
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    /// Two or three feature columns.
    Features(Vec<usize>),
    /// Top 2 or 3 principal components.
    Pca(usize),
}

/// Where models are trained when the display is a projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainSpace {
    /// On every feature; the drawn boundary is a slice through the display plane.
    Full,
    /// On the displayed coordinates only; the drawn boundary is exact.
    Projected,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExperimentSpec {
    pub dataset: DatasetChoice,
    pub view: View,
    pub train_space: TrainSpace,
    /// One trainer configuration per side-by-side pane.
    pub panes: Vec<TrainerConfig>,
    /// Seed for data generation and subsampling.
    pub seed: u64,
}

pub const MAX_PANES: usize = 3;

impl ExperimentSpec {
    pub fn new(dataset: DatasetChoice, config: TrainerConfig) -> Self {
        Self {
            view: dataset.default_view(),
            dataset,
            train_space: TrainSpace::Full,
            panes: vec![config],
            seed: 1,
        }
    }

    pub fn with_view(mut self, view: View) -> Self {
        self.view = view;
        self
    }

    pub fn with_train_space(mut self, space: TrainSpace) -> Self {
        self.train_space = space;
        self
    }

    /// Add a side-by-side pane trained with `config` on the same data.
    pub fn compare(mut self, config: TrainerConfig) -> Self {
        self.panes.push(config);
        self
    }
}

impl Default for ExperimentSpec {
    fn default() -> Self {
        Self::new(
            DatasetChoice::OverlappingBlobs,
            TrainerConfig::svm(1.0).expect("C = 1 is valid"),
        )
    }
}

/// Maps display coordinates (features or principal components, plus the
/// target for regression) to world space: `world = (z − center) · scale`.
/// One uniform scale keeps angles and margins geometrically honest.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayFrame {
    pub center: Vec<f64>,
    pub scale: f64,
}

/// World-space half extent the data is fitted into.
const WORLD_HALF_EXTENT: f64 = 5.0;

impl DisplayFrame {
    fn fit(z: &DMatrix<f64>) -> Self {
        let mut center = Vec::with_capacity(z.ncols());
        let mut half = 1e-9f64;
        for c in z.column_iter() {
            let (lo, hi) = c
                .iter()
                .fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
            center.push(0.5 * (lo + hi));
            half = half.max(0.5 * (hi - lo));
        }
        Self {
            center,
            scale: WORLD_HALF_EXTENT / half,
        }
    }

    pub fn to_world(&self, z: impl IntoIterator<Item = f64>) -> Vec3 {
        let mut p = [0.0f32; 3];
        for (k, v) in z.into_iter().enumerate().take(3) {
            p[k] = ((v - self.center[k]) * self.scale) as f32;
        }
        Vec3::from(p)
    }

    /// The world-space boundary of `f(z) = w·z + b`: substituting
    /// `z = world/scale + center` gives `(w/scale)·world + (w·center + b)`.
    pub fn boundary(&self, w: &DVector<f64>, b: f64) -> Option<Boundary> {
        let mut ww = [0.0f32; 3];
        let mut bw = b;
        for k in 0..w.len().min(3) {
            ww[k] = (w[k] / self.scale) as f32;
            bw += w[k] * self.center[k];
        }
        Boundary::from_linear(Vec3::from(ww), bw as f32)
    }
}

pub struct Pane {
    pub trainer: Trainer,
}

/// The loaded experiment. Replaced wholesale when a new one finishes loading;
/// `generation` tells the scene to rebuild.
#[derive(Resource)]
pub struct Experiment {
    pub spec: ExperimentSpec,
    pub name: String,
    pub task: Task,
    /// 2 or 3.
    pub dims: usize,
    pub axis_names: Vec<String>,
    /// Sample positions in world space (z = 0 for 2-D).
    pub points: Vec<Vec3>,
    pub classes: Vec<usize>,
    pub class_names: Vec<String>,
    /// World-space bounds of the points, padded.
    pub bounds: (Vec3, Vec3),
    pub data: Arc<TrainingData>,
    /// Classification only.
    pub projection: Option<Projection>,
    pub frame: DisplayFrame,
    pub panes: Vec<Pane>,
    pub generation: u64,
    /// Regression only: indices of generated outliers.
    pub outliers: Vec<usize>,
    /// Side length when samples are square images (MNIST), for weight views.
    pub image_side: Option<usize>,
}

impl Experiment {
    /// Build synchronously. Prefer [`LoadExperiment`] in an app.
    pub fn build(spec: ExperimentSpec) -> Result<Self, String> {
        if spec.panes.is_empty() || spec.panes.len() > MAX_PANES {
            return Err(format!(
                "need 1 to {MAX_PANES} panes, got {}",
                spec.panes.len()
            ));
        }
        let (dataset, outliers, image_side) = load_dataset(&spec)?;
        let task = dataset.task();
        let class_names = dataset.class_names();
        let classes = dataset.class_indices();
        let full = dataset.training_data().map_err(|e| e.to_string())?;

        let (display, axis_names, projection, data) = match task {
            Task::Regression => {
                if !(1..=2).contains(&dataset.dim()) {
                    return Err(format!(
                        "regression display needs 1 or 2 features, got {}",
                        dataset.dim()
                    ));
                }
                let mut z = dataset.features.clone().insert_column(dataset.dim(), 0.0);
                z.set_column(dataset.dim(), &full.y);
                let mut names = dataset.feature_names.clone();
                names.push("y".into());
                (z, names, None, full)
            }
            Task::Classification => {
                let projection = match &spec.view {
                    View::Features(cols) => {
                        if !(2..=3).contains(&cols.len())
                            || cols.iter().any(|&c| c >= dataset.dim())
                        {
                            return Err(format!(
                                "view needs 2 or 3 of the {} features",
                                dataset.dim()
                            ));
                        }
                        Projection::features(&dataset.features, cols, &dataset.feature_names)
                    }
                    View::Pca(k) => {
                        if !(2..=3).contains(k) || *k > dataset.dim() {
                            return Err(format!("PCA view needs 2 or 3 components, got {k}"));
                        }
                        Projection::pca(&dataset.features, *k)
                    }
                };
                let z = projection.project(&dataset.features);
                let data = match spec.train_space {
                    TrainSpace::Full => full,
                    TrainSpace::Projected => TrainingData {
                        x: z.clone(),
                        y: full.y,
                        task,
                    },
                };
                (z, projection.axis_names.clone(), Some(projection), data)
            }
        };

        let frame = DisplayFrame::fit(&display);
        let points: Vec<Vec3> = display
            .row_iter()
            .map(|r| frame.to_world(r.iter().copied()))
            .collect();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in &points {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        let pad = (hi - lo).max_element() * 0.08;
        let dims = display.ncols();
        let pad3 = if dims == 3 {
            Vec3::splat(pad)
        } else {
            Vec3::new(pad, pad, 0.0)
        };

        let data = Arc::new(data);
        let panes = spec
            .panes
            .iter()
            .map(|cfg| Trainer::new(cfg.clone(), data.clone()).map(|trainer| Pane { trainer }))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            name: spec.dataset.label(),
            task,
            dims,
            axis_names,
            points,
            classes,
            class_names,
            bounds: (lo - pad3, hi + pad3),
            data,
            projection,
            frame,
            panes,
            generation: 0,
            outliers,
            image_side,
            spec,
        })
    }

    /// Whether the drawn boundary is the model's true boundary, rather than
    /// its intersection with the display plane.
    pub fn is_exact(&self) -> bool {
        match (&self.projection, self.spec.train_space) {
            (Some(p), TrainSpace::Full) => p.is_exact(),
            _ => true,
        }
    }

    pub fn explained_variance(&self) -> Option<&[f64]> {
        match &self.projection.as_ref()?.kind {
            ProjectionKind::Pca {
                explained_variance_ratio,
            } => Some(explained_variance_ratio),
            ProjectionKind::Features(_) => None,
        }
    }

    /// The model's boundary in world space.
    pub fn boundary(&self, model: &LinearModel) -> Option<Boundary> {
        let (w, b) = match (self.task, &self.projection, self.spec.train_space) {
            // Fit plane y = w·x + b as the zero set of y − w·x − b.
            (Task::Regression, ..) => {
                let w = DVector::from_iterator(
                    model.dim() + 1,
                    model.w.iter().map(|v| -v).chain([1.0]),
                );
                (w, -model.b)
            }
            (Task::Classification, Some(p), TrainSpace::Full) => {
                let r = p.restrict(model);
                (r.w, r.b)
            }
            _ => (model.w.clone(), model.b),
        };
        self.frame.boundary(&w, b)
    }

    /// Regression: the world-space point on the fit directly above or below
    /// sample `i` (its prediction).
    pub fn fitted_point(&self, model: &LinearModel, i: usize) -> Option<Vec3> {
        if self.task != Task::Regression {
            return None;
        }
        let x = self.data.x.row(i);
        let y_hat = model.w.dot(&x.transpose()) + model.b;
        let mut p = self.points[i];
        let k = self.dims - 1;
        p[k] = ((y_hat - self.frame.center[k]) * self.frame.scale) as f32;
        Some(p)
    }

    /// Apply a changed pane configuration. Changing the model, loss, batch
    /// size, or seed restarts that pane from the shared initialization;
    /// anything else (λ, C, δ, margin, learning rate, budget) continues from
    /// the current parameters. Returns whether the pane restarted.
    pub fn update_pane(&mut self, i: usize, config: TrainerConfig) -> Result<bool, String> {
        let pane = self.panes.get_mut(i).ok_or("no such pane")?;
        let old = pane.trainer.config();
        let restart = old.model != config.model
            || old.loss != config.loss
            || old.batch_size != config.batch_size
            || old.seed != config.seed;
        if restart {
            pane.trainer =
                Trainer::new(config.clone(), self.data.clone()).map_err(|e| e.to_string())?;
        } else {
            pane.trainer
                .reconfigure(config.clone())
                .map_err(|e| e.to_string())?;
        }
        self.spec.panes[i] = config;
        Ok(restart)
    }

    /// Restart every pane from step 0 with its current configuration.
    pub fn reset(&mut self) {
        for pane in &mut self.panes {
            let cfg = pane.trainer.config().clone();
            pane.trainer =
                Trainer::new(cfg, self.data.clone()).expect("configuration was already validated");
        }
    }

    pub fn add_pane(&mut self, config: TrainerConfig) -> Result<(), String> {
        if self.panes.len() >= MAX_PANES {
            return Err(format!("at most {MAX_PANES} panes"));
        }
        let trainer = Trainer::new(config.clone(), self.data.clone()).map_err(|e| e.to_string())?;
        self.panes.push(Pane { trainer });
        self.spec.panes.push(config);
        Ok(())
    }

    pub fn remove_pane(&mut self, i: usize) {
        if self.panes.len() > 1 && i < self.panes.len() {
            self.panes.remove(i);
            self.spec.panes.remove(i);
        }
    }

    /// Highest step any pane has reached.
    pub fn max_step(&self) -> usize {
        self.panes
            .iter()
            .map(|p| p.trainer.steps_taken())
            .max()
            .unwrap_or(0)
    }

    /// Short description of a pane's configuration.
    pub fn pane_label(&self, i: usize) -> String {
        let cfg = self.panes[i].trainer.config();
        let mut s = format!("{} · {}", cfg.model, cfg.loss);
        match cfg.model {
            ModelKind::Svm => {
                if let Some(c) = cfg.c() {
                    s += &format!(" · C = {}", fmt_num(c));
                }
            }
            _ if cfg.lambda > 0.0 => s += &format!(" · λ = {}", fmt_num(cfg.lambda)),
            _ => {}
        }
        if cfg.loss == bevaru_core::LossKind::Huber {
            s += &format!(" · δ = {}", fmt_num(cfg.loss_params.huber_delta()));
        }
        s
    }
}

/// Compact number formatting for labels.
pub fn fmt_num(v: f64) -> String {
    if v != 0.0 && (v.abs() >= 1e4 || v.abs() < 1e-2) {
        format!("{v:.1e}")
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn load_dataset(spec: &ExperimentSpec) -> Result<(Dataset, Vec<usize>, Option<usize>), String> {
    Ok(match &spec.dataset {
        DatasetChoice::SeparableBlobs => (dataset::blobs(60, true, spec.seed), vec![], None),
        DatasetChoice::OverlappingBlobs => (dataset::blobs(80, false, spec.seed), vec![], None),
        DatasetChoice::Regression { dims, outliers } => {
            let (d, out) = dataset::regression(80, *dims, *outliers, spec.seed);
            (d, out, None)
        }
        DatasetChoice::Iris { positive, negative } => {
            let task = match negative {
                Some(n) => BinaryTask::Pair {
                    positive: positive.clone(),
                    negative: n.clone(),
                },
                None => BinaryTask::OneVsRest {
                    positive: positive.clone(),
                },
            };
            (
                dataset::iris().binary(&task).map_err(|e| e.to_string())?,
                vec![],
                None,
            )
        }
        #[cfg(feature = "mnist")]
        DatasetChoice::Mnist {
            positive,
            negative,
            samples,
        } => {
            use bevaru_core::mnist::{IMAGE_SIDE, MnistLoader, MnistOptions};
            let loader = MnistLoader::new().map_err(|e| e.to_string())?;
            let opts = MnistOptions {
                digits: Some(vec![*positive, *negative]),
                max_samples: Some(*samples),
                seed: spec.seed,
                ..Default::default()
            };
            let all = loader.load(&opts).map_err(|e| e.to_string())?;
            let pair = BinaryTask::Pair {
                positive: positive.to_string(),
                negative: negative.to_string(),
            };
            (
                all.binary(&pair).map_err(|e| e.to_string())?,
                vec![],
                Some(IMAGE_SIDE),
            )
        }
    })
}

/// Request a new experiment. The current one stays on screen until the new
/// one is ready.
#[derive(Message, Debug, Clone)]
pub struct LoadExperiment(pub ExperimentSpec);

/// The experiment to load at startup; [`ExperimentSpec::default`] if absent.
#[derive(Resource, Debug, Clone)]
pub struct StartupExperiment(pub ExperimentSpec);

#[derive(Resource, Default)]
pub struct ExperimentLoader {
    task: Option<BevyTask<Result<Experiment, String>>>,
    /// Label of the experiment being loaded.
    pub loading: Option<String>,
    pub error: Option<String>,
    generation: u64,
}

pub struct ExperimentPlugin;

impl Plugin for ExperimentPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<LoadExperiment>()
            .init_resource::<ExperimentLoader>()
            .add_systems(Startup, request_startup_experiment)
            .add_systems(PreUpdate, (start_loading, finish_loading).chain());
    }
}

fn request_startup_experiment(
    startup: Option<Res<StartupExperiment>>,
    mut load: MessageWriter<LoadExperiment>,
) {
    load.write(LoadExperiment(
        startup.map(|s| s.0.clone()).unwrap_or_default(),
    ));
}

fn start_loading(
    mut requests: MessageReader<LoadExperiment>,
    mut loader: ResMut<ExperimentLoader>,
) {
    // Only the latest request matters; dropping a running task cancels it.
    if let Some(LoadExperiment(spec)) = requests.read().last().cloned() {
        loader.loading = Some(spec.dataset.label());
        loader.error = None;
        loader.task =
            Some(AsyncComputeTaskPool::get().spawn(async move { Experiment::build(spec) }));
    }
}

fn finish_loading(mut commands: Commands, mut loader: ResMut<ExperimentLoader>) {
    let Some(task) = loader.task.as_mut() else {
        return;
    };
    let Some(result) = check_ready(task) else {
        return;
    };
    loader.task = None;
    loader.loading = None;
    match result {
        Ok(mut experiment) => {
            loader.generation += 1;
            experiment.generation = loader.generation;
            commands.insert_resource(experiment);
        }
        Err(e) => {
            error!("bevaru: experiment failed to load: {e}");
            loader.error = Some(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevaru_core::LossKind;

    #[test]
    fn regression_boundary_is_the_fit_line() {
        let spec = ExperimentSpec::new(
            DatasetChoice::Regression {
                dims: 1,
                outliers: 0.0,
            },
            TrainerConfig::regression(LossKind::Mse),
        );
        let e = Experiment::build(spec).unwrap();
        assert_eq!(e.dims, 2);
        let m = LinearModel {
            w: DVector::from_vec(vec![0.5]),
            b: 2.0,
        };
        let b = e.boundary(&m).unwrap();
        for i in [0, 7, 30] {
            let on_fit = e.fitted_point(&m, i).unwrap();
            assert!(
                b.eval(on_fit).abs() < 1e-3,
                "fitted point lies on the drawn line"
            );
        }
    }

    #[test]
    fn world_boundary_agrees_with_model_on_samples() {
        // Training in projected space: the drawn boundary must classify each
        // displayed point exactly as the model does.
        let spec = ExperimentSpec::new(
            DatasetChoice::Iris {
                positive: "versicolor".into(),
                negative: Some("virginica".into()),
            },
            TrainerConfig::svm(1.0).unwrap(),
        )
        .with_train_space(TrainSpace::Projected);
        let e = Experiment::build(spec).unwrap();
        assert!(e.is_exact());
        let m = LinearModel {
            w: DVector::from_vec(vec![1.5, -0.7]),
            b: 0.2,
        };
        let b = e.boundary(&m).unwrap();
        let f = m.decisions(&e.data.x);
        for i in 0..e.points.len() {
            assert!((b.eval(e.points[i]) as f64 - f[i]).abs() < 1e-3 * f[i].abs().max(1.0));
        }
    }

    #[test]
    fn full_space_training_on_feature_view_is_a_slice() {
        let spec = ExperimentSpec::new(
            DatasetChoice::Iris {
                positive: "setosa".into(),
                negative: None,
            },
            TrainerConfig::logistic(),
        );
        let e = Experiment::build(spec).unwrap();
        assert!(!e.is_exact());
        assert_eq!(e.axis_names, ["petal length (cm)", "petal width (cm)"]);
        assert_eq!(e.data.x.ncols(), 4);
    }

    #[test]
    fn update_pane_restarts_only_on_structural_change() {
        let mut e = Experiment::build(ExperimentSpec::default()).unwrap();
        for _ in 0..10 {
            e.panes[0].trainer.step();
        }
        let cfg = e.panes[0].trainer.config().clone().with_c(5.0).unwrap();
        assert!(!e.update_pane(0, cfg.clone()).unwrap());
        assert_eq!(e.panes[0].trainer.steps_taken(), 10);
        let cfg = TrainerConfig {
            loss: LossKind::SquaredHinge,
            ..cfg
        };
        assert!(e.update_pane(0, cfg).unwrap());
        assert_eq!(e.panes[0].trainer.steps_taken(), 0);
    }

    #[test]
    fn invalid_specs_are_errors() {
        let mut spec = ExperimentSpec::default().with_view(View::Features(vec![0, 5]));
        assert!(Experiment::build(spec.clone()).is_err());
        spec.view = View::Features(vec![0, 1]);
        spec.panes[0].loss = LossKind::Mse;
        assert!(
            Experiment::build(spec)
                .err()
                .unwrap()
                .contains("regression loss")
        );
    }
}
