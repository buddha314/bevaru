//! "Training objective in 3D" (#18): orbit the training objective over a
//! model weight and bias. Hinge, squared hinge, and logistic (binary
//! cross-entropy) use the same fixed binary data.

use bevaru_core::TrainingData;
use bevaru_core::loss::{LossKind, LossParams, Task};
use bevaru_core::model::LinearModel;
use bevaru_core::nalgebra::{DMatrix, DVector};
use bevaru_core::surface::{SurfaceSettings, sample_objective_surface};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use super::layout::SideInsets;
use super::{ExperienceEntity, ExperienceStarted, ExperienceStopped, in_experience};
use crate::capture::HideOverlays;
use crate::loss_surface::objective_surface_mesh;
use crate::orbit::{OrbitPlugin, OrbitRig, OrbitView};

pub const ID: &str = "loss-surface";

const RESOLUTION: usize = 65;

const HOME: OrbitView = OrbitView {
    yaw: 0.8,
    pitch: 0.55,
    distance: 14.0,
    target: Vec3::new(0.0, 0.0, 1.5),
};

pub struct LossSurfaceExperiencePlugin;

impl Plugin for LossSurfaceExperiencePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<OrbitPlugin>() {
            app.add_plugins(OrbitPlugin);
        }
        app.add_observer(start)
            .add_observer(stop)
            .add_systems(
                Update,
                (update_surface, draw_axes).run_if(in_experience(ID)),
            )
            .add_systems(EguiPrimaryContextPass, ui.run_if(in_experience(ID)));
    }
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ObjectiveControls {
    pub loss: LossKind,
    pub c: f64,
    pub logistic_lambda: f64,
    pub margin: f64,
    pub height_scale: f32,
}

impl Default for ObjectiveControls {
    fn default() -> Self {
        Self {
            loss: LossKind::Hinge,
            c: 10.0,
            logistic_lambda: 0.1,
            margin: 1.0,
            height_scale: 1.0,
        }
    }
}

impl ObjectiveControls {
    fn settings(&self) -> SurfaceSettings {
        SurfaceSettings {
            weight_index: 0,
            weight_range: (-3.0, 3.0),
            bias_range: (-3.0, 3.0),
            resolution: RESOLUTION,
            loss: self.loss,
            loss_params: LossParams::default()
                .with_margin(self.margin)
                .unwrap_or_default(),
            lambda: if self.loss == LossKind::Logistic {
                self.logistic_lambda
            } else {
                1.0 / self.c
            },
        }
    }
}

/// The sampled objective on screen, and the controls it reflects.
#[derive(Resource)]
pub struct ObjectiveSurfaceState {
    data: TrainingData,
    base: LinearModel,
    mesh: Handle<Mesh>,
    pub shown: ObjectiveControls,
    pub min: f64,
    pub max: f64,
    pub error: Option<String>,
}

/// Eight fixed one-feature binary samples, so both horizontal axes (the
/// weight and the bias) are directly interpretable.
fn data() -> TrainingData {
    TrainingData {
        x: DMatrix::from_row_slice(8, 1, &[-2.0, -1.5, -1.0, -0.4, 0.3, 0.8, 1.4, 2.0]),
        y: DVector::from_row_slice(&[-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0]),
        task: Task::Classification,
    }
}

fn start(
    started: On<ExperienceStarted>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if started.id != ID {
        return;
    }
    let controls = ObjectiveControls::default();
    let (data, base) = (data(), LinearModel::zeros(1));
    let sampled = sample_objective_surface(&data, &base, controls.settings())
        .map_err(|e| e.to_string())
        .and_then(|grid| {
            objective_surface_mesh(&grid, controls.height_scale)
                .map(|mesh| (grid, mesh))
                .map_err(|e| e.to_string())
        });
    let (grid, mesh) = match sampled {
        Ok(ok) => ok,
        Err(e) => {
            error!("bevaru: could not sample the objective surface: {e}");
            return;
        }
    };
    let mesh = meshes.add(mesh);
    commands.spawn((
        ExperienceEntity,
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
    ));
    commands.spawn((
        ExperienceEntity,
        OrbitRig::bundle(HOME),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.97, 0.97, 0.98)),
            ..default()
        },
    ));
    commands.insert_resource(ObjectiveSurfaceState {
        data,
        base,
        mesh,
        shown: controls.clone(),
        min: grid.min,
        max: grid.max,
        error: None,
    });
    commands.insert_resource(controls);
}

fn stop(stopped: On<ExperienceStopped>, mut commands: Commands) {
    if stopped.id == ID {
        commands.remove_resource::<ObjectiveSurfaceState>();
        commands.remove_resource::<ObjectiveControls>();
    }
}

fn update_surface(
    controls: Option<Res<ObjectiveControls>>,
    surface: Option<ResMut<ObjectiveSurfaceState>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Some(controls), Some(mut surface)) = (controls, surface) else {
        return;
    };
    if surface.shown == *controls {
        return;
    }
    match sample_objective_surface(&surface.data, &surface.base, controls.settings())
        .map_err(|e| e.to_string())
        .and_then(|grid| {
            objective_surface_mesh(&grid, controls.height_scale)
                .map(|mesh| (grid, mesh))
                .map_err(|e| e.to_string())
        }) {
        Ok((grid, mesh)) => {
            if let Some(mut current) = meshes.get_mut(&surface.mesh) {
                *current = mesh;
            }
            surface.min = grid.min;
            surface.max = grid.max;
            surface.error = None;
        }
        Err(e) => surface.error = Some(e),
    }
    surface.shown = controls.clone();
}

fn ui(
    mut contexts: EguiContexts,
    controls: Option<ResMut<ObjectiveControls>>,
    surface: Option<Res<ObjectiveSurfaceState>>,
    mut rigs: Query<&mut OrbitRig, With<ExperienceEntity>>,
    mut insets: ResMut<SideInsets>,
    hide: Option<Res<HideOverlays>>,
) -> Result {
    let Some(mut controls) = controls else {
        return Ok(());
    };
    if hide.is_some() {
        SideInsets::set(&mut insets, SideInsets::default());
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    let mut root = egui::Ui::new(
        ctx.clone(),
        "loss-surface-root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    let panel = egui::Panel::left("loss-surface-controls")
        .default_size(320.0)
        .show(&mut root, |ui| {
            // Room for the lobby's "◀ Lobby" button.
            ui.add_space(36.0);
            ui.heading("Training objective in 3D");
            ui.label("X (red): model weight");
            ui.label("Y (green): bias");
            ui.label("Z (blue): training objective");
            ui.small("Eight fixed, one-feature binary samples. Other weights are fixed at zero.");
            ui.separator();

            let mut c = controls.clone();
            egui::ComboBox::from_label("Loss")
                .selected_text(match c.loss {
                    LossKind::Hinge => "Hinge",
                    LossKind::SquaredHinge => "Squared hinge",
                    _ => "Logistic (binary cross-entropy)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut c.loss, LossKind::Hinge, "Hinge");
                    ui.selectable_value(&mut c.loss, LossKind::SquaredHinge, "Squared hinge");
                    ui.selectable_value(
                        &mut c.loss,
                        LossKind::Logistic,
                        "Logistic (binary cross-entropy)",
                    );
                });
            if c.loss == LossKind::Logistic {
                ui.add(egui::Slider::new(&mut c.logistic_lambda, 0.0..=2.0).text("L2 λ"));
            } else {
                ui.add(
                    egui::Slider::new(&mut c.c, 0.1..=100.0)
                        .logarithmic(true)
                        .text("C"),
                );
                ui.add(egui::Slider::new(&mut c.margin, 0.1..=3.0).text("hinge margin"));
            }
            ui.add(egui::Slider::new(&mut c.height_scale, 0.2..=2.0).text("height scale"));
            if c != *controls {
                *controls = c;
            }
            ui.separator();
            if let Some(surface) = surface.as_ref() {
                ui.label(format!(
                    "Objective range: {:.3} to {:.3}",
                    surface.min, surface.max
                ));
                if let Some(error) = &surface.error {
                    ui.colored_label(egui::Color32::RED, error);
                }
            }
            ui.small("Left-drag to orbit; right-drag to pan; scroll to zoom.");
            if ui.button("Frame surface (F)").clicked() {
                for mut rig in &mut rigs {
                    rig.reset = true;
                }
            }
        });
    SideInsets::set(
        &mut insets,
        SideInsets {
            left: panel.response.rect.width(),
            right: 0.0,
        },
    );
    Ok(())
}

fn draw_axes(mut gizmos: Gizmos) {
    gizmos.line(
        Vec3::new(-3.4, 3.2, 0.0),
        Vec3::new(3.2, 3.2, 0.0),
        Color::srgb(0.85, 0.1, 0.1),
    );
    gizmos.line(
        Vec3::new(3.2, -3.4, 0.0),
        Vec3::new(3.2, 3.2, 0.0),
        Color::srgb(0.1, 0.65, 0.25),
    );
    gizmos.line(
        Vec3::new(3.2, 3.2, 0.0),
        Vec3::new(3.2, 3.2, 5.0),
        Color::srgb(0.1, 0.3, 0.85),
    );
}
