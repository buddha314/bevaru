## ADDED Requirements

### Requirement: Remote control of a running app
Behind an opt-in `remote` cargo feature, the bevaru app SHALL serve Bevy Remote Protocol (BRP) methods to:
- list experiences;
- enter an experience by id;
- leave to the lobby;
- send playback commands;
- start or stop a hyperparameter sweep;
- read a summary of the current state (screen, active experience, panes and their configurations, step, loss, sweep progress).

The server SHALL bind to localhost only, and SHALL be enabled only with `--remote` on the `bevaru` binary or by adding the plugin.

#### Scenario: Enter an experience remotely
- **WHEN** a client sends the BRP method to enter `iris-svm` to an app started with `--remote`
- **THEN** the app leaves the lobby, loads the Iris experience, and a later state query reports it running

#### Scenario: Remote off by default
- **WHEN** the app is built without the `remote` feature, or started without `--remote`
- **THEN** no network port is opened

#### Scenario: Unknown experience
- **WHEN** a client asks to enter an id that isn't registered
- **THEN** the method returns a JSON-RPC error listing the valid ids, and the app stays where it was

### Requirement: MCP bridge to a running app
`bevaru-mcp --app <url>` SHALL expose the remote methods as additional MCP tools. Without `--app`, those tools SHALL NOT be listed.

#### Scenario: Agent drives the window
- **WHEN** an MCP client connected to `bevaru-mcp --app http://127.0.0.1:15702` calls the enter-experience tool with `loss-curves`
- **THEN** the running window switches to the loss-curves experience

#### Scenario: App not running
- **WHEN** a remote tool is called and nothing answers at the URL
- **THEN** the tool returns an error saying the app isn't reachable at that URL, and how to start it with `--remote`
