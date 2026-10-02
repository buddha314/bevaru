//! Training playback, hyperparameter sweeps, and the per-pane state the scene
//! draws (an interpolated boundary chasing the target snapshot).

use bevaru_core::{LinearModel, LossKind, ModelKind, Snapshot, Status, Trainer, TrainerConfig};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task as BevyTask, futures::check_ready};

use crate::experiment::Experiment;
use crate::geometry::Boundary;

/// Training playback, shared by every pane so comparisons stay in lockstep.
#[derive(Resource, Debug, Clone)]
pub struct Playback {
    pub playing: bool,
    pub steps_per_second: f32,
    /// The step being displayed.
    pub cursor: usize,
    /// When true the cursor tracks the newest step; scrubbing clears it.
    pub follow: bool,
    /// Time for the displayed boundary to settle on a new target, seconds.
    pub transition_secs: f32,
    accumulator: f32,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            playing: false,
            steps_per_second: 30.0,
            cursor: 0,
            follow: true,
            transition_secs: 0.3,
            accumulator: 0.0,
        }
    }
}

/// Upper bound on steps per frame, so a high speed cannot stall rendering.
const MAX_STEPS_PER_FRAME: usize = 200;
/// Wall-clock time per frame that training may use.
const TRAINING_BUDGET: std::time::Duration = std::time::Duration::from_millis(8);

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum PlaybackCommand {
    Play,
    Pause,
    Toggle,
    /// Advance every pane exactly one step (pauses first).
    Step,
    /// Restart every pane from step 0.
    Reset,
    /// Show an earlier step without re-running training.
    Seek(usize),
}

/// A hyperparameter that can be swept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SweepParam {
    C,
    Lambda,
    HuberDelta,
    Margin,
    LearningRate,
}

impl SweepParam {
    pub const ALL: [SweepParam; 5] = [
        SweepParam::C,
        SweepParam::Lambda,
        SweepParam::HuberDelta,
        SweepParam::Margin,
        SweepParam::LearningRate,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SweepParam::C => "C",
            SweepParam::Lambda => "λ",
            SweepParam::HuberDelta => "Huber δ",
            SweepParam::Margin => "Hinge margin",
            SweepParam::LearningRate => "Learning rate",
        }
    }

    pub fn applies_to(self, cfg: &TrainerConfig) -> bool {
        match self {
            SweepParam::C => cfg.model == ModelKind::Svm,
            SweepParam::Lambda | SweepParam::LearningRate => true,
            SweepParam::HuberDelta => cfg.loss == LossKind::Huber,
            SweepParam::Margin => matches!(cfg.loss, LossKind::Hinge | LossKind::SquaredHinge),
        }
    }

    /// `cfg` with this parameter set to `value`.
    pub fn apply(self, cfg: &TrainerConfig, value: f64) -> Result<TrainerConfig, String> {
        let mut cfg = cfg.clone();
        match self {
            SweepParam::C => return cfg.with_c(value).map_err(|e| e.to_string()),
            SweepParam::Lambda => cfg.lambda = value,
            SweepParam::HuberDelta => {
                cfg.loss_params = cfg
                    .loss_params
                    .with_huber_delta(value)
                    .map_err(|e| e.to_string())?
            }
            SweepParam::Margin => {
                cfg.loss_params = cfg
                    .loss_params
                    .with_margin(value)
                    .map_err(|e| e.to_string())?
            }
            SweepParam::LearningRate => {
                cfg.learning_rate = match cfg.learning_rate {
                    bevaru_core::LearningRate::Constant(_) => {
                        bevaru_core::LearningRate::Constant(value)
                    }
                    bevaru_core::LearningRate::InverseDecay { initial, decay } => {
                        // Keep the decay horizon, change the starting step.
                        bevaru_core::LearningRate::InverseDecay {
                            initial: value,
                            decay: decay * value / initial,
                        }
                    }
                }
            }
        }
        Ok(cfg)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SweepSpec {
    pub param: SweepParam,
    pub from: f64,
    pub to: f64,
    pub samples: usize,
    pub log: bool,
}

impl Default for SweepSpec {
    fn default() -> Self {
        Self {
            param: SweepParam::C,
            from: 0.01,
            to: 100.0,
            samples: 15,
            log: true,
        }
    }
}

impl SweepSpec {
    pub fn values(&self) -> Vec<f64> {
        let n = self.samples.max(2);
        (0..n)
            .map(|i| {
                let t = i as f64 / (n - 1) as f64;
                if self.log && self.from > 0.0 && self.to > 0.0 {
                    (self.from.ln() + t * (self.to.ln() - self.from.ln())).exp()
                } else {
                    self.from + t * (self.to - self.from)
                }
            })
            .collect()
    }
}

/// A sweep: every pane trained to convergence at every sampled value, off
/// the render thread, then played back as an animation.
#[derive(Resource, Default)]
pub struct Sweep {
    pub spec: SweepSpec,
    pub active: bool,
    pub playing: bool,
    pub values: Vec<f64>,
    /// `results[pane][value]`.
    pub results: Vec<Vec<Option<Result<Snapshot, String>>>>,
    pub index: usize,
    pub secs_per_value: f32,
    timer: f32,
    tasks: Vec<(usize, usize, BevyTask<Result<Snapshot, String>>)>,
    generation: u64,
    pane_revision: u64,
}

impl Sweep {
    pub fn progress(&self) -> (usize, usize) {
        let done = self
            .results
            .iter()
            .flatten()
            .filter(|r| r.is_some())
            .count();
        (done, self.results.iter().map(Vec::len).sum())
    }

    pub fn is_computing(&self) -> bool {
        !self.tasks.is_empty()
    }

    fn value_ready(&self, i: usize) -> bool {
        self.results
            .iter()
            .all(|pane| pane.get(i).is_some_and(Option::is_some))
    }

    pub fn current_value(&self) -> Option<f64> {
        self.values.get(self.index).copied()
    }

    /// The finished solution shown for `pane` at the current value.
    pub fn result(&self, pane: usize) -> Option<&Snapshot> {
        self.results
            .get(pane)?
            .get(self.index)?
            .as_ref()?
            .as_ref()
            .ok()
    }
}

#[derive(Message, Debug, Clone, PartialEq)]
pub enum SweepCommand {
    /// Compute and start playing a sweep.
    Start(SweepSpec),
    Stop,
    Play,
    Pause,
    Seek(usize),
}

/// What one pane currently shows.
#[derive(Debug, Clone, Default)]
pub struct PaneView {
    /// Interpolated boundary actually drawn this frame.
    pub shown: Option<Boundary>,
    /// Where `shown` is heading.
    pub target: Option<Boundary>,
    /// Model behind `target`, for residuals and weight images.
    pub model: Option<LinearModel>,
    pub support_vectors: Vec<usize>,
    pub step: usize,
    pub loss: f64,
    pub status: Option<Status>,
}

#[derive(Resource, Debug, Default)]
pub struct PaneViews(pub Vec<PaneView>);

/// Systems that advance training and sweeps and update [`PaneViews`];
/// rendering runs after them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlaybackSystems;

pub struct PlaybackPlugin;

impl Plugin for PlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Playback>()
            .init_resource::<Sweep>()
            .init_resource::<PaneViews>()
            .add_message::<PlaybackCommand>()
            .add_message::<SweepCommand>()
            .add_systems(
                Update,
                (
                    on_new_experiment,
                    handle_playback_commands,
                    handle_sweep_commands,
                    advance_training,
                    poll_sweep,
                    advance_sweep,
                    update_views,
                )
                    .chain()
                    .in_set(PlaybackSystems)
                    .run_if(resource_exists::<Experiment>),
            );
    }
}

fn on_new_experiment(
    experiment: Res<Experiment>,
    mut playback: ResMut<Playback>,
    mut sweep: ResMut<Sweep>,
    mut views: ResMut<PaneViews>,
) {
    if sweep.generation != experiment.generation {
        *sweep = Sweep {
            spec: sweep.spec.clone(),
            generation: experiment.generation,
            pane_revision: experiment.pane_revision,
            ..default()
        };
        playback.cursor = 0;
        playback.follow = true;
        views.0.clear();
    } else if sweep.pane_revision != experiment.pane_revision {
        *sweep = Sweep {
            spec: sweep.spec.clone(),
            secs_per_value: sweep.secs_per_value,
            generation: experiment.generation,
            pane_revision: experiment.pane_revision,
            ..default()
        };
    }
    views
        .0
        .resize_with(experiment.panes.len(), PaneView::default);
}

fn handle_playback_commands(
    mut commands: MessageReader<PlaybackCommand>,
    mut playback: ResMut<Playback>,
    mut experiment: ResMut<Experiment>,
    mut sweep: ResMut<Sweep>,
) {
    for cmd in commands.read() {
        match *cmd {
            PlaybackCommand::Play => {
                playback.playing = true;
                playback.follow = true;
                sweep.active = false;
            }
            PlaybackCommand::Pause => playback.playing = false,
            PlaybackCommand::Toggle => {
                playback.playing = !playback.playing;
                if playback.playing {
                    playback.follow = true;
                    sweep.active = false;
                }
            }
            PlaybackCommand::Step => {
                playback.playing = false;
                sweep.active = false;
                for pane in &mut experiment.panes {
                    pane.trainer.step();
                }
                playback.cursor = experiment.max_step();
                playback.follow = true;
            }
            PlaybackCommand::Reset => {
                experiment.reset();
                sweep.active = false;
                sweep.playing = false;
                playback.cursor = 0;
                playback.follow = true;
                playback.playing = false;
            }
            PlaybackCommand::Seek(step) => {
                playback.cursor = step.min(experiment.max_step());
                playback.follow = playback.cursor == experiment.max_step();
                playback.playing = false;
                sweep.active = false;
            }
        }
    }
}

fn advance_training(
    time: Res<Time>,
    mut playback: ResMut<Playback>,
    mut experiment: ResMut<Experiment>,
) {
    if !playback.playing {
        playback.accumulator = 0.0;
        return;
    }
    playback.accumulator += time.delta_secs() * playback.steps_per_second.max(0.0);
    let n = (playback.accumulator.floor() as usize).min(MAX_STEPS_PER_FRAME);
    playback.accumulator -= n as f32;
    playback.accumulator = playback.accumulator.min(MAX_STEPS_PER_FRAME as f32);
    // Stay responsive with expensive models: stop when the frame's training
    // budget is spent, and drop the backlog rather than chase it.
    let start = std::time::Instant::now();
    for _ in 0..n {
        for pane in &mut experiment.panes {
            pane.trainer.step();
        }
        if start.elapsed() > TRAINING_BUDGET {
            playback.accumulator = 0.0;
            break;
        }
    }
    if playback.follow {
        playback.cursor = experiment.max_step();
    }
    if experiment
        .panes
        .iter()
        .all(|p| p.trainer.status() != Status::Running)
    {
        playback.playing = false;
    }
}

fn handle_sweep_commands(
    mut commands: MessageReader<SweepCommand>,
    mut sweep: ResMut<Sweep>,
    mut playback: ResMut<Playback>,
    experiment: Res<Experiment>,
) {
    for cmd in commands.read() {
        match cmd {
            SweepCommand::Start(spec) => {
                playback.playing = false;
                let values = spec.values();
                let pool = AsyncComputeTaskPool::get();
                let mut tasks = Vec::new();
                for (p, pane) in experiment.panes.iter().enumerate() {
                    let base = pane.trainer.config().clone();
                    for (i, &v) in values.iter().enumerate() {
                        let data = experiment.data.clone();
                        let param = spec.param;
                        let base = base.clone();
                        tasks.push((
                            p,
                            i,
                            pool.spawn(async move {
                                // Inapplicable parameters (e.g. C on a regression pane)
                                // leave the pane's own configuration in place.
                                let cfg = if param.applies_to(&base) {
                                    param.apply(&base, v)?
                                } else {
                                    base
                                };
                                let mut t = Trainer::new(cfg, data).map_err(|e| e.to_string())?;
                                match t.run() {
                                    Status::Diverged => Err("diverged".to_string()),
                                    _ => Ok(t.latest().clone()),
                                }
                            }),
                        ));
                    }
                }
                *sweep = Sweep {
                    spec: spec.clone(),
                    active: true,
                    playing: true,
                    results: vec![vec![None; values.len()]; experiment.panes.len()],
                    values,
                    index: 0,
                    secs_per_value: if sweep.secs_per_value > 0.0 {
                        sweep.secs_per_value
                    } else {
                        0.6
                    },
                    timer: 0.0,
                    tasks,
                    generation: experiment.generation,
                    pane_revision: experiment.pane_revision,
                };
            }
            SweepCommand::Stop => {
                sweep.active = false;
                sweep.playing = false;
                sweep.tasks.clear();
            }
            SweepCommand::Play => {
                if !sweep.values.is_empty() {
                    sweep.active = true;
                    sweep.playing = true;
                    playback.playing = false;
                }
            }
            SweepCommand::Pause => sweep.playing = false,
            SweepCommand::Seek(i) => {
                if *i < sweep.values.len() {
                    sweep.active = true;
                    sweep.playing = false;
                    sweep.index = *i;
                }
            }
        }
    }
}

fn poll_sweep(mut sweep: ResMut<Sweep>) {
    let mut finished = Vec::new();
    sweep
        .tasks
        .retain_mut(|(p, i, task)| match check_ready(task) {
            Some(r) => {
                finished.push((*p, *i, r));
                false
            }
            None => true,
        });
    for (p, i, r) in finished {
        sweep.results[p][i] = Some(r);
    }
}

fn advance_sweep(time: Res<Time>, mut sweep: ResMut<Sweep>) {
    if !(sweep.active && sweep.playing) || sweep.values.is_empty() {
        return;
    }
    sweep.timer += time.delta_secs();
    let next = (sweep.index + 1) % sweep.values.len();
    // Hold on the current value until the next one has been computed.
    if sweep.timer >= sweep.secs_per_value && sweep.value_ready(next) {
        sweep.timer = 0.0;
        sweep.index = next;
    }
}

fn update_views(
    time: Res<Time>,
    playback: Res<Playback>,
    sweep: Res<Sweep>,
    experiment: Res<Experiment>,
    mut views: ResMut<PaneViews>,
) {
    // Exponential approach: ~99% of the way after `transition_secs`. Unlike a
    // fixed tween, this stays smooth when the target moves every frame.
    let alpha = if playback.transition_secs > 0.0 {
        1.0 - (-4.6 * time.delta_secs() / playback.transition_secs).exp()
    } else {
        1.0
    };
    for (i, pane) in experiment.panes.iter().enumerate() {
        let Some(view) = views.0.get_mut(i) else {
            continue;
        };
        let snap = match (sweep.active, sweep.result(i)) {
            (true, Some(s)) => s,
            (true, None) if view.model.is_some() => continue, // hold until computed
            _ => pane.trainer.snapshot(playback.cursor),
        };
        view.target = experiment.boundary(&snap.model);
        view.model = Some(snap.model.clone());
        view.support_vectors.clone_from(&snap.support_vectors);
        view.step = snap.step;
        view.loss = snap.loss;
        view.status = Some(pane.trainer.status());
        view.shown = match (view.shown, view.target) {
            (Some(shown), Some(target)) if alpha < 1.0 => {
                let next = Boundary::lerp(&shown, &target, alpha);
                // Snap once visually indistinguishable.
                let close = next.normal.dot(target.normal) > 1.0 - 1e-7
                    && (next.offset - target.offset).abs() < 1e-4
                    && (next.scale / target.scale - 1.0).abs() < 1e-4;
                Some(if close { target } else { next })
            }
            (_, target) => target,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::{ExperimentPlugin, ExperimentSpec};
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins((ExperimentPlugin, PlaybackPlugin));
        // Fixed frame time so speeds are deterministic.
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(100),
        ));
        app
    }

    /// Run frames until the experiment has loaded.
    fn wait_for_experiment(app: &mut App) {
        for _ in 0..5_000 {
            app.update();
            if app.world().contains_resource::<Experiment>() {
                app.update();
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("experiment never loaded");
    }

    fn send(app: &mut App, cmd: PlaybackCommand) {
        app.world_mut().write_message(cmd);
        app.update();
    }

    #[test]
    fn pause_halts_and_step_advances_exactly_one() {
        let mut app = app();
        wait_for_experiment(&mut app);
        let steps = |app: &App| {
            app.world().resource::<Experiment>().panes[0]
                .trainer
                .steps_taken()
        };
        assert_eq!(steps(&app), 0);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(steps(&app), 0, "paused by default");
        send(&mut app, PlaybackCommand::Step);
        assert_eq!(steps(&app), 1);
        assert_eq!(app.world().resource::<PaneViews>().0[0].step, 1);
        send(&mut app, PlaybackCommand::Play);
        for _ in 0..5 {
            app.update();
        }
        let played = steps(&app);
        assert!(played > 1);
        send(&mut app, PlaybackCommand::Pause);
        let shown = app.world().resource::<PaneViews>().0[0].target;
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(steps(&app), played);
        assert_eq!(
            app.world().resource::<PaneViews>().0[0].target,
            shown,
            "boundary stays fixed"
        );
    }

    #[test]
    fn scrubbing_shows_earlier_snapshot_without_retraining() {
        let mut app = app();
        wait_for_experiment(&mut app);
        for _ in 0..30 {
            send(&mut app, PlaybackCommand::Step);
        }
        send(&mut app, PlaybackCommand::Seek(10));
        let world = app.world();
        let e = world.resource::<Experiment>();
        assert_eq!(e.panes[0].trainer.steps_taken(), 30);
        let view = &world.resource::<PaneViews>().0[0];
        assert_eq!(view.step, 10);
        assert_eq!(
            view.model.as_ref(),
            Some(&e.panes[0].trainer.snapshot(10).model)
        );
    }

    #[test]
    fn comparison_panes_stay_in_lockstep() {
        let mut app = app();
        let spec = ExperimentSpec::default().compare(TrainerConfig::logistic());
        app.insert_resource(crate::experiment::StartupExperiment(spec));
        wait_for_experiment(&mut app);
        for _ in 0..10 {
            send(&mut app, PlaybackCommand::Step);
        }
        let views = &app.world().resource::<PaneViews>().0;
        assert_eq!(views.len(), 2);
        assert!(views.iter().all(|v| v.step == 10));
    }

    #[test]
    fn sweep_computes_off_thread_and_animates() {
        let mut app = app();
        wait_for_experiment(&mut app);
        let spec = SweepSpec {
            samples: 4,
            ..default()
        };
        app.world_mut().write_message(SweepCommand::Start(spec));
        for _ in 0..2000 {
            app.update();
            if !app.world().resource::<Sweep>().is_computing() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let sweep = app.world().resource::<Sweep>();
        assert_eq!(sweep.progress(), (4, 4));
        let c = sweep.current_value().unwrap();
        // The pane shows the converged solution for the current C.
        let view = &app.world().resource::<PaneViews>().0[0];
        assert_eq!(view.model.as_ref(), sweep.result(0).map(|s| &s.model));
        // 100 ms frames, 0.6 s per value: the animation advances.
        for _ in 0..8 {
            app.update();
        }
        assert_ne!(app.world().resource::<Sweep>().current_value().unwrap(), c);
    }

    #[test]
    fn changing_panes_discards_sweep_results() {
        let mut app = app();
        wait_for_experiment(&mut app);
        let spec = SweepSpec {
            samples: 3,
            ..default()
        };

        app.world_mut()
            .write_message(SweepCommand::Start(spec.clone()));
        app.update();
        assert_eq!(app.world().resource::<Sweep>().results.len(), 1);
        app.world_mut()
            .resource_mut::<Experiment>()
            .add_pane(TrainerConfig::logistic())
            .unwrap();
        app.update();
        let sweep = app.world().resource::<Sweep>();
        assert!(!sweep.active);
        assert!(sweep.results.is_empty());
        assert_eq!(app.world().resource::<PaneViews>().0.len(), 2);

        app.world_mut()
            .write_message(SweepCommand::Start(spec.clone()));
        app.update();
        assert_eq!(app.world().resource::<Sweep>().results.len(), 2);
        let experiment = app.world_mut().resource_mut::<Experiment>();
        let cfg = experiment.panes[0]
            .trainer
            .config()
            .clone()
            .with_c(5.0)
            .unwrap();
        experiment.into_inner().update_pane(0, cfg).unwrap();
        app.update();
        let sweep = app.world().resource::<Sweep>();
        assert!(!sweep.active);
        assert!(sweep.results.is_empty());
        assert_eq!(
            app.world().resource::<Experiment>().panes[0]
                .trainer
                .config()
                .c(),
            Some(5.0)
        );

        app.world_mut().write_message(SweepCommand::Start(spec));
        app.update();
        app.world_mut().resource_mut::<Experiment>().remove_pane(0);
        app.update();
        assert!(!app.world().resource::<Sweep>().active);
        assert!(app.world().resource::<Sweep>().results.is_empty());
        assert_eq!(app.world().resource::<PaneViews>().0.len(), 1);
    }

    #[test]
    fn interpolation_is_gradual_then_settles() {
        let mut app = app();
        wait_for_experiment(&mut app);
        app.world_mut().resource_mut::<Playback>().transition_secs = 0.3;
        // Step 0 has w = 0 and no boundary; train a little and settle first.
        for _ in 0..20 {
            send(&mut app, PlaybackCommand::Step);
        }
        for _ in 0..10 {
            app.update();
        }
        for _ in 0..40 {
            app.world_mut().write_message(PlaybackCommand::Step);
        }
        app.update();
        let view = app.world().resource::<PaneViews>().0[0].clone();
        let (shown, target) = (view.shown.unwrap(), view.target.unwrap());
        // First frame after a jump of 40 steps: partway there.
        assert_ne!(shown, target);
        for _ in 0..10 {
            app.update();
        }
        let view = &app.world().resource::<PaneViews>().0[0];
        assert_eq!(view.shown, view.target, "settles within ~0.3 s");
    }

    #[test]
    fn log_sweep_values_span_range() {
        let v = SweepSpec {
            from: 0.01,
            to: 100.0,
            samples: 5,
            log: true,
            ..default()
        }
        .values();
        let expected = [0.01, 0.1, 1.0, 10.0, 100.0];
        for (a, b) in v.iter().zip(expected) {
            assert!((a / b - 1.0).abs() < 1e-9);
        }
    }
}
