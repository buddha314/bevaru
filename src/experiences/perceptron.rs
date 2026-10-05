//! "Perceptron in 3D": the diagram of [`crate::diagram`] as a slide-ready
//! 3-D scene, with live weights, typeset formulas on hover, and a slide view
//! (`H`) that hides every control so a window capture is the slide.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use super::{ExperienceEntity, ExperienceStarted, ExperienceStopped, in_experience};
use crate::capture::HideOverlays;
use crate::diagram::{Activation, Diagram, DiagramAssets, HoverTarget, pick, spawn_diagram};
use crate::formula::{Formula, typeset};
use crate::orbit::{OrbitPlugin, OrbitRig, OrbitView};

pub const ID: &str = "perceptron";

/// A gentle three-quarter view: depth is visible, the layout still reads.
pub const HOME: OrbitView = OrbitView {
    yaw: -1.22,
    pitch: 0.32,
    distance: 13.5,
    target: Vec3::new(0.6, 0.0, 0.0),
};

/// Nearly front-on and filling a 16:9 window, for slides.
pub const SLIDE: OrbitView = OrbitView {
    yaw: -1.4,
    pitch: 0.16,
    distance: 13.5,
    target: Vec3::new(0.6, 0.0, 0.1),
};

pub struct PerceptronExperiencePlugin;

impl Plugin for PerceptronExperiencePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<OrbitPlugin>() {
            app.add_plugins(OrbitPlugin);
        }
        app.add_observer(start)
            .add_observer(stop)
            .add_systems(
                Update,
                (toggle_slide_view, rebuild).run_if(in_experience(ID)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (controls, labels, formula_tooltip)
                    .chain()
                    .run_if(in_experience(ID)),
            );
    }
}

/// What the student has set. A change rebuilds the diagram.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct PerceptronControls {
    pub weights: [f64; 3],
    pub bias: f64,
    pub activation: Activation,
    pub labels: bool,
}

impl Default for PerceptronControls {
    fn default() -> Self {
        Self {
            weights: [0.8, -0.5, 0.3],
            bias: 0.1,
            activation: Activation::Sigmoid,
            labels: true,
        }
    }
}

impl PerceptronControls {
    pub fn diagram(&self) -> Diagram {
        Diagram::perceptron(self.weights, self.bias, self.activation)
    }
}

/// The diagram on screen and the controls it reflects.
#[derive(Resource, Debug)]
pub struct PerceptronScene {
    pub root: Entity,
    pub shown: PerceptronControls,
    pub diagram: Diagram,
}

/// Whether slide view is on, and whether it added `HideOverlays` (a
/// thumbnail capture may have added it already; that one is left alone).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SlideView {
    pub on: bool,
    inserted_hide: bool,
}

fn start(
    started: On<ExperienceStarted>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    hidden: Option<Res<HideOverlays>>,
) {
    if started.id != ID {
        return;
    }
    // Load the typesetting engine and fonts now, off the main thread, so the
    // first hover doesn't stall a frame.
    bevy::tasks::AsyncComputeTaskPool::get()
        .spawn(async { crate::formula::warm_up() })
        .detach();
    let assets = DiagramAssets::new(&mut meshes, &mut materials);
    let controls = PerceptronControls::default();
    let diagram = controls.diagram();
    let root = spawn_diagram(
        &mut commands,
        &mut meshes,
        &assets,
        &diagram,
        ExperienceEntity,
    );
    // Key light from the upper front left, a dim fill from the right, and a
    // rim light from behind that outlines each pill's edge. Ambient light is
    // kept low so the shading, not flat colour, shows the volume.
    for (direction, illuminance) in [
        (Vec3::new(0.55, 1.0, -0.75), 7000.0),
        (Vec3::new(-0.8, 0.6, -0.1), 1400.0),
        (Vec3::new(-0.2, -1.0, -0.45), 3800.0),
    ] {
        commands.spawn((
            ExperienceEntity,
            DirectionalLight {
                illuminance,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::default().looking_to(direction, Vec3::Z),
        ));
    }
    // `BEVARU_SLIDE_VIEW=1` opens in slide view, for scripted slide captures.
    let slide_on = std::env::var_os("BEVARU_SLIDE_VIEW").is_some();
    let slide = SlideView {
        on: slide_on,
        inserted_hide: slide_on && hidden.is_none(),
    };
    if slide.inserted_hide {
        commands.init_resource::<HideOverlays>();
    }
    let view = if slide_on { SLIDE } else { HOME };
    commands.spawn((
        ExperienceEntity,
        OrbitRig::bundle(view),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::WHITE),
            ..default()
        },
        AmbientLight {
            brightness: 350.0,
            ..default()
        },
    ));
    commands.insert_resource(assets);
    commands.insert_resource(PerceptronScene {
        root,
        shown: controls.clone(),
        diagram,
    });
    commands.insert_resource(controls);
    commands.insert_resource(slide);
}

fn stop(stopped: On<ExperienceStopped>, mut commands: Commands, slide: Option<Res<SlideView>>) {
    if stopped.id != ID {
        return;
    }
    if slide.is_some_and(|s| s.inserted_hide) {
        commands.remove_resource::<HideOverlays>();
    }
    commands.remove_resource::<PerceptronControls>();
    commands.remove_resource::<PerceptronScene>();
    commands.remove_resource::<DiagramAssets>();
    commands.remove_resource::<SlideView>();
}

/// Replace the diagram when the controls change. The shared meshes and
/// materials stay; only the entities (and the group backdrop) are rebuilt.
fn rebuild(
    mut commands: Commands,
    controls: Option<Res<PerceptronControls>>,
    scene: Option<ResMut<PerceptronScene>>,
    assets: Option<Res<DiagramAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Some(controls), Some(mut scene), Some(assets)) = (controls, scene, assets) else {
        return;
    };
    if scene.shown == *controls {
        return;
    }
    let diagram = controls.diagram();
    commands.entity(scene.root).despawn();
    scene.root = spawn_diagram(
        &mut commands,
        &mut meshes,
        &assets,
        &diagram,
        ExperienceEntity,
    );
    scene.diagram = diagram;
    scene.shown = controls.clone();
}

/// `H` turns slide view on and off: the controls and lobby button hide, the
/// camera frames the diagram for 16:9, and the labels stay.
fn toggle_slide_view(
    mut commands: Commands,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    egui: Option<Res<bevy_egui::input::EguiWantsInput>>,
    hidden: Option<Res<HideOverlays>>,
    slide: Option<ResMut<SlideView>>,
    mut rigs: Query<&mut OrbitRig, With<ExperienceEntity>>,
) {
    let (Some(keys), Some(mut slide)) = (keys, slide) else {
        return;
    };
    if egui.is_some_and(|e| e.wants_any_keyboard_input()) || !keys.just_pressed(KeyCode::KeyH) {
        return;
    }
    slide.on = !slide.on;
    if slide.on && hidden.is_none() {
        commands.init_resource::<HideOverlays>();
        slide.inserted_hide = true;
    } else if !slide.on && slide.inserted_hide {
        commands.remove_resource::<HideOverlays>();
        slide.inserted_hide = false;
    }
    let view = if slide.on { SLIDE } else { HOME };
    for mut rig in &mut rigs {
        rig.home = view;
        rig.view = view;
    }
}

fn controls(
    mut contexts: EguiContexts,
    controls: Option<ResMut<PerceptronControls>>,
    hide: Option<Res<HideOverlays>>,
    mut rigs: Query<&mut OrbitRig, With<ExperienceEntity>>,
) -> Result {
    let Some(mut controls) = controls else {
        return Ok(());
    };
    if hide.is_some() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut c = controls.clone();
    egui::Window::new("Perceptron")
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .resizable(false)
        .show(ctx, |ui| {
            for (i, w) in c.weights.iter_mut().enumerate() {
                ui.add(egui::Slider::new(w, -2.0..=2.0).text(format!("w{}", i + 1)));
            }
            ui.add(egui::Slider::new(&mut c.bias, -2.0..=2.0).text("bias b"));
            ui.horizontal(|ui| {
                ui.label("Activation");
                ui.selectable_value(&mut c.activation, Activation::Sigmoid, "sigmoid σ");
                ui.selectable_value(&mut c.activation, Activation::Step, "step");
            });
            ui.checkbox(&mut c.labels, "labels");
            ui.separator();
            ui.small("Blue tubes are positive weights, orange negative; thickness is |w|.");
            ui.small("Left-drag to orbit; right-drag to pan; scroll to zoom.");
            ui.horizontal(|ui| {
                if ui.button("Reset view (F)").clicked() {
                    for mut rig in &mut rigs {
                        rig.reset = true;
                    }
                }
                ui.small("H: slide view");
            });
        });
    if c != *controls {
        *controls = c;
    }
    Ok(())
}

/// Labels projected from the scene, kept inside the view. They are part of
/// the diagram, so they stay in slide view.
fn labels(
    mut contexts: EguiContexts,
    controls: Option<Res<PerceptronControls>>,
    scene: Option<Res<PerceptronScene>>,
    cameras: Query<(&Camera, &GlobalTransform), (With<OrbitRig>, With<ExperienceEntity>)>,
) -> Result {
    let (Some(controls), Some(scene)) = (controls, scene) else {
        return Ok(());
    };
    if !controls.labels {
        return Ok(());
    }
    let Some((camera, cam)) = cameras.iter().next() else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?.clone();
    let clip = camera
        .logical_viewport_rect()
        .map(|r| {
            egui::Rect::from_min_max(egui::pos2(r.min.x, r.min.y), egui::pos2(r.max.x, r.max.y))
        })
        .unwrap_or_else(|| ctx.viewport_rect());
    let painter = ctx
        .layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("perceptron-labels"),
        ))
        .with_clip_rect(clip);
    crate::diagram::paint_labels(&painter, clip, camera, cam, &scene.diagram, &|p| p);
    Ok(())
}

/// Typeset formulas as egui textures, by Typst source and scale. An error
/// (or a build without `math`) is remembered, so it isn't retried each frame.
#[derive(Default)]
pub(crate) struct FormulaTextures(HashMap<(String, u32), Result<egui::TextureHandle, String>>);

impl FormulaTextures {
    /// More than enough for one diagram's formulas at a few weight values.
    const CAPACITY: usize = 128;

    fn get(
        &mut self,
        ctx: &egui::Context,
        formula: &Formula,
        pixels_per_point: f32,
    ) -> Result<egui::TextureHandle, String> {
        let key = (formula.typst.clone(), (pixels_per_point * 100.0) as u32);
        if !self.0.contains_key(&key) && self.0.len() >= Self::CAPACITY {
            self.0.clear();
        }
        self.0
            .entry(key)
            .or_insert_with(|| {
                let img = typeset(&formula.typst, 20.0 * pixels_per_point)?;
                let image =
                    egui::ColorImage::from_rgba_premultiplied([img.width, img.height], &img.rgba);
                Ok(ctx.load_texture("perceptron-formula", image, egui::TextureOptions::LINEAR))
            })
            .clone()
    }
}

/// Show the formula of the node or tube under the pointer, typeset beside
/// it (or as plain text without the `math` feature). It works in slide view
/// too, so a presenter can hover live, but not during thumbnail capture.
fn formula_tooltip(
    mut contexts: EguiContexts,
    scene: Option<Res<PerceptronScene>>,
    slide: Option<Res<SlideView>>,
    hidden: Option<Res<HideOverlays>>,
    cameras: Query<(&Camera, &GlobalTransform), (With<OrbitRig>, With<ExperienceEntity>)>,
    mut textures: Local<FormulaTextures>,
) -> Result {
    let Some(scene) = scene else {
        return Ok(());
    };
    let capturing = hidden.is_some() && !slide.is_some_and(|s| s.on);
    let Some((camera, cam)) = cameras.iter().next() else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?.clone();
    let Some(pointer) = ctx.pointer_hover_pos() else {
        return Ok(());
    };
    if capturing || ctx.is_pointer_over_egui() {
        return Ok(());
    }
    let diagram = &scene.diagram;
    let screen = |p: Vec3| camera.world_to_viewport(cam, p).ok();
    let right = cam.right().as_vec3();
    let nodes: Vec<_> = diagram
        .nodes
        .iter()
        .map(|n| {
            let c = screen(n.pos())?;
            let edge = screen(n.pos() + right * n.radius)?;
            Some((c, c.distance(edge)))
        })
        .collect();
    let edges: Vec<_> = diagram
        .edges
        .iter()
        .map(|e| {
            let p = diagram.placement(e)?;
            let tip = p.arrow.map_or(p.end, |(_, tip, _)| tip);
            Some((screen(p.start)?, screen(tip)?))
        })
        .collect();
    let formula = match pick(&nodes, &edges, Vec2::new(pointer.x, pointer.y), 8.0) {
        Some(HoverTarget::Node(i)) => diagram.nodes[i].formula.as_ref(),
        Some(HoverTarget::Edge(i)) => diagram.edges[i].formula.as_ref(),
        None => None,
    };
    let Some(formula) = formula else {
        return Ok(());
    };
    let ppp = ctx.pixels_per_point();
    let typeset = textures.get(&ctx, formula, ppp);
    egui::Area::new(egui::Id::new("perceptron-formula"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pointer + egui::vec2(18.0, 18.0))
        .interactable(false)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::same(10))
                .shadow(egui::Shadow {
                    offset: [0, 2],
                    blur: 8,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(40),
                })
                .show(ui, |ui| match &typeset {
                    Ok(texture) => {
                        let size = texture.size_vec2() / ppp;
                        ui.image(egui::load::SizedTexture::new(texture.id(), size));
                    }
                    Err(_) => {
                        ui.label(
                            egui::RichText::new(&formula.text)
                                .size(18.0)
                                .color(egui::Color32::from_rgb(30, 30, 40)),
                        );
                    }
                });
        });
    Ok(())
}
