# Capability Manifest Specification

## Purpose

Define the machine-readable description of bevaru's capabilities, built from the code so it can't fall behind it, and the wire format and schemas agents use to make requests.
## Requirements
### Requirement: Code-derived capability manifest
The library SHALL provide a function that returns a manifest describing bevaru's capabilities, built from the code rather than written by hand. The manifest SHALL describe:
- every loss: id, name, task, whether it can be trained, and its hyperparameters with their valid ranges;
- every model: id, name, task, and allowed losses;
- every dataset choice and view kind, with their parameters;
- every sweep parameter, with the models or losses it applies to;
- every registered experience: id, title, summary, category, kind, and requirements;
- the public messages an app can send to drive bevaru (loading, playback, sweeps, lobby navigation);
- every 3-D loss-shape view: id, family, losses, axis names and ranges, height label, hyperparameters, and caption.

#### Scenario: Every loss is described
- **WHEN** the manifest is built
- **THEN** it contains exactly one entry per `LossKind::ALL` member, and each entry lists the hyperparameters that loss uses with their valid ranges

#### Scenario: Registered experiences appear
- **WHEN** a third-party experience is registered and the manifest is built from that app's registry
- **THEN** the experience appears in the manifest with its id, title, category, kind, and availability

#### Scenario: Every loss-shape view is described
- **WHEN** the manifest is built
- **THEN** it contains one entry per loss-shape view in the catalogue, with that view's axes, ranges, losses, and caption

### Requirement: Completeness is enforced at compile time
The descriptions of losses, models, and sweep parameters SHALL be produced by exhaustive `match` expressions, so adding a variant without describing it fails to compile.

#### Scenario: New loss without a description
- **WHEN** a developer adds a `LossKind` variant and builds without updating the manifest
- **THEN** compilation fails at the manifest's non-exhaustive match

### Requirement: Machine-readable form
The manifest SHALL serialize to JSON with a stable top-level shape and a `schema_version`. Inputs that agents send (experiment specs, trainer configurations, sweep specs, loss evaluations) SHALL have JSON Schemas generated from their Rust types.

#### Scenario: JSON round trip
- **WHEN** the manifest is serialized to JSON and parsed back
- **THEN** the result equals the original manifest

#### Scenario: Schemas accept what the library produces
- **WHEN** each built-in experience's experiment spec is serialized to JSON
- **THEN** it validates against the published experiment-spec schema

