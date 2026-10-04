# From a bevaru visualization to a slide

There are three ways to put a bevaru diagram on a slide, in order of how far along each is.

```text
Bevaru diagram (src/diagram.rs: nodes, edges, labels, groups)
      │
      ├── 1. static image ──────────────► any slide             available now
      │
      ├── 2. interactive 3-D (WASM) ────► embedded in a slide   next
      │
      └── 3. editable 2-D shapes ───────► PPTX / ODP slide      possible (projection in place)
             via Diagram::project
```

## 1. Static image (available now)

Open the experience and press `H` for **slide view**. It does three things:
- hides every control and the lobby button;
- keeps the labels;
- frames the diagram nearly front-on, for a 16:9 slide on white.

Then take any screenshot. To capture automatically:

```sh
cargo run --release -- perceptron   # press H, then take a screenshot

# or open straight into slide view and save the window after 8 s
BEVARU_SLIDE_VIEW=1 BEVARU_SCREENSHOT=perceptron.png BEVARU_SCREENSHOT_AFTER=8 \
    cargo run --release -- perceptron
```

The image looks like the slide, but it isn't editable, and the 3-D interaction is lost.

## 2. Interactive 3-D, embedded (next)

The high-value target: the slide shows the live scene, and the presenter can orbit it. The route:
1. **A WASM build** of the experience (Bevy supports `wasm32-unknown-unknown` with WebGPU or WebGL2). It's served as a single page that opens straight into slide view.
2. **Embedding:**
   - **HTML-based decks** (reveal.js, and Euro-Office's web editors via a plugin) can embed it in an iframe.
   - **PPTX/ODP** can't embed a live web page portably. The slide carries a static image (route 1) with a hyperlink to the hosted page, which is what presentation tools support today.

Not started. What it needs: a `wasm32` build target in CI (bevaru has no hosted CI, so a local script), feature gating (no `ureq`/MNIST on the web), and asset loading over HTTP.

## 3. Editable 2-D shapes (the projection is in place)

Better than a PNG: the slide gets ordinary presentation objects that can be moved, recoloured, and relabelled in Euro-Office, LibreOffice, or PowerPoint.

`Diagram::project(view, aspect)` already gives, in slide coordinates ([0, 1]², origin top-left, using Bevy's own camera projection):
- every node's centre and apparent radius;
- every edge's endpoints, and whether it has an arrowhead;
- every label's position and text.

An exporter maps these to [ECMA-376 presets](shape-vocabulary.md):

| Diagram element | Slide object |
| --------------- | ------------ |
| node | `ellipse`, filled with the role colour, at the projected centre and radius |
| edge | `straightConnector1`, with `headEnd` for arrows, coloured and weighted by the edge's weight |
| label | a text box at the projected position |
| group | `roundRect` behind its members, translucent |

Writing PPTX is standard file I/O (zip + XML), and needs no Euro-Office code; see [licensing](licensing.md). It could live in a small permissive crate, or come from an existing Rust PPTX/ODF writer if one fits. A 3-D-looking "projection" is just the same positions with sizes scaled by depth.

Not started. The projection, the shape names, and the licensing boundary are the groundwork.
