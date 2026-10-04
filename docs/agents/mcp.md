# Using the bevaru MCP server

`bevaru-mcp` serves bevaru over the [Model Context Protocol](https://modelcontextprotocol.io) on stdio. Agents can then evaluate losses, build datasets, train and sweep models, and render charts without writing Rust or opening a window. It needs no GPU or display.

## Install

```sh
cargo install --git https://github.com/buddha314/bevaru bevaru-mcp
# with MNIST support (downloads ~11 MB once, when first used):
cargo install --git https://github.com/buddha314/bevaru bevaru-mcp --features mnist
```

From a checkout, `cargo run --release -p bevaru-mcp` runs it without installing.

## Connect a client

The server is a plain stdio command, `bevaru-mcp`, with no arguments.

**Claude Code:**

```sh
claude mcp add bevaru -- bevaru-mcp
```

**Clients configured with JSON** (Claude Desktop's `claude_desktop_config.json`, Cursor's `.cursor/mcp.json`, and others):

```json
{
  "mcpServers": {
    "bevaru": { "command": "bevaru-mcp" }
  }
}
```

**Codex** (`~/.codex/config.toml`):

```toml
[mcp_servers.bevaru]
command = "bevaru-mcp"
```

## Tools

| Tool | What it does |
| ---- | ------------ |
| `describe` | The capability manifest, or one section (`losses`, `models`, `datasets`, `views`, `sweep_parameters`, `experiences`, `messages`, `tools`, `schemas`). Call this first. |
| `list_experiences` | The lobby's experiences: id, title, summary, category, kind. |
| `evaluate_losses` | Loss values and (sub)gradients at points. |
| `build_dataset` | A dataset's displayed coordinates, labels, axis names, and PCA explained variance. |
| `train` | Train one model; returns the trajectory, final weights and bias, status, accuracy or RMSE, and support vectors for SVMs. |
| `sweep` | Train to convergence at each value of one hyperparameter. |
| `render_loss_chart` | Losses against their argument, as a PNG. |
| `render_training_chart` | A model's training objective by step, as a PNG. |

Full descriptions and input types are in the [capability reference](capabilities.md#tools); each input's JSON Schema is in [`capabilities.json`](capabilities.json), and clients receive it with the tool list.

### Examples

Evaluate hinge and logistic loss at a few margins:

```json
{ "losses": ["hinge", "logistic"], "points": [-1, 0, 0.5, 1, 2] }
```

Train a linear SVM with C = 10 on the overlapping Iris classes, using the two petal features shown in the lobby:

```json
{
  "dataset": { "kind": "iris", "positive": "versicolor", "negative": "virginica" },
  "view": { "kind": "features", "columns": [2, 3] },
  "train_on": "displayed-axes",
  "trainer": { "loss": "hinge", "c": 10, "max_steps": 4000 }
}
```

Sweep C for that model (same fields, plus `sweep`):

```json
{
  "dataset": { "kind": "iris", "positive": "versicolor", "negative": "virginica" },
  "trainer": { "loss": "hinge" },
  "sweep": { "parameter": "c", "from": 0.01, "to": 100, "samples": 9, "log": true }
}
```

## Driving a running window

With `--app <url>`, the server also offers `app_*` tools that control a bevaru window started with `cargo run --features remote -- --remote`. That includes opening experiences, playback, sweeps and reading state. See [drive a running app](recipes/drive-a-running-app.md).

```sh
bevaru-mcp --app http://127.0.0.1:15702
```

## Limits

They keep each call fast and its response small:
- at most 100 000 training steps per call;
- 2 to 50 sweep values;
- at most 10 000 points in `evaluate_losses`;
- trajectories downsampled to at most 500 points;
- `build_dataset` returns at most 5 000 points (`truncated` says when it cut).

## Resources

The server also serves its docs, embedded so they always match the server's version:
- `bevaru://capabilities.json`
- `bevaru://capabilities.md`
- `bevaru://docs/guide.md`
- `bevaru://docs/mcp.md`
- `bevaru://docs/recipes/train-headlessly.md`
- `bevaru://docs/recipes/add-an-experience.md`
- `bevaru://docs/recipes/drive-a-running-app.md`

## Errors

Invalid input comes back as a tool error naming the field and its valid range. For example: `trainer.huber_delta: invalid Huber delta = 0: must be finite and > 0`. The server keeps running; fix the field and call again.

## Safety

The server talks only over stdio and opens no network ports. The headless tools are read-only: they compute and return results, and change nothing on disk (MNIST, if enabled, is downloaded once into the user cache directory). The `app_*` tools exist only with `--app`; they make HTTP requests to that address, which is expected to be a local bevaru window.
