## ADDED Requirements

### Requirement: Versioned analysis document
The application SHALL support self-contained JSON analysis documents identified by `format: "bevaru-analysis"`, `version: 1`, and `kind: "graph-3d"`, with suggested extension `.bevaru.json`. The payload SHALL preserve graph data, resolved XYZ positions, layout settings, supported computed results, camera state, and selection. Source metadata SHALL be optional and SHALL NOT be dereferenced when opening. Initial results SHALL support an empty collection and per-node degree values with computation settings. Unknown versions, kinds, and result types SHALL be rejected without changing current state.

#### Scenario: Resume without the original source
- **WHEN** a saved graph analysis is opened after its original source file is removed and its layout backend is unavailable
- **THEN** graph data, positions, results, camera, and selection are restored without reading that source or recomputing layout

#### Scenario: Unsupported document
- **WHEN** a document has an unsupported format, version, analysis kind, or result type
- **THEN** an actionable compatibility error is shown and the current analysis remains intact

### Requirement: Bounded validated opening
Open Analysis SHALL read and validate a local document off the UI thread before atomic installation. It SHALL enforce a 4 MiB read limit, 500 nodes, and 2,000 edges, reuse graph topology validation, and validate finite coordinates/results, valid camera values, and node references. Relative paths SHALL resolve against the displayed process working directory. Failed, canceled, or superseded opens SHALL preserve the active document and SHALL NOT install stale results.

#### Scenario: Invalid or unreadable file
- **WHEN** opening fails because of an unreadable path, malformed JSON, invalid references, invalid values, or exceeded limits
- **THEN** the error identifies the condition and leaves current data, path, and dirty state unchanged

#### Scenario: Raw graph is opened as an analysis
- **WHEN** the user selects a raw graph JSON file through Open Analysis
- **THEN** the app explains that Load Graph imports raw graph data without replacing the current analysis

### Requirement: Save and Save As
The experience SHALL expose Open Analysis, Save, and Save As with visible active path, busy, error, and dirty state. Save SHALL use the active document path or request Save As for an unsaved analysis. Save As SHALL request a destination and require explicit confirmation before replacing an existing file. Writes SHALL use an immutable snapshot and SHALL preserve semantic values across save/open round trips without coordinate normalization drift. The writer SHALL enforce document size limits before committing.

#### Scenario: Save a new analysis
- **WHEN** a raw graph has been imported and the user invokes Save
- **THEN** a destination is requested and successful saving establishes the active document path

#### Scenario: Save while edits continue
- **WHEN** the analysis changes after a save snapshot is taken
- **THEN** that snapshot is saved consistently and newer changes remain marked unsaved

#### Scenario: Save As is canceled
- **WHEN** the user cancels destination selection or overwrite confirmation
- **THEN** no destination is modified and the active document path and dirty state remain unchanged

### Requirement: Atomic persistence and external change handling
Saving SHALL write and sync a temporary sibling before atomic destination commit. Precommit failure SHALL preserve the previous destination, remove the temporary file, report the error, and retain unsaved state. Save SHALL detect destination changes or deletion relative to its last loaded/saved contents, recheck before commit, and offer explicit overwrite, Save As, or Cancel. The implementation SHALL document filesystem/concurrent-writer limits and SHALL NOT silently use a non-atomic replacement.

#### Scenario: Write fails before commit
- **WHEN** saving encounters a permission, disk-space, serialization, or atomic-replacement error before commit
- **THEN** existing destination bytes remain unchanged, temporary files are removed, and the analysis remains unsaved

#### Scenario: Another program edits the file
- **WHEN** destination contents differ from the baseline when Save checks them
- **THEN** the app requires an explicit conflict choice before replacing the external changes

### Requirement: Unsaved change guard
Persisted data, layout, results, camera, and selection changes SHALL mark the document dirty. Successful open SHALL establish clean state and raw import SHALL create an unsaved document. Replacing an analysis, returning to the lobby, Escape, and normal app quit SHALL offer Save, Discard, or Cancel before discarding dirty state. Failed saving SHALL NOT authorize the pending discard.

#### Scenario: Cancel leaving
- **WHEN** the user presses Escape with unsaved changes and chooses Cancel
- **THEN** the current experience and analysis remain active with no state discarded

#### Scenario: Save before quitting fails
- **WHEN** the user chooses Save in the unsaved-changes prompt during normal quit and saving fails
- **THEN** the application remains open with the analysis intact and allows retry, Discard, or Cancel

### Requirement: File task lifecycle
File tasks SHALL be bounded, associated with the active session/request and snapshot revision, and cleaned up on experience stop. Canceled open results SHALL never recreate state. Pending writes SHALL finish or cancel safely before an authorized stop, and canceled precommit writes SHALL remove their temporary files. Repeated experience visits SHALL retain no file-operation resources or handles.

#### Scenario: Superseded open completes late
- **WHEN** an older open finishes after a newer request or session has taken over
- **THEN** its results are discarded without changing the active document
