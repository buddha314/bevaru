//! A two-parameter slice through a model's training objective.

use std::fmt;

use crate::dataset::TrainingData;
use crate::loss::{LossKind, LossParams, Task};
use crate::model::{LinearModel, objective};

/// The ranges and loss used to sample one weight and the bias.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSettings {
    pub weight_index: usize,
    pub weight_range: (f64, f64),
    pub bias_range: (f64, f64),
    /// Number of samples along each axis, including both endpoints.
    pub resolution: usize,
    pub loss: LossKind,
    pub loss_params: LossParams,
    pub lambda: f64,
}

/// Row-major samples: `row` selects bias, `column` selects the weight.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectiveSurface {
    pub settings: SurfaceSettings,
    pub values: Vec<f64>,
    pub min: f64,
    pub max: f64,
}

impl ObjectiveSurface {
    pub fn weight_at(&self, column: usize) -> f64 {
        interpolate(self.settings.weight_range, column, self.settings.resolution)
    }

    pub fn bias_at(&self, row: usize) -> f64 {
        interpolate(self.settings.bias_range, row, self.settings.resolution)
    }

    pub fn value(&self, column: usize, row: usize) -> f64 {
        self.values[row * self.settings.resolution + column]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceError {
    Resolution(usize),
    Range(&'static str),
    WeightIndex { index: usize, dimensions: usize },
    DataShape,
    NonFiniteInput,
    LossTask { loss: LossKind, task: Task },
    Lambda(f64),
    NonFiniteObjective { column: usize, row: usize },
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolution(n) => write!(f, "surface resolution {n} must be between 2 and 257"),
            Self::Range(axis) => write!(f, "{axis} range must have finite, increasing bounds"),
            Self::WeightIndex { index, dimensions } => {
                write!(f, "weight index {index} is outside {dimensions} dimensions")
            }
            Self::DataShape => f.write_str("surface data and base model dimensions do not match"),
            Self::NonFiniteInput => f.write_str("surface data or base model is not finite"),
            Self::LossTask { loss, task } => {
                write!(f, "loss {loss} is incompatible with {task:?} data")
            }
            Self::Lambda(lambda) => {
                write!(
                    f,
                    "regularization strength {lambda} must be finite and nonnegative"
                )
            }
            Self::NonFiniteObjective { column, row } => {
                write!(f, "objective is not finite at column {column}, row {row}")
            }
        }
    }
}

impl std::error::Error for SurfaceError {}

fn interpolate((lo, hi): (f64, f64), index: usize, resolution: usize) -> f64 {
    lo + (hi - lo) * index as f64 / (resolution - 1) as f64
}

/// Sample the exact objective minimized by the trainer at every grid point.
pub fn sample_objective_surface(
    data: &TrainingData,
    base: &LinearModel,
    settings: SurfaceSettings,
) -> Result<ObjectiveSurface, SurfaceError> {
    if !(2..=257).contains(&settings.resolution) {
        return Err(SurfaceError::Resolution(settings.resolution));
    }
    for (name, (lo, hi)) in [
        ("weight", settings.weight_range),
        ("bias", settings.bias_range),
    ] {
        if !lo.is_finite() || !hi.is_finite() || lo >= hi {
            return Err(SurfaceError::Range(name));
        }
    }
    if !settings.lambda.is_finite() || settings.lambda < 0.0 {
        return Err(SurfaceError::Lambda(settings.lambda));
    }
    if settings.loss.task() != data.task {
        return Err(SurfaceError::LossTask {
            loss: settings.loss,
            task: data.task,
        });
    }
    if data.y.is_empty() || data.x.nrows() != data.y.len() || data.x.ncols() != base.dim() {
        return Err(SurfaceError::DataShape);
    }
    if settings.weight_index >= base.dim() {
        return Err(SurfaceError::WeightIndex {
            index: settings.weight_index,
            dimensions: base.dim(),
        });
    }
    if !base.is_finite()
        || data.x.iter().any(|v| !v.is_finite())
        || data.y.iter().any(|v| !v.is_finite())
    {
        return Err(SurfaceError::NonFiniteInput);
    }

    let n = settings.resolution;
    let mut model = base.clone();
    let mut values = Vec::with_capacity(n * n);
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for row in 0..n {
        model.b = interpolate(settings.bias_range, row, n);
        for column in 0..n {
            model.w[settings.weight_index] = interpolate(settings.weight_range, column, n);
            let value = objective(
                settings.loss,
                &settings.loss_params,
                settings.lambda,
                data.task,
                &model,
                &data.x,
                &data.y,
            );
            if !value.is_finite() {
                return Err(SurfaceError::NonFiniteObjective { column, row });
            }
            min = min.min(value);
            max = max.max(value);
            values.push(value);
        }
    }
    Ok(ObjectiveSurface {
        settings,
        values,
        min,
        max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{DMatrix, DVector};

    fn data() -> TrainingData {
        TrainingData {
            x: DMatrix::from_row_slice(3, 2, &[-1.0, 0.5, 0.0, -0.5, 1.0, 1.0]),
            y: DVector::from_row_slice(&[-1.0, -1.0, 1.0]),
            task: Task::Classification,
        }
    }

    fn settings(loss: LossKind) -> SurfaceSettings {
        SurfaceSettings {
            weight_index: 0,
            weight_range: (-2.0, 2.0),
            bias_range: (-1.0, 1.0),
            resolution: 3,
            loss,
            loss_params: LossParams::default(),
            lambda: 0.25,
        }
    }

    #[test]
    fn grid_matches_objective_with_other_weight_held_fixed() {
        let data = data();
        let mut base = LinearModel::zeros(2);
        base.w[1] = 0.75;
        let cfg = settings(LossKind::Hinge);
        let grid = sample_objective_surface(&data, &base, cfg).unwrap();
        assert_eq!(grid.values.len(), 9);
        assert_eq!(grid.weight_at(0), -2.0);
        assert_eq!(grid.weight_at(1), 0.0);
        assert_eq!(grid.bias_at(2), 1.0);
        for row in 0..3 {
            for column in 0..3 {
                let mut model = base.clone();
                model.w[0] = grid.weight_at(column);
                model.b = grid.bias_at(row);
                let expected = objective(
                    cfg.loss,
                    &cfg.loss_params,
                    cfg.lambda,
                    data.task,
                    &model,
                    &data.x,
                    &data.y,
                );
                assert!((grid.value(column, row) - expected).abs() < 1e-12);
            }
        }
        assert_eq!(
            grid.min,
            grid.values.iter().copied().fold(f64::INFINITY, f64::min)
        );
        assert_eq!(
            grid.max,
            grid.values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max)
        );
    }

    #[test]
    fn loss_and_hyperparameters_change_the_surface() {
        let data = data();
        let base = LinearModel::zeros(2);
        let hinge = sample_objective_surface(&data, &base, settings(LossKind::Hinge)).unwrap();
        let logistic =
            sample_objective_surface(&data, &base, settings(LossKind::Logistic)).unwrap();
        assert_ne!(hinge.value(1, 1), logistic.value(1, 1));

        let mut wider_margin = settings(LossKind::Hinge);
        wider_margin.loss_params = wider_margin.loss_params.with_margin(2.0).unwrap();
        let changed = sample_objective_surface(&data, &base, wider_margin).unwrap();
        assert!(changed.value(1, 1) > hinge.value(1, 1));

        let mut stronger_regularization = settings(LossKind::Hinge);
        stronger_regularization.lambda = 1.0;
        let changed = sample_objective_surface(&data, &base, stronger_regularization).unwrap();
        assert!(changed.value(2, 1) > hinge.value(2, 1));
        assert_eq!(changed.value(1, 1), hinge.value(1, 1));
    }

    #[test]
    fn invalid_requests_are_rejected() {
        let data = data();
        let base = LinearModel::zeros(2);
        let mut cfg = settings(LossKind::Hinge);
        cfg.resolution = 1;
        assert_eq!(
            sample_objective_surface(&data, &base, cfg),
            Err(SurfaceError::Resolution(1))
        );
        cfg = settings(LossKind::Hinge);
        cfg.weight_range = (1.0, -1.0);
        assert_eq!(
            sample_objective_surface(&data, &base, cfg),
            Err(SurfaceError::Range("weight"))
        );
        cfg = settings(LossKind::Hinge);
        cfg.weight_index = 2;
        assert!(matches!(
            sample_objective_surface(&data, &base, cfg),
            Err(SurfaceError::WeightIndex { .. })
        ));
        cfg = settings(LossKind::Mse);
        assert!(matches!(
            sample_objective_surface(&data, &base, cfg),
            Err(SurfaceError::LossTask { .. })
        ));
        let bad = TrainingData {
            y: DVector::zeros(2),
            ..data
        };
        assert_eq!(
            sample_objective_surface(&bad, &base, settings(LossKind::Hinge)),
            Err(SurfaceError::DataShape)
        );
    }
}
