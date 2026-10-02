//! Iris, versicolor vs virginica on the petal features: hinge-loss SVM beside
//! logistic regression while C sweeps from 0.01 to 100. Watch the SVM margin
//! narrow and the support vectors thin out as C grows.
//!
//! ```sh
//! cargo run --release --example iris_svm
//! ```

mod shared;

use bevaru::core::TrainerConfig;
use bevaru::{
    BevaruPlugin, DatasetChoice, Experiment, ExperimentSpec, StartupExperiment, SweepCommand,
    SweepSpec, TrainSpace,
};
use bevy::prelude::*;

fn main() {
    let spec = ExperimentSpec::new(
        DatasetChoice::Iris {
            positive: "versicolor".into(),
            negative: Some("virginica".into()),
        },
        TrainerConfig {
            max_steps: 4000,
            ..TrainerConfig::svm(1.0).unwrap()
        },
    )
    // Train on the two petal features shown, so the drawn line is exact.
    .with_train_space(TrainSpace::Projected)
    .compare(TrainerConfig {
        max_steps: 4000,
        lambda: 0.01,
        ..TrainerConfig::logistic()
    });

    App::new()
        .add_plugins(shared::window("bevaru — Iris: SVM C sweep"))
        .insert_resource(StartupExperiment(spec))
        .add_plugins((BevaruPlugin, shared::DevScreenshot))
        .add_systems(Update, start_sweep.run_if(resource_added::<Experiment>))
        .run();
}

/// Sweep C once loaded. Logistic regression has no C, so its pane holds still
/// as a reference.
fn start_sweep(mut sweep: MessageWriter<SweepCommand>) {
    sweep.write(SweepCommand::Start(SweepSpec {
        from: 0.01,
        to: 100.0,
        samples: 20,
        ..default()
    }));
}
