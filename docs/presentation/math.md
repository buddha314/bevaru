# Math typesetting

Hovering a node or tube in **Perceptron in 3D** shows its formula, typeset, with the live values: z = Σ wᵢxᵢ + b, σ(z) = 1/(1 + e⁻ᶻ), and so on. This page records how bevaru typesets math, and why.

## What bevaru uses: Typst, through ruviz

[ruviz](https://crates.io/crates/ruviz), bevaru's charting dependency, has a `typst-math` feature. It embeds the [Typst](https://typst.app) engine (Apache-2.0, pure Rust, works offline) and exposes `ruviz::render::typst_text::render_raster`, which turns Typst markup into an RGBA image.

Bevaru's `math` cargo feature (on by default) turns `typst-math` on:
- **Typesetting:** `src/formula.rs` typesets; the perceptron experience shows each image in an egui tooltip.
- **Caching:** typeset formulas are cached as egui textures, keyed by source.
- **Warm-up:** the engine loads off the main thread when the experience starts.
- **Without `math`** (`--no-default-features`): tooltips show each formula's plain-text form instead. `bevaru-mcp` builds this way, since it has no tooltips.

Measured in the spike (2026-10-04, release build):

| | |
| - | - |
| First formula | ~100 ms (engine and fonts load once; done in the background) |
| Each later formula | 0.2–0.7 ms |
| Binary size | +32 MB stripped (36 MB vs 4.4 MB for ruviz alone), mostly embedded fonts. About +30% on bevaru's 107 MB release binary |
| Licence | Apache-2.0 (Typst), compatible with bevaru's MIT OR Apache-2.0 |

Formulas carry two forms, `Formula { typst, text }`:
- **`typst`** is typeset. Negative terms are coloured like negative tubes: `#text(fill: rgb("#d55e00"))[$-0.5$]`.
- **`text`** is the plain Unicode fallback, also useful for exports and screen readers.

## Typst syntax, not LaTeX

Formulas are written in Typst's math syntax:

| Typst | LaTeX |
| ----- | ----- |
| `$ sigma(z) = 1/(1 + e^(-z)) $` | `\sigma(z) = \frac{1}{1 + e^{-z}}` |
| `$ z = sum_(i=1)^3 w_i x_i + b $` | `z = \sum_{i=1}^{3} w_i x_i + b` |
| `$ H(z) = cases(1 & "if" z >= 0, 0 & "if" z < 0) $` | `H(z) = \begin{cases} 1 & z \ge 0 \\ 0 & z < 0 \end{cases}` |
| `$ hat(y) = sigma(bold(w)^top bold(x) + b) $` | `\hat{y} = \sigma(\mathbf{w}^\top \mathbf{x} + b)` |

LaTeX input is possible through [mitex](https://github.com/mitex-rs/mitex) (Apache-2.0, a LaTeX → Typst converter). In the spike it handled `\sum`, `\frac`, and Greek letters, but:
- `cases` lost its `&` alignment;
- `\mathbf` and `\!` failed;

because mitex's output relies on helper definitions from its own Typst package, which Typst fetches from the network. Bevaru's formulas are its own, so writing them in Typst is simplest. Accepting user-typed LaTeX would mean bundling mitex's definitions.

## Alternatives considered

| Option | What it produces | Verdict |
| ------ | ---------------- | ------- |
| **Typst via ruviz `typst-math`** | RGBA image | **Chosen.** Already a dependency, offline, fast, good quality |
| `typst` 0.15 directly (+ `typst-render`) | RGBA image or SVG | The same engine, newer version, but a second copy alongside ruviz's 0.13. Only if newer Typst features are needed |
| [KaTeX](https://katex.org) (`katex` crate) | HTML | Needs a browser to display. **The right choice for a future WASM build**: draw formulas as HTML over the canvas, with no font payload in the `.wasm` |
| MathJax (`mathjax_svg`) | SVG | Needs a JavaScript runtime. Too heavy |
| [`pulldown-latex`](https://crates.io/crates/pulldown-latex) | MathML | No renderer. **The route for editable slide export**: ODP stores MathML equations natively, so formulas stay editable in LibreOffice and Euro-Office |
| Tectonic | PDF (a full TeX engine) | Downloads support files; far too heavy for tooltips |
| Unicode text in egui | text | What the labels use. No fractions, sums with limits, or cases |

## Where this is going

- **The labels in the scene** could use the same typeset images instead of Unicode text.
- **WASM / slides in a browser:** KaTeX as an HTML overlay. See [path to slides](slides.md).
- **Editable PPTX/ODP export:** write each formula as MathML (from `pulldown-latex`, or from a Typst → MathML path if one matures) into a native equation object.
