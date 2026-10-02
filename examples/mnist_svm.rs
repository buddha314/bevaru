//! A linear SVM separating handwritten 3s from 8s in all 784 pixel
//! dimensions. The scene shows the data's top two principal components and
//! the boundary's slice through that plane; the right panel shows what the
//! model actually learned — its weight vector as a 28 × 28 image.
//!
//! Downloads MNIST (~11 MB) on first run and caches it; set
//! `BEVARU_MNIST_DIR` to use an existing copy.
//!
//! ```sh
//! cargo run --release --features mnist --example mnist_svm
//! ```

mod shared;

use bevaru::core::TrainerConfig;
use bevaru::{
    BevaruPlugin, DatasetChoice, Experiment, ExperimentSpec, PlaybackCommand, StartupExperiment,
};
use bevy::prelude::*;

fn main() {
    let spec = ExperimentSpec::new(
        DatasetChoice::Mnist {
            positive: 3,
            negative: 8,
            samples: 2000,
        },
        TrainerConfig {
            max_steps: 600,
            ..TrainerConfig::svm(10.0).unwrap()
        },
    );

    App::new()
        .add_plugins(shared::window("bevaru — MNIST 3 vs 8"))
        .insert_resource(StartupExperiment(spec))
        .add_plugins((BevaruPlugin, shared::DevScreenshot))
        .add_systems(Update, play.run_if(resource_added::<Experiment>))
        .run();
}

fn play(mut playback: MessageWriter<PlaybackCommand>) {
    playback.write(PlaybackCommand::Play);
}
