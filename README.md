# bevaru

`bevaru` is a Rust/Bevy plugin that leverages [ruviz](https://github.com/Ameyanagi/ruviz) to generate interactive scientific plots in Bevy.

Initial focus: Machine Learning / Deep Learning visual illustrations.  
Planned direction: richer 3D/VR visualizations and Blender-authored assets.

## Install in a Bevy project

```toml
[dependencies]
bevaru = "0.1"
```

## What is included right now

- A cargo-installable Rust crate (`bevaru`)
- A Bevy plugin (`BevaruPlugin`) that renders an ML sigmoid plot with `ruviz`
- Interactive refresh via `RefreshPlotEvent` (you can trigger it from Bevy input/UI systems)
- Example Bevy project entry point: `examples/ml_interactive.rs` (auto-refresh loop)

Run the example:

```bash
cargo run --example ml_interactive
```
