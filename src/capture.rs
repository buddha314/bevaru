//! Developer capture: screenshots for docs, and lobby thumbnails.
//!
//! - `BEVARU_SCREENSHOT=out.png` saves the whole window after
//!   `BEVARU_SCREENSHOT_AFTER` seconds (default 5) and exits.
//! - `bevaru <id> --thumbnail out.png` saves just the scene (the pane area
//!   between the panels, or the whole window for custom experiences), with
//!   UI overlays hidden and plain background trimmed, fitted into 480 × 270,
//!   then exits.
//!   `scripts/thumbnails.sh` runs it for every built-in experience.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::PrimaryWindow;
use image::imageops::{self, FilterType};

use crate::experiment::Experiment;
use crate::scene::UiInsets;

pub const THUMBNAIL_SIZE: (u32, u32) = (480, 270);

/// How long to wait for a requested screenshot before giving up. Frames can
/// stall (a hidden window, a GPU waking from power saving), so exit only once
/// the image is written, or after this long with an error.
const CAPTURE_TIMEOUT_SECS: f32 = 30.0;

/// Present while capturing a thumbnail: UI overlays (labels, buttons) hide.
#[derive(Resource, Debug, Default)]
pub struct HideOverlays;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Window,
    Thumbnail,
}

#[derive(Debug, Clone)]
pub struct CapturePlugin {
    pub path: PathBuf,
    pub after_secs: f32,
    pub mode: CaptureMode,
}

impl CapturePlugin {
    /// A full-window capture configured by `BEVARU_SCREENSHOT`, if set.
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os("BEVARU_SCREENSHOT")?;
        Some(Self {
            path: path.into(),
            after_secs: after_secs(5.0),
            mode: CaptureMode::Window,
        })
    }

    pub fn thumbnail(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            after_secs: after_secs(8.0),
            mode: CaptureMode::Thumbnail,
        }
    }
}

fn after_secs(default: f32) -> f32 {
    std::env::var("BEVARU_SCREENSHOT_AFTER")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        let config = self.clone();
        if config.mode == CaptureMode::Thumbnail {
            app.init_resource::<HideOverlays>();
        }
        let saved = Arc::new(AtomicBool::new(false));
        app.add_systems(
            Update,
            move |mut commands: Commands,
                  time: Res<Time<Real>>,
                  mut state: Local<u8>,
                  mut requested_at: Local<f32>,
                  mut exit: MessageWriter<AppExit>,
                  window: Single<&Window, With<PrimaryWindow>>,
                  insets: Option<Res<UiInsets>>,
                  experiment: Option<Res<Experiment>>| {
                let t = time.elapsed_secs();
                if *state == 0 && t > config.after_secs {
                    let crop = (config.mode == CaptureMode::Thumbnail).then(|| {
                        let insets = match (experiment.is_some(), insets) {
                            (true, Some(i)) => *i,
                            _ => UiInsets::default(),
                        };
                        scene_rect(&window, insets)
                    });
                    let path = config.path.clone();
                    let saved = saved.clone();
                    commands.spawn(Screenshot::primary_window()).observe(
                        move |shot: On<ScreenshotCaptured>| {
                            match save(&shot.image, crop, &path) {
                                Ok(()) => info!("bevaru: saved {}", path.display()),
                                Err(e) => error!("bevaru: capture failed: {e}"),
                            }
                            saved.store(true, Ordering::SeqCst);
                        },
                    );
                    *requested_at = t;
                    *state = 1;
                } else if *state == 1 && saved.load(Ordering::SeqCst) {
                    exit.write(AppExit::Success);
                    *state = 2;
                } else if *state == 1 && t > *requested_at + CAPTURE_TIMEOUT_SECS {
                    error!(
                        "bevaru: no screenshot after {CAPTURE_TIMEOUT_SECS} s (is the window hidden?); exiting without one"
                    );
                    exit.write(AppExit::error());
                    *state = 2;
                }
            },
        );
    }
}

/// The scene area in physical pixels: `(x, y, width, height)`.
fn scene_rect(window: &Window, insets: UiInsets) -> (u32, u32, u32, u32) {
    let s = window.scale_factor();
    let x = (insets.left * s).round() as u32;
    let right = (insets.right * s).round() as u32;
    let (w, h) = (window.physical_width(), window.physical_height());
    (x, 0, w.saturating_sub(x + right).max(1), h)
}

fn save(
    shot: &Image,
    crop: Option<(u32, u32, u32, u32)>,
    path: &std::path::Path,
) -> Result<(), String> {
    let mut img = shot
        .clone()
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .to_rgba8();
    if let Some((x, y, w, h)) = crop {
        img = imageops::crop_imm(&img, x, y, w, h).to_image();
        img = fit_thumbnail(&img);
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    img.save(path).map_err(|e| e.to_string())
}

/// Trim the plain background around the content, then scale it to fit the
/// thumbnail with a small margin, padding with that background colour.
fn fit_thumbnail(img: &image::RgbaImage) -> image::RgbaImage {
    let (tw, th) = THUMBNAIL_SIZE;
    let bg = *img.get_pixel(0, 0);
    let differs = |p: &image::Rgba<u8>| p.0.iter().zip(bg.0).any(|(a, b)| a.abs_diff(b) > 12);
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in img.enumerate_pixels() {
        if differs(p) {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    let content = if x0 <= x1 && y0 <= y1 {
        imageops::crop_imm(img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image()
    } else {
        img.clone()
    };
    let (cw, ch) = content.dimensions();
    let margin = 0.92;
    let scale = (tw as f32 * margin / cw as f32).min(th as f32 * margin / ch as f32);
    let (sw, sh) = (
        ((cw as f32 * scale) as u32).max(1),
        ((ch as f32 * scale) as u32).max(1),
    );
    let scaled = imageops::resize(&content, sw, sh, FilterType::Lanczos3);
    let mut out = image::RgbaImage::from_pixel(tw, th, bg);
    imageops::overlay(
        &mut out,
        &scaled,
        ((tw - sw) / 2) as i64,
        ((th - sh) / 2) as i64,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnail_trims_background_and_fits() {
        // A 1000 × 1000 grey image with a red 100 × 50 block off-centre.
        let mut img = image::RgbaImage::from_pixel(1000, 1000, image::Rgba([240, 240, 240, 255]));
        for x in 700..800 {
            for y in 100..150 {
                img.put_pixel(x, y, image::Rgba([200, 30, 30, 255]));
            }
        }
        let out = fit_thumbnail(&img);
        assert_eq!(out.dimensions(), THUMBNAIL_SIZE);
        // The block (2:1) is scaled up to fill most of the height, centred.
        let red = out
            .pixels()
            .filter(|p| p.0[0] > 150 && p.0[1] < 100)
            .count();
        assert!(red > 480 * 270 / 3, "content fills the thumbnail: {red}");
        assert_eq!(*out.get_pixel(240, 135), image::Rgba([200, 30, 30, 255]));
        assert_eq!(*out.get_pixel(0, 0), image::Rgba([240, 240, 240, 255]));
    }
}
