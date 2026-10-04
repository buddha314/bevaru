//! Loss functions with values, (sub)gradients, and typed hyperparameters.
//!
//! Regression losses take the residual `r = ŷ − y`. Classification losses take
//! the margin `m = y · f(x)` with labels `y ∈ {−1, +1}`.

use std::fmt;

/// The kind of supervised task a loss belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Task {
    Regression,
    Classification,
}

/// Every loss bevaru knows about. A closed enum so the active loss can be
/// switched from a UI dropdown and matched exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LossKind {
    Mse,
    Mae,
    Huber,
    Hinge,
    SquaredHinge,
    Logistic,
    ZeroOne,
}

impl LossKind {
    pub const ALL: [LossKind; 7] = [
        LossKind::Mse,
        LossKind::Mae,
        LossKind::Huber,
        LossKind::Hinge,
        LossKind::SquaredHinge,
        LossKind::Logistic,
        LossKind::ZeroOne,
    ];

    /// Stable kebab-case identifier for APIs and tools. Unlike [`name`](Self::name),
    /// it never changes for presentation reasons.
    pub fn id(self) -> &'static str {
        match self {
            LossKind::Mse => "mse",
            LossKind::Mae => "mae",
            LossKind::Huber => "huber",
            LossKind::Hinge => "hinge",
            LossKind::SquaredHinge => "squared-hinge",
            LossKind::Logistic => "logistic",
            LossKind::ZeroOne => "zero-one",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            LossKind::Mse => "Mean squared error",
            LossKind::Mae => "Mean absolute error",
            LossKind::Huber => "Huber",
            LossKind::Hinge => "Hinge",
            LossKind::SquaredHinge => "Squared hinge",
            LossKind::Logistic => "Logistic (log loss)",
            LossKind::ZeroOne => "0-1",
        }
    }

    pub fn task(self) -> Task {
        match self {
            LossKind::Mse | LossKind::Mae | LossKind::Huber => Task::Regression,
            LossKind::Hinge | LossKind::SquaredHinge | LossKind::Logistic | LossKind::ZeroOne => {
                Task::Classification
            }
        }
    }

    /// Losses with a usable gradient. 0-1 loss is flat almost everywhere, so
    /// it can be plotted but not trained on.
    pub fn is_trainable(self) -> bool {
        self != LossKind::ZeroOne
    }

    /// Whether the loss is differentiable everywhere (no kinks or jumps).
    pub fn is_smooth(self) -> bool {
        matches!(self, LossKind::Mse | LossKind::Logistic)
    }

    /// Losses valid for `task`, in display order.
    pub fn for_task(task: Task) -> impl Iterator<Item = LossKind> {
        Self::ALL.into_iter().filter(move |l| l.task() == task)
    }

    /// Pointwise loss at `x` — the residual for regression losses, the margin
    /// for classification losses.
    pub fn value(self, x: f64, p: &LossParams) -> f64 {
        match self {
            LossKind::Mse => x * x,
            LossKind::Mae => x.abs(),
            LossKind::Huber => {
                let d = p.huber_delta;
                if x.abs() <= d {
                    0.5 * x * x
                } else {
                    d * (x.abs() - 0.5 * d)
                }
            }
            LossKind::Hinge => (p.margin - x).max(0.0),
            LossKind::SquaredHinge => {
                let h = (p.margin - x).max(0.0);
                h * h
            }
            // ln(1 + e^{−m}), computed without overflow for large |m|.
            LossKind::Logistic => softplus(-x),
            LossKind::ZeroOne => {
                if x <= 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Pointwise (sub)gradient with respect to `x`. At a kink the subgradient
    /// returned is the one a solver should use: `0` for MAE at `r = 0`, `0` for
    /// hinge exactly at the margin.
    pub fn grad(self, x: f64, p: &LossParams) -> f64 {
        match self {
            LossKind::Mse => 2.0 * x,
            LossKind::Mae => sign(x),
            LossKind::Huber => {
                let d = p.huber_delta;
                if x.abs() <= d { x } else { d * sign(x) }
            }
            LossKind::Hinge => {
                if x < p.margin {
                    -1.0
                } else {
                    0.0
                }
            }
            LossKind::SquaredHinge => -2.0 * (p.margin - x).max(0.0),
            LossKind::Logistic => -sigmoid(-x),
            LossKind::ZeroOne => 0.0,
        }
    }
}

impl fmt::Display for LossKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Logistic sigmoid `1 / (1 + e^{−x})`, stable for large |x|.
pub fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

/// `ln(1 + e^x)`, stable for large |x|.
fn softplus(x: f64) -> f64 {
    if x > 0.0 {
        x + (-x).exp().ln_1p()
    } else {
        x.exp().ln_1p()
    }
}

fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// An invalid hyperparameter value.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamError {
    pub name: &'static str,
    pub value: f64,
    pub valid: &'static str,
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid {} = {}: must be {}",
            self.name, self.value, self.valid
        )
    }
}

impl std::error::Error for ParamError {}

pub(crate) fn check_positive(name: &'static str, value: f64) -> Result<f64, ParamError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(ParamError {
            name,
            value,
            valid: "finite and > 0",
        })
    }
}

/// Loss hyperparameters. Fields are private so every value has been
/// validated; construct with [`LossParams::new`] or the `with_*` setters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossParams {
    huber_delta: f64,
    margin: f64,
}

impl Default for LossParams {
    fn default() -> Self {
        Self {
            huber_delta: 1.0,
            margin: 1.0,
        }
    }
}

impl LossParams {
    pub fn new(huber_delta: f64, margin: f64) -> Result<Self, ParamError> {
        Ok(Self {
            huber_delta: check_positive("Huber delta", huber_delta)?,
            margin: check_positive("hinge margin", margin)?,
        })
    }

    pub fn with_huber_delta(self, delta: f64) -> Result<Self, ParamError> {
        Self::new(delta, self.margin)
    }

    pub fn with_margin(self, margin: f64) -> Result<Self, ParamError> {
        Self::new(self.huber_delta, margin)
    }

    pub fn huber_delta(&self) -> f64 {
        self.huber_delta
    }

    pub fn margin(&self) -> f64 {
        self.margin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> LossParams {
        LossParams::default()
    }

    #[test]
    fn all_losses_enumerable_with_names() {
        for l in LossKind::ALL {
            assert_eq!(LossKind::from_id(l.id()), Some(l));
        }
        assert_eq!(LossKind::from_id("Hinge"), None);
        assert_eq!(LossKind::ALL.len(), 7);
        for l in LossKind::ALL {
            assert!(!l.name().is_empty());
        }
    }

    #[test]
    fn losses_classified_by_task() {
        use LossKind::*;
        for l in [Mse, Mae, Huber] {
            assert_eq!(l.task(), Task::Regression);
        }
        for l in [Hinge, SquaredHinge, Logistic, ZeroOne] {
            assert_eq!(l.task(), Task::Classification);
        }
    }

    #[test]
    fn mse_value_and_gradient() {
        assert_eq!(LossKind::Mse.value(2.0, &p()), 4.0);
        assert_eq!(LossKind::Mse.grad(2.0, &p()), 4.0);
    }

    #[test]
    fn mae_subgradient_at_zero() {
        assert_eq!(LossKind::Mae.value(0.0, &p()), 0.0);
        assert_eq!(LossKind::Mae.grad(0.0, &p()), 0.0);
    }

    #[test]
    fn huber_switches_regime_at_delta() {
        let p = LossParams::new(1.0, 1.0).unwrap();
        assert_eq!(LossKind::Huber.value(0.5, &p), 0.125);
        assert_eq!(LossKind::Huber.value(3.0, &p), 2.5);
    }

    #[test]
    fn hinge_zero_beyond_margin() {
        assert_eq!(LossKind::Hinge.value(1.5, &p()), 0.0);
        assert_eq!(LossKind::Hinge.value(0.25, &p()), 0.75);
        assert_eq!(LossKind::Hinge.grad(1.5, &p()), 0.0);
        assert_eq!(LossKind::Hinge.grad(0.25, &p()), -1.0);
    }

    #[test]
    fn logistic_is_smooth_at_zero() {
        assert!((LossKind::Logistic.value(0.0, &p()) - 2f64.ln()).abs() < 1e-15);
        assert!((LossKind::Logistic.grad(0.0, &p()) + 0.5).abs() < 1e-15);
    }

    #[test]
    fn logistic_is_stable_for_extreme_margins() {
        let l = LossKind::Logistic;
        assert!((l.value(-800.0, &p()) - 800.0).abs() < 1e-9);
        assert!(l.value(800.0, &p()) >= 0.0 && l.value(800.0, &p()) < 1e-300);
        assert!((l.grad(-800.0, &p()) + 1.0).abs() < 1e-15);
    }

    #[test]
    fn gradients_agree_with_finite_differences() {
        let p = LossParams::new(1.3, 0.8).unwrap();
        let h = 1e-6;
        // Points chosen away from every kink (0, ±δ, the margin).
        let points = [-2.7, -1.1, -0.45, 0.35, 0.6, 1.05, 2.9];
        for l in LossKind::ALL.into_iter().filter(|l| l.is_trainable()) {
            for &x in &points {
                let fd = (l.value(x + h, &p) - l.value(x - h, &p)) / (2.0 * h);
                let g = l.grad(x, &p);
                let err = (g - fd).abs() / g.abs().max(fd.abs()).max(1.0);
                assert!(err < 1e-4, "{l} at {x}: analytic {g}, numeric {fd}");
            }
        }
    }

    #[test]
    fn invalid_huber_delta_rejected() {
        let err = LossParams::new(0.0, 1.0).unwrap_err();
        assert_eq!(err.name, "Huber delta");
        assert!(err.to_string().contains("> 0"));
        assert!(LossParams::default().with_margin(-1.0).is_err());
        assert!(LossParams::default().with_huber_delta(f64::NAN).is_err());
    }
}
