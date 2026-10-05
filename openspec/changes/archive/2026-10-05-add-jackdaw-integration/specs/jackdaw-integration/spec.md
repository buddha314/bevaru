## ADDED Requirements

### Requirement: Authorable components
The library SHALL provide reflected components that a scene editor can add to entities, and that bevaru turns into its visual assets. They SHALL need no Jackdaw dependency in bevaru, and each SHALL be `Component`, `Reflect`, and `Default`, with a doc comment describing it:
- `PerceptronDiagram`: the three weights, the bias, the activation (sigmoid or step), and label visibility. It builds the perceptron diagram as children of its entity.
- `LossShapeSurface`: a loss-shape view id plus its hyperparameters (Huber δ, margin, resolution, entropy-removed). It builds that surface as a mesh child.
- `LobbyEntry`: overrides for one lobby card (see the experience-lobby capability).

#### Scenario: Building on add
- **WHEN** an entity gains a `PerceptronDiagram` with default values
- **THEN** within one update it has the perceptron's seven nodes and six edges as descendants

#### Scenario: Rebuilding on change
- **WHEN** a `PerceptronDiagram`'s first weight changes from positive to negative
- **THEN** its children are rebuilt, the first weight's tube takes the negative colour, and no previous children remain

#### Scenario: Removal cleans up
- **WHEN** the component is removed from the entity
- **THEN** the entity's authored children are despawned

#### Scenario: Invalid values don't panic
- **WHEN** a `LossShapeSurface` names an unknown view id
- **THEN** one warning names the entity and the field, no surface is built, and the app keeps running

### Requirement: Authoring plugin
The library SHALL provide `AuthoringPlugin`, which registers every authorable type for reflection explicitly and runs the systems that build them. It SHALL work without the lobby or the experiences, so another game can add it alone to use bevaru's assets.

#### Scenario: Registered types
- **WHEN** `AuthoringPlugin` is added to an app
- **THEN** the type registry contains `PerceptronDiagram`, `LossShapeSurface`, and `LobbyEntry`, each reflecting `Component` and `Default`

#### Scenario: Standalone use
- **WHEN** an app with only Bevy's minimal plugins, asset support, and `AuthoringPlugin` spawns a `LossShapeSurface`
- **THEN** the surface mesh is built without any bevaru experience or lobby plugin

### Requirement: Jackdaw project
The repository SHALL contain a standalone Jackdaw project at `jackdaw/`. It SHALL be outside bevaru's cargo workspace, and SHALL contain:
- **Its manifest:** depends on Bevy 0.19, on bevaru by path, and on `jackdaw_runtime` at a pinned git revision.
- **`jackdaw.toml`:** names its root plugin.
- **The game:** a library plugin that runs the bevaru app with the Jackdaw runtime and loads `assets/lobby.bsn`.
- **`assets/lobby.bsn`:** one `LobbyEntry` per built-in experience.
- **Example scenes** that place a perceptron diagram and loss-shape surfaces.

bevaru's own build, tests, and `scripts/check.sh` SHALL NOT build or resolve it. An opt-in script SHALL build it and run its tests.

#### Scenario: Isolated from bevaru
- **WHEN** `cargo build` or `scripts/check.sh` runs at the repository root
- **THEN** no Jackdaw crate is resolved or compiled

#### Scenario: The project builds
- **WHEN** the opt-in Jackdaw check runs
- **THEN** `jackdaw/` builds, and its committed `.bsn` scenes load headlessly with the expected components

#### Scenario: Edited lobby runs
- **WHEN** `lobby.bsn` reorders two cards and hides a third, and the project is run
- **THEN** the lobby shows the new order without the hidden card

### Requirement: Jackdaw documentation
The project SHALL document, in `docs/jackdaw.md` and a README section:
- how to open `jackdaw/` in Jackdaw, edit the lobby, and run it;
- how to use bevaru's authorable components in one's own Jackdaw project, as a git dependency, since bevaru is not on crates.io;
- that lobby edits apply to the Jackdaw project, not the main `bevaru` binary;
- how to upgrade the pinned Jackdaw revision;
- the upstream findings, as draft issues that are not filed without the maintainer's approval.

#### Scenario: Findings recorded
- **WHEN** `docs/jackdaw.md` is read
- **THEN** it describes `jd import` rejecting an exact Bevy pin, and its unavailable `cargo add jackdaw_runtime@0.19` suggestion, each with reproduction steps
