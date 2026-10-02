//! Linear models trained one (sub)gradient step at a time, with a snapshot of
//! every step so playback can scrub backwards.

use std::fmt;
use std::sync::Arc;

use nalgebra::{DMatrix, DVector};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;

use crate::dataset::TrainingData;
use crate::loss::{LossKind, LossParams, ParamError, Task, check_positive};

/// `f(x) = w·x + b`.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearModel {
    pub w: DVector<f64>,
    pub b: f64,
}

impl LinearModel {
    pub fn zeros(dim: usize) -> Self {
        Self {
            w: DVector::zeros(dim),
            b: 0.0,
        }
    }

    pub fn dim(&self) -> usize {
        self.w.len()
    }

    pub fn decision(&self, x: &DVector<f64>) -> f64 {
        self.w.dot(x) + self.b
    }

    /// `f(x)` for every row of `x`.
    pub fn decisions(&self, x: &DMatrix<f64>) -> DVector<f64> {
        (x * &self.w).add_scalar(self.b)
    }

    pub fn is_finite(&self) -> bool {
        self.b.is_finite() && self.w.iter().all(|v| v.is_finite())
    }
}

/// Which model is being trained. Each pairs with a fixed set of losses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelKind {
    LinearRegression,
    Svm,
    LogisticRegression,
}

impl ModelKind {
    pub const ALL: [ModelKind; 3] = [
        ModelKind::LinearRegression,
        ModelKind::Svm,
        ModelKind::LogisticRegression,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ModelKind::LinearRegression => "Linear regression",
            ModelKind::Svm => "Linear SVM",
            ModelKind::LogisticRegression => "Logistic regression",
        }
    }

    pub fn task(self) -> Task {
        match self {
            ModelKind::LinearRegression => Task::Regression,
            _ => Task::Classification,
        }
    }

    /// Losses this model can be trained with.
    pub fn losses(self) -> &'static [LossKind] {
        match self {
            ModelKind::LinearRegression => &[LossKind::Mse, LossKind::Mae, LossKind::Huber],
            ModelKind::Svm => &[LossKind::Hinge, LossKind::SquaredHinge],
            ModelKind::LogisticRegression => &[LossKind::Logistic],
        }
    }

    pub fn default_loss(self) -> LossKind {
        self.losses()[0]
    }

    /// The model a loss implies, for "switch loss" in the UI.
    pub fn for_loss(loss: LossKind) -> Option<ModelKind> {
        Self::ALL.into_iter().find(|m| m.losses().contains(&loss))
    }
}

impl fmt::Display for ModelKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Step size at step `t` (0-based).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LearningRate {
    Constant(f64),
    /// `η_t = initial / (1 + decay · t)`.
    InverseDecay {
        initial: f64,
        decay: f64,
    },
}

impl LearningRate {
    pub fn at(self, t: usize) -> f64 {
        match self {
            LearningRate::Constant(eta) => eta,
            LearningRate::InverseDecay { initial, decay } => initial / (1.0 + decay * t as f64),
        }
    }

    pub fn initial(self) -> f64 {
        self.at(0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrainerConfig {
    pub model: ModelKind,
    pub loss: LossKind,
    pub loss_params: LossParams,
    /// L2 regularization strength λ (bias excluded). For an SVM, `λ = 1/C`.
    pub lambda: f64,
    pub learning_rate: LearningRate,
    /// Mini-batch size; `None` for full-batch descent.
    pub batch_size: Option<usize>,
    pub max_steps: usize,
    /// Relative loss change below which a step counts as "calm".
    pub tolerance: f64,
    /// Consecutive calm steps required to declare convergence.
    pub patience: usize,
    pub seed: u64,
    /// Snapshots kept before history is decimated.
    pub history_cap: usize,
}

impl TrainerConfig {
    fn base(model: ModelKind, lambda: f64, learning_rate: LearningRate) -> Self {
        Self {
            model,
            loss: model.default_loss(),
            loss_params: LossParams::default(),
            lambda,
            learning_rate,
            batch_size: None,
            max_steps: 2000,
            tolerance: 1e-7,
            patience: 25,
            seed: 0,
            history_cap: 10_000,
        }
    }

    pub fn regression(loss: LossKind) -> Self {
        Self {
            loss,
            ..Self::base(
                ModelKind::LinearRegression,
                0.0,
                LearningRate::InverseDecay {
                    initial: 0.05,
                    decay: 0.002,
                },
            )
        }
    }

    pub fn logistic() -> Self {
        Self::base(
            ModelKind::LogisticRegression,
            0.0,
            LearningRate::InverseDecay {
                initial: 0.1,
                decay: 0.001,
            },
        )
    }

    /// Soft-margin SVM. The step size is scaled by `λ` so that large `C`
    /// (small λ) and small `C` (large λ) both converge without diverging.
    pub fn svm(c: f64) -> Result<Self, ParamError> {
        let lambda = 1.0 / check_positive("C", c)?;
        Ok(Self::base(ModelKind::Svm, lambda, Self::svm_rate(lambda)))
    }

    fn svm_rate(lambda: f64) -> LearningRate {
        let initial = 0.5f64.min(1.0 / lambda);
        LearningRate::InverseDecay {
            initial,
            decay: initial * lambda,
        }
    }

    /// Set an SVM's `C`, rescaling its step size to match.
    pub fn with_c(mut self, c: f64) -> Result<Self, ParamError> {
        self.lambda = 1.0 / check_positive("C", c)?;
        self.learning_rate = Self::svm_rate(self.lambda);
        Ok(self)
    }

    /// `C = 1/λ`, or `None` when unregularized.
    pub fn c(&self) -> Option<f64> {
        (self.lambda > 0.0).then(|| 1.0 / self.lambda)
    }

    pub fn validate(&self, data_task: Task) -> Result<(), ConfigError> {
        if !self.loss.is_trainable() {
            return Err(ConfigError::Untrainable(self.loss));
        }
        if !self.model.losses().contains(&self.loss) {
            return Err(ConfigError::IncompatibleLoss {
                model: self.model,
                loss: self.loss,
            });
        }
        if self.model.task() != data_task {
            return Err(ConfigError::WrongTask {
                model: self.model,
                data: data_task,
            });
        }
        let bad = |name, value, valid| Err(ConfigError::Param(ParamError { name, value, valid }));
        if !(self.lambda.is_finite() && self.lambda >= 0.0) {
            return bad("lambda", self.lambda, "finite and >= 0");
        }
        let eta = self.learning_rate.initial();
        if !(eta.is_finite() && eta > 0.0) {
            return bad("learning rate", eta, "finite and > 0");
        }
        if let LearningRate::InverseDecay { decay, .. } = self.learning_rate
            && !(decay.is_finite() && decay >= 0.0)
        {
            return bad("decay", decay, "finite and >= 0");
        }
        if self.batch_size == Some(0) {
            return bad("batch size", 0.0, ">= 1");
        }
        if self.max_steps == 0 {
            return bad("max steps", 0.0, ">= 1");
        }
        if self.history_cap < 2 {
            return bad("history cap", self.history_cap as f64, ">= 2");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    IncompatibleLoss { model: ModelKind, loss: LossKind },
    WrongTask { model: ModelKind, data: Task },
    Untrainable(LossKind),
    Param(ParamError),
    EmptyData,
    DimensionMismatch { features: usize, targets: usize },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let task = |t: Task| match t {
            Task::Regression => "regression",
            Task::Classification => "classification",
        };
        match self {
            ConfigError::IncompatibleLoss { model, loss } => write!(
                f,
                "{loss} is a {} loss; {model} needs one of: {}",
                task(loss.task()),
                model
                    .losses()
                    .iter()
                    .map(|l| l.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ConfigError::WrongTask { model, data } => {
                write!(
                    f,
                    "{model} is a {} model but the data is a {} task",
                    task(model.task()),
                    task(*data)
                )
            }
            ConfigError::Untrainable(l) => {
                write!(f, "{l} has no useful gradient and cannot be trained on")
            }
            ConfigError::Param(e) => e.fmt(f),
            ConfigError::EmptyData => f.write_str("training data is empty"),
            ConfigError::DimensionMismatch { features, targets } => {
                write!(f, "{features} feature rows but {targets} targets")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Mean pointwise loss of `model` over `(x, y)`, plus `λ/2 · ‖w‖²` (bias not
/// regularized). This is the objective the trainer minimizes.
pub fn objective(
    loss: LossKind,
    params: &LossParams,
    lambda: f64,
    task: Task,
    model: &LinearModel,
    x: &DMatrix<f64>,
    y: &DVector<f64>,
) -> f64 {
    let n = y.len().max(1) as f64;
    let f = model.decisions(x);
    let data: f64 = f
        .iter()
        .zip(y.iter())
        .map(|(&f, &y)| loss.value(pointwise_arg(task, f, y), params))
        .sum();
    data / n + 0.5 * lambda * model.w.norm_squared()
}

/// Residual `f − y` for regression, margin `y · f` for classification.
fn pointwise_arg(task: Task, f: f64, y: f64) -> f64 {
    match task {
        Task::Regression => f - y,
        Task::Classification => y * f,
    }
}

/// Why the trainer stopped, or that it hasn't.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Converged,
    BudgetExhausted,
    /// Parameters became non-finite; the last finite state is kept.
    Diverged,
}

/// The model state after a step.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub step: usize,
    pub model: LinearModel,
    /// Full-batch objective (mean loss + regularization).
    pub loss: f64,
    /// SVM only: indices of points on or inside the margin.
    pub support_vectors: Vec<usize>,
}

pub struct Trainer {
    config: TrainerConfig,
    data: Arc<TrainingData>,
    model: LinearModel,
    initial: Snapshot,
    latest: Snapshot,
    /// One snapshot per `stride` steps; step 0 lives in `initial`.
    history: Vec<Snapshot>,
    stride: usize,
    status: Status,
    calm: usize,
    rng: ChaCha8Rng,
    order: Vec<usize>,
    /// `f(x)` for every sample under the current model, shared by the
    /// snapshot and the next gradient so each step costs two matrix-vector
    /// products.
    decisions: DVector<f64>,
}

/// Points with `y f(x) ≤ margin + SV_TOLERANCE` count as support vectors;
/// subgradient descent never lands exactly on the margin.
const SV_TOLERANCE: f64 = 1e-3;

impl Trainer {
    /// Start from `w = 0, b = 0`, so restarting with a new loss starts from
    /// the same place. Data is shared, so many trainers (comparison panes,
    /// sweep values) can train on one copy.
    pub fn new(
        config: TrainerConfig,
        data: impl Into<Arc<TrainingData>>,
    ) -> Result<Self, ConfigError> {
        let data = data.into();
        config.validate(data.task)?;
        if data.y.is_empty() {
            return Err(ConfigError::EmptyData);
        }
        if data.x.nrows() != data.y.len() {
            return Err(ConfigError::DimensionMismatch {
                features: data.x.nrows(),
                targets: data.y.len(),
            });
        }
        let model = LinearModel::zeros(data.x.ncols());
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        let order = (0..data.y.len()).collect();
        let mut t = Self {
            initial: Snapshot {
                step: 0,
                model: model.clone(),
                loss: 0.0,
                support_vectors: Vec::new(),
            },
            latest: Snapshot {
                step: 0,
                model: model.clone(),
                loss: 0.0,
                support_vectors: Vec::new(),
            },
            config,
            data,
            model,
            history: Vec::new(),
            stride: 1,
            status: Status::Running,
            calm: 0,
            rng,
            order,
            decisions: DVector::zeros(0),
        };
        t.decisions = t.model.decisions(&t.data.x);
        t.initial = t.make_snapshot(0);
        t.latest = t.initial.clone();
        Ok(t)
    }

    pub fn config(&self) -> &TrainerConfig {
        &self.config
    }

    pub fn data(&self) -> &Arc<TrainingData> {
        &self.data
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn model(&self) -> &LinearModel {
        &self.model
    }

    pub fn steps_taken(&self) -> usize {
        self.latest.step
    }

    pub fn latest(&self) -> &Snapshot {
        &self.latest
    }

    /// Stored snapshots after step 0, oldest first. Every step is kept until
    /// `history_cap`, after which older steps are thinned.
    pub fn history(&self) -> &[Snapshot] {
        &self.history
    }

    /// `(step, loss)` for every stored snapshot including step 0.
    pub fn loss_curve(&self) -> Vec<(usize, f64)> {
        std::iter::once(&self.initial)
            .chain(&self.history)
            .chain(
                (self.history.last().map(|s| s.step) != Some(self.latest.step))
                    .then_some(&self.latest),
            )
            .map(|s| (s.step, s.loss))
            .collect()
    }

    /// The snapshot at `step`, or the nearest stored one before it once
    /// history has been thinned. Requests past the end return the latest.
    pub fn snapshot(&self, step: usize) -> &Snapshot {
        if step == 0 {
            return &self.initial;
        }
        if step >= self.latest.step {
            return &self.latest;
        }
        match self.history.binary_search_by_key(&step, |s| s.step) {
            Ok(i) => &self.history[i],
            Err(0) => &self.initial,
            Err(i) => &self.history[i - 1],
        }
    }

    /// Change λ and continue from the current parameters.
    pub fn set_lambda(&mut self, lambda: f64) -> Result<(), ConfigError> {
        let mut config = self.config.clone();
        config.lambda = lambda;
        if config.model == ModelKind::Svm && lambda > 0.0 {
            config.learning_rate = TrainerConfig::svm_rate(lambda);
        }
        self.reconfigure(config)
    }

    /// Change loss hyperparameters and continue from the current parameters.
    pub fn set_loss_params(&mut self, params: LossParams) -> Result<(), ConfigError> {
        let config = TrainerConfig {
            loss_params: params,
            ..self.config.clone()
        };
        self.reconfigure(config)
    }

    /// Apply a new configuration mid-run, keeping the current parameters and
    /// history. A converged trainer resumes; an exhausted budget stays so
    /// unless `max_steps` was raised.
    pub fn reconfigure(&mut self, config: TrainerConfig) -> Result<(), ConfigError> {
        config.validate(self.data.task)?;
        self.config = config;
        self.calm = 0;
        if self.status == Status::Converged
            || (self.status == Status::BudgetExhausted && self.latest.step < self.config.max_steps)
        {
            self.status = Status::Running;
        }
        Ok(())
    }

    /// Take one optimization step. Does nothing unless `Running`.
    pub fn step(&mut self) -> Status {
        if self.status != Status::Running {
            return self.status;
        }
        let t = self.latest.step;
        let (gw, gb) = self.gradient();
        let eta = self.config.learning_rate.at(t);
        let mut next = self.model.clone();
        next.w -= eta * gw;
        next.b -= eta * gb;
        if !next.is_finite() {
            self.status = Status::Diverged;
            return self.status;
        }
        self.model = next;
        self.decisions = self.model.decisions(&self.data.x);
        let snap = self.make_snapshot(t + 1);
        if !snap.loss.is_finite() {
            self.status = Status::Diverged;
            return self.status;
        }
        let prev = self.latest.loss;
        let rel = (snap.loss - prev).abs() / prev.abs().max(1e-12);
        self.calm = if rel < self.config.tolerance {
            self.calm + 1
        } else {
            0
        };
        self.record(snap);
        if self.calm >= self.config.patience {
            self.status = Status::Converged;
        } else if self.latest.step >= self.config.max_steps {
            self.status = Status::BudgetExhausted;
        }
        self.status
    }

    /// Step until the trainer stops; returns why.
    pub fn run(&mut self) -> Status {
        while self.step() == Status::Running {}
        self.status
    }

    fn gradient(&mut self) -> (DVector<f64>, f64) {
        let n = self.data.y.len();
        let cfg = &self.config;
        let task = self.data.task;
        // dL/df for sample i: chain through the residual or the margin.
        let dl_df = |f: f64, y: f64| match task {
            Task::Regression => cfg.loss.grad(f - y, &cfg.loss_params),
            Task::Classification => cfg.loss.grad(y * f, &cfg.loss_params) * y,
        };
        let (mut gw, gb) = match cfg.batch_size {
            Some(k) if k < n => {
                // Partial Fisher–Yates: the first k entries are a uniform sample.
                let (batch, _) = self.order.partial_shuffle(&mut self.rng, k);
                let mut gw = DVector::zeros(self.model.dim());
                let mut gb = 0.0;
                for &i in batch.iter() {
                    let g = dl_df(self.decisions[i], self.data.y[i]);
                    if g != 0.0 {
                        gw.axpy(g, &self.data.x.row(i).transpose(), 1.0);
                        gb += g;
                    }
                }
                (gw / k as f64, gb / k as f64)
            }
            _ => {
                let g = DVector::from_fn(n, |i, _| dl_df(self.decisions[i], self.data.y[i]));
                (self.data.x.tr_mul(&g) / n as f64, g.sum() / n as f64)
            }
        };
        gw.axpy(cfg.lambda, &self.model.w, 1.0);
        (gw, gb)
    }

    fn make_snapshot(&self, step: usize) -> Snapshot {
        let cfg = &self.config;
        let f = &self.decisions;
        let task = self.data.task;
        let data_loss: f64 = f
            .iter()
            .zip(self.data.y.iter())
            .map(|(&f, &y)| cfg.loss.value(pointwise_arg(task, f, y), &cfg.loss_params))
            .sum::<f64>()
            / f.len().max(1) as f64;
        let loss = data_loss + 0.5 * cfg.lambda * self.model.w.norm_squared();
        let support_vectors = if cfg.model == ModelKind::Svm {
            let limit = cfg.loss_params.margin() + SV_TOLERANCE;
            (0..f.len())
                .filter(|&i| self.data.y[i] * f[i] <= limit)
                .collect()
        } else {
            Vec::new()
        };
        Snapshot {
            step,
            model: self.model.clone(),
            loss,
            support_vectors,
        }
    }

    fn record(&mut self, snap: Snapshot) {
        if snap.step.is_multiple_of(self.stride) {
            self.history.push(snap.clone());
        }
        if self.history.len() > self.config.history_cap {
            self.stride *= 2;
            let stride = self.stride;
            self.history.retain(|s| s.step % stride == 0);
        }
        self.latest = snap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{blobs, regression};

    fn separable() -> TrainingData {
        blobs(60, true, 11).training_data().unwrap()
    }

    #[test]
    fn regularization_excludes_bias() {
        let x = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let y = DVector::from_vec(vec![5.0, 5.0]);
        let m = LinearModel {
            w: DVector::zeros(2),
            b: 5.0,
        };
        let total = objective(
            LossKind::Mse,
            &LossParams::default(),
            1.0,
            Task::Regression,
            &m,
            &x,
            &y,
        );
        assert_eq!(total, 0.0);
    }

    #[test]
    fn classifier_rejects_regression_loss() {
        let cfg = TrainerConfig {
            loss: LossKind::Mse,
            ..TrainerConfig::svm(1.0).unwrap()
        };
        let err = Trainer::new(cfg, separable()).err().unwrap();
        assert!(matches!(err, ConfigError::IncompatibleLoss { .. }));
        assert!(
            err.to_string()
                .contains("Mean squared error is a regression loss"),
            "{err}"
        );
    }

    #[test]
    fn zero_one_and_task_mismatch_rejected() {
        let cfg = TrainerConfig {
            loss: LossKind::ZeroOne,
            ..TrainerConfig::svm(1.0).unwrap()
        };
        assert_eq!(
            Trainer::new(cfg, separable()).err(),
            Some(ConfigError::Untrainable(LossKind::ZeroOne))
        );
        let reg = regression(20, 1, 0.0, 1).0.training_data().unwrap();
        assert!(matches!(
            Trainer::new(TrainerConfig::logistic(), reg).err(),
            Some(ConfigError::WrongTask { .. })
        ));
    }

    #[test]
    fn stepping_is_deterministic() {
        let cfg = TrainerConfig {
            batch_size: Some(16),
            seed: 42,
            ..TrainerConfig::svm(1.0).unwrap()
        };
        let mut a = Trainer::new(cfg.clone(), separable()).unwrap();
        let mut b = Trainer::new(cfg, separable()).unwrap();
        for _ in 0..100 {
            a.step();
            b.step();
            assert_eq!(a.model(), b.model());
        }
    }

    #[test]
    fn logistic_loss_decreases_on_separable_data() {
        let cfg = TrainerConfig {
            max_steps: 500,
            ..TrainerConfig::logistic()
        };
        let mut t = Trainer::new(cfg, separable()).unwrap();
        t.run();
        assert!(t.snapshot(500).loss < t.snapshot(1).loss);
    }

    #[test]
    fn large_c_narrows_margin_on_overlapping_data() {
        let data = blobs(80, false, 5).training_data().unwrap();
        let fit = |c: f64| {
            let cfg = TrainerConfig {
                max_steps: 20_000,
                ..TrainerConfig::svm(c).unwrap()
            };
            let mut t = Trainer::new(cfg, data.clone()).unwrap();
            t.run();
            let m = t.model().clone();
            let hinge = objective(
                LossKind::Hinge,
                &LossParams::default(),
                0.0,
                Task::Classification,
                &m,
                &data.x,
                &data.y,
            );
            (m.w.norm(), hinge)
        };
        let (w_small, hinge_small) = fit(0.01);
        let (w_large, hinge_large) = fit(100.0);
        assert!(
            w_large > w_small,
            "‖w‖: C=100 {w_large} vs C=0.01 {w_small}"
        );
        assert!(
            hinge_large <= hinge_small,
            "hinge: C=100 {hinge_large} vs C=0.01 {hinge_small}"
        );
    }

    #[test]
    fn scrubbing_returns_exact_earlier_step() {
        let mut t = Trainer::new(TrainerConfig::svm(1.0).unwrap(), separable()).unwrap();
        let mut at_50 = None;
        for _ in 0..200 {
            t.step();
            if t.steps_taken() == 50 {
                at_50 = Some(t.latest().clone());
            }
        }
        assert_eq!(t.steps_taken(), 200);
        assert_eq!(t.snapshot(50), &at_50.unwrap());
        assert_eq!(t.snapshot(0).model, LinearModel::zeros(2));
    }

    #[test]
    fn support_vectors_lie_within_margin() {
        let cfg = TrainerConfig {
            max_steps: 5000,
            ..TrainerConfig::svm(10.0).unwrap()
        };
        let mut t = Trainer::new(cfg, separable()).unwrap();
        t.run();
        let s = t.latest();
        assert!(!s.support_vectors.is_empty());
        let f = s.model.decisions(&t.data().x);
        for &i in &s.support_vectors {
            assert!(t.data().y[i] * f[i] <= 1.0 + SV_TOLERANCE);
        }
    }

    #[test]
    fn budget_exhausted() {
        let cfg = TrainerConfig {
            max_steps: 10,
            ..TrainerConfig::svm(1.0).unwrap()
        };
        let mut t = Trainer::new(cfg, separable()).unwrap();
        for _ in 0..15 {
            t.step();
        }
        assert_eq!(t.status(), Status::BudgetExhausted);
        assert_eq!(t.history().len(), 10);
    }

    #[test]
    fn history_is_capped_and_thinned() {
        let cfg = TrainerConfig {
            max_steps: 100,
            history_cap: 16,
            tolerance: 0.0,
            ..TrainerConfig::logistic()
        };
        let mut t = Trainer::new(cfg, separable()).unwrap();
        t.run();
        assert!(t.history().len() <= 16);
        assert_eq!(t.snapshot(100).step, 100);
        let s = t.snapshot(37);
        assert!(
            s.step <= 37 && s.step > 37 - 16,
            "nearest earlier, got {}",
            s.step
        );
    }

    #[test]
    fn mae_fit_is_closer_to_inlier_trend_than_mse() {
        // Generator's true trend is y = 0.8x + 1; outliers sit far above it.
        let (d, outliers) = regression(100, 1, 0.1, 8);
        let data = d.training_data().unwrap();
        let inlier_error = |loss| {
            let cfg = TrainerConfig {
                max_steps: 5000,
                ..TrainerConfig::regression(loss)
            };
            let mut t = Trainer::new(cfg, data.clone()).unwrap();
            t.run();
            let m = t.model();
            (m.w[0] - 0.8).abs() + (m.b - 1.0).abs()
        };
        assert_eq!(outliers.len(), 10);
        let (mse, mae) = (inlier_error(LossKind::Mse), inlier_error(LossKind::Mae));
        assert!(mae < mse, "MAE error {mae} should beat MSE error {mse}");
    }

    #[test]
    fn diverging_rate_is_detected() {
        let reg = regression(50, 1, 0.0, 2).0.training_data().unwrap();
        let cfg = TrainerConfig {
            learning_rate: LearningRate::Constant(10.0),
            ..TrainerConfig::regression(LossKind::Mse)
        };
        let mut t = Trainer::new(cfg, reg).unwrap();
        assert_eq!(t.run(), Status::Diverged);
        assert!(t.model().is_finite());
    }

    #[test]
    fn set_lambda_continues_from_current_parameters() {
        let mut t = Trainer::new(TrainerConfig::svm(1.0).unwrap(), separable()).unwrap();
        for _ in 0..20 {
            t.step();
        }
        let before = t.model().clone();
        t.set_lambda(0.5).unwrap();
        assert_eq!(t.model(), &before);
        t.step();
        assert_eq!(t.steps_taken(), 21);
    }

    #[test]
    fn trains_in_high_dimensions() {
        // 784-D stand-in for MNIST: two classes differing in a random subset of pixels.
        use rand::RngExt;
        let mut rng = ChaCha8Rng::seed_from_u64(9);
        let (n, d) = (80, 784);
        let x = DMatrix::from_fn(n, d, |i, j| {
            let on = if i < n / 2 { j % 3 == 0 } else { j % 3 == 1 };
            if on {
                rng.random_range(0.5..1.0)
            } else {
                rng.random_range(0.0..0.2)
            }
        });
        let y = DVector::from_fn(n, |i, _| if i < n / 2 { 1.0 } else { -1.0 });
        let cfg = TrainerConfig {
            max_steps: 50,
            ..TrainerConfig::svm(1.0).unwrap()
        };
        let mut t = Trainer::new(
            cfg,
            TrainingData {
                x,
                y,
                task: Task::Classification,
            },
        )
        .unwrap();
        t.run();
        assert_eq!(t.model().w.len(), 784);
        assert!(t.latest().loss < t.snapshot(0).loss);
    }
}
