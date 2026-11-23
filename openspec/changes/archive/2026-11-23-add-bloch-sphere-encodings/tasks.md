## 1. Core: qubits and encodings (`bevaru-core::quantum`)

- [x] 1.1 Add the `quantum` module: state vector for n ≤ 2 qubits, starting at |0…0⟩, and the reduced Bloch vector of any qubit
- [x] 1.2 Gates as generators: single-qubit a·σ (RX, RY, RZ, H) with partial application exp(−i·s·G), Bloch axis and angle; and the Z⊗Z entangler
- [x] 1.3 The encoding catalogue (`basis`, `angle-ry`, `angle-rx`, `phase`, `dense-angle`, `amplitude`, `reuploading`, `zz-feature-map`), with ids, names, formulas, features, and circuits
- [x] 1.4 Gate paths and encoding loci
- [x] 1.5 Unit tests for every scenario in `specs/quantum-encodings`

## 2. Experience (`src/experiences/bloch.rs`)

- [x] 2.1 Plugin, controls and animation resources, start and stop observers; register `bloch-sphere` in `builtin.rs` under "Quantum computing"; add to the 3-D group in `ExperiencesPlugin`
- [x] 2.2 Scene: per-qubit translucent sphere, axes, equator and meridians (gizmo group), arrow meshes, lights, orbit camera
- [x] 2.3 Gate-by-gate animation: progress along the gate path, trail, rotation axis, current gate label
- [x] 2.4 Sweep animation: progress along the locus, faint full locus and bright traced part
- [x] 2.5 egui controls, and projected basis-state labels
- [x] 2.6 `examples/bloch_sphere.rs`

## 3. Tests, docs, assets

- [x] 3.1 Headless tests: start shows one sphere at |0⟩; switching to `zz-feature-map` shows two; playback reaches the encoded state; pause holds; add the resources to `assert_clean` and the id to the round-trip test
- [x] 3.2 Update the expected built-in ids in the registry test
- [x] 3.3 README: experience table row, run command, and a short section
- [x] 3.4 Regenerate agent docs (`cargo run -p bevaru-mcp -- gen-docs`)
- [x] 3.5 Thumbnail with `scripts/thumbnails.sh bloch-sphere`
- [x] 3.6 `scripts/check.sh --tests` passes
