## Context

The lobby (`src/lobby.rs`) has back-navigation (`Esc`, "◀ Lobby") and an on-exit handler (`leave_on_exit`, which runs in `Last` on `AppExit`). It has no way to quit.

## Goals / Non-Goals

**Goals:** a discoverable, clean quit from the lobby, plus a keyboard shortcut.

**Non-Goals:** quitting from inside experiences (they go back to the lobby first); a confirmation dialog (nothing in the lobby can be lost).

## Decisions

- **Send `AppExit::Success`, not `std::process::exit`.** Bevy's normal shutdown then runs, including `leave_on_exit`, which stops any active experience and despawns its entities.
- **The button sits at the top right, next to the title**, styled like the other lobby controls, with a hover hint naming Ctrl+Q.
- **Ctrl+Q, not Esc or Q alone.** Esc already means "back", so reusing it in the lobby would make one extra press quit the app. A bare Q would quit while typing elsewhere. Ctrl+Q is the common desktop convention. The shortcut is ignored while egui wants keyboard input, like `Esc` handling.
- **Testable headlessly.** The shortcut is a plain system (`exit_shortcut`), testable with `ButtonInput<KeyCode>`, like the existing `Esc` test. The button only calls the same writer.

## Risks / Trade-offs

- **[Accidental quit]** → There's no confirmation, which is acceptable: the lobby holds no unsaved state, and leaving an experience already cleaned it up.
