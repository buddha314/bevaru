//! `cargo run` here plays bevaru with the lobby edited in Jackdaw; the
//! editor's Play button runs this same binary.

use bevy::prelude::*;

fn main() -> AppExit {
    let default_plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "bevaru".into(),
            resolution: (1600, 900).into(),
            ..default()
        }),
        ..default()
    });
    // When the editor plays the game inside its viewport it asks for a
    // windowless launch; outside the editor this returns the plugins as is.
    let default_plugins = jackdaw_runtime::maybe_windowless(default_plugins);

    let mut app = App::new();
    app.add_plugins(default_plugins)
        .add_plugins(avian3d::prelude::PhysicsPlugins::default())
        .add_plugins(jackdaw_runtime::JackdawPlugin)
        .add_plugins(bevaru_jackdaw::GamePlugin);
    // `BEVARU_SCREENSHOT=out.png` saves the window and exits, as in bevaru.
    if let Some(capture) = bevaru::capture::CapturePlugin::from_env() {
        app.add_plugins(capture);
    }
    app.run()
}
