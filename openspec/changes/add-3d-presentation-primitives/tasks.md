## 1. Diagram model (`src/diagram.rs`)

- [x] 1.1 `Diagram`, `Node` (with `NodeRole`), `Edge`, and `Group`, with stable string ids, serde, and validation that names the offending id
- [x] 1.2 `Diagram::perceptron(weights, bias)`: inputs x₁–x₃, bias b, Σ, σ, and y; four weighted edges into Σ; arrowed Σ → σ → y; an input-layer group; weight labels
- [x] 1.3 Tests: the perceptron validates (7 nodes, 6 edges, the weights on the Σ edges, arrowheads); a dangling edge is rejected by name; JSON round-trips

## 2. 3-D rendering from Bevy primitives

- [x] 2.1 One shared unit mesh each for the sphere, cylinder, and cone; nodes, tubes, and arrowheads as transforms of them (`Quat::from_rotation_arc`), with tubes ending at node surfaces
- [x] 2.2 Weight encoding: Okabe–Ito sign colours and radius from |w|, with a neutral thread at w = 0; the input-layer group as a translucent extruded `Capsule2d` backdrop
- [x] 2.3 Projected egui labels (nodes, weights), kept inside the view as in `shape_view`, and shown in slide view
- [x] 2.4 Tests: same mesh-asset count for the perceptron and a larger diagram with the same groups (nodes, tubes, and cones share meshes; each backdrop has its own); tube endpoints lie on node surfaces; sign changes hue and |w| orders radii

## 3. 2-D slide projection

- [x] 3.1 `Diagram::project(view, aspect)` → node centres and sizes, edge endpoints, and label positions in [0, 1]² (origin top-left), as a pure function
- [x] 3.2 Tests: a front-on view preserves left-right and top-bottom order inside [0, 1]; the projection uses Bevy's `PerspectiveProjection`, and an orbit view's target lands at the slide centre

## 4. Experience

- [x] 4.1 Register "Perceptron in 3D" (`perceptron`, *Diagrams* category) as a custom experience, added only when mesh assets and gizmos exist; orbit rig with home view; white background
- [x] 4.2 Floating controls: three weight sliders, bias, activation (step or sigmoid), label toggle, and *Reset view*; changes update the diagram in place
- [x] 4.3 Slide view on `H`: insert or remove `HideOverlays`, frame for 16:9, keep labels; `Esc` still leaves the experience
- [x] 4.4 `examples/perceptron_3d.rs` via `shared::run_experience`; thumbnail via `scripts/thumbnails.sh perceptron`
- [x] 4.5 Headless lobby tests: open, live weight change, slide-view toggle, and the ten-round-trip leak test

## 5. Presentation docs (`docs/presentation/`)

- [x] 5.1 `geometry.md`: provenance table, giving each operation and the Bevy type or crate that performs it, and the spike's evaluation of `bevy_procedural_meshes`, `bevy_prototype_lyon`, and Lyon
- [x] 5.2 `upstream-gaps.md`: concave `Polygon` meshing and extrusion (Bevy), rounded-rectangle primitive (Bevy), 3-D text (Bevy), tube or sweep along a path (`procedural_modelling` or `bevy_procedural_meshes`), and solid extrusion with caps (`bevy_procedural_meshes`). Each lists the upstream issues it found and has a draft issue. Nothing is filed without approval
- [x] 5.3 `shape-vocabulary.md`: the issue's shapes by ECMA-376 preset name, classification, FOSS route, and gap link
- [x] 5.4 `licensing.md`: the AGPL boundary with Euro-Office (no code, data, or translated formulas), ECMA-376 names as the source, permissive crates, and the open question on reusing definition formulas
- [x] 5.5 `slides.md`: the path from static capture (slide view) to an embedded WASM build and an editable 2-D fallback from `Diagram::project` as ECMA-376 presets
- [x] 5.6 README: a "Perceptron in 3D" section with a slide-view screenshot, links to `docs/presentation/`, and credit to vgarciasc/simulated-annealing-viz under credits

## 6. Verification

- [x] 6.1 `scripts/check.sh`; `scripts/check.sh --tests` before merging; regenerate `docs/agents/capabilities.*`
- [ ] 6.2 Look at the experience in a real window: orbit, live weights, the activation switch, labels, and a slide-view capture that reads well on a 16:9 slide
