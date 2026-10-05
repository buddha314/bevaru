//! Components a scene editor can author, which bevaru turns into its visual
//! assets. They are plain reflected data with no editor dependency: any
//! `Reflect` component shows up in editors such as
//! [Jackdaw](https://github.com/jbuehler23/jackdaw), with its doc comment as
//! the tooltip. See `docs/jackdaw.md`.
//!
//! Add [`AuthoringPlugin`], then put a [`PerceptronDiagram`] or a
//! [`LossShapeSurface`] on any entity: bevaru builds the geometry as its
//! children, and rebuilds it when a field changes. [`LobbyEntry`] overrides
//! a lobby card (see [`crate::lobby`]).

use bevaru_core::shapes::{ShapeParams, ShapeView};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::diagram::{Activation, Diagram, DiagramAssets, paint_labels, spawn_diagram};
use crate::shape_view::ShapePlot;

/// A perceptron diagram (inputs, weights, Σ, activation, output), built
/// from Bevy primitives as children of this entity. Blue tubes are positive
/// weights, orange negative; thickness is |w|.
#[derive(Component, Reflect, Debug, Clone, PartialEq)]
#[reflect(Component, Default)]
pub struct PerceptronDiagram {
    /// The weights w₁, w₂, w₃ on the three inputs.
    pub weights: [f64; 3],
    /// The bias b, on a constant +1 input.
    pub bias: f64,
    /// Sigmoid or step.
    pub activation: Activation,
    /// Draw the labels (x₁, w₁ = 0.8, Σ, …). Needs egui in the app.
    pub labels: bool,
}

impl Default for PerceptronDiagram {
    fn default() -> Self {
        Self {
            weights: [0.8, -0.5, 0.3],
            bias: 0.1,
            activation: Activation::Sigmoid,
            labels: true,
        }
    }
}

impl PerceptronDiagram {
    pub fn diagram(&self) -> Diagram {
        Diagram::perceptron(self.weights, self.bias, self.activation)
    }
}

/// A 3-D loss shape: a loss drawn as a surface over two inputs, coloured
/// cool to warm by height, in a 10 × 10 × 6 box (scale the entity to resize
/// it). `view` is a view id such as `probability-vs-truth-cross-entropy` or
/// `two-scores-hinge`; the capability manifest lists them all.
#[derive(Component, Reflect, Debug, Clone, PartialEq)]
#[reflect(Component, Default)]
pub struct LossShapeSurface {
    /// Which view, by id (e.g. `prediction-vs-truth-huber`).
    pub view: String,
    /// Huber δ (> 0), for views that use it.
    pub huber_delta: f64,
    /// Hinge margin (> 0), for views that use it.
    pub margin: f64,
    /// Samples per axis, 3 to 201.
    pub resolution: u32,
    /// Cross-entropy only: subtract the entropy H(p), giving KL divergence.
    pub entropy_removed: bool,
}

impl Default for LossShapeSurface {
    fn default() -> Self {
        Self {
            view: ShapeView::DEFAULT.id().into(),
            huber_delta: 1.0,
            margin: 1.0,
            resolution: 41,
            entropy_removed: false,
        }
    }
}

impl LossShapeSurface {
    /// The view and parameters, or the field that is wrong and why.
    pub fn resolve(&self) -> Result<(ShapeView, ShapeParams), (&'static str, String)> {
        let view = ShapeView::from_id(&self.view)
            .ok_or_else(|| ("view", format!("unknown view {:?}", self.view)))?;
        let mut params = ShapeParams::default();
        params.loss = params
            .loss
            .with_huber_delta(self.huber_delta)
            .map_err(|e| ("huber_delta", e.to_string()))?;
        params.loss = params
            .loss
            .with_margin(self.margin)
            .map_err(|e| ("margin", e.to_string()))?;
        if self.entropy_removed && view != ShapeView::CrossEntropy {
            return Err((
                "entropy_removed",
                format!(
                    "applies only to {}, not {}",
                    ShapeView::CrossEntropy.id(),
                    view.id()
                ),
            ));
        }
        params.entropy_removed = self.entropy_removed;
        Ok((view, params))
    }
}

/// Overrides one card in bevaru's lobby: its place, its wording, or whether
/// it is shown. Experiences without an entry keep their defaults and come
/// after the ordered ones. A hidden experience still opens by id.
#[derive(Component, Reflect, Debug, Clone, Default, PartialEq)]
#[reflect(Component, Default)]
pub struct LobbyEntry {
    /// The experience id, e.g. `iris-svm` (`cargo run -- --list` shows all).
    pub experience: String,
    /// Position in the lobby; lower comes first.
    pub order: i32,
    /// A replacement card title.
    pub title: Option<String>,
    /// A replacement one-sentence summary.
    pub summary: Option<String>,
    /// A replacement category heading.
    pub category: Option<String>,
    /// Leave this card out of the lobby.
    pub hidden: bool,
}

/// Registers the authorable components and builds them.
pub struct AuthoringPlugin;

impl Plugin for AuthoringPlugin {
    fn build(&self, app: &mut App) {
        // Explicit, although `reflect_auto_register` usually suffices: a
        // dependency's types can be stripped before auto-registration runs.
        app.register_type::<PerceptronDiagram>()
            .register_type::<LossShapeSurface>()
            .register_type::<LobbyEntry>()
            .register_type::<Activation>()
            .add_systems(
                Update,
                (
                    build_perceptrons,
                    build_loss_shapes,
                    clear_removed::<PerceptronDiagram>,
                    clear_removed::<LossShapeSurface>,
                ),
            )
            // Runs only in apps with egui; harmless otherwise.
            .add_systems(EguiPrimaryContextPass, perceptron_labels);
    }
}

/// The child entity bevaru built for an authored component.
#[derive(Component, Debug)]
pub struct AuthoredBuild(pub Entity);

/// The diagram an authored perceptron shows, kept for its labels.
#[derive(Component, Debug)]
pub struct AuthoredDiagram(pub Diagram);

/// Shared meshes and materials for authored diagrams, separate from the
/// perceptron experience's own (which it removes when it stops).
#[derive(Resource)]
struct AuthoringAssets(DiagramAssets);

fn replace_build(
    commands: &mut Commands,
    owner: Entity,
    previous: Option<&AuthoredBuild>,
    child: Entity,
) {
    if let Some(AuthoredBuild(old)) = previous
        && let Ok(mut old) = commands.get_entity(*old)
    {
        old.despawn();
    }
    commands
        .entity(owner)
        .add_child(child)
        .insert(AuthoredBuild(child));
}

fn build_perceptrons(
    mut commands: Commands,
    changed: Query<
        (Entity, &PerceptronDiagram, Option<&AuthoredBuild>),
        Changed<PerceptronDiagram>,
    >,
    assets: Option<Res<AuthoringAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if changed.is_empty() {
        return;
    }
    let assets = match assets {
        Some(a) => a.0.clone(),
        None => {
            let a = DiagramAssets::new(&mut meshes, &mut materials);
            commands.insert_resource(AuthoringAssets(a.clone()));
            a
        }
    };
    for (owner, perceptron, previous) in &changed {
        let diagram = perceptron.diagram();
        let root = spawn_diagram(&mut commands, &mut meshes, &assets, &diagram, ());
        replace_build(&mut commands, owner, previous, root);
        commands.entity(owner).insert(AuthoredDiagram(diagram));
    }
}

fn build_loss_shapes(
    mut commands: Commands,
    changed: Query<(Entity, &LossShapeSurface, Option<&AuthoredBuild>), Changed<LossShapeSurface>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (owner, surface, previous) in &changed {
        let built = surface.resolve().and_then(|(view, params)| {
            let n = (surface.resolution as usize).clamp(3, bevaru_core::shapes::MAX_RESOLUTION);
            let grid = view
                .sample(&params, n)
                .map_err(|e| ("resolution", e.to_string()))?;
            ShapePlot::from_grid(view, params, grid)
                .mesh()
                .map_err(|e| ("view", e.to_string()))
        });
        match built {
            Ok(mesh) => {
                let child = commands
                    .spawn((
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::WHITE,
                            perceptual_roughness: 0.85,
                            double_sided: true,
                            cull_mode: None,
                            ..default()
                        })),
                        Transform::default(),
                    ))
                    .id();
                replace_build(&mut commands, owner, previous, child);
            }
            Err((field, why)) => {
                warn!("bevaru: LossShapeSurface on {owner}: field `{field}`: {why}");
                if let Some(AuthoredBuild(old)) = previous
                    && let Ok(mut old) = commands.get_entity(*old)
                {
                    old.despawn();
                }
                if let Ok(mut owner) = commands.get_entity(owner) {
                    owner.remove::<AuthoredBuild>();
                }
            }
        }
    }
}

/// When an authored component is removed, despawn what was built for it.
fn clear_removed<T: Component>(
    mut commands: Commands,
    mut removed: RemovedComponents<T>,
    builds: Query<&AuthoredBuild>,
) {
    for owner in removed.read() {
        let Ok(AuthoredBuild(child)) = builds.get(owner) else {
            continue;
        };
        if let Ok(mut child) = commands.get_entity(*child) {
            child.despawn();
        }
        commands
            .entity(owner)
            .remove::<(AuthoredBuild, AuthoredDiagram)>();
    }
}

/// Labels for authored perceptrons, through the first active 3-D camera.
fn perceptron_labels(
    mut contexts: EguiContexts,
    diagrams: Query<(&PerceptronDiagram, &AuthoredDiagram, &GlobalTransform)>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
) -> Result {
    if diagrams.is_empty() {
        return Ok(());
    }
    let Some((camera, cam)) = cameras.iter().find(|(c, _)| c.is_active) else {
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
            egui::Id::new("authored-labels"),
        ))
        .with_clip_rect(clip);
    for (perceptron, AuthoredDiagram(diagram), at) in &diagrams {
        if perceptron.labels {
            paint_labels(&painter, clip, camera, cam, diagram, &|p| {
                at.transform_point(p)
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::{DiagramRoot, NEGATIVE};
    use bevy::mesh::skinning::SkinnedMeshInverseBindposes;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<SkinnedMeshInverseBindposes>()
            .add_plugins(AuthoringPlugin);
        app
    }

    fn descendants(app: &mut App, root: Entity) -> Vec<Entity> {
        let mut q = app.world_mut().query::<&Children>();
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            if let Ok(children) = q.get(app.world(), e) {
                for c in children.iter() {
                    out.push(c);
                    stack.push(c);
                }
            }
        }
        out
    }

    #[test]
    fn perceptron_builds_on_add_and_rebuilds_on_change() {
        let mut app = app();
        let owner = app.world_mut().spawn(PerceptronDiagram::default()).id();
        app.update();
        let roots = |app: &mut App| {
            app.world_mut()
                .query_filtered::<Entity, With<DiagramRoot>>()
                .iter(app.world())
                .count()
        };
        assert_eq!(roots(&mut app), 1);
        // Nodes and edges: 7 tablets, 6 tubes, 2 arrowheads, 1 backdrop.
        let meshes = |app: &mut App| {
            let all = descendants(app, owner);
            all.iter()
                .filter(|e| app.world().get::<Mesh3d>(**e).is_some())
                .count()
        };
        assert_eq!(meshes(&mut app), 7 + 6 + 2 + 1);

        app.world_mut()
            .get_mut::<PerceptronDiagram>(owner)
            .unwrap()
            .weights[0] = -1.0;
        app.update();
        assert_eq!(roots(&mut app), 1, "the old build is gone");
        let diagram = &app.world().get::<AuthoredDiagram>(owner).unwrap().0;
        let w1 = diagram.edges.iter().find(|e| e.id == "w1").unwrap();
        assert_eq!(w1.weight, Some(-1.0));
        // The w₁ tube now uses the negative material.
        let assets = &app.world().resource::<AuthoringAssets>().0;
        let negative = assets.negative.clone();
        let colors = app.world().resource::<Assets<StandardMaterial>>();
        assert_eq!(colors.get(&negative).unwrap().base_color, NEGATIVE);
        let uses_negative = descendants(&mut app, owner).into_iter().any(|e| {
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(e)
                .is_some_and(|m| m.0 == negative)
        });
        assert!(uses_negative);
    }

    #[test]
    fn removal_and_despawn_clean_up() {
        let mut app = app();
        let owner = app.world_mut().spawn(PerceptronDiagram::default()).id();
        app.update();
        assert!(!descendants(&mut app, owner).is_empty());
        app.world_mut()
            .entity_mut(owner)
            .remove::<PerceptronDiagram>();
        app.update();
        assert!(descendants(&mut app, owner).is_empty());
        assert!(app.world().get::<AuthoredBuild>(owner).is_none());
        // Despawning an authored entity takes its build with it.
        let other = app.world_mut().spawn(LossShapeSurface::default()).id();
        app.update();
        app.world_mut().entity_mut(other).despawn();
        app.update();
        let left = app.world_mut().query::<&Mesh3d>().iter(app.world()).count();
        assert_eq!(left, 0);
    }

    #[test]
    fn loss_surface_builds_standalone_and_rejects_bad_fields() {
        let mut app = app();
        let good = app
            .world_mut()
            .spawn(LossShapeSurface {
                view: "two-scores-hinge".into(),
                ..default()
            })
            .id();
        let bad = app
            .world_mut()
            .spawn(LossShapeSurface {
                view: "no-such-view".into(),
                ..default()
            })
            .id();
        app.update();
        assert_eq!(descendants(&mut app, good).len(), 1);
        assert!(descendants(&mut app, bad).is_empty());
        let err = LossShapeSurface {
            view: "no-such-view".into(),
            ..default()
        }
        .resolve()
        .unwrap_err();
        assert_eq!(err.0, "view");
        let err = LossShapeSurface {
            margin: -1.0,
            ..default()
        }
        .resolve()
        .unwrap_err();
        assert_eq!(err.0, "margin");
        let err = LossShapeSurface {
            view: "two-scores-hinge".into(),
            entropy_removed: true,
            ..default()
        }
        .resolve()
        .unwrap_err();
        assert_eq!(err.0, "entropy_removed");
        // Fixing the field builds it.
        app.world_mut()
            .get_mut::<LossShapeSurface>(bad)
            .unwrap()
            .view = "prediction-vs-truth-mse".into();
        app.update();
        assert_eq!(descendants(&mut app, bad).len(), 1);
    }

    #[test]
    fn types_are_registered_for_editors() {
        let app = app();
        let registry = app.world().resource::<AppTypeRegistry>().read();
        for path in [
            std::any::type_name::<PerceptronDiagram>(),
            std::any::type_name::<LossShapeSurface>(),
            std::any::type_name::<LobbyEntry>(),
        ] {
            let reg = registry
                .get_with_type_path(path)
                .unwrap_or_else(|| panic!("{path} not registered"));
            assert!(reg.data::<ReflectComponent>().is_some(), "{path}");
            assert!(reg.data::<ReflectDefault>().is_some(), "{path}");
        }
    }
}
