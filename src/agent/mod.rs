//! Machine-readable description of bevaru, for AI agents and other tools.
//!
//! [`manifest`] builds a [`Manifest`] from the code itself. Losses, models,
//! sweep parameters, and control messages are described by exhaustive
//! `match`es, so a new variant fails to compile until it is described here;
//! experiences come from the live [`ExperienceRegistry`]. [`api`] holds the
//! validated wire format agents send, [`tools`] the catalog of agent tools,
//! and [`docs`] the generators for `docs/agents/`, kept current by a test.

pub mod api;
pub mod docs;
pub mod run;
pub mod tools;

use std::collections::BTreeMap;

use bevaru_core::{LossKind, LossParams, ModelKind, Task, TrainerConfig};
use serde::{Deserialize, Serialize};

use crate::experiences::{ExperienceKind, ExperienceRegistry, Requirement};
use crate::playback::{PlaybackCommand, SweepCommand, SweepParam, SweepSpec};

/// The Bevy Remote Protocol methods a running app serves with the `remote`
/// feature (`bevaru::remote`), in documentation order. Listed here, outside
/// the feature gate, so the docs describe them in every build.
pub const REMOTE_METHODS: [(&str, &str); 7] = [
    (
        "bevaru.experiences.list",
        "List the registered experiences.",
    ),
    (
        "bevaru.experiences.enter",
        "Start an experience by id: {\"id\": \"iris-svm\"}.",
    ),
    (
        "bevaru.experiences.leave",
        "Return to the lobby, cleaning up the current experience.",
    ),
    (
        "bevaru.playback",
        "Playback command: \"play\", \"pause\", \"toggle\", \"step\", \"reset\", or {\"seek\": {\"step\": n}}.",
    ),
    (
        "bevaru.sweep.start",
        "Start a sweep: {\"parameter\", \"from\", \"to\", \"samples\", \"log\"}.",
    ),
    ("bevaru.sweep.stop", "Stop the running sweep."),
    (
        "bevaru.state",
        "The app's state: screen, active experience, panes, playback, sweep.",
    ),
];

/// Bumped when the manifest's shape changes incompatibly.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub bevaru_version: String,
    pub losses: Vec<LossInfo>,
    pub models: Vec<ModelInfo>,
    pub datasets: Vec<DatasetInfo>,
    pub views: Vec<ViewInfo>,
    pub sweep_parameters: Vec<SweepParamInfo>,
    pub experiences: Vec<ExperienceInfo>,
    pub messages: Vec<MessageInfo>,
    pub tools: Vec<tools::ToolInfo>,
    /// JSON-RPC methods served by a running app started with `--remote`.
    pub remote_methods: Vec<RemoteMethodInfo>,
    /// JSON Schemas of the request types in [`api`], by type name.
    pub schemas: BTreeMap<String, serde_json::Value>,
}

/// A numeric parameter and its valid range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamInfo {
    pub id: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<f64>,
    /// Lower bound, and whether it is excluded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exclusive_minimum: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
}

impl ParamInfo {
    fn new(id: &str, description: &str) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            default: None,
            minimum: None,
            exclusive_minimum: false,
            maximum: None,
        }
    }
    fn default(mut self, v: f64) -> Self {
        self.default = Some(v);
        self
    }
    fn positive(mut self) -> Self {
        self.minimum = Some(0.0);
        self.exclusive_minimum = true;
        self
    }
    fn at_least(mut self, v: f64) -> Self {
        self.minimum = Some(v);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LossInfo {
    pub id: String,
    pub name: String,
    pub task: String,
    /// What the loss is a function of: "residual" (ŷ − y) or "margin" (y·f(x)).
    pub argument: String,
    pub formula: String,
    pub trainable: bool,
    pub smooth: bool,
    /// The model this loss trains, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub hyperparameters: Vec<ParamInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub task: String,
    pub losses: Vec<String>,
    pub default_loss: String,
    pub description: String,
    pub hyperparameters: Vec<ParamInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetInfo {
    /// The `kind` in a `DatasetRequest`.
    pub id: String,
    pub description: String,
    pub task: String,
    /// Input features; for regression the display adds the target as an axis.
    pub features: String,
    pub feature_names: Vec<String>,
    pub classes: Vec<String>,
    pub default_view: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_feature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewInfo {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepParamInfo {
    pub id: String,
    pub name: String,
    /// Losses whose trainers this parameter changes.
    pub applies_to: Vec<String>,
    pub default_from: f64,
    pub default_to: f64,
    pub default_log: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperienceInfo {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub category: String,
    /// "experiment" or "custom".
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_feature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Whether this build can run it. Omitted from generated docs, which
    /// must not depend on the features a build happens to have.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteMethodInfo {
    pub method: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageInfo {
    /// Rust path of the message type.
    pub name: String,
    pub description: String,
    /// For enum messages, each variant and what it does.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<(String, String)>,
}

pub(crate) fn task_id(task: Task) -> &'static str {
    match task {
        Task::Regression => "regression",
        Task::Classification => "classification",
    }
}

/// What a loss is a function of.
pub fn argument(loss: LossKind) -> &'static str {
    match loss.task() {
        Task::Regression => "residual",
        Task::Classification => "margin",
    }
}

fn loss_info(loss: LossKind) -> LossInfo {
    let d = LossParams::default();
    let delta = ParamInfo::new(
        "huber_delta",
        "Where Huber switches from quadratic to linear.",
    )
    .default(d.huber_delta())
    .positive();
    let margin = ParamInfo::new(
        "margin",
        "Margin the hinge losses demand; zero loss beyond it.",
    )
    .default(d.margin())
    .positive();
    let (formula, hyperparameters) = match loss {
        LossKind::Mse => ("r²", vec![]),
        LossKind::Mae => ("|r|", vec![]),
        LossKind::Huber => ("½r² if |r| ≤ δ, else δ(|r| − ½δ)", vec![delta]),
        LossKind::Hinge => ("max(0, margin − m)", vec![margin]),
        LossKind::SquaredHinge => ("max(0, margin − m)²", vec![margin]),
        LossKind::Logistic => ("ln(1 + e^(−m))", vec![]),
        LossKind::ZeroOne => ("1 if m ≤ 0, else 0", vec![]),
    };
    LossInfo {
        id: loss.id().into(),
        name: loss.name().into(),
        task: task_id(loss.task()).into(),
        argument: argument(loss).into(),
        formula: formula.into(),
        trainable: loss.is_trainable(),
        smooth: loss.is_smooth(),
        model: ModelKind::for_loss(loss).map(|m| m.id().into()),
        hyperparameters,
    }
}

fn model_info(model: ModelKind) -> ModelInfo {
    let base = match model {
        ModelKind::Svm => TrainerConfig::svm(1.0).expect("C = 1 is valid"),
        ModelKind::LogisticRegression => TrainerConfig::logistic(),
        ModelKind::LinearRegression => TrainerConfig::regression(model.default_loss()),
    };
    let description = match model {
        ModelKind::LinearRegression => {
            "f(x) = w·x + b fitted to real targets by (sub)gradient descent on the mean loss of the residuals."
        }
        ModelKind::Svm => {
            "Soft-margin linear SVM: minimizes mean hinge loss + (1/2C)‖w‖², the same minimizer as ½‖w‖² + C·mean(hinge)."
        }
        ModelKind::LogisticRegression => {
            "Linear classifier trained on logistic (log) loss; its output is a probability via the sigmoid."
        }
    };
    let mut hyperparameters = Vec::new();
    match model {
        ModelKind::Svm => hyperparameters.push(
            ParamInfo::new("c", "Soft-margin penalty; larger C means fewer margin violations and a narrower margin.")
                .default(base.c().unwrap_or(1.0))
                .positive(),
        ),
        ModelKind::LinearRegression | ModelKind::LogisticRegression => hyperparameters.push(
            ParamInfo::new("lambda", "L2 regularization strength (bias excluded).")
                .default(base.lambda)
                .at_least(0.0),
        ),
    }
    hyperparameters.extend([
        ParamInfo::new(
            "learning_rate.initial",
            "Step size at step 0; it decays as initial / (1 + decay·step).",
        )
        .default(base.learning_rate.initial())
        .positive(),
        ParamInfo::new("max_steps", "Step budget.")
            .default(base.max_steps as f64)
            .at_least(1.0),
        ParamInfo::new("tolerance", "Relative loss change that counts as calm.")
            .default(base.tolerance)
            .at_least(0.0),
        ParamInfo::new(
            "patience",
            "Consecutive calm steps that count as converged.",
        )
        .default(base.patience as f64)
        .at_least(0.0),
        ParamInfo::new("batch_size", "Mini-batch size; omit for full batch.").at_least(1.0),
    ]);
    ModelInfo {
        id: model.id().into(),
        name: model.name().into(),
        task: task_id(model.task()).into(),
        losses: model.losses().iter().map(|l| l.id().into()).collect(),
        default_loss: model.default_loss().id().into(),
        description: description.into(),
        hyperparameters,
    }
}

fn dataset_info(d: &api::DatasetRequest) -> DatasetInfo {
    use api::DatasetRequest as D;
    let iris = bevaru_core::dataset::iris();
    let (id, description, task, features, names, classes, view, requires) = match d {
        D::SeparableBlobs => (
            "separable-blobs",
            "Two 2-D Gaussian classes (60 each), guaranteed linearly separable with a gap.",
            Task::Classification,
            "2".into(),
            vec!["x₁".into(), "x₂".into()],
            vec!["A".into(), "B".into()],
            "features [0, 1]",
            None,
        ),
        D::OverlappingBlobs => (
            "overlapping-blobs",
            "Two 2-D Gaussian classes (80 each) that overlap, so no line separates them.",
            Task::Classification,
            "2".into(),
            vec!["x₁".into(), "x₂".into()],
            vec!["A".into(), "B".into()],
            "features [0, 1]",
            None,
        ),
        D::Regression { .. } => (
            "regression",
            "80 points of y = w·x + b + noise, x in [−5, 5], with a chosen fraction of outliers far above the trend.",
            Task::Regression,
            "1 or 2".into(),
            vec!["x1".into(), "x2".into()],
            vec![],
            "features plus the target as the last axis",
            None,
        ),
        D::Iris { .. } => (
            "iris",
            "Fisher's Iris (150 samples, 3 classes of 50), reduced to a binary task: one class vs another or vs the rest.",
            Task::Classification,
            "4".into(),
            iris.feature_names.clone(),
            iris.class_names(),
            "features [2, 3] (petal length, petal width)",
            None,
        ),
        D::Mnist { .. } => (
            "mnist",
            "Handwritten digits, 28 × 28 pixels scaled to [0, 1]; two digits, subsampled with a fixed seed.",
            Task::Classification,
            "784".into(),
            vec!["pixel (row, column)".into()],
            (0..10).map(|d| d.to_string()).collect(),
            "pca 2",
            Some("mnist".to_string()),
        ),
    };
    DatasetInfo {
        id: id.into(),
        description: description.into(),
        task: task_id(task).into(),
        features,
        feature_names: names,
        classes,
        default_view: view.into(),
        requires_feature: requires,
    }
}

pub(crate) fn sweep_info(param: SweepParam) -> SweepParamInfo {
    let (from, to, log) = param.default_range();
    let applies_to = LossKind::ALL
        .into_iter()
        .filter(|l| l.is_trainable())
        .filter(|&l| {
            TrainerConfig::try_from(&api::TrainerRequest::for_loss(l))
                .is_ok_and(|cfg| param.applies_to(&cfg))
        })
        .map(|l| l.id().into())
        .collect();
    SweepParamInfo {
        id: param.id().into(),
        name: param.name().into(),
        applies_to,
        default_from: from,
        default_to: to,
        default_log: log,
    }
}

fn playback_variant(cmd: &PlaybackCommand) -> (&'static str, &'static str) {
    match cmd {
        PlaybackCommand::Play => ("Play", "Start training playback."),
        PlaybackCommand::Pause => ("Pause", "Stop advancing training."),
        PlaybackCommand::Toggle => ("Toggle", "Play if paused, pause if playing."),
        PlaybackCommand::Step => ("Step", "Advance every pane exactly one step."),
        PlaybackCommand::Reset => ("Reset", "Restart every pane from step 0."),
        PlaybackCommand::Seek(_) => ("Seek(step)", "Show an earlier step without retraining."),
    }
}

fn sweep_variant(cmd: &SweepCommand) -> (&'static str, &'static str) {
    match cmd {
        SweepCommand::Start(_) => (
            "Start(SweepSpec)",
            "Train every pane to convergence at each value, off the render thread, then animate.",
        ),
        SweepCommand::Stop => ("Stop", "Cancel the sweep and return to training playback."),
        SweepCommand::Play => ("Play", "Resume animating a computed sweep."),
        SweepCommand::Pause => ("Pause", "Hold on the current value."),
        SweepCommand::Seek(_) => ("Seek(index)", "Show the solution at one sweep value."),
    }
}

fn messages() -> Vec<MessageInfo> {
    let variants = |list: Vec<(&str, &str)>| -> Vec<(String, String)> {
        list.into_iter()
            .map(|(a, b)| (a.into(), b.into()))
            .collect()
    };
    let playback = [
        PlaybackCommand::Play,
        PlaybackCommand::Pause,
        PlaybackCommand::Toggle,
        PlaybackCommand::Step,
        PlaybackCommand::Reset,
        PlaybackCommand::Seek(0),
    ];
    let sweep = [
        SweepCommand::Start(SweepSpec::default()),
        SweepCommand::Stop,
        SweepCommand::Play,
        SweepCommand::Pause,
        SweepCommand::Seek(0),
    ];
    let msg = |name: &str, description: &str, v: Vec<(String, String)>| MessageInfo {
        name: name.into(),
        description: description.into(),
        variants: v,
    };
    vec![
        msg(
            "bevaru::experiment::LoadExperiment",
            "Build and show an ExperimentSpec (off the main thread); replaces the current experiment.",
            vec![],
        ),
        msg(
            "bevaru::experiment::UnloadExperiment",
            "Remove the current experiment and everything it created; cancels a pending load.",
            vec![],
        ),
        msg(
            "bevaru::playback::PlaybackCommand",
            "Control training playback in every pane at once.",
            variants(playback.iter().map(playback_variant).collect()),
        ),
        msg(
            "bevaru::playback::SweepCommand",
            "Control hyperparameter sweeps.",
            variants(sweep.iter().map(sweep_variant).collect()),
        ),
        msg(
            "bevaru::lobby::EnterExperience",
            "Start a registered experience by id (needs LobbyPlugin); leaves the current one first.",
            vec![],
        ),
        msg(
            "bevaru::lobby::LeaveExperience",
            "Return to the lobby, cleaning up the running experience.",
            vec![],
        ),
        msg(
            "bevaru::scene::FrameData",
            "Fit every pane's camera to its data.",
            vec![],
        ),
        msg(
            "bevaru::RefreshPlotEvent",
            "Re-render the sigmoid plot into PlotPngBytes.",
            vec![],
        ),
    ]
}

/// The experiences in `registry`, as the manifest describes them.
pub fn experience_infos(registry: &ExperienceRegistry) -> Vec<ExperienceInfo> {
    registry
        .iter()
        .map(|e| ExperienceInfo {
            id: e.id.into(),
            title: e.title.into(),
            summary: e.summary.into(),
            category: e.category.into(),
            kind: match e.kind {
                ExperienceKind::Experiment { .. } => "experiment",
                ExperienceKind::Custom => "custom",
            }
            .into(),
            requires_feature: match e.requires {
                Requirement::None => None,
                Requirement::Feature { name, .. } => Some(name.into()),
            },
            note: e.note.map(Into::into),
            available: Some(e.is_available()),
        })
        .collect()
}

/// Describe bevaru, with the experiences in `registry`.
pub fn manifest(registry: &ExperienceRegistry) -> Manifest {
    use api::DatasetRequest as D;
    let datasets = [
        D::SeparableBlobs,
        D::OverlappingBlobs,
        D::Regression {
            features: 1,
            outlier_fraction: 0.1,
        },
        D::Iris {
            positive: "versicolor".into(),
            negative: None,
        },
        D::Mnist {
            positive: 3,
            negative: 8,
            samples: 2000,
        },
    ];
    let experiences = experience_infos(registry);
    let mut schemas = BTreeMap::new();
    for (name, schema) in api::schemas() {
        schemas.insert(name.to_string(), schema);
    }
    Manifest {
        schema_version: SCHEMA_VERSION,
        bevaru_version: env!("CARGO_PKG_VERSION").into(),
        losses: LossKind::ALL.into_iter().map(loss_info).collect(),
        models: ModelKind::ALL.into_iter().map(model_info).collect(),
        datasets: datasets.iter().map(dataset_info).collect(),
        views: vec![
            ViewInfo { id: "features".into(), description: "Display 2 or 3 chosen feature columns; other features are held at their mean for the boundary slice.".into() },
            ViewInfo { id: "pca".into(), description: "Display the top 2 or 3 principal components, with each component's explained variance.".into() },
        ],
        sweep_parameters: SweepParam::ALL.into_iter().map(sweep_info).collect(),
        experiences,
        messages: messages(),
        tools: tools::tools(),
        remote_methods: REMOTE_METHODS
            .iter()
            .map(|(method, description)| RemoteMethodInfo {
                method: (*method).into(),
                description: (*description).into(),
            })
            .collect(),
        schemas,
    }
}

/// The manifest of the built-in experiences, independent of which cargo
/// features this build has: what the generated docs are made from.
pub fn builtin_manifest() -> Manifest {
    let mut app = bevy::prelude::App::new();
    app.add_plugins(crate::experiences::ExperiencesPlugin);
    let mut m = manifest(app.world().resource::<ExperienceRegistry>());
    for e in &mut m.experiences {
        e.available = None;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiences::{Experience, RegisterExperience};

    #[test]
    fn every_loss_model_and_sweep_parameter_is_described_once() {
        let m = builtin_manifest();
        let mut described: Vec<&str> = m.losses.iter().map(|l| l.id.as_str()).collect();
        let mut expected: Vec<&str> = LossKind::ALL.iter().map(|l| l.id()).collect();
        described.sort_unstable();
        expected.sort_unstable();
        assert_eq!(described, expected);
        assert_eq!(m.models.len(), ModelKind::ALL.len());
        assert_eq!(m.sweep_parameters.len(), SweepParam::ALL.len());
        let huber = m.losses.iter().find(|l| l.id == "huber").unwrap();
        assert_eq!(huber.hyperparameters[0].id, "huber_delta");
        assert!(huber.hyperparameters[0].exclusive_minimum);
        let c = m.sweep_parameters.iter().find(|p| p.id == "c").unwrap();
        assert_eq!(c.applies_to, ["hinge", "squared-hinge"]);
        let delta = m
            .sweep_parameters
            .iter()
            .find(|p| p.id == "huber-delta")
            .unwrap();
        assert_eq!(delta.applies_to, ["huber"]);
    }

    #[test]
    fn registered_experiences_appear() {
        let mut app = bevy::prelude::App::new();
        app.add_plugins(crate::experiences::ExperiencesPlugin)
            .register_experience(Experience {
                id: "third-party",
                title: "Third party",
                summary: "From another crate.",
                category: "Elsewhere",
                thumbnail: None,
                requires: Requirement::None,
                note: None,
                kind: ExperienceKind::Custom,
            });
        let m = manifest(app.world().resource::<ExperienceRegistry>());
        let e = m
            .experiences
            .iter()
            .find(|e| e.id == "third-party")
            .unwrap();
        assert_eq!(
            (e.kind.as_str(), e.category.as_str(), e.available),
            ("custom", "Elsewhere", Some(true))
        );
    }

    #[test]
    fn manifest_round_trips_through_json() {
        let m = builtin_manifest();
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Manifest>(&json).unwrap(), m);
    }

    #[test]
    fn docs_manifest_does_not_depend_on_features() {
        let m = builtin_manifest();
        assert!(m.experiences.iter().all(|e| e.available.is_none()));
        let mnist = m.experiences.iter().find(|e| e.id == "mnist-svm").unwrap();
        assert_eq!(mnist.requires_feature.as_deref(), Some("mnist"));
    }
}
