//! Linear regression on data with 15% outliers, fitted three ways side by
//! side: MSE is dragged toward the outliers, MAE ignores them, and Huber sits
//! between, controlled by δ.
//!
//! ```sh
//! cargo run --release --example regression_mse_vs_mae
//! ```

#[path = "shared/mod.rs"]
mod shared;

fn main() {
    shared::run_experience("regression-mse-vs-mae");
}
