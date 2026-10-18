## ADDED Requirements

### Requirement: Discoverable graph experience
The app SHALL register a custom experience with stable id `graph-3d` in the Graphs category, include a reproducible embedded thumbnail, and support existing lobby and command-line selection. Entering SHALL show an embedded noncoplanar sample independently of the working directory or network. Default startup SHALL continue to open the lobby.

#### Scenario: Start the prototype directly
- **WHEN** the user runs `cargo run -- graph-3d`
- **THEN** the embedded sample opens in a 3D viewport with source and node/edge counts visible

### Requirement: Validated local graph input
The experience SHALL accept a local JSON path through a path field and Load action, and provide a Sample action. The version-1 schema SHALL contain `directed: false`, nodes with unique nonempty string ids and optional labels and XYZ positions, and edges referencing node ids by `source` and `target`. Positions SHALL be supplied for all nodes or none. Relative paths SHALL resolve against the process working directory, as explained in the UI. The parser SHALL preserve isolated nodes and input node order.

#### Scenario: Load a graph with positions
- **WHEN** the user loads a valid JSON file containing complete finite positions
- **THEN** the displayed graph preserves its topology, labels, and relative XYZ geometry after uniform normalization

#### Scenario: Load invalid or unsupported input
- **WHEN** input has invalid JSON, unsupported version, directed edges, duplicate ids, unknown endpoints, self-loops, duplicate undirected edges, partial positions, or nonfinite coordinates
- **THEN** the experience reports the offending condition and keeps the previously displayed graph intact

#### Scenario: Input exceeds prototype bounds
- **WHEN** input exceeds 2 MiB, 500 nodes, or 2,000 edges
- **THEN** loading stops with a message naming the limit before layout or scene creation, and byte limits are enforced during file reading

#### Scenario: Path cannot be read
- **WHEN** the selected path is missing or unreadable
- **THEN** the experience shows an actionable file error without replacing the current graph

### Requirement: Bounded three-dimensional layout
For graphs without positions the experience SHALL compute one finite XYZ position per node through the selected backend with deterministic initialization and at most 200 iterations or an equivalently documented bounded backend work budget. It SHALL report the layout method and preserve disconnected components and isolated nodes. The tetrahedron fixture SHALL have nonzero extent in all three axes and nonzero tetrahedral volume within documented tolerances.

#### Scenario: Compute and repeat layout
- **WHEN** the same ordered position-free graph is loaded twice with identical settings
- **THEN** both layouts have matching finite coordinates within a documented tolerance and preserve every node and edge

#### Scenario: Degenerate graph
- **WHEN** an empty graph, singleton, disconnected graph, or graph with coincident supplied positions is loaded
- **THEN** layout and camera fitting complete without division by zero, invalid transforms, or lost nodes, and empty input shows an empty-state message

### Requirement: Interactive graph inspection
The experience SHALL render graph nodes and undirected edges with depth in a perspective 3D viewport and provide orbit, pan, zoom, and Fit/reset controls. The user SHALL select a node by viewport picking or a list selector and see its id, label, and degree, with the selected node and incident edges highlighted. Pointer interaction over UI SHALL not operate the scene.

#### Scenario: Explore and inspect a node
- **WHEN** the user rotates and zooms the sample and selects a node
- **THEN** perspective and occlusion reflect the graph's XYZ positions and the inspector shows that node's metadata and degree

#### Scenario: Restore camera framing
- **WHEN** the user pans away and activates Fit/reset
- **THEN** the camera frames the entire current graph within the viewport left available by the controls

### Requirement: Responsive atomic replacement
File parsing and layout SHALL run off the UI thread with bounded worker concurrency and a visible loading state. A successful latest request SHALL atomically replace the graph and clear stale selection; an unsuccessful request SHALL preserve the current graph. Only results belonging to the current experience session and latest request SHALL be applied.

#### Scenario: New request overtakes an old request
- **WHEN** a second load is requested before the first finishes
- **THEN** the older result cannot replace the second request's graph or overwrite its loading/error state

### Requirement: Complete graph experience cleanup
The experience SHALL mark scene entities with `ExperienceEntity`, remove its resources and task handles on `ExperienceStopped`, restore shared viewport state, and release owned assets. Late task results SHALL not recreate graph state after leaving. Ten headless entry/exit cycles SHALL return entity, mesh, material, image, and registered texture counts to the initial lobby baseline.

#### Scenario: Leave while loading
- **WHEN** the user leaves `graph-3d` with a request in progress
- **THEN** the lobby becomes usable immediately and completion of that request installs no entities, resources, or textures

#### Scenario: Repeated visits
- **WHEN** the graph experience is entered and left ten times, including successful and failed replacements
- **THEN** counts return to the lobby baseline and opening another experience shows no graph state
