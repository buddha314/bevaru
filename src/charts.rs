//! ruviz charts rendered off the main thread into Bevy images: loss curves,
//! the training objective, and (for image data such as MNIST) each pane's
//! weight vector as a picture.
//!
//! A chart re-renders only when a hash of its inputs changes, at most every
//! [`MIN_RENDER_INTERVAL`] seconds; a render that finishes after its inputs
//! moved on is discarded.

use std::hash::{DefaultHasher, Hash, Hasher};

use bevaru_core::{LossKind, LossParams, Task};
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{AsyncComputeTaskPool, Task as BevyTask, futures::check_ready};
use ruviz::prelude::{Color as RColor, LineStyle, Plot};

use crate::experiment::{Experiment, fmt_num};
use crate::playback::{PaneViews, Playback, Sweep};
use crate::scene::CLASS_COLORS;

pub const MIN_RENDER_INTERVAL: f32 = 0.1;

#[derive(Resource, Debug, Clone)]
pub struct ChartSettings {
    pub show_loss_curves: bool,
    pub show_training: bool,
    /// Losses drawn on the loss-curve charts; empty means every loss valid
    /// for the current task. Classification and regression losses go on
    /// separate charts (margin vs residual axes).
    pub overlay: Vec<LossKind>,
    /// Half-width of the margin/residual axis.
    pub range: f64,
    pub size: UVec2,
}

impl Default for ChartSettings {
    fn default() -> Self {
        Self {
            show_loss_curves: true,
            show_training: true,
            overlay: Vec::new(),
            range: 3.0,
            size: UVec2::new(560, 340),
        }
    }
}

/// One chart's image and render state.
#[derive(Default)]
pub struct ChartSlot {
    pub image: Handle<Image>,
    /// Inputs hash of what `image` shows.
    shown: Option<u64>,
    /// Inputs hash this frame wants.
    wanted: Option<u64>,
    pending: Option<(u64, BevyTask<Result<ruviz::core::Image, String>>)>,
    last_spawn: f32,
    pub visible: bool,
}

impl ChartSlot {
    pub fn is_ready(&self) -> bool {
        self.visible && self.shown.is_some()
    }
}

#[derive(Resource, Default)]
pub struct Charts {
    /// Classification losses vs margin.
    pub classification: ChartSlot,
    /// Regression losses vs residual.
    pub regression: ChartSlot,
    pub training: ChartSlot,
    /// Renders started, for diagnostics and tests.
    pub renders: u64,
}

/// Per-pane weight vectors drawn as images when samples are square images.
#[derive(Resource, Default)]
pub struct WeightImages {
    pub images: Vec<Handle<Image>>,
    pub max_abs: Vec<f64>,
}

pub struct ChartsPlugin;

impl Plugin for ChartsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChartSettings>()
            .init_resource::<Charts>()
            .init_resource::<WeightImages>()
            .add_systems(Startup, create_chart_images)
            .add_systems(
                Update,
                (request_charts, collect_charts, update_weight_images)
                    .chain()
                    .after(crate::playback::PlaybackSystems)
                    .run_if(resource_exists::<Experiment>),
            );
    }
}

fn blank(size: UVec2) -> Image {
    Image::new_fill(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn create_chart_images(
    mut charts: ResMut<Charts>,
    settings: Res<ChartSettings>,
    mut images: ResMut<Assets<Image>>,
) {
    let charts = &mut *charts;
    for slot in [
        &mut charts.classification,
        &mut charts.regression,
        &mut charts.training,
    ] {
        slot.image = images.add(blank(settings.size));
    }
}

/// A line chart, as plain data so it can be hashed and sent to a thread.
#[derive(Debug, Clone, PartialEq)]
pub struct LineChart {
    pub title: String,
    pub xlabel: String,
    pub ylabel: String,
    pub series: Vec<Series>,
    /// Vertical dashed marker (x, label).
    pub marker: Option<(f64, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub label: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub color: [u8; 3],
    pub width: f32,
}

impl LineChart {
    fn key(&self, size: UVec2) -> u64 {
        let mut h = DefaultHasher::new();
        (&self.title, &self.xlabel, &self.ylabel, size.x, size.y).hash(&mut h);
        for s in &self.series {
            (&s.label, s.color, s.width.to_bits()).hash(&mut h);
            s.x.iter()
                .chain(&s.y)
                .for_each(|v| v.to_bits().hash(&mut h));
        }
        if let Some((x, l)) = &self.marker {
            (x.to_bits(), l).hash(&mut h);
        }
        h.finish()
    }

    pub fn render(&self, size: UVec2) -> Result<ruviz::core::Image, String> {
        let mut series = self.series.iter().filter(|s| !s.x.is_empty());
        let first = series.next().ok_or("chart has no data")?;
        let color = |c: [u8; 3]| RColor::from_rgb(c[0], c[1], c[2]);
        let mut plot = Plot::new()
            .size_px(size.x, size.y)
            .title(self.title.as_str())
            .xlabel(self.xlabel.as_str())
            .ylabel(self.ylabel.as_str())
            .line(&first.x, &first.y)
            .label(first.label.as_str())
            .color(color(first.color))
            .line_width(first.width);
        for s in series {
            plot = plot
                .line(&s.x, &s.y)
                .label(s.label.as_str())
                .color(color(s.color))
                .line_width(s.width);
        }
        if let Some((x, label)) = &self.marker {
            let (lo, hi) = self
                .series
                .iter()
                .flat_map(|s| s.y.iter().copied())
                .filter(|v| v.is_finite())
                .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
            if lo <= hi {
                plot = plot
                    .line(&[*x, *x], &[lo, hi])
                    .label(label.as_str())
                    .color(RColor::from_rgb(90, 90, 96))
                    .line_style(LineStyle::Dashed)
                    .line_width(1.5);
            }
        }
        plot.legend_best().render().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
fn rgb(c: Color) -> [u8; 3] {
    let s = c.to_srgba();
    [
        (s.red * 255.0) as u8,
        (s.green * 255.0) as u8,
        (s.blue * 255.0) as u8,
    ]
}

/// Distinct colours for loss overlays (Okabe–Ito, after the class colours).
const LOSS_COLORS: [[u8; 3]; 7] = [
    [0, 114, 178],
    [213, 94, 0],
    [0, 158, 115],
    [204, 121, 167],
    [230, 159, 0],
    [86, 180, 233],
    [0, 0, 0],
];
const PANE_COLORS: [[u8; 3]; 3] = [[0, 114, 178], [213, 94, 0], [0, 158, 115]];

/// Loss value against margin (classification) or residual (regression).
pub fn loss_curve_chart(
    task: Task,
    losses: &[LossKind],
    params: &LossParams,
    active: &[LossKind],
    range: f64,
) -> LineChart {
    let n = 301;
    let x: Vec<f64> = (0..n)
        .map(|i| -range + 2.0 * range * i as f64 / (n - 1) as f64)
        .collect();
    let series = losses
        .iter()
        .filter(|l| l.task() == task)
        .map(|&l| {
            let idx = LossKind::ALL.iter().position(|&k| k == l).unwrap_or(0);
            Series {
                label: l.name().to_string(),
                y: x.iter().map(|&v| l.value(v, params)).collect(),
                x: x.clone(),
                color: LOSS_COLORS[idx],
                width: if active.contains(&l) { 3.0 } else { 1.6 },
            }
        })
        .collect();
    let (title, xlabel) = match task {
        Task::Classification => ("Classification losses", "margin m = y·f(x)"),
        Task::Regression => ("Regression losses", "residual r = ŷ − y"),
    };
    LineChart {
        title: title.into(),
        xlabel: xlabel.into(),
        ylabel: "loss".into(),
        series,
        marker: None,
    }
}

/// At most `max` evenly spaced points, always keeping the last.
fn downsample(points: &[(usize, f64)], max: usize) -> (Vec<f64>, Vec<f64>) {
    let stride = points.len().div_ceil(max.max(1)).max(1);
    let mut kept: Vec<&(usize, f64)> = points.iter().step_by(stride).collect();
    if let Some(last) = points.last()
        && kept.last().map(|p| p.0) != Some(last.0)
    {
        kept.push(last);
    }
    (
        kept.iter().map(|p| p.0 as f64).collect(),
        kept.iter().map(|p| p.1).collect(),
    )
}

fn training_chart(experiment: &Experiment, playback: &Playback, sweep: &Sweep) -> LineChart {
    if sweep.active && !sweep.values.is_empty() {
        let log = sweep.spec.log;
        let xs: Vec<f64> = sweep
            .values
            .iter()
            .map(|&v| if log { v.log10() } else { v })
            .collect();
        let name = sweep.spec.param.name();
        let series = (0..experiment.panes.len())
            .map(|p| {
                let (x, y): (Vec<f64>, Vec<f64>) = sweep.results[p]
                    .iter()
                    .zip(&xs)
                    .filter_map(|(r, &x)| r.as_ref()?.as_ref().ok().map(|s| (x, s.loss)))
                    .unzip();
                Series {
                    label: format!("pane {}", p + 1),
                    x,
                    y,
                    color: PANE_COLORS[p % 3],
                    width: 2.2,
                }
            })
            .collect();
        let marker = sweep.current_value().map(|v| {
            (
                if log { v.log10() } else { v },
                format!("{name} = {}", fmt_num(v)),
            )
        });
        return LineChart {
            title: "Converged objective across the sweep".into(),
            xlabel: if log {
                format!("log₁₀ {name}")
            } else {
                name.to_string()
            },
            ylabel: "objective".into(),
            series,
            marker,
        };
    }
    let series = experiment
        .panes
        .iter()
        .enumerate()
        .map(|(p, pane)| {
            let (x, y) = downsample(&pane.trainer.loss_curve(), 400);
            Series {
                label: format!("pane {}", p + 1),
                x,
                y,
                color: PANE_COLORS[p % 3],
                width: 2.2,
            }
        })
        .collect();
    LineChart {
        title: "Training objective".into(),
        xlabel: "step".into(),
        ylabel: "mean loss + regularization".into(),
        series,
        marker: (playback.cursor > 0)
            .then(|| (playback.cursor as f64, format!("step {}", playback.cursor))),
    }
}

/// Chart inputs for this frame; starts renders whose inputs changed.
fn request_charts(
    time: Res<Time<Real>>,
    settings: Res<ChartSettings>,
    experiment: Res<Experiment>,
    playback: Res<Playback>,
    sweep: Res<Sweep>,
    mut charts: ResMut<Charts>,
) {
    let now = time.elapsed_secs();
    let size = settings.size;
    let configs: Vec<_> = experiment
        .panes
        .iter()
        .map(|p| p.trainer.config())
        .collect();
    let active: Vec<LossKind> = configs.iter().map(|c| c.loss).collect();
    let params = configs.first().map(|c| c.loss_params).unwrap_or_default();
    let overlay: Vec<LossKind> = if settings.overlay.is_empty() {
        LossKind::for_task(experiment.task).collect()
    } else {
        settings.overlay.clone()
    };

    let mut wanted: Vec<(fn(&mut Charts) -> &mut ChartSlot, Option<LineChart>)> = Vec::new();
    for task in [Task::Classification, Task::Regression] {
        let chart = (settings.show_loss_curves && overlay.iter().any(|l| l.task() == task))
            .then(|| loss_curve_chart(task, &overlay, &params, &active, settings.range));
        let slot: fn(&mut Charts) -> &mut ChartSlot = match task {
            Task::Classification => |c| &mut c.classification,
            Task::Regression => |c| &mut c.regression,
        };
        wanted.push((slot, chart));
    }
    wanted.push((
        |c| &mut c.training,
        settings
            .show_training
            .then(|| training_chart(&experiment, &playback, &sweep)),
    ));

    for (slot_of, chart) in wanted {
        let charts = &mut *charts;
        let slot = slot_of(charts);
        slot.visible = chart.is_some();
        let Some(chart) = chart else { continue };
        let key = chart.key(size);
        slot.wanted = Some(key);
        if slot.shown == Some(key)
            || slot.pending.is_some()
            || now - slot.last_spawn < MIN_RENDER_INTERVAL
        {
            continue;
        }
        slot.last_spawn = now;
        slot.pending = Some((
            key,
            AsyncComputeTaskPool::get().spawn(async move { chart.render(size) }),
        ));
        charts.renders += 1;
    }
}

fn collect_charts(mut charts: ResMut<Charts>, mut images: ResMut<Assets<Image>>) {
    let charts = &mut *charts;
    for slot in [
        &mut charts.classification,
        &mut charts.regression,
        &mut charts.training,
    ] {
        let Some((key, task)) = slot.pending.as_mut() else {
            continue;
        };
        let Some(result) = check_ready(task) else {
            continue;
        };
        let key = *key;
        slot.pending = None;
        match result {
            // Discard renders whose inputs have since changed.
            Ok(img) if slot.wanted == Some(key) => {
                let new = Image::new(
                    Extent3d {
                        width: img.width,
                        height: img.height,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    img.pixels,
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                if let Some(mut existing) = images.get_mut(&slot.image) {
                    *existing = new;
                }
                slot.shown = Some(key);
            }
            Ok(_) => {}
            Err(e) => {
                warn!("bevaru: chart render failed: {e}");
                // Don't retry the same inputs every frame.
                slot.shown = Some(key);
            }
        }
    }
}

/// Diverging colours: positive weights toward class 0's colour, negative
/// toward class 1's, white at zero — matching the decision regions.
pub fn weight_pixels(w: &[f64], max_abs: f64) -> Vec<u8> {
    let pos = CLASS_COLORS[0].to_srgba();
    let neg = CLASS_COLORS[1].to_srgba();
    let mut px = Vec::with_capacity(w.len() * 4);
    for &v in w {
        let t = if max_abs > 0.0 {
            (v / max_abs).clamp(-1.0, 1.0) as f32
        } else {
            0.0
        };
        let c = if t >= 0.0 { pos } else { neg };
        let a = t.abs();
        let mix = |ch: f32| ((1.0 - a) * 255.0 + a * ch * 255.0) as u8;
        px.extend_from_slice(&[mix(c.red), mix(c.green), mix(c.blue), 255]);
    }
    px
}

fn update_weight_images(
    experiment: Res<Experiment>,
    views: Res<PaneViews>,
    mut weights: ResMut<WeightImages>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(side) = experiment.image_side else {
        weights.images.clear();
        return;
    };
    // Weights are only a picture when training used every pixel.
    if experiment.data.x.ncols() != side * side {
        weights.images.clear();
        return;
    }
    while weights.images.len() < experiment.panes.len() {
        let mut img = blank(UVec2::splat(side as u32));
        img.sampler = ImageSampler::nearest();
        weights.images.push(images.add(img));
        weights.max_abs.push(0.0);
    }
    weights.images.truncate(experiment.panes.len());
    weights.max_abs.truncate(experiment.panes.len());
    for (p, view) in views.0.iter().enumerate().take(weights.images.len()) {
        let Some(model) = &view.model else { continue };
        let max_abs = model.w.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        weights.max_abs[p] = max_abs;
        if let Some(mut img) = images.get_mut(&weights.images[p]) {
            img.data = Some(weight_pixels(model.w.as_slice(), max_abs));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::{ExperimentPlugin, StartupExperiment};
    use crate::playback::{PlaybackCommand, PlaybackPlugin};
    use crate::{ExperimentSpec, core::TrainerConfig};
    use std::time::Duration;

    #[test]
    fn classification_chart_overlays_selected_losses() {
        let losses = [
            LossKind::Hinge,
            LossKind::Logistic,
            LossKind::ZeroOne,
            LossKind::Mse,
        ];
        let c = loss_curve_chart(
            Task::Classification,
            &losses,
            &LossParams::default(),
            &[LossKind::Hinge],
            3.0,
        );
        let names: Vec<&str> = c.series.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(names, ["Hinge", "Logistic (log loss)", "0-1"]);
        assert_eq!(
            (c.series[0].x[0], *c.series[0].x.last().unwrap()),
            (-3.0, 3.0)
        );
        assert!(
            c.series[0].width > c.series[1].width,
            "active loss emphasized"
        );
    }

    #[test]
    fn huber_curve_tracks_delta() {
        let p = |d| LossParams::default().with_huber_delta(d).unwrap();
        let a = loss_curve_chart(Task::Regression, &[LossKind::Huber], &p(1.0), &[], 3.0);
        let b = loss_curve_chart(Task::Regression, &[LossKind::Huber], &p(2.0), &[], 3.0);
        assert_ne!(a.series[0].y, b.series[0].y);
        assert_ne!(a.key(UVec2::ONE), b.key(UVec2::ONE));
    }

    #[test]
    fn charts_render_to_requested_size() {
        let c = loss_curve_chart(
            Task::Classification,
            &LossKind::ALL,
            &LossParams::default(),
            &[],
            3.0,
        );
        let img = c.render(UVec2::new(320, 200)).unwrap();
        assert_eq!((img.width, img.height), (320, 200));
        assert_eq!(img.pixels.len(), 320 * 200 * 4);
    }

    #[test]
    fn downsample_keeps_endpoints() {
        let pts: Vec<(usize, f64)> = (0..=1000).map(|i| (i, i as f64)).collect();
        let (x, _) = downsample(&pts, 100);
        assert!(x.len() <= 102);
        assert_eq!((x[0], *x.last().unwrap()), (0.0, 1000.0));
    }

    #[test]
    fn weight_colours_are_diverging() {
        let px = weight_pixels(&[1.0, 0.0, -1.0], 1.0);
        assert_eq!(&px[4..8], &[255, 255, 255, 255], "zero is white");
        assert_eq!(px[0..3], rgb(CLASS_COLORS[0]));
        assert_eq!(px[8..11], rgb(CLASS_COLORS[1]));
    }

    fn chart_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            .add_plugins((ExperimentPlugin, PlaybackPlugin, ChartsPlugin))
            .insert_resource(StartupExperiment(ExperimentSpec::default()));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(50),
        ));
        app
    }

    fn settle(app: &mut App) {
        for _ in 0..400 {
            app.update();
            let charts = app.world().resource::<Charts>();
            let idle = [&charts.classification, &charts.training]
                .iter()
                .all(|s| s.pending.is_none() && s.shown == s.wanted);
            if app.world().contains_resource::<Experiment>() && idle && charts.renders > 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("charts never settled");
    }

    #[test]
    fn no_renders_without_input_changes_and_one_on_change() {
        let mut app = chart_app();
        settle(&mut app);
        let before = app.world().resource::<Charts>().renders;
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Charts>().renders,
            before,
            "60 idle frames, no renders"
        );
        assert!(app.world().resource::<Charts>().training.is_ready());

        // A training step changes the training chart only.
        app.world_mut().write_message(PlaybackCommand::Step);
        settle(&mut app);
        assert_eq!(app.world().resource::<Charts>().renders, before + 1);

        // Changing a loss parameter re-renders the loss-curve chart.
        let mut cfg = app.world().resource::<Experiment>().panes[0]
            .trainer
            .config()
            .clone();
        cfg.loss_params = cfg.loss_params.with_margin(0.5).unwrap();
        app.world_mut()
            .resource_mut::<Experiment>()
            .update_pane(0, cfg)
            .unwrap();
        settle(&mut app);
        assert_eq!(app.world().resource::<Charts>().renders, before + 2);
        let _ = TrainerConfig::logistic();
    }
}
