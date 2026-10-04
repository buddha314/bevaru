//! The wire format agents use to talk to bevaru (MCP tools, remote methods).
//!
//! These types are deliberately separate from the library's internal types:
//! the wire format stays stable while internals change, and every request is
//! validated field by field on the way in ([`TryFrom`]). Each type derives
//! `JsonSchema`, and its doc comments become the schema's descriptions.

use std::fmt;

use bevaru_core::{LearningRate, LossKind, LossParams, ModelKind, TrainerConfig};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::experiment::{DatasetChoice, ExperimentSpec, MAX_PANES, TrainSpace, View};
use crate::playback::{SweepParam, SweepSpec};

/// A request field that failed validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiError {
    /// Dotted path to the field, e.g. `panes[0].huber_delta`.
    pub field: String,
    pub message: String,
}

impl ApiError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }

    fn within(mut self, prefix: &str) -> Self {
        self.field = if self.field.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix}.{}", self.field)
        };
        self
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ApiError {}

fn positive(field: &str, v: f64) -> Result<f64, ApiError> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(ApiError::new(
            field,
            format!("must be finite and > 0, got {v}"),
        ))
    }
}

/// Which data to use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DatasetRequest {
    /// Two 2-D Gaussian classes, guaranteed linearly separable.
    SeparableBlobs,
    /// Two 2-D Gaussian classes that overlap.
    OverlappingBlobs,
    /// Linear regression data, y = w·x + b + noise, with outliers pushed far above the trend.
    Regression {
        /// Number of input features: 1 (fit line) or 2 (fit plane).
        features: usize,
        /// Fraction of points that are outliers, 0 to 0.4.
        outlier_fraction: f64,
    },
    /// Fisher's Iris data reduced to a binary task.
    Iris {
        /// Positive class: "setosa", "versicolor" or "virginica".
        positive: String,
        /// Negative class; omit for one-vs-rest.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        negative: Option<String>,
    },
    /// Two MNIST digits (needs the `mnist` feature; downloads ~11 MB once).
    Mnist {
        positive: u8,
        negative: u8,
        /// Samples to use, 50 to 12000.
        samples: usize,
    },
}

/// How to project the data for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ViewRequest {
    /// Two or three feature columns, by index.
    Features { columns: Vec<usize> },
    /// The top 2 or 3 principal components.
    Pca { components: usize },
}

/// Where models train when the display is a projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TrainOn {
    /// Every feature; the drawn boundary is a slice through the display plane.
    AllFeatures,
    /// Only the displayed axes; the drawn boundary is exact.
    DisplayedAxes,
}

/// A step-size schedule: `initial / (1 + decay · step)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LearningRateRequest {
    /// Step size at step 0 (> 0).
    pub initial: f64,
    /// Decay rate (≥ 0); 0 keeps the step size constant.
    #[serde(default)]
    pub decay: f64,
}

/// One trainer. The loss chooses the model: hinge and squared-hinge train a
/// linear SVM, logistic trains logistic regression, and mse / mae / huber
/// train linear regression. Omitted fields use that model's defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrainerRequest {
    /// Loss id: mse, mae, huber, hinge, squared-hinge, logistic.
    pub loss: String,
    /// SVM soft-margin C (> 0). SVM only; sets λ = 1/C and a matching step size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<f64>,
    /// L2 regularization λ (≥ 0), bias excluded. For an SVM, prefer `c`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda: Option<f64>,
    /// Huber δ (> 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub huber_delta: Option<f64>,
    /// Hinge margin (> 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_rate: Option<LearningRateRequest>,
    /// Mini-batch size (≥ 1); omit for full-batch descent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<usize>,
    /// Step budget (≥ 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<usize>,
    /// Relative loss change counted as "calm" for convergence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
    /// Consecutive calm steps that count as converged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patience: Option<usize>,
    /// Seed for mini-batch sampling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl TrainerRequest {
    /// Defaults for `loss`, nothing overridden.
    pub fn for_loss(loss: LossKind) -> Self {
        Self {
            loss: loss.id().to_string(),
            c: None,
            lambda: None,
            huber_delta: None,
            margin: None,
            learning_rate: None,
            batch_size: None,
            max_steps: None,
            tolerance: None,
            patience: None,
            seed: None,
        }
    }

    /// The request that reproduces `config` exactly.
    pub fn from_config(config: &TrainerConfig) -> Self {
        let (initial, decay) = match config.learning_rate {
            LearningRate::Constant(eta) => (eta, 0.0),
            LearningRate::InverseDecay { initial, decay } => (initial, decay),
        };
        let svm = config.model == ModelKind::Svm;
        Self {
            loss: config.loss.id().to_string(),
            c: if svm { config.c() } else { None },
            lambda: (!svm).then_some(config.lambda),
            huber_delta: Some(config.loss_params.huber_delta()),
            margin: Some(config.loss_params.margin()),
            learning_rate: Some(LearningRateRequest { initial, decay }),
            batch_size: config.batch_size,
            max_steps: Some(config.max_steps),
            tolerance: Some(config.tolerance),
            patience: Some(config.patience),
            seed: Some(config.seed),
        }
    }
}

impl TryFrom<&TrainerRequest> for TrainerConfig {
    type Error = ApiError;

    fn try_from(r: &TrainerRequest) -> Result<Self, ApiError> {
        let loss = LossKind::from_id(&r.loss).ok_or_else(|| {
            ApiError::new(
                "loss",
                format!(
                    "unknown loss {:?}; expected one of {}",
                    r.loss,
                    trainable_loss_ids().join(", ")
                ),
            )
        })?;
        let model = ModelKind::for_loss(loss).ok_or_else(|| {
            ApiError::new(
                "loss",
                format!("{} cannot be trained on (no useful gradient)", loss.id()),
            )
        })?;
        let mut config = match model {
            ModelKind::Svm => TrainerConfig::svm(1.0).expect("C = 1 is valid"),
            ModelKind::LogisticRegression => TrainerConfig::logistic(),
            ModelKind::LinearRegression => TrainerConfig::regression(loss),
        };
        config.loss = loss;
        if let Some(c) = r.c {
            if model != ModelKind::Svm {
                return Err(ApiError::new(
                    "c",
                    format!("C applies only to SVM losses, not {}", loss.id()),
                ));
            }
            config = config
                .with_c(positive("c", c)?)
                .map_err(|e| ApiError::new("c", e.to_string()))?;
        }
        if let Some(lambda) = r.lambda {
            if !(lambda.is_finite() && lambda >= 0.0) {
                return Err(ApiError::new(
                    "lambda",
                    format!("must be finite and ≥ 0, got {lambda}"),
                ));
            }
            config.lambda = lambda;
        }
        let mut params = LossParams::default();
        if let Some(d) = r.huber_delta {
            params = params
                .with_huber_delta(d)
                .map_err(|e| ApiError::new("huber_delta", e.to_string()))?;
        }
        if let Some(m) = r.margin {
            params = params
                .with_margin(m)
                .map_err(|e| ApiError::new("margin", e.to_string()))?;
        }
        config.loss_params = params;
        if let Some(lr) = r.learning_rate {
            positive("learning_rate.initial", lr.initial)?;
            if !(lr.decay.is_finite() && lr.decay >= 0.0) {
                return Err(ApiError::new(
                    "learning_rate.decay",
                    format!("must be finite and ≥ 0, got {}", lr.decay),
                ));
            }
            config.learning_rate = if lr.decay == 0.0 {
                LearningRate::Constant(lr.initial)
            } else {
                LearningRate::InverseDecay {
                    initial: lr.initial,
                    decay: lr.decay,
                }
            };
        }
        if let Some(b) = r.batch_size {
            if b == 0 {
                return Err(ApiError::new("batch_size", "must be ≥ 1"));
            }
            config.batch_size = Some(b);
        }
        if let Some(n) = r.max_steps {
            if n == 0 {
                return Err(ApiError::new("max_steps", "must be ≥ 1"));
            }
            config.max_steps = n;
        }
        if let Some(t) = r.tolerance {
            if !(t.is_finite() && t >= 0.0) {
                return Err(ApiError::new(
                    "tolerance",
                    format!("must be finite and ≥ 0, got {t}"),
                ));
            }
            config.tolerance = t;
        }
        if let Some(p) = r.patience {
            config.patience = p;
        }
        if let Some(s) = r.seed {
            config.seed = s;
        }
        config
            .validate(model.task())
            .map_err(|e| ApiError::new("", e.to_string()))?;
        Ok(config)
    }
}

/// Ids of the losses a trainer accepts.
pub fn trainable_loss_ids() -> Vec<&'static str> {
    LossKind::ALL
        .into_iter()
        .filter(|l| l.is_trainable())
        .map(LossKind::id)
        .collect()
}

/// A dataset, a view of it, and one trainer per side-by-side pane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExperimentRequest {
    pub dataset: DatasetRequest,
    /// Omit for the dataset's natural view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub train_on: Option<TrainOn>,
    /// One to three trainers, compared side by side on the same data.
    pub panes: Vec<TrainerRequest>,
    /// Seed for data generation and subsampling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl ExperimentRequest {
    /// The request that reproduces `spec` exactly.
    pub fn from_spec(spec: &ExperimentSpec) -> Self {
        let dataset = match &spec.dataset {
            DatasetChoice::SeparableBlobs => DatasetRequest::SeparableBlobs,
            DatasetChoice::OverlappingBlobs => DatasetRequest::OverlappingBlobs,
            DatasetChoice::Regression { dims, outliers } => DatasetRequest::Regression {
                features: *dims,
                outlier_fraction: *outliers,
            },
            DatasetChoice::Iris { positive, negative } => DatasetRequest::Iris {
                positive: positive.clone(),
                negative: negative.clone(),
            },
            #[cfg(feature = "mnist")]
            DatasetChoice::Mnist {
                positive,
                negative,
                samples,
            } => DatasetRequest::Mnist {
                positive: *positive,
                negative: *negative,
                samples: *samples,
            },
        };
        let view = match &spec.view {
            View::Features(columns) => ViewRequest::Features {
                columns: columns.clone(),
            },
            View::Pca(k) => ViewRequest::Pca { components: *k },
        };
        Self {
            dataset,
            view: Some(view),
            train_on: Some(match spec.train_space {
                TrainSpace::Full => TrainOn::AllFeatures,
                TrainSpace::Projected => TrainOn::DisplayedAxes,
            }),
            panes: spec.panes.iter().map(TrainerRequest::from_config).collect(),
            seed: Some(spec.seed),
        }
    }
}

impl TryFrom<&DatasetRequest> for DatasetChoice {
    type Error = ApiError;

    fn try_from(d: &DatasetRequest) -> Result<Self, ApiError> {
        Ok(match d {
            DatasetRequest::SeparableBlobs => DatasetChoice::SeparableBlobs,
            DatasetRequest::OverlappingBlobs => DatasetChoice::OverlappingBlobs,
            DatasetRequest::Regression {
                features,
                outlier_fraction,
            } => {
                if !(1..=2).contains(features) {
                    return Err(ApiError::new(
                        "features",
                        format!("must be 1 or 2, got {features}"),
                    ));
                }
                if !(0.0..=0.4).contains(outlier_fraction) {
                    return Err(ApiError::new(
                        "outlier_fraction",
                        format!("must be between 0 and 0.4, got {outlier_fraction}"),
                    ));
                }
                DatasetChoice::Regression {
                    dims: *features,
                    outliers: *outlier_fraction,
                }
            }
            DatasetRequest::Iris { positive, negative } => {
                const CLASSES: [&str; 3] = ["setosa", "versicolor", "virginica"];
                let check = |field: &str, name: &str| {
                    if CLASSES.contains(&name) {
                        Ok(())
                    } else {
                        Err(ApiError::new(
                            field,
                            format!(
                                "unknown Iris class {name:?}; expected one of {}",
                                CLASSES.join(", ")
                            ),
                        ))
                    }
                };
                check("positive", positive)?;
                if let Some(n) = negative {
                    check("negative", n)?;
                    if n == positive {
                        return Err(ApiError::new("negative", "must differ from positive"));
                    }
                }
                DatasetChoice::Iris {
                    positive: positive.clone(),
                    negative: negative.clone(),
                }
            }
            #[cfg(feature = "mnist")]
            DatasetRequest::Mnist {
                positive,
                negative,
                samples,
            } => {
                for (field, d) in [("positive", positive), ("negative", negative)] {
                    if *d > 9 {
                        return Err(ApiError::new(
                            field,
                            format!("must be a digit 0-9, got {d}"),
                        ));
                    }
                }
                if positive == negative {
                    return Err(ApiError::new("negative", "must differ from positive"));
                }
                if !(50..=12_000).contains(samples) {
                    return Err(ApiError::new(
                        "samples",
                        format!("must be 50 to 12000, got {samples}"),
                    ));
                }
                DatasetChoice::Mnist {
                    positive: *positive,
                    negative: *negative,
                    samples: *samples,
                }
            }
            #[cfg(not(feature = "mnist"))]
            DatasetRequest::Mnist { .. } => {
                return Err(ApiError::new(
                    "kind",
                    "mnist needs the `mnist` feature; rebuild with --features mnist",
                ));
            }
        })
    }
}

impl TryFrom<&ExperimentRequest> for ExperimentSpec {
    type Error = ApiError;

    fn try_from(r: &ExperimentRequest) -> Result<Self, ApiError> {
        let dataset = DatasetChoice::try_from(&r.dataset).map_err(|e| e.within("dataset"))?;
        if r.panes.is_empty() || r.panes.len() > MAX_PANES {
            return Err(ApiError::new(
                "panes",
                format!("need 1 to {MAX_PANES} trainers, got {}", r.panes.len()),
            ));
        }
        let panes = r
            .panes
            .iter()
            .enumerate()
            .map(|(i, p)| TrainerConfig::try_from(p).map_err(|e| e.within(&format!("panes[{i}]"))))
            .collect::<Result<Vec<_>, _>>()?;
        let task = dataset.task();
        if let Some(i) = panes.iter().position(|p| p.model.task() != task) {
            return Err(ApiError::new(
                format!("panes[{i}].loss"),
                format!(
                    "{} is not a loss for this dataset's task",
                    panes[i].loss.id()
                ),
            ));
        }
        let view = match &r.view {
            None => dataset.default_view(),
            Some(ViewRequest::Features { columns }) => {
                if !(2..=3).contains(&columns.len()) {
                    return Err(ApiError::new(
                        "view.columns",
                        "must list 2 or 3 feature columns",
                    ));
                }
                View::Features(columns.clone())
            }
            Some(ViewRequest::Pca { components }) => {
                if !(2..=3).contains(components) {
                    return Err(ApiError::new("view.components", "must be 2 or 3"));
                }
                View::Pca(*components)
            }
        };
        Ok(ExperimentSpec {
            dataset,
            view,
            train_space: match r.train_on {
                Some(TrainOn::DisplayedAxes) => TrainSpace::Projected,
                _ => TrainSpace::Full,
            },
            panes,
            seed: r.seed.unwrap_or(1),
        })
    }
}

/// A hyperparameter sweep: each value trained to convergence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SweepRequest {
    /// Parameter id: c, lambda, huber-delta, margin, learning-rate.
    pub parameter: String,
    pub from: f64,
    pub to: f64,
    /// Number of values, 2 to 50.
    pub samples: usize,
    /// Space values logarithmically (both ends must be > 0).
    #[serde(default)]
    pub log: bool,
}

impl TryFrom<&SweepRequest> for SweepSpec {
    type Error = ApiError;

    fn try_from(r: &SweepRequest) -> Result<Self, ApiError> {
        let param = SweepParam::from_id(&r.parameter).ok_or_else(|| {
            let ids: Vec<&str> = SweepParam::ALL.into_iter().map(SweepParam::id).collect();
            ApiError::new(
                "parameter",
                format!(
                    "unknown parameter {:?}; expected one of {}",
                    r.parameter,
                    ids.join(", ")
                ),
            )
        })?;
        if !(2..=50).contains(&r.samples) {
            return Err(ApiError::new(
                "samples",
                format!("must be 2 to 50, got {}", r.samples),
            ));
        }
        for (field, v) in [("from", r.from), ("to", r.to)] {
            if !v.is_finite() {
                return Err(ApiError::new(field, "must be finite"));
            }
            if r.log && v <= 0.0 {
                return Err(ApiError::new(field, "must be > 0 for a log sweep"));
            }
        }
        Ok(SweepSpec {
            param,
            from: r.from,
            to: r.to,
            samples: r.samples,
            log: r.log,
        })
    }
}

/// Evaluate losses pointwise: at residuals (regression losses) or margins
/// (classification losses).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LossEvalRequest {
    /// Loss ids, e.g. ["hinge", "logistic"]. Includes zero-one.
    pub losses: Vec<String>,
    /// Points at which to evaluate, at most 10000.
    pub points: Vec<f64>,
    /// Huber δ (> 0), default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub huber_delta: Option<f64>,
    /// Hinge margin (> 0), default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
}

/// Values and (sub)gradients for one loss.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LossEvaluation {
    pub loss: String,
    /// "residual" or "margin".
    pub argument: String,
    pub values: Vec<f64>,
    pub gradients: Vec<f64>,
}

/// Evaluate a [`LossEvalRequest`].
pub fn evaluate_losses(r: &LossEvalRequest) -> Result<Vec<LossEvaluation>, ApiError> {
    if r.points.len() > 10_000 {
        return Err(ApiError::new(
            "points",
            format!("at most 10000 points, got {}", r.points.len()),
        ));
    }
    if let Some(i) = r.points.iter().position(|p| !p.is_finite()) {
        return Err(ApiError::new(format!("points[{i}]"), "must be finite"));
    }
    let mut params = LossParams::default();
    if let Some(d) = r.huber_delta {
        params = params
            .with_huber_delta(d)
            .map_err(|e| ApiError::new("huber_delta", e.to_string()))?;
    }
    if let Some(m) = r.margin {
        params = params
            .with_margin(m)
            .map_err(|e| ApiError::new("margin", e.to_string()))?;
    }
    r.losses
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let loss = LossKind::from_id(id).ok_or_else(|| {
                let ids: Vec<&str> = LossKind::ALL.into_iter().map(LossKind::id).collect();
                ApiError::new(
                    format!("losses[{i}]"),
                    format!("unknown loss {id:?}; expected one of {}", ids.join(", ")),
                )
            })?;
            Ok(LossEvaluation {
                loss: loss.id().to_string(),
                argument: super::argument(loss).to_string(),
                values: r.points.iter().map(|&x| loss.value(x, &params)).collect(),
                gradients: r.points.iter().map(|&x| loss.grad(x, &params)).collect(),
            })
        })
        .collect()
}

/// Train one model on a dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrainRequest {
    pub dataset: DatasetRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub train_on: Option<TrainOn>,
    pub trainer: TrainerRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl TrainRequest {
    /// As a one-pane experiment.
    pub fn experiment(&self) -> ExperimentRequest {
        ExperimentRequest {
            dataset: self.dataset.clone(),
            view: self.view.clone(),
            train_on: self.train_on,
            panes: vec![self.trainer.clone()],
            seed: self.seed,
        }
    }
}

/// Sweep one hyperparameter of a trainer on a dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SweepToolRequest {
    #[serde(flatten)]
    pub train: TrainRequest,
    pub sweep: SweepRequest,
}

/// Build a dataset and its displayed coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DatasetViewRequest {
    pub dataset: DatasetRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

/// A chart of losses against their argument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LossChartRequest {
    /// Loss ids. Classification and regression losses go on separate charts;
    /// list losses of one kind.
    pub losses: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub huber_delta: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
    /// Half-width of the x axis (> 0), default 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<f64>,
}

/// Which part of the manifest to return.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DescribeRequest {
    /// One of losses, models, datasets, views, sweep_parameters,
    /// experiences, messages, tools, schemas; omit for everything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

/// No arguments.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArguments {}

/// Start a registered experience in a running app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnterRequest {
    /// Experience id, e.g. "iris-svm".
    pub id: String,
}

/// A playback command for a running app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum PlaybackRequest {
    Play,
    Pause,
    Toggle,
    Step,
    Reset,
    /// Show an earlier step.
    Seek {
        step: usize,
    },
}

/// A playback command for a running app, as a tool argument (MCP tool
/// inputs must be objects).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AppPlaybackRequest {
    pub command: PlaybackRequest,
}

impl From<&PlaybackRequest> for crate::playback::PlaybackCommand {
    fn from(r: &PlaybackRequest) -> Self {
        use crate::playback::PlaybackCommand as P;
        match r {
            PlaybackRequest::Play => P::Play,
            PlaybackRequest::Pause => P::Pause,
            PlaybackRequest::Toggle => P::Toggle,
            PlaybackRequest::Step => P::Step,
            PlaybackRequest::Reset => P::Reset,
            PlaybackRequest::Seek { step } => P::Seek(*step),
        }
    }
}

/// Make a generated schema portable across MCP clients: optional fields
/// mean "may be omitted", so drop the `null` alternatives schemars adds for
/// `Option` (`"type": ["number", "null"]`, or a `{"type": "null"}` branch of
/// `anyOf`). Clients that map schemas onto single-type dialects (such as
/// Gemini function declarations) reject or misread those.
pub fn portable(mut schema: serde_json::Value) -> serde_json::Value {
    fn walk(v: &mut serde_json::Value) {
        use serde_json::Value;
        match v {
            Value::Object(map) => {
                if let Some(Value::Array(types)) = map.get_mut("type") {
                    types.retain(|t| t != "null");
                    if types.len() == 1 {
                        let only = types.remove(0);
                        map.insert("type".into(), only);
                    }
                }
                if let Some(Value::Array(branches)) = map.get_mut("anyOf") {
                    branches.retain(|b| b.get("type").is_none_or(|t| t != "null"));
                    if branches.len() == 1 {
                        let only = branches.remove(0);
                        map.remove("anyOf");
                        if let Value::Object(inner) = only {
                            for (k, val) in inner {
                                map.entry(k).or_insert(val);
                            }
                        }
                    }
                }
                map.values_mut().for_each(walk);
            }
            Value::Array(items) => items.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut schema);
    schema
}

/// JSON Schemas of every request type, by name, in their portable form.
pub fn schemas() -> Vec<(&'static str, serde_json::Value)> {
    use schemars::schema_for;
    let list = vec![
        (
            "AppPlaybackRequest",
            schema_for!(AppPlaybackRequest).to_value(),
        ),
        ("DatasetRequest", schema_for!(DatasetRequest).to_value()),
        (
            "DatasetViewRequest",
            schema_for!(DatasetViewRequest).to_value(),
        ),
        ("DescribeRequest", schema_for!(DescribeRequest).to_value()),
        ("EnterRequest", schema_for!(EnterRequest).to_value()),
        (
            "ExperimentRequest",
            schema_for!(ExperimentRequest).to_value(),
        ),
        ("LossChartRequest", schema_for!(LossChartRequest).to_value()),
        ("LossEvalRequest", schema_for!(LossEvalRequest).to_value()),
        ("NoArguments", schema_for!(NoArguments).to_value()),
        ("PlaybackRequest", schema_for!(PlaybackRequest).to_value()),
        ("SweepRequest", schema_for!(SweepRequest).to_value()),
        ("SweepToolRequest", schema_for!(SweepToolRequest).to_value()),
        ("TrainRequest", schema_for!(TrainRequest).to_value()),
        ("TrainerRequest", schema_for!(TrainerRequest).to_value()),
        ("ViewRequest", schema_for!(ViewRequest).to_value()),
    ];
    list.into_iter()
        .map(|(name, schema)| (name, portable(schema)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiences::{ExperienceKind, ExperienceRegistry, ExperiencesPlugin};
    use bevy::prelude::App;

    #[test]
    fn built_in_experiment_specs_round_trip_through_the_wire_format() {
        let mut app = App::new();
        app.add_plugins(ExperiencesPlugin);
        let registry = app.world().resource::<ExperienceRegistry>();
        let mut checked = 0;
        for e in registry.iter().filter(|e| e.is_available()) {
            if let ExperienceKind::Experiment { spec, .. } = &e.kind {
                let spec = spec();
                let request = ExperimentRequest::from_spec(&spec);
                // Through JSON, as an agent would send it.
                let json = serde_json::to_string(&request).unwrap();
                let parsed: ExperimentRequest = serde_json::from_str(&json).unwrap();
                assert_eq!(ExperimentSpec::try_from(&parsed).unwrap(), spec, "{}", e.id);
                checked += 1;
            }
        }
        assert!(checked >= 3);
    }

    #[test]
    fn hinge_evaluation_matches_the_spec() {
        let out = evaluate_losses(&LossEvalRequest {
            losses: vec!["hinge".into()],
            points: vec![1.5, 0.25],
            huber_delta: None,
            margin: Some(1.0),
        })
        .unwrap();
        assert_eq!(out[0].values, [0.0, 0.75]);
        assert_eq!(out[0].gradients, [0.0, -1.0]);
        assert_eq!(out[0].argument, "margin");
    }

    #[test]
    fn invalid_requests_name_the_field() {
        let mut t = TrainerRequest::for_loss(LossKind::Huber);
        t.huber_delta = Some(0.0);
        let err = TrainerConfig::try_from(&t).unwrap_err();
        assert_eq!(err.field, "huber_delta");
        assert!(err.message.contains("> 0"), "{err}");

        let mut t = TrainerRequest::for_loss(LossKind::Mse);
        t.c = Some(1.0);
        assert_eq!(TrainerConfig::try_from(&t).unwrap_err().field, "c");

        let req = ExperimentRequest {
            dataset: DatasetRequest::SeparableBlobs,
            view: None,
            train_on: None,
            panes: vec![TrainerRequest::for_loss(LossKind::Mse)],
            seed: None,
        };
        assert_eq!(
            ExperimentSpec::try_from(&req).unwrap_err().field,
            "panes[0].loss"
        );

        let bad_iris = DatasetRequest::Iris {
            positive: "rose".into(),
            negative: None,
        };
        let req = ExperimentRequest {
            dataset: bad_iris,
            ..req
        };
        assert_eq!(
            ExperimentSpec::try_from(&req).unwrap_err().field,
            "dataset.positive"
        );

        let sweep = SweepRequest {
            parameter: "gamma".into(),
            from: 0.1,
            to: 1.0,
            samples: 5,
            log: true,
        };
        assert!(
            SweepSpec::try_from(&sweep)
                .unwrap_err()
                .message
                .contains("huber-delta")
        );
        assert!(TrainerConfig::try_from(&TrainerRequest::for_loss(LossKind::ZeroOne)).is_err());
    }

    #[test]
    fn built_in_specs_validate_against_the_published_schema() {
        let schema = schemas()
            .into_iter()
            .find(|(n, _)| *n == "ExperimentRequest")
            .unwrap()
            .1;
        let validator = jsonschema::validator_for(&schema).unwrap();
        let mut app = App::new();
        app.add_plugins(ExperiencesPlugin);
        for e in app.world().resource::<ExperienceRegistry>().iter() {
            if let (true, ExperienceKind::Experiment { spec, .. }) = (e.is_available(), &e.kind) {
                let json = serde_json::to_value(ExperimentRequest::from_spec(&spec())).unwrap();
                let errors: Vec<String> = validator
                    .iter_errors(&json)
                    .map(|e| e.to_string())
                    .collect();
                assert!(errors.is_empty(), "{}: {errors:?}", e.id);
            }
        }
        let bad = serde_json::json!({"dataset": {"kind": "spirals"}, "panes": []});
        assert!(!validator.is_valid(&bad));
    }

    #[test]
    fn published_schemas_have_no_nullable_types() {
        for (name, schema) in schemas() {
            let text = schema.to_string();
            assert!(
                !text.contains("\"null\""),
                "{name} still has a null type: {text}"
            );
        }
        // Optional fields are optional, not required.
        let trainer = schemas()
            .into_iter()
            .find(|(n, _)| *n == "TrainerRequest")
            .unwrap()
            .1;
        assert_eq!(trainer["required"], serde_json::json!(["loss"]));
        assert_eq!(trainer["properties"]["c"]["type"], "number");
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err =
            serde_json::from_str::<TrainerRequest>(r#"{"loss":"hinge","gamma":2}"#).unwrap_err();
        assert!(err.to_string().contains("gamma"));
    }

    #[test]
    fn schemas_carry_descriptions_and_tags() {
        let schema = schemars::schema_for!(ExperimentRequest).to_value();
        let text = schema.to_string();
        assert!(text.contains("separable-blobs"));
        assert!(text.contains("One to three trainers"));
    }
}
