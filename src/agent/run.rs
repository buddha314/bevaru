//! Running agent tools: the work behind each entry in [`super::tools`],
//! independent of any protocol. The MCP server and the remote hooks call
//! these, so every transport validates and computes the same way.

use bevaru_core::shapes::{ShapeView, SurfaceGrid};
use bevaru_core::{LossKind, LossParams, Status, Task, Trainer, TrainerConfig};
use bevy::math::UVec2;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::api::{
    ApiError, DatasetViewRequest, DescribeRequest, ExperimentRequest, LossChartRequest,
    LossShapeRenderRequest, LossShapeRequest, ShapeQuery, SweepToolRequest, TrainRequest,
    TrainerRequest,
};
use super::{Manifest, argument};
use crate::charts::{LineChart, Series, loss_curve_chart};
use crate::experiment::{Experiment, ExperimentSpec};
use crate::playback::SweepSpec;

pub const MAX_STEPS: usize = 100_000;
pub const MAX_TRAJECTORY_POINTS: usize = 500;
pub const MAX_DATASET_POINTS: usize = 5_000;
pub const CHART_SIZE: UVec2 = UVec2::new(640, 400);
pub const SHAPE_SIZE: UVec2 = UVec2::new(900, 680);

/// The manifest, or one section of it.
pub fn describe(manifest: &Manifest, req: &DescribeRequest) -> Result<Value, ApiError> {
    let value = serde_json::to_value(manifest).expect("manifest serializes");
    match &req.section {
        None => Ok(value),
        Some(section) => value.get(section).cloned().ok_or_else(|| {
            let sections: Vec<&str> = value
                .as_object()
                .map(|o| o.keys().map(String::as_str).collect())
                .unwrap_or_default();
            ApiError::new(
                "section",
                format!(
                    "unknown section {section:?}; expected one of {}",
                    sections.join(", ")
                ),
            )
        }),
    }
}

/// A dataset as displayed: coordinates in data units (features, principal
/// components, or features plus the target for regression).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetView {
    pub name: String,
    pub task: String,
    pub axis_names: Vec<String>,
    pub class_names: Vec<String>,
    /// One row per sample, one column per axis.
    pub points: Vec<Vec<f64>>,
    /// Class index per sample (classification); empty for regression.
    pub classes: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explained_variance: Option<Vec<f64>>,
    pub total_points: usize,
    /// True when `points` was cut to the first `MAX_DATASET_POINTS`.
    pub truncated: bool,
}

fn build(req: &ExperimentRequest) -> Result<Experiment, ApiError> {
    let spec = ExperimentSpec::try_from(req)?;
    for (i, p) in spec.panes.iter().enumerate() {
        if p.max_steps > MAX_STEPS {
            return Err(ApiError::new(
                format!("panes[{i}].max_steps"),
                format!("at most {MAX_STEPS} per call, got {}", p.max_steps),
            ));
        }
    }
    Experiment::build(spec).map_err(|e| ApiError::new("dataset", e))
}

fn default_trainer(req: &ExperimentRequest) -> TrainerRequest {
    let task = req
        .dataset
        .clone()
        .try_into()
        .map(|d: crate::experiment::DatasetChoice| d.task())
        .unwrap_or(Task::Classification);
    TrainerRequest::for_loss(match task {
        Task::Classification => LossKind::Hinge,
        Task::Regression => LossKind::Mse,
    })
}

impl TryFrom<crate::agent::api::DatasetRequest> for crate::experiment::DatasetChoice {
    type Error = ApiError;
    fn try_from(d: crate::agent::api::DatasetRequest) -> Result<Self, ApiError> {
        Self::try_from(&d)
    }
}

pub fn build_dataset(req: &DatasetViewRequest) -> Result<DatasetView, ApiError> {
    let mut exp = ExperimentRequest {
        dataset: req.dataset.clone(),
        view: req.view.clone(),
        train_on: None,
        panes: vec![],
        seed: req.seed,
    };
    exp.panes.push(default_trainer(&exp));
    let e = build(&exp)?;
    let total = e.points.len();
    // A features view is centred on the data mean internally; give agents
    // the actual feature values. PCA coordinates are centred by nature.
    let offsets: Vec<f64> = match e.projection.as_ref().map(|p| (&p.kind, &p.origin)) {
        Some((bevaru_core::ProjectionKind::Features(cols), origin)) => {
            cols.iter().map(|&c| origin[c]).collect()
        }
        _ => vec![0.0; e.dims],
    };
    let points = e
        .points
        .iter()
        .take(MAX_DATASET_POINTS)
        .map(|p| {
            (0..e.dims)
                .map(|k| p[k] as f64 / e.frame.scale + e.frame.center[k] + offsets[k])
                .collect()
        })
        .collect();
    Ok(DatasetView {
        name: e.name.clone(),
        task: super::task_id(e.task).into(),
        axis_names: e.axis_names.clone(),
        class_names: e.class_names.clone(),
        points,
        classes: if e.task == Task::Classification {
            e.classes.iter().take(MAX_DATASET_POINTS).copied().collect()
        } else {
            Vec::new()
        },
        explained_variance: e.explained_variance().map(<[f64]>::to_vec),
        total_points: total,
        truncated: total > MAX_DATASET_POINTS,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelParams {
    pub weights: Vec<f64>,
    pub bias: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainResult {
    pub model: String,
    pub loss: String,
    /// "converged", "budget-exhausted", or "diverged".
    pub status: String,
    pub steps: usize,
    /// Mean loss plus regularization at the end.
    pub objective: f64,
    pub params: ModelParams,
    /// `(step, objective)`, at most `MAX_TRAJECTORY_POINTS`, always including the last step.
    pub trajectory: Vec<(usize, f64)>,
    /// Classification: fraction of training samples on the correct side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_accuracy: Option<f64>,
    /// Regression: root-mean-square residual on the training data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_rmse: Option<f64>,
    /// SVM: indices of samples on or inside the margin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_vectors: Option<Vec<usize>>,
    /// The full configuration used, defaults filled in.
    pub trainer: TrainerRequest,
}

fn status_id(s: Status) -> &'static str {
    match s {
        Status::Running => "running",
        Status::Converged => "converged",
        Status::BudgetExhausted => "budget-exhausted",
        Status::Diverged => "diverged",
    }
}

fn downsample(points: &[(usize, f64)], max: usize) -> Vec<(usize, f64)> {
    let stride = points.len().div_ceil(max.max(1)).max(1);
    let mut kept: Vec<(usize, f64)> = points.iter().step_by(stride).copied().collect();
    if let (Some(last), Some(kept_last)) = (points.last(), kept.last())
        && last.0 != kept_last.0
    {
        if kept.len() == max {
            kept.pop();
        }
        kept.push(*last);
    }
    kept
}

fn summarize(trainer: &Trainer) -> TrainResult {
    let cfg = trainer.config();
    let latest = trainer.latest();
    let data = trainer.data();
    let f = latest.model.decisions(&data.x);
    let n = data.y.len().max(1) as f64;
    let (accuracy, rmse) = match data.task {
        Task::Classification => {
            let correct = f
                .iter()
                .zip(data.y.iter())
                .filter(|(f, y)| **f * **y > 0.0)
                .count();
            (Some(correct as f64 / n), None)
        }
        Task::Regression => {
            let sq: f64 = f
                .iter()
                .zip(data.y.iter())
                .map(|(f, y)| (f - y).powi(2))
                .sum();
            (None, Some((sq / n).sqrt()))
        }
    };
    TrainResult {
        model: cfg.model.id().into(),
        loss: cfg.loss.id().into(),
        status: status_id(trainer.status()).into(),
        steps: latest.step,
        objective: latest.loss,
        params: ModelParams {
            weights: latest.model.w.iter().copied().collect(),
            bias: latest.model.b,
        },
        trajectory: downsample(&trainer.loss_curve(), MAX_TRAJECTORY_POINTS),
        training_accuracy: accuracy,
        training_rmse: rmse,
        support_vectors: (cfg.model == bevaru_core::ModelKind::Svm)
            .then(|| latest.support_vectors.clone()),
        trainer: TrainerRequest::from_config(cfg),
    }
}

/// Train one model to convergence or its step budget.
pub fn train(req: &TrainRequest) -> Result<TrainResult, ApiError> {
    let mut e = build(&req.experiment()).map_err(|err| {
        // Experiment errors name `panes[0]`; the tool's field is `trainer`.
        ApiError::new(err.field.replacen("panes[0]", "trainer", 1), err.message)
    })?;
    let mut trainer = e.panes.remove(0).trainer;
    trainer.run();
    Ok(summarize(&trainer))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepPoint {
    pub value: f64,
    pub status: String,
    pub steps: usize,
    pub objective: f64,
    pub params: ModelParams,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_accuracy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_rmse: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_vector_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepResult {
    pub parameter: String,
    pub points: Vec<SweepPoint>,
}

/// Train to convergence at every value of one hyperparameter, in parallel.
pub fn sweep(req: &SweepToolRequest) -> Result<SweepResult, ApiError> {
    let spec = SweepSpec::try_from(&req.sweep)
        .map_err(|e| ApiError::new(format!("sweep.{}", e.field), e.message))?;
    let e = build(&req.train.experiment())
        .map_err(|err| ApiError::new(err.field.replacen("panes[0]", "trainer", 1), err.message))?;
    let base = e.panes[0].trainer.config().clone();
    if !spec.param.applies_to(&base) {
        let manifest_entry = super::sweep_info(spec.param);
        return Err(ApiError::new(
            "sweep.parameter",
            format!(
                "{} applies to {}, but this trainer uses {}",
                spec.param.id(),
                manifest_entry.applies_to.join(", "),
                base.loss.id()
            ),
        ));
    }
    let configs: Vec<(f64, TrainerConfig)> = spec
        .values()
        .into_iter()
        .map(|v| {
            spec.param
                .apply(&base, v)
                .map(|cfg| (v, cfg))
                .map_err(|msg| ApiError::new("sweep", format!("at {v}: {msg}")))
        })
        .collect::<Result<_, _>>()?;
    let data = e.data.clone();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = configs.len().div_ceil(threads).max(1);
    let points: Vec<SweepPoint> = std::thread::scope(|s| {
        let handles: Vec<_> = configs
            .chunks(chunk)
            .map(|batch| {
                let data = data.clone();
                s.spawn(move || {
                    batch
                        .iter()
                        .map(|(value, cfg)| {
                            let mut t = Trainer::new(cfg.clone(), data.clone())
                                .expect("configuration validated above");
                            t.run();
                            let r = summarize(&t);
                            SweepPoint {
                                value: *value,
                                status: r.status,
                                steps: r.steps,
                                objective: r.objective,
                                params: r.params,
                                training_accuracy: r.training_accuracy,
                                training_rmse: r.training_rmse,
                                support_vector_count: r.support_vectors.map(|v| v.len()),
                            }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("sweep worker panicked"))
            .collect()
    });
    Ok(SweepResult {
        parameter: spec.param.id().into(),
        points,
    })
}

fn png(chart: &LineChart) -> Result<Vec<u8>, ApiError> {
    let img = chart
        .render(CHART_SIZE)
        .map_err(|e| ApiError::new("", format!("rendering failed: {e}")))?;
    let rgba = image::RgbaImage::from_raw(img.width, img.height, img.pixels)
        .ok_or_else(|| ApiError::new("", "renderer returned a malformed image"))?;
    let mut out = std::io::Cursor::new(Vec::new());
    rgba.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| ApiError::new("", format!("PNG encoding failed: {e}")))?;
    Ok(out.into_inner())
}

/// Losses against their argument, as a PNG.
pub fn render_loss_chart(req: &LossChartRequest) -> Result<Vec<u8>, ApiError> {
    if req.losses.is_empty() {
        return Err(ApiError::new("losses", "list at least one loss"));
    }
    let losses = req
        .losses
        .iter()
        .enumerate()
        .map(|(i, id)| {
            LossKind::from_id(id).ok_or_else(|| {
                ApiError::new(format!("losses[{i}]"), format!("unknown loss {id:?}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let task = losses[0].task();
    if let Some(i) = losses.iter().position(|l| l.task() != task) {
        return Err(ApiError::new(
            format!("losses[{i}]"),
            format!(
                "{} is a function of the {}, but {} is a function of the {}; chart them separately",
                losses[i].id(),
                argument(losses[i]),
                losses[0].id(),
                argument(losses[0])
            ),
        ));
    }
    let mut params = LossParams::default();
    if let Some(d) = req.huber_delta {
        params = params
            .with_huber_delta(d)
            .map_err(|e| ApiError::new("huber_delta", e.to_string()))?;
    }
    if let Some(m) = req.margin {
        params = params
            .with_margin(m)
            .map_err(|e| ApiError::new("margin", e.to_string()))?;
    }
    let range = req.range.unwrap_or(3.0);
    if !(range.is_finite() && range > 0.0) {
        return Err(ApiError::new(
            "range",
            format!("must be finite and > 0, got {range}"),
        ));
    }
    png(&loss_curve_chart(task, &losses, &params, &[], range))
}

/// One input axis of a sampled loss shape: its sample coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeAxisSamples {
    pub symbol: String,
    pub name: String,
    pub values: Vec<f64>,
}

/// The slice of a surface that is the familiar 2-D loss curve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeSlice {
    pub description: String,
    /// What the 2-D curve is a function of: "residual" or "margin".
    pub argument: String,
    /// `[x, y, argument, loss]` along the slice: where it lies on the
    /// surface, and the 2-D curve's argument and value there.
    pub points: Vec<[f64; 4]>,
}

/// A loss-shape view sampled on a grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LossShapeSample {
    pub view: String,
    pub title: String,
    pub caption: String,
    pub x: ShapeAxisSamples,
    pub y: ShapeAxisSamples,
    pub height_label: String,
    /// `values[row][column]` is the loss at `(x.values[column], y.values[row])`;
    /// `null` off the domain (outside the probability triangle).
    pub values: Vec<Vec<Option<f64>>>,
    /// `[row, column]` of every value capped at `cap`.
    pub clipped: Vec<[usize; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap: Option<f64>,
    pub min: f64,
    pub max: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slice: Option<ShapeSlice>,
}

fn sample_grid(query: &ShapeQuery) -> Result<SurfaceGrid, ApiError> {
    query
        .view
        .sample(&query.params, query.resolution)
        .map_err(|e| ApiError::new("", e.to_string()))
}

fn axis_values(a: &bevaru_core::shapes::Axis) -> Vec<f64> {
    (0..a.n).map(|i| a.at(i)).collect()
}

/// Sample a loss-shape view: every value is the library's loss at that point.
pub fn sample_loss_shape(req: &LossShapeRequest) -> Result<LossShapeSample, ApiError> {
    let query = ShapeQuery::try_from(req)?;
    let grid = sample_grid(&query)?;
    let view = query.view;
    let (nx, ny) = (grid.x.n, grid.y.n);
    let values = (0..ny)
        .map(|row| (0..nx).map(|column| grid.value(column, row)).collect())
        .collect();
    let clipped = (0..ny)
        .flat_map(|row| (0..nx).map(move |column| [row, column]))
        .filter(|&[row, column]| grid.is_clipped(column, row))
        .collect();
    let slice = view.slice().and_then(|s| {
        Some(ShapeSlice {
            description: super::slice_description(s).into(),
            argument: argument(*view.losses().first()?).into(),
            points: view.slice_points(&query.params, query.resolution)?,
        })
    });
    Ok(LossShapeSample {
        view: view.id().into(),
        title: view.title().into(),
        caption: view.caption().into(),
        x: ShapeAxisSamples {
            symbol: grid.x.symbol.clone(),
            name: grid.x.name.clone(),
            values: axis_values(&grid.x),
        },
        y: ShapeAxisSamples {
            symbol: grid.y.symbol.clone(),
            name: grid.y.name.clone(),
            values: axis_values(&grid.y),
        },
        height_label: grid.height_label.clone(),
        values,
        clipped,
        cap: grid.cap,
        min: grid.min,
        max: grid.max,
        slice,
    })
}

/// Render a loss-shape view as a 3-D surface PNG with ruviz, coloured with
/// the interactive view's cool-to-warm colormap. Off-domain samples (outside
/// the probability triangle) are gaps, so that view keeps a triangular base.
pub fn render_loss_shape(req: &LossShapeRenderRequest) -> Result<Vec<u8>, ApiError> {
    let query = ShapeQuery::try_from(&req.shape())?;
    let azimuth = req.azimuth.unwrap_or(-60.0);
    let elevation = req.elevation.unwrap_or(30.0);
    if !azimuth.is_finite() {
        return Err(ApiError::new(
            "azimuth",
            format!("must be finite, got {azimuth}"),
        ));
    }
    if !(elevation.is_finite() && (-90.0..=90.0).contains(&elevation)) {
        return Err(ApiError::new(
            "elevation",
            format!("must be −90 to 90, got {elevation}"),
        ));
    }
    let grid = sample_grid(&query)?;
    let (x, y) = (axis_values(&grid.x), axis_values(&grid.y));
    let triangle = query.view == ShapeView::ThreeClassProbabilities;
    // ruviz drops every cell that touches a gap. On the triangle, the cells
    // along the edge q₁ = 0 would then leave lone spikes up to the cap; the
    // loss there is unbounded, so samples within one cell beyond the edge
    // are drawn at the cap, making the edge a solid wall.
    let step = (grid.x.max - grid.x.min) / (grid.x.n - 1) as f64;
    let z: Vec<Vec<f64>> = (0..grid.y.n)
        .map(|row| {
            (0..grid.x.n)
                .map(|column| match (grid.value(column, row), grid.cap) {
                    (Some(v), _) => v,
                    (None, Some(cap)) if triangle && x[column] + y[row] <= 1.0 + 1.001 * step => {
                        cap
                    }
                    (None, _) => f64::NAN,
                })
                .collect()
        })
        .collect();
    let mut title = if triangle {
        // Drawn on its right-triangle base, so say what the third axis is.
        "Three-class cross-entropy, q₁ = 1 − q₂ − q₃".to_string()
    } else {
        query.view.title().to_string()
    };
    if let Some(cap) = grid.cap.filter(|_| grid.clipped.iter().any(|&c| c)) {
        title.push_str(&format!(" (clipped at {cap})"));
    }
    let zlabel = grid.height_label.clone();
    let top = if grid.max > grid.min {
        grid.max
    } else {
        grid.min + 1.0
    };
    ruviz::surface(&x, &y, &z)
        .cmap(ruviz::render::ColorMap::coolwarm())
        .title(title)
        .xlabel(format!("{} — {}", grid.x.symbol, grid.x.name))
        .ylabel(format!("{} — {}", grid.y.symbol, grid.y.name))
        .zlabel(zlabel)
        .zlim(grid.min, top)
        .azimuth_deg(azimuth as f32)
        .elevation_deg(elevation as f32)
        .size_px(SHAPE_SIZE.x, SHAPE_SIZE.y)
        .render_png_bytes()
        .map_err(|e| ApiError::new("", format!("rendering failed: {e}")))
}

/// Train one model and chart its objective by step, as a PNG.
pub fn render_training_chart(req: &TrainRequest) -> Result<Vec<u8>, ApiError> {
    let result = train(req)?;
    let (x, y): (Vec<f64>, Vec<f64>) = result
        .trajectory
        .iter()
        .map(|&(s, l)| (s as f64, l))
        .unzip();
    png(&LineChart {
        title: format!("Training objective · {} ({})", result.loss, result.status),
        xlabel: "step".into(),
        ylabel: "mean loss + regularization".into(),
        series: vec![Series {
            label: result.loss.clone(),
            x,
            y,
            color: [0, 114, 178],
            width: 2.2,
        }],
        marker: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::api::{DatasetRequest, SweepRequest};

    fn blobs_svm() -> TrainRequest {
        let mut trainer = TrainerRequest::for_loss(LossKind::Hinge);
        trainer.c = Some(1.0);
        trainer.max_steps = Some(200);
        TrainRequest {
            dataset: DatasetRequest::SeparableBlobs,
            view: None,
            train_on: None,
            trainer,
            seed: None,
        }
    }

    #[test]
    fn training_an_svm_reports_trajectory_params_and_support_vectors() {
        let r = train(&blobs_svm()).unwrap();
        assert_eq!(r.model, "svm");
        assert!(r.steps <= 200 && r.steps > 0);
        assert!(r.trajectory.len() <= MAX_TRAJECTORY_POINTS);
        assert_eq!(r.trajectory.last().unwrap().0, r.steps);
        assert_eq!(r.params.weights.len(), 2);
        assert!(!r.support_vectors.unwrap().is_empty());
        assert!(r.training_accuracy.unwrap() > 0.95);
        assert_eq!(r.trainer.c, Some(1.0));
    }

    #[test]
    fn step_budget_is_capped() {
        let mut req = blobs_svm();
        req.trainer.max_steps = Some(MAX_STEPS + 1);
        let err = train(&req).unwrap_err();
        assert_eq!(err.field, "trainer.max_steps");
    }

    #[test]
    fn invalid_hyperparameter_names_the_field() {
        let mut req = blobs_svm();
        req.trainer = TrainerRequest::for_loss(LossKind::Huber);
        req.trainer.huber_delta = Some(0.0);
        req.dataset = DatasetRequest::Regression {
            features: 1,
            outlier_fraction: 0.1,
        };
        let err = train(&req).unwrap_err();
        assert_eq!(err.field, "trainer.huber_delta");
    }

    #[test]
    fn sweep_returns_one_solution_per_value_and_rejects_inapplicable_parameters() {
        let req = SweepToolRequest {
            train: blobs_svm(),
            sweep: SweepRequest {
                parameter: "c".into(),
                from: 0.01,
                to: 100.0,
                samples: 5,
                log: true,
            },
        };
        let r = sweep(&req).unwrap();
        assert_eq!(r.points.len(), 5);
        let norm = |p: &SweepPoint| p.params.weights.iter().map(|w| w * w).sum::<f64>().sqrt();
        assert!(norm(&r.points[4]) > norm(&r.points[0]), "‖w‖ grows with C");

        let bad = SweepToolRequest {
            sweep: SweepRequest {
                parameter: "huber-delta".into(),
                ..req.sweep.clone()
            },
            ..req
        };
        let err = sweep(&bad).unwrap_err();
        assert_eq!(err.field, "sweep.parameter");
        assert!(err.message.contains("huber"), "{err}");
    }

    #[test]
    fn dataset_view_returns_display_coordinates() {
        let v = build_dataset(&DatasetViewRequest {
            dataset: DatasetRequest::Iris {
                positive: "setosa".into(),
                negative: None,
            },
            view: Some(crate::agent::api::ViewRequest::Features {
                columns: vec![2, 3],
            }),
            seed: None,
        })
        .unwrap();
        assert_eq!(v.total_points, 150);
        assert_eq!(v.axis_names, ["petal length (cm)", "petal width (cm)"]);
        // The first Iris sample: petal length 1.4, petal width 0.2.
        assert!(
            (v.points[0][0] - 1.4).abs() < 1e-4 && (v.points[0][1] - 0.2).abs() < 1e-4,
            "{:?}",
            v.points[0]
        );
        assert_eq!(v.classes.len(), 150);
    }

    #[test]
    fn charts_are_png_and_mixed_tasks_are_rejected() {
        let png_bytes = render_loss_chart(&LossChartRequest {
            losses: vec!["hinge".into(), "logistic".into()],
            huber_delta: None,
            margin: None,
            range: None,
        })
        .unwrap();
        assert_eq!(&png_bytes[..8], b"\x89PNG\r\n\x1a\n");
        let err = render_loss_chart(&LossChartRequest {
            losses: vec!["hinge".into(), "mse".into()],
            huber_delta: None,
            margin: None,
            range: None,
        })
        .unwrap_err();
        assert_eq!(err.field, "losses[1]");
        let training = render_training_chart(&blobs_svm()).unwrap();
        assert_eq!(&training[..4], b"\x89PNG");
    }

    #[test]
    fn describe_returns_sections() {
        let m = super::super::builtin_manifest();
        let losses = describe(
            &m,
            &DescribeRequest {
                section: Some("losses".into()),
            },
        )
        .unwrap();
        assert_eq!(losses.as_array().unwrap().len(), LossKind::ALL.len());
        let err = describe(
            &m,
            &DescribeRequest {
                section: Some("nope".into()),
            },
        )
        .unwrap_err();
        assert!(err.message.contains("experiences"));
    }

    #[test]
    fn downsample_caps_and_keeps_the_last_point() {
        let pts: Vec<(usize, f64)> = (0..=2000).map(|i| (i, i as f64)).collect();
        let d = downsample(&pts, 500);
        assert!(d.len() <= 500);
        assert_eq!(d.last().unwrap().0, 2000);
    }

    fn shape(view: &str) -> LossShapeRequest {
        LossShapeRequest {
            view: view.into(),
            huber_delta: None,
            margin: None,
            resolution: None,
            entropy_removed: None,
        }
    }

    fn render(view: &str) -> LossShapeRenderRequest {
        LossShapeRenderRequest {
            view: view.into(),
            huber_delta: None,
            margin: None,
            resolution: None,
            entropy_removed: None,
            azimuth: None,
            elevation: None,
        }
    }

    #[test]
    fn cross_entropy_sample_has_the_entropy_valley() {
        let s = sample_loss_shape(&LossShapeRequest {
            resolution: Some(21),
            ..shape("probability-vs-truth-cross-entropy")
        })
        .unwrap();
        assert_eq!((s.x.values.len(), s.y.values.len()), (21, 21));
        assert_eq!(s.values.len(), 21);
        assert!(s.values.iter().all(|r| r.len() == 21));
        assert_eq!(s.cap, Some(8.0));
        assert!(!s.clipped.is_empty() && !s.caption.is_empty());
        // Along q = p the value is H(p). The q axis starts at ε, so compare
        // where the two axes' samples coincide in value.
        let mut checked = 0;
        for (column, &p) in s.x.values.iter().enumerate() {
            for (row, &q) in s.y.values.iter().enumerate() {
                if (p - q).abs() < 1e-12 {
                    let h = bevaru_core::shapes::entropy(p);
                    assert!((s.values[row][column].unwrap() - h).abs() < 1e-9);
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "some samples lie on q = p");
        let slice = s.slice.unwrap();
        assert_eq!(slice.argument, "margin");
        assert_eq!(slice.points.len(), 21);
    }

    #[test]
    fn shape_requests_name_the_bad_field() {
        let err = sample_loss_shape(&shape("nope")).unwrap_err();
        assert_eq!(err.field, "view");
        assert!(err.message.contains("two-scores-hinge"));
        let err = sample_loss_shape(&LossShapeRequest {
            resolution: Some(102),
            ..shape("two-scores-hinge")
        })
        .unwrap_err();
        assert_eq!(err.field, "resolution");
        let err = sample_loss_shape(&LossShapeRequest {
            margin: Some(-1.0),
            ..shape("two-scores-hinge")
        })
        .unwrap_err();
        assert_eq!(err.field, "margin");
        let err = sample_loss_shape(&LossShapeRequest {
            entropy_removed: Some(true),
            ..shape("two-scores-hinge")
        })
        .unwrap_err();
        assert_eq!(err.field, "entropy_removed");
        let err = render_loss_shape(&LossShapeRenderRequest {
            elevation: Some(120.0),
            ..render("two-scores-hinge")
        })
        .unwrap_err();
        assert_eq!(err.field, "elevation");
    }

    #[test]
    fn triangle_samples_are_null_off_the_simplex() {
        let s = sample_loss_shape(&LossShapeRequest {
            resolution: Some(11),
            ..shape("three-class-probabilities")
        })
        .unwrap();
        for (row, &q3) in s.y.values.iter().enumerate() {
            for (column, &q2) in s.x.values.iter().enumerate() {
                let on = q2 + q3 <= 1.0 + 1e-9;
                assert_eq!(s.values[row][column].is_some(), on, "({q2}, {q3})");
            }
        }
    }

    #[test]
    fn loss_shapes_render_headlessly_as_png() {
        for view in [
            "probability-vs-truth-cross-entropy",
            "three-class-probabilities",
        ] {
            let png = render_loss_shape(&LossShapeRenderRequest {
                resolution: Some(31),
                ..render(view)
            })
            .unwrap();
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "{view}");
            if let Some(dir) = std::env::var_os("BEVARU_DUMP_SHAPES") {
                std::fs::write(std::path::Path::new(&dir).join(format!("{view}.png")), &png)
                    .unwrap();
            }
        }
    }
}
