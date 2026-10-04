//! Bevy-independent core of bevaru: loss functions, linear models trained a
//! step at a time, datasets, and projections for display.

pub use nalgebra;

pub mod dataset;
pub mod loss;
pub mod model;
pub mod projection;
pub mod surface;

pub use dataset::{BinaryTask, Dataset, DatasetError, Targets, TrainingData};
pub use loss::{LossKind, LossParams, ParamError, Task};
pub use model::{
    ConfigError, LearningRate, LinearModel, ModelKind, Snapshot, Status, Trainer, TrainerConfig,
};
pub use projection::{Projection, ProjectionKind};

#[cfg(feature = "mnist")]
pub mod mnist;
