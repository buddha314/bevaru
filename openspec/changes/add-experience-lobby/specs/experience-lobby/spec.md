## ADDED Requirements

### Requirement: Experience catalogue
The library SHALL define a catalogue of experiences. Each entry SHALL have a stable kebab-case id, a title, a one-sentence description of what it teaches, the experiment it loads, the action taken once it has loaded (none, play training, or start a hyperparameter sweep), and any build requirement. The catalogue SHALL include at least: the Iris SVM C sweep, MSE vs MAE vs Huber regression, loss curves, and MNIST 3 vs 8. The `--example` binaries SHALL be built from the same catalogue entries.

#### Scenario: Ids are unique and stable
- **WHEN** the catalogue is enumerated
- **THEN** every id is unique, kebab-case, and the four required experiences are present under the ids `iris-svm`, `regression-mse-vs-mae`, `loss-curves`, and `mnist-svm`

#### Scenario: Every available experience loads
- **WHEN** each experience available in the current build is loaded headlessly
- **THEN** its experiment builds without error

#### Scenario: Example and lobby agree
- **WHEN** `cargo run --example iris_svm` is started, and separately `iris-svm` is chosen in the lobby
- **THEN** both load the same experiment and run the same start action

### Requirement: Lobby screen
When the `bevaru` binary starts without arguments, it SHALL show a lobby listing every catalogue entry as a card with its title, description, and requirements. No experiment SHALL be loaded and no training SHALL run while the lobby is shown.

#### Scenario: Lobby on plain start
- **WHEN** a user runs `cargo run`
- **THEN** the lobby is shown with one card per catalogue entry, and no experiment is loaded

#### Scenario: Lobby is idle
- **WHEN** the lobby has been shown for 60 frames
- **THEN** no experiment load has been requested and no chart has been rendered

### Requirement: Choosing an experience
Activating a card SHALL leave the lobby, load that experience's experiment, and run its start action once the experiment has loaded. A loading indicator SHALL be shown until it is ready.

#### Scenario: Pick the Iris sweep
- **WHEN** the user activates the `iris-svm` card
- **THEN** the lobby closes, a loading indicator appears, and once loaded the C sweep starts automatically

#### Scenario: Load failure returns to the lobby
- **WHEN** a chosen experience fails to load (for example the MNIST download fails)
- **THEN** the user is returned to the lobby and the error is shown on that experience's card

### Requirement: Returning to the lobby
While an experience is running, a "Back to lobby" control and the `Esc` key SHALL return to the lobby. Leaving SHALL stop training playback, cancel any in-progress sweep or experiment load, and remove the experience's scene entities, cameras, and chart images, so that choosing another experience starts clean.

#### Scenario: Back to lobby tears down the experience
- **WHEN** a user is mid-sweep in `iris-svm` and presses `Esc`
- **THEN** the lobby is shown, no scene entities or pane cameras remain, the sweep's background tasks are dropped, and no experiment resource remains

#### Scenario: Esc while typing does not leave
- **WHEN** a UI text field has keyboard focus and the user presses `Esc`
- **THEN** the experience keeps running

#### Scenario: Round trip
- **WHEN** a user opens `regression-mse-vs-mae`, returns to the lobby, then opens `loss-curves`
- **THEN** only the loss-curves experiment's panes and data are shown

### Requirement: Availability gating
Experiences whose build requirement is not met SHALL be listed but disabled, with the reason and how to enable them.

#### Scenario: MNIST without the feature
- **WHEN** the binary is built without the `mnist` feature
- **THEN** the MNIST card is visible, cannot be activated, and says to rebuild with `--features mnist`

#### Scenario: MNIST with the feature
- **WHEN** the binary is built with the `mnist` feature
- **THEN** the MNIST card can be activated and notes the one-time ~11 MB download

### Requirement: Command-line selection
The `bevaru` binary SHALL accept an experience id as its first argument and open that experience directly, skipping the lobby; `--list` SHALL print every id with its title and exit. An unknown id SHALL print the valid ids and exit with a non-zero status.

#### Scenario: Deep link
- **WHEN** a user runs `cargo run -- loss-curves`
- **THEN** the loss-curves experience opens without the lobby being shown, and "Back to lobby" still leads to the lobby

#### Scenario: Unknown id
- **WHEN** a user runs `cargo run -- nonsense`
- **THEN** the program prints the valid ids and exits with a non-zero status, without opening a window
