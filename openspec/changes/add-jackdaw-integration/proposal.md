## Why

[Jackdaw](https://github.com/jbuehler23/jackdaw) is the interim scene editor for Bevy 0.19, the same Bevy bevaru uses. [#29](https://github.com/buddha314/bevaru/issues/29) asks for the files that let people edit bevaru's lobby, and reuse bevaru's visual assets (the 3-D perceptron and the loss-shape surfaces), inside Jackdaw.

A spike on 2026-10-04 showed this is practical:
- **Components:** Jackdaw puts any `#[derive(Component, Reflect, Default)]` type from a project's dependencies in its Add Component picker, with doc comments as tooltips, and needs no Jackdaw-specific code.
- **It builds:** a fresh Jackdaw project depending on bevaru plus the pinned `jackdaw_runtime` compiled cleanly in 2.5 minutes, on one Bevy 0.19.1 with no conflicts.
- **What's missing:** bevaru has no authorable components yet. Everything is built in code by the experiences.

## What Changes

- **Authorable components in bevaru,** with no Jackdaw dependency. Each is plain reflected data, and bevaru builds the geometry when one is added or changed:
  - `PerceptronDiagram { weights, bias, activation, labels }`: the perceptron from `src/diagram.rs`, as children of the entity.
  - `LossShapeSurface { view, huber_delta, margin, resolution, entropy_removed }`: one loss-shape surface, as a mesh child.
  - `LobbyEntry { experience, order, title, summary, category, hidden }`: an override for one lobby card.
- **`AuthoringPlugin`:** registers these types explicitly, so a linker can't strip them from Jackdaw's schema, and runs the systems that build them. A user's own game adds it to use bevaru's assets.
- **Lobby layout overrides:** while `LobbyEntry` entities exist, the lobby uses them to reorder, retitle, recategorise, or hide cards. A hidden experience still opens from `cargo run -- <id>`. Without entries, the lobby is unchanged.
- **A ready-made Jackdaw project in `jackdaw/`:** its own cargo `[workspace]`, so it never touches bevaru's build. It contains:
  - `jackdaw.toml` naming its root plugin;
  - a `Cargo.toml` with `bevy = "0.19"`, bevaru as a path dependency, and `jackdaw_runtime` pinned to the git revision `jd 0.19.0` generates;
  - a `GamePlugin` that runs the bevaru app (lobby included) with the Jackdaw runtime;
  - **`assets/lobby.bsn`**, one `LobbyEntry` per built-in experience, which is the lobby to edit;
  - **example scenes** showing a perceptron and loss shapes placed in a scene, to copy into a user's own project.

  Open it with `jd open jackdaw`; Play, or `cargo run` in `jackdaw/`, runs bevaru with the edited lobby.
- **Docs** (`docs/jackdaw.md` and a README section):
  - how to edit the lobby;
  - how to use bevaru's components in your own Jackdaw project (bevaru is not on crates.io, so as a git dependency);
  - the upstream findings.
- **Upstream findings,** as drafts for the maintainer. Nothing is filed.
  - `jd import` rejects an exact pin like `bevy = "=0.19.1"` as a Bevy mismatch, though it is Bevy 0.19.
  - `jd import` suggests `cargo add jackdaw_runtime@0.19`, but 0.19 isn't on crates.io (the newest is 0.4.1, for Bevy 0.18). `jd new` uses a pinned git revision instead.

## Capabilities

### New Capabilities
- `jackdaw-integration`: the authorable components and `AuthoringPlugin`, the `jackdaw/` project with its lobby and example scenes, the build check, and the docs and upstream findings.

### Modified Capabilities
- `experience-lobby`: the lobby SHALL apply `LobbyEntry` overrides (order, title, summary, category, hidden) when present, and be unchanged when none are.

## Impact

- **Code:**
  - new `src/authoring.rs`;
  - lobby ordering and labels read the overrides (`src/lobby.rs`, `src/experiences/mod.rs`);
  - a new standalone `jackdaw/` project.
- **Dependencies:**
  - **bevaru:** none new. bevaru never depends on Jackdaw.
  - **`jackdaw/` alone:** depends on `jackdaw_runtime` (git, pinned) and `avian3d` 0.7.
- **Build:** `jackdaw/` is not a workspace member, so `cargo build` and `scripts/check.sh` for bevaru are unaffected. A separate opt-in `scripts/check-jackdaw.sh` builds it. It's slower and needs the network.
- **Existing behaviour:** unchanged when no `LobbyEntry` exists, which is every run outside `jackdaw/`.
