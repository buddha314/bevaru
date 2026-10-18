## ADDED Requirements

### Requirement: Graph tool comparison
The change SHALL record a sourced comparison of rust-igraph, petgraph, Graphviz, and existing Bevy rendering support. It SHALL distinguish storage, import, 3D layout, rendering, and interaction roles, record source inspection dates, and distinguish advertised capabilities from verified results. It SHALL evaluate version/toolchain fit, determinism, packaging, licensing, and runtime evidence before selecting a backend.

#### Scenario: A renderer and layout backend are selected
- **WHEN** the evaluation is completed
- **THEN** a decision record names the rendering approach and layout backend, gives reasons and limitations, and links the inspected primary sources

### Requirement: Reproducible rust-igraph feasibility spike
The evaluation SHALL attempt a standalone, pinned rust-igraph spike that constructs graphs and obtains 3D coordinates. It SHALL record build/run commands, dependency identity, toolchain, API usage, license metadata, and success or concrete blockers. It SHALL check tetrahedron, disconnected, and isolated-node cases and measure 100- and 500-node cases with recorded edge counts, machine, and work budget. It SHALL report coordinate completeness, finiteness, and repeated-run determinism within a stated tolerance.

#### Scenario: Candidate satisfies the required checks
- **WHEN** the spike builds and produces valid reproducible 3D coordinates
- **THEN** the evidence includes the commands, checked invariants, timing measurements, and remaining integration considerations

#### Scenario: Candidate cannot be integrated
- **WHEN** an API, packaging, performance, or license consideration prevents selection
- **THEN** the record describes the blocker and identifies an independently implemented bounded core layout as the prototype fallback without adding rust-igraph to the main dependency graph
