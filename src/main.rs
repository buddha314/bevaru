//! `cargo run` opens the lobby. `cargo run -- <experience-id>` opens one
//! experience directly; `cargo run -- --list` lists them. For docs,
//! `--thumbnail <out.png>` captures the experience's scene and exits.

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
    if let Some(path) = thumbnail {
        app.add_plugins(CapturePlugin::thumbnail(path));
    }
    app.run();
    ExitCode::SUCCESS
}
