## Context

The current experience registry supports custom scenes, and `src/orbit.rs` supplies orbit/pan/zoom behavior. `src/experiences/layout.rs` reserves camera space around egui panels. Existing loss-shape scenes demonstrate headless asset setup and lifecycle handling. Graph data and layout mathematics belong in `bevaru-core`; rendering and input belong in the plugin.

### Initial tool survey

Sources inspected on 2026-10-18. These observations guide a feasibility spike; no dependency has been selected or benchmarked.

| Candidate | Role and evidence | Fit and remaining work |
| --- | --- | --- |
| [rust-igraph](https://github.com/Totoro-jam/rust-igraph), [API](https://docs.rs/rust-igraph/latest/rust_igraph/) | Rust graph analysis, import, and layout library; upstream advertises 3D layouts. Published API inspected identifies 0.7.0 and GPL-2.0-or-later. | Priority candidate for layout. Compile a pinned-version spike to verify actual 3D entry points, initialization/seed control, error behavior, and build cost. No rendering layer is supplied for bevaru. |
| [petgraph](https://docs.rs/petgraph/latest/petgraph/), [manifest](https://github.com/petgraph/petgraph/blob/master/Cargo.toml) | Rust graph representations, algorithms, and DOT export. | Candidate storage/analysis layer; a separate 3D layout implementation would be needed. Verify version/license metadata before selection. |
| [Graphviz](https://graphviz.org/docs/attrs/dim/) | `neato`, `fdp`, and `sfdp` support layout dimension configuration. | Useful offline comparison; inspect coordinate output separately. External executable and packaging make it less convenient for an embedded prototype. |
| Existing Bevy scene + orbit support | Already available in this repository. | Preferred renderer and camera interaction; no additional graph-viewer framework or Bevy-version compatibility layer. |
| Small local 3D force layout | Proposed Bevy-free implementation, not an evaluated external tool. | Fallback if external layouts fail the selection criteria; bounded quadratic repulsion is acceptable only at the stated prototype limits. |

The comparison must separate topology, import, coordinate generation, and display. A graph library alone does not provide an interactive 3D viewer. Upstream C igraph documentation is background only, not proof of Rust API parity.

## Goals / Non-Goals

**Goals:** Identify a practical tool combination; verify rust-igraph with an isolated executable spike; load a small local graph; show actual XYZ geometry and topology; inspect nodes; preserve clean experience teardown.

**Non-Goals:** A general graph editor, million-node rendering, live graph streaming, training a graph neural network, all upstream file formats, remote file access, and new graph-specific MCP/BRP tools. GraphML/GML/edge-list adapters can follow after the JSON path works.

## Decisions

### 1. Make backend selection an explicit first deliverable

Run the rust-igraph spike outside the main workspace dependency graph, recording an exact crate version or commit, toolchain, command, license metadata, API calls, output, and elapsed time. Test a tetrahedron, a disconnected graph, and isolated vertices; require one finite XYZ position per node and reproducible output within a documented tolerance. Measure 100- and 500-node graphs with recorded edge counts and iteration budget on a named machine. Do not infer Rust layout support from the C implementation.

Record a decision table covering API maturity, 3D support, determinism, build footprint, platform/toolchain fit, license, and measured runtime. rust-igraph is the first candidate, not a mandatory dependency. Its declared license differs from this MIT workspace: document the distribution implications as an unresolved selection consideration rather than changing project licensing in this prototype. If it cannot be selected, use an independently implemented bounded 3D force layout in core and preserve the spike results. Merely marking a dependency optional does not resolve its license considerations. Do not copy upstream implementation into the fallback.

### 2. Define a minimal interchange format independent of the backend

Use JSON with `version: 1`, `directed: false`, `nodes`, and `edges`. Each node has a unique nonempty string `id`, optional `label`, and optional `[x,y,z]` `position`. Each edge has string `source` and `target`. Positions must be present for every node or absent for every node. Preserve input order through a stable id-to-index map. Supplied coordinates retain relative geometry through centering and uniform scaling.

```json
{
  "version": 1,
  "directed": false,
  "nodes": [
    {"id": "a", "label": "Alice", "position": [0, 0, 0]},
    {"id": "b", "position": [1, 0, 1]}
  ],
  "edges": [{"source": "a", "target": "b"}]
}
```

Prototype bounds: at most 2 MiB of input, 500 nodes, 2,000 edges, and 200 layout iterations. Enforce byte limits while reading, before JSON allocation. Reject unsupported versions, directed graphs, duplicate ids, missing endpoints, self-loops, duplicate undirected edges, partial positions, and nonfinite coordinates with actionable errors. Empty and singleton graphs are valid. Disconnected components and isolated nodes are preserved. A simple undirected contract avoids implying support for arrowheads, parallel-edge routing, or self-loop rendering.

### 3. Use one custom experience and an atomic load pipeline

Register `graph-3d` in the Graphs category, reachable from the lobby and `cargo run -- graph-3d`. Start with an embedded noncoplanar sample so the demo needs no network or working-directory assumptions. Provide a local path text field plus Load and Sample controls; relative paths resolve against the process working directory and the UI explains that convention.

Read, validate, and lay out off the UI thread. Retain the displayed graph until a replacement succeeds. Show loading progress as a state indicator, source, counts, layout method, and errors. Use a request generation counter so only the newest active request can install results. Bound worker concurrency and cooperatively cancel between force-layout iterations; a backend without cancellation must have bounded work and discard stale completion. Leaving removes task handles and prevents late results from recreating scene state.

### 4. Reuse the native scene and interaction primitives

Render nodes as shared sphere meshes and undirected edges as depth-tested line gizmos. Keep graph bounds and a stable node id component for inspection. Reuse `OrbitRig` and side-inset viewport handling; Fit/reset frames the current graph with sane empty/singleton/coincident-coordinate behavior. Ray/sphere picking selects the nearest visible node under the cursor and displays its id, label, and degree; clicks over egui do not pick nodes or move the camera. Highlight the selected node and incident edges. Use a list selector as an accessible alternative to picking.

Scope every graph entity, camera, and light with `ExperienceEntity`; remove graph resources, tasks, selection, panel inset changes, and owned asset handles on `ExperienceStopped`. Verify shared mesh/material reuse and zero retained assets after unload. Do not cache graph handles across experiences in `Local` state.

### 5. Verify behavior headlessly and appearance manually

Unit-test parsing and layout invariants in core. Use the existing headless scene harness for replacement, stale result rejection, node inspection, camera fitting, and ten entry/exit cycles. Cover default startup and registry discovery. The implementation must add capability descriptions and regenerate agent references with `cargo run -p bevaru-mcp -- gen-docs`; generated files are never edited manually. A visual smoke test exercises the embedded graph and a file without positions, checks depth from several angles, and records a screenshot plus command and machine information. Runtime measurements are evidence, not an unmeasured frame-rate guarantee.

## Risks / Trade-offs

- New backend API or dependency churn → Pin the evaluated version and keep backend types behind core-owned data structures.
- Layout time grows quadratically → Enforce the input/iteration limits, use background work, and record timings before increasing scope.
- Bad or degenerate coordinates break fitting → Validate finiteness, use robust centering/scaling, and cover coincident/empty inputs.
- Loading or UI state leaks across visits → Generation checks, stop cleanup, and repeated headless round trips.
- Dense graphs obscure topology → Keep the prototype bounded, offer selection highlighting, and defer advanced edge routing.

## Migration Plan

Add the experience without changing existing ids, startup behavior, or saved data. Integrate a backend only after the selection record exists. Rollback removes the new experience, its core module, fixtures, and any new dependencies, then regenerates agent docs. No existing data migration is needed.

## Open Questions

- Does the pinned rust-igraph 3D API support deterministic initialization and acceptable bounded execution? Resolve with the spike.
- Is its dependency license suitable for the intended distribution? Record the selection outcome; the fallback keeps the prototype actionable without requiring that dependency.
- Are the proposed limits useful for target datasets? Start with these bounds and revisit only with measurements and concrete examples.
