//! The committed scenes are current, and load into a world with the
//! components they promise.

use bevaru::authoring::{LobbyEntry, LossShapeSurface, PerceptronDiagram};
use bevaru::experiences::{ExperienceRegistry, ExperiencesPlugin};
use bevaru_jackdaw::scenes::{self, SCENES};
use bevy::prelude::*;
use jackdaw_bsn::{apply_dirty_ast_patches, parse_bsn_text, spawn_from_ast};

fn committed(path: &str) -> String {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{}: {e}", full.display()))
}

/// A world with bevaru's authorable types (and Bevy's) registered, holding
/// the scene's entities.
fn load(path: &str) -> World {
    // `scenes::world` registers bevaru's authorable types (AuthoringPlugin).
    let mut app = scenes::world();
    let mut world = std::mem::take(app.world_mut());
    let ast = parse_bsn_text(&committed(path)).unwrap_or_else(|e| panic!("{path}: {e:?}"));
    world.insert_resource(ast);
    spawn_from_ast(&mut world);
    apply_dirty_ast_patches(&mut world);
    world
}

#[test]
fn committed_scenes_are_current() {
    for (path, scene) in SCENES {
        assert!(
            committed(path) == scene(),
            "assets/{path} is out of date: run `cargo run --example generate_scenes` in jackdaw/"
        );
    }
}

#[test]
fn lobby_scene_has_one_entry_per_experience() {
    let mut world = load("lobby.bsn");
    let mut ids: Vec<String> = world
        .query::<&LobbyEntry>()
        .iter(&world)
        .map(|e| e.experience.clone())
        .collect();
    let mut app = App::new();
    app.add_plugins(ExperiencesPlugin);
    let registry = app.world().resource::<ExperienceRegistry>();
    let mut expected: Vec<String> = registry.iter().map(|e| e.id.to_string()).collect();
    // Orders follow the lobby, so the default layout is unchanged.
    let entries: Vec<LobbyEntry> = world.query::<&LobbyEntry>().iter(&world).cloned().collect();
    let refs: Vec<&LobbyEntry> = entries.iter().collect();
    let layout = registry.lobby_layout(&refs);
    let laid_out: Vec<&str> = layout
        .categories
        .iter()
        .flat_map(|(_, cards)| cards.iter().map(|c| c.experience.id))
        .collect();
    let lobby_order: Vec<&str> = registry.iter().map(|e| e.id).collect();
    assert_eq!(laid_out, lobby_order);
    ids.sort();
    expected.sort();
    assert_eq!(ids, expected);
}

#[test]
fn example_scenes_hold_valid_components() {
    let mut world = load("examples/perceptron.bsn");
    let diagrams: Vec<PerceptronDiagram> = world
        .query::<&PerceptronDiagram>()
        .iter(&world)
        .cloned()
        .collect();
    assert_eq!(diagrams, [PerceptronDiagram::default()]);
    assert_eq!(world.query::<&Camera3d>().iter(&world).count(), 1);

    let mut world = load("examples/loss-shapes.bsn");
    let surfaces: Vec<LossShapeSurface> = world
        .query::<&LossShapeSurface>()
        .iter(&world)
        .cloned()
        .collect();
    assert_eq!(surfaces.len(), 3);
    for s in &surfaces {
        s.resolve()
            .unwrap_or_else(|(field, why)| panic!("{}: {field}: {why}", s.view));
    }
}
