//! Projecting data to 2-D or 3-D for display, and restricting a model trained
//! in the full space to the displayed plane.

use nalgebra::{DMatrix, DVector, SymmetricEigen};

use crate::model::LinearModel;

/// An affine map from the data space to display coordinates:
/// `z = C (x − origin)`, with orthonormal rows in `C`. A displayed point `z`
/// corresponds to the data-space point `origin + Cᵀ z`.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    pub kind: ProjectionKind,
    /// `k × d`, orthonormal rows.
    pub components: DMatrix<f64>,
    /// Data-space point the display plane passes through (the data mean).
    pub origin: DVector<f64>,
    /// Display axis labels.
    pub axis_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectionKind {
    /// Selected feature columns; the remaining features are held at their mean.
    Features(Vec<usize>),
    /// Principal components, with each component's fraction of total variance.
    Pca { explained_variance_ratio: Vec<f64> },
}

impl Projection {
    /// Display the given feature columns of `x`.
    pub fn features(x: &DMatrix<f64>, columns: &[usize], feature_names: &[String]) -> Self {
        let d = x.ncols();
        let mut components = DMatrix::zeros(columns.len(), d);
        for (r, &c) in columns.iter().enumerate() {
            components[(r, c)] = 1.0;
        }
        Self {
            kind: ProjectionKind::Features(columns.to_vec()),
            components,
            origin: column_mean(x),
            axis_names: columns.iter().map(|&c| feature_names[c].clone()).collect(),
        }
    }

    /// Fit the top `k` principal components of `x`.
    pub fn pca(x: &DMatrix<f64>, k: usize) -> Self {
        let (n, d) = x.shape();
        assert!(k >= 1 && k <= d, "PCA needs 1 ≤ k ≤ {d}, got {k}");
        let mean = column_mean(x);
        let mut centered = x.clone();
        for mut row in centered.row_iter_mut() {
            row -= mean.transpose();
        }
        let cov = centered.transpose() * &centered / (n.max(2) - 1) as f64;
        let total = cov.trace();
        let (values, vectors) = top_eigenpairs(&cov, k);
        let mut components = DMatrix::zeros(k, d);
        let mut ratio = Vec::with_capacity(k);
        for r in 0..k {
            let mut v = vectors.column(r).into_owned();
            // Fix the sign so results are reproducible across platforms.
            if let Some(m) = v.iter().copied().max_by(|a, b| a.abs().total_cmp(&b.abs()))
                && m < 0.0
            {
                v = -v;
            }
            components.set_row(r, &v.transpose());
            ratio.push(if total > 0.0 {
                values[r].max(0.0) / total
            } else {
                0.0
            });
        }
        Self {
            kind: ProjectionKind::Pca {
                explained_variance_ratio: ratio,
            },
            components,
            origin: mean,
            axis_names: (1..=k).map(|i| format!("PC{i}")).collect(),
        }
    }

    pub fn dims(&self) -> usize {
        self.components.nrows()
    }

    /// Project every row of `x`: returns `n × k`.
    pub fn project(&self, x: &DMatrix<f64>) -> DMatrix<f64> {
        let mut centered = x.clone();
        for mut row in centered.row_iter_mut() {
            row -= self.origin.transpose();
        }
        centered * self.components.transpose()
    }

    /// The model's decision function restricted to the display plane, in
    /// display coordinates. Exact for every point on the plane: for
    /// `x = origin + Cᵀz`, `w·x + b = (Cw)·z + (w·origin + b)`.
    pub fn restrict(&self, model: &LinearModel) -> LinearModel {
        LinearModel {
            w: &self.components * &model.w,
            b: model.w.dot(&self.origin) + model.b,
        }
    }

    /// True when the display shows every dimension the model uses, so the
    /// drawn boundary is the model's actual boundary rather than a slice.
    pub fn is_exact(&self) -> bool {
        self.components.nrows() == self.components.ncols()
    }
}

/// The `k` largest eigenpairs of a symmetric PSD matrix, largest first.
/// Small matrices are decomposed exactly; large ones (MNIST's 784 × 784) use
/// subspace iteration with a Rayleigh–Ritz step, which needs only `k` columns.
fn top_eigenpairs(cov: &DMatrix<f64>, k: usize) -> (Vec<f64>, DMatrix<f64>) {
    let d = cov.nrows();
    let (values, vectors) = if d <= 64 {
        let eig = SymmetricEigen::new(cov.clone());
        (eig.eigenvalues, eig.eigenvectors)
    } else {
        // Oversample the subspace for faster, more reliable convergence.
        let p = (k + 8).min(d);
        // Deterministic, non-degenerate start.
        let mut q = DMatrix::from_fn(d, p, |i, j| {
            ((i * 7 + j * 13 + 1) % 17) as f64 - 8.0 + (i == j) as u8 as f64
        });
        q = q.qr().q();
        for _ in 0..500 {
            let next = (cov * &q).qr().q();
            // Converged when the subspace stops moving.
            let moved = (&next * (next.transpose() * &q) - &q).abs().max();
            q = next;
            if moved < 1e-10 {
                break;
            }
        }
        let small = SymmetricEigen::new(q.transpose() * cov * &q);
        (small.eigenvalues, &q * small.eigenvectors)
    };
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[b].total_cmp(&values[a]));
    order.truncate(k);
    let top = DMatrix::from_columns(&order.iter().map(|&i| vectors.column(i)).collect::<Vec<_>>());
    (order.iter().map(|&i| values[i]).collect(), top)
}

fn column_mean(x: &DMatrix<f64>) -> DVector<f64> {
    let n = x.nrows().max(1) as f64;
    DVector::from_iterator(x.ncols(), x.column_iter().map(|c| c.sum() / n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::iris;

    #[test]
    fn feature_pair_projection_is_exact_positions() {
        let d = iris();
        let p = Projection::features(&d.features, &[2, 3], &d.feature_names);
        let z = p.project(&d.features);
        // Feature projection is centered; adding the origin back gives the raw values.
        for i in 0..d.len() {
            assert!((z[(i, 0)] + p.origin[2] - d.features[(i, 2)]).abs() < 1e-12);
            assert!((z[(i, 1)] + p.origin[3] - d.features[(i, 3)]).abs() < 1e-12);
        }
        assert_eq!(p.axis_names, ["petal length (cm)", "petal width (cm)"]);
    }

    #[test]
    fn pca_components_orthonormal_and_ordered() {
        let d = iris();
        let p = Projection::pca(&d.features, 3);
        let gram = &p.components * p.components.transpose();
        assert!((gram - DMatrix::identity(3, 3)).abs().max() < 1e-10);
        let ProjectionKind::Pca {
            explained_variance_ratio: r,
        } = &p.kind
        else {
            panic!()
        };
        assert!(r[0] >= r[1] && r[1] >= r[2]);
        // Iris PC1 explains ~92.5% of variance.
        assert!((r[0] - 0.9246).abs() < 1e-3, "PC1 ratio {}", r[0]);
    }

    #[test]
    fn subspace_iteration_matches_exact_pca() {
        // 100-D data with a planted 3-D structure: exercises the large-d path.
        use rand::{RngExt, SeedableRng};
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(3);
        let (n, d) = (400, 100);
        let x = DMatrix::from_fn(n, d, |_, j| {
            let scale = match j {
                0 => 9.0,
                1 => 5.0,
                2 => 3.0,
                _ => 0.3,
            };
            scale * rng.random_range(-1.0..1.0)
        });
        let p = Projection::pca(&x, 2);
        let exact = SymmetricEigen::new({
            let mean = column_mean(&x);
            let mut c = x.clone();
            for mut row in c.row_iter_mut() {
                row -= mean.transpose();
            }
            c.transpose() * &c / (n - 1) as f64
        });
        let mut order: Vec<usize> = (0..d).collect();
        order.sort_by(|&a, &b| exact.eigenvalues[b].total_cmp(&exact.eigenvalues[a]));
        for r in 0..2 {
            let exact = exact.eigenvectors.column(order[r]);
            let dot = p.components.row(r).transpose().dot(&exact).abs();
            assert!((dot - 1.0).abs() < 1e-8, "component {r}: |cos| = {dot}");
        }
        let gram = &p.components * p.components.transpose();
        assert!((gram - DMatrix::identity(2, 2)).abs().max() < 1e-10);
    }

    #[test]
    fn restriction_matches_decision_function_on_plane() {
        let d = iris();
        let p = Projection::pca(&d.features, 2);
        let m = LinearModel {
            w: DVector::from_vec(vec![0.3, -1.2, 0.7, 2.0]),
            b: -0.4,
        };
        let r = p.restrict(&m);
        let z = DVector::from_vec(vec![0.8, -1.5]);
        let x = &p.origin + p.components.transpose() * &z;
        assert!((m.decision(&x) - r.decision(&z)).abs() < 1e-10);
        assert!(!p.is_exact());
    }
}
