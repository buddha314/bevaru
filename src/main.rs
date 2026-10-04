//! `cargo run` opens the lobby. `cargo run -- <experience-id>` opens one
//! experience directly; `cargo run -- --list` lists them. For docs,
//! `--thumbnail <out.png>` captures the experience's scene and exits.
//! With the `remote` feature, `--remote [--remote-port N]` lets agents drive
//! the window over Bevy Remote Protocol on 127.0.0.1 (default port 15702).

use std::process::ExitCode;

use bevaru::app::windowed_app;
use bevaru::capture::CapturePlugin;
use bevaru::experiences::{ExperienceRegistry, ExperiencesPlugin};
use bevaru::lobby::{Launch, list, parse_args};
use bevy::prelude::*;

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let thumbnail = match args.iter().position(|a| a == "--thumbnail") {
        Some(i) if i + 1 < args.len() => {
            let path = args.remove(i + 1);
            args.remove(i);
            Some(path)
        }
        Some(_) => {
            eprintln!("--thumbnail needs an output path");
            return ExitCode::from(2);
        }
        None => None,
    };

    let remote = take_flag(&mut args, "--remote");
    let remote_port = match take_value(&mut args, "--remote-port") {
        Ok(None) => None,
        Ok(Some(p)) => match p.parse::<u16>() {
            Ok(p) => Some(p),
            Err(_) => {
                eprintln!("--remote-port needs a port number, got {p:?}");
                return ExitCode::from(2);
            }
        },
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    if (remote || remote_port.is_some()) && !cfg!(feature = "remote") {
        eprintln!(
            "remote control needs the `remote` feature: cargo run --features remote -- --remote"
        );
        return ExitCode::from(2);
    }

    // The registry is plain data; read it without opening a window.
    let registry = {
        let mut app = App::new();
        app.add_plugins(ExperiencesPlugin);
        app.world().resource::<ExperienceRegistry>().clone()
    };
    let start = match parse_args(&args, &registry) {
        Ok(Launch::List) => {
            print!("{}", list(&registry));
            return ExitCode::SUCCESS;
        }
        Ok(Launch::Lobby) => None,
        Ok(Launch::Experience(id)) => Some(id),
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    let mut app = windowed_app(start);
    #[cfg(feature = "remote")]
    if remote || remote_port.is_some() {
        let port = remote_port.unwrap_or(bevaru::remote::DEFAULT_PORT);
        eprintln!("bevaru: remote control on http://127.0.0.1:{port}");
        app.add_plugins(bevaru::remote::RemoteHooksPlugin { port: Some(port) });
    }
    if let Some(path) = thumbnail {
        app.add_plugins(CapturePlugin::thumbnail(path));
    }
    app.run();
    ExitCode::SUCCESS
}

/// Remove `flag` from `args`; whether it was present.
fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    let before = args.len();
    args.retain(|a| a != flag);
    args.len() != before
}

/// Remove `flag <value>` from `args`.
fn take_value(args: &mut Vec<String>, flag: &str) -> Result<Option<String>, String> {
    match args.iter().position(|a| a == flag) {
        None => Ok(None),
        Some(i) if i + 1 < args.len() => {
            let value = args.remove(i + 1);
            args.remove(i);
            Ok(Some(value))
        }
        Some(_) => Err(format!("{flag} needs a value")),
    }
}
