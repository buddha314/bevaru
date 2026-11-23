## Why

Quantum machine learning starts by *encoding* classical data into qubit states, and every encoding is a sequence of rotations. On paper the encodings look like near-identical gate lists. On the Bloch sphere they are visibly different shapes: angle encoding traces a meridian, phase encoding the equator, dense angle encoding covers the sphere, and an entangling feature map pulls each qubit's vector *inside* the ball. bevaru already teaches ML ideas as animated 3-D shapes. This extends it to the first idea of quantum ML, for the same audience of students coming from mathematics.

## What Changes

- A new **Bevy-independent quantum module in `bevaru-core`**:
  - small state vectors;
  - gates written as rotation generators, so any gate can be applied *partially*, which is how its rotation is animated;
  - reduced Bloch vectors for any qubit;
  - a catalogue of common **data encodings**, each with a stable kebab-case id, its features and ranges, its circuit, and a formula.
- **The encodings:**
  - basis;
  - angle (RY and RX);
  - phase;
  - dense angle (two features);
  - amplitude (two features);
  - data re-uploading;
  - the two-qubit ZZ feature map of Havlíček et al.
- Two kinds of path for any encoding:
  - the **gate path**: the trajectory of each qubit's Bloch vector while the circuit's gates rotate it, for fixed data;
  - the **encoding locus**: the curve traced as one feature sweeps its range.
- A new custom experience, **`bloch-sphere`** ("Bloch sphere: data encodings"), in a new lobby category, "Quantum computing". It has two modes:
  - **gate by gate**: watch each rotation, with its axis shown;
  - **sweep the data**: watch the encoded state move and leave its locus behind.

  It also has the standard orbit camera, basis-state labels, controls, a thumbnail, and an `--example bloch_sphere` wrapper.
- Docs: a README row and section, and regenerated agent capability docs, which list experiences.

No existing behaviour changes. Nothing is **BREAKING**.

## Capabilities

### New Capabilities
- `quantum-encodings`: the Bevy-independent qubit state, rotation-generator gates, Bloch vectors, the encoding catalogue, and the gate-path and locus computations.
- `bloch-sphere-experience`: the `bloch-sphere` lobby experience that draws one Bloch sphere per qubit and animates encodings in both modes.

### Modified Capabilities
None. The lobby's registry, thumbnail, and clean-up requirements already cover new experiences. The new one must meet them, and the existing leak test is extended to check it.

## Impact

- `crates/bevaru-core`: a new `quantum` module (uses `nalgebra`'s complex type; no new dependencies).
- `src/experiences/`: a new `bloch.rs`, registered in `builtin.rs`, added to the `ExperiencesPlugin` 3-D group, and extended in `lobby_tests.rs`.
- `examples/bloch_sphere.rs` and `assets/thumbnails/bloch-sphere.png`.
- `src/formula.rs`: the perceptron's formula-texture cache moves here to be shared.
- `src/shape_view.rs`: a one-line fix for a latent race (per-system run condition) that the new systems' scheduling exposed.
- `README.md`, `docs/images/bloch_sphere.webp`, `docs/agents/capabilities.*` (regenerated).
