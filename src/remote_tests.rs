//! Headless tests of the `bevaru.*` remote methods, sent through BRP's
//! in-process mailbox: the same dispatch HTTP requests go through.

use bevy::prelude::*;
use bevy::remote::{BrpMessage, BrpResult, BrpSender};
use serde_json::{Value, json};

use super::RemoteHooksPlugin;
use crate::experiment::Experiment;
use crate::lobby::{AppScreen, LobbyPlugin};
use crate::playback::Sweep;

fn app() -> App {
    let mut app = crate::lobby::tests::headless(Some(LobbyPlugin::default()));
    app.add_plugins(RemoteHooksPlugin { port: None });
    app.update(); // the mailbox is created at startup
    app
}

fn call(app: &mut App, method: &str, params: Value) -> BrpResult {
    let (tx, rx) = async_channel::bounded(1);
    let mailbox = (**app.world().resource::<BrpSender>()).clone();
    mailbox
        .try_send(BrpMessage {
            method: method.into(),
            params: (!params.is_null()).then_some(params),
            sender: tx,
        })
        .expect("mailbox open");
    for _ in 0..20 {
        app.update();
        if let Ok(reply) = rx.try_recv() {
            return reply;
        }
    }
    panic!("no reply to {method}");
}

fn screen(app: &App) -> AppScreen {
    *app.world().resource::<State<AppScreen>>().get()
}

#[test]
fn enter_then_state_then_leave() {
    let mut app = app();
    let listed = call(&mut app, "bevaru.experiences.list", Value::Null).unwrap();
    assert!(
        listed["experiences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == "iris-svm")
    );

    let r = call(
        &mut app,
        "bevaru.experiences.enter",
        json!({"id": "iris-svm"}),
    )
    .unwrap();
    assert_eq!(r["entering"], "iris-svm");
    crate::lobby::tests::run_until(&mut app, "iris running", |w| {
        w.contains_resource::<Experiment>()
            && *w.resource::<State<AppScreen>>().get() == AppScreen::Running
    });

    let s = call(&mut app, "bevaru.state", Value::Null).unwrap();
    assert_eq!(s["screen"], "running");
    assert_eq!(s["active_experience"], "iris-svm");
    assert_eq!(s["experiment"]["panes"].as_array().unwrap().len(), 2);
    assert_eq!(s["experiment"]["panes"][0]["trainer"]["loss"], "hinge");
    assert_eq!(
        s["sweep"]["active"], true,
        "iris starts its C sweep on load"
    );

    call(&mut app, "bevaru.sweep.stop", Value::Null).unwrap();
    call(&mut app, "bevaru.playback", json!("step")).unwrap();
    assert!(!app.world().resource::<Sweep>().active);

    call(&mut app, "bevaru.experiences.leave", Value::Null).unwrap();
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(screen(&app), AppScreen::Lobby);
    assert!(!app.world().contains_resource::<Experiment>());
    assert_eq!(
        call(&mut app, "bevaru.state", Value::Null).unwrap()["experiment"],
        Value::Null
    );
}

#[test]
fn unknown_experience_lists_valid_ids_and_stays_put() {
    let mut app = app();
    let err = call(
        &mut app,
        "bevaru.experiences.enter",
        json!({"id": "nonsense"}),
    )
    .unwrap_err();
    assert!(err.message.contains("loss-curves"), "{}", err.message);
    assert!(err.data.unwrap()["valid_ids"].as_array().unwrap().len() >= 5);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(screen(&app), AppScreen::Lobby);
}

#[test]
fn invalid_params_and_commands_without_an_experiment_are_errors() {
    let mut app = app();
    let err = call(&mut app, "bevaru.playback", json!("fast-forward")).unwrap_err();
    assert!(err.message.contains("invalid params"), "{}", err.message);
    let err = call(&mut app, "bevaru.playback", json!("play")).unwrap_err();
    assert!(err.message.contains("no experiment is running"));
    let err = call(
        &mut app,
        "bevaru.sweep.start",
        json!({"parameter": "c", "from": 0.1, "to": 10, "samples": 99}),
    )
    .unwrap_err();
    assert!(err.message.contains("samples"), "{}", err.message);
}

#[test]
fn no_mailbox_without_the_plugin() {
    let mut app = crate::lobby::tests::headless(Some(LobbyPlugin::default()));
    app.update();
    assert!(!app.world().contains_resource::<BrpSender>());
}
