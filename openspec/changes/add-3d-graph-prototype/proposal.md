## Why

bevaru can display interactive 3D scenes but has no general graph-loading experience. Evaluating graph libraries, especially rust-igraph, alongside a bounded prototype will establish how network data can become an explorable 3D scene without coupling graph mathematics to Bevy.

## What Changes

- Compare graph storage, import, layout, and rendering tools, distinguishing verified capabilities from claims requiring a spike.
- Evaluate rust-igraph first, including its actual 3D layout API, reproducibility, performance, packaging, and license fit; record an explicit backend decision and fallback.
- Add a `graph-3d` custom experience with an embedded sample and a local JSON path input, node/edge counts, loading/error feedback, and graph replacement.
- Render nodes and edges in genuine 3D with orbit, pan, zoom, fit/reset, and node inspection; use supplied coordinates or compute a bounded deterministic layout.
- Cover validation, asynchronous completion, and experience teardown headlessly; document a reproducible visual smoke test.

## Capabilities

### New Capabilities

- `graph-tool-evaluation`: Evidence-based comparison and a reproducible backend feasibility spike.
- `graph-3d`: Validated graph loading, layout, rendering, inspection, and lifecycle behavior for a prototype experience.

### Modified Capabilities

None. The new experience uses existing registry, command-line selection, and cleanup contracts.

## Impact

Planned implementation touches Bevy-free graph types/layout in `crates/bevaru-core`, a new experience under `src/experiences/`, experience registration, agent capability descriptions and generated documentation, fixtures, and headless lobby tests. Any selected dependency is added only after evaluation; rust-igraph is not assumed to be a renderer or an approved dependency. Existing experience ids and default lobby startup remain unchanged. This change creates planning artifacts; implementation is tracked in `tasks.md`.
