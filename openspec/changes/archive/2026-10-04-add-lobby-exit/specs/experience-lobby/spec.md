## ADDED Requirements

### Requirement: Exiting from the lobby
The lobby screen SHALL offer a visible Exit control that quits the application cleanly, by sending Bevy's `AppExit::Success` rather than terminating the process. `Ctrl+Q` SHALL do the same while the lobby is shown, unless a UI text field has keyboard focus. `Esc` SHALL NOT quit the application. If an experience is active when the app exits, it SHALL be stopped first, through the existing exit path.

#### Scenario: Exit button
- **WHEN** the user clicks Exit in the lobby
- **THEN** the app sends `AppExit::Success` and shuts down normally, with exit status 0

#### Scenario: Keyboard shortcut
- **WHEN** the user presses Ctrl+Q in the lobby
- **THEN** the app exits as if Exit had been clicked

#### Scenario: Esc does not quit
- **WHEN** the user presses Esc in the lobby
- **THEN** the app keeps running
