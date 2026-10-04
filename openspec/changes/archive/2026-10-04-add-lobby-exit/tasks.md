## 1. Implementation

- [x] 1.1 Add the Exit button to the lobby header, writing `AppExit::Success`
- [x] 1.2 Add the `exit_shortcut` system (Ctrl+Q, lobby only, ignored while egui wants the keyboard)
- [x] 1.3 Headless tests: Ctrl+Q in the lobby sends `AppExit`; Esc in the lobby does not
- [x] 1.4 README: add Ctrl+Q to the key list and mention Exit

## 2. Verification

- [x] 2.1 `scripts/check.sh` compiles cleanly; run the new tests *(the three keyboard tests pass, including the existing Esc test)*
- [x] 2.2 In a real window, check the button is visible, and that the app exits with status 0 *(screenshot shows Exit at the top right of the lobby; a real run exits with status 0 through `AppExit`, the path the button uses; the click itself is not automated)*
