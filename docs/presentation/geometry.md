# Geometry provenance

Bevaru composes geometry; it does not generate it. This page names the component that performs every geometry operation in the presentation diagrams (`src/diagram.rs`). Anything not listed here doesn't exist in bevaru.

## What draws the perceptron

| Element | Operation | Performed by | Bevaru's part |
| ------- | --------- | ------------ | ------------- |
| Node (input, Σ, σ, y) | round tablet mesh | Bevy `Capsule3d` → `Capsule3dMeshBuilder` (`bevy_mesh`) | one shared unit capsule, its axis pointing at the camera, flattened along that axis by a `Transform` scale. That gives a circle of constant radius in the diagram's plane, a short edge band, and domed faces. Bevy's renderer corrects normals for the non-uniform scale |
| Where a tube meets a node | ray–capsule distance | bevaru (`Node::surface_distance`), closed-form; in the diagram's plane it is just the radius | placement only: where to put the tube's end, not a mesh |
| Weight / connection | cylinder mesh | Bevy `Cylinder` → `CylinderMeshBuilder` | one shared unit mesh, placed between two points by a `Transform` (`Quat::from_rotation_arc`) |
| Arrowhead | cone mesh | Bevy `Cone` → `ConeMeshBuilder` | one shared unit mesh, placed by a `Transform` |
| Group backdrop (input layer) | 2-D stadium extruded to a slab | Bevy `Capsule2d` + `Extrusion` (`Extrudable for Capsule2dMeshBuilder`) | chooses size and depth; one mesh per group |
| Weight sign / magnitude | colour, radius | Bevy `StandardMaterial`; `Transform` scale | the encoding (Okabe–Ito blue / vermillion; radius from \|w\|) |
| Labels | text placed at projected 3-D points | `egui` painter, positions from Bevy `Camera::world_to_viewport` | where each label anchors |
| Slide projection | world → slide coordinates | Bevy `PerspectiveProjection::get_clip_from_view` and the view transform | maps NDC to [0, 1]² |
| Camera | orbit, pan, zoom | `bevaru::orbit` (input → `Transform`; no geometry) | the rig |

Bevaru contains no vertex generation, tessellation, triangulation, or extrusion code. Nodes, tubes, and arrowheads share one unit mesh per kind, whatever the diagram's size. A test (`mesh_count_does_not_grow_with_the_diagram`) checks this.

## The FOSS stack evaluated (spike, 2026-10-04, Bevy 0.19.1)

| Component | Version | Licence | What it offers | Status here |
| --------- | ------- | ------- | -------------- | ----------- |
| **Bevy** `bevy_math` + `bevy_mesh` | 0.19.1 | MIT / Apache-2.0 | `Meshable`: sphere, cuboid, cylinder, cone, conical frustum, capsule, torus, tetrahedron, plane. `Extrudable`: circle, ellipse, rectangle, triangle, rhombus, regular and convex polygon, annulus, 2-D capsule, circular sector and segment, ring | **Used.** Covers the whole perceptron |
| [`bevy_procedural_meshes`](https://github.com/bevy-procedural/meshes) | 0.19.0 | MIT / Apache-2.0 | `PMesh` builder; Lyon fill and stroke (`lyon` feature); `extrude` | **Evaluated, not adopted.** Self-described "very early stage", and its `extrude` builds only the side wall of an outline (no caps). It plans to become a plugin over `procedural_modelling` |
| [`procedural_modelling`](https://github.com/bevy-procedural/modelling) | 0.5.0 | MIT / Apache-2.0 | half-edge meshes; ear-clipping, sweep-line, and Delaunay triangulation; extrude; linear loft; prisms, platonic and round primitives | **Evaluated, not adopted.** Early stage. The natural home for "loft along a path" (unchecked in its feature list) |
| [`bevy_prototype_lyon`](https://github.com/rparrett/bevy_prototype_lyon) | 0.17.0 | MIT / Apache-2.0 | 2-D shapes, paths, Bézier curves, fills and strokes as 2-D meshes | **Evaluated, not adopted.** 2-D only; useful for an editable 2-D fallback |
| [Lyon](https://github.com/nical/lyon) | 1.0 | MIT / Apache-2.0 | path model and fill/stroke tessellation | **The path vocabulary to use** when a shape needs one, through `bevy_procedural_meshes` or directly |

**When to adopt:** the first shape that needs a concave fill, a path, or a sweep (an arrow, chevron, star, callout, or curved connector) adopts Lyon for the path, and either upstream Bevy polygon meshing or `bevy_procedural_meshes`/`procedural_modelling` for the solid. See [the gap log](upstream-gaps.md).
