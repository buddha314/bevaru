//! Explainable 3-D shapes of loss functions: each loss as a surface over two
//! meaningful inputs (truth and prediction, two class scores, a value and a
//! hyperparameter, three-class probabilities or scores).
//!
//! Every sample is computed by the library's own loss functions
//! ([`LossKind::value`] and the multiclass functions here), never by a
//! re-implementation, and every view's gradient is exact. Views are an
//! exhaustive enum, so adding one without describing it fails to compile.

use std::fmt;

use crate::loss::{LossKind, LossParams, ParamError};
use crate::surface::ObjectiveSurface;

/// One axis of a sampled grid: `n` evenly spaced samples over `[min, max]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    /// Short symbol, e.g. `ŷ` or `z_correct`.
    pub symbol: String,
    /// What the axis is, in words.
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub n: usize,
}

impl Axis {
    fn new(symbol: &str, name: &str, (min, max): (f64, f64), n: usize) -> Self {
        Self {
            symbol: symbol.into(),
            name: name.into(),
            min,
            max,
            n,
        }
    }

    /// The coordinate of sample `i`.
    pub fn at(&self, i: usize) -> f64 {
        if self.n < 2 {
            return self.min;
        }
        self.min + (self.max - self.min) * i as f64 / (self.n - 1) as f64
    }
}

/// A sampled surface, row-major: `row` selects `y`, `column` selects `x`.
/// Samples outside the view's domain (off the probability triangle) are
/// `None`; capped samples are flagged in `clipped`.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceGrid {
    pub x: Axis,
    pub y: Axis,
    pub height_label: String,
    pub values: Vec<Option<f64>>,
    pub clipped: Vec<bool>,
    /// The cap applied to heights, if any.
    pub cap: Option<f64>,
    /// Smallest and largest present values.
    pub min: f64,
    pub max: f64,
}

impl SurfaceGrid {
    pub fn value(&self, column: usize, row: usize) -> Option<f64> {
        self.values[row * self.x.n + column]
    }

    pub fn is_clipped(&self, column: usize, row: usize) -> bool {
        self.clipped[row * self.x.n + column]
    }

    fn from_fn(
        x: Axis,
        y: Axis,
        height_label: &str,
        cap: Option<f64>,
        f: impl Fn(f64, f64) -> Option<f64>,
    ) -> Self {
        let mut values = Vec::with_capacity(x.n * y.n);
        let mut clipped = Vec::with_capacity(x.n * y.n);
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        for row in 0..y.n {
            let yv = y.at(row);
            for column in 0..x.n {
                let v = f(x.at(column), yv);
                let (v, c) = match (v, cap) {
                    (Some(v), Some(cap)) if v > cap => (Some(cap), true),
                    (v, _) => (v, false),
                };
                if let Some(v) = v {
                    min = min.min(v);
                    max = max.max(v);
                }
                values.push(v);
                clipped.push(c);
            }
        }
        Self {
            x,
            y,
            height_label: height_label.into(),
            values,
            clipped,
            cap,
            min,
            max,
        }
    }
}

impl ObjectiveSurface {
    /// This parameter-space surface as a general grid (x = the weight,
    /// y = the bias), so one mesh builder serves both kinds of surface.
    pub fn to_grid(&self) -> SurfaceGrid {
        let s = &self.settings;
        let n = s.resolution;
        SurfaceGrid {
            x: Axis::new(&format!("w{}", s.weight_index), "weight", s.weight_range, n),
            y: Axis::new("b", "bias", s.bias_range, n),
            height_label: "objective".into(),
            values: self.values.iter().copied().map(Some).collect(),
            clipped: vec![false; self.values.len()],
            cap: None,
            min: self.min,
            max: self.max,
        }
    }
}

// ---------------------------------------------------------------------------
// Loss functions not in `LossKind`: probability-space and three-class losses.

/// Binary cross-entropy −[p ln q + (1 − p) ln(1 − q)] of predicted
/// probability `q` against true probability `p`. Uses the convention
/// 0 · ln 0 = 0, so it is finite for p ∈ {0, 1} and q ∈ (0, 1).
pub fn cross_entropy(p: f64, q: f64) -> f64 {
    let on = if p == 0.0 { 0.0 } else { -p * q.ln() };
    let off = if p == 1.0 {
        0.0
    } else {
        -(1.0 - p) * (-q).ln_1p()
    };
    on + off
}

/// Entropy H(p) of a Bernoulli(p), in nats.
pub fn entropy(p: f64) -> f64 {
    cross_entropy(p, p)
}

/// Logistic sigmoid.
pub fn sigmoid(x: f64) -> f64 {
    crate::loss::sigmoid(x)
}

/// Softmax cross-entropy −ln softmax(scores)[truth], via log-sum-exp.
pub fn softmax_cross_entropy(scores: &[f64], truth: usize) -> f64 {
    let max = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let lse = max + scores.iter().map(|s| (s - max).exp()).sum::<f64>().ln();
    lse - scores[truth]
}

/// Weston–Watkins multiclass hinge: the sum over rivals of
/// max(0, margin + z_j − z_truth).
pub fn hinge_weston_watkins(scores: &[f64], truth: usize, margin: f64) -> f64 {
    scores
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != truth)
        .map(|(_, z)| (margin + z - scores[truth]).max(0.0))
        .sum()
}

/// Crammer–Singer multiclass hinge: only the worst rival counts,
/// max(0, margin + max_j z_j − z_truth).
pub fn hinge_crammer_singer(scores: &[f64], truth: usize, margin: f64) -> f64 {
    let worst = scores
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != truth)
        .map(|(_, z)| *z)
        .fold(f64::NEG_INFINITY, f64::max);
    (margin + worst - scores[truth]).max(0.0)
}

// ---------------------------------------------------------------------------
// The catalogue.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShapeFamily {
    PredictionVsTruth,
    ProbabilityVsTruth,
    TwoScores,
    Hyperparameter,
    ThreeClass,
}

impl ShapeFamily {
    pub fn id(self) -> &'static str {
        match self {
            ShapeFamily::PredictionVsTruth => "prediction-vs-truth",
            ShapeFamily::ProbabilityVsTruth => "probability-vs-truth",
            ShapeFamily::TwoScores => "two-scores",
            ShapeFamily::Hyperparameter => "hyperparameter",
            ShapeFamily::ThreeClass => "three-class",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ShapeFamily::PredictionVsTruth => "Prediction vs truth",
            ShapeFamily::ProbabilityVsTruth => "Probability vs truth",
            ShapeFamily::TwoScores => "Two class scores",
            ShapeFamily::Hyperparameter => "Hyperparameter as an axis",
            ShapeFamily::ThreeClass => "Three classes",
        }
    }
}

/// Every 3-D loss-shape view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShapeView {
    PredictionMse,
    PredictionMae,
    PredictionHuber,
    CrossEntropy,
    TwoScoresHinge,
    TwoScoresSquaredHinge,
    TwoScoresLogistic,
    TwoScoresZeroOne,
    HuberDelta,
    HingeMargin,
    SquaredHingeMargin,
    ThreeClassProbabilities,
    ThreeClassSoftmax,
    ThreeClassHingeWestonWatkins,
    ThreeClassHingeCrammerSinger,
}

/// Which line through a surface is the familiar 2-D loss curve, and what
/// that curve's argument (the residual or the margin) is along it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slice {
    /// Truth y = 0: along the prediction axis, ŷ is the residual.
    ZeroTruth,
    /// Other-class score 0: along the correct-class axis, z is the margin.
    ZeroOtherScore,
    /// True probability p = 1: predicted q has margin m = ln(q / (1 − q)).
    CertainTruth,
    /// The row at the current hyperparameter value.
    HyperparameterRow,
}

/// Hyperparameters and options for sampling a view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeParams {
    pub loss: LossParams,
    /// Cap for unbounded (cross-entropy) heights.
    pub cap: f64,
    /// Plot cross-entropy minus the entropy H(p): the KL divergence.
    pub entropy_removed: bool,
}

impl Default for ShapeParams {
    fn default() -> Self {
        Self {
            loss: LossParams::default(),
            cap: 8.0,
            entropy_removed: false,
        }
    }
}

/// Smallest probability sampled, so cross-entropy stays finite. Its
/// worst case, −ln ε ≈ 9.2, is above the default cap, so the corners clip.
pub const EPSILON: f64 = 1e-4;
pub const DEFAULT_RESOLUTION: usize = 81;
pub const MAX_RESOLUTION: usize = 201;

#[derive(Debug, Clone, PartialEq)]
pub enum ShapeError {
    Resolution(usize),
    Cap(f64),
    /// Entropy-removed mode only applies to the cross-entropy view.
    EntropyRemovedNotApplicable(ShapeView),
    Param(ParamError),
}

impl fmt::Display for ShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShapeError::Resolution(n) => {
                write!(f, "resolution {n} must be between 3 and {MAX_RESOLUTION}")
            }
            ShapeError::Cap(c) => write!(f, "cap {c} must be finite and > 0"),
            ShapeError::EntropyRemovedNotApplicable(v) => write!(
                f,
                "entropy-removed (KL) mode applies only to {}, not {}",
                ShapeView::CrossEntropy.id(),
                v.id()
            ),
            ShapeError::Param(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ShapeError {}

const SCORES: (f64, f64) = (-4.0, 4.0);
const VALUES: (f64, f64) = (-3.0, 3.0);
const HYPER: (f64, f64) = (0.1, 3.0);
const UNIT: (f64, f64) = (0.0, 1.0);

impl ShapeView {
    pub const ALL: [ShapeView; 15] = [
        ShapeView::PredictionMse,
        ShapeView::PredictionMae,
        ShapeView::PredictionHuber,
        ShapeView::CrossEntropy,
        ShapeView::TwoScoresHinge,
        ShapeView::TwoScoresSquaredHinge,
        ShapeView::TwoScoresLogistic,
        ShapeView::TwoScoresZeroOne,
        ShapeView::HuberDelta,
        ShapeView::HingeMargin,
        ShapeView::SquaredHingeMargin,
        ShapeView::ThreeClassProbabilities,
        ShapeView::ThreeClassSoftmax,
        ShapeView::ThreeClassHingeWestonWatkins,
        ShapeView::ThreeClassHingeCrammerSinger,
    ];

    /// The view students meet first: the classic cross-entropy surface.
    pub const DEFAULT: ShapeView = ShapeView::CrossEntropy;

    pub fn id(self) -> &'static str {
        match self {
            ShapeView::PredictionMse => "prediction-vs-truth-mse",
            ShapeView::PredictionMae => "prediction-vs-truth-mae",
            ShapeView::PredictionHuber => "prediction-vs-truth-huber",
            ShapeView::CrossEntropy => "probability-vs-truth-cross-entropy",
            ShapeView::TwoScoresHinge => "two-scores-hinge",
            ShapeView::TwoScoresSquaredHinge => "two-scores-squared-hinge",
            ShapeView::TwoScoresLogistic => "two-scores-logistic",
            ShapeView::TwoScoresZeroOne => "two-scores-zero-one",
            ShapeView::HuberDelta => "hyperparameter-huber-delta",
            ShapeView::HingeMargin => "hyperparameter-hinge-margin",
            ShapeView::SquaredHingeMargin => "hyperparameter-squared-hinge-margin",
            ShapeView::ThreeClassProbabilities => "three-class-probabilities",
            ShapeView::ThreeClassSoftmax => "three-class-softmax",
            ShapeView::ThreeClassHingeWestonWatkins => "three-class-hinge-weston-watkins",
            ShapeView::ThreeClassHingeCrammerSinger => "three-class-hinge-crammer-singer",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.id() == id)
    }

    pub fn family(self) -> ShapeFamily {
        use ShapeView::*;
        match self {
            PredictionMse | PredictionMae | PredictionHuber => ShapeFamily::PredictionVsTruth,
            CrossEntropy => ShapeFamily::ProbabilityVsTruth,
            TwoScoresHinge | TwoScoresSquaredHinge | TwoScoresLogistic | TwoScoresZeroOne => {
                ShapeFamily::TwoScores
            }
            HuberDelta | HingeMargin | SquaredHingeMargin => ShapeFamily::Hyperparameter,
            ThreeClassProbabilities
            | ThreeClassSoftmax
            | ThreeClassHingeWestonWatkins
            | ThreeClassHingeCrammerSinger => ShapeFamily::ThreeClass,
        }
    }

    pub fn title(self) -> &'static str {
        use ShapeView::*;
        match self {
            PredictionMse => "Mean squared error over truth and prediction",
            PredictionMae => "Mean absolute error over truth and prediction",
            PredictionHuber => "Huber loss over truth and prediction",
            CrossEntropy => "Binary cross-entropy over true and predicted probability",
            TwoScoresHinge => "Hinge loss over two class scores",
            TwoScoresSquaredHinge => "Squared hinge loss over two class scores",
            TwoScoresLogistic => "Logistic (softmax) loss over two class scores",
            TwoScoresZeroOne => "0-1 loss over two class scores",
            HuberDelta => "Huber loss over residual and δ",
            HingeMargin => "Hinge loss over margin and the margin parameter",
            SquaredHingeMargin => "Squared hinge loss over margin and the margin parameter",
            ThreeClassProbabilities => "Three-class cross-entropy on the probability triangle",
            ThreeClassSoftmax => "Softmax cross-entropy over two rival scores",
            ThreeClassHingeWestonWatkins => "Weston–Watkins hinge over two rival scores",
            ThreeClassHingeCrammerSinger => "Crammer–Singer hinge over two rival scores",
        }
    }

    /// The `LossKind`s this view shows. Every `LossKind` appears somewhere.
    pub fn losses(self) -> Vec<LossKind> {
        use ShapeView::*;
        match self {
            PredictionMse => vec![LossKind::Mse],
            PredictionMae => vec![LossKind::Mae],
            PredictionHuber | HuberDelta => vec![LossKind::Huber],
            // Cross-entropy on probabilities is logistic loss on margins.
            CrossEntropy | TwoScoresLogistic => vec![LossKind::Logistic],
            TwoScoresHinge | HingeMargin => vec![LossKind::Hinge],
            TwoScoresSquaredHinge | SquaredHingeMargin => vec![LossKind::SquaredHinge],
            TwoScoresZeroOne => vec![LossKind::ZeroOne],
            // Multiclass generalizations; their binary cases are the logistic
            // and hinge views.
            ThreeClassProbabilities | ThreeClassSoftmax => vec![LossKind::Logistic],
            ThreeClassHingeWestonWatkins | ThreeClassHingeCrammerSinger => vec![LossKind::Hinge],
        }
    }

    /// Hyperparameters (`LossParams` fields) the view uses.
    pub fn hyperparameters(self) -> &'static [&'static str] {
        use ShapeView::*;
        match self {
            PredictionHuber => &["huber_delta"],
            TwoScoresHinge | TwoScoresSquaredHinge => &["margin"],
            ThreeClassHingeWestonWatkins | ThreeClassHingeCrammerSinger => &["margin"],
            // The axis itself is the hyperparameter; the value picks the slice.
            HuberDelta => &["huber_delta"],
            HingeMargin | SquaredHingeMargin => &["margin"],
            PredictionMse
            | PredictionMae
            | CrossEntropy
            | TwoScoresLogistic
            | TwoScoresZeroOne
            | ThreeClassProbabilities
            | ThreeClassSoftmax => &[],
        }
    }

    /// The axes, at `n` samples each.
    pub fn axes(self, n: usize) -> (Axis, Axis) {
        use ShapeView::*;
        match self {
            PredictionMse | PredictionMae | PredictionHuber => (
                Axis::new("y", "true value", VALUES, n),
                Axis::new("ŷ", "prediction", VALUES, n),
            ),
            CrossEntropy => (
                Axis::new("p", "true probability", UNIT, n),
                Axis::new("q", "predicted probability", (EPSILON, 1.0 - EPSILON), n),
            ),
            TwoScoresHinge | TwoScoresSquaredHinge | TwoScoresLogistic | TwoScoresZeroOne => (
                Axis::new("z_correct", "score of the correct class", SCORES, n),
                Axis::new("z_other", "score of the other class", SCORES, n),
            ),
            HuberDelta => (
                Axis::new("r", "residual ŷ − y", VALUES, n),
                Axis::new("δ", "Huber δ", HYPER, n),
            ),
            HingeMargin | SquaredHingeMargin => (
                Axis::new("m", "margin y·f(x)", VALUES, n),
                Axis::new("μ", "margin parameter", HYPER, n),
            ),
            ThreeClassProbabilities => (
                Axis::new("q₂", "probability of class 2", UNIT, n),
                Axis::new("q₃", "probability of class 3", UNIT, n),
            ),
            ThreeClassSoftmax | ThreeClassHingeWestonWatkins | ThreeClassHingeCrammerSinger => (
                Axis::new("z₂", "score of rival class 2", SCORES, n),
                Axis::new("z₃", "score of rival class 3", SCORES, n),
            ),
        }
    }

    pub fn height_label(self, entropy_removed: bool) -> &'static str {
        match (self, entropy_removed) {
            (ShapeView::CrossEntropy, true) => "KL divergence (nats)",
            (ShapeView::CrossEntropy | ShapeView::ThreeClassProbabilities, false) => {
                "cross-entropy (nats)"
            }
            _ => "loss",
        }
    }

    /// The line through the surface that is the 2-D loss curve, for views
    /// whose loss has one. Three-class losses have no 2-D chart; their
    /// captions relate them to the binary views instead.
    pub fn slice(self) -> Option<Slice> {
        use ShapeView::*;
        match self {
            PredictionMse | PredictionMae | PredictionHuber => Some(Slice::ZeroTruth),
            CrossEntropy => Some(Slice::CertainTruth),
            TwoScoresHinge | TwoScoresSquaredHinge | TwoScoresLogistic | TwoScoresZeroOne => {
                Some(Slice::ZeroOtherScore)
            }
            HuberDelta | HingeMargin | SquaredHingeMargin => Some(Slice::HyperparameterRow),
            ThreeClassProbabilities
            | ThreeClassSoftmax
            | ThreeClassHingeWestonWatkins
            | ThreeClassHingeCrammerSinger => None,
        }
    }

    /// What the view teaches, for students. At most about 80 words.
    pub fn caption(self) -> &'static str {
        use ShapeView::*;
        match self {
            PredictionMse => {
                "Loss is zero exactly on the diagonal ŷ = y, where the prediction is right. It depends only on the residual ŷ − y, so the trough looks the same all along the diagonal: a parabola. Big misses cost quadratically more, which is why MSE chases outliers."
            }
            PredictionMae => {
                "Loss is zero on the diagonal ŷ = y and grows in straight lines either side: a V-shaped trough. The sharp crease at the bottom has no gradient, and every miss costs in proportion to its size, so a few outliers can't dominate."
            }
            PredictionHuber => {
                "A trough along ŷ = y whose bottom is rounded like MSE within δ of the diagonal and whose walls are straight like MAE beyond it. δ sets where the quadratic part ends: small errors are treated gently, outliers linearly."
            }
            CrossEntropy => {
                "Loss is high where truth and prediction disagree: predicting q near 0 when p is 1, or near 1 when p is 0, costs without bound (heights are capped). The valley follows q = p, but its floor is the entropy H(p), not zero: even a perfect forecast of a 50/50 outcome pays ln 2. Remove the entropy to see the KL divergence, which is zero exactly when q = p."
            }
            TwoScoresHinge => {
                "Hinge loss depends only on the gap z_correct − z_other, so the surface is a sheet folded along that direction. It is flat (zero) once the correct class wins by the margin, and rises linearly when it doesn't: it stops caring about examples it already gets right."
            }
            TwoScoresSquaredHinge => {
                "Like hinge, a sheet folded along the gap between the scores, flat once the correct class wins by the margin. Below the margin it rises quadratically, so badly wrong examples are pushed much harder."
            }
            TwoScoresLogistic => {
                "A smooth version of hinge: it depends only on the gap z_correct − z_other, never quite reaches zero, and keeps nudging even confident correct answers. This is softmax cross-entropy for two classes, and the same loss as binary cross-entropy on probabilities."
            }
            TwoScoresZeroOne => {
                "A cliff: loss is 1 wherever the other class scores at least as high, and 0 wherever the correct class wins. It counts mistakes exactly, but it is flat everywhere else, so it has no gradient to learn from. Smooth losses exist because of this."
            }
            HuberDelta => {
                "Every row is a Huber curve for one δ. Small δ makes Huber behave like a scaled absolute error; large δ makes it a squared error over the whole range. The bend where the parabola meets the straight walls slides outward as δ grows."
            }
            HingeMargin => {
                "Every row is a hinge curve for one margin parameter. The crease where the loss becomes zero sits at m = μ, so a larger margin parameter demands that the correct class win by more before the loss stops."
            }
            SquaredHingeMargin => {
                "Every row is a squared-hinge curve for one margin parameter. The flat region starts at m = μ; below it the loss grows quadratically, so moving the margin moves where the bowl begins."
            }
            ThreeClassProbabilities => {
                "With three classes, a prediction is a point on a triangle q₁ + q₂ + q₃ = 1; class 1 is the true one. Cross-entropy is −ln q₁, so only the probability on the true class matters: contour lines run parallel to the opposite edge, and how the wrong classes share the rest changes nothing."
            }
            ThreeClassSoftmax => {
                "The true class scores 0; the axes are the two rivals' scores. Softmax cross-entropy acts like a smooth maximum: it mostly listens to the strongest rival, so the surface is a rounded corner. With one rival far below, it reduces to the two-score logistic loss."
            }
            ThreeClassHingeWestonWatkins => {
                "Weston–Watkins adds up the violation from every rival that comes within the margin of the true class. Where both rivals compete, both count, so the surface rises twice as steeply along the diagonal z₂ = z₃."
            }
            ThreeClassHingeCrammerSinger => {
                "Crammer–Singer counts only the worst rival: the loss is the hinge of the largest rival score. The surface is a fold along z₂ = z₃, where the worst rival switches, and it is half the Weston–Watkins loss where both rivals compete equally."
            }
        }
    }

    /// The loss at input point `(x, y)`, exactly as the library computes it.
    /// `None` outside the view's domain (off the probability triangle).
    pub fn value(self, x: f64, y: f64, params: &ShapeParams) -> Option<f64> {
        use ShapeView::*;
        let p = &params.loss;
        let at = |loss: LossKind, arg: f64, params: &LossParams| loss.value(arg, params);
        Some(match self {
            PredictionMse => at(LossKind::Mse, y - x, p),
            PredictionMae => at(LossKind::Mae, y - x, p),
            PredictionHuber => at(LossKind::Huber, y - x, p),
            CrossEntropy => {
                let ce = cross_entropy(x, y);
                if params.entropy_removed {
                    ce - entropy(x)
                } else {
                    ce
                }
            }
            TwoScoresHinge => at(LossKind::Hinge, x - y, p),
            TwoScoresSquaredHinge => at(LossKind::SquaredHinge, x - y, p),
            TwoScoresLogistic => at(LossKind::Logistic, x - y, p),
            TwoScoresZeroOne => at(LossKind::ZeroOne, x - y, p),
            HuberDelta => at(LossKind::Huber, x, &p.with_huber_delta(y).ok()?),
            HingeMargin => at(LossKind::Hinge, x, &p.with_margin(y).ok()?),
            SquaredHingeMargin => at(LossKind::SquaredHinge, x, &p.with_margin(y).ok()?),
            ThreeClassProbabilities => {
                let q1 = 1.0 - x - y;
                if q1 < EPSILON || x < 0.0 || y < 0.0 {
                    return None;
                }
                -q1.ln()
            }
            ThreeClassSoftmax => softmax_cross_entropy(&[0.0, x, y], 0),
            ThreeClassHingeWestonWatkins => hinge_weston_watkins(&[0.0, x, y], 0, p.margin()),
            ThreeClassHingeCrammerSinger => hinge_crammer_singer(&[0.0, x, y], 0, p.margin()),
        })
    }

    /// ∂loss/∂(x, y) at `(x, y)`: the (sub)gradient the library's loss
    /// functions use. `None` outside the domain.
    pub fn gradient(self, x: f64, y: f64, params: &ShapeParams) -> Option<(f64, f64)> {
        use ShapeView::*;
        let p = &params.loss;
        let g = |loss: LossKind, arg: f64| loss.grad(arg, p);
        Some(match self {
            // L(ŷ − y): ∂/∂y = −g, ∂/∂ŷ = g.
            PredictionMse => (-g(LossKind::Mse, y - x), g(LossKind::Mse, y - x)),
            PredictionMae => (-g(LossKind::Mae, y - x), g(LossKind::Mae, y - x)),
            PredictionHuber => (-g(LossKind::Huber, y - x), g(LossKind::Huber, y - x)),
            CrossEntropy => {
                let (pv, q) = (x, y);
                // ∂CE/∂q = (q − p) / (q(1 − q)); ∂CE/∂p = ln((1 − q)/q).
                let dq = (q - pv) / (q * (1.0 - q));
                let dp = if params.entropy_removed {
                    // ∂KL/∂p = ln(p/q) − ln((1 − p)/(1 − q)), finite inside (0, 1).
                    if pv <= 0.0 || pv >= 1.0 {
                        return Some((f64::NAN, dq));
                    }
                    (pv / q).ln() - ((1.0 - pv) / (1.0 - q)).ln()
                } else {
                    ((1.0 - q) / q).ln()
                };
                (dp, dq)
            }
            // L(z_c − z_o): ∂/∂z_c = g, ∂/∂z_o = −g.
            TwoScoresHinge => (g(LossKind::Hinge, x - y), -g(LossKind::Hinge, x - y)),
            TwoScoresSquaredHinge => (
                g(LossKind::SquaredHinge, x - y),
                -g(LossKind::SquaredHinge, x - y),
            ),
            TwoScoresLogistic => (g(LossKind::Logistic, x - y), -g(LossKind::Logistic, x - y)),
            TwoScoresZeroOne => (0.0, 0.0),
            HuberDelta => {
                let params = p.with_huber_delta(y).ok()?;
                // ∂/∂δ: 0 in the quadratic part, |r| − δ in the linear part.
                let dd = if x.abs() <= y { 0.0 } else { x.abs() - y };
                (LossKind::Huber.grad(x, &params), dd)
            }
            HingeMargin => {
                let params = p.with_margin(y).ok()?;
                let active = if x < y { 1.0 } else { 0.0 };
                (LossKind::Hinge.grad(x, &params), active)
            }
            SquaredHingeMargin => {
                let params = p.with_margin(y).ok()?;
                let h = (y - x).max(0.0);
                (LossKind::SquaredHinge.grad(x, &params), 2.0 * h)
            }
            ThreeClassProbabilities => {
                let q1 = 1.0 - x - y;
                if q1 < EPSILON || x < 0.0 || y < 0.0 {
                    return None;
                }
                // −ln(1 − q₂ − q₃): both partials are 1/q₁.
                (1.0 / q1, 1.0 / q1)
            }
            ThreeClassSoftmax => {
                let s = [0.0, x, y];
                let max = x.max(y).max(0.0);
                let e: Vec<f64> = s.iter().map(|v| (v - max).exp()).collect();
                let sum: f64 = e.iter().sum();
                (e[1] / sum, e[2] / sum)
            }
            ThreeClassHingeWestonWatkins => {
                let m = p.margin();
                let on = |z: f64| if m + z > 0.0 { 1.0 } else { 0.0 };
                (on(x), on(y))
            }
            ThreeClassHingeCrammerSinger => {
                let m = p.margin();
                if m + x.max(y) <= 0.0 {
                    (0.0, 0.0)
                } else if x >= y {
                    (1.0, 0.0)
                } else {
                    (0.0, 1.0)
                }
            }
        })
    }

    /// Sample the view on a `resolution` × `resolution` grid.
    pub fn sample(
        self,
        params: &ShapeParams,
        resolution: usize,
    ) -> Result<SurfaceGrid, ShapeError> {
        if !(3..=MAX_RESOLUTION).contains(&resolution) {
            return Err(ShapeError::Resolution(resolution));
        }
        if !(params.cap.is_finite() && params.cap > 0.0) {
            return Err(ShapeError::Cap(params.cap));
        }
        if params.entropy_removed && self != ShapeView::CrossEntropy {
            return Err(ShapeError::EntropyRemovedNotApplicable(self));
        }
        let (x, y) = self.axes(resolution);
        let cap = matches!(
            self,
            ShapeView::CrossEntropy | ShapeView::ThreeClassProbabilities
        )
        .then_some(params.cap);
        Ok(SurfaceGrid::from_fn(
            x,
            y,
            self.height_label(params.entropy_removed),
            cap,
            |xv, yv| self.value(xv, yv, params),
        ))
    }

    /// The slice as `n` points `(input x, input y, argument, loss)`: where it
    /// lies on the surface, and the 2-D chart's argument (residual or margin)
    /// and loss there. `None` for views without a 2-D curve.
    pub fn slice_points(self, params: &ShapeParams, n: usize) -> Option<Vec<[f64; 4]>> {
        let (xa, ya) = self.axes(n);
        let point =
            |x: f64, y: f64, arg: f64| [x, y, arg, self.value(x, y, params).unwrap_or(f64::NAN)];
        let points = match self.slice()? {
            Slice::ZeroTruth => (0..n).map(|i| point(0.0, ya.at(i), ya.at(i))).collect(),
            Slice::ZeroOtherScore => (0..n).map(|i| point(xa.at(i), 0.0, xa.at(i))).collect(),
            Slice::CertainTruth => (0..n)
                .map(|i| {
                    let q = ya.at(i);
                    point(1.0, q, (q / (1.0 - q)).ln())
                })
                .collect(),
            Slice::HyperparameterRow => {
                let h = match self {
                    ShapeView::HuberDelta => params.loss.huber_delta(),
                    _ => params.loss.margin(),
                };
                (0..n).map(|i| point(xa.at(i), h, xa.at(i))).collect()
            }
        };
        Some(points)
    }
}

impl fmt::Display for ShapeView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> ShapeParams {
        ShapeParams::default()
    }

    #[test]
    fn every_loss_has_a_shape_and_every_view_is_complete() {
        for loss in LossKind::ALL {
            assert!(
                ShapeView::ALL.iter().any(|v| v.losses().contains(&loss)),
                "{} has no view",
                loss.id()
            );
        }
        let mut ids: Vec<&str> = ShapeView::ALL.iter().map(|v| v.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ShapeView::ALL.len());
        for v in ShapeView::ALL {
            assert_eq!(ShapeView::from_id(v.id()), Some(v));
            let (x, y) = v.axes(5);
            assert!(x.min < x.max && y.min < y.max && x.min.is_finite() && y.max.is_finite());
            let words = v.caption().split_whitespace().count();
            assert!(
                (20..=85).contains(&words),
                "{}: caption has {words} words",
                v.id()
            );
        }
    }

    #[test]
    fn prediction_vs_truth_matches_mse_and_is_zero_on_the_diagonal() {
        let g = ShapeView::PredictionMse.sample(&params(), 81).unwrap();
        for row in 0..81 {
            for col in 0..81 {
                let (y, yhat) = (g.x.at(col), g.y.at(row));
                assert_eq!(
                    g.value(col, row),
                    Some(LossKind::Mse.value(yhat - y, &LossParams::default()))
                );
            }
            assert_eq!(g.value(row, row), Some(0.0), "diagonal");
        }
    }

    #[test]
    fn hinge_depends_only_on_the_gap() {
        let p = params();
        for &(a, b) in &[(1.0, 0.0), (0.0, -1.0), (2.5, 1.5)] {
            let v = ShapeView::TwoScoresHinge.value(a, b, &p).unwrap();
            assert_eq!(v, (1.0f64 - (a - b)).max(0.0));
            let shifted = ShapeView::TwoScoresHinge
                .value(a + 1.7, b + 1.7, &p)
                .unwrap();
            assert!((v - shifted).abs() < 1e-12);
        }
    }

    #[test]
    fn logistic_equals_binary_cross_entropy() {
        let p = params();
        for &(zc, zo) in &[(2.0, -1.0), (-1.5, 0.5), (0.0, 0.0), (6.0, -3.0)] {
            let logistic = ShapeView::TwoScoresLogistic.value(zc, zo, &p).unwrap();
            let bce = cross_entropy(1.0, sigmoid(zc - zo));
            assert!((logistic - bce).abs() < 1e-9, "{logistic} vs {bce}");
        }
    }

    #[test]
    fn huber_rows_are_2d_curves() {
        let g = ShapeView::HuberDelta.sample(&params(), 41).unwrap();
        for row in 0..41 {
            let delta = g.y.at(row);
            let lp = LossParams::default().with_huber_delta(delta).unwrap();
            for col in 0..41 {
                assert_eq!(
                    g.value(col, row),
                    Some(LossKind::Huber.value(g.x.at(col), &lp))
                );
            }
        }
    }

    #[test]
    fn cross_entropy_valley_floor_is_the_entropy() {
        let p = params();
        for &pv in &[0.1, 0.3, 0.5, 0.8] {
            let floor = ShapeView::CrossEntropy.value(pv, pv, &p).unwrap();
            assert!((floor - entropy(pv)).abs() < 1e-12);
            for &q in &[0.05, 0.2, 0.6, 0.95] {
                assert!(ShapeView::CrossEntropy.value(pv, q, &p).unwrap() >= floor - 1e-12);
            }
        }
        assert!((entropy(0.5) - 2f64.ln()).abs() < 1e-12);
    }

    #[test]
    fn kl_mode_is_nonnegative_and_zero_on_the_diagonal() {
        let p = ShapeParams {
            entropy_removed: true,
            ..params()
        };
        let g = ShapeView::CrossEntropy.sample(&p, 41).unwrap();
        assert!(g.values.iter().flatten().all(|&v| v >= -1e-12));
        for &pv in &[0.2, 0.5, 0.9] {
            assert!(ShapeView::CrossEntropy.value(pv, pv, &p).unwrap().abs() < 1e-12);
        }
        assert_eq!(
            ShapeView::PredictionMse.sample(&p, 9).unwrap_err(),
            ShapeError::EntropyRemovedNotApplicable(ShapeView::PredictionMse)
        );
    }

    #[test]
    fn cross_entropy_is_never_infinite_and_flags_clipping() {
        let g = ShapeView::CrossEntropy.sample(&params(), 81).unwrap();
        assert!(
            g.values
                .iter()
                .flatten()
                .all(|v| v.is_finite() && *v <= 8.0)
        );
        assert!(g.clipped.iter().any(|&c| c), "corners exceed the cap");
        assert_eq!(g.cap, Some(8.0));
        assert!(
            ShapeView::PredictionMse
                .sample(&params(), 9)
                .unwrap()
                .cap
                .is_none()
        );
    }

    #[test]
    fn three_class_triangle_only_the_true_probability_matters() {
        let p = params();
        let v = ShapeView::ThreeClassProbabilities;
        // q₁ = 0.4 split differently between the two wrong classes.
        let a = v.value(0.1, 0.5, &p).unwrap();
        let b = v.value(0.45, 0.15, &p).unwrap();
        assert!((a - b).abs() < 1e-12 && (a + 0.4f64.ln()).abs() < 1e-12);
        assert_eq!(v.value(0.7, 0.7, &p), None, "off the triangle");
        let g = v.sample(&p, 41).unwrap();
        assert!(g.values.iter().any(Option::is_none) && g.values.iter().any(Option::is_some));
    }

    #[test]
    fn sum_versus_worst_violation() {
        let p = params();
        let ww = ShapeView::ThreeClassHingeWestonWatkins
            .value(0.5, 0.5, &p)
            .unwrap();
        let cs = ShapeView::ThreeClassHingeCrammerSinger
            .value(0.5, 0.5, &p)
            .unwrap();
        let sm = ShapeView::ThreeClassSoftmax.value(0.5, 0.5, &p).unwrap();
        assert!((ww - 3.0).abs() < 1e-12);
        assert!((cs - 1.5).abs() < 1e-12);
        assert!((sm - (1.0 + 2.0 * 0.5f64.exp()).ln()).abs() < 1e-12);
    }

    #[test]
    fn three_class_reduces_to_binary_when_one_rival_vanishes() {
        let p = params();
        for &z2 in &[-2.0, -0.3, 0.0, 1.2] {
            // True class scores 0, so the gap is −z₂.
            let sm = ShapeView::ThreeClassSoftmax.value(z2, -50.0, &p).unwrap();
            let lg = ShapeView::TwoScoresLogistic.value(0.0, z2, &p).unwrap();
            assert!((sm - lg).abs() < 1e-9);
            for v in [
                ShapeView::ThreeClassHingeWestonWatkins,
                ShapeView::ThreeClassHingeCrammerSinger,
            ] {
                let h = v.value(z2, -50.0, &p).unwrap();
                let bin = ShapeView::TwoScoresHinge.value(0.0, z2, &p).unwrap();
                assert!((h - bin).abs() < 1e-9, "{}", v.id());
            }
        }
    }

    #[test]
    fn slices_equal_the_2d_curves() {
        let p = params();
        let lp = LossParams::default();
        for (view, loss) in [
            (ShapeView::PredictionMse, LossKind::Mse),
            (ShapeView::PredictionHuber, LossKind::Huber),
            (ShapeView::TwoScoresHinge, LossKind::Hinge),
            (ShapeView::TwoScoresLogistic, LossKind::Logistic),
            (ShapeView::CrossEntropy, LossKind::Logistic),
            (ShapeView::HingeMargin, LossKind::Hinge),
        ] {
            for [_, _, arg, value] in view.slice_points(&p, 31).unwrap() {
                let expected = loss.value(arg, &lp);
                assert!(
                    (value - expected).abs() < 1e-9,
                    "{} at {arg}: {value} vs {expected}",
                    view.id()
                );
            }
        }
        assert!(ShapeView::ThreeClassSoftmax.slice_points(&p, 31).is_none());
    }

    #[test]
    fn gradients_match_finite_differences() {
        let p = params();
        let h = 1e-6;
        // Points away from kinks, cliffs, and the simplex edge.
        let points = |v: ShapeView| -> Vec<(f64, f64)> {
            match v.family() {
                ShapeFamily::ProbabilityVsTruth => vec![(0.3, 0.6), (0.8, 0.2)],
                ShapeFamily::ThreeClass if v == ShapeView::ThreeClassProbabilities => {
                    vec![(0.2, 0.3), (0.1, 0.6)]
                }
                ShapeFamily::Hyperparameter => vec![(-1.3, 0.7), (2.1, 1.1), (0.25, 1.7)],
                _ => vec![(0.7, -1.9), (-2.3, 0.4), (1.37, 1.0)],
            }
        };
        for v in ShapeView::ALL
            .into_iter()
            .filter(|v| *v != ShapeView::TwoScoresZeroOne)
        {
            for (x, y) in points(v) {
                let (gx, gy) = v.gradient(x, y, &p).unwrap();
                let f = |a: f64, b: f64| v.value(a, b, &p).unwrap();
                let nx = (f(x + h, y) - f(x - h, y)) / (2.0 * h);
                let ny = (f(x, y + h) - f(x, y - h)) / (2.0 * h);
                let close = |a: f64, b: f64| (a - b).abs() / a.abs().max(b.abs()).max(1.0) < 1e-4;
                assert!(
                    close(gx, nx) && close(gy, ny),
                    "{} at ({x}, {y}): ({gx}, {gy}) vs ({nx}, {ny})",
                    v.id()
                );
            }
        }
        assert_eq!(
            ShapeView::TwoScoresZeroOne.gradient(1.0, 0.0, &p),
            Some((0.0, 0.0))
        );
    }

    #[test]
    fn invalid_requests_are_errors() {
        assert_eq!(
            ShapeView::PredictionMse.sample(&params(), 2).unwrap_err(),
            ShapeError::Resolution(2)
        );
        let bad = ShapeParams {
            cap: 0.0,
            ..params()
        };
        assert_eq!(
            ShapeView::CrossEntropy.sample(&bad, 9).unwrap_err(),
            ShapeError::Cap(0.0)
        );
    }

    #[test]
    fn objective_surfaces_convert_to_grids() {
        use crate::dataset::blobs;
        use crate::model::LinearModel;
        use crate::surface::{SurfaceSettings, sample_objective_surface};
        let data = blobs(20, true, 1).training_data().unwrap();
        let settings = SurfaceSettings {
            weight_index: 0,
            weight_range: (-2.0, 2.0),
            bias_range: (-1.0, 1.0),
            resolution: 9,
            loss: LossKind::Hinge,
            loss_params: LossParams::default(),
            lambda: 0.0,
        };
        let s = sample_objective_surface(&data, &LinearModel::zeros(2), settings).unwrap();
        let g = s.to_grid();
        assert_eq!((g.x.n, g.y.n), (9, 9));
        assert_eq!(g.value(3, 5), Some(s.value(3, 5)));
        assert_eq!((g.x.at(8), g.y.at(0)), (2.0, -1.0));
    }
}
