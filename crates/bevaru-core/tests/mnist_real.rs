//! Against the real MNIST files. Ignored by default because it needs either
//! network access or `BEVARU_MNIST_DIR`:
//!
//! ```sh
//! cargo test -p bevaru-core --features mnist --release -- --ignored
//! ```
#![cfg(feature = "mnist")]

use bevaru_core::mnist::{MnistLoader, MnistOptions, Split};
use bevaru_core::{BinaryTask, Trainer, TrainerConfig};

#[test]
#[ignore = "needs MNIST (network or BEVARU_MNIST_DIR)"]
fn linear_svm_separates_three_from_eight() {
    let loader = MnistLoader::new().unwrap();
    let pair = BinaryTask::Pair {
        positive: "3".into(),
        negative: "8".into(),
    };
    let load = |split, n| {
        let opts = MnistOptions {
            split,
            digits: Some(vec![3, 8]),
            max_samples: Some(n),
            seed: 1,
        };
        loader
            .load(&opts)
            .unwrap()
            .binary(&pair)
            .unwrap()
            .training_data()
            .unwrap()
    };
    let train = load(Split::Train, 1500);
    let test = load(Split::Test, 500);
    let cfg = TrainerConfig {
        max_steps: 400,
        ..TrainerConfig::svm(10.0).unwrap()
    };
    let mut t = Trainer::new(cfg, train).unwrap();
    t.run();
    let f = t.model().decisions(&test.x);
    let correct = f
        .iter()
        .zip(test.y.iter())
        .filter(|(f, y)| **f * **y > 0.0)
        .count();
    let acc = correct as f64 / test.y.len() as f64;
    assert_eq!(t.model().w.len(), 784);
    assert!(acc > 0.9, "test accuracy {acc}");
    eprintln!("3-vs-8 test accuracy: {acc:.3}");
}
