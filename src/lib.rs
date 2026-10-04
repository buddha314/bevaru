//! Interactive, animated machine-learning visualizations for Bevy.
//!
//! Add [`BevaruPlugin`] after `DefaultPlugins` for the full interactive scene,
//! or after `MinimalPlugins` for headless training and plotting. Pick what
//! loads with [`StartupExperiment`]; the math lives in [`bevaru_core`],
//! re-exported as [`core`].

pub mod agent;
pub mod app;
pub mod capture;
pub mod charts;
pub mod controls;
pub mod experiences;
pub mod experiment;
pub mod geometry;
pub mod lobby;
pub mod loss_surface;
pub mod orbit;
pub mod playback;
#[cfg(feature = "remote")]
pub mod remote;
pub mod scene;
pub mod shape_view;

pub use bevaru_core as core;
pub use experiment::{
    DatasetChoice, Experiment, ExperimentSpec, LoadExperiment, StartupExperiment, TrainSpace, View,
};
pub use playback::{Playback, PlaybackCommand, Sweep, SweepCommand, SweepParam, SweepSpec};

use bevy::prelude::*;
use ruviz::prelude::Plot;

/// Resource storing the latest rendered plot PNG bytes.
#[derive(Resource, Default, Clone, Deref, DerefMut)]
pub struct PlotPngBytes(pub Vec<u8>);

/// Message used to request an interactive plot refresh.
#[derive(Message, Debug, Default, Clone, Copy)]
pub struct RefreshPlotEvent;

/// Configuration for generating an ML-friendly sigmoid plot.
#[derive(Resource, Debug, Clone)]
pub struct MlPlotConfig {
    pub min_x: f64,
    pub max_x: f64,
    pub samples: usize,
}

impl Default for MlPlotConfig {
    fn default() -> Self {
        Self {
            min_x: -6.0,
            max_x: 6.0,
            samples: 120,
        }
    }
}

/// Everything bevaru offers. With a renderer present (`DefaultPlugins` added
/// first) this includes the scene, charts, and control panel; headless apps
/// get experiments, training playback, sweeps, and the sigmoid plot.
#[derive(Default)]
pub struct BevaruPlugin;

impl Plugin for BevaruPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            SigmoidPlotPlugin,
            experiment::ExperimentPlugin,
            playback::PlaybackPlugin,
        ));
        if app.is_plugin_added::<bevy::render::RenderPlugin>() {
            app.add_plugins((
                scene::ScenePlugin,
                charts::ChartsPlugin,
                controls::ControlsPlugin,
            ));
        }
    }
}

/// The original scaffold: a ruviz sigmoid plot kept current in
/// [`PlotPngBytes`], re-rendered on [`RefreshPlotEvent`].
#[derive(Default)]
pub struct SigmoidPlotPlugin;

impl Plugin for SigmoidPlotPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlotPngBytes>()
            .init_resource::<MlPlotConfig>()
            .add_message::<RefreshPlotEvent>()
            .add_systems(Startup, generate_initial_plot)
            .add_systems(Update, refresh_plot_on_event);
    }
}

fn generate_initial_plot(mut bytes: ResMut<PlotPngBytes>, config: Res<MlPlotConfig>) {
    update_plot_bytes(&mut bytes, &config);
}

fn refresh_plot_on_event(
    mut reader: MessageReader<RefreshPlotEvent>,
    mut bytes: ResMut<PlotPngBytes>,
    config: Res<MlPlotConfig>,
) {
    if reader.read().next().is_some() {
        update_plot_bytes(&mut bytes, &config);
    }
}

fn update_plot_bytes(bytes: &mut PlotPngBytes, config: &MlPlotConfig) {
    let (x, y) = build_sigmoid_data(config);
    match Plot::new()
        .line(&x, &y)
        .title("Sigmoid Activation")
        .xlabel("x")
        .ylabel("σ(x)")
        .render_png_bytes()
    {
        Ok(png) => bytes.0 = png,
        Err(err) => error!("bevaru plot render failed: {err}"),
    }
}

fn build_sigmoid_data(config: &MlPlotConfig) -> (Vec<f64>, Vec<f64>) {
    let span = config.max_x - config.min_x;
    let denom = (config.samples.saturating_sub(1) as f64).max(1.0);
    let x: Vec<f64> = (0..config.samples)
        .map(|i| config.min_x + span * (i as f64 / denom))
        .collect();
    let y = x.iter().map(|v| 1.0 / (1.0 + (-v).exp())).collect();
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigmoid_data_respects_requested_sample_count() {
        let cfg = MlPlotConfig {
            min_x: -2.0,
            max_x: 2.0,
            samples: 16,
        };
        let (x, y) = build_sigmoid_data(&cfg);
        assert_eq!(x.len(), 16);
        assert_eq!(y.len(), 16);
        assert!(y.iter().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn plugin_populates_plot_bytes_after_startup() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(BevaruPlugin);
        app.update();

        let bytes = app.world().resource::<PlotPngBytes>();
        assert!(!bytes.is_empty());
    }
}
