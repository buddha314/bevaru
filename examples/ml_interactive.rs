use bevaru::{BevaruPlugin, PlotPngBytes, RefreshPlotEvent};
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(BevaruPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, auto_refresh)
        .add_systems(Update, show_plot_size)
        .run();
}

#[derive(Resource)]
struct RefreshTimer(Timer);

fn setup(mut commands: Commands) {
    info!("bevaru example started; plot bytes will refresh every second.");
    commands.insert_resource(RefreshTimer(Timer::from_seconds(1.0, TimerMode::Repeating)));
}

fn auto_refresh(
    time: Res<Time>,
    mut timer: ResMut<RefreshTimer>,
    mut events: EventWriter<RefreshPlotEvent>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        events.send_default();
    }
}

fn show_plot_size(plot: Res<PlotPngBytes>) {
    if plot.is_changed() {
        info!("Rendered plot bytes: {}", plot.len());
    }
}
