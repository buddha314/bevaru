## Why

Tracking issue: [#31 — Load and save resumable analysis files](https://github.com/buddha314/bevaru/issues/31).

Loading a graph for one viewing session does not let users preserve an analysis and return to it later. bevaru needs a documented file workflow that retains input data, computed positions, analysis settings, and results across sessions.

## What Changes

- Add a versioned local analysis document, initially supporting the `graph-3d` experience with an extensible analysis-kind discriminator.
- Provide Open, Save, and Save As actions, with clear path, busy, error, and unsaved-change states.
- Save self-contained graph data, resolved XYZ positions, layout settings, supported analysis results, and view/selection state so reopening does not require the source file or rerunning layout.
- Validate before replacing active state; write files atomically and handle overwrite conflicts and unsaved changes explicitly.
- Document the file contract and add round-trip, failure, concurrency, and cleanup coverage.

## Capabilities

### New Capabilities

- `analysis-files`: Versioned analysis documents, local open/save workflows, restoration, and failure handling.

### Modified Capabilities

None. This adds persistence alongside existing experience behavior and the separately proposed graph loader.

## Impact

Depends on the graph data/scene work in [add-3d-graph-prototype](../add-3d-graph-prototype/proposal.md). That change owns raw graph JSON import and layout; this change owns resumable analysis documents and writes. Planned code spans Bevy-free document types and validation in `crates/bevaru-core`, filesystem orchestration and UI in the plugin, graph state adapters, fixtures, and documentation. Other analysis kinds, CSV/GraphML conversion, image export, cloud storage, and automatic saving are deferred. No existing file format or experience id changes.
