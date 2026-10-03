//! The windowed bevaru app shared by the `bevaru` binary and the examples.

use bevy::prelude::*;

use crate::BevaruPlugin;
use crate::capture::CapturePlugin;
use crate::experiences::{ActiveExperience, ExperienceRegistry};
use crate::lobby::LobbyPlugin;

/// A 1600 × 900 window with every bevaru plugin and the lobby. With `start`,
/// it opens that experience directly; "Back to lobby" still works.
pub fn windowed_app(start: Option<&'static str>) -> App {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "bevaru".into(),
            resolution: (1600, 900).into(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins((BevaruPlugin, LobbyPlugin { start }))
    .add_systems(Update, window_title);
    if let Some(capture) = CapturePlugin::from_env() {
        app.add_plugins(capture);
    }
    app
}

/// "bevaru — <experience>" while one is running.
fn window_title(
    active: Res<ActiveExperience>,
    registry: Res<ExperienceRegistry>,
    mut window: Single<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    if !active.is_changed() {
        return;
    }
    window.title = match active.0.and_then(|id| registry.get(id)) {
        Some(e) => format!("bevaru — {}", e.title),
        None => "bevaru".into(),
    };
}
