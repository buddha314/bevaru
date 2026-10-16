//! The lobby: pick an experience, come back, pick another.
//!
//! [`LobbyPlugin`] starts the app idle on [`AppScreen::Lobby`]. Sending
//! [`EnterExperience`] starts a registered experience; [`LeaveExperience`]
//! (the "Back to lobby" control, or `Esc`) returns. Every way out — Back,
//! `Esc`, a failed load, switching experiences, quitting — goes through one
//! path that stops the experience, unloads its experiment, and despawns its
//! entities, so the next experience starts clean.

use std::collections::HashMap;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::charts::ChartSettings;
use crate::experiences::{
    ActiveExperience, ExperienceKind, ExperienceRegistry, ExperienceStarted, ExperienceStopped,
    ExperiencesPlugin, OnLoaded,
};
use crate::experiment::{
    ExperimentLoadFailed, ExperimentLoaded, LoadExperiment, StartupMode, UnloadExperiment,
};
use crate::playback::{PlaybackCommand, SweepCommand};

/// Which screen the app is on.
#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AppScreen {
    #[default]
    Lobby,
    /// An experiment experience is loading.
    Loading,
    Running,
}

/// Start the experience with this id (leaving the current one first).
#[derive(Message, Debug, Clone, Copy)]
pub struct EnterExperience(pub &'static str);

/// Leave the running experience and return to the lobby.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct LeaveExperience;

/// The last error per experience, shown on its card.
#[derive(Resource, Debug, Default)]
pub struct LobbyErrors(pub HashMap<&'static str, String>);

/// A load the lobby requested and is waiting for, with what to do when it
/// arrives. Tracked explicitly rather than inferred from [`AppScreen`]: a
/// fast load can finish before the requested `Loading` transition applies.
#[derive(Resource, Debug, Default)]
struct PendingActions(Option<Vec<OnLoaded>>);

#[derive(Default)]
pub struct LobbyPlugin {
    /// Skip the lobby and start this experience (for deep links and examples).
    pub start: Option<&'static str>,
}

impl LobbyPlugin {
    pub fn starting_with(id: &'static str) -> Self {
        Self { start: Some(id) }
    }
}

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<ExperiencesPlugin>() {
            app.add_plugins(ExperiencesPlugin);
        }
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.insert_resource(StartupMode::Idle)
            .init_state::<AppScreen>()
            .init_resource::<LobbyErrors>()
            .init_resource::<PendingActions>()
            .add_message::<EnterExperience>()
            .add_message::<LeaveExperience>()
            .add_message::<LoadExperiment>()
            .add_message::<UnloadExperiment>()
            .add_message::<PlaybackCommand>()
            .add_message::<SweepCommand>()
            .add_observer(experiment_ready)
            .add_observer(experiment_failed)
            .add_systems(Update, (handle_leave, handle_enter).chain())
            .add_systems(Last, leave_on_exit);
        if let Some(id) = self.start {
            app.add_systems(Startup, move |mut enter: MessageWriter<EnterExperience>| {
                enter.write(EnterExperience(id));
            });
        }
        if app.is_plugin_added::<bevy::render::RenderPlugin>() {
            app.add_plugins(ui::LobbyUiPlugin);
        }
    }
}

/// The single exit path: stop the experience, unload its experiment (which
/// also cancels a load in progress), and show the lobby. `ExperienceStopped`
/// despawns every `ExperienceEntity`.
fn leave(
    commands: &mut Commands,
    active: &mut ActiveExperience,
    next: &mut NextState<AppScreen>,
    unload: &mut MessageWriter<UnloadExperiment>,
    pending: &mut PendingActions,
) {
    if let Some(id) = active.0.take() {
        commands.trigger(ExperienceStopped { id });
    }
    unload.write(UnloadExperiment);
    pending.0 = None;
    next.set(AppScreen::Lobby);
}

fn handle_leave(
    mut requests: MessageReader<LeaveExperience>,
    mut commands: Commands,
    mut active: ResMut<ActiveExperience>,
    mut next: ResMut<NextState<AppScreen>>,
    mut unload: MessageWriter<UnloadExperiment>,
    mut pending: ResMut<PendingActions>,
) {
    if requests.read().count() > 0 {
        leave(
            &mut commands,
            &mut active,
            &mut next,
            &mut unload,
            &mut pending,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_enter(
    mut requests: MessageReader<EnterExperience>,
    mut commands: Commands,
    registry: Res<ExperienceRegistry>,
    mut active: ResMut<ActiveExperience>,
    mut next: ResMut<NextState<AppScreen>>,
    mut unload: MessageWriter<UnloadExperiment>,
    mut load: MessageWriter<LoadExperiment>,
    mut pending: ResMut<PendingActions>,
    mut errors: ResMut<LobbyErrors>,
) {
    for &EnterExperience(id) in requests.read() {
        let Some(experience) = registry.get(id) else {
            warn!("bevaru: no experience with id {id:?}");
            continue;
        };
        if let Some(reason) = experience.requires.unmet_reason() {
            errors.0.insert(experience.id, reason);
            continue;
        }
        // Switching: leave the current experience through the usual path.
        // Unloading runs before the new load in the same frame.
        if active.0.is_some() {
            leave(
                &mut commands,
                &mut active,
                &mut next,
                &mut unload,
                &mut pending,
            );
        }
        errors.0.remove(experience.id);
        active.0 = Some(experience.id);
        commands.trigger(ExperienceStarted { id: experience.id });
        match &experience.kind {
            ExperienceKind::Experiment { spec, on_loaded } => {
                load.write(LoadExperiment(spec()));
                pending.0 = Some(on_loaded.clone());
                next.set(AppScreen::Loading);
            }
            ExperienceKind::Custom => next.set(AppScreen::Running),
        }
    }
}

/// The experiment we were loading arrived: show it and run its actions.
fn experiment_ready(
    _loaded: On<ExperimentLoaded>,
    mut next: ResMut<NextState<AppScreen>>,
    mut pending: ResMut<PendingActions>,
    mut playback: MessageWriter<PlaybackCommand>,
    mut sweep: MessageWriter<SweepCommand>,
    charts: Option<ResMut<ChartSettings>>,
) {
    // Only loads the lobby requested; those started from inside a running
    // experience (its Data panel) need no screen change.
    let Some(actions) = pending.0.take() else {
        return;
    };
    next.set(AppScreen::Running);
    let mut charts = charts;
    for action in actions {
        match action {
            OnLoaded::Play => {
                playback.write(PlaybackCommand::Play);
            }
            OnLoaded::Sweep(spec) => {
                sweep.write(SweepCommand::Start(spec));
            }
            OnLoaded::ShowLosses(losses) => {
                if let Some(charts) = charts.as_deref_mut() {
                    charts.overlay = losses;
                }
            }
        }
    }
}

/// The experiment we were loading failed: back to the lobby, error on its card.
#[allow(clippy::too_many_arguments)]
fn experiment_failed(
    failed: On<ExperimentLoadFailed>,
    mut commands: Commands,
    mut active: ResMut<ActiveExperience>,
    mut next: ResMut<NextState<AppScreen>>,
    mut unload: MessageWriter<UnloadExperiment>,
    mut pending: ResMut<PendingActions>,
    mut errors: ResMut<LobbyErrors>,
) {
    if pending.0.is_none() {
        return;
    }
    if let Some(id) = active.0 {
        errors.0.insert(id, failed.error.clone());
    }
    leave(
        &mut commands,
        &mut active,
        &mut next,
        &mut unload,
        &mut pending,
    );
}

/// Quitting mid-experience stops it first, so its stop handlers run.
fn leave_on_exit(
    mut exits: MessageReader<AppExit>,
    mut commands: Commands,
    mut active: ResMut<ActiveExperience>,
    mut next: ResMut<NextState<AppScreen>>,
    mut unload: MessageWriter<UnloadExperiment>,
    mut pending: ResMut<PendingActions>,
) {
    if exits.read().count() > 0 && active.0.is_some() {
        leave(
            &mut commands,
            &mut active,
            &mut next,
            &mut unload,
            &mut pending,
        );
    }
}

/// How the `bevaru` binary was asked to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    Lobby,
    List,
    Experience(&'static str),
}

/// Parse the binary's arguments (excluding the program name).
pub fn parse_args(args: &[String], registry: &ExperienceRegistry) -> Result<Launch, String> {
    match args {
        [] => Ok(Launch::Lobby),
        [flag] if flag == "--list" => Ok(Launch::List),
        [id] if !id.starts_with('-') => registry
            .get(id)
            .map(|e| Launch::Experience(e.id))
            .ok_or_else(|| format!("unknown experience {id:?}\n\n{}", list(registry))),
        _ => Err(format!(
            "usage: bevaru [--list | <experience-id>]\n\n{}",
            list(registry)
        )),
    }
}

/// Every id with its title, one per line, in lobby order.
pub fn list(registry: &ExperienceRegistry) -> String {
    let width = registry.iter().map(|e| e.id.len()).max().unwrap_or(0);
    let mut out = String::from("experiences:\n");
    for e in registry.iter() {
        let unavailable = e
            .requires
            .unmet_reason()
            .map(|r| format!("  ({r})"))
            .unwrap_or_default();
        out += &format!("  {:width$}  {}{unavailable}\n", e.id, e.title);
    }
    out
}

#[cfg(test)]
#[path = "lobby_tests.rs"]
pub(crate) mod tests;

pub mod ui {
    //! The lobby's egui screens: the card grid, the loading screen, and the
    //! "Back to lobby" overlay for custom experiences.

    use bevy::asset::RenderAssetUsages;
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    use bevy::prelude::*;
    use bevy_egui::input::EguiWantsInput;
    use bevy_egui::{EguiContexts, EguiPrimaryContextPass, EguiTextureHandle, egui};

    use super::{AppScreen, EnterExperience, LeaveExperience, LobbyErrors};
    use crate::experiences::{ActiveExperience, ExperienceRegistry, LobbyCard, Thumbnail};
    use crate::experiment::{Experiment, ExperimentLoader};

    pub struct LobbyUiPlugin;

    impl Plugin for LobbyUiPlugin {
        fn build(&self, app: &mut App) {
            app.init_resource::<Thumbnails>()
                .add_systems(
                    EguiPrimaryContextPass,
                    (
                        lobby.run_if(in_state(AppScreen::Lobby)),
                        loading.run_if(in_state(AppScreen::Loading)),
                        back_overlay.run_if(
                            in_state(AppScreen::Running)
                                .and_then(not(resource_exists::<Experiment>))
                                .and_then(not(resource_exists::<crate::capture::HideOverlays>)),
                        ),
                    ),
                )
                .add_systems(
                    Update,
                    (
                        escape_to_lobby.run_if(not(in_state(AppScreen::Lobby))),
                        exit_shortcut.run_if(in_state(AppScreen::Lobby)),
                    ),
                );
        }
    }

    /// Card thumbnails as egui textures, created once and kept for the life
    /// of the app (the lobby always needs them).
    #[derive(Resource, Default)]
    struct Thumbnails(std::collections::HashMap<&'static str, Option<egui::TextureId>>);

    const CARD_WIDTH: f32 = 300.0;
    const THUMB_SIZE: egui::Vec2 = egui::vec2(CARD_WIDTH - 16.0, (CARD_WIDTH - 16.0) * 9.0 / 16.0);

    fn root(ctx: &egui::Context) -> egui::Ui {
        egui::Ui::new(
            ctx.clone(),
            "bevaru-lobby-root".into(),
            egui::UiBuilder::new()
                .layer_id(egui::LayerId::background())
                .max_rect(ctx.viewport_rect()),
        )
    }

    fn lobby(
        mut contexts: EguiContexts,
        registry: Res<ExperienceRegistry>,
        errors: Res<LobbyErrors>,
        mut thumbnails: ResMut<Thumbnails>,
        mut images: ResMut<Assets<Image>>,
        assets: Res<AssetServer>,
        mut enter: MessageWriter<EnterExperience>,
        mut exit: MessageWriter<AppExit>,
        entries: Query<&crate::authoring::LobbyEntry>,
        mut warned: Local<std::collections::HashSet<String>>,
    ) -> Result {
        let entries: Vec<_> = entries.iter().collect();
        let layout = registry.lobby_layout(&entries);
        for id in &layout.unknown {
            if warned.insert(id.clone()) {
                warn!("bevaru: a LobbyEntry names unknown experience {id:?}; ignoring it");
            }
        }
        for e in registry.iter() {
            if !thumbnails.0.contains_key(e.id) {
                let handle = match e.thumbnail {
                    Some(Thumbnail::Embedded(bytes)) => Image::from_buffer(
                        bytes,
                        ImageType::Extension("png"),
                        CompressedImageFormats::NONE,
                        true,
                        ImageSampler::linear(),
                        RenderAssetUsages::default(),
                    )
                    .map_err(|err| warn!("bevaru: bad thumbnail for {}: {err}", e.id))
                    .ok()
                    .map(|img| images.add(img)),
                    Some(Thumbnail::Asset(path)) => Some(assets.load(path)),
                    None => None,
                };
                let id = handle.map(|h| contexts.add_image(EguiTextureHandle::Strong(h)));
                thumbnails.0.insert(e.id, id);
            }
        }
        let ctx = contexts.ctx_mut()?.clone();
        let mut root = root(&ctx);
        egui::CentralPanel::default().show(&mut root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("bevaru").size(30.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(egui::RichText::new("Exit").size(16.0))
                            .on_hover_text("Quit bevaru (Ctrl+Q)")
                            .clicked()
                        {
                            exit.write(AppExit::Success);
                        }
                    });
                });
                ui.label("Interactive machine-learning visualizations. Pick an experience; press Esc to come back, or Ctrl+Q to quit.");
                ui.add_space(18.0);
                // Categories flow side by side, wrapping whole categories to
                // the next row when the window is too narrow for them.
                const CARD_GAP: f32 = 14.0;
                const CATEGORY_GAP: f32 = 28.0;
                let width = ui.available_width();
                let mut rows: Vec<Vec<(&str, &Vec<LobbyCard>)>> = vec![Vec::new()];
                let mut used = 0.0;
                for (category, experiences) in &layout.categories {
                    let w = experiences.len() as f32 * (CARD_WIDTH + CARD_GAP) + CATEGORY_GAP;
                    let row = rows.last_mut().expect("rows starts non-empty");
                    if !row.is_empty() && used + w > width {
                        rows.push(Vec::new());
                        used = 0.0;
                    }
                    used += w;
                    rows.last_mut()
                        .expect("just pushed")
                        .push((category.as_str(), experiences));
                }
                for row in rows {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = CATEGORY_GAP;
                        for (category, experiences) in row {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(category).size(20.0).strong());
                                ui.add_space(4.0);
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = CARD_GAP;
                                    for c in experiences {
                                        let e = c.experience;
                                        let thumb = thumbnails.0.get(e.id).copied().flatten();
                                        if card(ui, c, thumb, errors.0.get(e.id)) {
                                            enter.write(EnterExperience(e.id));
                                        }
                                    }
                                });
                            });
                        }
                    });
                    ui.add_space(22.0);
                }
            });
        });
        Ok(())
    }

    /// One experience card; returns true when activated.
    fn card(
        ui: &mut egui::Ui,
        card: &LobbyCard,
        thumbnail: Option<egui::TextureId>,
        error: Option<&String>,
    ) -> bool {
        let e = card.experience;
        let available = e.is_available();
        let frame = egui::Frame::group(ui.style())
            .inner_margin(8.0)
            .corner_radius(6.0)
            .fill(ui.visuals().extreme_bg_color);
        let response = ui
            .add_enabled_ui(available, |ui| {
                frame
                    .show(ui, |ui| {
                        ui.set_width(CARD_WIDTH - 16.0);
                        ui.vertical(|ui| {
                            match thumbnail {
                                Some(id) => {
                                    ui.image(egui::load::SizedTexture::new(id, THUMB_SIZE));
                                }
                                None => placeholder(ui, &card.title),
                            }
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(&card.title).strong().size(16.0));
                            ui.label(&card.summary);
                            if let Some(note) = e.note {
                                ui.small(note);
                            }
                            if let Some(reason) = e.requires.unmet_reason() {
                                ui.small(egui::RichText::new(reason).italics());
                            }
                            if let Some(err) = error {
                                ui.colored_label(egui::Color32::from_rgb(220, 70, 60), err);
                            }
                        });
                    })
                    .response
            })
            .inner;
        let response = ui.interact(
            response.rect,
            egui::Id::new(("experience-card", e.id)),
            if available {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        );
        if available && response.hovered() {
            ui.painter().rect_stroke(
                response.rect,
                6.0,
                egui::Stroke::new(2.0, ui.visuals().selection.bg_fill),
                egui::StrokeKind::Inside,
            );
        }
        available
            && response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
    }

    /// Stand-in for a missing thumbnail: the title on a tinted panel.
    fn placeholder(ui: &mut egui::Ui, title: &str) {
        let (rect, _) = ui.allocate_exact_size(THUMB_SIZE, egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 4.0, ui.visuals().faint_bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            title,
            egui::FontId::proportional(18.0),
            ui.visuals().weak_text_color(),
        );
    }

    fn loading(
        mut contexts: EguiContexts,
        registry: Res<ExperienceRegistry>,
        active: Res<ActiveExperience>,
        loader: Res<ExperimentLoader>,
        mut leave: MessageWriter<LeaveExperience>,
    ) -> Result {
        let ctx = contexts.ctx_mut()?.clone();
        let title = active
            .0
            .and_then(|id| registry.get(id))
            .map_or("", |e| e.title);
        let mut root = root(&ctx);
        egui::CentralPanel::default().show(&mut root, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.35);
                ui.spinner();
                ui.heading(format!("Loading {title}…"));
                if let Some(label) = &loader.loading {
                    ui.small(label);
                }
                ui.add_space(12.0);
                if ui.button("◀ Back to lobby").clicked() {
                    leave.write(LeaveExperience);
                }
            });
        });
        Ok(())
    }

    /// Custom experiences have no control panel, so they get this button.
    fn back_overlay(
        mut contexts: EguiContexts,
        mut leave: MessageWriter<LeaveExperience>,
    ) -> Result {
        let ctx = contexts.ctx_mut()?;
        egui::Area::new(egui::Id::new("bevaru-back"))
            .fixed_pos(egui::pos2(12.0, 12.0))
            .show(ctx, |ui| {
                if ui.button("◀ Lobby").on_hover_text("Esc").clicked() {
                    leave.write(LeaveExperience);
                }
            });
        Ok(())
    }

    /// Ctrl+Q quits from the lobby (Esc never quits: it means "back").
    pub(crate) fn exit_shortcut(
        keys: Res<ButtonInput<KeyCode>>,
        egui: Option<Res<EguiWantsInput>>,
        mut exit: MessageWriter<AppExit>,
    ) {
        if egui.is_some_and(|e| e.wants_any_keyboard_input()) {
            return;
        }
        let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
        if ctrl && keys.just_pressed(KeyCode::KeyQ) {
            exit.write(AppExit::Success);
        }
    }

    pub(crate) fn escape_to_lobby(
        keys: Res<ButtonInput<KeyCode>>,
        egui: Option<Res<EguiWantsInput>>,
        mut leave: MessageWriter<LeaveExperience>,
    ) {
        // Esc in a text field belongs to the field.
        if egui.is_some_and(|e| e.wants_any_keyboard_input()) {
            return;
        }
        if keys.just_pressed(KeyCode::Escape) {
            leave.write(LeaveExperience);
        }
    }
}
