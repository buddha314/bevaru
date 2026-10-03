//! The original scaffold as the `sigmoid` experience: ruviz renders a sigmoid plot to PNG
//! bytes ([`PlotPngBytes`]), refreshed every second via [`RefreshPlotEvent`],
//! and shown as a sprite.
//!
//! ```sh
//! cargo run --example ml_interactive
//! ```

#[path = "shared/mod.rs"]
mod shared;

fn main() {
    shared::run_experience("sigmoid");
}
