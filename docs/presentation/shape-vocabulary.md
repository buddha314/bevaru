# Presentation-shape vocabulary

Presentation software shares one vocabulary of shapes: the **preset geometries of ECMA-376 / ISO/IEC 29500 (Office Open XML)**, enumerated as `ST_ShapeType`. PowerPoint, LibreOffice Impress (via OOXML import), and Euro-Office all implement it. Euro-Office's `sdkjs/common/Drawings/Format/CreateGeometry.js` defines 227 presets, transcribed from the standard's definitions.

Bevaru keys shapes by these **standard names**, so a bevaru diagram can map onto any of those editors. It doesn't use any implementation's code. See [licensing](licensing.md).

## How presets are defined

Every preset in the standard is a **path definition** in a shape coordinate space (width `w`, height `h`), with:
- **adjust values** (`adj1`, `adj2`, …, in 1/100 000ths), which are the handles a user drags (an arrow's head length, a rounded corner's radius);
- **guides**: formulas computed from the adjust values and the shape's size;
- **paths**: `moveTo`, `lnTo`, `arcTo`, `quadBezTo`, and `cubicBezTo`, built from the guides;
- **connection sites and text rectangles**.

For mapping onto 3-D geometry, the useful classification is:

| Class | Meaning | 3-D route |
| ----- | ------- | --------- |
| **Fixed** | No adjust values; the outline only scales | the matching Bevy 2-D primitive, extruded |
| **Adjust-parameterised** | Adjust values change the outline (corner radius, head size) | a Bevy primitive with parameters, or a path rebuilt from the adjusts |
| **Path-defined** | Concave or curved outline from line and curve segments | Lyon path → concave fill → extrude (needs gap G1 or G5) |
| **Connector** | A path between two shapes' connection sites, not a closed shape | tube between points, or along a path (needs G4) |

## The issue's shapes

The gap codes (G1–G5) refer to the [gap log](upstream-gaps.md).

| Shape | ECMA-376 preset | Class | Route through the FOSS stack | Gap |
| ----- | --------------- | ----- | ---------------------------- | --- |
| rectangle | `rect` | fixed | `Rectangle` → `Extrusion` → `Mesh3d` | — |
| rounded rectangle | `roundRect` | adjust (corner radius) | `Capsule2d` covers the stadium case only; general case needs a rounded rectangle | G2 |
| ellipse | `ellipse` | fixed | `Ellipse` → `Extrusion` → `Mesh3d` (`Sphere` / scaled sphere for a 3-D node) | — |
| triangle | `triangle` (isosceles), `rtTriangle` | adjust (apex) / fixed | `Triangle2d` → `Extrusion` | — |
| diamond | `diamond` | fixed | `Rhombus` → `Extrusion` | — |
| arrow | `rightArrow` (also `leftArrow`, `upArrow`, `downArrow`) | adjust (shaft width, head length) | concave 7-gon: `Polygon` → mesh → extrude; or a tube + `Cone` in 3-D | G1 |
| double arrow | `leftRightArrow`, `upDownArrow` | adjust | concave 10-gon, as above | G1 |
| curved arrow | `curvedRightArrow` (and left, up, down) | path-defined (arcs) | Lyon path with arcs → fill → extrude; or a tube along an arc + `Cone` | G1/G5, G4 |
| chevron | `chevron`; `homePlate` (pentagon arrow) | adjust | `chevron` is a concave hexagon (G1); `homePlate` is convex → `ConvexPolygon` → `Extrusion` | G1 |
| star | `star5` (also `star4`…`star32`) | adjust (inner radius) | concave 2n-gon → mesh → extrude | G1 |
| cross | `plus`; `mathPlus` | adjust (arm width) | concave 12-gon; or two `Cuboid`s in 3-D | G1 |
| bracket | `leftBracket`, `rightBracket`, `bracketPair` | path-defined, open (stroke) | stroked Lyon path → mesh, or tubes along the path | G4 / G5 |
| brace | `leftBrace`, `rightBrace`, `bracePair` | path-defined, open, with arcs | stroked Lyon path, or tube along the path | G4 / G5 |
| callout | `wedgeRectCallout`; `borderCallout1` (line callouts) | adjust (tail tip) | rectangle plus tail: one concave polygon | G1 |
| speech bubble | `wedgeRoundRectCallout`, `wedgeEllipseCallout`, `cloudCallout` | adjust; path-defined | rounded rectangle or ellipse plus tail, joined; cloud is arcs | G1, G2 |
| flowchart: process | `flowChartProcess` | fixed | `Rectangle` → `Extrusion` | — |
| flowchart: decision | `flowChartDecision` | fixed | `Rhombus` → `Extrusion` | — |
| flowchart: terminator | `flowChartTerminator` | fixed | `Capsule2d` (stadium) → `Extrusion` | — |
| flowchart: data (I/O) | `flowChartInputOutput` | fixed | parallelogram: `ConvexPolygon` → `Extrusion` | — |
| flowchart: document | `flowChartDocument` | path-defined (wavy base) | Lyon path → fill → extrude | G1/G5 |
| flowchart: predefined process | `flowChartPredefinedProcess` | fixed | `Rectangle` + two thin `Cuboid`s | — |
| flowchart: connector | `flowChartConnector` | fixed | `Circle` → `Extrusion` | — |
| connector (straight) | `straightConnector1` (`line` without arrowheads) | connector | `Cylinder` between points (+ `Cone` heads): **what the perceptron uses** | — |
| elbow connector | `bentConnector3` (also 2, 4, 5) | connector, adjust (bend positions) | consecutive `Cylinder` segments (+ spheres at the joints); or a sweep for rounded bends | — (G4 for smooth bends) |
| curved connector | `curvedConnector3` (also 2, 4, 5) | connector, cubic Bézier | tube along a Bézier path | G4 |

## The rest of the 227

The remaining presets fall into the same classes:
- **Path-defined:** more arrows (`notchedRightArrow`, `uturnArrow`, `circularArrow`, …), more stars and banners (`ribbon`, `wave`, `doubleWave`), action buttons, block shapes (`can`, `cube`, `bevel`, `frame`, `donut`, `noSmoking`, `blockArc`), and math operators.
- **Fixed or adjust-parameterised:** polygons (`pentagon`, `hexagon`, `octagon`, …), which map to `RegularPolygon` / `ConvexPolygon` → `Extrusion`.

G1 (concave polygon meshing) alone unlocks most of the rest; G4 (sweep) unlocks the curved family.

## Toward an editable 2-D fallback

`Diagram::project` already gives every node, edge, and label a position in slide coordinates. An exporter would write:
- **nodes** as `ellipse` (the tablet's round outline);
- **edges** as `straightConnector1` with `tailEnd`/`headEnd` arrows;
- **groups** as `roundRect`;
- **labels** as text bodies.

These are all fixed or simple adjust presets, needing no geometry at all. See [path to slides](slides.md).
