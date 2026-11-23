## Context

bevaru's experiences are Bevy plugins registered in a lobby. The 3-D ones (`loss-shapes`, `loss-surface`, `perceptron`) share an orbit camera with Z up, draw with built-in primitives and gizmos, and label points by projecting them into egui. Math lives in `bevaru-core`, which must not depend on Bevy. This change adds the first quantum topic. The audience is students coming from mathematics; the companion material is a set of Jupyter notebooks on exciton Hamiltonians and VQE.

## Goals / Non-Goals

**Goals:**
- Make the common data encodings visible as shapes: meridian, equator, covering the sphere, curves, and vectors shrinking into the ball.
- Animate *rotations*, not just end states. Each gate is a rotation the student can watch, with its axis.
- Keep the math exact, testable, and Bevy-free.

**Non-Goals:**
- A general quantum circuit simulator or circuit editor. The catalogue is fixed and small.
- More than two qubits, or noise and mixed-state channels beyond the reduced states that entanglement produces.
- Agent-API tools for encodings (MCP or BRP). The experience appears in the manifest like any other, but there is no encoding tool yet.
- Importing trajectories from the notebooks. That is a possible follow-up.

## Decisions

**Gates as generators, not matrices.** Every gate is stored as its generator, so a partial application exp(−i·s·G) is exact and animation is just s running from 0 to 1. Single-qubit generators are a·σ, with closed form exp(−i a·σ) = cos|a| I − i sin|a| (â·σ). This handles non-commuting cases such as Hadamard ((x+z)/√2) without a matrix exponential. The two-qubit entangler is diagonal (c·Z⊗Z), so it is a phase per basis state. *Alternative:* store 2×2 or 4×4 unitaries and interpolate. Rejected: interpolating unitaries is not a rotation path and needs a matrix logarithm.

**State vectors with reduced Bloch vectors.** One code path for one and two qubits: a 2ⁿ complex vector, gates applied per qubit, and Bloch vectors from the reduced density matrix. This is what shows entanglement: the ZZ feature map's vectors shrink inside the sphere. Complex numbers come from `nalgebra::Complex`, already a dependency.

**Global phase is ignored.** Phase gates are implemented as RZ, and X as RX(π). Both are equal up to a global phase, which the Bloch sphere cannot show. The formula text uses the textbook gate names.

**Two precomputed paths per configuration.** When the encoding, features, or mode change, the experience recomputes:
- the gate path, with a fixed number of samples per gate;
- the locus, with a fixed number of samples across the range.

Each frame then only indexes or interpolates by time. Both are cheap (at most a few thousand 4-element state updates), so they run synchronously, not on a task.

**Drawing.**
- The sphere is a translucent `Sphere` mesh with alpha blending.
- The state arrow is a `Cylinder` shaft plus a `Cone` tip, placed each frame by transform. This follows the presentation-diagram rule of built-in primitives only.
- Guides, trails, loci, and rotation axes are gizmos, in a dedicated gizmo config group with a wider line, so other experiences' gizmos are unaffected.
- Labels are projected egui text, as in the perceptron.
- Spheres for a two-qubit encoding sit side by side along x.

**Animation state is a resource advanced by `Time`.** The state is mode, progress, playing, and speed. It loops with a short hold at the end of each cycle. Tests drive it with `TimeUpdateStrategy::ManualDuration`, as the existing headless tests do.

**Formulas are typeset like the perceptron's.** The encoding formula and the basis-state kets are Typst math, typeset through `crate::formula` and cached as egui textures. The perceptron's texture cache moves into `formula.rs` so both experiences share it. Typst ink is dark, so the panel formula sits on a light card. Without the `math` feature, the plain-text forms are drawn in egui's monospace font (Hack), which, unlike the proportional font, has ⟩ and ⊗.

**Thumbnail showcase.** A thumbnail capture sets `HideOverlays` before the experience starts. When it is present, the experience opens paused partway through dense angle encoding's second rotation, so the card shows an axis, a turning circle, and a trail rather than a bare arrow.

**Category "Quantum computing".** A new lobby heading. Registering it last keeps the existing lobby order unchanged.

## Risks / Trade-offs

- [Gizmo line width is per config group] → Use a custom `GizmoConfigGroup` initialized by the plugin. Headless tests already include `GizmoPlugin`.
- [Translucent sphere sorting can hide the arrow] → Draw the arrow opaque and the sphere with low alpha and no depth write, then check visually in the thumbnail.
- [Thumbnail needs a GPU and display] → Generate it with `scripts/thumbnails.sh bloch-sphere` and commit the PNG, as the other built-ins do.
- [Paths lying on the sphere depth-fight it, and coinciding gizmo strips hide each other] → Trails and loci are drawn at 1.012× the radius, and the bright trace is drawn before the faint full path.
- [Adding systems changed scheduler ordering and exposed a latent race in `shape_view`: its chain's single `run_if(resource_exists::<ShapePlot>)` was checked once, but `ShapePlot` could be removed at a command flush inside the chain] → `distributive_run_if`, so each system checks for itself.
- [Two-qubit layout on narrow windows] → The orbit home view frames both spheres, and F resets it.
