## Context

[#20](https://github.com/buddha314/bevaru/issues/20) asks for neural-network diagrams rendered in 3-D, good enough to put on a teaching slide, starting with a perceptron. It sets a strict policy:
- **Bevaru provides composition and semantics, not geometry.** Shapes come from Bevy first, then existing FOSS crates.
- **Gaps go upstream.** Anything missing and generally useful is fixed where it belongs.
- **Euro-Office (AGPL-3.0) is a reference vocabulary only.** None of its code comes into bevaru.

**Spike results (2026-10-04),** against the pinned Bevy 0.19.1:

| Need | What exists | Verdict |
| ---- | ----------- | ------- |
| Nodes | `Sphere`, `Capsule3d` (`Meshable`) | Bevy covers it |
| Weights (tubes between two points) | `Cylinder`, placed with a `Transform` | Bevy covers it; placement is a few lines of bevaru code |
| Arrowheads | `Cone` | Bevy covers it |
| Flat shapes with depth | `Extrusion<T>` for circle, ellipse, rectangle, triangle, rhombus, regular and convex polygon, annulus, 2-D capsule, sectors, rings | Bevy covers it |
| Concave outlines (arrows, chevrons, stars) | `Polygon` exists in `bevy_math` but is neither `Meshable` nor `Extrudable` | **Gap: Bevy** |
| Rounded rectangle | not a Bevy primitive (`Capsule2d` gives a stadium) | **Gap: Bevy** |
| Text in a 3-D scene | only `Text2d` | **Gap: Bevy** (long-standing). Bevaru already projects egui labels for the loss shapes |
| Tube along a curve; connectors | nothing in Bevy; `bevy_procedural_meshes` has no sweep | **Gap**, upstream home to be decided |
| Lyon paths, fill, and stroke | `bevy_procedural_meshes` 0.19 (`lyon` feature), `bevy_prototype_lyon` 0.17, both MIT/Apache-2.0 on Bevy 0.19 | Available when a shape needs it |

About `bevy_procedural_meshes`:
- Its README calls it "very early stage" and says it will become a thin plugin over `procedural_modelling`.
- Its `PMesh::extrude` builds only the side wall of an outline, so a solid extrusion needs a filled cap, the wall, and a second cap, assembled by the caller.
- Nothing in the perceptron needs it.

About Euro-Office:
- **The geometry:** its `sdkjs/common/Drawings/Format/CreateGeometry.js` (AGPL-3.0, © Ascensio System SIA) holds 227 preset shapes. They are a direct transcription of the ECMA-376 (Office Open XML) preset shape definitions: the same adjust values (`adj1 = 50000`) and guide formulas (`AddGuide('dx1', 0, 'ss', 'a2', '100000')`).
- **The vocabulary:** it belongs to the standard (`ST_ShapeType`: `rect`, `roundRect`, `rightArrow`, `chevron`, …). Euro-Office is one implementation of it.

## Goals / Non-Goals

**Goals:**
- A perceptron that looks right on a slide, built only from Bevy primitives and bevaru placement code.
- A small declarative diagram model with ids, so the same diagram can also become 2-D slide shapes later.
- Each geometry operation documented with the component that performs it.
- Every gap logged, with an upstream home and a draft issue.
- The ECMA-376 shape vocabulary mapped onto the FOSS stack.
- The licensing boundary documented.
- The path to slides documented.

**Non-Goals:**
- A general shape library, or any tessellation, extrusion, or mesh generation in bevaru.
- Implementing presentation shapes beyond what the perceptron needs (no arrows, callouts, or connectors yet).
- PPTX or ODP export, embedding, or WASM builds. They are documented as next steps.
- Filing upstream issues or PRs without the maintainer's go-ahead.
- Multi-layer networks. The model allows them, but only the perceptron ships.

## Decisions

### 1. Bevy primitives only, for now
Every mesh comes from Bevy's `Meshable` primitives. Bevaru only places and scales them. No new dependencies are added.

`bevy_procedural_meshes` and Lyon are evaluated and recorded, and adopted the day a shape needs a concave fill or a path. This respects "integrate, don't reinvent" without taking an early-stage dependency for nothing.

*Alternative:* adopt `bevy_procedural_meshes` now for uniformity. Rejected: nothing uses it, and its API is still moving.

### 2. One shared unit mesh per primitive
The diagram uses one unit capsule, one unit cylinder, and one unit cone.
- **Nodes are round tablets:** the capsule's axis points at the camera, and its `Transform` scale flattens it along that axis. That gives a circle of constant radius, a short edge band, and domed faces like a prescription pill, and it gives the scene a clear internal plane.
- **Attachment:** a closed-form ray–capsule distance places tube ends on the rim. In the diagram's plane it is the radius.
- **Lighting:** a coated (clearcoat) material, plus key, fill, and rim lights, brings out the volume. Each element is a `Transform`, with translation, rotation (`Quat::from_rotation_arc(Vec3::Y, dir)`), and scale. That gives:
- a fixed number of mesh assets, whatever the diagram's size;
- less clean-up to leak;
- no "tube between two points" mesh code, because it is just a transform.

That placement helper is bevaru composition, not geometry, so it stays local.

### 3. A declarative diagram model in `src/diagram.rs`
```text
Diagram { nodes, edges, groups }
  Node  { id, label, role: Input | Bias | Sum | Activation | Output, position, radius }
  Edge  { id, from, to, weight: Option<f64>, label, arrow }
  Group { id, label, members }
```
- **Plain serde data:** ids are stable strings, and nothing in the model knows about Bevy rendering.
- **Validation:** edge endpoints and group members must exist, and ids must be unique.
- **`Diagram::perceptron(weights, bias)`** builds the milestone diagram:
  - inputs x₁, x₂, x₃ and a bias b;
  - a sum node Σ, an activation node σ, and the output y;
  - weighted edges into Σ, and arrows Σ → σ → y.

### 4. Encoding weights
- **Colour** shows the sign: blue for positive, vermillion for negative (Okabe–Ito, readable with colour-vision deficiencies).
- **Radius** grows with |w| between a minimum and a maximum, so a zero weight is a thin grey thread, not a missing edge.
- **Arrowheads** are cones at the head end. Each edge is shortened by the node radii so tubes stop at the surface.

### 5. Labels are projected overlays, the same as for loss shapes
Bevy 0.19 has no text in 3-D space. Labels (x₁, w₁ = 0.8, Σ, σ, y) are egui text projected from world positions, kept inside the view, as in `shape_view`. They are diagram content, so they stay visible in slide view.

*Alternatives:*
- **Render-to-texture `Text2d` quads:** one camera and image per label, more assets, and more leak surface.
- **A third-party 3-D text crate:** a new dependency for a gap Bevy itself should close. It's logged in the gap log.

### 6. Slide view
The experience opens with a minimal floating panel: labels and weights toggles, an activation choice (step or sigmoid), and *Reset view*.
- **`H` toggles slide view.** It hides that panel and the lobby button by inserting the existing `HideOverlays` marker, and it frames the diagram for 16:9.
- **Leaving slide view:** `Esc` still leaves the experience, and `H` restores the panel.
- **Capture:** any screen capture of slide view is the slide image, including `BEVARU_SCREENSHOT`.
- **Background:** white, so the image blends into a light slide.

### 7. A 2-D projection keeps the editable fallback possible
`Diagram::project(view, aspect)` uses the camera's view and perspective to give every node, edge, and label a position and size in normalised slide coordinates ([0, 1]², origin top-left).
- It is pure math, with no GPU, and tested.
- It's the bridge to a future "editable 2-D fallback":
  - **nodes →** `ellipse`;
  - **edges →** `straightConnector1` with arrowheads;
  - **labels →** text boxes;
  - **groups →** `roundRect`, as ECMA-376 presets.

Nothing writes PPTX or ODP in this change.

### 8. The shape vocabulary is keyed by ECMA-376 names
`docs/presentation/shape-vocabulary.md` covers the issue's list: rectangle, rounded rectangle, ellipse, triangle, diamond, arrows, chevron, star, cross, bracket, brace, callouts, flowchart symbols, and connectors.
- **Names:** each is listed by its `ST_ShapeType` preset name.
- **Kind:** each is classified as a fixed preset, an adjust-parameterised preset, a path-defined preset, or a connector.
- **Route:** each is mapped to a FOSS route:
  - ellipse → `Ellipse` → `Extrusion` → `Mesh3d`;
  - rightArrow → path → (Lyon fill + extrude, a gap) → `Mesh3d`.

The vocabulary cites the standard, not Euro-Office.

### 9. The licensing boundary is written down
`docs/presentation/licensing.md` sets the boundary:
- **Never copy from Euro-Office:** no code, data tables, or translated formulas from `sdkjs` or `core` (AGPL-3.0, plus CC BY-SA 4.0 for their artwork).
- **Allowed:** shape *names* and *semantics* from ECMA-376 / ISO/IEC 29500, and permissive Rust crates.
- **Euro-Office's role:** a behavioural reference and a possible target for contributions.
- **Still to check:** the terms for reusing the standard's `presetShapeDefinitions.xml` formulas. This is an open question below.

### 10. A gap log with draft upstream issues
`docs/presentation/upstream-gaps.md` gives each gap:
- what's missing and who needs it;
- the proposed upstream home (Bevy, `procedural_modelling`, or `bevy_procedural_meshes`);
- a search for existing upstream issues;
- a ready-to-file draft.

Filing waits for the maintainer.

### 11. The lobby
"Perceptron in 3D" (id `perceptron`) is a custom experience in a new **Diagrams** category, and `examples/perceptron_3d.rs` opens it.
- **Clean-up:** like the loss shapes, its entities carry `ExperienceEntity`, its resources go on stop, and it's added only when mesh assets and gizmos exist.
- **Tests:** it joins the ten-round-trip leak test.

## Risks / Trade-offs

- **[Projected labels don't follow occlusion]** A label stays visible when its node is behind another. → The perceptron is shallow, and the home and slide views are chosen so nothing overlaps. A real fix is 3-D text in Bevy, which is gap-logged.
- **[Diagrams that differ from slide conventions]** A 3-D perceptron can read worse than the familiar 2-D one. → The slide view looks nearly front-on, with gentle depth. The capture is reviewed in a real window before merge.
- **[Shape-vocabulary scope creep]** Mapping ~200 presets would be a project of its own. → Cover the issue's list (~25) and note how the rest classify.
- **[Upstream may decline]** Bevy may not want a concave-polygon triangulator. → The draft names alternatives, such as `procedural_modelling` and a Lyon-backed adapter crate. If an upstream home declines, a separate permissive crate (not bevaru) is the fallback.
- **[ECMA-376 reuse terms]** The standard may restrict reproducing its definition XML. → The vocabulary uses names only. Importing formulas is an open question for later.

## Open Questions

- Should the diagram's weights later come from a model bevaru trains (for example the Iris logistic regression), so the slide shows *learned* weights? This change shows fixed, adjustable illustrative weights.
- What are the exact reuse terms for ECMA-376's `presetShapeDefinitions.xml`, if bevaru ever imports formulas rather than names?
- Is *Diagrams* the right lobby category, or should it be *Neural networks*?
