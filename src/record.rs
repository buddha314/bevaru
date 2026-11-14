//! Developer capture: record an experience to a video file for editing.
//!
//! `bevaru <id> --record out.mov` opens the experience at 1920 × 1080, waits
//! for it to finish loading, records every rendered frame, pipes them to
//! `ffmpeg`, and exits. `scripts/record-demos.sh` runs it for every built-in
//! experience.
//!
//! When recording starts, the app clock is locked to the video's frame rate
//! ([`TimeUpdateStrategy::ManualDuration`]): each rendered frame advances
//! animation time by exactly `1 / fps`, however long the frame really took
//! to render, read back, and encode. The video therefore plays back at true
//! speed with no dropped frames, however fast or slow the capture runs.
//!
//! Training also restarts at that moment, so it plays out on camera rather
//! than during the lead-in: playback is reset to step 0 and played, and a
//! running sweep is rewound to its first value.
//!
//! Environment:
//! - `BEVARU_RECORD_SECS`: length of the video (default 20).
//! - `BEVARU_RECORD_FPS`: frame rate (default 30).
//! - `BEVARU_RECORD_LEAD`: seconds (real time) to run after loading before
//!   recording, so meshes and plots built on background threads are ready
//!   for the first frame (default 1).
//! - `BEVARU_RECORD_ORBIT`: turn 3-D orbit cameras at this many degrees per
//!   second while recording (default 0: the camera stays put).
//! - `BEVARU_RECORD_CODEC`: `prores` (ProRes 422 HQ, default; use `.mov`),
//!   `h264` (OpenH264; use `.mp4`), or `vp9` (use `.webm`).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::time::TimeUpdateStrategy;
use bevy::window::{PrimaryWindow, WindowResolution};

use crate::experiences::ActiveExperience;
use crate::lobby::AppScreen;
use crate::orbit::OrbitRig;
use crate::playback::{Playback, PlaybackCommand, Sweep, SweepCommand};

pub const VIDEO_SIZE: (u32, u32) = (1920, 1080);

/// Give up if the experience has not loaded after this long (wall clock;
/// MNIST downloads its data on first use).
const MAX_WAIT: Duration = Duration::from_secs(300);
/// How long to give the compositor to apply [`VIDEO_SIZE`] before recording
/// whatever size the window is, letterboxed.
const RESIZE_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct RecordPlugin {
    pub path: PathBuf,
    pub secs: f32,
    pub fps: u32,
    pub lead_secs: f32,
    /// Degrees per second.
    pub orbit: f32,
    pub codec: Codec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    ProRes,
    H264,
    Vp9,
}

impl Codec {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "prores" => Some(Self::ProRes),
            "h264" => Some(Self::H264),
            "vp9" => Some(Self::Vp9),
            _ => None,
        }
    }

    /// ffmpeg output arguments.
    fn args(self, fps: u32) -> Vec<String> {
        let gop = fps.to_string();
        let a: &[&str] = match self {
            // 422 HQ, 10-bit: the usual editing intermediate. Every frame is
            // a keyframe, so editors scrub and cut it frame-accurately.
            Self::ProRes => &[
                "-c:v",
                "prores_ks",
                "-profile:v",
                "3",
                "-vendor",
                "apl0",
                "-pix_fmt",
                "yuv422p10le",
            ],
            Self::H264 => &[
                "-c:v",
                "libopenh264",
                "-b:v",
                "16M",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ],
            Self::Vp9 => &[
                "-c:v",
                "libvpx-vp9",
                "-crf",
                "18",
                "-b:v",
                "0",
                "-row-mt",
                "1",
                "-pix_fmt",
                "yuv420p",
            ],
        };
        let mut v: Vec<String> = a.iter().map(|s| s.to_string()).collect();
        if self != Self::ProRes {
            v.extend(["-g".into(), gop]);
        }
        v
    }
}

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

impl RecordPlugin {
    /// A recording to `path`, configured by the `BEVARU_RECORD_*` variables.
    pub fn from_env(path: impl Into<PathBuf>) -> Result<Self, String> {
        let codec = match std::env::var("BEVARU_RECORD_CODEC") {
            Ok(s) => Codec::parse(&s)
                .ok_or_else(|| format!("BEVARU_RECORD_CODEC: unknown codec {s:?}"))?,
            Err(_) => Codec::ProRes,
        };
        let fps: u32 = env("BEVARU_RECORD_FPS", 30);
        if fps == 0 {
            return Err("BEVARU_RECORD_FPS must be positive".into());
        }
        Ok(Self {
            path: path.into(),
            secs: env("BEVARU_RECORD_SECS", 20.0),
            fps,
            lead_secs: env("BEVARU_RECORD_LEAD", 1.0),
            orbit: env("BEVARU_RECORD_ORBIT", 0.0),
            codec,
        })
    }

    fn frames(&self) -> u32 {
        (self.secs * self.fps as f32).round().max(1.0) as u32
    }
}

/// One captured frame, numbered in the order it was requested.
struct Frame {
    index: u32,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[derive(Resource)]
struct Recorder {
    config: RecordPlugin,
    phase: Phase,
    /// Wall-clock time the app started waiting.
    started: std::time::Instant,
    /// Wall-clock time when the experience was first seen running.
    running_since: Option<std::time::Instant>,
    requested: u32,
    /// Shared with the screenshot observers; `None` once the stream closes.
    sender: Arc<Mutex<Option<SyncSender<Frame>>>>,
    writer: Option<JoinHandle<Result<u32, String>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Waiting,
    Recording,
    Finishing,
    Done,
}

impl Plugin for RecordPlugin {
    fn build(&self, app: &mut App) {
        // Bounded, so a slow encoder holds the app back instead of piling up
        // gigabytes of frames.
        let (tx, rx) = sync_channel::<Frame>(8);
        let path = self.path.clone();
        let (fps, codec, total) = (self.fps, self.codec, self.frames());
        let writer = std::thread::spawn(move || write_video(rx, &path, fps, codec, total));
        app.insert_resource(Recorder {
            config: self.clone(),
            phase: Phase::Waiting,
            started: std::time::Instant::now(),
            running_since: None,
            requested: 0,
            sender: Arc::new(Mutex::new(Some(tx))),
            writer: Some(writer),
        })
        .add_systems(Startup, size_window)
        .add_systems(Update, (record, turn_cameras));
    }
}

fn size_window(mut window: Single<&mut Window, With<PrimaryWindow>>) {
    let (w, h) = VIDEO_SIZE;
    window.resolution = WindowResolution::new(w, h).with_scale_factor_override(1.0);
    window.resizable = false;
}

fn record(
    mut commands: Commands,
    mut rec: ResMut<Recorder>,
    screen: Option<Res<State<AppScreen>>>,
    active: Option<Res<ActiveExperience>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
    (playback, sweep): (Option<Res<Playback>>, Option<Res<Sweep>>),
    (mut playback_cmd, mut sweep_cmd): (
        MessageWriter<PlaybackCommand>,
        MessageWriter<SweepCommand>,
    ),
) {
    let total = rec.config.frames();
    match rec.phase {
        Phase::Waiting => {
            // An experiment experience is ready once it reaches `Running`; a
            // custom one has no loading screen, so being active is enough.
            let screen = screen.map(|s| *s.get());
            let running = screen == Some(AppScreen::Running)
                || active.is_some_and(|a| a.0.is_some()) && screen != Some(AppScreen::Loading);
            let sized = (window.physical_width(), window.physical_height()) == VIDEO_SIZE;
            if running && (sized || rec.started.elapsed() > RESIZE_GRACE) {
                rec.running_since
                    .get_or_insert_with(std::time::Instant::now);
            }
            let lead = rec.config.lead_secs;
            let settled = rec
                .running_since
                .is_some_and(|t| t.elapsed().as_secs_f32() >= lead);
            if settled {
                if !sized {
                    warn!(
                        "bevaru: window is {}×{}, not {}×{}; frames will be scaled",
                        window.physical_width(),
                        window.physical_height(),
                        VIDEO_SIZE.0,
                        VIDEO_SIZE.1
                    );
                }
                info!(
                    "bevaru: recording {total} frames ({} s at {} fps) to {}",
                    rec.config.secs,
                    rec.config.fps,
                    rec.config.path.display()
                );
                commands.insert_resource(TimeUpdateStrategy::ManualDuration(
                    Duration::from_secs_f64(1.0 / rec.config.fps as f64),
                ));
                restart(&playback, &sweep, &mut playback_cmd, &mut sweep_cmd);
                rec.phase = Phase::Recording;
                // Let the restart and the new clock take effect first.
                return;
            } else if rec.started.elapsed() > MAX_WAIT {
                error!(
                    "bevaru: not ready to record after {MAX_WAIT:?} (running: {running}, window {}×{})",
                    window.physical_width(),
                    window.physical_height()
                );
                rec.sender.lock().unwrap().take();
                exit.write(AppExit::error());
                rec.phase = Phase::Done;
                return;
            }
            return;
        }
        Phase::Finishing => {
            if rec.writer.as_ref().is_some_and(|w| w.is_finished()) {
                let result = rec.writer.take().unwrap().join();
                match result {
                    Ok(Ok(n)) => {
                        info!("bevaru: wrote {n} frames to {}", rec.config.path.display());
                        exit.write(AppExit::Success);
                    }
                    Ok(Err(e)) => {
                        error!("bevaru: recording failed: {e}");
                        exit.write(AppExit::error());
                    }
                    Err(_) => {
                        error!("bevaru: video writer panicked");
                        exit.write(AppExit::error());
                    }
                }
                rec.phase = Phase::Done;
            }
            return;
        }
        Phase::Recording => {}
        Phase::Done => return,
    }

    let index = rec.requested;
    // Each observer owns a sender, taken now: the frame arrives a few frames
    // later, after the shared one may already be closed.
    let Some(tx) = rec.sender.lock().unwrap().clone() else {
        return;
    };
    commands
        .spawn(Screenshot::primary_window())
        .observe(move |shot: On<ScreenshotCaptured>| {
            match shot.image.clone().try_into_dynamic() {
                Ok(img) => {
                    let img = img.to_rgba8();
                    let (width, height) = img.dimensions();
                    // Blocks while the encoder catches up; an error means the
                    // writer has already failed and reports why itself.
                    let _ = tx.send(Frame {
                        index,
                        width,
                        height,
                        rgba: img.into_raw(),
                    });
                }
                Err(e) => error!("bevaru: frame {index}: {e}"),
            }
        });
    rec.requested += 1;
    if rec.requested >= total {
        // Drop our sender. The writer finishes after the last frame, or when
        // every observer's clone is gone if some frames never arrive.
        rec.sender.lock().unwrap().take();
        rec.phase = Phase::Finishing;
    }
}

/// Replay whatever the experience animates from the start: a sweep from its
/// first value, training from step 0. Custom experiences have neither.
fn restart(
    playback: &Option<Res<Playback>>,
    sweep: &Option<Res<Sweep>>,
    playback_cmd: &mut MessageWriter<PlaybackCommand>,
    sweep_cmd: &mut MessageWriter<SweepCommand>,
) {
    if sweep.as_ref().is_some_and(|s| s.active) {
        sweep_cmd.write(SweepCommand::Seek(0));
        sweep_cmd.write(SweepCommand::Play);
    } else if playback.as_ref().is_some_and(|p| p.playing) {
        playback_cmd.write(PlaybackCommand::Reset);
        playback_cmd.write(PlaybackCommand::Play);
    }
}

/// Turn every orbit camera at the configured rate while recording.
fn turn_cameras(rec: Res<Recorder>, time: Res<Time>, mut rigs: Query<&mut OrbitRig>) {
    if rec.config.orbit == 0.0 || rec.phase == Phase::Waiting {
        return;
    }
    let step = rec.config.orbit.to_radians() * time.delta_secs();
    for mut rig in &mut rigs {
        rig.view.yaw += step;
    }
}

/// Receive frames (possibly out of order), and stream them in order to
/// ffmpeg. Returns the number of frames written.
fn write_video(
    rx: Receiver<Frame>,
    path: &Path,
    fps: u32,
    codec: Codec,
    total: u32,
) -> Result<u32, String> {
    let mut pending = BTreeMap::new();
    let mut next = 0u32;
    let mut ffmpeg: Option<(Child, ChildStdin, (u32, u32))> = None;
    let write = |frame: Frame,
                 ffmpeg: &mut Option<(Child, ChildStdin, (u32, u32))>|
     -> Result<(), String> {
        if ffmpeg.is_none() {
            *ffmpeg = Some(spawn_ffmpeg(path, fps, codec, frame.width, frame.height)?);
        }
        let (_, stdin, size) = ffmpeg.as_mut().unwrap();
        if (frame.width, frame.height) != *size {
            return Err(format!(
                "frame {} is {}×{}, but the video is {}×{}; was the window resized?",
                frame.index, frame.width, frame.height, size.0, size.1
            ));
        }
        stdin
            .write_all(&frame.rgba)
            .map_err(|e| format!("writing to ffmpeg: {e}"))
    };
    for frame in rx {
        pending.insert(frame.index, frame);
        while let Some(frame) = pending.remove(&next) {
            write(frame, &mut ffmpeg)?;
            next += 1;
        }
        if next >= total {
            break;
        }
    }
    if !pending.is_empty() || next < total {
        warn!(
            "bevaru: {} of {total} frames arrived ({} out of order)",
            next,
            pending.len()
        );
    }
    let Some((mut child, stdin, _)) = ffmpeg else {
        return Err("no frames were captured".into());
    };
    drop(stdin);
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("ffmpeg exited with {status}"));
    }
    Ok(next)
}

fn spawn_ffmpeg(
    path: &Path,
    fps: u32,
    codec: Codec,
    width: u32,
    height: u32,
) -> Result<(Child, ChildStdin, (u32, u32)), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let (vw, vh) = VIDEO_SIZE;
    // If the compositor refused the requested size, letterbox to it so every
    // clip in a reel has the same frame.
    let fit = format!(
        "scale={vw}:{vh}:force_original_aspect_ratio=decrease:flags=lanczos,pad={vw}:{vh}:(ow-iw)/2:(oh-ih)/2,setsar=1"
    );
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba"])
        .args(["-s", &format!("{width}x{height}")])
        .args(["-framerate", &fps.to_string(), "-i", "-"])
        .args(["-vf", &fit, "-r", &fps.to_string()])
        .args([
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-colorspace",
            "bt709",
        ])
        .args(codec.args(fps))
        .arg(path)
        .stdin(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("starting ffmpeg (is it installed?): {e}"))?;
    let stdin = child.stdin.take().expect("stdin is piped");
    Ok((child, stdin, (width, height)))
}
