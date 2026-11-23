//! The Bloch sphere and common quantum data encodings, animated as rotations:
//! angle encoding traces a meridian, phase encoding the equator, dense angle
//! encoding covers the sphere, and the ZZ feature map entangles two qubits so
//! each one's vector shrinks inside its sphere. Run the circuit gate by gate,
//! or sweep a feature and watch the encoding's locus appear.
//!
//! ```sh
//! cargo run --release --example bloch_sphere
//! ```

#[path = "shared/mod.rs"]
mod shared;

fn main() {
    shared::run_experience("bloch-sphere");
}
