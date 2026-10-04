## ADDED Requirements

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
