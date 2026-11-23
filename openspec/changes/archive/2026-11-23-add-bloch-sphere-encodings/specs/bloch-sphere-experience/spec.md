## ADDED Requirements

### Requirement: Bloch sphere experience
bevaru SHALL register a custom experience with id `bloch-sphere`, titled "Bloch sphere: data encodings", in the lobby category "Quantum computing". It SHALL have an embedded thumbnail and an `--example bloch_sphere` wrapper. It SHALL draw one Bloch sphere per qubit of the selected encoding, each with:
- a translucent sphere;
- x, y and z axes;
- the equator and two meridians;
- labels for |0⟩, |1⟩, |+⟩, |−⟩, |+i⟩, and |−i⟩;
- an arrow for the qubit's current Bloch vector.

The camera SHALL be the orbit rig, with Z (|0⟩) up.

#### Scenario: Start
- **WHEN** `bloch-sphere` is started
- **THEN** one sphere with its state arrow is shown for the default encoding (`angle-ry`), and the arrow starts at |0⟩

#### Scenario: Two-qubit encoding
- **WHEN** `zz-feature-map` is selected
- **THEN** two spheres are shown side by side, and each arrow is that qubit's reduced Bloch vector, so it can lie inside its sphere

### Requirement: Gate-by-gate animation
In gate-by-gate mode, the experience SHALL animate the selected encoding's circuit at the current feature values:
- each gate in turn rotates the state continuously from s = 0 to 1, along the gate path;
- the path travelled so far is drawn as a trail;
- while a single-qubit gate runs, its rotation axis is drawn through that qubit's sphere;
- the current gate's label is shown.

Playback SHALL loop, and SHALL support play/pause, speed, and restart.

#### Scenario: Animation reaches the encoded state
- **WHEN** gate-by-gate playback runs to the end of the circuit
- **THEN** every arrow equals the encoded state's Bloch vector

#### Scenario: Pause holds the state
- **WHEN** playback is paused
- **THEN** time passing does not move the arrows

### Requirement: Data-sweep animation
In sweep mode, the experience SHALL sweep one chosen feature across its range, with the other features held at their current values, and SHALL move each arrow to the encoded state at the current value. Each sphere SHALL show the full locus faintly and the part already traced brightly.

#### Scenario: Phase sweep traces the equator
- **WHEN** `phase` is swept
- **THEN** the drawn locus lies in the equatorial plane

### Requirement: Controls
A control panel SHALL let the student:
- choose the encoding;
- set each feature with a slider over its range;
- choose the mode and, in sweep mode, the swept feature;
- play, pause, restart, and set the speed;
- toggle the trail or locus and the labels;
- reset the view.

It SHALL show the encoding's formula. Changing the encoding or a feature SHALL restart the animation from |0…0⟩.

#### Scenario: Changing encoding
- **WHEN** the encoding changes from `angle-ry` to `zz-feature-map`
- **THEN** the scene shows two spheres and the animation restarts at |00⟩

### Requirement: Clean leave
Leaving `bloch-sphere` SHALL remove every entity and resource it created, as for every experience.

#### Scenario: Round trips
- **WHEN** the lobby round-trip test enters and leaves `bloch-sphere` ten times
- **THEN** entity, mesh, material, and image counts return to their baseline and no `bloch-sphere` resource remains
