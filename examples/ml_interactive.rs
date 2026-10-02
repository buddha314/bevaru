//! The original scaffold, now on screen: ruviz renders a sigmoid plot to PNG
//! bytes ([`PlotPngBytes`]), refreshed every second via [`RefreshPlotEvent`],
//! and shown as a sprite.
//!
//! ```sh
//! cargo run --example ml_interactive
//! ```

mod shared;

use bevaru::{PlotPngBytes, RefreshPlotEvent, SigmoidPlotPlugin};
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(shared::window("bevaru — sigmoid"))
        .add_plugins((SigmoidPlotPlugin, shared::DevScreenshot))
        .insert_resource(ClearColor(Color::WHITE))
        .add_systems(Startup, setup)
        .add_systems(Update, (auto_refresh, show_plot))
        .run();
}

#[derive(Resource)]
struct RefreshTimer(Timer);

#[derive(Component)]
struct PlotSprite;

fn setup(mut commands: Commands) {
    info!("bevaru example started; the plot refreshes every second.");
    commands.insert_resource(RefreshTimer(Timer::from_seconds(1.0, TimerMode::Repeating)));
    commands.spawn(Camera2d);
    commands.spawn((PlotSprite, Sprite::default()));
}

fn auto_refresh(
    time: Res<Time>,
    mut timer: ResMut<RefreshTimer>,
    mut events: MessageWriter<RefreshPlotEvent>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        events.write_default();
    }
}

/// Decode the PNG into a texture whenever the plot is re-rendered.
fn show_plot(
    plot: Res<PlotPngBytes>,
    mut images: ResMut<Assets<Image>>,
    mut sprite: Single<&mut Sprite, With<PlotSprite>>,
) {
    if !plot.is_changed() || plot.is_empty() {
        return;
    }
    match Image::from_buffer(
        &plot,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::default(),
        RenderAssetUsages::default(),
    ) {
        Ok(image) => {
            info!("Rendered plot: {} bytes", plot.len());
            sprite.image = images.add(image);
        }
        Err(e) => error!("could not decode plot: {e}"),
    }
}
