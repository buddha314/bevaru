# MCP Server Specification

## Purpose

Define `bevaru-mcp`, the Model Context Protocol server that gives agents headless tools for bevaru's losses, datasets, training, sweeps, and charts, plus its docs as resources.
## Requirements
### Requirement: Stdio MCP server
The workspace SHALL provide a `bevaru-mcp` binary that speaks the Model Context Protocol over stdio. It SHALL need no window or GPU, and it SHALL work with any MCP client that supports stdio servers.

#### Scenario: Client lists tools
- **WHEN** an MCP client connects to `bevaru-mcp` and lists its tools
- **THEN** it receives every headless tool with its description and JSON input schema

#### Scenario: No display available
- **WHEN** `bevaru-mcp` runs in an environment with no display or GPU
- **THEN** every headless tool works

### Requirement: Resources
The server SHALL expose the generated capability reference and every doc under `docs/agents/` as MCP resources, served from copies embedded in the binary, so they match the version of the server.

#### Scenario: Reading the manifest
- **WHEN** a client reads the `bevaru://capabilities.json` resource
- **THEN** it receives the same manifest that `gen-docs` writes to `docs/agents/capabilities.json`

### Requirement: Headless tools
The server SHALL provide tools to:
- describe capabilities (the whole manifest, or one section);
- list experiences;
- evaluate one or more losses over a set of points, with hyperparameters;
- build a dataset with a view and return its displayed coordinates and labels;
- train a model on a dataset to a step budget and return the loss trajectory, final parameters, status, and (for SVMs) support vectors;
- run a hyperparameter sweep and return the converged solution for each value;
- render a loss-curve or training chart as a PNG image.

Inputs SHALL be validated against the published schemas. Invalid input SHALL produce a tool error that names the field and the valid range, never a crash.

#### Scenario: Evaluate hinge loss
- **WHEN** a client calls the loss-evaluation tool with hinge loss, margin 1, at points [1.5, 0.25]
- **THEN** it returns values [0, 0.75] and gradients [0, −1]

#### Scenario: Train and inspect
- **WHEN** a client trains a linear SVM with C = 1 on separable blobs for 200 steps
- **THEN** it returns 200 loss values (or fewer if training converged), the final weights and bias, the status, and at least one support vector

#### Scenario: Invalid hyperparameter
- **WHEN** a client asks for Huber loss with δ = 0
- **THEN** the tool returns an error naming the Huber δ and its valid range, and the server keeps serving

#### Scenario: Chart as an image
- **WHEN** a client asks for a chart of classification losses
- **THEN** it receives PNG image content showing those losses

### Requirement: Tools and manifest agree
Every tool SHALL correspond to a manifest entry, and a test SHALL check this in both directions.

#### Scenario: Undocumented tool
- **WHEN** a tool is added to the server without a manifest entry
- **THEN** the consistency test fails, naming the tool

### Requirement: Loss-shape sampling tool
The MCP server SHALL provide a headless tool that samples a loss-shape view by id, with optional hyperparameters, resolution (at most 101 × 101), and entropy-removed mode for cross-entropy. It SHALL return:
- the axis names and sample coordinates;
- the grid of values;
- which points were clipped;
- the caption;
- the slice that equals the 2-D curve.

Unknown view ids and invalid hyperparameters SHALL be tool errors naming the field.

#### Scenario: Sample the cross-entropy surface
- **WHEN** a client samples the binary cross-entropy view at resolution 21
- **THEN** it receives a 21 × 21 grid whose values along q = p equal the entropy H(p), with the clip cap and caption

#### Scenario: Unknown view
- **WHEN** a client asks for a view id that doesn't exist
- **THEN** the tool returns an error listing the valid view ids

### Requirement: Loss-shape rendering tool
The MCP server SHALL provide a headless tool that renders a loss-shape view as a 3-D surface PNG. It SHALL:
- take the same inputs as the sampling tool, plus an optional camera azimuth and elevation;
- use the same cool-to-warm colormap as the interactive view;
- label the axes and the height;
- give a triangular base for the three-class probability view.

#### Scenario: Render the cross-entropy surface
- **WHEN** a client renders the binary cross-entropy view
- **THEN** it receives a PNG image showing the clipped surface with labelled p, q, and loss axes

#### Scenario: No display needed
- **WHEN** the rendering tool runs with no display or GPU
- **THEN** it still returns the PNG

