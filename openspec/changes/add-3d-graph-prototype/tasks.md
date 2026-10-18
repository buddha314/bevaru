## 1. Evaluate tools and select the layout backend

- [ ] 1.1 Expand the sourced comparison in `design.md` with pinned candidate versions, license metadata, toolchain/platform fit, and separate storage/import/layout/rendering roles.
- [ ] 1.2 Create and run a standalone pinned rust-igraph 3D layout spike outside the main workspace dependency graph; record exact APIs, commands, toolchain, output, or concrete blockers.
- [ ] 1.3 Verify coordinate completeness, finiteness, noncoplanar tetrahedron layout, disconnected/isolated nodes, and repeated-run tolerance; measure 100- and 500-node cases with graph size, budget, and machine recorded.
- [ ] 1.4 Record the backend decision, including dependency license considerations and fallback rationale; add a main-workspace dependency only if selected. Keep core-owned types independent of backend types.

## 2. Implement Bevy-free graph data and layout

- [ ] 2.1 Add version-1 JSON graph types, stable id mapping, byte/node/edge bounds, and validation errors in `bevaru-core` for the input contract in `specs/graph-3d/spec.md`.
- [ ] 2.2 Implement supplied-coordinate normalization and the selected deterministic bounded 3D layout adapter, or an independent local force-layout fallback with cancellation checks.
- [ ] 2.3 Add fixtures and core tests for complete/absent positions, invalid fields and topology, limits, empty/singleton/coincident inputs, disconnected nodes, finiteness, determinism, and tetrahedron volume.

## 3. Add the graph experience and loading pipeline

- [ ] 3.1 Register the `graph-3d` custom experience in the Graphs category and wire its plugin into the existing headless-compatible experience setup.
- [ ] 3.2 Embed a noncoplanar sample; implement local path, Load, and Sample controls, working-directory guidance, counts, backend name, loading state, and actionable errors.
- [ ] 3.3 Implement bounded background read/validate/layout work with byte-limited reading, session/request generation checks, atomic successful replacement, preserved graph on failure, and cleared selection on success.
- [ ] 3.4 Test unreadable files, validation failure, successful replacement, superseded requests, and leaving during pending work using deterministic task completion control.

## 4. Render and inspect the 3D graph

- [ ] 4.1 Render shared sphere nodes and depth-tested edge gizmos, adding graph-owned camera/light entities and stable node identifiers.
- [ ] 4.2 Integrate `OrbitRig`, side-inset viewport handling, and Fit/reset for normal and degenerate graph bounds; ignore scene input over egui.
- [ ] 4.3 Add nearest-node viewport picking, list selection, id/label/degree inspection, and selected-node/incident-edge highlighting.
- [ ] 4.4 Add headless assertions for topology/position-to-scene mapping, picking, selection clearing, UI input exclusion, and camera fit behavior.

## 5. Complete lifecycle and discovery integration

- [ ] 5.1 Scope all entities with `ExperienceEntity`; remove graph resources, selection, background task handles, viewport overrides, and owned asset handles on stop and reject late completions.
- [ ] 5.2 Extend ten-round-trip leak tests with graph replacements and failures; verify entity/asset/texture baselines and switching into another experience.
- [ ] 5.3 Verify lobby discovery, `--list`, direct `graph-3d` selection, and unchanged default lobby startup.
- [ ] 5.4 Generate and embed a thumbnail with a documented reproducible command; update agent capability descriptions and regenerate references with `cargo run -p bevaru-mcp -- gen-docs`.

## 6. Validate and document the prototype

- [ ] 6.1 Document the JSON contract, limits, backend decision and evidence, startup command, local-file workflow, controls, and unsupported graph features in a normal project documentation page; link it from the README.
- [ ] 6.2 Run a visual smoke test of the embedded graph and a position-free local file; record commands, machine, timing evidence, and a screenshot demonstrating 3D depth and inspection.
- [ ] 6.3 Run `scripts/check.sh --tests` and `openspec validate add-3d-graph-prototype --strict`; resolve failures and record results.
