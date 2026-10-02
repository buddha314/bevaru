//! Helpers shared by the examples.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

/// `DefaultPlugins` with a titled window.
pub fn window(title: &str) -> impl PluginGroup {
    DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: title.into(),
            resolution: (1600, 900).into(),
            ..default()
        }),
        ..default()
    })
}

/// For docs and CI: with `BEVARU_SCREENSHOT=out.png`, save a screenshot after
/// `BEVARU_SCREENSHOT_AFTER` seconds (default 5) and exit.
pub struct DevScreenshot;

impl Plugin for DevScreenshot {
    fn build(&self, app: &mut App) {
        if let Ok(path) = std::env::var("BEVARU_SCREENSHOT") {
            let after = std::env::var("BEVARU_SCREENSHOT_AFTER")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5.0);
            app.add_systems(
                Update,
                move |mut commands: Commands,
                      time: Res<Time<Real>>,
                      mut state: Local<u8>,
                      mut exit: MessageWriter<AppExit>| {
                    let t = time.elapsed_secs();
                    if *state == 0 && t > after {
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk(path.clone()));
                        *state = 1;
                    } else if *state == 1 && t > after + 1.5 {
                        exit.write(AppExit::Success);
                    }
                },
            );
        }
    }
}
