//! Datasets: a common representation, bundled Iris, binary-task derivation,
//! and seeded synthetic generators.

use std::fmt;

use nalgebra::{DMatrix, DVector};
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::loss::Task;

/// What a dataset's samples are labelled with.
#[derive(Debug, Clone, PartialEq)]
pub enum Targets {
    /// Multi-class labels, indices into `names`.
    Classes {
        labels: Vec<usize>,
        names: Vec<String>,
    },
    /// A binary task with labels in `{−1, +1}`.
    Binary {
        labels: Vec<f64>,
        positive: String,
        negative: String,
    },
    /// Real-valued regression targets.
    Values(Vec<f64>),
}

/// Rows are samples, columns are features.
#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    pub name: String,
    pub features: DMatrix<f64>,
    pub feature_names: Vec<String>,
    pub targets: Targets,
}

/// Features and targets ready for a trainer.
#[derive(Debug, Clone, PartialEq)]
pub struct TrainingData {
    pub x: DMatrix<f64>,
    pub y: DVector<f64>,
    pub task: Task,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DatasetError {
    UnknownClass(String),
    SameClass(String),
    NotMultiClass,
    NeedsBinaryTask,
    Parse(String),
}

impl fmt::Display for DatasetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatasetError::UnknownClass(c) => write!(f, "unknown class {c:?}"),
            DatasetError::SameClass(c) => write!(f, "cannot compare class {c:?} with itself"),
            DatasetError::NotMultiClass => f.write_str("dataset does not have multi-class labels"),
            DatasetError::NeedsBinaryTask => {
                f.write_str("multi-class data must be reduced to a binary task before training")
            }
            DatasetError::Parse(e) => write!(f, "parse error: {e}"),
        }
    }
}

impl std::error::Error for DatasetError {}

/// How to reduce a multi-class dataset to a binary task.
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryTask {
    /// `positive` vs `negative`; other classes are dropped.
    Pair { positive: String, negative: String },
    /// `positive` vs every other class.
    OneVsRest { positive: String },
}

impl Dataset {
    pub fn len(&self) -> usize {
        self.features.nrows()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dim(&self) -> usize {
        self.features.ncols()
    }

    pub fn task(&self) -> Task {
        match self.targets {
            Targets::Values(_) => Task::Regression,
            _ => Task::Classification,
        }
    }

    pub fn class_names(&self) -> Vec<String> {
        match &self.targets {
            Targets::Classes { names, .. } => names.clone(),
            Targets::Binary {
                positive, negative, ..
            } => vec![positive.clone(), negative.clone()],
            Targets::Values(_) => Vec::new(),
        }
    }

    /// Class index of each sample, for colouring. Binary tasks map `+1 → 0`
    /// and `−1 → 1`; regression data is all class 0.
    pub fn class_indices(&self) -> Vec<usize> {
        match &self.targets {
            Targets::Classes { labels, .. } => labels.clone(),
            Targets::Binary { labels, .. } => labels
                .iter()
                .map(|&y| if y > 0.0 { 0 } else { 1 })
                .collect(),
            Targets::Values(v) => vec![0; v.len()],
        }
    }

    pub fn binary(&self, task: &BinaryTask) -> Result<Dataset, DatasetError> {
        let Targets::Classes { labels, names } = &self.targets else {
            return Err(DatasetError::NotMultiClass);
        };
        let index = |name: &str| {
            names
                .iter()
                .position(|n| n == name)
                .ok_or_else(|| DatasetError::UnknownClass(name.into()))
        };
        let (pos, neg, neg_name) = match task {
            BinaryTask::Pair { positive, negative } => {
                if positive == negative {
                    return Err(DatasetError::SameClass(positive.clone()));
                }
                (index(positive)?, Some(index(negative)?), negative.clone())
            }
            BinaryTask::OneVsRest { positive } => {
                (index(positive)?, None, format!("not {positive}"))
            }
        };
        let keep: Vec<usize> = (0..labels.len())
            .filter(|&i| labels[i] == pos || neg.is_none_or(|n| labels[i] == n))
            .collect();
        let features = self.features.select_rows(&keep);
        let labels = keep
            .iter()
            .map(|&i| if labels[i] == pos { 1.0 } else { -1.0 })
            .collect();
        Ok(Dataset {
            name: format!("{}: {} vs {}", self.name, names[pos], neg_name),
            features,
            feature_names: self.feature_names.clone(),
            targets: Targets::Binary {
                labels,
                positive: names[pos].clone(),
                negative: neg_name,
            },
        })
    }

    /// Features and targets for a trainer. Multi-class data must first be
    /// reduced with [`Dataset::binary`].
    pub fn training_data(&self) -> Result<TrainingData, DatasetError> {
        let (y, task) = match &self.targets {
            Targets::Values(v) => (v.clone(), Task::Regression),
            Targets::Binary { labels, .. } => (labels.clone(), Task::Classification),
            Targets::Classes { .. } => return Err(DatasetError::NeedsBinaryTask),
        };
        Ok(TrainingData {
            x: self.features.clone(),
            y: DVector::from_vec(y),
            task,
        })
    }

    /// Keep only the given feature columns.
    pub fn select_features(&self, columns: &[usize]) -> Dataset {
        Dataset {
            name: self.name.clone(),
            features: self.features.select_columns(columns),
            feature_names: columns
                .iter()
                .map(|&c| self.feature_names[c].clone())
                .collect(),
            targets: self.targets.clone(),
        }
    }

    /// A seeded random subset of at most `n` samples, in original order.
    pub fn subsample(&self, n: usize, seed: u64) -> Dataset {
        if n >= self.len() {
            return self.clone();
        }
        let mut idx: Vec<usize> = (0..self.len()).collect();
        idx.shuffle(&mut ChaCha8Rng::seed_from_u64(seed));
        idx.truncate(n);
        idx.sort_unstable();
        let targets = match &self.targets {
            Targets::Classes { labels, names } => Targets::Classes {
                labels: idx.iter().map(|&i| labels[i]).collect(),
                names: names.clone(),
            },
            Targets::Binary {
                labels,
                positive,
                negative,
            } => Targets::Binary {
                labels: idx.iter().map(|&i| labels[i]).collect(),
                positive: positive.clone(),
                negative: negative.clone(),
            },
            Targets::Values(v) => Targets::Values(idx.iter().map(|&i| v[i]).collect()),
        };
        Dataset {
            name: self.name.clone(),
            features: self.features.select_rows(&idx),
            feature_names: self.feature_names.clone(),
            targets,
        }
    }
}

/// Fisher's Iris data (scikit-learn's copy, which corrects two transcription
/// errors in the UCI file). Public domain.
const IRIS_CSV: &str = include_str!("../data/iris.csv");

pub fn iris() -> Dataset {
    parse_iris(IRIS_CSV).expect("bundled iris.csv is valid")
}

fn parse_iris(csv: &str) -> Result<Dataset, DatasetError> {
    let mut lines = csv.lines();
    let header: Vec<&str> = lines
        .next()
        .ok_or(DatasetError::Parse("empty".into()))?
        .split(',')
        .collect();
    let names: Vec<String> = header
        .iter()
        .skip(2)
        .map(|s| s.trim().to_string())
        .collect();
    let mut values = Vec::new();
    let mut labels = Vec::new();
    for (n, line) in lines.enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split(',').collect();
        let bad = || DatasetError::Parse(format!("row {}: {line:?}", n + 1));
        if cols.len() != 5 {
            return Err(bad());
        }
        for c in &cols[..4] {
            values.push(c.trim().parse::<f64>().map_err(|_| bad())?);
        }
        labels.push(cols[4].trim().parse::<usize>().map_err(|_| bad())?);
    }
    Ok(Dataset {
        name: "Iris".into(),
        features: DMatrix::from_row_slice(labels.len(), 4, &values),
        feature_names: [
            "sepal length (cm)",
            "sepal width (cm)",
            "petal length (cm)",
            "petal width (cm)",
        ]
        .map(String::from)
        .to_vec(),
        targets: Targets::Classes { labels, names },
    })
}

/// Standard normal sample by Box–Muller.
pub(crate) fn normal(rng: &mut ChaCha8Rng) -> f64 {
    let u1: f64 = 1.0 - rng.random::<f64>(); // (0, 1], avoids ln(0)
    let u2: f64 = rng.random();
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// Two 2-D Gaussian classes. When `separable`, samples are guaranteed to lie
/// strictly on their own side of the line `x + y = 0`, at least `gap` away.
pub fn blobs(n_per_class: usize, separable: bool, seed: u64) -> Dataset {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let (center, spread, gap) = if separable {
        (1.6, 0.6, 0.3)
    } else {
        (0.9, 0.9, 0.0)
    };
    let mut values = Vec::with_capacity(4 * n_per_class);
    let mut labels = Vec::with_capacity(2 * n_per_class);
    for y in [1.0, -1.0] {
        let mut made = 0;
        while made < n_per_class {
            let p = [
                y * center + spread * normal(&mut rng),
                y * center + spread * normal(&mut rng),
            ];
            // Signed distance to x + y = 0.
            if separable && y * (p[0] + p[1]) / 2f64.sqrt() < gap {
                continue;
            }
            values.extend_from_slice(&p);
            labels.push(y);
            made += 1;
        }
    }
    let kind = if separable {
        "separable"
    } else {
        "overlapping"
    };
    Dataset {
        name: format!("Blobs ({kind})"),
        features: DMatrix::from_row_slice(labels.len(), 2, &values),
        feature_names: vec!["x₁".into(), "x₂".into()],
        targets: Targets::Binary {
            labels,
            positive: "A".into(),
            negative: "B".into(),
        },
    }
}

/// Linear regression data `y = w·x + b + noise` with `x ∈ [−5, 5]^dims`.
/// Exactly `round(n · outlier_fraction)` points are outliers, pushed far above
/// the trend; their indices are returned alongside the data.
pub fn regression(
    n: usize,
    dims: usize,
    outlier_fraction: f64,
    seed: u64,
) -> (Dataset, Vec<usize>) {
    assert!(dims >= 1, "regression needs at least one feature");
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let w: Vec<f64> = (0..dims).map(|j| 0.8 - 0.5 * j as f64).collect();
    let b = 1.0;
    let mut x = Vec::with_capacity(n * dims);
    let mut y = Vec::with_capacity(n);
    for _ in 0..n {
        let row: Vec<f64> = (0..dims).map(|_| rng.random_range(-5.0..5.0)).collect();
        let t = row.iter().zip(&w).map(|(a, b)| a * b).sum::<f64>() + b;
        y.push(t + 0.5 * normal(&mut rng));
        x.extend(row);
    }
    let n_out = (n as f64 * outlier_fraction.clamp(0.0, 1.0)).round() as usize;
    let mut idx: Vec<usize> = (0..n).collect();
    idx.shuffle(&mut rng);
    let mut outliers = idx[..n_out].to_vec();
    outliers.sort_unstable();
    for &i in &outliers {
        y[i] += rng.random_range(8.0..14.0);
    }
    let dataset = Dataset {
        name: format!("Regression ({:.0}% outliers)", outlier_fraction * 100.0),
        features: DMatrix::from_row_slice(n, dims, &x),
        feature_names: (1..=dims).map(|j| format!("x{j}")).collect(),
        targets: Targets::Values(y),
    };
    (dataset, outliers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iris_shape() {
        let d = iris();
        assert_eq!(d.len(), 150);
        assert_eq!(d.dim(), 4);
        assert_eq!(d.feature_names.len(), 4);
        let Targets::Classes { labels, names } = &d.targets else {
            panic!()
        };
        assert_eq!(names, &["setosa", "versicolor", "virginica"]);
        for c in 0..3 {
            assert_eq!(labels.iter().filter(|&&l| l == c).count(), 50);
        }
        // First row of Fisher's data.
        assert_eq!(
            d.features.row(0).iter().copied().collect::<Vec<_>>(),
            [5.1, 3.5, 1.4, 0.2]
        );
    }

    #[test]
    fn iris_setosa_vs_versicolor() {
        let b = iris()
            .binary(&BinaryTask::Pair {
                positive: "setosa".into(),
                negative: "versicolor".into(),
            })
            .unwrap();
        assert_eq!(b.len(), 100);
        let Targets::Binary { labels, .. } = &b.targets else {
            panic!()
        };
        assert!(labels[..50].iter().all(|&y| y == 1.0));
        assert!(labels[50..].iter().all(|&y| y == -1.0));
    }

    #[test]
    fn one_vs_rest_keeps_everything() {
        let b = iris()
            .binary(&BinaryTask::OneVsRest {
                positive: "virginica".into(),
            })
            .unwrap();
        assert_eq!(b.len(), 150);
        assert_eq!(
            b.training_data()
                .unwrap()
                .y
                .iter()
                .filter(|&&y| y > 0.0)
                .count(),
            50
        );
    }

    #[test]
    fn binary_task_errors() {
        let d = iris();
        let pair = |a: &str, b: &str| BinaryTask::Pair {
            positive: a.into(),
            negative: b.into(),
        };
        assert_eq!(
            d.binary(&pair("setosa", "rose")),
            Err(DatasetError::UnknownClass("rose".into()))
        );
        assert!(matches!(
            d.binary(&pair("setosa", "setosa")),
            Err(DatasetError::SameClass(_))
        ));
        assert_eq!(d.training_data(), Err(DatasetError::NeedsBinaryTask));
    }

    #[test]
    fn separable_blobs_are_separable() {
        let d = blobs(100, true, 7);
        let t = d.training_data().unwrap();
        for i in 0..d.len() {
            assert!(t.y[i] * (t.x[(i, 0)] + t.x[(i, 1)]) > 0.0);
        }
    }

    #[test]
    fn outlier_contamination_is_exact_and_seeded() {
        let (d, out) = regression(100, 1, 0.1, 3);
        assert_eq!(out.len(), 10);
        assert_eq!(d.len(), 100);
        let (d2, out2) = regression(100, 1, 0.1, 3);
        assert_eq!(d, d2);
        assert_eq!(out, out2);
        assert_ne!(regression(100, 1, 0.1, 4).0, d);
    }

    #[test]
    fn subsample_is_seeded() {
        let d = iris();
        let a = d.subsample(30, 1);
        assert_eq!(a.len(), 30);
        assert_eq!(a, d.subsample(30, 1));
        assert_eq!(d.subsample(1000, 1), d);
    }
}
