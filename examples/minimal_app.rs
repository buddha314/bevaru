//! The smallest bevaru app: one experiment, SVM beside logistic regression on Iris.
//!
//! ```sh
//! cargo run --example minimal_app
//! ```

use bevaru::core::TrainerConfig;
use bevaru::{BevaruPlugin, DatasetChoice, ExperimentSpec, StartupExperiment};
use bevy::prelude::*;

fn main() {
    let spec = ExperimentSpec::new(
        DatasetChoice::Iris {
            positive: "versicolor".into(),
            negative: Some("virginica".into()),
        },
        TrainerConfig::svm(1.0).expect("C = 1 is valid"),
    )
    .compare(TrainerConfig::logistic());

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(StartupExperiment(spec))
        .add_plugins(BevaruPlugin) // after DefaultPlugins
        .run();
}
