//! A linear SVM separating handwritten 3s from 8s in all 784 pixel
//! dimensions. The scene shows the data's top two principal components and
//! the boundary's slice through that plane; the right panel shows what the
//! model actually learned — its weight vector as a 28 × 28 image.
//!
//! Downloads MNIST (~11 MB) on first run and caches it; set
//! `BEVARU_MNIST_DIR` to use an existing copy.
//!
//! ```sh
//! cargo run --release --features mnist --example mnist_svm
//! ```

#[path = "shared/mod.rs"]
mod shared;

fn main() {
    shared::run_experience("mnist-svm");
}
