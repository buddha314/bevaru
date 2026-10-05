//! bevaru as a Jackdaw project: edit the lobby in `assets/lobby.bsn`, and
//! reuse bevaru's visual assets ([`PerceptronDiagram`], [`LossShapeSurface`])
//! in scenes. See `docs/jackdaw.md` in the bevaru repository.
//!
//! - By default the game runs bevaru's lobby, with the [`LobbyEntry`]
//!   overrides that `assets/lobby.bsn` holds (order, titles, categories,
//!   hidden cards).
//! - With `BEVARU_SCENE=<path>` (the example Play runs in `jackdaw.toml`) it
//!   shows that authored scene instead, building bevaru's components in it.
//!
//! The editor's viewport shows authored scenes without running game code,
//! so bevaru's components appear as geometry under Play (or `cargo run`).
//!
//! [`PerceptronDiagram`]: bevaru::authoring::PerceptronDiagram
//! [`LossShapeSurface`]: bevaru::authoring::LossShapeSurface
//! [`LobbyEntry`]: bevaru::authoring::LobbyEntry

pub mod scenes;

use bevaru::authoring::AuthoringPlugin;
use bevaru::lobby::LobbyPlugin;
use bevy::prelude::*;
use jackdaw_runtime::prelude::*;

/// The scene the lobby's overrides come from.
pub const LOBBY_SCENE: &str = "lobby.bsn";

/// bevaru's app for `cargo run` and the editor's Play button.
#[derive(Default)]
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AuthoringPlugin);
        match std::env::var("BEVARU_SCENE") {
            // An example scene: bevaru's components in an authored scene.
            Ok(scene) => {
                // bevaru's diagrams are drawn for a white page (dark labels).
                app.insert_resource(ClearColor(Color::WHITE));
                if !app.is_plugin_added::<bevy_egui::EguiPlugin>() {
                    // For the perceptron's labels.
                    app.add_plugins(bevy_egui::EguiPlugin::default());
                }
                app.add_systems(
                    Startup,
                    move |mut commands: Commands, assets: Res<AssetServer>| {
                        commands.spawn(JackdawSceneRoot(assets.load(scene.clone())));
                    },
                );
            }
            // The lobby, with the overrides in lobby.bsn.
            Err(_) => {
                app.add_plugins((bevaru::BevaruPlugin, LobbyPlugin::default()))
                    .add_systems(
                        Startup,
                        |mut commands: Commands, assets: Res<AssetServer>| {
                            commands.spawn(JackdawSceneRoot(assets.load(LOBBY_SCENE)));
                        },
                    );
            }
        }
    }
}
