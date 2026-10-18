## 1. Define the document contract

- [ ] 1.1 Integrate against graph data/state from `add-3d-graph-prototype`; preserve its raw graph import contract and document the dependency.
- [ ] 1.2 Add Bevy-free versioned analysis envelope and graph payload types, including stored coordinates, layout settings, degree results, camera, selection, and optional provenance.
- [ ] 1.3 Implement validation for version/kind/result compatibility, topology, references, finite values, camera constraints, and document/graph limits.
- [ ] 1.4 Add semantic round-trip fixtures, unsupported-format tests, malformed/oversized input tests, and restoration without source file or layout backend.

## 2. Implement file operations

- [ ] 2.1 Implement bounded asynchronous reading and validation, immutable save snapshots, session/request generations, serialized writes, and revision-aware completion.
- [ ] 2.2 Select and verify a platform-appropriate atomic persistence helper; implement temporary sibling write/sync/commit, precommit cleanup, and distinct postcommit durability reporting.
- [ ] 2.3 Implement destination baselines, external modification/deletion detection, overwrite confirmation, and Save As conflict choices.
- [ ] 2.4 Test actual temporary-directory round trips and injected read/write/sync/rename errors; assert previous destination bytes and active analysis survive precommit failures.

## 3. Connect analysis state and controls

- [ ] 3.1 Add Open Analysis, Save, Save As, path fields with working-directory guidance, and visible path/busy/error/dirty states.
- [ ] 3.2 Capture and restore graph data, exact scene positions, layout settings, results, camera, and selection without rerunning layout or introducing normalization drift.
- [ ] 3.3 Track persisted changes with revisions; make raw import unsaved, successful open clean, and save completion clean only for the saved revision.
- [ ] 3.4 Guard Open/import/Sample, Back, Escape, and normal quit with Save/Discard/Cancel before teardown; preserve state and pending action when saving fails.
- [ ] 3.5 Clean up document resources and task handles on stop, prevent stale open completion, and resolve pending writes safely before leaving.

## 4. Validate integration and document the workflow

- [ ] 4.1 Add headless tests for canceled dialogs, dirty navigation through every exit path, edits during save, superseded opens, and repeated experience cleanup.
- [ ] 4.2 Document the file schema, supported results, limits, raw-import distinction, sample workflow, errors, and filesystem/conflict limitations in project documentation; link it from the README.
- [ ] 4.3 Run a visual smoke test covering import, edit, Save As, quit/reopen, missing original source, overwrite conflict, and write failure.
- [ ] 4.4 Regenerate agent docs if capability descriptions or served docs change; run `scripts/check.sh --tests` and strict OpenSpec validation and record results.
