# Upstream gap log

Capabilities the presentation diagrams need that the FOSS stack lacks, and where each belongs. The project policy is **upstream first**: a generally useful capability goes to the project it belongs in, not into bevaru.

**Nothing here has been filed.** Each draft is ready, but filing an issue or opening a PR upstream is the maintainer's decision.

Searched 2026-10-04.

| # | Gap | Needed for | Upstream home | Existing upstream work | Next step |
| - | --- | ---------- | ------------- | ---------------------- | --------- |
| G1 | Mesh and extrude a concave (simple) `Polygon` | arrows, chevrons, stars, crosses, callouts | **Bevy** (`bevy_mesh`) | [bevy#15255](https://github.com/bevyengine/bevy/issues/15255), open, *S-Ready-For-Implementation*, *X-Uncontroversial* | **A PR**, not an issue: the design is agreed |
| G2 | Rounded-rectangle 2-D primitive (meshable, extrudable) | group backdrops, `roundRect`, `wedgeRoundRectCallout`, flowchart symbols | **Bevy** (`bevy_math`, `bevy_mesh`) | [bevy#13652](https://github.com/bevyengine/bevy/pull/13652) (squircle) closed unmerged, waiting on author and an expert; [bevy#10572](https://github.com/bevyengine/bevy/issues/10572) tracks primitives | Draft issue D2 |
| G3 | Text rendered by a 3-D camera | labels that respect depth and occlusion | **Bevy** | [bevy#5598](https://github.com/bevyengine/bevy/issues/5598), open, *S-Needs-Design* | Follow; comment with this use case only if useful |
| G4 | Tube (sweep / loft) along a path | curved arrows, curved and elbow connectors | **`procedural_modelling`** ("Loft along path" is unchecked in its feature list) | none found | Draft issue D4 |
| G5 | Solid extrusion with caps from a Lyon fill | any Lyon-defined shape as a solid | **`bevy_procedural_meshes`** (`extrude` builds the side wall only) | none found | Draft issue D5; may be moot once it moves onto `procedural_modelling` |

Placement code that only orients a primitive, such as a cylinder between two points, is bevaru composition, not a gap: it is a `Transform`, not geometry.

## G1: concave polygons in Bevy (draft PR plan)

The thread on bevy#15255 records the maintainers' preferences:
- **No new dependency** for this feature (`earcutr` and `iTriangle` were suggested and declined).
- **Ear-clipping** looks like the right algorithm.
- **Self-intersecting polygons** are unresolved: apply a fill rule, or reject them.

A PR that fits:

> **Implement `Meshable` and `Extrudable` for `Polygon` (simple polygons)**
>
> Fixes #15255.
> - `PolygonMeshBuilder`: triangulate a simple polygon (convex or concave) by ear-clipping, in `bevy_mesh`, with no new dependencies. Winding is normalised and normals face +Z, as for the other 2-D builders.
> - `Extrudable for PolygonMeshBuilder`: side walls along the outline, matching `Extrusion<ConvexPolygon>`, including the UV conventions from #23540.
> - Self-intersecting input is rejected with an error (or meshed as empty, with a warning), and documented. Fill rules can follow as a separate change.
> - Tests: a concave arrow outline, a star, CW vs CCW input, collinear points, and the triangle count n − 2.

## G2: rounded rectangle in Bevy (draft issue D2)

> **Add a rounded-rectangle 2-D primitive**
>
> **What problem does this solve?** Rounded rectangles are the most common shape in diagrams, slides, and UI-style 3-D scenes (cards, callouts, flowchart symbols). Today the closest is `Capsule2d` (a stadium), or extruding a hand-built polygon (blocked on #15255).
>
> **What solution would you like?** A `RoundedRectangle { half_size: Vec2, radius: f32 }` in `bevy_math` with `Meshable` (a resolution setting for the corner arcs), `Extrudable`, and gizmos, consistent with `Rectangle` and `Capsule2d`. A radius of 0 gives a rectangle, and a radius of `min(half_size)` gives a stadium.
>
> **Alternatives considered:** the squircle in #13652, a different shape whose PR stalled; per-application polygon builders.

## G3: 3-D text in Bevy

bevy#5598 already describes this. Bevaru uses the workaround its thread mentions (2-D text matched to the 3-D camera, here via egui), which is enough for slides. No action, unless the maintainer wants to add the presentation use case to the thread.

## G4: loft along a path in `procedural_modelling` (draft issue D4)

> **Loft (sweep) a profile along a path**
>
> The feature list has "Linear Loft (Triangle, Polygon)" and an unchecked "Loft along path". A sweep of a 2-D profile (a circle for tubes; any polygon in general) along a polyline or Bézier path, with rotation-minimising frames and optional end caps, would cover curved arrows, curved and elbow connectors, and pipes. Bevaru would use it for diagram connectors; [bevaru](https://github.com/buddha314/bevaru) is happy to help implement it if that's welcome.

## G5: capped extrusion in `bevy_procedural_meshes` (draft issue D5)

> **`extrude` with caps**
>
> `PMesh::extrude(direction)` produces the side wall of an outline. A solid needs the filled outline at both ends as well, with normals facing outwards. Would a `extrude_solid` (or a `caps: bool` option) built from the existing Lyon `fill` be welcome here, or is this better left for `procedural_modelling`, which the README says this crate will wrap?
