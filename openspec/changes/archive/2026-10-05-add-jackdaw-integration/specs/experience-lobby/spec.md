## ADDED Requirements

### Requirement: Lobby layout overrides
The lobby SHALL apply `LobbyEntry` overrides present in the world. An entry names an experience id and MAY set:
- its order;
- its title, summary, and category;
- whether it is hidden.

Overridden experiences SHALL be ordered by `order`. Those without an entry SHALL follow in registration order. A hidden experience SHALL NOT appear as a card, but SHALL still start from the command line, the examples, and remote control, and SHALL stay in `--list`. An entry naming an unknown experience SHALL be ignored with a warning. With no `LobbyEntry` in the world, the lobby SHALL be unchanged.

#### Scenario: Reorder and retitle
- **WHEN** entries give `loss-shapes` order 0 with title "Shapes of loss", and `iris-svm` order 1
- **THEN** the lobby lists "Shapes of loss" before the Iris card, and both before experiences without entries

#### Scenario: Move to another category
- **WHEN** an entry sets `perceptron`'s category to "Loss functions"
- **THEN** its card appears under "Loss functions" and the *Diagrams* heading disappears if it is now empty

#### Scenario: Hidden but reachable
- **WHEN** an entry hides `sigmoid`
- **THEN** the lobby shows no Sigmoid card, and starting `sigmoid` by id still works

#### Scenario: No entries, no change
- **WHEN** no `LobbyEntry` exists
- **THEN** the lobby's categories, order, titles, and summaries are exactly those of the registry

#### Scenario: Unknown id
- **WHEN** an entry names `no-such-experience`
- **THEN** it is ignored, and one warning names the id
