## MODIFIED Requirements

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
