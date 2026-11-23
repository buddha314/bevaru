## ADDED Requirements

### Requirement: Qubit state and Bloch vectors
`bevaru-core` SHALL provide a state vector for a small number of qubits, starting in |0…0⟩. For any qubit, the state vector SHALL return that qubit's Bloch vector r = (⟨X⟩, ⟨Y⟩, ⟨Z⟩), computed from its reduced density matrix, so |r| = 1 for a qubit in a pure product state and |r| < 1 for an entangled one. |0⟩ SHALL map to +z and |1⟩ to −z. The module SHALL NOT depend on Bevy.

#### Scenario: Basis states
- **WHEN** the Bloch vector of a fresh one-qubit state is read
- **THEN** it is (0, 0, 1), and after a full X rotation it is (0, 0, −1)

#### Scenario: Entangled qubit
- **WHEN** both qubits of a Bell state are read
- **THEN** each Bloch vector is (0, 0, 0)

### Requirement: Gates as rotation generators
A gate SHALL be described by its generator G, with the gate equal to exp(−iG) up to a global phase, so that it can be applied partially as exp(−i·s·G) for s in [0, 1]. Single-qubit gates SHALL use a generator a·σ on one qubit (RX, RY, RZ, phase, and Hadamard as a rotation by π about (x+z)/√2). The two-qubit entangler SHALL use c·Z⊗Z. Every single-qubit gate SHALL report its Bloch-sphere rotation axis and angle. Applying a gate with s = 1 SHALL equal the full gate up to a global phase.

#### Scenario: Hadamard
- **WHEN** the Hadamard generator is applied fully to |0⟩
- **THEN** the Bloch vector is (1, 0, 0), and halfway (s = 0.5) the vector lies on the great circle about the axis (1, 0, 1)/√2

#### Scenario: Partial rotation
- **WHEN** RY(θ) is applied with s = 0.5 to |0⟩
- **THEN** the result equals RY(θ/2)|0⟩

### Requirement: Encoding catalogue
The module SHALL provide a catalogue of data encodings. Each has a stable kebab-case id, a name, a formula, a qubit count, and named features with ranges and defaults, and maps feature values to a circuit (a sequence of labelled gates). It SHALL include at least:

| id | qubits | features | circuit |
| -- | ------ | -------- | ------- |
| `basis` | 1 | bit b | RX(πb) |
| `angle-ry` | 1 | x | RY(x) |
| `angle-rx` | 1 | x | RX(x) |
| `phase` | 1 | x | H, then RZ(x) |
| `dense-angle` | 1 | x₁, x₂ | RY(x₁), then RZ(x₂) |
| `amplitude` | 1 | x₁, x₂ | RY(2·atan2(x₂, x₁)) |
| `reuploading` | 1 | x | (RY(x), RZ(x)) repeated 3 times |
| `zz-feature-map` | 2 | x₁, x₂ | H⊗H, RZ(2x₁)⊗RZ(2x₂), exp(−i(π−x₁)(π−x₂)Z⊗Z) |

Ids SHALL be unique and kebab-case.

#### Scenario: Angle encoding stays on a meridian
- **WHEN** `angle-ry` encodes any x
- **THEN** the Bloch vector is (sin x, 0, cos x)

#### Scenario: Phase encoding stays on the equator
- **WHEN** `phase` encodes any x
- **THEN** the Bloch vector is (cos x, sin x, 0)

#### Scenario: Amplitude encoding forgets the norm
- **WHEN** `amplitude` encodes (x₁, x₂) and (2x₁, 2x₂)
- **THEN** the two states are equal

#### Scenario: The ZZ feature map entangles
- **WHEN** `zz-feature-map` encodes features at which the ZZ phase is not a multiple of π/2
- **THEN** each qubit's Bloch vector has length strictly less than 1

### Requirement: Gate paths and encoding loci
The module SHALL compute, for each qubit:
- the **gate path** of an encoding at fixed features: the Bloch vector sampled while each gate is applied from s = 0 to 1 in turn, starting at |0…0⟩;
- the **locus** of an encoding: the Bloch vector of the fully encoded state as one chosen feature sweeps its range, with the other features held fixed.

The last point of a gate path SHALL equal the encoded state's Bloch vector.

#### Scenario: Gate path ends at the encoding
- **WHEN** the gate path of any encoding at its default features is computed
- **THEN** its final point matches the Bloch vector of the encoded state to 1e-9

#### Scenario: Loci of the single-feature rotations
- **WHEN** the locus of `angle-ry` over its range and of `phase` over its range are computed
- **THEN** every `angle-ry` point has y = 0, and every `phase` point has z = 0
