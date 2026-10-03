//! `cargo run` must open the lobby. The no-argument path itself is covered
//! by `lobby::parse_args` (no args → `Launch::Lobby`); these checks keep the
//! manifest and the binary pointed at it.

use std::process::Command;

#[test]
fn default_run_is_the_lobby_binary() {
    let manifest =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    assert!(
        manifest
            .lines()
            .any(|l| l.trim() == r#"default-run = "bevaru""#),
        "Cargo.toml must keep `default-run = \"bevaru\"` so plain `cargo run` opens the lobby"
    );
}

#[test]
fn the_default_binary_is_the_lobby_app() {
    // `--list` and unknown ids are answered before any window opens.
    let bin = env!("CARGO_BIN_EXE_bevaru");
    let list = Command::new(bin).arg("--list").output().unwrap();
    assert!(list.status.success());
    let out = String::from_utf8(list.stdout).unwrap();
    for id in [
        "iris-svm",
        "regression-mse-vs-mae",
        "loss-curves",
        "mnist-svm",
        "sigmoid",
    ] {
        assert!(out.contains(id), "{id} missing from --list:\n{out}");
    }
    let unknown = Command::new(bin)
        .arg("no-such-experience")
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
}
