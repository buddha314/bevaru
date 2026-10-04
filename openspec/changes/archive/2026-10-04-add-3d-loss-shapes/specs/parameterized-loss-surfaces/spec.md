## ADDED Requirements

### Requirement: Lobby experience
The parameter-space objective surfaces SHALL also be a lobby experience, "Training objective in 3D", in the *Loss functions* category, with an embedded thumbnail. Its controls, camera, and live updates SHALL match the `loss_surface` example. The example SHALL open the experience directly, so the two cannot drift. Leaving the experience SHALL leave nothing behind.

#### Scenario: Open from the lobby
- **WHEN** a user activates the "Training objective in 3D" card
- **THEN** the hinge objective surface is shown with its loss selector, parameter sliders, and orbit controls

#### Scenario: Example and lobby agree
- **WHEN** `cargo run --example loss_surface` is started, and separately the card is chosen in the lobby
- **THEN** both start the same registered experience

#### Scenario: Clean exit
- **WHEN** the experience is entered and left ten times
- **THEN** entity, mesh, material, and image counts return to the lobby baseline
