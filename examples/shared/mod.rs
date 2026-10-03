//! Helpers shared by the examples.

/// Open a registered experience directly ("Back to lobby" still works).
/// `BEVARU_SCREENSHOT=out.png` captures the window and exits, for docs.
pub fn run_experience(id: &'static str) {
    bevaru::app::windowed_app(Some(id)).run();
}
