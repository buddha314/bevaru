//! Orbit the training objective over a model weight and bias. Hinge, squared
//! hinge, and logistic (binary cross-entropy) use the same fixed binary data.
//!
//! ```sh
//! cargo run --release --example loss_surface
//! ```

use bevaru::capture::CapturePlugin;
use bevaru::core::TrainingData;
use bevaru::core::loss::{LossKind, LossParams, Task};
use bevaru::core::model::LinearModel;
use bevaru::core::nalgebra::{DMatrix, DVector};
use bevaru::core::surface::{SurfaceSettings, sample_objective_surface};
use bevaru::loss_surface::objective_surface_mesh;
use bevaru::orbit::{OrbitPlugin, OrbitRig, OrbitView};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

const RESOLUTION: usize = 65;

#[derive(Resource)]
struct Controls {
    loss: LossKind,
    c: f64,
    logistic_lambda: f64,
    margin: f64,
    height_scale: f32,
    revision: u64,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            loss: LossKind::Hinge,
            c: 10.0,
            logistic_lambda: 0.1,
            margin: 1.0,
            height_scale: 1.0,
            revision: 0,
        }
    }
}

impl Controls {
    fn settings(&self) -> SurfaceSettings {
        SurfaceSettings {
            weight_index: 0,
            weight_range: (-3.0, 3.0),
            bias_range: (-3.0, 3.0),
            resolution: RESOLUTION,
            loss: self.loss,
            loss_params: LossParams::default().with_margin(self.margin).unwrap(),
            lambda: if self.loss == LossKind::Logistic {
                self.logistic_lambda
            } else {
                1.0 / self.c
            },
        }
    }
}

#[derive(Resource)]
struct SurfaceState {
    data: TrainingData,
    base: LinearModel,
    mesh: Handle<Mesh>,
    min: f64,
    max: f64,
    revision: u64,
    error: Option<String>,
}

const HOME: OrbitView = OrbitView {
    yaw: 0.8,
    pitch: 0.55,
    distance: 14.0,
    target: Vec3::new(0.0, 0.0, 1.5),
};

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "bevaru — loss surface".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }),
        EguiPlugin::default(),
        OrbitPlugin,
    ));
    if let Some(capture) = CapturePlugin::from_env() {
        app.add_plugins(capture);
    }
    app.init_resource::<Controls>()
        .insert_resource(ClearColor(Color::srgb(0.97, 0.97, 0.98)))
        .add_systems(Startup, setup)
        .add_systems(Update, (update_surface, draw_axes))
        .add_systems(EguiPrimaryContextPass, ui)
        .run();
}

fn setup(
    mut commands: Commands,
    controls: Res<Controls>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // One feature makes both horizontal axes directly interpretable. The data
    // stay fixed while the selected objective changes.
    let data = TrainingData {
        x: DMatrix::from_row_slice(8, 1, &[-2.0, -1.5, -1.0, -0.4, 0.3, 0.8, 1.4, 2.0]),
        y: DVector::from_row_slice(&[-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0]),
        task: Task::Classification,
    };
    let base = LinearModel::zeros(1);
    let sampled = sample_objective_surface(&data, &base, controls.settings()).unwrap();
    let mesh = meshes.add(objective_surface_mesh(&sampled, controls.height_scale).unwrap());
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
    ));
    commands.spawn(OrbitRig::bundle(HOME));
    commands.insert_resource(SurfaceState {
        data,
        base,
        mesh,
        min: sampled.min,
        max: sampled.max,
        revision: controls.revision,
        error: None,
    });
}

fn update_surface(
    controls: Res<Controls>,
    mut surface: Option<ResMut<SurfaceState>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(ref mut surface) = surface else {
        return;
    };
    if controls.revision == surface.revision {
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
        Err(e) => surface.error = Some(e.to_string()),
    }
    surface.revision = controls.revision;
}

fn ui(
    mut contexts: EguiContexts,
    mut controls: ResMut<Controls>,
    surface: Option<Res<SurfaceState>>,
    mut rig: Single<&mut OrbitRig>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let mut root = egui::Ui::new(
        ctx.clone(),
        "loss-surface-root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    egui::Panel::left("loss-surface-controls")
        .default_size(320.0)
        .show(&mut root, |ui| {
            ui.heading("Loss surface");
            ui.label("X (red): model weight");
            ui.label("Y (green): bias");
            ui.label("Z (blue): training objective");
            ui.small("Eight fixed, one-feature binary samples. Other weights are fixed at zero.");
            ui.separator();

            let mut changed = false;
            egui::ComboBox::from_label("Loss")
                .selected_text(match controls.loss {
                    LossKind::Hinge => "Hinge",
                    LossKind::SquaredHinge => "Squared hinge",
                    _ => "Logistic (binary cross-entropy)",
                })
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(&mut controls.loss, LossKind::Hinge, "Hinge")
                        .changed();
                    changed |= ui
                        .selectable_value(
                            &mut controls.loss,
                            LossKind::SquaredHinge,
                            "Squared hinge",
                        )
                        .changed();
                    changed |= ui
                        .selectable_value(
                            &mut controls.loss,
                            LossKind::Logistic,
                            "Logistic (binary cross-entropy)",
                        )
                        .changed();
                });
            if controls.loss == LossKind::Logistic {
                changed |= ui
                    .add(egui::Slider::new(&mut controls.logistic_lambda, 0.0..=2.0).text("L2 λ"))
                    .changed();
            } else {
                changed |= ui
                    .add(
                        egui::Slider::new(&mut controls.c, 0.1..=100.0)
                            .logarithmic(true)
                            .text("C"),
                    )
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut controls.margin, 0.1..=3.0).text("hinge margin"))
                    .changed();
            }
            changed |= ui
                .add(egui::Slider::new(&mut controls.height_scale, 0.2..=2.0).text("height scale"))
                .changed();
            if changed {
                controls.revision += 1;
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
                rig.reset = true;
            }
        });
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
