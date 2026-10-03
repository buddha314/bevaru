## MODIFIED Requirements

### Requirement: Bevy plugin entry point
The crate SHALL expose `BevaruPlugin`, which adds all visualization systems, and SHALL also expose its constituent sub-plugins individually so an app can opt into only what it needs. The existing `PlotPngBytes` resource and `RefreshPlotEvent` event SHALL remain available. An app SHALL be able to start the plugin idle, with no experiment loaded until one is requested; in that state the scene, charts, playback, and control panel SHALL do nothing. Without an explicit choice, the plugin SHALL keep its current behaviour of loading `StartupExperiment` or the default experiment at startup.

#### Scenario: Headless use still works
- **WHEN** an app adds only `MinimalPlugins` and `BevaruPlugin`
- **THEN** startup succeeds and `PlotPngBytes` is populated, as with the existing scaffold

#### Scenario: Minimal app
- **WHEN** an app adds `DefaultPlugins` and `BevaruPlugin` and spawns a scene from a dataset and model config
- **THEN** the scene renders with no further setup

#### Scenario: Idle start
- **WHEN** an app adds `BevaruPlugin` configured to start idle and runs 60 frames
- **THEN** no experiment is loaded, no pane camera or scene entity exists, and no chart render has started

## ADDED Requirements

### Requirement: Experiment teardown
Unloading the current experiment SHALL remove everything it created: its scene entities and pane cameras, chart and weight images, pane views, in-flight sweep tasks, and any pending experiment load. Loading a new experiment afterwards SHALL behave exactly as a first load.

#### Scenario: Unload leaves nothing behind
- **WHEN** an experiment with two panes is unloaded
- **THEN** no entity tagged as scene content and no pane camera remains, and the `Experiment` resource no longer exists

#### Scenario: Reload after unload
- **WHEN** an experiment is unloaded and a different one is loaded
- **THEN** its scene matches what a fresh app would show for that experiment
