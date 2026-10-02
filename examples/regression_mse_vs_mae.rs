//! Linear regression on data with 15% outliers, fitted three ways side by
//! side: MSE is dragged toward the outliers, MAE ignores them, and Huber sits
//! between, controlled by δ.
//!
//! ```sh
//! cargo run --release --example regression_mse_vs_mae
//! ```

mod shared;

use bevaru::core::{LossKind, TrainerConfig};
use bevaru::{
    BevaruPlugin, DatasetChoice, Experiment, ExperimentSpec, PlaybackCommand, StartupExperiment,
};
use bevy::prelude::*;

fn main() {
    let config = |loss| TrainerConfig {
        max_steps: 3000,
        ..TrainerConfig::regression(loss)
    };
    let spec = ExperimentSpec::new(
        DatasetChoice::Regression {
            dims: 1,
            outliers: 0.15,
        },
        config(LossKind::Mse),
    )
    .compare(config(LossKind::Mae))
    .compare(config(LossKind::Huber));

    App::new()
        .add_plugins(shared::window("bevaru — MSE vs MAE vs Huber"))
        .insert_resource(StartupExperiment(spec))
        .add_plugins((BevaruPlugin, shared::DevScreenshot))
        .add_systems(Update, play.run_if(resource_added::<Experiment>))
        .run();
}

fn play(mut playback: MessageWriter<PlaybackCommand>) {
    playback.write(PlaybackCommand::Play);
}
