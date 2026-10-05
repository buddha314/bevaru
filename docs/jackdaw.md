# bevaru in the Jackdaw editor

[Jackdaw](https://github.com/jbuehler23/jackdaw) is a scene editor for Bevy 0.19, the Bevy bevaru uses. With it you can:

- **edit bevaru's lobby**: reorder, reword, recategorise, or hide cards;
- **use bevaru's visual assets in your own scenes**: the 3-D perceptron diagram and the loss-shape surfaces, as ordinary components you add in the inspector.

bevaru itself has no Jackdaw dependency. The integration is a separate project in [`jackdaw/`](../jackdaw), plus three reflected components in bevaru ([`src/authoring.rs`](../src/authoring.rs)).

## Requirements

Install Jackdaw 0.19 (it builds on a pinned nightly; see its README):

```sh
cargo +nightly-2026-03-05 install --git https://github.com/jbuehler23/jackdaw jackdaw --locked
```

Or use a [release bundle](https://github.com/jbuehler23/jackdaw/releases). `jd --version` should report `jd 0.19.0 (targets bevy 0.19)`.

## Edit the lobby

```sh
jd open jackdaw        # from the bevaru repository root
```

1. Open `assets/lobby.bsn`. The hierarchy has a `Lobby` entity with one child per experience (`iris-svm`, `loss-shapes`, …), each holding a **LobbyEntry**.
2. Select one and edit it in the inspector:

   | Field | Effect |
   | ----- | ------ |
   | `order` | Position: lower comes first. The generated file uses steps of 10, so a card can be slotted in between. |
   | `title`, `summary` | Replace the card's wording. |
   | `category` | Move the card under another heading (a new heading is created; an emptied one disappears). |
   | `hidden` | Leave the card out of the lobby. The experience still opens with `cargo run -- <id>` and from the examples. |

3. Save, then press **Play** (the *Lobby* run), or run it from a terminal:

   ```sh
   cd jackdaw && cargo run --release
   ```

**Edits apply to this project, not to the main `bevaru` binary.** `cargo run` at the repository root shows the standard lobby. The edited lobby is what `jackdaw/` runs.

Experiences without an entry keep their place after the ordered ones, so deleting an entry restores that card's defaults.

## Use bevaru's assets in your scenes

Three components appear in Jackdaw's **Add Component** picker, with their doc comments as tooltips:

| Component | What it builds | Fields |
| --------- | -------------- | ------ |
| **PerceptronDiagram** | the 3-D perceptron: inputs, weighted tubes (blue positive, orange negative, thickness \|w\|), Σ, activation, output | `weights`, `bias`, `activation` (sigmoid or step), `labels` |
| **LossShapeSurface** | a loss as a 3-D surface over two inputs, coloured cool to warm by height, in a 10 × 10 × 6 box (scale the entity to resize it) | `view` (an id such as `two-scores-hinge`; see [the loss-shape list](agents/capabilities.md#loss-shapes)), `huber_delta`, `margin`, `resolution`, `entropy_removed` |
| **LobbyEntry** | nothing visible; overrides a lobby card (above) | `experience`, `order`, `title`, `summary`, `category`, `hidden` |

They're listed without a category. Jackdaw's category attribute (`@EditorCategory`) is a type from `jackdaw_runtime`, which bevaru deliberately doesn't depend on, so search for them by name.

**The editor's viewport shows the authored scene without running game code**, so a `PerceptronDiagram` looks like an empty entity until you press Play. Play runs the game, which builds it. Every field edit rebuilds it live.

`jackdaw/assets/examples/` has a scene for each component. Open one, then pick its Play run (*Perceptron example*, *Loss shapes example*), or:

```sh
cd jackdaw && BEVARU_SCENE=examples/perceptron.bsn cargo run --release
```

### In your own Jackdaw project

bevaru isn't on crates.io, so add it from git, next to `jackdaw_runtime`:

```toml
[dependencies]
bevaru = { git = "https://github.com/buddha314/bevaru" }
```

Then add the plugin in your `GamePlugin`:

```rust
app.add_plugins(bevaru::authoring::AuthoringPlugin);
```

`AuthoringPlugin` registers the components (so Jackdaw finds them after **Rebuild Project**) and builds them. It needs neither the lobby nor any other part of bevaru. The perceptron's labels draw only if your app has `bevy_egui`'s `EguiPlugin` (bevaru uses `bevy_egui` 0.42), and bevaru's diagrams are drawn for a light background.

## Keeping the project current

- **New experiences:** `jackdaw/assets/lobby.bsn` is generated from bevaru's registry. After adding an experience, regenerate it:

  ```sh
  cd jackdaw && cargo run --example generate_scenes
  ```

  This overwrites the three starter scenes, so commit editor edits first. A test fails until the committed files match.
- **Checks:** `scripts/check-jackdaw.sh` builds `jackdaw/`, runs clippy, and tests that the scenes are current and load. It's separate from `scripts/check.sh` because it fetches Jackdaw from git and compiles its runtime and avian (minutes, and it needs the network). Run it after changing `jackdaw/` or `src/authoring.rs`. `scripts/check.sh --tests` separately checks that no Jackdaw crate enters bevaru's own dependency tree.
- **Upgrading Jackdaw:** the `jackdaw_runtime` revision in `jackdaw/Cargo.toml` is the one `jd 0.19.0` writes for a new project. After updating Jackdaw:

  ```sh
  jd upgrade jackdaw            # preview
  jd upgrade jackdaw --apply
  ```

  `jd upgrade` leaves git dependencies alone, so also update the `rev` of `jackdaw_runtime` and `jackdaw_bsn` (both must match) to what `jd new` writes. Then run `scripts/check-jackdaw.sh`.

## Upstream findings (drafts, not filed)

Found while setting this up (2026-10-04, `jd 0.19.0`). These are drafts for the maintainer to file, not filed issues.

### `jd import` rejects an exact Bevy pin

**Reproduce:** a Bevy 0.19 project whose manifest pins `bevy = "=0.19.1"` (bevaru does):

```text
$ jd import /path/to/bevaru
jd import: this jackdaw targets Bevy 0.19, and the project depends on bevy =0.19.1.
The editor and your game code have to share one Bevy.
```

> **`jd import`: an exact patch pin (`bevy = "=0.19.1"`) is reported as a Bevy-minor mismatch**
>
> `jd import` and `jd doctor` compare the project's Bevy requirement with the editor's minor, but `=0.19.1` (an exact requirement within 0.19) is reported as a mismatch, so the project needs `--allow-bevy-mismatch`, and the mismatch note persists in `jackdaw.toml`. Expected: any requirement that only admits 0.19.x versions (`=0.19.1`, `~0.19.1`, `>=0.19.1, <0.20`) is accepted as Bevy 0.19.

### `jd import` suggests a version that isn't published

**Reproduce:** `jd import --allow-bevy-mismatch` on any project without `jackdaw_runtime` prints:

```text
note: to load your authored .bsn scenes in the game, add the runtime:
  cargo add jackdaw_runtime@0.19 --features physics,pie
```

crates.io has `jackdaw_runtime` 0.4.1 (Bevy 0.18) as its newest version, so `cargo add jackdaw_runtime@0.19` fails. `jd new` instead writes a git dependency pinned to a revision.

> **`jd import` suggests `cargo add jackdaw_runtime@0.19`, which crates.io doesn't have**
>
> The 0.19 line isn't published (newest is 0.4.1, for Bevy 0.18). Suggest the same git dependency `jd new` writes, e.g. `jackdaw_runtime = { git = "https://github.com/jbuehler23/jackdaw", rev = "<rev>", features = ["physics", "pie"] }`, until 0.19 is on crates.io.
