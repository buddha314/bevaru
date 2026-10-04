## Why

Teaching slides draw neural networks as flat circles and arrows. [#20](https://github.com/buddha314/bevaru/issues/20) asks for the same diagrams rendered spatially, starting with a perceptron that can sit on a slide. It also sets a policy: bevaru must not grow its own geometry engine. It composes existing FOSS primitives and fixes gaps upstream.

A spike on 2026-10-04 found that Bevy 0.19 alone covers the whole perceptron:
- **Nodes:** `Sphere` and `Capsule3d`.
- **Weights:** `Cylinder` placed between two points.
- **Arrowheads:** `Cone`.
- **Group backdrops:** extruded 2-D primitives.

The same spike found real gaps worth taking upstream:
- **Concave polygons:** Bevy can't turn a concave `Polygon` into a mesh or extrude it, and presentation arrows, chevrons, and stars need that.
- **Rounded rectangles:** Bevy has no rounded-rectangle primitive.
- **3-D text:** Bevy has no text in a 3-D scene.

`bevy_procedural_meshes` is early-stage, and its `extrude` builds only side walls, so it isn't needed yet.

## What Changes

- **"Perceptron in 3D":** a new lobby experience, in a new *Diagrams* category, plus a `perceptron_3d` example. Three inputs, a weighted sum with bias, an activation, and an output.
  - **Geometry:** spheres and capsules for nodes, and tubes for weights. Each tube's thickness and colour encode the weight's magnitude and sign. Cones mark direction.
  - **Labels:** text projected onto the screen, as the loss-shape axes are.
  - **Camera and capture:** the orbit rig, and a slide-ready framing with no control panels.
- **A small declarative diagram model:** nodes, edges, labels, and groups, with stable ids. This is the Bevaru-specific "semantics" layer.
  - **3-D rendering:** maps each kind to existing Bevy primitives.
  - **2-D projection:** gives slide coordinates for each element. This keeps an editable 2-D fallback (presentation shapes) possible later, without building it now.
- **Geometry provenance:** each geometry operation is documented with the FOSS component responsible for it. No custom tessellator, extrusion engine, or mesh generator is added; the only bevaru-specific code is how primitives are placed.
- **Presentation docs** (`docs/presentation/`):
  - **Gap log:** missing capabilities, each with a proposed upstream home and a draft issue.
  - **Shape vocabulary:** common presentation shapes, keyed by the **ECMA-376 (OOXML) preset names** and classified by how each maps onto the FOSS stack.
  - **Licensing boundary:** Euro-Office is AGPL-3.0, and its `CreateGeometry.js` is a transcription of the ECMA-376 presets. Bevaru references the standard's vocabulary and never Euro-Office code.
  - **Path to slides:** a static capture today, an embedded WASM build later, and an editable 2-D fallback built from the diagram model.
- **Credit:** the README cites [vgarciasc/simulated-annealing-viz](https://github.com/vgarciasc/simulated-annealing-viz) as an inspiration, as the issue asks.
- **Upstream contributions:** proposed as draft issues in the gap log. Filing them is the maintainer's call.

## Capabilities

### New Capabilities
- `presentation-diagrams`: the declarative diagram model, its 3-D rendering from existing primitives, the 2-D projection, the perceptron experience and example, slide-ready capture, the geometry-provenance rule, and the presentation docs (gap log, shape vocabulary, licensing boundary, path to slides).

### Modified Capabilities
None. The new experience registers like any other, and the capability manifest lists it automatically.

## Impact

- **Code:**
  - new `src/diagram.rs`, holding the model, the 3-D mapping, and the 2-D projection;
  - new `src/experiences/perceptron.rs`;
  - a new `examples/perceptron_3d.rs`;
  - a lobby thumbnail.
- **Docs:**
  - new `docs/presentation/` (gap log, shape vocabulary, licensing, path to slides);
  - a README section and credit;
  - regenerated `docs/agents/capabilities.*` (the new experience).
- **Dependencies:** none. `bevy_procedural_meshes`, `bevy_prototype_lyon`, and Lyon were evaluated and recorded, and they're deferred until a shape needs them.
- **Licensing:** no AGPL code or data is copied. The shape vocabulary cites ECMA-376 names.
- **Existing behaviour:** unchanged.
