//! Typeset formulas: Typst math source to an RGBA image, through ruviz's
//! `typst-math` feature (the `math` cargo feature, on by default).
//!
//! Formulas are written in Typst's math syntax (`$ sigma(z) = 1/(1 + e^(-z)) $`),
//! not LaTeX; see `docs/presentation/math.md` for why. Without the `math`
//! feature, [`typeset`] reports that typesetting is unavailable and callers
//! show the formula's plain-text form instead.

use serde::{Deserialize, Serialize};

/// A formula: Typst math source, and the same formula as plain Unicode text
/// for builds without `math`, exports, and accessibility.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Formula {
    pub typst: String,
    pub text: String,
}

impl Formula {
    pub fn new(typst: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            typst: typst.into(),
            text: text.into(),
        }
    }
}

/// A typeset formula: premultiplied RGBA8 pixels, row by row.
#[derive(Debug, Clone, PartialEq)]
pub struct FormulaImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Whether this build can typeset formulas.
pub const AVAILABLE: bool = cfg!(feature = "math");

/// Ink colour for typeset formulas: dark, for a light tooltip.
const INK: (u8, u8, u8) = (30, 30, 40);

/// Typeset Typst math source at `size_pt` points.
#[cfg(feature = "math")]
pub fn typeset(typst: &str, size_pt: f32) -> Result<FormulaImage, String> {
    use ruviz::render::Color;
    use ruviz::render::typst_text::render_raster;

    let out = render_raster(
        typst,
        size_pt,
        Color::from_rgb(INK.0, INK.1, INK.2),
        0.0,
        "formula",
    )
    .map_err(|e| e.to_string())?;
    let (width, height) = (out.pixmap.width() as usize, out.pixmap.height() as usize);
    if width == 0 || height == 0 {
        return Err("the formula typeset to an empty image".into());
    }
    Ok(FormulaImage {
        width,
        height,
        rgba: out.pixmap.data().to_vec(),
    })
}

/// Typesetting is unavailable without the `math` feature.
#[cfg(not(feature = "math"))]
pub fn typeset(_typst: &str, _size_pt: f32) -> Result<FormulaImage, String> {
    let _ = INK;
    Err("bevaru was built without the `math` feature".into())
}

/// Load the typesetting engine and fonts (about 0.1 s the first time), so
/// the first real formula doesn't stall a frame. Call off the main thread.
pub fn warm_up() {
    if AVAILABLE {
        let _ = typeset("$x$", 12.0);
    }
}

/// An `rgb("#rrggbb")` Typst colour for a Bevy colour.
pub fn typst_color(c: bevy::color::Color) -> String {
    use bevy::color::ColorToPacked;
    let [r, g, b, _] = c.to_srgba().to_u8_array();
    format!("rgb(\"#{r:02x}{g:02x}{b:02x}\")")
}

#[cfg(all(test, feature = "math"))]
mod tests {
    use super::*;

    #[test]
    fn typesets_math_and_reports_errors() {
        let img = typeset("$ sigma(z) = 1/(1 + e^(-z)) $", 20.0).unwrap();
        assert!(img.width > 10 && img.height > 10);
        assert_eq!(img.rgba.len(), img.width * img.height * 4);
        assert!(img.rgba.chunks(4).any(|p| p[3] > 0), "some ink");
        assert!(typeset("$ undefined_function_xyz(z) $", 20.0).is_err());
    }
}
