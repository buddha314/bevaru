//! Every loss on one screen: classification losses against the margin and
//! regression losses against the residual. Adjust Huber δ and the hinge
//! margin in the Models panel and watch the curves change, while a hinge SVM
//! trains on separable data.
//!
//! ```sh
//! cargo run --release --example loss_curves
//! ```

#[path = "shared/mod.rs"]
mod shared;

fn main() {
    shared::run_experience("loss-curves");
}
