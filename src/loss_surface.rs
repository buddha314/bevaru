//! Reusable Bevy mesh for a sampled objective surface.

use bevaru_core::surface::ObjectiveSurface;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Build an indexed surface with X = selected weight, Y = bias, and
/// Z = objective value multiplied by `height_scale`.
pub fn objective_surface_mesh(
    surface: &ObjectiveSurface,
    height_scale: f32,
) -> Result<Mesh, &'static str> {
    if !height_scale.is_finite() || height_scale <= 0.0 {
        return Err("height scale must be finite and positive");
    }
    let n = surface.settings.resolution;
    if n < 2 || n.checked_mul(n) != Some(surface.values.len()) {
        return Err("surface samples must form a square grid of at least 2 by 2");
    }
    let mut positions = Vec::with_capacity(n * n);
    let mut colors = Vec::with_capacity(n * n);
    let span = (surface.max - surface.min).max(f64::EPSILON);
    for row in 0..n {
        for column in 0..n {
            let value = surface.value(column, row);
            let position = [
                surface.weight_at(column) as f32,
                surface.bias_at(row) as f32,
                (value as f32) * height_scale,
            ];
            if position.iter().any(|v| !v.is_finite()) {
                return Err("surface coordinates exceed the renderable range");
            }
            positions.push(position);
            let t = ((value - surface.min) / span).clamp(0.0, 1.0) as f32;
            colors.push([0.06 + 0.88 * t, 0.4 - 0.15 * t, 0.86 - 0.72 * t, 1.0]);
        }
    }

    let mut indices = Vec::with_capacity((n - 1) * (n - 1) * 6);
    for row in 0..n - 1 {
        for column in 0..n - 1 {
            let a = (row * n + column) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevaru_core::dataset::TrainingData;
    use bevaru_core::loss::{LossKind, LossParams, Task};
    use bevaru_core::model::LinearModel;
    use bevaru_core::nalgebra::{DMatrix, DVector};
    use bevaru_core::surface::{SurfaceSettings, sample_objective_surface};
    use bevy::mesh::VertexAttributeValues;

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
        let Some(VertexAttributeValues::Float32x3(vertices)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("surface mesh has no 3-D positions");
        };
        assert_eq!(vertices.len(), 4);
        assert_eq!(vertices[0], [-1.0, -2.0, (grid.value(0, 0) * 2.0) as f32]);
        assert_eq!(vertices[3], [1.0, 2.0, (grid.value(1, 1) * 2.0) as f32]);
        assert!(matches!(mesh.indices(), Some(Indices::U32(indices)) if indices.len() == 6));
        assert!(objective_surface_mesh(&grid, 0.0).is_err());
        let mut incomplete = grid;
        incomplete.values.pop();
        assert!(objective_surface_mesh(&incomplete, 1.0).is_err());
    }
}
