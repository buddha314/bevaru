//! Reusable Bevy meshes for sampled surfaces: #18's objective surfaces over
//! model parameters, and the loss shapes of [`bevaru_core::shapes`].
//!
//! A [`GridMap`] places a [`SurfaceGrid`] in the world (Z is up), and
//! [`HeightColors`] colours it with ruviz's cool-to-warm colormap, so the live
//! mesh and ruviz's rendered surfaces look alike.

use bevaru_core::shapes::SurfaceGrid;
use bevaru_core::surface::ObjectiveSurface;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The box loss shapes are fitted into: [`BOX_SIDE`] × [`BOX_SIDE`] in the
/// ground plane, [`BOX_HEIGHT`] tall, centred on the origin with the lowest
/// value at Z = 0.
pub const BOX_SIDE: f32 = 10.0;
pub const BOX_HEIGHT: f32 = 6.0;

/// An affine map from grid inputs and values to the world: the ground point is
/// `origin + x·ex + y·ey`, and the height is `value·height_scale + height_offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMap {
    pub origin: Vec2,
    pub ex: Vec2,
    pub ey: Vec2,
    pub height_scale: f32,
    pub height_offset: f32,
}

impl GridMap {
    /// Inputs are world units; heights are scaled. #18's original mapping.
    pub fn identity(height_scale: f32) -> Self {
        Self {
            origin: Vec2::ZERO,
            ex: Vec2::X,
            ey: Vec2::Y,
            height_scale,
            height_offset: 0.0,
        }
    }

    /// Fit the grid's axes and value range into the fixed box. Each input axis
    /// spans the box side; where both axes have the same range (most views),
    /// they share one scale and diagonals stay at 45°.
    pub fn fit(grid: &SurfaceGrid) -> Self {
        let sx = BOX_SIDE / (grid.x.max - grid.x.min) as f32;
        let sy = BOX_SIDE / (grid.y.max - grid.y.min) as f32;
        let half = BOX_SIDE / 2.0;
        Self {
            origin: Vec2::new(
                -half - grid.x.min as f32 * sx,
                -half - grid.y.min as f32 * sy,
            ),
            ex: Vec2::new(sx, 0.0),
            ey: Vec2::new(0.0, sy),
            ..Self::identity(1.0)
        }
        .with_heights(grid)
    }

    /// The probability triangle drawn equilateral: the grid is (q₂, q₃) with
    /// q₁ = 1 − q₂ − q₃, and the corners are where each class has probability 1.
    pub fn ternary(grid: &SurfaceGrid) -> Self {
        let [q1, q2, q3] = Self::ternary_corners();
        Self {
            origin: q1,
            ex: q2 - q1,
            ey: q3 - q1,
            ..Self::identity(1.0)
        }
        .with_heights(grid)
    }

    /// World ground positions of the corners where class 1 (the true class),
    /// 2, and 3 have probability 1: an equilateral triangle of side
    /// [`BOX_SIDE`], centred on the origin. The true class is at the front, so the wall where
    /// q₁ → 0 stands at the back and doesn't hide the valley.
    pub fn ternary_corners() -> [Vec2; 3] {
        let h = BOX_SIDE * 3f32.sqrt() / 2.0;
        [
            Vec2::new(0.0, -2.0 * h / 3.0),
            Vec2::new(-BOX_SIDE / 2.0, h / 3.0),
            Vec2::new(BOX_SIDE / 2.0, h / 3.0),
        ]
    }

    fn with_heights(mut self, grid: &SurfaceGrid) -> Self {
        let span = (grid.max - grid.min).max(1e-9) as f32;
        self.height_scale = BOX_HEIGHT / span;
        self.height_offset = -grid.min as f32 * self.height_scale;
        self
    }

    pub fn ground(&self, x: f64, y: f64) -> Vec2 {
        self.origin + x as f32 * self.ex + y as f32 * self.ey
    }

    pub fn height(&self, value: f64) -> f32 {
        value as f32 * self.height_scale + self.height_offset
    }

    pub fn world(&self, x: f64, y: f64, value: f64) -> Vec3 {
        self.ground(x, y).extend(self.height(value))
    }

    /// The grid inputs at a ground point.
    pub fn inputs(&self, ground: Vec2) -> (f64, f64) {
        let m = Mat2::from_cols(self.ex, self.ey);
        let v = m.inverse() * (ground - self.origin);
        (v.x as f64, v.y as f64)
    }

    /// The value at a world height.
    pub fn value(&self, height: f32) -> f64 {
        ((height - self.height_offset) / self.height_scale) as f64
    }

    /// Map an input-space direction (e.g. a gradient) to the ground plane.
    pub fn ground_direction(&self, dx: f64, dy: f64) -> Vec2 {
        dx as f32 * self.ex + dy as f32 * self.ey
    }
}

/// Heights coloured with ruviz's `coolwarm` (blue–white–red) over a value
/// range, as linear RGBA for vertex colours.
#[derive(Debug, Clone)]
pub struct HeightColors {
    pub min: f64,
    pub max: f64,
    lut: Vec<[f32; 4]>,
}

impl HeightColors {
    const STEPS: usize = 256;

    pub fn coolwarm(min: f64, max: f64) -> Self {
        let map = ruviz::render::ColorMap::coolwarm();
        let lut = (0..Self::STEPS)
            .map(|i| {
                let c = map.sample(i as f64 / (Self::STEPS - 1) as f64);
                Color::srgba_u8(c.r, c.g, c.b, 255)
                    .to_linear()
                    .to_f32_array()
            })
            .collect();
        Self { min, max, lut }
    }

    /// Position of `value` in the range, 0 to 1.
    pub fn fraction(&self, value: f64) -> f64 {
        let span = (self.max - self.min).max(1e-12);
        ((value - self.min) / span).clamp(0.0, 1.0)
    }

    pub fn linear(&self, value: f64) -> [f32; 4] {
        let i = (self.fraction(value) * (Self::STEPS - 1) as f64).round() as usize;
        self.lut[i]
    }

    /// The colour at fraction `t` of the range, for legends.
    pub fn at_fraction(&self, t: f64) -> Color {
        let i = (t.clamp(0.0, 1.0) * (Self::STEPS - 1) as f64).round() as usize;
        Color::LinearRgba(LinearRgba::from_f32_array(self.lut[i]))
    }
}

/// Build an indexed, lit surface for `grid` placed by `map`, coloured by
/// `colors`. Masked samples leave holes: a triangle is drawn only when all
/// three corners are present, so the probability triangle keeps a straight
/// hypotenuse.
pub fn grid_mesh(
    grid: &SurfaceGrid,
    map: &GridMap,
    colors: &HeightColors,
) -> Result<Mesh, &'static str> {
    if !map.height_scale.is_finite() || map.height_scale <= 0.0 {
        return Err("height scale must be finite and positive");
    }
    let (nx, ny) = (grid.x.n, grid.y.n);
    if nx < 2 || ny < 2 || nx.checked_mul(ny) != Some(grid.values.len()) {
        return Err("surface samples must form a grid of at least 2 by 2");
    }
    let mut positions = Vec::with_capacity(nx * ny);
    let mut vertex_colors = Vec::with_capacity(nx * ny);
    for row in 0..ny {
        for column in 0..nx {
            let value = grid.value(column, row);
            let position = map.world(grid.x.at(column), grid.y.at(row), value.unwrap_or(grid.min));
            if !position.is_finite() {
                return Err("surface coordinates exceed the renderable range");
            }
            positions.push(position.to_array());
            vertex_colors.push(colors.linear(value.unwrap_or(grid.min)));
        }
    }

    let present = |i: u32| grid.values[i as usize].is_some();
    let mut indices = Vec::with_capacity((nx - 1) * (ny - 1) * 6);
    for row in 0..ny - 1 {
        for column in 0..nx - 1 {
            let a = (row * nx + column) as u32;
            let b = a + 1;
            let c = a + nx as u32;
            let d = c + 1;
            for tri in [[a, b, c], [b, d, c]] {
                if tri.into_iter().all(present) {
                    indices.extend_from_slice(&tri);
                }
            }
        }
    }
    if indices.is_empty() {
        return Err("surface has no samples to draw");
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vertex_colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_smooth_normals();
    Ok(mesh)
}

/// Build an indexed surface with X = selected weight, Y = bias, and
/// Z = objective value multiplied by `height_scale`.
pub fn objective_surface_mesh(
    surface: &ObjectiveSurface,
    height_scale: f32,
) -> Result<Mesh, &'static str> {
    let grid = surface.to_grid();
    grid_mesh(
        &grid,
        &GridMap::identity(height_scale),
        &HeightColors::coolwarm(grid.min, grid.max),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevaru_core::dataset::TrainingData;
    use bevaru_core::loss::{LossKind, LossParams, Task};
    use bevaru_core::model::LinearModel;
    use bevaru_core::nalgebra::{DMatrix, DVector};
    use bevaru_core::shapes::{ShapeParams, ShapeView};
    use bevaru_core::surface::{SurfaceSettings, sample_objective_surface};
    use bevy::mesh::VertexAttributeValues;

    fn positions(mesh: &Mesh) -> &[[f32; 3]] {
        let Some(VertexAttributeValues::Float32x3(vertices)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("surface mesh has no 3-D positions");
        };
        vertices
    }

    fn triangles(mesh: &Mesh) -> usize {
        match mesh.indices() {
            Some(Indices::U32(i)) => i.len() / 3,
            _ => panic!("no u32 indices"),
        }
    }

    #[test]
    fn mesh_preserves_parameter_coordinates_and_heights() {
        let data = TrainingData {
            x: DMatrix::from_row_slice(1, 1, &[1.0]),
            y: DVector::from_row_slice(&[1.0]),
            task: Task::Classification,
        };
        let grid = sample_objective_surface(
            &data,
            &LinearModel::zeros(1),
            SurfaceSettings {
                weight_index: 0,
                weight_range: (-1.0, 1.0),
                bias_range: (-2.0, 2.0),
                resolution: 2,
                loss: LossKind::Hinge,
                loss_params: LossParams::default(),
                lambda: 0.0,
            },
        )
        .unwrap();
        let mesh = objective_surface_mesh(&grid, 2.0).unwrap();
        let vertices = positions(&mesh);
        assert_eq!(vertices.len(), 4);
        assert_eq!(vertices[0], [-1.0, -2.0, (grid.value(0, 0) * 2.0) as f32]);
        assert_eq!(vertices[3], [1.0, 2.0, (grid.value(1, 1) * 2.0) as f32]);
        assert_eq!(triangles(&mesh), 2);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(objective_surface_mesh(&grid, 0.0).is_err());
        let mut incomplete = grid;
        incomplete.values.pop();
        assert!(objective_surface_mesh(&incomplete, 1.0).is_err());
    }

    #[test]
    fn fitted_shapes_fill_the_box() {
        let grid = ShapeView::PredictionMse
            .sample(&ShapeParams::default(), 9)
            .unwrap();
        let map = GridMap::fit(&grid);
        let mesh = grid_mesh(&grid, &map, &HeightColors::coolwarm(grid.min, grid.max)).unwrap();
        let v = positions(&mesh);
        let (lo, hi) = v.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| {
            (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))
        });
        let half = BOX_SIDE / 2.0;
        assert!(lo.abs_diff_eq(Vec3::new(-half, -half, 0.0), 1e-4), "{lo}");
        assert!(
            hi.abs_diff_eq(Vec3::new(half, half, BOX_HEIGHT), 1e-4),
            "{hi}"
        );
        // Round trip from the ground plane back to inputs.
        let (x, y) = map.inputs(map.ground(1.25, -2.5));
        assert!((x - 1.25).abs() < 1e-5 && (y + 2.5).abs() < 1e-5);
        assert!((map.value(map.height(3.0)) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn triangle_view_is_triangular_and_equilateral() {
        let grid = ShapeView::ThreeClassProbabilities
            .sample(&ShapeParams::default(), 11)
            .unwrap();
        let map = GridMap::ternary(&grid);
        let mesh = grid_mesh(&grid, &map, &HeightColors::coolwarm(grid.min, grid.max)).unwrap();
        // A full square would have 2·10² triangles; the triangle has 10².
        assert_eq!(triangles(&mesh), 100);
        let [a, b, c] = GridMap::ternary_corners();
        for (p, q) in [(a, b), (b, c), (c, a)] {
            assert!((p.distance(q) - BOX_SIDE).abs() < 1e-4);
        }
        assert!(map.ground(0.0, 0.0).abs_diff_eq(a, 1e-5));
        assert!(map.ground(1.0, 0.0).abs_diff_eq(b, 1e-5));
        assert!(map.ground(0.0, 1.0).abs_diff_eq(c, 1e-5));
    }

    #[test]
    fn colors_run_cool_to_warm() {
        let colors = HeightColors::coolwarm(0.0, 8.0);
        let [lr, _, lb, _] = colors.linear(0.0);
        let [hr, _, hb, _] = colors.linear(8.0);
        assert!(lb > lr && hr > hb, "low is blue, high is red");
        assert_eq!(colors.linear(-5.0), colors.linear(0.0), "clamped");
    }
}
