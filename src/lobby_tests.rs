//! Headless tests of the lobby, the experience lifecycle, and clean-up on
//! leave. They run the real scene and chart plugins without a GPU.

use std::time::Duration;

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::ecs::resource::IsResource;
use bevy::gizmos::GizmoPlugin;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;

use super::*;
use crate::SigmoidPlotPlugin;
use crate::charts::{ChartSettings, Charts, ChartsPlugin, WeightImages};
use crate::experiences::sigmoid::{SigmoidExperiencePlugin, SigmoidRefresh, SigmoidSprite};
use crate::experiences::{
    Experience, ExperienceEntity, ExperienceKind, RegisterExperience, Requirement,
};
use crate::experiment::{
    DatasetChoice, Experiment, ExperimentLoader, ExperimentPlugin, ExperimentSpec, View,
};
use crate::playback::{PaneViews, PlaybackPlugin, Sweep};
use crate::scene::{PaneCamera, SceneEntity, ScenePlugin};

/// Everything `DefaultPlugins` + `BevaruPlugin` + `LobbyPlugin` would add,
/// minus the window, renderer, and egui.
fn headless(lobby: Option<LobbyPlugin>) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        GizmoPlugin,
        StatesPlugin,
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<Image>()
    .init_asset::<SkinnedMeshInverseBindposes>()
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        50,
    )))
    .add_plugins((
        SigmoidPlotPlugin,
        ExperimentPlugin,
        PlaybackPlugin,
        ScenePlugin,
        ChartsPlugin,
        ExperiencesPlugin,
        // Added by ExperiencesPlugin only when a renderer is present.
        SigmoidExperiencePlugin,
    ));
    if let Some(lobby) = lobby {
        app.add_plugins(lobby);
    }
    app
}

fn lobby_app() -> App {
    headless(Some(LobbyPlugin::default()))
}

/// Step until `done` holds (async loads and renders need wall-clock time).
fn run_until(app: &mut App, what: &str, done: impl Fn(&World) -> bool) {
    // Generous wall-clock limit: under a parallel test run, background loads
    // share the task pool with every other test.
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        app.update();
        if done(app.world()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("timed out waiting for {what}");
}

fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn screen(app: &App) -> AppScreen {
    *app.world().resource::<State<AppScreen>>().get()
}

fn count<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, F>()
        .iter(app.world())
        .count()
}

fn enter(app: &mut App, id: &'static str) {
    app.world_mut().write_message(EnterExperience(id));
    // The screen change lands a frame after the request; when switching,
    // the old experience is still "Running" until then.
    frames(app, 2);
    run_until(app, id, |w| {
        *w.resource::<State<AppScreen>>().get() == AppScreen::Running
            && w.resource::<ActiveExperience>().0 == Some(id)
    });
}

fn leave_now(app: &mut App) {
    app.world_mut().write_message(LeaveExperience);
    // Unload, despawn, then let the asset system free what was dropped.
    frames(app, 10);
}

#[derive(Debug, PartialEq, Eq)]
struct Counts {
    entities: usize,
    meshes: usize,
    materials: usize,
    images: usize,
}

fn counts(app: &mut App) -> Counts {
    Counts {
        // Bevy stores each resource type in an entity that it keeps and reuses
        // after the resource is removed; those are bookkeeping, not content.
        entities: count::<Without<IsResource>>(app),
        meshes: app.world().resource::<Assets<Mesh>>().len(),
        materials: app.world().resource::<Assets<StandardMaterial>>().len(),
        images: app.world().resource::<Assets<Image>>().len(),
    }
}

fn assert_clean(app: &mut App) {
    assert_eq!(screen(app), AppScreen::Lobby);
    assert!(!app.world().contains_resource::<Experiment>());
    assert!(!app.world().resource::<ExperimentLoader>().is_loading());
    assert!(!app.world().resource::<Sweep>().is_computing());
    assert!(app.world().resource::<PaneViews>().0.is_empty());
    assert!(app.world().resource::<WeightImages>().images.is_empty());
    assert_eq!(count::<With<SceneEntity>>(app), 0);
    assert_eq!(count::<With<PaneCamera>>(app), 0);
    assert_eq!(count::<With<ExperienceEntity>>(app), 0);
    assert!(!app.world().contains_resource::<SigmoidRefresh>());
}

#[test]
fn idle_start_loads_nothing() {
    let mut app = headless(None);
    app.insert_resource(StartupMode::Idle);
    frames(&mut app, 60);
    assert!(!app.world().contains_resource::<Experiment>());
    assert!(!app.world().resource::<ExperimentLoader>().is_loading());
    assert_eq!(count::<With<PaneCamera>>(&mut app), 0);
    assert_eq!(count::<With<SceneEntity>>(&mut app), 0);
    assert_eq!(app.world().resource::<Charts>().renders, 0);
}

#[test]
fn default_startup_still_loads() {
    let mut app = headless(None);
    run_until(&mut app, "default experiment", |w| {
        w.contains_resource::<Experiment>()
    });
}

#[test]
fn lobby_is_idle() {
    let mut app = lobby_app();
    frames(&mut app, 60);
    assert_eq!(screen(&app), AppScreen::Lobby);
    assert!(!app.world().resource::<ExperimentLoader>().is_loading());
    assert_eq!(app.world().resource::<Charts>().renders, 0);
    assert_clean(&mut app);
    assert_eq!(count::<With<SigmoidSprite>>(&mut app), 0);
}

#[test]
fn unload_leaves_nothing_and_reload_matches_fresh() {
    let two_panes = || ExperimentSpec::default().compare(bevaru_core::TrainerConfig::logistic());
    let mut app = headless(None);
    app.insert_resource(StartupMode::Idle);
    frames(&mut app, 5);
    let baseline = counts(&mut app);

    app.world_mut().write_message(LoadExperiment(two_panes()));
    run_until(&mut app, "scene", |w| w.contains_resource::<Experiment>());
    frames(&mut app, 5);
    assert_eq!(count::<With<PaneCamera>>(&mut app), 2);
    let loaded = count::<With<SceneEntity>>(&mut app);

    app.world_mut().write_message(UnloadExperiment);
    frames(&mut app, 10);
    assert!(!app.world().contains_resource::<Experiment>());
    assert_eq!(count::<With<SceneEntity>>(&mut app), 0);
    assert_eq!(count::<With<PaneCamera>>(&mut app), 0);
    assert_eq!(counts(&mut app), baseline);

    // Reloading after an unload builds the same scene a fresh load does.
    app.world_mut().write_message(LoadExperiment(two_panes()));
    run_until(&mut app, "reload", |w| w.contains_resource::<Experiment>());
    frames(&mut app, 5);
    assert_eq!(count::<With<SceneEntity>>(&mut app), loaded);
    assert_eq!(count::<With<PaneCamera>>(&mut app), 2);
}

#[test]
fn choosing_an_experiment_loads_it_and_runs_its_action() {
    let mut app = lobby_app();
    app.world_mut().write_message(EnterExperience("iris-svm"));
    app.update();
    app.update();
    assert_eq!(screen(&app), AppScreen::Loading);
    run_until(&mut app, "iris sweep", |w| w.resource::<Sweep>().active);
    assert_eq!(screen(&app), AppScreen::Running);
    assert_eq!(app.world().resource::<Experiment>().panes.len(), 2);
}

#[test]
fn leaving_mid_sweep_tears_everything_down() {
    let mut app = lobby_app();
    enter(&mut app, "iris-svm");
    // `active` is set when the sweep starts; whether its solutions are still
    // computing depends on machine load, and teardown must work either way.
    run_until(&mut app, "sweep started", |w| w.resource::<Sweep>().active);
    leave_now(&mut app);
    assert_clean(&mut app);
}

#[test]
fn leaving_during_a_load_discards_it() {
    let mut app = lobby_app();
    app.world_mut()
        .write_message(EnterExperience("regression-mse-vs-mae"));
    app.update();
    app.world_mut().write_message(LeaveExperience);
    // Long enough for the abandoned load to have finished several times over.
    for _ in 0..100 {
        app.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_clean(&mut app);
}

#[test]
fn round_trip_shows_only_the_second_experience() {
    let mut app = lobby_app();
    enter(&mut app, "regression-mse-vs-mae");
    frames(&mut app, 5);
    assert_eq!(count::<With<PaneCamera>>(&mut app), 3);
    leave_now(&mut app);
    enter(&mut app, "sigmoid");
    frames(&mut app, 5);
    assert_eq!(count::<With<SceneEntity>>(&mut app), 0);
    assert_eq!(count::<With<PaneCamera>>(&mut app), 0);
    assert_eq!(count::<With<SigmoidSprite>>(&mut app), 1);
    assert!(!app.world().contains_resource::<Experiment>());
    assert!(app.world().contains_resource::<SigmoidRefresh>());
}

#[test]
fn sigmoid_round_trip_removes_its_entities_and_resources() {
    let mut app = lobby_app();
    enter(&mut app, "sigmoid");
    frames(&mut app, 3);
    assert_eq!(count::<With<ExperienceEntity>>(&mut app), 2);
    leave_now(&mut app);
    assert_clean(&mut app);
}

#[test]
fn switching_directly_between_experiences() {
    let mut app = lobby_app();
    enter(&mut app, "loss-curves");
    assert_eq!(app.world().resource::<ChartSettings>().overlay.len(), 7);
    // Enter another without going through the lobby first.
    enter(&mut app, "regression-mse-vs-mae");
    frames(&mut app, 5);
    assert_eq!(count::<With<PaneCamera>>(&mut app), 3);
    assert!(
        app.world().resource::<ChartSettings>().overlay.is_empty(),
        "overlay reset"
    );
}

#[test]
fn starting_with_an_id_skips_the_lobby() {
    let mut app = headless(Some(LobbyPlugin::starting_with("loss-curves")));
    run_until(&mut app, "loss-curves", |w| {
        *w.resource::<State<AppScreen>>().get() == AppScreen::Running
    });
    assert_eq!(
        app.world().resource::<ActiveExperience>().0,
        Some("loss-curves")
    );
    leave_now(&mut app);
    assert_eq!(screen(&app), AppScreen::Lobby);
}

#[test]
fn load_failure_returns_to_the_lobby_with_the_error() {
    fn broken() -> ExperimentSpec {
        ExperimentSpec::new(
            DatasetChoice::SeparableBlobs,
            bevaru_core::TrainerConfig::logistic(),
        )
        .with_view(View::Features(vec![0, 9]))
    }
    let mut app = lobby_app();
    app.register_experience(Experience {
        id: "broken",
        title: "Broken",
        summary: "Fails to load.",
        category: "Tests",
        thumbnail: None,
        requires: Requirement::None,
        note: None,
        kind: ExperienceKind::Experiment {
            spec: broken,
            on_loaded: vec![],
        },
    });
    app.world_mut().write_message(EnterExperience("broken"));
    run_until(&mut app, "failure", |w| {
        w.resource::<LobbyErrors>().0.contains_key("broken")
    });
    frames(&mut app, 5);
    assert_clean(&mut app);
    assert!(app.world().resource::<LobbyErrors>().0["broken"].contains("feature"));
}

#[test]
fn unavailable_experiences_cannot_start() {
    let mut app = lobby_app();
    app.register_experience(Experience {
        id: "needs-gpu-feature",
        title: "Gated",
        summary: "Needs a feature this build lacks.",
        category: "Tests",
        thumbnail: None,
        requires: Requirement::Feature {
            name: "imaginary",
            enabled: false,
        },
        note: None,
        kind: ExperienceKind::Custom,
    });
    app.world_mut()
        .write_message(EnterExperience("needs-gpu-feature"));
    frames(&mut app, 3);
    assert_eq!(screen(&app), AppScreen::Lobby);
    assert!(
        app.world().resource::<LobbyErrors>().0["needs-gpu-feature"]
            .contains("--features imaginary")
    );
}

#[test]
fn quitting_stops_the_running_experience() {
    let mut app = lobby_app();
    enter(&mut app, "sigmoid");
    app.world_mut().write_message(AppExit::Success);
    app.update();
    assert_eq!(app.world().resource::<ActiveExperience>().0, None);
    assert!(!app.world().contains_resource::<SigmoidRefresh>());
}

#[test]
fn repeated_round_trips_do_not_grow_the_app() {
    let mut app = lobby_app();
    frames(&mut app, 5);
    let baseline = counts(&mut app);
    let ids: Vec<&'static str> = app
        .world()
        .resource::<ExperienceRegistry>()
        .iter()
        // MNIST needs the network; the rest exercise every code path.
        .filter(|e| e.requires == Requirement::None)
        .map(|e| e.id)
        .collect();
    assert!(ids.len() >= 4);
    for _ in 0..10 {
        for &id in &ids {
            enter(&mut app, id);
            // Let the scene build and a chart render start.
            frames(&mut app, 4);
            leave_now(&mut app);
        }
    }
    assert_clean(&mut app);
    assert_eq!(counts(&mut app), baseline);
}

#[test]
fn parse_args_handles_every_form() {
    let mut app = App::new();
    app.add_plugins(ExperiencesPlugin);
    let registry = app.world().resource::<ExperienceRegistry>();
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(parse_args(&[], registry), Ok(Launch::Lobby));
    assert_eq!(parse_args(&args(&["--list"]), registry), Ok(Launch::List));
    assert_eq!(
        parse_args(&args(&["iris-svm"]), registry),
        Ok(Launch::Experience("iris-svm"))
    );
    let err = parse_args(&args(&["nonsense"]), registry).unwrap_err();
    assert!(err.contains("unknown experience") && err.contains("loss-curves"));
    assert!(
        parse_args(&args(&["a", "b"]), registry)
            .unwrap_err()
            .contains("usage")
    );
    assert!(list(registry).contains("sigmoid"));
}

#[test]
fn escape_returns_to_the_lobby() {
    let mut app = lobby_app();
    app.init_resource::<ButtonInput<KeyCode>>().add_systems(
        Update,
        super::ui::escape_to_lobby.run_if(not(in_state(AppScreen::Lobby))),
    );
    enter(&mut app, "sigmoid");
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    frames(&mut app, 3);
    assert_clean(&mut app);
}
