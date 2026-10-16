//! Writes the starter scenes in `assets/` with Jackdaw's own BSN writer:
//!
//! - `lobby.bsn`: one `LobbyEntry` per built-in experience, in lobby order;
//! - `examples/perceptron.bsn`: a perceptron diagram with a camera and light;
//! - `examples/loss-shapes.bsn`: three loss-shape surfaces side by side.
//!
//! ```sh
//! cargo run --example generate_scenes   # from jackdaw/
//! ```
//!
//! Run it again after adding an experience to bevaru (a test fails until
//! you do). It overwrites the files, so commit editor edits first.

use std::path::Path;

fn main() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    for (path, scene) in bevaru_jackdaw::scenes::SCENES {
        let path = assets.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, scene()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        println!("wrote {}", path.display());
    }
}
