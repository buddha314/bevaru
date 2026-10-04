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
