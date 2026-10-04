//! "Perceptron in 3D": the diagram of [`crate::diagram`] as a slide-ready
//! 3-D scene, with live weights and a slide view (`H`) that hides every
//! control so a window capture is the slide.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use super::{ExperienceEntity, ExperienceStarted, ExperienceStopped, in_experience};
use crate::capture::HideOverlays;
use crate::diagram::{
    Activation, Diagram, DiagramAssets, NEGATIVE, POSITIVE, Sign, sign, spawn_diagram,
};
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
                (controls, labels).run_if(in_experience(ID)),
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
    let ink = egui::Color32::from_rgb(40, 40, 50);
    let color = |c: Color| {
        let [r, g, b, _] = c.to_srgba().to_u8_array();
        egui::Color32::from_rgb(r, g, b)
    };
    for anchor in scene.diagram.label_anchors() {
        let Ok(pos) = camera.world_to_viewport(cam, anchor.at) else {
            continue;
        };
        let weight = scene
            .diagram
            .edges
            .iter()
            .find(|e| e.id == anchor.id)
            .and_then(|e| e.weight);
        let (size, fill) = match weight.map(sign) {
            Some(Sign::Positive) => (17.0, color(POSITIVE)),
            Some(Sign::Negative) => (17.0, color(NEGATIVE)),
            Some(Sign::Zero) => (17.0, ink),
            None => (22.0, ink),
        };
        let galley = painter.layout_no_wrap(anchor.text, egui::FontId::proportional(size), fill);
        let half = galley.size() / 2.0;
        let inner = clip.shrink(4.0);
        let x = pos.x.clamp(
            inner.min.x + half.x,
            (inner.max.x - half.x).max(inner.min.x + half.x),
        );
        let y = pos.y.clamp(
            inner.min.y + half.y,
            (inner.max.y - half.y).max(inner.min.y + half.y),
        );
        painter.galley(egui::pos2(x, y) - half, galley, fill);
    }
    Ok(())
}
