//! Every loss as a 3-D surface over two quantities students already know:
//! truth and prediction, true and predicted probability, two class scores, a
//! value and a hyperparameter, or three classes. Each surface highlights the
//! slice that is the familiar 2-D loss curve, shown beside it.
//!
//! ```sh
//! cargo run --release --example loss_shapes
//! ```

#[path = "shared/mod.rs"]
mod shared;

fn main() {
    shared::run_experience("loss-shapes");
}
