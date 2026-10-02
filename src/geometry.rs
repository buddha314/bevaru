//! Display-space geometry: decision boundaries, their interpolation, and
//! clipping to the visible data bounds. Pure functions, no ECS.

use bevy::math::{Vec2, Vec3};

/// A linear function on display space, `f(p) = scale · (n·p + offset)`, with
/// `n` a unit vector (`z = 0` for 2-D). Its zero set is the decision boundary;
/// `f = ±1` are the SVM margins.
///
/// Storing (unit normal, offset, scale) rather than raw `w` is what lets an
/// animation rotate the boundary instead of shrinking `w` through zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Boundary {
    pub normal: Vec3,
    pub offset: f32,
    pub scale: f32,
}

impl Boundary {
    /// From `f(p) = w·p + b`; `None` when `w` is (numerically) zero, since
    /// such a function has no boundary to draw.
    pub fn from_linear(w: Vec3, b: f32) -> Option<Self> {
        let scale = w.length();
        if !(scale.is_finite() && scale > 1e-9 && b.is_finite()) {
            return None;
        }
        Some(Self {
            normal: w / scale,
            offset: b / scale,
            scale,
        })
    }

    pub fn eval(&self, p: Vec3) -> f32 {
        self.scale * (self.normal.dot(p) + self.offset)
    }

    /// Distance from the boundary to the `f = ±1` margin.
    pub fn margin_width(&self) -> f32 {
        1.0 / self.scale
    }

    /// Interpolate: normals by spherical interpolation (rotation), offsets
    /// linearly, and scales geometrically (so margin widths change smoothly
    /// across orders of magnitude, as in a C sweep).
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            normal: slerp_unit(a.normal, b.normal, t),
            offset: a.offset + (b.offset - a.offset) * t,
            scale: a.scale * (b.scale / a.scale).powf(t),
        }
    }

    /// Points on the level set `f = level` inside the rectangle (2-D).
    pub fn clip_to_rect(&self, level: f32, min: Vec2, max: Vec2) -> Option<(Vec2, Vec2)> {
        let n = self.normal.truncate();
        let c = level / self.scale - self.offset; // n·p = c
        let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
        let mut hits: Vec<Vec2> = Vec::with_capacity(4);
        for i in 0..4 {
            let (p, q) = (corners[i], corners[(i + 1) % 4]);
            let (fp, fq) = (n.dot(p) - c, n.dot(q) - c);
            if (fp <= 0.0 && fq > 0.0) || (fp > 0.0 && fq <= 0.0) {
                hits.push(p + (q - p) * (fp / (fp - fq)));
            }
        }
        match hits.as_slice() {
            [a, b, ..] if a.distance(*b) > 1e-6 => Some((*a, *b)),
            _ => None,
        }
    }

    /// The polygon where the level set `f = level` cuts the box (3-D),
    /// ordered around its centroid; empty when the plane misses the box.
    pub fn clip_to_box(&self, level: f32, min: Vec3, max: Vec3) -> Vec<Vec3> {
        let c = level / self.scale - self.offset;
        let corner = |i: usize| {
            Vec3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { min.y } else { max.y },
                if i & 4 == 0 { min.z } else { max.z },
            )
        };
        let mut pts = Vec::new();
        for i in 0..8 {
            for bit in [1, 2, 4] {
                let j = i | bit;
                if j == i {
                    continue;
                }
                let (p, q) = (corner(i), corner(j));
                let (fp, fq) = (self.normal.dot(p) - c, self.normal.dot(q) - c);
                if (fp <= 0.0 && fq > 0.0) || (fp > 0.0 && fq <= 0.0) {
                    pts.push(p + (q - p) * (fp / (fp - fq)));
                }
            }
        }
        if pts.len() < 3 {
            return Vec::new();
        }
        let center = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
        let u = self.normal.any_orthonormal_vector();
        let v = self.normal.cross(u);
        pts.sort_by(|a, b| {
            let (da, db) = (*a - center, *b - center);
            da.dot(v)
                .atan2(da.dot(u))
                .total_cmp(&db.dot(v).atan2(db.dot(u)))
        });
        pts.dedup_by(|a, b| a.distance(*b) < 1e-6);
        pts
    }
}

/// Rotate unit vector `a` toward `b` by fraction `t` of the angle between
/// them. Antiparallel vectors rotate about an arbitrary perpendicular axis
/// (in-plane for 2-D vectors) rather than passing through zero.
pub fn slerp_unit(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let dot = a.dot(b).clamp(-1.0, 1.0);
    if dot > 0.9995 {
        return a.lerp(b, t).normalize();
    }
    let theta = dot.acos();
    let perp = b - a * dot;
    let u = if perp.length_squared() > 1e-10 {
        perp.normalize()
    } else if a.z == 0.0 && b.z == 0.0 {
        Vec3::new(-a.y, a.x, 0.0)
    } else {
        a.any_orthonormal_vector()
    };
    (a * (theta * t).cos() + u * (theta * t).sin()).normalize()
}

/// Smoothstep easing.
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(w: Vec2, b: f32) -> Boundary {
        Boundary::from_linear(w.extend(0.0), b).unwrap()
    }

    #[test]
    fn zero_weights_have_no_boundary() {
        assert_eq!(Boundary::from_linear(Vec3::ZERO, 1.0), None);
        assert_eq!(Boundary::from_linear(Vec3::splat(f32::NAN), 1.0), None);
    }

    #[test]
    fn eval_matches_linear_function() {
        let w = Vec3::new(2.0, -1.0, 0.5);
        let b = Boundary::from_linear(w, 3.0).unwrap();
        let p = Vec3::new(0.3, 1.7, -2.0);
        assert!((b.eval(p) - (w.dot(p) + 3.0)).abs() < 1e-5);
        assert!((b.margin_width() - 1.0 / w.length()).abs() < 1e-6);
    }

    #[test]
    fn interpolation_rotates_instead_of_collapsing() {
        // w and −w: a naive lerp of w passes through 0 at t = 0.5.
        let a = line(Vec2::X, 0.0);
        let b = line(-Vec2::X, 0.0);
        for i in 0..=10 {
            let m = Boundary::lerp(&a, &b, i as f32 / 10.0);
            assert!((m.normal.length() - 1.0).abs() < 1e-5);
            assert_eq!(m.normal.z, 0.0, "2-D boundaries rotate in-plane");
            assert!(m.scale > 0.0);
        }
        let mid = Boundary::lerp(&a, &b, 0.5);
        assert!(
            mid.normal.dot(Vec3::X).abs() < 1e-5,
            "halfway is perpendicular"
        );
    }

    #[test]
    fn interpolation_passes_through_intermediate_angles() {
        let a = line(Vec2::X, 0.0);
        let b = line(Vec2::Y, 0.0);
        let mid = Boundary::lerp(&a, &b, 0.5);
        let expected = Vec3::new(1.0, 1.0, 0.0).normalize();
        assert!((mid.normal - expected).length() < 1e-5);
        assert_eq!(Boundary::lerp(&a, &b, 0.0), a);
        assert!((Boundary::lerp(&a, &b, 1.0).normal - b.normal).length() < 1e-5);
    }

    #[test]
    fn scale_interpolates_geometrically() {
        let a = Boundary {
            scale: 0.01,
            ..line(Vec2::X, 0.0)
        };
        let b = Boundary {
            scale: 100.0,
            ..line(Vec2::X, 0.0)
        };
        assert!((Boundary::lerp(&a, &b, 0.5).scale - 1.0).abs() < 1e-4);
    }

    #[test]
    fn line_clips_to_rect() {
        // x = 1 through a [-2, 2]² box.
        let b = line(Vec2::X, -1.0);
        let (p, q) = b
            .clip_to_rect(0.0, Vec2::splat(-2.0), Vec2::splat(2.0))
            .unwrap();
        assert!((p.x - 1.0).abs() < 1e-6 && (q.x - 1.0).abs() < 1e-6);
        assert!((p.y - q.y).abs() > 3.9);
        // Margin f = +1 is at x = 2 for scale 1.
        let (m, _) = b
            .clip_to_rect(1.0, Vec2::splat(-3.0), Vec2::splat(3.0))
            .unwrap();
        assert!((m.x - 2.0).abs() < 1e-6);
        // Outside the box: nothing to draw.
        assert!(
            b.clip_to_rect(0.0, Vec2::splat(5.0), Vec2::splat(6.0))
                .is_none()
        );
    }

    #[test]
    fn plane_clips_to_box() {
        // z = 0 through the unit cube centered at 0: a square.
        let b = Boundary::from_linear(Vec3::Z, 0.0).unwrap();
        let poly = b.clip_to_box(0.0, Vec3::splat(-1.0), Vec3::splat(1.0));
        assert_eq!(poly.len(), 4);
        assert!(poly.iter().all(|p| p.z.abs() < 1e-6));
        // A diagonal cut x + y + z = 0 through the cube is a hexagon.
        let d = Boundary::from_linear(Vec3::ONE, 0.0).unwrap();
        assert_eq!(
            d.clip_to_box(0.0, Vec3::splat(-1.0), Vec3::splat(1.0))
                .len(),
            6
        );
        assert!(
            b.clip_to_box(0.0, Vec3::splat(2.0), Vec3::splat(3.0))
                .is_empty()
        );
    }
}
