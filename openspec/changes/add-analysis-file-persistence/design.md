## Context

[The graph prototype](../add-3d-graph-prototype/design.md) introduces raw JSON import and computed 3D coordinates. Persistence must capture a usable analysis, including data that originally came from an external file. This change builds on that prototype; it does not require rust-igraph or expose backend-specific serialized types.

## Goals / Non-Goals

**Goals:** Open and save self-contained local analysis documents; resume graph exploration without recomputing coordinates; preserve state on failed operations; make file and unsaved-change status clear.

**Non-Goals:** Supporting every experience in version 1, arbitrary third-party graph formats, image/video exports, cloud synchronization, autosave, serialization of Bevy entities or executable code, and new agent tools. Other analysis kinds can adopt the envelope through separately specified adapters.

## Decisions

### A versioned self-contained JSON document

Use `.bevaru.json` with an envelope containing `format: "bevaru-analysis"`, `version: 1`, `kind: "graph-3d"`, and `payload`. The graph payload embeds ordered node/edge data, all resolved finite XYZ coordinates, layout method/seed/settings, supported computed results, camera orbit target/orientation/distance, and selected node id. Optional source display name is provenance only: opening never follows it. Do not persist machine-specific absolute source paths by default.

Version 1 supports an empty results collection and a per-node degree result; results carry a type and computation settings and must reference existing node ids with finite numeric values. Future result types need explicit schema support. Reject unknown versions, kinds, and result types rather than silently losing information on the next save. Backend method names are informational when reopening a completed layout: the saved geometry must remain readable without that backend installed. Recomputing can report that a backend is unavailable.

Keep serde document types, validation, and conversion to graph types in core. Put actual filesystem I/O and background tasks in the app. Reuse graph topology validation and the prototype's 500-node/2,000-edge limits; enforce a 4 MiB document limit during reading and before saving. Validate finite camera values, positive distance, normalized orientation, and selection/result references. Restore stored scene coordinates without renormalization drift. Round-trip equality concerns semantic values, not JSON whitespace or key ordering.

### Explicit path-based operations

Use egui path fields initially to avoid a new native-dialog dependency. Open Analysis handles analysis documents; the prototype's Load Graph remains the raw-data importer. Save uses the active document path and Save As requests another path; an unsaved analysis routes Save to Save As. Relative paths resolve against the process working directory, displayed in the UI. Opening raw graph JSON through Open Analysis gives guidance to use Load Graph.

Successful open establishes a clean document path. Importing raw data creates a new unsaved document. Data, layout, results, selection, and camera changes increment a document revision and mark it dirty; transient UI changes do not. Open/import/Sample, Back to lobby, Escape, and normal app quit are deferred when dirty until Save, Discard, or Cancel. Save failure preserves the pending operation and dirty state; Cancel leaves the experience active. Integrate this guard before existing lobby/quit teardown rather than trying to restore state after teardown. A forced process termination cannot offer this dialog.

### Atomic background operations and explicit overwrite

Read/validate documents in bounded background work; install a complete validated snapshot atomically only for the latest active session/request. Opening errors leave the current graph, path, and revision intact. Saving captures an immutable revision snapshot; success clears dirty state only if no newer edits exist. Serialize writes per document, prevent pending navigation from racing a save, and keep the app responsive.

Write a uniquely named temporary sibling in the destination directory, flush/sync it, then commit with the platform's atomic rename/replace primitive. A failed precommit leaves the existing destination intact and removes the temporary file. A successful rename is the commit point; report any subsequent durability problem distinctly. Do not truncate the destination or silently fall back to non-atomic replacement. Save As requires explicit confirmation when its destination exists. Save compares the current destination contents with the baseline captured on open/last save and requires explicit overwrite or Save As if it has changed or disappeared. Recheck immediately before commit and document that this detects ordinary external edits but cannot guarantee cross-process locking on every filesystem. Paths with missing parents fail clearly rather than creating an unexpected directory tree.

Stop handling drops task handles and removes document resources and temporary files owned by canceled precommit writes. A write already committed is reported as saved; a canceled open never installs late state. Atomicity and conflict behavior require real temporary-directory tests in addition to injected I/O failures.

## Risks / Trade-offs

- Envelope growth and format drift → Version explicitly and reject unsupported input before state changes.
- View changes frequently mark dirty → Revision tracking is simple and honest; coalesce camera changes per frame and snapshot on Save.
- Filesystem semantics differ → Use and test a narrow persistence helper; report unsupported atomic replacement without weakening the contract.
- Existing escape/quit paths bypass guards → Centralize pending destructive navigation before experience stop; test every route.
- Concurrent external editors → Baseline comparison plus immediate recheck; no promise of portable file locking.

## Migration Plan

Implement after graph types and state are available. Keep raw graph version-1 JSON import intact and distinct from analysis document versioning. No migration is required for existing files. Rollback removes persistence actions without changing raw import. Future document revisions require explicit migrations and fixtures.

## Open Questions

- Which atomic replacement helper best fits the supported platforms? Resolve through a focused implementation spike and failure tests.
- Which additional analysis kind should adopt this format next? Defer until graph round trips work.
