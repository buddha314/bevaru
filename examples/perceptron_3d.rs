//! A perceptron drawn in 3-D for slides: inputs, weights (blue positive,
//! orange negative, thickness |w|), a weighted sum, an activation, and the
//! output. Change the weights live; press `H` for slide view, which hides
//! every control so a screenshot is the slide.
//!
//! ```sh
//! cargo run --release --example perceptron_3d
//! ```

#[path = "shared/mod.rs"]
mod shared;

fn main() {
    shared::run_experience("perceptron");
}
