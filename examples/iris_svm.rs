//! Iris, versicolor vs virginica on the petal features: hinge-loss SVM beside
//! logistic regression while C sweeps from 0.01 to 100. Watch the SVM margin
//! narrow and the support vectors thin out as C grows.
//!
//! ```sh
//! cargo run --release --example iris_svm
//! ```

#[path = "shared/mod.rs"]
mod shared;

fn main() {
    shared::run_experience("iris-svm");
}
