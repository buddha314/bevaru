//! The experience registry: everything the lobby can start.
//!
//! An experience is registered once with [`RegisterExperience::register_experience`]
//! and is then reachable from the lobby, from `cargo run -- <id>`, and from the
//! `--example` binaries. It is either:
//!
//! - an **experiment** ([`ExperienceKind::Experiment`]): a dataset and model
//!   shown with the standard scene, charts, and control panel; or
//! - **custom** ([`ExperienceKind::Custom`]): the registering plugin brings its
//!   own systems. Gate them with [`in_experience`], set up and tear down in
//!   observers of [`ExperienceStarted`] and [`ExperienceStopped`], and mark
//!   every entity it spawns with [`ExperienceEntity`] so it is despawned
//!   when the experience stops.
//!
//! ```ignore
//! app.register_experience(Experience {
//!     id: "my-demo",
//!     title: "My demo",
//!     summary: "What this teaches, in one sentence.",
//!     category: "Activation functions",
//!     thumbnail: None,
//!     requires: Requirement::None,
//!     note: None,
//!     kind: ExperienceKind::Custom,
//! })
//! .add_observer(|ev: On<ExperienceStarted>, mut commands: Commands| {
//!     if ev.id == "my-demo" {
//!         commands.spawn((ExperienceEntity, Camera2d));
//!     }
//! })
//! .add_systems(Update, my_system.run_if(in_experience("my-demo")));
//! ```

mod builtin;
pub mod layout;
pub mod loss_shapes;
pub mod loss_surface;
pub mod perceptron;
pub mod sigmoid;

use std::fmt;

use bevaru_core::LossKind;
use bevy::prelude::*;

use crate::experiment::ExperimentSpec;
use crate::playback::SweepSpec;

/// An action run once an experiment experience has finished loading.
#[derive(Debug, Clone, PartialEq)]
pub enum OnLoaded {
    /// Start training playback.
    Play,
    /// Compute and animate a hyperparameter sweep.
    Sweep(SweepSpec),
    /// Overlay these losses on the loss-curve charts.
    ShowLosses(Vec<LossKind>),
}

#[derive(Clone)]
pub enum ExperienceKind {
    /// A dataset and model in the standard scene, charts, and controls.
    Experiment {
        spec: fn() -> ExperimentSpec,
        on_loaded: Vec<OnLoaded>,
    },
    /// The registering plugin provides the systems; see the module docs.
    Custom,
}

impl fmt::Debug for ExperienceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExperienceKind::Experiment { on_loaded, .. } => f
                .debug_struct("Experiment")
                .field("on_loaded", on_loaded)
                .finish_non_exhaustive(),
            ExperienceKind::Custom => f.write_str("Custom"),
        }
    }
}

/// What must be true of the build for an experience to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    None,
    /// A cargo feature, and whether this build has it.
    Feature {
        name: &'static str,
        enabled: bool,
    },
}

impl Requirement {
    pub fn is_met(self) -> bool {
        match self {
            Requirement::None => true,
            Requirement::Feature { enabled, .. } => enabled,
        }
    }

    /// Why the experience can't run, and how to enable it.
    pub fn unmet_reason(self) -> Option<String> {
        match self {
            Requirement::Feature {
                name,
                enabled: false,
            } => Some(format!(
                "Needs the `{name}` feature: rebuild with `--features {name}`."
            )),
            _ => None,
        }
    }
}

/// A card image. Built-in experiences embed theirs in the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thumbnail {
    /// PNG bytes, typically from `include_bytes!`.
    Embedded(&'static [u8]),
    /// A path for the `AssetServer`.
    Asset(&'static str),
}

#[derive(Debug, Clone)]
pub struct Experience {
    /// Stable kebab-case id, used on the command line.
    pub id: &'static str,
    pub title: &'static str,
    /// One sentence: what it teaches.
    pub summary: &'static str,
    /// Lobby heading.
    pub category: &'static str,
    pub thumbnail: Option<Thumbnail>,
    pub requires: Requirement,
    /// Extra information for the card, e.g. a one-time download.
    pub note: Option<&'static str>,
    pub kind: ExperienceKind,
}

impl Experience {
    pub fn is_available(&self) -> bool {
        self.requires.is_met()
    }
}

/// Every registered experience, grouped by category in the order categories
/// were first registered, and in registration order within a category.
#[derive(Resource, Debug, Clone, Default)]
pub struct ExperienceRegistry {
    entries: Vec<Experience>,
}

impl ExperienceRegistry {
    /// Add an experience; fails on an invalid or duplicate id.
    pub fn add(&mut self, experience: Experience) -> Result<(), String> {
        let id = experience.id;
        if !is_kebab_case(id) {
            return Err(format!(
                "experience id {id:?} must be kebab-case (a-z, 0-9, single hyphens)"
            ));
        }
        if self.get(id).is_some() {
            return Err(format!("experience id {id:?} is already registered"));
        }
        self.entries.push(experience);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Experience> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Categories in first-registration order, each with its experiences.
    pub fn by_category(&self) -> Vec<(&'static str, Vec<&Experience>)> {
        let mut groups: Vec<(&'static str, Vec<&Experience>)> = Vec::new();
        for e in &self.entries {
            match groups.iter_mut().find(|(c, _)| *c == e.category) {
                Some((_, list)) => list.push(e),
                None => groups.push((e.category, vec![e])),
            }
        }
        groups
    }

    /// All experiences in lobby order.
    pub fn iter(&self) -> impl Iterator<Item = &Experience> {
        self.by_category().into_iter().flat_map(|(_, list)| list)
    }
}

/// One card as the lobby shows it, after any [`LobbyEntry`] override.
///
/// [`LobbyEntry`]: crate::authoring::LobbyEntry
#[derive(Debug, Clone)]
pub struct LobbyCard<'a> {
    pub experience: &'a Experience,
    pub title: String,
    pub summary: String,
}

/// The lobby's categories and cards, and any entries naming experiences
/// that don't exist.
#[derive(Debug, Clone, Default)]
pub struct LobbyLayout<'a> {
    pub categories: Vec<(String, Vec<LobbyCard<'a>>)>,
    pub unknown: Vec<String>,
}

impl ExperienceRegistry {
    /// The lobby with `entries` applied. Experiences with an entry come
    /// first, by `order` (ties keep lobby order); the rest follow in lobby
    /// order. Hidden ones are left out. Categories appear in the order of
    /// their first card. With no entries this is exactly `by_category`.
    pub fn lobby_layout<'a>(
        &'a self,
        entries: &[&crate::authoring::LobbyEntry],
    ) -> LobbyLayout<'a> {
        let entry = |id: &str| entries.iter().rev().find(|e| e.experience == id).copied();
        let unknown = entries
            .iter()
            .filter(|e| self.get(&e.experience).is_none())
            .map(|e| e.experience.clone())
            .collect();
        let lobby: Vec<&Experience> = self.iter().collect();
        let mut ordered: Vec<(usize, &Experience)> = lobby
            .iter()
            .enumerate()
            .filter(|(_, e)| entry(e.id).is_some())
            .map(|(i, e)| (i, *e))
            .collect();
        ordered.sort_by_key(|(i, e)| (entry(e.id).map_or(0, |x| x.order), *i));
        let rest = lobby.iter().filter(|e| entry(e.id).is_none()).copied();
        let mut categories: Vec<(String, Vec<LobbyCard<'a>>)> = Vec::new();
        for e in ordered.into_iter().map(|(_, e)| e).chain(rest) {
            let over = entry(e.id);
            if over.is_some_and(|o| o.hidden) {
                continue;
            }
            let category = over
                .and_then(|o| o.category.clone())
                .unwrap_or_else(|| e.category.to_string());
            let card = LobbyCard {
                experience: e,
                title: over
                    .and_then(|o| o.title.clone())
                    .unwrap_or_else(|| e.title.to_string()),
                summary: over
                    .and_then(|o| o.summary.clone())
                    .unwrap_or_else(|| e.summary.to_string()),
            };
            match categories.iter_mut().find(|(c, _)| *c == category) {
                Some((_, cards)) => cards.push(card),
                None => categories.push((category, vec![card])),
            }
        }
        LobbyLayout {
            categories,
            unknown,
        }
    }
}

fn is_kebab_case(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Register experiences on an [`App`].
pub trait RegisterExperience {
    /// Add an experience to the [`ExperienceRegistry`].
    ///
    /// # Panics
    /// If the id is not kebab-case or is already registered: a programming
    /// error, caught the first time the app is built.
    fn register_experience(&mut self, experience: Experience) -> &mut Self;
}

impl RegisterExperience for App {
    fn register_experience(&mut self, experience: Experience) -> &mut Self {
        self.init_resource::<ExperienceRegistry>();
        let result = self
            .world_mut()
            .resource_mut::<ExperienceRegistry>()
            .add(experience);
        if let Err(e) = result {
            panic!("bevaru: {e}");
        }
        self
    }
}

/// The running experience, if any.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActiveExperience(pub Option<&'static str>);

/// Run condition: true while the experience `id` is active.
pub fn in_experience(id: &'static str) -> impl Fn(Option<Res<ActiveExperience>>) -> bool + Clone {
    move |active| active.is_some_and(|a| a.0 == Some(id))
}

/// Triggered when an experience starts.
#[derive(Event, Debug, Clone, Copy)]
pub struct ExperienceStarted {
    pub id: &'static str,
}

/// Triggered when an experience stops, before its [`ExperienceEntity`]
/// entities are despawned.
#[derive(Event, Debug, Clone, Copy)]
pub struct ExperienceStopped {
    pub id: &'static str,
}

/// Marks an entity as belonging to the running experience; it is despawned
/// when the experience stops.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ExperienceEntity;

/// The registry, the built-in experiences, and the experience lifecycle.
pub struct ExperiencesPlugin;

impl Plugin for ExperiencesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExperienceRegistry>()
            .init_resource::<ActiveExperience>()
            .add_observer(despawn_experience_entities);
        builtin::register(app);
        app.add_plugins(layout::SideInsetsPlugin);
        // The 3-D experiences need mesh and material assets and gizmos, but no
        // renderer, so headless apps with those run them too (the lobby tests
        // do). Apps without them still list them.
        let world = app.world();
        if world.contains_resource::<Assets<Mesh>>()
            && world.contains_resource::<Assets<StandardMaterial>>()
            && app.is_plugin_added::<bevy::gizmos::GizmoPlugin>()
        {
            app.add_plugins((
                loss_shapes::LossShapesExperiencePlugin,
                loss_surface::LossSurfaceExperiencePlugin,
                perceptron::PerceptronExperiencePlugin,
            ));
        }
        // The sigmoid experience draws a sprite, so it needs a renderer;
        // headless apps still list it.
        if app.is_plugin_added::<bevy::render::RenderPlugin>() {
            app.add_plugins(sigmoid::SigmoidExperiencePlugin);
        }
    }
}

fn despawn_experience_entities(
    _stopped: On<ExperienceStopped>,
    mut commands: Commands,
    entities: Query<Entity, With<ExperienceEntity>>,
) {
    for e in &entities {
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(id: &'static str, category: &'static str) -> Experience {
        Experience {
            id,
            title: id,
            summary: "",
            category,
            thumbnail: None,
            requires: Requirement::None,
            note: None,
            kind: ExperienceKind::Custom,
        }
    }

    fn registry() -> ExperienceRegistry {
        let mut app = App::new();
        app.add_plugins(ExperiencesPlugin);
        app.world().resource::<ExperienceRegistry>().clone()
    }

    #[test]
    fn built_in_experiences_are_registered() {
        let r = registry();
        for id in [
            "iris-svm",
            "regression-mse-vs-mae",
            "loss-curves",
            "mnist-svm",
            "loss-shapes",
            "loss-surface",
            "perceptron",
            "sigmoid",
        ] {
            let e = r.get(id).unwrap_or_else(|| panic!("{id} missing"));
            assert!(is_kebab_case(e.id));
            assert!(!e.category.is_empty() && !e.title.is_empty() && !e.summary.is_empty());
        }
        let ids: Vec<&str> = r.iter().map(|e| e.id).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn mnist_availability_follows_the_feature() {
        let r = registry();
        let mnist = r.get("mnist-svm").unwrap();
        assert_eq!(mnist.is_available(), cfg!(feature = "mnist"));
        if !cfg!(feature = "mnist") {
            assert!(
                mnist
                    .requires
                    .unmet_reason()
                    .unwrap()
                    .contains("--features mnist")
            );
        } else {
            assert!(mnist.note.unwrap().contains("11 MB"));
        }
    }

    #[test]
    #[should_panic(expected = "\"iris-svm\" is already registered")]
    fn duplicate_ids_panic_with_the_id() {
        let mut app = App::new();
        app.add_plugins(ExperiencesPlugin)
            .register_experience(custom("iris-svm", "Mine"));
    }

    #[test]
    fn ids_must_be_kebab_case() {
        let mut r = ExperienceRegistry::default();
        for bad in ["", "Iris", "iris_svm", "-iris", "iris-", "a--b", "a b"] {
            assert!(r.add(custom(bad, "x")).is_err(), "{bad:?} accepted");
        }
        assert!(r.add(custom("ok-1", "x")).is_ok());
    }

    #[test]
    fn third_party_entries_group_by_category_in_order() {
        let mut app = App::new();
        app.add_plugins(ExperiencesPlugin)
            .register_experience(custom("relu", "Activation functions"))
            .register_experience(custom("attention", "Transformers"));
        let r = app.world().resource::<ExperienceRegistry>();
        let groups = r.by_category();
        let activations = groups
            .iter()
            .find(|(c, _)| *c == "Activation functions")
            .unwrap();
        // Sigmoid registered first, then the third-party entry.
        let ids: Vec<&str> = activations.1.iter().map(|e| e.id).collect();
        assert_eq!(ids, ["sigmoid", "relu"]);
        assert_eq!(groups.last().unwrap().0, "Transformers");
    }

    fn entry(id: &str, order: i32) -> crate::authoring::LobbyEntry {
        crate::authoring::LobbyEntry {
            experience: id.into(),
            order,
            ..Default::default()
        }
    }

    fn titles<'a>(layout: &LobbyLayout<'a>) -> Vec<(String, Vec<String>)> {
        layout
            .categories
            .iter()
            .map(|(c, cards)| (c.clone(), cards.iter().map(|k| k.title.clone()).collect()))
            .collect()
    }

    #[test]
    fn lobby_layout_without_entries_is_the_registry() {
        let r = registry();
        let layout = r.lobby_layout(&[]);
        let expected: Vec<(String, Vec<String>)> = r
            .by_category()
            .into_iter()
            .map(|(c, list)| {
                (
                    c.to_string(),
                    list.iter().map(|e| e.title.to_string()).collect(),
                )
            })
            .collect();
        assert_eq!(titles(&layout), expected);
        assert!(layout.unknown.is_empty());
        let summaries: Vec<&str> = layout
            .categories
            .iter()
            .flat_map(|(_, cards)| cards.iter().map(|k| k.summary.as_str()))
            .collect();
        let registered: Vec<&str> = r.iter().map(|e| e.summary).collect();
        assert_eq!(summaries, registered);
    }

    #[test]
    fn lobby_entries_reorder_retitle_recategorise_and_hide() {
        let r = registry();
        let shapes = crate::authoring::LobbyEntry {
            title: Some("Shapes of loss".into()),
            ..entry("loss-shapes", 0)
        };
        let iris = entry("iris-svm", 1);
        let moved = crate::authoring::LobbyEntry {
            category: Some("Loss functions".into()),
            ..entry("perceptron", 2)
        };
        let hidden = crate::authoring::LobbyEntry {
            hidden: true,
            ..entry("sigmoid", 3)
        };
        let ghost = entry("no-such-experience", 4);
        let layout = r.lobby_layout(&[&shapes, &iris, &moved, &hidden, &ghost]);
        let flat: Vec<&str> = layout
            .categories
            .iter()
            .flat_map(|(_, cards)| cards.iter().map(|k| k.experience.id))
            .collect();
        // Ordered entries first, in order, grouped by category.
        let cats: Vec<&str> = layout.categories.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(cats[0], "Loss functions");
        assert_eq!(layout.categories[0].1[0].title, "Shapes of loss");
        let pos = |id: &str| flat.iter().position(|x| *x == id).unwrap();
        assert!(pos("loss-shapes") < pos("iris-svm"));
        // Moved: perceptron is under Loss functions; Diagrams is gone (empty).
        assert!(
            layout.categories[0]
                .1
                .iter()
                .any(|k| k.experience.id == "perceptron")
        );
        assert!(!cats.contains(&"Diagrams"));
        // Hidden: no Sigmoid card, but still registered and startable by id.
        assert!(!flat.contains(&"sigmoid"));
        assert!(r.get("sigmoid").is_some());
        assert!(!cats.contains(&"Activation functions"));
        // Unknown ids are reported, not shown.
        assert_eq!(layout.unknown, ["no-such-experience"]);
        // Every registered, non-hidden experience is still shown exactly once.
        assert_eq!(flat.len(), r.len() - 1);
    }

    #[test]
    fn every_available_experiment_experience_builds() {
        // Feature-gated experiences (MNIST) need the network or a cached
        // download; the core crate's ignored test covers that path.
        let offline = |e: &&Experience| e.is_available() && e.requires == Requirement::None;
        for e in registry().iter().filter(offline) {
            if let ExperienceKind::Experiment { spec, .. } = &e.kind
                && let Err(err) = crate::experiment::Experiment::build(spec())
            {
                panic!("{} failed to build: {err}", e.id);
            }
        }
    }
}
