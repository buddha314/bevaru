//! Add your own experiences to the lobby: an experiment (a dataset and models
//! in the standard scene) and a custom one with its own systems.
//!
//! ```sh
//! cargo run --example custom_experience
//! ```

use bevaru::app::windowed_app;
use bevaru::core::{LossKind, TrainerConfig};
use bevaru::experiences::{
    Experience, ExperienceEntity, ExperienceKind, ExperienceStarted, ExperienceStopped, OnLoaded,
    RegisterExperience, Requirement, in_experience,
};
use bevaru::{DatasetChoice, ExperimentSpec};
use bevy::prelude::*;

fn huber_vs_mse() -> ExperimentSpec {
    let config = |loss| TrainerConfig::regression(loss);
    ExperimentSpec::new(
        DatasetChoice::Regression {
            dims: 1,
            outliers: 0.25,
        },
        config(LossKind::Huber),
    )
    .compare(config(LossKind::Mse))
}

/// A resource the custom experience owns; removed when it stops.
#[derive(Resource)]
struct Spin(f32);

fn main() {
    windowed_app(None)
        // An experiment experience: shown with the standard scene and controls.
        .register_experience(Experience {
            id: "heavy-outliers",
            title: "25% outliers: Huber vs MSE",
            summary: "How far a quarter of bad points drags each fit.",
            category: "Regression",
            thumbnail: None, // the lobby draws a placeholder
            requires: Requirement::None,
            note: None,
            kind: ExperienceKind::Experiment {
                spec: huber_vs_mse,
                on_loaded: vec![OnLoaded::Play],
            },
        })
        // A custom experience: this plugin provides everything it shows.
        .register_experience(Experience {
            id: "spinning-square",
            title: "Spinning square",
            summary: "The smallest custom experience.",
            category: "Demos",
            thumbnail: None,
            requires: Requirement::None,
            note: None,
            kind: ExperienceKind::Custom,
        })
        .add_observer(start_square)
        .add_observer(stop_square)
        .add_systems(Update, spin.run_if(in_experience("spinning-square")))
        .run();
}

fn start_square(
    started: On<ExperienceStarted>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if started.id != "spinning-square" {
        return;
    }
    commands.insert_resource(Spin(0.0));
    // Everything spawned carries ExperienceEntity, so leaving despawns it.
    commands.spawn((ExperienceEntity, Camera2d));
    commands.spawn((
        ExperienceEntity,
        Mesh2d(meshes.add(Rectangle::new(200.0, 200.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.0, 0.45, 0.7))),
    ));
}

fn stop_square(stopped: On<ExperienceStopped>, mut commands: Commands) {
    if stopped.id == "spinning-square" {
        commands.remove_resource::<Spin>();
    }
}

fn spin(
    time: Res<Time>,
    mut angle: ResMut<Spin>,
    mut squares: Query<&mut Transform, With<Mesh2d>>,
) {
    angle.0 += time.delta_secs();
    for mut t in &mut squares {
        t.rotation = Quat::from_rotation_z(angle.0);
    }
}
