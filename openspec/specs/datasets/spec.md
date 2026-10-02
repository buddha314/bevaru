# Datasets Specification

## Purpose

Define the datasets, task derivation, and projections available to Bevaru.

## Requirements

### Requirement: Common dataset representation
All datasets SHALL be exposed through one `Dataset` type holding a feature matrix, targets (class labels or real values), feature names, and class names where applicable.

#### Scenario: Iris shape
- **WHEN** the Iris dataset is loaded
- **THEN** it has 150 samples, 4 named features, and 3 classes of 50 samples each

### Requirement: Bundled Iris
The Iris dataset SHALL be embedded in the crate so it loads offline with no I/O.

#### Scenario: Offline load
- **WHEN** Iris is loaded with networking unavailable
- **THEN** it loads successfully

### Requirement: Binary task derivation
Multi-class datasets SHALL support deriving a binary classification task, either one class versus another or one versus rest, with labels mapped to `{−1, +1}`.

#### Scenario: Iris setosa vs versicolor
- **WHEN** a binary task "setosa vs versicolor" is derived from Iris
- **THEN** it has 100 samples, with setosa labelled `+1` and versicolor `−1`

#### Scenario: MNIST digit pair
- **WHEN** a binary task "3 vs 8" is derived from MNIST
- **THEN** it contains only samples of digits 3 and 8

### Requirement: On-demand MNIST
MNIST SHALL be available behind an `mnist` cargo feature. On first use it SHALL be downloaded, verified against pinned SHA-256 checksums, and cached in a user cache directory; subsequent loads SHALL read only the cache. A configurable subsample size SHALL be supported, with a fixed seed for reproducibility.

#### Scenario: Checksum mismatch
- **WHEN** a downloaded MNIST file does not match its pinned checksum
- **THEN** loading fails with an error naming the file, and the bad file is not cached

#### Scenario: Cached reload
- **WHEN** MNIST is loaded a second time
- **THEN** no network request is made

#### Scenario: Feature disabled
- **WHEN** the crate is built without the `mnist` feature
- **THEN** it compiles with no HTTP dependency and Iris remains available

### Requirement: Synthetic generators
The core crate SHALL provide seeded generators for: linearly separable 2-class blobs, overlapping 2-class blobs, and 1D/2D linear regression data with a configurable fraction of outliers.

#### Scenario: Outlier contamination
- **WHEN** regression data is generated with 100 points and outlier fraction `0.1`
- **THEN** exactly 10 points are outliers and the same seed reproduces the same data

### Requirement: Projection for display
The system SHALL project datasets to 2D (or 3D) for rendering, by selecting feature columns or by PCA. Models SHALL be trainable either in the projected space or in the original space; when trained in the original space, the displayed boundary SHALL be the model's decision boundary restricted to the projected plane, and the UI SHALL label it as a projection.

#### Scenario: Feature-pair projection
- **WHEN** Iris is displayed with features (petal length, petal width)
- **THEN** each point is positioned at exactly those two feature values

#### Scenario: PCA projection of MNIST
- **WHEN** a 784-dimensional MNIST subset is projected by PCA to 2D
- **THEN** the two components are orthonormal and ordered by explained variance, which is reported to the UI
