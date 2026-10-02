//! Every loss on one screen: classification losses against the margin and
//! regression losses against the residual. Adjust Huber δ and the hinge
//! margin in the Models panel and watch the curves change, while a hinge SVM
//! trains on separable data.
//!
//! ```sh
//! cargo run --release --example loss_curves
//! ```

mod shared;

use bevaru::charts::ChartSettings;
use bevaru::core::{LossKind, TrainerConfig};
use bevaru::{
    BevaruPlugin, DatasetChoice, Experiment, ExperimentSpec, PlaybackCommand, StartupExperiment,
};
use bevy::prelude::*;

fn main() {
    let spec = ExperimentSpec::new(
        DatasetChoice::SeparableBlobs,
        TrainerConfig::svm(10.0).unwrap(),
    );

    App::new()
        .add_plugins(shared::window("bevaru — loss functions"))
        .insert_resource(StartupExperiment(spec))
        .add_plugins((BevaruPlugin, shared::DevScreenshot))
        .insert_resource(ChartSettings {
            overlay: LossKind::ALL.to_vec(),
            ..default()
        })
        .add_systems(Update, play.run_if(resource_added::<Experiment>))
        .run();
}

fn play(mut playback: MessageWriter<PlaybackCommand>) {
    playback.write(PlaybackCommand::Play);
}
