## Why

The lobby is the app's home screen, but it has no way to quit. Users have to close the window or kill the process, and nothing on screen says how. A visible Exit makes the lobby a complete front door. A clean exit (Bevy's `AppExit`) also lets every shutdown hook run, rather than relying on the window manager.

## What Changes

- An **Exit** button on the lobby screen that quits the app cleanly by sending `AppExit::Success`.
- **Ctrl+Q** does the same from the lobby (ignored while a text field has focus).
- `Esc` keeps its current meaning, back to the lobby, and never quits, so one stray key can't close the app.
- If an experience is somehow still active when quitting, the existing on-exit handling stops it first, as it already does when the window closes.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `experience-lobby`: adds a requirement for quitting cleanly from the lobby.

## Impact

- **Code:** `src/lobby.rs` (the button, the shortcut system, and a headless test).
- **Docs:** README key list.
- **Dependencies, APIs, agent manifest:** unchanged (`AppExit` is Bevy's own message).
