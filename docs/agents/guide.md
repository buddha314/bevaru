# Building with bevaru: a guide for agents

This guide is for agents (and people) building applications that use bevaru. For exact ids, ranges, and schemas, use the [capability reference](capabilities.md) or [`capabilities.json`](capabilities.json): both are generated from the code and always current.

## Pick a level

| You want to… | Use | Needs a window? |
| ------------ | --- | --------------- |
| Compute losses, train models, run sweeps in your own code | `bevaru-core` (re-exported as `bevaru::core`) | No |
| Show an interactive visualization of a dataset and models | `BevaruPlugin` with an `ExperimentSpec` | Yes |
| Offer several visualizations behind a menu | `LobbyPlugin` and the experience registry | Yes |
| Train and sweep inside a Bevy app without rendering | `BevaruPlugin` after `MinimalPlugins` | No |

## Concepts

- **Experiment:** a dataset, how it is displayed (a `View`), and one to three trainers compared side by side (`ExperimentSpec`). Loading one builds the data, the display, and the trainers off the main thread.
- **Trainer:** a model plus its loss and hyperparameters (`TrainerConfig`). The loss decides the model: hinge and squared-hinge train an SVM, logistic trains logistic regression, mse/mae/huber train linear regression.
- **Experience:** an entry in the lobby. Either an experiment with an action to run once loaded (play, sweep, show losses), or custom: your own systems, active only while the experience runs.
- **Messages:** you drive bevaru by sending Bevy messages: `LoadExperiment`, `PlaybackCommand`, `SweepCommand`, `EnterExperience`, `LeaveExperience`. Each is listed with its variants in the [reference](capabilities.md#messages).

## A minimal app

This is [`examples/minimal_app.rs`](../../examples/minimal_app.rs) (`cargo run --example minimal_app`):

<!-- include: examples/minimal_app.rs -->
```rust
//! The smallest bevaru app: one experiment, SVM beside logistic regression on Iris.
//!
//! ```sh
//! cargo run --example minimal_app
//! ```

use bevaru::core::TrainerConfig;
use bevaru::{BevaruPlugin, DatasetChoice, ExperimentSpec, StartupExperiment};
use bevy::prelude::*;

fn main() {
    let spec = ExperimentSpec::new(
        DatasetChoice::Iris {
            positive: "versicolor".into(),
            negative: Some("virginica".into()),
        },
        TrainerConfig::svm(1.0).expect("C = 1 is valid"),
    )
    .compare(TrainerConfig::logistic());

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(StartupExperiment(spec))
        .add_plugins(BevaruPlugin) // after DefaultPlugins
        .run();
}
```

Add `bevaru` with `bevaru = { git = "https://github.com/buddha314/bevaru" }`. `BevaruPlugin` adds the scene, charts, and control panel when a renderer is present, and runs headless otherwise.

## The lobby and your own experiences

`bevaru::app::windowed_app(None)` builds the same app as `cargo run`: a window, `BevaruPlugin`, and the lobby. Register more experiences on it with `register_experience`; they appear in the lobby, work with `cargo run -- <id>` in that binary, and are cleaned up on leave like the built-ins. See [Add an experience](recipes/add-an-experience.md).

Rules for custom experiences:
- gate your systems with `run_if(in_experience("your-id"))`;
- set up in an observer of `ExperienceStarted`, and remove your resources in one of `ExperienceStopped`;
- mark every entity you spawn with `ExperienceEntity`; it is despawned when the experience stops.

## Without a window

For computation only, use `bevaru-core` directly: build a `Dataset`, get its `TrainingData`, and step a `Trainer`. Every step's parameters and loss are kept, so you can inspect the whole trajectory. See [Train a model headlessly](recipes/train-headlessly.md).

## Without writing code: MCP

If you are an agent with an MCP client, [`bevaru-mcp`](mcp.md) lets you evaluate losses, build datasets, train and sweep models, and render charts by calling tools, with no Rust and no window. Its inputs are the request types below.

## Validating input from users or other agents

`bevaru::agent::api` has request types (`ExperimentRequest`, `TrainerRequest`, `SweepRequest`, …) that deserialize from JSON, convert into library types with field-level validation, and have JSON Schemas (in [`capabilities.json`](capabilities.json) under `schemas`). Use them when your application accepts configurations from outside: an invalid request comes back as an `ApiError` naming the field and the valid range.

## Staying current

The [reference](capabilities.md) and [manifest](capabilities.json) are regenerated from the code and checked by a test, so they describe the version of bevaru you are reading. Ids (losses, models, sweep parameters, experiences) are stable; display names may change.
