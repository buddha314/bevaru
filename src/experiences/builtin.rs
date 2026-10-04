//! The experiences that ship with bevaru.

use bevaru_core::{LossKind, TrainerConfig};
use bevy::prelude::*;

use super::{Experience, ExperienceKind, OnLoaded, RegisterExperience, Requirement, Thumbnail};
use crate::experiment::{DatasetChoice, ExperimentSpec, TrainSpace};
use crate::playback::SweepSpec;

pub(super) fn register(app: &mut App) {
    app.register_experience(Experience {
        id: "iris-svm",
        title: "Iris: SVM margin vs C",
        summary: "Watch a linear SVM's margin narrow and its support vectors thin out as C grows, beside logistic regression.",
        category: "Classification",
        thumbnail: Some(Thumbnail::Embedded(include_bytes!(
            "../../assets/thumbnails/iris-svm.png"
        ))),
        requires: Requirement::None,
        note: None,
        kind: ExperienceKind::Experiment {
            spec: iris_svm,
            on_loaded: vec![OnLoaded::Sweep(SweepSpec {
                from: 0.01,
                to: 100.0,
                samples: 20,
                ..default()
            })],
        },
    })
    .register_experience(Experience {
        id: "mnist-svm",
        title: "MNIST: 3 vs 8",
        summary: "A linear SVM in all 784 pixel dimensions, seen through PCA, with the learned weights shown as an image.",
        category: "Classification",
        thumbnail: Some(Thumbnail::Embedded(include_bytes!(
            "../../assets/thumbnails/mnist-svm.png"
        ))),
        requires: Requirement::Feature {
            name: "mnist",
            enabled: cfg!(feature = "mnist"),
        },
        note: Some("Downloads MNIST (~11 MB) once and caches it."),
        kind: ExperienceKind::Experiment {
            spec: mnist_svm,
            on_loaded: vec![OnLoaded::Play],
        },
    })
    .register_experience(Experience {
        id: "regression-mse-vs-mae",
        title: "Outliers: MSE vs MAE vs Huber",
        summary: "Three fits of the same data with 15% outliers: MSE is dragged toward them, MAE ignores them, Huber sits between.",
        category: "Regression",
        thumbnail: Some(Thumbnail::Embedded(include_bytes!(
            "../../assets/thumbnails/regression-mse-vs-mae.png"
        ))),
        requires: Requirement::None,
        note: None,
        kind: ExperienceKind::Experiment {
            spec: regression_mse_vs_mae,
            on_loaded: vec![OnLoaded::Play],
        },
    })
    .register_experience(Experience {
        id: "loss-curves",
        title: "Every loss, side by side",
        summary: "Classification losses against the margin and regression losses against the residual, live as you change δ and the margin.",
        category: "Loss functions",
        thumbnail: Some(Thumbnail::Embedded(include_bytes!(
            "../../assets/thumbnails/loss-curves.png"
        ))),
        requires: Requirement::None,
        note: None,
        kind: ExperienceKind::Experiment {
            spec: loss_curves,
            on_loaded: vec![OnLoaded::ShowLosses(LossKind::ALL.to_vec()), OnLoaded::Play],
        },
    })
    .register_experience(Experience {
        id: super::loss_shapes::ID,
        title: "Loss shapes in 3D",
        summary: "Every loss as a surface over two quantities you know, such as truth and prediction or two class scores, with the familiar 2-D curve as a slice.",
        category: "Loss functions",
        thumbnail: Some(Thumbnail::Embedded(include_bytes!(
            "../../assets/thumbnails/loss-shapes.png"
        ))),
        requires: Requirement::None,
        note: None,
        kind: ExperienceKind::Custom,
    })
    .register_experience(Experience {
        id: super::loss_surface::ID,
        title: "Training objective in 3D",
        summary: "The training objective over a model's weight and bias, for hinge, squared hinge, and logistic loss: the shape of training with each loss.",
        category: "Loss functions",
        thumbnail: Some(Thumbnail::Embedded(include_bytes!(
            "../../assets/thumbnails/loss-surface.png"
        ))),
        requires: Requirement::None,
        note: None,
        kind: ExperienceKind::Custom,
    })
    .register_experience(Experience {
        id: super::perceptron::ID,
        title: "Perceptron in 3D",
        summary: "A perceptron drawn for slides: inputs, weights, a sum, an activation, and the output, with weights you can change live.",
        category: "Diagrams",
        thumbnail: Some(Thumbnail::Embedded(include_bytes!(
            "../../assets/thumbnails/perceptron.png"
        ))),
        requires: Requirement::None,
        note: None,
        kind: ExperienceKind::Custom,
    })
    .register_experience(Experience {
        id: super::sigmoid::ID,
        title: "Sigmoid",
        summary: "The logistic sigmoid σ(x) = 1 / (1 + e^−x), plotted with ruviz.",
        category: "Activation functions",
        thumbnail: Some(Thumbnail::Embedded(include_bytes!(
            "../../assets/thumbnails/sigmoid.png"
        ))),
        requires: Requirement::None,
        note: None,
        kind: ExperienceKind::Custom,
    });
}

fn iris_svm() -> ExperimentSpec {
    ExperimentSpec::new(
        DatasetChoice::Iris {
            positive: "versicolor".into(),
            negative: Some("virginica".into()),
        },
        TrainerConfig {
            max_steps: 4000,
            ..TrainerConfig::svm(1.0).expect("C = 1 is valid")
        },
    )
    // Train on the two petal features shown, so the drawn line is exact.
    .with_train_space(TrainSpace::Projected)
    .compare(TrainerConfig {
        max_steps: 4000,
        lambda: 0.01,
        ..TrainerConfig::logistic()
    })
}

fn regression_mse_vs_mae() -> ExperimentSpec {
    let config = |loss| TrainerConfig {
        max_steps: 3000,
        ..TrainerConfig::regression(loss)
    };
    ExperimentSpec::new(
        DatasetChoice::Regression {
            dims: 1,
            outliers: 0.15,
        },
        config(LossKind::Mse),
    )
    .compare(config(LossKind::Mae))
    .compare(config(LossKind::Huber))
}

fn loss_curves() -> ExperimentSpec {
    ExperimentSpec::new(
        DatasetChoice::SeparableBlobs,
        TrainerConfig::svm(10.0).expect("C = 10 is valid"),
    )
}

#[cfg(feature = "mnist")]
fn mnist_svm() -> ExperimentSpec {
    ExperimentSpec::new(
        DatasetChoice::Mnist {
            positive: 3,
            negative: 8,
            samples: 2000,
        },
        TrainerConfig {
            max_steps: 600,
            ..TrainerConfig::svm(10.0).expect("C = 10 is valid")
        },
    )
}

/// Never called: the entry is unavailable without the feature.
#[cfg(not(feature = "mnist"))]
fn mnist_svm() -> ExperimentSpec {
    ExperimentSpec::default()
}
