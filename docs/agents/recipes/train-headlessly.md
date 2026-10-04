# Recipe: train a model headlessly

Train a linear SVM on Iris and inspect the result, with no window and no Bevy: everything here is in `bevaru-core` (re-exported as `bevaru::core`).

This is [`examples/headless_training.rs`](../../../examples/headless_training.rs); run it with `cargo run --example headless_training`.

<!-- include: examples/headless_training.rs -->
```rust
//! Train a linear SVM on Iris with no window and no Bevy: just `bevaru-core`.
//!
//! ```sh
//! cargo run --example headless_training
//! ```

use bevaru::core::dataset::{BinaryTask, iris};
use bevaru::core::{Status, Trainer, TrainerConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Versicolor vs virginica: the two Iris classes that overlap.
    let data = iris()
        .binary(&BinaryTask::Pair {
            positive: "versicolor".into(),
            negative: "virginica".into(),
        })?
        .training_data()?;

    let config = TrainerConfig {
        max_steps: 4000,
        ..TrainerConfig::svm(10.0)?
    };
    let mut trainer = Trainer::new(config, data)?;
    let status = trainer.run();

    let result = trainer.latest();
    let f = result.model.decisions(&trainer.data().x);
    let correct = f
        .iter()
        .zip(trainer.data().y.iter())
        .filter(|(f, y)| **f * **y > 0.0)
        .count();
    println!(
        "{status:?} after {} steps: objective {:.4}, accuracy {}/{}, {} support vectors",
        result.step,
        result.loss,
        correct,
        trainer.data().y.len(),
        result.support_vectors.len()
    );
    // Every step is kept, so you can inspect the trajectory.
    let halfway = trainer.snapshot(result.step / 2);
    println!("objective at step {}: {:.4}", halfway.step, halfway.loss);
    assert!(matches!(
        status,
        Status::Converged | Status::BudgetExhausted
    ));
    Ok(())
}
```

## Variations

- **Another model:** `TrainerConfig::logistic()`, or `TrainerConfig::regression(LossKind::Huber)` on regression data (`bevaru::core::dataset::regression`).
- **Another hyperparameter:** set fields on the config (`lambda`, `learning_rate`, `batch_size`, `max_steps`) or use `with_c` for an SVM. Valid ranges are in the [reference](../capabilities.md#models).
- **Step by step:** call `trainer.step()` instead of `run()`; it returns the status after each step.
- **A sweep:** build one trainer per value (`SweepParam::apply` in the `bevaru` crate does the bookkeeping) and `run()` each to convergence.
