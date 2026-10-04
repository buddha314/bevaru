# Recipe: drive a running app

Control a visible bevaru window from an agent or a script: open experiences, play or step training, run sweeps, and read the app's state. This uses [Bevy Remote Protocol](https://docs.rs/bevy_remote) (JSON-RPC 2.0 over HTTP), served on 127.0.0.1 only and only when you ask for it.

## 1. Start the app with remote control

```sh
cargo run --release --features remote -- --remote          # http://127.0.0.1:15702
cargo run --release --features remote -- --remote-port 16000
```

Without the `remote` feature, or without `--remote`, no port is opened.

## 2a. From an MCP client

Run the MCP server pointed at the app; it adds `app_*` tools beside the headless ones:

```sh
bevaru-mcp --app http://127.0.0.1:15702
# Claude Code: claude mcp add bevaru-app -- bevaru-mcp --app http://127.0.0.1:15702
```

| Tool | Does |
| ---- | ---- |
| `app_list_experiences` | List the app's experiences. |
| `app_enter_experience` | `{"id": "iris-svm"}`: open one (leaving the current one). |
| `app_leave_experience` | Back to the lobby. |
| `app_playback` | `{"command": "play"}`, `"pause"`, `"toggle"`, `"step"`, `"reset"`, or `{"seek": {"step": 40}}`. |
| `app_start_sweep` | `{"parameter": "c", "from": 0.01, "to": 100, "samples": 9, "log": true}`. |
| `app_stop_sweep` | Stop the sweep. |
| `app_state` | Screen, active experience, panes and their configurations, step, objective, sweep progress. |

If nothing is listening, the tools say so and how to start the app.

## 2b. Directly, with any HTTP client

Each tool is one JSON-RPC method (listed in the [reference](../capabilities.md#remote-methods)):

```sh
curl -s -X POST http://127.0.0.1:15702 \
  -d '{"jsonrpc":"2.0","id":1,"method":"bevaru.experiences.enter","params":{"id":"iris-svm"}}'
# {"jsonrpc":"2.0","id":1,"result":{"entering":"iris-svm"}}

curl -s -X POST http://127.0.0.1:15702 \
  -d '{"jsonrpc":"2.0","id":2,"method":"bevaru.state"}'
```

Entering is asynchronous: the experiment loads off the main thread, so poll `bevaru.state` until `screen` is `"running"`. An unknown id is a JSON-RPC error whose `data.valid_ids` lists the registered experiences. BRP's built-in `world.*` methods (inspecting entities and components) work on the same port.

## Notes

- Remote commands go through the same paths as the UI, so leaving an experience cleans up exactly as `Esc` does.
- `--remote` is for local use. Don't forward the port to other machines: anything that can reach it can control the app.
