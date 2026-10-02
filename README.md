# bevaru

`bevaru` is a Bevy plugin for interactive, animated machine-learning visualizations, with charts drawn by [ruviz](https://github.com/Ameyanagi/ruviz).

The first subject is loss functions. You can watch how MSE, MAE, Huber, hinge, squared hinge, logistic and 0-1 loss, and their hyperparameters, move the fitted model:
- the decision line or plane,
- SVM margins and support vectors,
- regression fits and their residuals.

The data can be synthetic, Iris, or MNIST.

![Iris: a hinge-loss SVM during a C sweep, beside logistic regression](docs/images/iris.webp)

## Examples

Run these commands from a checkout of the current `main` branch. If Cargo lists only `ml_interactive`, update your checkout: the visualizer examples were added later.

`cargo run` launches the Iris SVM visualizer by default. Use `cargo run --release` for smoother animation, or `cargo run --release --example iris_svm` to select it explicitly.

| Example | Shows |
| ------- | ----- |
| `cargo run --release --example iris_svm` | Versicolor vs virginica. A linear SVM's margin narrows and its support vectors thin out as C sweeps from 0.01 to 100. Logistic regression sits beside it as a reference. |
| `cargo run --release --example regression_mse_vs_mae` | 15% outliers. MSE is dragged toward them, MAE ignores them, and Huber lands in between depending on δ. |
| `cargo run --release --example loss_curves` | Every loss plotted against margin or residual. Edit δ and the hinge margin live while an SVM trains. |
| `cargo run --release --features mnist --example mnist_svm` | 3 vs 8 in all 784 pixels. The scene shows the PCA view; a 28 × 28 image shows the weights the model learned. |
| `cargo run --example ml_interactive` | The original scaffold: a ruviz sigmoid plot shown as a sprite. |

![MSE, MAE and Huber fits on data with outliers](docs/images/regression_mse_vs_mae.webp)

**In the app:**
- **Control panel (left):** pick the data, view, losses and hyperparameters. Add up to three side-by-side panes, play, pause, step or scrub training, and run hyperparameter sweeps.
- **Chart panel (right):** loss curves, the training objective, and learned weights.
- **Mouse:** in 2D, drag to pan and scroll to zoom. In 3D, left-drag to orbit and right-drag to pan.
- **Keys:** `Space` play/pause · `S` step · `R` reset · `F` frame data.

## Visual verification walkthrough

Run each example from the repository root. The first build may take a few minutes.

1. **Iris SVM and C sweep:** Run `cargo run --release --example iris_svm`. The automatic sweep moves `C` from 0.01 to 100: the SVM boundary and dashed margins should move, support vectors should have gold rings, and the logistic-regression pane should stay fixed. During the sweep, click **+ Compare (add pane)**; three panes should appear and the sweep should stop without a crash. Drag the `C` slider, switch **Hinge** to **Squared hinge**, and press **Play** to watch training continue.
2. **Regression with outliers:** Run `cargo run --release --example regression_mse_vs_mae`. MSE should lean toward the high outliers, MAE should stay nearer the main trend, and Huber should lie between. Drag **Huber δ** to update its fit and loss curve. In **Display**, toggle **Residuals** and check that the vertical segments appear and disappear.
3. **Loss curves and playback:** Run `cargo run --release --example loss_curves`. Both classification and regression loss charts should show their legends. Drag the **margin** slider in **Models** and watch the hinge curve change. Press `Space` to pause or play, `S` to step, and `R` to reset. Scrub the step slider backward; the boundary and training-chart marker should follow the selected step.
4. **MNIST:** Run `cargo run --release --features mnist --example mnist_svm`. The first run downloads and verifies MNIST; `BEVARU_MNIST_DIR` can point to a folder with the four original `*-ubyte.gz` files instead. The 3-vs-8 scene should label its PCA boundary as a **projection**, and the right panel should show a 28 × 28 weight image.

For a 3-D camera check, run Iris, select **3-D (third feature)**, and click **Load**. Left-drag to orbit, right-drag to pan, and scroll to zoom. Press `F` or click **Frame data** to fit every point in view again. Switch back to 2-D to check **Decision regions**; toggle **SVM margins** in either view.

To save a screenshot automatically and exit after eight seconds, for example:

```sh
BEVARU_SCREENSHOT=iris.png BEVARU_SCREENSHOT_AFTER=8 cargo run --release --example iris_svm
```

![MNIST 3 vs 8: the boundary's slice through the top two principal components, and the learned weights](docs/images/mnist.webp)

## Use it in your own app

```toml
[dependencies]
bevaru = { git = "https://github.com/buddha314/bevaru" }
```

```rust
use bevaru::core::TrainerConfig;
use bevaru::{BevaruPlugin, DatasetChoice, ExperimentSpec, StartupExperiment};
use bevy::prelude::*;

fn main() {
    let spec = ExperimentSpec::new(
        DatasetChoice::Iris { positive: "versicolor".into(), negative: Some("virginica".into()) },
        TrainerConfig::svm(1.0).unwrap(),
    )
    .compare(TrainerConfig::logistic());

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(StartupExperiment(spec))
        .add_plugins(BevaruPlugin) // after DefaultPlugins
        .run();
}
```

**Driving it from code:**
- Send `PlaybackCommand` (`Play`, `Step`, `Seek(n)`, …) to control training playback.
- Send `SweepCommand::Start(SweepSpec { .. })` to run a hyperparameter sweep.
- Send `LoadExperiment(spec)` to switch to new data.

`BevaruPlugin` adapts to the app it's added to:
- **After `DefaultPlugins`:** it adds the scene, charts and egui control panel.
- **After `MinimalPlugins`:** it runs headless — experiments, training and sweeps work with nothing rendered.

## Layout

| Crate | Contents |
| ----- | -------- |
| `crates/bevaru-core` | Everything without Bevy: losses with their gradients, linear models trained one step at a time (with step history), datasets, and PCA. |
| `bevaru` (root) | The Bevy plugin: scene, ruviz charts, playback and sweeps, and the egui controls. |

![A 3-D view: the SVM decision plane and margins over three Iris features](docs/images/iris3d.webp)

## MNIST

MNIST is optional, behind the `mnist` cargo feature.
- **Download and checks:** on first use it downloads about 11 MB, checks each file against fixed SHA-256 checksums, and caches it in your user cache directory.
- **Existing copy:** set `BEVARU_MNIST_DIR` to a folder containing the four original `*-ubyte.gz` files to use them instead.
- **Real-data test:** this test is skipped by default because it needs the data.

  ```sh
  cargo test -p bevaru-core --features mnist --release -- --ignored
  ```

## Datasets and credits

- **Iris:** Fisher's Iris data (R. A. Fisher, 1936), public domain. The bundled copy is scikit-learn's, which fixes two errors in the UCI file.
- **MNIST:** Y. LeCun, C. Cortes and C. J. C. Burges, *The MNIST database of handwritten digits*.
- **Colours:** the Okabe–Ito palette, which stays distinguishable with common colour-vision deficiencies.
