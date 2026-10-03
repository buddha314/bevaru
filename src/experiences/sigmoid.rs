//! The ruviz sigmoid plot (the original scaffold) as a custom experience:
//! [`SigmoidPlotPlugin`] keeps [`PlotPngBytes`] current, and this shows it
//! as a sprite, refreshed every second.

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

use super::{ExperienceEntity, ExperienceStarted, ExperienceStopped, in_experience};
use crate::{PlotPngBytes, RefreshPlotEvent, SigmoidPlotPlugin};

pub const ID: &str = "sigmoid";

pub struct SigmoidExperiencePlugin;

impl Plugin for SigmoidExperiencePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<SigmoidPlotPlugin>() {
            app.add_plugins(SigmoidPlotPlugin);
        }
        app.add_observer(start)
            .add_observer(stop)
            .add_systems(Update, (refresh, show).chain().run_if(in_experience(ID)));
    }
}

#[derive(Resource)]
pub(crate) struct SigmoidRefresh(Timer);

#[derive(Component)]
pub(crate) struct SigmoidSprite;

fn start(
    started: On<ExperienceStarted>,
    mut commands: Commands,
    mut refresh: MessageWriter<RefreshPlotEvent>,
) {
    if started.id != ID {
        return;
    }
    commands.insert_resource(SigmoidRefresh(Timer::from_seconds(
        1.0,
        TimerMode::Repeating,
    )));
    commands.spawn((
        ExperienceEntity,
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::WHITE),
            ..default()
        },
    ));
    commands.spawn((ExperienceEntity, SigmoidSprite, Sprite::default()));
    refresh.write_default();
}

fn stop(stopped: On<ExperienceStopped>, mut commands: Commands) {
    if stopped.id == ID {
        commands.remove_resource::<SigmoidRefresh>();
    }
}

fn refresh(
    time: Res<Time>,
    timer: Option<ResMut<SigmoidRefresh>>,
    mut events: MessageWriter<RefreshPlotEvent>,
) {
    if let Some(mut timer) = timer
        && timer.0.tick(time.delta()).just_finished()
    {
        events.write_default();
    }
}

/// Decode the PNG into the sprite's texture whenever the plot is re-rendered
/// or the sprite is new. The previous texture is freed with its handle.
fn show(
    plot: Res<PlotPngBytes>,
    mut images: ResMut<Assets<Image>>,
    mut sprite: Single<&mut Sprite, With<SigmoidSprite>>,
) {
    let fresh = sprite.image == Handle::default();
    if plot.is_empty() || !(plot.is_changed() || fresh) {
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
        Ok(image) => sprite.image = images.add(image),
        Err(e) => error!("bevaru: could not decode the sigmoid plot: {e}"),
    }
}
