# Licensing boundary: Euro-Office, ECMA-376, and bevaru

This is the project's position for contributors, not legal advice. If a question goes beyond it, ask the maintainer before writing code.

## The parties

| Source | What it is | Licence | How bevaru may use it |
| ------ | ---------- | ------- | --------------------- |
| **bevaru** | this project | MIT OR Apache-2.0 | — |
| **Euro-Office** [`sdkjs`](https://github.com/Euro-Office/sdkjs), [`core`](https://github.com/Euro-Office/core) | an office suite, including presentation geometry (`sdkjs/common/Drawings/Format/CreateGeometry.js`, 227 presets) and format conversion (PPT, PPTX, ODP) | **AGPL-3.0** (© Ascensio System SIA), with GUI artwork and docs under **CC BY-SA 4.0** | **reference only** |
| **ECMA-376 / ISO/IEC 29500** | the Office Open XML standard, which defines the preset shapes (`ST_ShapeType`) and their geometry | Ecma standard, available free of charge | shape **names and semantics**; see the open question below on definition formulas |
| Bevy, Lyon, `bevy_procedural_meshes`, `procedural_modelling`, `bevy_prototype_lyon` | geometry and rendering | MIT and/or Apache-2.0 | dependencies, as needed |

## The rules

1. **No Euro-Office code or data enters bevaru.** That includes copying, translating (JavaScript → Rust), or transcribing its geometry formulas, adjust-value tables, guide lists, or path data, even though they originate in the standard. AGPL-3.0 would apply to the result, and bevaru is permissively licensed.
2. **Shape names and meanings come from ECMA-376.** Names like `roundRect`, `rightArrow`, and `straightConnector1` identify a standard's concepts; using them to label bevaru's shapes is interoperability, not copying an implementation.
3. **Geometry comes from permissive Rust crates** (Bevy first), as listed in [geometry provenance](geometry.md). Bevaru builds a shape from primitives, not from a transcribed preset.
4. **Euro-Office can be read to understand behaviour.** For example: which shapes are paths, which have adjust handles, and how a connector attaches. What is learned is written down as a description, not as code.
5. **Tighter integration goes upstream.** If bevaru ever needs something from Euro-Office (say, importing a bevaru diagram as editable slide objects), the work belongs in Euro-Office as a contribution under its licence, talking to bevaru through an open format (PPTX/ODP) or a documented interface. Euro-Office internals don't come into bevaru.

## Interoperating through formats is clean

An **editable 2-D fallback** writes a PPTX or ODP file that uses preset names, positions, and sizes. That is writing a standard file format, which any licence permits. A file bevaru writes can be opened in Euro-Office, LibreOffice, or PowerPoint, and no code is shared.

## Open question: reusing the standard's formulas

ECMA-376 ships its preset definitions as XML (`presetShapeDefinitions.xml`, with adjust values, guides, and paths). If bevaru ever wanted to *generate* exact preset outlines (for example, a pixel-faithful `rightArrow` with adjustable head), it would need those formulas from the standard itself, never from Euro-Office.

Before doing that, confirm:
- the copyright and reuse terms Ecma attaches to that file;
- whether an MIT/Apache-2.0 project may redistribute it or code derived from it.

Until then, bevaru uses names only and builds shapes from primitives.
