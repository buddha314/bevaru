//! Orbit the training objective over a model weight and bias. Hinge, squared
//! hinge, and logistic (binary cross-entropy) use the same fixed binary data.
//!
//! ```sh
//! cargo run --release --example loss_surface
//! ```

#[path = "shared/mod.rs"]
mod shared;

fn main() {
    shared::run_experience("loss-surface");
}
