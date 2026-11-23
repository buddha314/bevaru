//! Qubits on the Bloch sphere, and the common ways to encode classical data
//! into them.
//!
//! A [`Gate`] is stored as its *generator* G, with the gate equal to
//! exp(−iG) up to a global phase. Applying it partially, as exp(−i·s·G) for s
//! in [0, 1], is exact, so a gate's rotation can be animated by running s
//! from 0 to 1. Single-qubit generators are a·σ on one qubit, a rotation of
//! the Bloch sphere by 2|a| about â; the two-qubit entangler is c·Z⊗Z.
//!
//! [`StateVector`] holds 2ⁿ amplitudes (qubit q is bit q of the index) and
//! returns each qubit's Bloch vector from its reduced density matrix, so an
//! entangled qubit's vector lies inside the sphere.
//!
//! [`Encoding`] is a catalogue of data encodings (angle, phase, dense angle,
//! amplitude, basis, data re-uploading, and the ZZ feature map), each mapping
//! feature values to a circuit. [`gate_path`] samples the Bloch vectors while
//! the circuit runs; [`locus`] samples the encoded state as one feature
//! sweeps its range.

use std::f64::consts::{FRAC_1_SQRT_2, PI};

use nalgebra::Complex;

pub type C64 = Complex<f64>;

/// A Bloch vector (⟨X⟩, ⟨Y⟩, ⟨Z⟩); |0⟩ is +z.
pub type Bloch = [f64; 3];

/// What a gate does, as the generator G of exp(−iG).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Generator {
    /// a·σ on one qubit: a rotation by 2|a| about â.
    Single { qubit: usize, a: [f64; 3] },
    /// c·Z⊗Z on two qubits: a phase that depends on their parity.
    ZZ { qubits: (usize, usize), c: f64 },
}

/// A labelled gate.
#[derive(Debug, Clone, PartialEq)]
pub struct Gate {
    pub label: String,
    pub generator: Generator,
}

impl Gate {
    fn single(label: String, qubit: usize, a: [f64; 3]) -> Self {
        Self {
            label,
            generator: Generator::Single { qubit, a },
        }
    }

    /// RX(θ) = exp(−iθX/2).
    pub fn rx(qubit: usize, theta: f64) -> Self {
        Self::single(format!("RX({theta:.2})"), qubit, [theta / 2.0, 0.0, 0.0])
    }

    /// RY(θ) = exp(−iθY/2).
    pub fn ry(qubit: usize, theta: f64) -> Self {
        Self::single(format!("RY({theta:.2})"), qubit, [0.0, theta / 2.0, 0.0])
    }

    /// RZ(θ) = exp(−iθZ/2); the phase gate P(θ) up to a global phase.
    pub fn rz(qubit: usize, theta: f64) -> Self {
        Self::single(format!("RZ({theta:.2})"), qubit, [0.0, 0.0, theta / 2.0])
    }

    /// Hadamard: a rotation by π about (x + z)/√2, up to a global phase.
    pub fn h(qubit: usize) -> Self {
        let c = PI / 2.0 * FRAC_1_SQRT_2;
        Self::single("H".into(), qubit, [c, 0.0, c])
    }

    /// exp(−i·c·Z⊗Z).
    pub fn zz(q0: usize, q1: usize, c: f64) -> Self {
        Self {
            label: format!("exp(−i·{c:.2}·Z⊗Z)"),
            generator: Generator::ZZ {
                qubits: (q0, q1),
                c,
            },
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// For a single-qubit gate: its qubit, unit rotation axis, and rotation
    /// angle on the Bloch sphere. `None` for the entangler or a zero rotation.
    pub fn bloch_rotation(&self) -> Option<(usize, Bloch, f64)> {
        match self.generator {
            Generator::Single { qubit, a } => {
                let n = norm(a);
                (n > 0.0).then(|| (qubit, [a[0] / n, a[1] / n, a[2] / n], 2.0 * n))
            }
            Generator::ZZ { .. } => None,
        }
    }

    /// The qubits the gate acts on.
    pub fn qubits(&self) -> Vec<usize> {
        match self.generator {
            Generator::Single { qubit, .. } => vec![qubit],
            Generator::ZZ { qubits, .. } => vec![qubits.0, qubits.1],
        }
    }
}

fn norm(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// The state of a few qubits: 2ⁿ amplitudes, qubit q being bit q of the index.
#[derive(Debug, Clone, PartialEq)]
pub struct StateVector {
    n_qubits: usize,
    amps: Vec<C64>,
}

impl StateVector {
    /// |0…0⟩.
    pub fn zero(n_qubits: usize) -> Self {
        assert!((1..=8).contains(&n_qubits), "1 to 8 qubits");
        let mut amps = vec![C64::new(0.0, 0.0); 1 << n_qubits];
        amps[0] = C64::new(1.0, 0.0);
        Self { n_qubits, amps }
    }

    pub fn n_qubits(&self) -> usize {
        self.n_qubits
    }

    pub fn amplitudes(&self) -> &[C64] {
        &self.amps
    }

    /// Apply exp(−i·s·G): the gate itself at s = 1, the identity at s = 0.
    pub fn apply(&mut self, gate: &Gate, s: f64) {
        match gate.generator {
            Generator::Single { qubit, a } => {
                assert!(qubit < self.n_qubits, "qubit out of range");
                let n = norm(a);
                if n == 0.0 {
                    return;
                }
                let (sin, cos) = (s * n).sin_cos();
                let (nx, ny, nz) = (a[0] / n, a[1] / n, a[2] / n);
                // cos·I − i·sin·(n·σ), with n·σ = [[nz, nx − i ny], [nx + i ny, −nz]]
                let mi = C64::new(0.0, -sin);
                let u00 = C64::new(cos, 0.0) + mi * nz;
                let u01 = mi * C64::new(nx, -ny);
                let u10 = mi * C64::new(nx, ny);
                let u11 = C64::new(cos, 0.0) - mi * nz;
                let bit = 1 << qubit;
                for i in 0..self.amps.len() {
                    if i & bit == 0 {
                        let (p, q) = (self.amps[i], self.amps[i | bit]);
                        self.amps[i] = u00 * p + u01 * q;
                        self.amps[i | bit] = u10 * p + u11 * q;
                    }
                }
            }
            Generator::ZZ {
                qubits: (q0, q1),
                c,
            } => {
                assert!(q0 < self.n_qubits && q1 < self.n_qubits && q0 != q1);
                for (i, amp) in self.amps.iter_mut().enumerate() {
                    let z0 = 1.0 - 2.0 * ((i >> q0) & 1) as f64;
                    let z1 = 1.0 - 2.0 * ((i >> q1) & 1) as f64;
                    *amp *= C64::from_polar(1.0, -s * c * z0 * z1);
                }
            }
        }
    }

    /// Apply every gate fully.
    pub fn run(&mut self, circuit: &[Gate]) {
        for g in circuit {
            self.apply(g, 1.0);
        }
    }

    /// Qubit `q`'s Bloch vector, from its reduced density matrix. Length 1
    /// for a pure, unentangled qubit; shorter when it is entangled.
    pub fn bloch(&self, q: usize) -> Bloch {
        assert!(q < self.n_qubits, "qubit out of range");
        let bit = 1 << q;
        let (mut p0, mut p1) = (0.0, 0.0);
        let mut rho01 = C64::new(0.0, 0.0);
        for i in 0..self.amps.len() {
            if i & bit == 0 {
                let (a, b) = (self.amps[i], self.amps[i | bit]);
                p0 += a.norm_sqr();
                p1 += b.norm_sqr();
                rho01 += a * b.conj();
            }
        }
        [2.0 * rho01.re, -2.0 * rho01.im, p0 - p1]
    }

    /// Every qubit's Bloch vector.
    pub fn blochs(&self) -> Vec<Bloch> {
        (0..self.n_qubits).map(|q| self.bloch(q)).collect()
    }

    /// |⟨self|other⟩|², which ignores global phase.
    pub fn fidelity(&self, other: &Self) -> f64 {
        self.amps
            .iter()
            .zip(&other.amps)
            .map(|(a, b)| a.conj() * b)
            .sum::<C64>()
            .norm_sqr()
    }
}

/// One input of an encoding, with its range and starting value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feature {
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
}

const fn feature(name: &'static str, min: f64, max: f64, default: f64) -> Feature {
    Feature {
        name,
        min,
        max,
        default,
    }
}

const ANGLE: [Feature; 1] = [feature("x", -PI, PI, PI / 3.0)];

/// A way to write classical features into qubits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    Basis,
    AngleRy,
    AngleRx,
    Phase,
    DenseAngle,
    Amplitude,
    Reuploading,
    ZzFeatureMap,
}

impl Encoding {
    pub const ALL: [Encoding; 8] = [
        Encoding::AngleRy,
        Encoding::AngleRx,
        Encoding::Phase,
        Encoding::DenseAngle,
        Encoding::Amplitude,
        Encoding::Basis,
        Encoding::Reuploading,
        Encoding::ZzFeatureMap,
    ];

    /// Layers of (RY(x), RZ(x)) in [`Encoding::Reuploading`].
    pub const REUPLOAD_LAYERS: usize = 3;

    /// Stable kebab-case id.
    pub fn id(self) -> &'static str {
        match self {
            Encoding::Basis => "basis",
            Encoding::AngleRy => "angle-ry",
            Encoding::AngleRx => "angle-rx",
            Encoding::Phase => "phase",
            Encoding::DenseAngle => "dense-angle",
            Encoding::Amplitude => "amplitude",
            Encoding::Reuploading => "reuploading",
            Encoding::ZzFeatureMap => "zz-feature-map",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Encoding::Basis => "Basis encoding",
            Encoding::AngleRy => "Angle encoding (RY)",
            Encoding::AngleRx => "Angle encoding (RX)",
            Encoding::Phase => "Phase encoding",
            Encoding::DenseAngle => "Dense angle encoding",
            Encoding::Amplitude => "Amplitude encoding",
            Encoding::Reuploading => "Data re-uploading",
            Encoding::ZzFeatureMap => "ZZ feature map",
        }
    }

    /// The encoded state, in textbook notation.
    pub fn formula(self) -> &'static str {
        match self {
            Encoding::Basis => "|b⟩ = X^b|0⟩   (drawn as RX(πb), so b can move continuously)",
            Encoding::AngleRy => "RY(x)|0⟩ = cos(x/2)|0⟩ + sin(x/2)|1⟩",
            Encoding::AngleRx => "RX(x)|0⟩ = cos(x/2)|0⟩ − i·sin(x/2)|1⟩",
            Encoding::Phase => "P(x)·H|0⟩ = (|0⟩ + e^{ix}|1⟩)/√2",
            Encoding::DenseAngle => "RZ(x₂)·RY(x₁)|0⟩ = cos(x₁/2)|0⟩ + e^{ix₂}·sin(x₁/2)|1⟩",
            Encoding::Amplitude => "(x₁|0⟩ + x₂|1⟩) / ‖x‖",
            Encoding::Reuploading => "[RZ(x)·RY(x)]³|0⟩",
            Encoding::ZzFeatureMap => "exp(−i(π−x₁)(π−x₂)Z⊗Z) · (RZ(2x₁)⊗RZ(2x₂)) · (H⊗H)|00⟩",
        }
    }

    /// What the encoding looks like on the Bloch sphere, in one sentence.
    pub fn shape(self) -> &'static str {
        match self {
            Encoding::Basis => {
                "Only the two poles carry data: a bit is the north or the south pole."
            }
            Encoding::AngleRy => {
                "The data is a latitude: x moves the state along the x–z meridian."
            }
            Encoding::AngleRx => {
                "The same idea about another axis: x moves the state along the y–z meridian."
            }
            Encoding::Phase => "The data is a longitude: x moves the state around the equator.",
            Encoding::DenseAngle => {
                "Two features as latitude and longitude: the whole sphere is reachable."
            }
            Encoding::Amplitude => {
                "Real amplitudes stay on the x–z meridian, and scaling x changes nothing: the norm is lost."
            }
            Encoding::Reuploading => {
                "Repeating the same rotations makes a curve that winds round the sphere: higher frequencies in x."
            }
            Encoding::ZzFeatureMap => {
                "The ZZ phase entangles the qubits, so each one's vector shrinks inside its sphere."
            }
        }
    }

    pub fn n_qubits(self) -> usize {
        match self {
            Encoding::ZzFeatureMap => 2,
            _ => 1,
        }
    }

    pub fn features(self) -> &'static [Feature] {
        const BASIS: [Feature; 1] = [feature("b", 0.0, 1.0, 1.0)];
        const DENSE: [Feature; 2] = [
            feature("x₁", 0.0, PI, PI / 3.0),
            feature("x₂", -PI, PI, PI / 4.0),
        ];
        const AMPLITUDE: [Feature; 2] =
            [feature("x₁", -1.0, 1.0, 0.6), feature("x₂", -1.0, 1.0, 0.8)];
        const REUPLOAD: [Feature; 1] = [feature("x", -PI, PI, 1.0)];
        const ZZ: [Feature; 2] = [feature("x₁", 0.0, PI, 1.0), feature("x₂", 0.0, PI, 2.0)];
        match self {
            Encoding::Basis => &BASIS,
            Encoding::AngleRy | Encoding::AngleRx | Encoding::Phase => &ANGLE,
            Encoding::DenseAngle => &DENSE,
            Encoding::Amplitude => &AMPLITUDE,
            Encoding::Reuploading => &REUPLOAD,
            Encoding::ZzFeatureMap => &ZZ,
        }
    }

    /// The default feature values.
    pub fn defaults(self) -> Vec<f64> {
        self.features().iter().map(|f| f.default).collect()
    }

    /// The circuit that encodes `x` (one value per feature).
    ///
    /// # Panics
    /// If `x` has the wrong number of values.
    pub fn circuit(self, x: &[f64]) -> Vec<Gate> {
        assert_eq!(
            x.len(),
            self.features().len(),
            "{}: wrong feature count",
            self.id()
        );
        match self {
            Encoding::Basis => {
                vec![Gate::rx(0, PI * x[0]).with_label(format!("X^b = RX(π·{:.2})", x[0]))]
            }
            Encoding::AngleRy => vec![Gate::ry(0, x[0])],
            Encoding::AngleRx => vec![Gate::rx(0, x[0])],
            Encoding::Phase => vec![
                Gate::h(0),
                Gate::rz(0, x[0]).with_label(format!("P({:.2})", x[0])),
            ],
            Encoding::DenseAngle => vec![Gate::ry(0, x[0]), Gate::rz(0, x[1])],
            Encoding::Amplitude => {
                let theta = 2.0 * x[1].atan2(x[0]);
                vec![Gate::ry(0, theta).with_label(format!("RY(2·atan2(x₂, x₁)) = RY({theta:.2})"))]
            }
            Encoding::Reuploading => (0..Self::REUPLOAD_LAYERS)
                .flat_map(|layer| {
                    [
                        Gate::ry(0, x[0]).with_label(format!(
                            "layer {}: RY({:.2})",
                            layer + 1,
                            x[0]
                        )),
                        Gate::rz(0, x[0]).with_label(format!(
                            "layer {}: RZ({:.2})",
                            layer + 1,
                            x[0]
                        )),
                    ]
                })
                .collect(),
            Encoding::ZzFeatureMap => {
                let phi = (PI - x[0]) * (PI - x[1]);
                vec![
                    Gate::h(0).with_label("H on qubit 1"),
                    Gate::h(1).with_label("H on qubit 2"),
                    Gate::rz(0, 2.0 * x[0])
                        .with_label(format!("RZ(2x₁) = RZ({:.2}) on qubit 1", 2.0 * x[0])),
                    Gate::rz(1, 2.0 * x[1])
                        .with_label(format!("RZ(2x₂) = RZ({:.2}) on qubit 2", 2.0 * x[1])),
                    Gate::zz(0, 1, phi).with_label(format!(
                        "exp(−i·(π−x₁)(π−x₂)·Z⊗Z), phase {phi:.2}: entangles"
                    )),
                ]
            }
        }
    }

    /// The encoded state.
    pub fn encode(self, x: &[f64]) -> StateVector {
        let mut s = StateVector::zero(self.n_qubits());
        s.run(&self.circuit(x));
        s
    }
}

/// Bloch vectors sampled while a circuit runs, gate by gate.
#[derive(Debug, Clone, PartialEq)]
pub struct GatePath {
    /// `points[q]` is qubit q's path: the start, then `samples_per_gate`
    /// points per gate.
    pub points: Vec<Vec<Bloch>>,
    pub samples_per_gate: usize,
    pub gates: Vec<Gate>,
}

impl GatePath {
    /// Index into `points[q]` of the end of gate `g` (0-based).
    pub fn end_of_gate(&self, g: usize) -> usize {
        (g + 1) * self.samples_per_gate
    }
}

/// Sample each qubit's Bloch vector as every gate of the encoding's circuit
/// is applied from s = 0 to 1 in turn, starting at |0…0⟩.
pub fn gate_path(encoding: Encoding, x: &[f64], samples_per_gate: usize) -> GatePath {
    let samples = samples_per_gate.max(1);
    let gates = encoding.circuit(x);
    let mut state = StateVector::zero(encoding.n_qubits());
    let mut points: Vec<Vec<Bloch>> = state.blochs().into_iter().map(|b| vec![b]).collect();
    for gate in &gates {
        for k in 1..=samples {
            let mut s = state.clone();
            s.apply(gate, k as f64 / samples as f64);
            for (q, b) in s.blochs().into_iter().enumerate() {
                points[q].push(b);
            }
        }
        state.apply(gate, 1.0);
    }
    GatePath {
        points,
        samples_per_gate: samples,
        gates,
    }
}

/// Each qubit's Bloch vector of the encoded state as feature `feature`
/// sweeps its range in `samples` steps (`samples + 1` points), the other
/// features held at `x`.
pub fn locus(encoding: Encoding, x: &[f64], feature: usize, samples: usize) -> Vec<Vec<Bloch>> {
    let f = encoding.features()[feature];
    let samples = samples.max(1);
    let mut out = vec![Vec::with_capacity(samples + 1); encoding.n_qubits()];
    let mut xs = x.to_vec();
    for k in 0..=samples {
        xs[feature] = f.min + (f.max - f.min) * k as f64 / samples as f64;
        for (q, b) in encoding.encode(&xs).blochs().into_iter().enumerate() {
            out[q].push(b);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    fn close(a: Bloch, b: Bloch) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < EPS)
    }

    fn len(b: Bloch) -> f64 {
        norm(b)
    }

    #[test]
    fn basis_states() {
        let mut s = StateVector::zero(1);
        assert!(close(s.bloch(0), [0.0, 0.0, 1.0]));
        s.apply(&Gate::rx(0, PI), 1.0);
        assert!(close(s.bloch(0), [0.0, 0.0, -1.0]));
    }

    #[test]
    fn bell_state_qubits_are_maximally_mixed() {
        // (|00⟩ + |11⟩)/√2. CNOT is not in the gate set, so set the amplitudes.
        let mut s = StateVector::zero(2);
        let r = FRAC_1_SQRT_2;
        s.amps = vec![
            C64::new(r, 0.0),
            C64::new(0.0, 0.0),
            C64::new(0.0, 0.0),
            C64::new(r, 0.0),
        ];
        assert!(close(s.bloch(0), [0.0; 3]) && close(s.bloch(1), [0.0; 3]));
    }

    #[test]
    fn hadamard_full_and_half() {
        let mut s = StateVector::zero(1);
        s.apply(&Gate::h(0), 1.0);
        assert!(close(s.bloch(0), [1.0, 0.0, 0.0]));
        let mut half = StateVector::zero(1);
        half.apply(&Gate::h(0), 0.5);
        let b = half.bloch(0);
        // On the great circle about (1, 0, 1)/√2 through |0⟩: b·n = 1/√2.
        assert!(((b[0] + b[2]) * FRAC_1_SQRT_2 - FRAC_1_SQRT_2).abs() < EPS);
        assert!((len(b) - 1.0).abs() < EPS);
        let (q, axis, angle) = Gate::h(0).bloch_rotation().unwrap();
        assert_eq!(q, 0);
        assert!(close(axis, [FRAC_1_SQRT_2, 0.0, FRAC_1_SQRT_2]));
        assert!((angle - PI).abs() < EPS);
    }

    #[test]
    fn partial_rotation_is_a_smaller_rotation() {
        let theta = 1.3;
        let mut half = StateVector::zero(1);
        half.apply(&Gate::ry(0, theta), 0.5);
        let mut direct = StateVector::zero(1);
        direct.apply(&Gate::ry(0, theta / 2.0), 1.0);
        assert!((half.fidelity(&direct) - 1.0).abs() < EPS);
    }

    #[test]
    fn rotation_axes_match_the_gate_names() {
        for (g, axis) in [
            (Gate::rx(0, 0.7), [1.0, 0.0, 0.0]),
            (Gate::ry(0, 0.7), [0.0, 1.0, 0.0]),
            (Gate::rz(0, 0.7), [0.0, 0.0, 1.0]),
        ] {
            let (_, a, angle) = g.bloch_rotation().unwrap();
            assert!(close(a, axis) && (angle - 0.7).abs() < EPS);
        }
        assert!(Gate::zz(0, 1, 0.3).bloch_rotation().is_none());
    }

    #[test]
    fn angle_encoding_stays_on_a_meridian() {
        for k in 0..=20 {
            let x = -PI + 2.0 * PI * k as f64 / 20.0;
            let b = Encoding::AngleRy.encode(&[x]).bloch(0);
            assert!(close(b, [x.sin(), 0.0, x.cos()]), "x = {x}");
            let bx = Encoding::AngleRx.encode(&[x]).bloch(0);
            assert!(close(bx, [0.0, -x.sin(), x.cos()]), "rx x = {x}");
        }
    }

    #[test]
    fn phase_encoding_stays_on_the_equator() {
        for k in 0..=20 {
            let x = -PI + 2.0 * PI * k as f64 / 20.0;
            let b = Encoding::Phase.encode(&[x]).bloch(0);
            assert!(close(b, [x.cos(), x.sin(), 0.0]), "x = {x}: {b:?}");
        }
    }

    #[test]
    fn dense_angle_is_latitude_and_longitude() {
        let (t, p) = (1.1, -2.0);
        let b = Encoding::DenseAngle.encode(&[t, p]).bloch(0);
        assert!(close(b, [t.sin() * p.cos(), t.sin() * p.sin(), t.cos()]));
    }

    #[test]
    fn amplitude_encoding_forgets_the_norm() {
        let a = Encoding::Amplitude.encode(&[0.3, -0.4]);
        let b = Encoding::Amplitude.encode(&[0.6, -0.8]);
        assert!((a.fidelity(&b) - 1.0).abs() < EPS);
        // And the amplitudes are the normalized features.
        let amps = a.amplitudes();
        assert!((amps[0].re - 0.6).abs() < EPS && (amps[1].re + 0.8).abs() < EPS);
    }

    #[test]
    fn basis_encoding_hits_the_poles() {
        assert!(close(
            Encoding::Basis.encode(&[0.0]).bloch(0),
            [0.0, 0.0, 1.0]
        ));
        assert!(close(
            Encoding::Basis.encode(&[1.0]).bloch(0),
            [0.0, 0.0, -1.0]
        ));
    }

    #[test]
    fn zz_feature_map_entangles() {
        let x = Encoding::ZzFeatureMap.defaults();
        let phi = (PI - x[0]) * (PI - x[1]);
        assert!((phi / (PI / 2.0)).fract().abs() > 0.05);
        let s = Encoding::ZzFeatureMap.encode(&x);
        for q in 0..2 {
            let l = len(s.bloch(q));
            assert!(l < 1.0 - 1e-3, "qubit {q} length {l}");
            // |r| = |cos 2φ| for this circuit.
            assert!((l - (2.0 * phi).cos().abs()).abs() < EPS);
        }
        // At φ = 0 (x₁ = π) nothing is entangled.
        let free = Encoding::ZzFeatureMap.encode(&[PI, 0.4]);
        assert!((len(free.bloch(0)) - 1.0).abs() < EPS);
    }

    #[test]
    fn single_qubit_encodings_are_pure() {
        for e in Encoding::ALL.into_iter().filter(|e| e.n_qubits() == 1) {
            let b = e.encode(&e.defaults()).bloch(0);
            assert!((len(b) - 1.0).abs() < EPS, "{}", e.id());
        }
    }

    #[test]
    fn gate_paths_end_at_the_encoding() {
        for e in Encoding::ALL {
            let x = e.defaults();
            let path = gate_path(e, &x, 16);
            let end = e.encode(&x).blochs();
            for (q, pts) in path.points.iter().enumerate() {
                assert_eq!(pts.len(), 1 + 16 * path.gates.len());
                assert!(close(pts[0], StateVector::zero(e.n_qubits()).bloch(q)));
                assert!(close(*pts.last().unwrap(), end[q]), "{} qubit {q}", e.id());
                assert!(close(pts[path.end_of_gate(path.gates.len() - 1)], end[q]));
            }
        }
    }

    #[test]
    fn loci_of_single_feature_rotations() {
        let ry = locus(Encoding::AngleRy, &[0.0], 0, 64);
        assert!(ry[0].iter().all(|b| b[1].abs() < EPS));
        let ph = locus(Encoding::Phase, &[0.0], 0, 64);
        assert!(ph[0].iter().all(|b| b[2].abs() < EPS));
        assert_eq!(ph[0].len(), 65);
    }

    #[test]
    fn ids_are_unique_kebab_case_and_round_trip() {
        let mut ids: Vec<&str> = Encoding::ALL.iter().map(|e| e.id()).collect();
        for id in &ids {
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            );
            assert_eq!(Encoding::from_id(id).unwrap().id(), *id);
        }
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), Encoding::ALL.len());
        for e in Encoding::ALL {
            for f in e.features() {
                assert!(
                    f.min < f.max && (f.min..=f.max).contains(&f.default),
                    "{}",
                    e.id()
                );
            }
        }
    }
}
