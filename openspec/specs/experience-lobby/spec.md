# Experience Lobby Specification

## Purpose

Define the experience registry and the lobby where users pick an experience: how experiences are registered, started, and left cleanly, how unavailable ones are shown, and how to open one from the command line.

## Requirements

### Requirement: Experience registry
The library SHALL provide an experience registry that plugins add to with `App::register_experience`. Each entry SHALL have:
- a stable kebab-case id;
- a title;
- a one-sentence description of what it teaches;
- a category, used to group cards in the lobby;
- an optional thumbnail;
- any build requirement;
- a kind (see "Experience kinds").

Registering an id that is already registered SHALL be rejected with an error naming the id. The registry SHALL list entries grouped by category, keeping registration order within each category. The lobby, the `--example` binaries, and command-line selection SHALL all read from the registry.

#### Scenario: Built-in experiences are registered
- **WHEN** the default experiences are registered
- **THEN** the registry contains `iris-svm`, `regression-mse-vs-mae`, `loss-curves`, `mnist-svm`, and `sigmoid`, each with a unique kebab-case id and a category

#### Scenario: Third-party registration
- **WHEN** another plugin calls `register_experience` with a new id
- **THEN** that experience appears in the lobby under its category, can be started and left like a built-in one, and is reachable via `cargo run -- <id>` when the binary includes that plugin

#### Scenario: Duplicate id
- **WHEN** two experiences are registered with the same id
- **THEN** registration fails with an error naming the duplicate id

#### Scenario: Example and lobby agree
- **WHEN** `cargo run --example iris_svm` is started, and separately `iris-svm` is chosen in the lobby
- **THEN** both start the same registered experience with the same behaviour

### Requirement: Experience kinds
An experience SHALL be one of two kinds:
- **Experiment**: an experiment spec plus an action taken once it has loaded (none, play training, or start a hyperparameter sweep). It uses the standard scene, charts, and control panel.
- **Custom**: the registering plugin provides its own systems. Those systems run only while that experience is active. The plugin is notified when the experience starts and stops. Entities it marks as belonging to the experience are despawned automatically when it stops.

#### Scenario: Every available experiment experience loads
- **WHEN** each available experiment-kind experience is loaded headlessly
- **THEN** its experiment builds without error

#### Scenario: Custom systems are scoped to their experience
- **WHEN** the `sigmoid` experience is not active
- **THEN** none of its systems run and none of its entities exist

#### Scenario: Sigmoid as a custom experience
- **WHEN** the user starts `sigmoid`
- **THEN** the ruviz sigmoid plot is shown and refreshes as in the `ml_interactive` example, without the experiment control panel

### Requirement: Lobby screen
When the `bevaru` binary starts without arguments, it SHALL show a lobby listing every registered experience as a card. Cards SHALL be grouped under category headings and show the thumbnail, title, description, and requirements. Nothing SHALL be loaded and no training SHALL run while the lobby is shown.

#### Scenario: Lobby on plain start
- **WHEN** a user runs `cargo run`
- **THEN** the lobby is shown with one card per registered experience, grouped by category, and no experiment is loaded

#### Scenario: Lobby is idle
- **WHEN** the lobby has been shown for 60 frames
- **THEN** no experiment load has been requested, no chart has been rendered, and no custom experience's systems have run

### Requirement: Thumbnails
Each card SHALL show its experience's thumbnail. Every built-in experience SHALL ship a thumbnail embedded in the binary, so the lobby works regardless of the working directory. An experience without a thumbnail SHALL show a placeholder with its title, not a broken image. Built-in thumbnails SHALL be reproducible by a documented command.

#### Scenario: Built-in thumbnails present
- **WHEN** the lobby is shown
- **THEN** all five built-in cards show their thumbnail images

#### Scenario: Missing thumbnail
- **WHEN** an experience is registered without a thumbnail
- **THEN** its card shows a placeholder with its title and is otherwise fully usable

#### Scenario: Run from another directory
- **WHEN** the built binary is run from a directory other than the repository root
- **THEN** all built-in thumbnails still display

### Requirement: Starting an experience
Activating a card SHALL leave the lobby and start that experience:
- **Experiment kind**: load its experiment, show a loading indicator until it is ready, then run its on-loaded action.
- **Custom kind**: activate it and send its start notification.

#### Scenario: Pick the Iris sweep
- **WHEN** the user activates the `iris-svm` card
- **THEN** the lobby closes, a loading indicator appears, and once loaded the C sweep starts automatically

#### Scenario: Load failure returns to the lobby
- **WHEN** a chosen experience fails to load (for example the MNIST download fails)
- **THEN** the user is returned to the lobby and the error is shown on that experience's card

### Requirement: Returning to the lobby
Every experience, experiment or custom, SHALL offer a "Back to lobby" control, and `Esc` SHALL do the same. Leaving SHALL:
- stop training playback;
- cancel any in-progress sweep or experiment load;
- unload the experiment and its scene, cameras, and chart images;
- send a custom experience its stop notification and despawn its marked entities.

Starting another experience afterwards SHALL begin from a clean state.

#### Scenario: Back to lobby tears down an experiment
- **WHEN** a user is mid-sweep in `iris-svm` and presses `Esc`
- **THEN** the lobby is shown, no scene entities or pane cameras remain, the sweep's background tasks are dropped, and no experiment resource remains

#### Scenario: Back to lobby from a custom experience
- **WHEN** a user in `sigmoid` clicks "Back to lobby"
- **THEN** the lobby is shown and none of the sigmoid experience's entities remain

#### Scenario: Esc while typing does not leave
- **WHEN** a UI text field has keyboard focus and the user presses `Esc`
- **THEN** the experience keeps running

#### Scenario: Round trip
- **WHEN** a user opens `regression-mse-vs-mae`, returns to the lobby, then opens `sigmoid`
- **THEN** only the sigmoid plot is shown, with no regression panes, markers, or charts

### Requirement: Clean-up on leave
Leaving an experience SHALL return the app to the state it was in before that experience started. Compared with the lobby before the experience was entered:
- the number of entities SHALL be the same;
- the number of mesh, material, and image assets SHALL be the same;
- no egui texture registered for the experience SHALL remain;
- no background task (experiment load, sweep, chart render) started by the experience SHALL still be referenced.

Resources inserted by a custom experience SHALL be removed by its stop handling. This SHALL hold however the experience is left: the Back control, `Esc`, a load failure, or switching experiences through the command line or API.

#### Scenario: Repeated round trips do not grow the app
- **WHEN** each available experience is entered and left ten times in a row
- **THEN** entity, mesh, material, image, and egui texture counts in the lobby after the last round trip equal those before the first

#### Scenario: Leaving during a load
- **WHEN** a user leaves `mnist-svm` while its experiment is still loading
- **THEN** the lobby is shown immediately, and when the background load finishes its result is discarded without creating an `Experiment`, scene entities, or textures

#### Scenario: Leaving during a sweep or chart render
- **WHEN** a user leaves `iris-svm` while sweep solutions and chart renders are in progress
- **THEN** none of their results are applied after leaving, and the next experience's charts show only its own data

#### Scenario: Custom experience resources
- **WHEN** a user leaves `sigmoid`
- **THEN** its refresh timer and any other resources it inserted no longer exist

### Requirement: Clean-up on quit
Closing the window or exiting while an experience is running SHALL stop that experience first (its stop notification is sent and its background work is dropped), and SHALL NOT leave partial files behind. A partial MNIST download left by an earlier crash SHALL be removed or replaced on the next load, never read as data.

#### Scenario: Quit mid-experience
- **WHEN** the user closes the window while `iris-svm` is running a sweep
- **THEN** the experience's stop notification is sent before the app exits, and the process exits normally

#### Scenario: Stale partial download
- **WHEN** the MNIST cache directory contains a `.partial` file from an interrupted download
- **THEN** the next MNIST load ignores it as data, replaces it, and leaves no `.partial` file once the load succeeds

### Requirement: Availability gating
Experiences whose build requirement is not met SHALL be listed but disabled, with the reason and how to enable them.

#### Scenario: MNIST without the feature
- **WHEN** the binary is built without the `mnist` feature
- **THEN** the MNIST card is visible, cannot be activated, and says to rebuild with `--features mnist`

#### Scenario: MNIST with the feature
- **WHEN** the binary is built with the `mnist` feature
- **THEN** the MNIST card can be activated and notes the one-time ~11 MB download

### Requirement: Command-line selection
The `bevaru` binary SHALL handle its first argument as follows:
- an experience id opens that experience directly, skipping the lobby;
- `--list` prints every id with its title and exits;
- an unknown id prints the valid ids and exits with a non-zero status.

#### Scenario: Deep link
- **WHEN** a user runs `cargo run -- loss-curves`
- **THEN** the loss-curves experience opens without the lobby being shown, and "Back to lobby" still leads to the lobby

#### Scenario: Unknown id
- **WHEN** a user runs `cargo run -- nonsense`
- **THEN** the program prints the valid ids and exits with a non-zero status, without opening a window

