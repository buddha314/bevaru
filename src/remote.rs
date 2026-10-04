//! Drive a running bevaru app from outside it: Bevy Remote Protocol (BRP)
//! methods under `bevaru.*`, behind the opt-in `remote` feature.
//!
//! Each method validates its input with the agent wire format
//! ([`crate::agent::api`]) and then sends the same message the UI would
//! (`EnterExperience`, `PlaybackCommand`, …), so remote control gets the same
//! clean-up and validation as clicking. The HTTP server binds to 127.0.0.1
//! only; the `bevaru` binary enables it with `--remote`.
//!
//! BRP's built-in `world.*` methods (entity and component inspection) are
//! available alongside these.

use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy::remote::{BrpError, BrpResult, RemotePlugin, error_codes};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::agent::api::{EnterRequest, NoArguments, PlaybackRequest, SweepRequest, TrainerRequest};
use crate::experiences::{ActiveExperience, ExperienceRegistry};
use crate::experiment::{Experiment, ExperimentLoader, fmt_num};
use crate::lobby::{AppScreen, EnterExperience, LeaveExperience, LobbyErrors};
use crate::playback::{PaneViews, Playback, PlaybackCommand, Sweep, SweepCommand, SweepSpec};

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;

/// Bevy's standard BRP port.
pub const DEFAULT_PORT: u16 = bevy::remote::http::DEFAULT_PORT;

pub use crate::agent::REMOTE_METHODS as METHODS;

/// Remote control for a bevaru app with [`crate::lobby::LobbyPlugin`].
pub struct RemoteHooksPlugin {
    /// Serve HTTP on 127.0.0.1 at this port; `None` registers the methods
    /// for in-process use only (tests).
    pub port: Option<u16>,
}

impl Default for RemoteHooksPlugin {
    fn default() -> Self {
        Self {
            port: Some(DEFAULT_PORT),
        }
    }
}

impl Plugin for RemoteHooksPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            RemotePlugin::default()
                .with_method_main(METHODS[0].0, list)
                .with_method_main(METHODS[1].0, enter)
                .with_method_main(METHODS[2].0, leave)
                .with_method_main(METHODS[3].0, playback)
                .with_method_main(METHODS[4].0, start_sweep)
                .with_method_main(METHODS[5].0, stop_sweep)
                .with_method_main(METHODS[6].0, state),
        );
        if let Some(port) = self.port {
            // RemoteHttpPlugin binds to 127.0.0.1 unless told otherwise.
            app.add_plugins(RemoteHttpPlugin::default().with_port(port));
        }
    }
}

fn invalid(message: impl Into<String>) -> BrpError {
    BrpError {
        code: error_codes::INVALID_PARAMS,
        message: message.into(),
        data: None,
    }
}

fn parse<T: DeserializeOwned>(params: Option<Value>) -> Result<T, BrpError> {
    serde_json::from_value(params.unwrap_or(Value::Object(Default::default())))
        .map_err(|e| invalid(format!("invalid params: {e}")))
}

fn no_experiment() -> BrpError {
    invalid("no experiment is running; enter one with bevaru.experiences.enter")
}

fn list(In(params): In<Option<Value>>, registry: Res<ExperienceRegistry>) -> BrpResult {
    let _: NoArguments = parse(params)?;
    Ok(json!({ "experiences": crate::agent::experience_infos(&registry) }))
}

fn enter(
    In(params): In<Option<Value>>,
    registry: Res<ExperienceRegistry>,
    mut enter: MessageWriter<EnterExperience>,
) -> BrpResult {
    let req: EnterRequest = parse(params)?;
    let Some(experience) = registry.get(&req.id) else {
        let ids: Vec<&str> = registry.iter().map(|e| e.id).collect();
        return Err(BrpError {
            code: error_codes::INVALID_PARAMS,
            message: format!(
                "unknown experience {:?}; valid ids: {}",
                req.id,
                ids.join(", ")
            ),
            data: Some(json!({ "valid_ids": ids })),
        });
    };
    if let Some(reason) = experience.requires.unmet_reason() {
        return Err(invalid(format!(
            "{} is unavailable in this build. {reason}",
            experience.id
        )));
    }
    enter.write(EnterExperience(experience.id));
    Ok(json!({ "entering": experience.id }))
}

fn leave(In(params): In<Option<Value>>, mut leave: MessageWriter<LeaveExperience>) -> BrpResult {
    let _: NoArguments = parse(params)?;
    leave.write(LeaveExperience);
    Ok(json!({ "leaving": true }))
}

fn playback(
    In(params): In<Option<Value>>,
    experiment: Option<Res<Experiment>>,
    mut commands: MessageWriter<PlaybackCommand>,
) -> BrpResult {
    let req: PlaybackRequest = parse(params)?;
    if experiment.is_none() {
        return Err(no_experiment());
    }
    commands.write(PlaybackCommand::from(&req));
    Ok(json!({ "sent": req }))
}

fn start_sweep(
    In(params): In<Option<Value>>,
    experiment: Option<Res<Experiment>>,
    mut commands: MessageWriter<SweepCommand>,
) -> BrpResult {
    let req: SweepRequest = parse(params)?;
    let spec = SweepSpec::try_from(&req).map_err(|e| invalid(e.to_string()))?;
    let Some(experiment) = experiment else {
        return Err(no_experiment());
    };
    if !experiment
        .panes
        .iter()
        .any(|p| spec.param.applies_to(p.trainer.config()))
    {
        return Err(invalid(format!(
            "{} applies to none of the running panes' losses",
            spec.param.id()
        )));
    }
    commands.write(SweepCommand::Start(spec));
    Ok(json!({ "started": req }))
}

fn stop_sweep(
    In(params): In<Option<Value>>,
    mut commands: MessageWriter<SweepCommand>,
) -> BrpResult {
    let _: NoArguments = parse(params)?;
    commands.write(SweepCommand::Stop);
    Ok(json!({ "stopped": true }))
}

fn status_id(s: Option<bevaru_core::Status>) -> &'static str {
    match s {
        None | Some(bevaru_core::Status::Running) => "running",
        Some(bevaru_core::Status::Converged) => "converged",
        Some(bevaru_core::Status::BudgetExhausted) => "budget-exhausted",
        Some(bevaru_core::Status::Diverged) => "diverged",
    }
}

#[allow(clippy::too_many_arguments)]
fn state(
    In(params): In<Option<Value>>,
    screen: Option<Res<State<AppScreen>>>,
    active: Option<Res<ActiveExperience>>,
    experiment: Option<Res<Experiment>>,
    views: Res<PaneViews>,
    playback: Res<Playback>,
    sweep: Res<Sweep>,
    loader: Res<ExperimentLoader>,
    errors: Option<Res<LobbyErrors>>,
) -> BrpResult {
    let _: NoArguments = parse(params)?;
    let screen = screen.map(|s| match s.get() {
        AppScreen::Lobby => "lobby",
        AppScreen::Loading => "loading",
        AppScreen::Running => "running",
    });
    let experiment = experiment.map(|e| {
        let panes: Vec<Value> = e
            .panes
            .iter()
            .enumerate()
            .map(|(i, pane)| {
                let view = views.0.get(i);
                json!({
                    "label": e.pane_label(i),
                    "trainer": TrainerRequest::from_config(pane.trainer.config()),
                    "step": view.map_or(0, |v| v.step),
                    "objective": view.map(|v| v.loss),
                    "status": status_id(view.and_then(|v| v.status)),
                    "steps_trained": pane.trainer.steps_taken(),
                })
            })
            .collect();
        json!({
            "name": e.name,
            "axes": e.axis_names,
            "samples": e.points.len(),
            "panes": panes,
        })
    });
    let (done, total) = sweep.progress();
    Ok(json!({
        "screen": screen,
        "active_experience": active.and_then(|a| a.0),
        "loading": loader.loading,
        "load_error": loader.error,
        "lobby_errors": errors.map(|e| e.0.clone()),
        "experiment": experiment,
        "playback": {
            "playing": playback.playing,
            "step": playback.cursor,
            "steps_per_second": playback.steps_per_second,
        },
        "sweep": {
            "active": sweep.active,
            "parameter": sweep.active.then(|| sweep.spec.param.id()),
            "current_value": sweep.current_value().filter(|_| sweep.active).map(fmt_num),
            "solutions_done": done,
            "solutions_total": total,
        },
    }))
}
