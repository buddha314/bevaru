//! Presentation diagrams: a small declarative model (nodes, edges, groups)
//! rendered in 3-D from Bevy's built-in primitives, and projected to 2-D
//! slide coordinates.
//!
//! Bevaru adds only composition here. Every mesh is a Bevy primitive
//! (`Sphere`, `Cylinder`, `Cone`, and `Extrusion<Capsule2d>`); this module
//! picks primitives and places them with transforms, and never generates
//! vertices itself (see `docs/presentation/geometry.md`). Nodes, tubes, and
//! arrowheads share one unit mesh per kind, whatever the diagram's size.

use std::collections::HashSet;
use std::fmt;

use bevy::camera::{CameraProjection, PerspectiveProjection};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::orbit::OrbitView;

// ---------------------------------------------------------------------------
// The model

/// What a node stands for; it sets the node's colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeRole {
    Input,
    Bias,
    Sum,
    Activation,
    Output,
}

impl NodeRole {
    pub const ALL: [NodeRole; 5] = [
        NodeRole::Input,
        NodeRole::Bias,
        NodeRole::Sum,
        NodeRole::Activation,
        NodeRole::Output,
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub role: NodeRole,
    /// World position (Z is up).
    pub position: [f32; 3],
    pub radius: f32,
}

impl Node {
    pub fn pos(&self) -> Vec3 {
        Vec3::from_array(self.position)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Draw an arrowhead at `to`.
    #[serde(default)]
    pub arrow: bool,
}

/// Nodes drawn on one backdrop, such as a layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub label: String,
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Diagram {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagramError {
    DuplicateId(String),
    MissingNode { edge: String, node: String },
    MissingMember { group: String, node: String },
}

impl fmt::Display for DiagramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagramError::DuplicateId(id) => write!(f, "id {id:?} is used more than once"),
            DiagramError::MissingNode { edge, node } => {
                write!(f, "edge {edge:?} refers to missing node {node:?}")
            }
            DiagramError::MissingMember { group, node } => {
                write!(f, "group {group:?} refers to missing node {node:?}")
            }
        }
    }
}

impl std::error::Error for DiagramError {}

/// The perceptron's activation function.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Activation {
    Step,
    #[default]
    Sigmoid,
}

impl Activation {
    pub fn symbol(self) -> &'static str {
        match self {
            Activation::Step => "step",
            Activation::Sigmoid => "σ",
        }
    }
}

/// Format a weight for a label: two decimals, without a trailing zero.
fn weight_text(w: f64) -> String {
    let s = format!("{w:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    match s {
        "-0" => "0".into(),
        s => s.replace('-', "−"),
    }
}

impl Diagram {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Check that ids are unique and every reference resolves.
    pub fn validate(&self) -> Result<(), DiagramError> {
        let mut seen = HashSet::new();
        let ids = self
            .nodes
            .iter()
            .map(|n| &n.id)
            .chain(self.edges.iter().map(|e| &e.id))
            .chain(self.groups.iter().map(|g| &g.id));
        for id in ids {
            if !seen.insert(id.as_str()) {
                return Err(DiagramError::DuplicateId(id.clone()));
            }
        }
        for e in &self.edges {
            for end in [&e.from, &e.to] {
                if self.node(end).is_none() {
                    return Err(DiagramError::MissingNode {
                        edge: e.id.clone(),
                        node: end.clone(),
                    });
                }
            }
        }
        for g in &self.groups {
            if let Some(m) = g.members.iter().find(|m| self.node(m).is_none()) {
                return Err(DiagramError::MissingMember {
                    group: g.id.clone(),
                    node: m.clone(),
                });
            }
        }
        Ok(())
    }

    /// A perceptron: inputs x₁–x₃ and a bias b feed a weighted sum Σ, then an
    /// activation, then the output y. Laid out left to right in the X–Z
    /// plane, so a camera on −Y sees it front-on.
    pub fn perceptron(weights: [f64; 3], bias: f64, activation: Activation) -> Self {
        let node = |id: &str, label: &str, role, x: f32, z: f32, radius: f32| Node {
            id: id.into(),
            label: label.into(),
            role,
            position: [x, 0.0, z],
            radius,
        };
        let mut nodes = vec![
            node("x1", "x₁", NodeRole::Input, -4.0, 2.4, 0.45),
            node("x2", "x₂", NodeRole::Input, -4.0, 0.8, 0.45),
            node("x3", "x₃", NodeRole::Input, -4.0, -0.8, 0.45),
            node("b", "+1", NodeRole::Bias, -4.0, -2.4, 0.35),
        ];
        nodes.extend([
            node("sum", "Σ", NodeRole::Sum, 0.0, 0.0, 0.7),
            node(
                "act",
                activation.symbol(),
                NodeRole::Activation,
                2.8,
                0.0,
                0.55,
            ),
            node("y", "y", NodeRole::Output, 5.4, 0.0, 0.5),
        ]);
        let weighted = |id: &str, from: &str, name: &str, w: f64| Edge {
            id: id.into(),
            from: from.into(),
            to: "sum".into(),
            weight: Some(w),
            label: Some(format!("{name} = {}", weight_text(w))),
            arrow: false,
        };
        let arrow = |id: &str, from: &str, to: &str| Edge {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            weight: None,
            label: None,
            arrow: true,
        };
        Self {
            nodes,
            edges: vec![
                weighted("w1", "x1", "w₁", weights[0]),
                weighted("w2", "x2", "w₂", weights[1]),
                weighted("w3", "x3", "w₃", weights[2]),
                weighted("wb", "b", "b", bias),
                arrow("sum-act", "sum", "act"),
                arrow("act-y", "act", "y"),
            ],
            groups: vec![Group {
                id: "inputs".into(),
                label: "inputs".into(),
                members: vec!["x1".into(), "x2".into(), "x3".into(), "b".into()],
            }],
        }
    }

    /// Where each label sits in the world: node labels above their node,
    /// edge labels above the edge's midpoint, group labels above the group.
    pub fn label_anchors(&self) -> Vec<LabelAnchor> {
        let mut out = Vec::new();
        for n in &self.nodes {
            let at = match n.role {
                // Inputs read left of their node, like a slide's x₁ ○ ───.
                NodeRole::Input | NodeRole::Bias => n.pos() - Vec3::X * (n.radius + 0.55),
                _ => n.pos() + Vec3::Z * (n.radius + 0.4),
            };
            out.push(LabelAnchor {
                id: n.id.clone(),
                text: n.label.clone(),
                at,
            });
        }
        for e in &self.edges {
            let (Some(text), Some(p)) = (&e.label, self.placement(e)) else {
                continue;
            };
            // Near the tail, where fanned-in edges are still apart.
            let at = p.start.lerp(p.end, 0.3) + Vec3::Z * (p.radius + 0.3);
            out.push(LabelAnchor {
                id: e.id.clone(),
                text: text.clone(),
                at,
            });
        }
        for g in &self.groups {
            if let Some(b) = self.group_bounds(g) {
                out.push(LabelAnchor {
                    id: g.id.clone(),
                    text: g.label.clone(),
                    at: Vec3::new(b.center.x, b.center.y, b.max_z + 0.45),
                });
            }
        }
        out
    }
}

/// A label's text and where it is anchored in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelAnchor {
    pub id: String,
    pub text: String,
    pub at: Vec3,
}

// ---------------------------------------------------------------------------
// Placement: which primitive goes where (no mesh generation)

/// Tube radius range for weights, in world units.
pub const MIN_TUBE: f32 = 0.035;
pub const MAX_TUBE: f32 = 0.16;
/// |w| at which a tube reaches its maximum radius.
pub const FULL_WEIGHT: f64 = 2.0;
/// Radius of edges without a weight (the arrows Σ → σ → y).
pub const PLAIN_TUBE: f32 = 0.06;

/// The radius that encodes |w|: thin at 0, thickest from `FULL_WEIGHT` on.
pub fn weight_radius(w: f64) -> f32 {
    let t = (w.abs() / FULL_WEIGHT).min(1.0) as f32;
    MIN_TUBE + (MAX_TUBE - MIN_TUBE) * t
}

/// A weight's sign, which picks its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    Positive,
    Negative,
    Zero,
}

pub fn sign(w: f64) -> Sign {
    if w > 1e-9 {
        Sign::Positive
    } else if w < -1e-9 {
        Sign::Negative
    } else {
        Sign::Zero
    }
}

/// Where an edge's tube runs, and its arrowhead if it has one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgePlacement {
    /// The tube runs from `start` (on the tail node's surface) to `end`.
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f32,
    /// Cone base centre, tip (on the head node's surface), and base radius.
    pub arrow: Option<(Vec3, Vec3, f32)>,
}

impl Diagram {
    pub fn placement(&self, e: &Edge) -> Option<EdgePlacement> {
        let (a, b) = (self.node(&e.from)?, self.node(&e.to)?);
        let dir = (b.pos() - a.pos()).try_normalize()?;
        let radius = e.weight.map_or(PLAIN_TUBE, weight_radius);
        let start = a.pos() + dir * a.radius;
        let tip = b.pos() - dir * b.radius;
        let arrow = e.arrow.then(|| {
            let (len, width) = (0.45, (radius * 3.0).max(0.16));
            (tip - dir * len, tip, width)
        });
        let end = arrow.map_or(tip, |(base, _, _)| base);
        Some(EdgePlacement {
            start,
            end,
            radius,
            arrow,
        })
    }

    fn group_bounds(&self, g: &Group) -> Option<GroupBounds> {
        let members: Vec<&Node> = g.members.iter().filter_map(|m| self.node(m)).collect();
        if members.is_empty() {
            return None;
        }
        let pad = 0.35;
        let lo = members
            .iter()
            .map(|n| n.pos() - Vec3::splat(n.radius + pad))
            .reduce(Vec3::min)?;
        let hi = members
            .iter()
            .map(|n| n.pos() + Vec3::splat(n.radius + pad))
            .reduce(Vec3::max)?;
        Some(GroupBounds {
            center: (lo + hi) / 2.0,
            half: (hi - lo) / 2.0,
            max_z: hi.z,
        })
    }
}

struct GroupBounds {
    center: Vec3,
    half: Vec3,
    max_z: f32,
}

/// A transform that turns the unit cylinder (radius 1, height 1, along Y)
/// into a tube from `a` to `b` of `radius`.
pub fn tube_transform(a: Vec3, b: Vec3, radius: f32) -> Transform {
    let d = b - a;
    Transform {
        translation: (a + b) / 2.0,
        rotation: Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y)),
        scale: Vec3::new(radius, d.length(), radius),
    }
}

/// A transform that turns the unit cone (radius 1, height 1, tip at +Y) into
/// an arrowhead from `base` to `tip`.
pub fn cone_transform(base: Vec3, tip: Vec3, radius: f32) -> Transform {
    tube_transform(base, tip, radius)
}

// ---------------------------------------------------------------------------
// Rendering

/// The shared unit meshes and the palette, created once per experience.
#[derive(Resource, Debug, Clone)]
pub struct DiagramAssets {
    pub sphere: Handle<Mesh>,
    pub cylinder: Handle<Mesh>,
    pub cone: Handle<Mesh>,
    pub roles: Vec<(NodeRole, Handle<StandardMaterial>)>,
    pub positive: Handle<StandardMaterial>,
    pub negative: Handle<StandardMaterial>,
    pub neutral: Handle<StandardMaterial>,
    pub group: Handle<StandardMaterial>,
}

/// Okabe–Ito blue and vermillion: readable with common colour-vision
/// deficiencies, and the same pair the rest of bevaru uses.
pub const POSITIVE: Color = Color::srgb(0.0, 0.447, 0.698);
pub const NEGATIVE: Color = Color::srgb(0.835, 0.369, 0.0);
pub const NEUTRAL: Color = Color::srgb(0.6, 0.6, 0.63);

pub fn role_color(role: NodeRole) -> Color {
    match role {
        NodeRole::Input => Color::srgb(0.84, 0.87, 0.92),
        NodeRole::Bias => Color::srgb(0.78, 0.78, 0.8),
        NodeRole::Sum => Color::srgb(0.27, 0.3, 0.38),
        NodeRole::Activation => Color::srgb(0.9, 0.62, 0.0),
        NodeRole::Output => Color::srgb(0.0, 0.62, 0.45),
    }
}

impl DiagramAssets {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mut mat = |c: Color| {
            materials.add(StandardMaterial {
                base_color: c,
                perceptual_roughness: 0.6,
                ..default()
            })
        };
        let roles = NodeRole::ALL.map(|r| (r, mat(role_color(r)))).to_vec();
        let positive = mat(POSITIVE);
        let negative = mat(NEGATIVE);
        let neutral = mat(NEUTRAL);
        let group = materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.62, 0.75, 0.16),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        Self {
            sphere: meshes.add(Sphere::new(1.0).mesh().uv(48, 24)),
            cylinder: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(24)),
            cone: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(24)),
            roles,
            positive,
            negative,
            neutral,
            group,
        }
    }

    fn role(&self, role: NodeRole) -> Handle<StandardMaterial> {
        self.roles
            .iter()
            .find(|(r, _)| *r == role)
            .map(|(_, h)| h.clone())
            .unwrap_or_else(|| self.neutral.clone())
    }

    fn edge(&self, weight: Option<f64>) -> Handle<StandardMaterial> {
        match weight.map(sign) {
            Some(Sign::Positive) => self.positive.clone(),
            Some(Sign::Negative) => self.negative.clone(),
            Some(Sign::Zero) => self.neutral.clone(),
            None => self.role(NodeRole::Sum),
        }
    }
}

/// The root entity of a spawned diagram; despawning it removes everything.
#[derive(Component, Debug, Default)]
pub struct DiagramRoot;

/// Spawn `diagram` under a new root with `marker`, from the shared meshes.
/// Each group's backdrop is an extruded `Capsule2d` sized to its members,
/// so it is the one mesh made per diagram.
pub fn spawn_diagram(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &DiagramAssets,
    diagram: &Diagram,
    marker: impl Bundle,
) -> Entity {
    let mut root = commands.spawn((
        DiagramRoot,
        Transform::default(),
        Visibility::default(),
        marker,
    ));
    root.with_children(|parent| {
        for n in &diagram.nodes {
            parent.spawn((
                Mesh3d(assets.sphere.clone()),
                MeshMaterial3d(assets.role(n.role)),
                Transform::from_translation(n.pos()).with_scale(Vec3::splat(n.radius)),
            ));
        }
        for e in &diagram.edges {
            let Some(p) = diagram.placement(e) else {
                continue;
            };
            let material = assets.edge(e.weight);
            parent.spawn((
                Mesh3d(assets.cylinder.clone()),
                MeshMaterial3d(material.clone()),
                tube_transform(p.start, p.end, p.radius),
            ));
            if let Some((base, tip, width)) = p.arrow {
                parent.spawn((
                    Mesh3d(assets.cone.clone()),
                    MeshMaterial3d(material),
                    cone_transform(base, tip, width),
                ));
            }
        }
        for g in &diagram.groups {
            let Some(b) = diagram.group_bounds(g) else {
                continue;
            };
            // A pill in the X–Z plane behind the nodes: Capsule2d's long
            // axis is local Y, so rotate local Y to world Z (and its
            // extrusion depth, local Z, to world −Y).
            let (along_z, across_x) = (b.half.z, b.half.x);
            let radius = across_x.min(along_z);
            let pill = Extrusion::new(
                Capsule2d::new(radius, (along_z - radius).max(0.0) * 2.0),
                0.04,
            );
            parent.spawn((
                Mesh3d(meshes.add(pill)),
                MeshMaterial3d(assets.group.clone()),
                Transform::from_translation(b.center + Vec3::Y * (b.half.y + 0.05))
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
        }
    });
    root.id()
}

// ---------------------------------------------------------------------------
// 2-D slide projection

/// A diagram as seen through a camera, in slide coordinates: [0, 1]² with
/// the origin at the top-left, as slide shapes are positioned.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SlideProjection {
    pub nodes: Vec<ProjectedNode>,
    pub edges: Vec<ProjectedEdge>,
    pub labels: Vec<ProjectedLabel>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectedNode {
    pub id: String,
    pub role: NodeRole,
    pub center: [f32; 2],
    /// Apparent radius, as a fraction of the slide's height.
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectedEdge {
    pub id: String,
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub arrow: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectedLabel {
    pub id: String,
    pub text: String,
    pub at: [f32; 2],
}

/// Project world points through `view` with Bevy's own perspective (the
/// default field of view) at `aspect` (width / height).
pub struct SlideCamera {
    clip_from_world: Mat4,
    view_right: Vec3,
}

impl SlideCamera {
    pub fn new(view: &OrbitView, aspect: f32) -> Self {
        let projection = PerspectiveProjection {
            aspect_ratio: aspect,
            ..default()
        };
        let world_from_view = view.transform().to_matrix();
        Self {
            clip_from_world: projection.get_clip_from_view() * world_from_view.inverse(),
            view_right: view.transform().right().into(),
        }
    }

    /// Slide coordinates of a world point, or `None` behind the camera.
    pub fn point(&self, p: Vec3) -> Option<[f32; 2]> {
        let clip = self.clip_from_world * p.extend(1.0);
        (clip.w > 1e-6).then(|| {
            let ndc = clip.truncate() / clip.w;
            [(ndc.x + 1.0) / 2.0, (1.0 - ndc.y) / 2.0]
        })
    }
}

impl Diagram {
    pub fn project(&self, view: &OrbitView, aspect: f32) -> SlideProjection {
        let cam = SlideCamera::new(view, aspect);
        let nodes = self
            .nodes
            .iter()
            .filter_map(|n| {
                let c = cam.point(n.pos())?;
                let edge = cam.point(n.pos() + cam.view_right * n.radius)?;
                // Horizontal extent in slide-width units → height units.
                let radius = (edge[0] - c[0]).abs() * aspect;
                Some(ProjectedNode {
                    id: n.id.clone(),
                    role: n.role,
                    center: c,
                    radius,
                })
            })
            .collect();
        let edges = self
            .edges
            .iter()
            .filter_map(|e| {
                let p = self.placement(e)?;
                let tip = p.arrow.map_or(p.end, |(_, tip, _)| tip);
                Some(ProjectedEdge {
                    id: e.id.clone(),
                    from: cam.point(p.start)?,
                    to: cam.point(tip)?,
                    arrow: e.arrow,
                })
            })
            .collect();
        let labels = self
            .label_anchors()
            .into_iter()
            .filter_map(|l| {
                Some(ProjectedLabel {
                    at: cam.point(l.at)?,
                    id: l.id,
                    text: l.text,
                })
            })
            .collect();
        SlideProjection {
            nodes,
            edges,
            labels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perceptron() -> Diagram {
        Diagram::perceptron([0.8, -0.5, 0.3], 0.1, Activation::Sigmoid)
    }

    #[test]
    fn perceptron_structure() {
        let d = perceptron();
        d.validate().unwrap();
        assert_eq!((d.nodes.len(), d.edges.len()), (7, 6));
        let into_sum: Vec<f64> = d
            .edges
            .iter()
            .filter(|e| e.to == "sum")
            .filter_map(|e| e.weight)
            .collect();
        assert_eq!(into_sum, [0.8, -0.5, 0.3, 0.1]);
        for id in ["sum-act", "act-y"] {
            assert!(d.edges.iter().find(|e| e.id == id).unwrap().arrow);
        }
        assert_eq!(d.node("act").unwrap().label, "σ");
        let step = Diagram::perceptron([0.8, -0.5, 0.3], 0.1, Activation::Step);
        assert_eq!(step.node("act").unwrap().label, "step");
        let labels: Vec<String> = d.edges.iter().filter_map(|e| e.label.clone()).collect();
        assert_eq!(labels, ["w₁ = 0.8", "w₂ = −0.5", "w₃ = 0.3", "b = 0.1"]);
    }

    #[test]
    fn invalid_diagrams_name_the_problem() {
        let mut d = perceptron();
        d.edges[0].to = "nowhere".into();
        assert_eq!(
            d.validate(),
            Err(DiagramError::MissingNode {
                edge: "w1".into(),
                node: "nowhere".into()
            })
        );
        let mut d = perceptron();
        d.edges[1].id = "x1".into();
        assert_eq!(d.validate(), Err(DiagramError::DuplicateId("x1".into())));
        let mut d = perceptron();
        d.groups[0].members.push("ghost".into());
        assert!(d.validate().unwrap_err().to_string().contains("ghost"));
    }

    #[test]
    fn json_round_trip() {
        let d = perceptron();
        let back: Diagram = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn tubes_end_on_node_surfaces_and_encode_weights() {
        let d = perceptron();
        for e in &d.edges {
            let p = d.placement(e).unwrap();
            let (a, b) = (d.node(&e.from).unwrap(), d.node(&e.to).unwrap());
            assert!(
                (p.start.distance(a.pos()) - a.radius).abs() < 1e-5,
                "{}",
                e.id
            );
            let tip = p.arrow.map_or(p.end, |(_, tip, _)| tip);
            assert!((tip.distance(b.pos()) - b.radius).abs() < 1e-5, "{}", e.id);
        }
        assert!(weight_radius(0.8) > weight_radius(-0.5));
        assert_eq!(weight_radius(0.0), MIN_TUBE);
        assert_eq!(weight_radius(10.0), MAX_TUBE);
        assert_eq!(sign(0.8), Sign::Positive);
        assert_eq!(sign(-0.5), Sign::Negative);
        assert_eq!(sign(0.0), Sign::Zero);
    }

    #[test]
    fn tube_transform_maps_the_unit_cylinder_onto_the_segment() {
        let (a, b) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, -1.0, 0.5));
        let t = tube_transform(a, b, 0.2);
        // The unit cylinder's axis ends are (0, ±½, 0).
        assert!(t.transform_point(Vec3::Y * 0.5).distance(b) < 1e-5);
        assert!(t.transform_point(-Vec3::Y * 0.5).distance(a) < 1e-5);
    }

    /// A diagram twice the perceptron's size, with the same one group.
    fn doubled() -> Diagram {
        let mut d = perceptron();
        let extra = perceptron();
        for mut n in extra.nodes {
            n.id = format!("{}-2", n.id);
            n.position[1] += 3.0;
            d.nodes.push(n);
        }
        for mut e in extra.edges {
            e.id = format!("{}-2", e.id);
            e.from = format!("{}-2", e.from);
            e.to = format!("{}-2", e.to);
            d.edges.push(e);
        }
        d
    }

    #[test]
    fn mesh_count_does_not_grow_with_the_diagram() {
        let count = |d: &Diagram| {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .add_plugins(AssetPlugin::default())
                .init_asset::<Mesh>()
                .init_asset::<StandardMaterial>();
            let world = app.world_mut();
            let assets = world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
                let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                DiagramAssets::new(&mut meshes, &mut materials)
            });
            world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
                let mut commands = world.commands();
                spawn_diagram(&mut commands, &mut meshes, &assets, d, ());
            });
            world.flush();
            world.resource::<Assets<Mesh>>().len()
        };
        let big = doubled();
        big.validate().unwrap();
        assert_eq!(count(&perceptron()), count(&big));
        // Three shared unit meshes plus one backdrop.
        assert_eq!(count(&perceptron()), 4);
    }

    fn front() -> OrbitView {
        OrbitView {
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
            distance: 22.0,
            target: Vec3::new(0.7, 0.0, 0.0),
        }
    }

    #[test]
    fn front_on_projection_keeps_the_layout() {
        let p = perceptron().project(&front(), 16.0 / 9.0);
        let at = |id: &str| p.nodes.iter().find(|n| n.id == id).unwrap().center;
        let inside = |c: [f32; 2]| (0.0..=1.0).contains(&c[0]) && (0.0..=1.0).contains(&c[1]);
        assert!(p.nodes.iter().all(|n| inside(n.center) && n.radius > 0.0));
        assert!(p.labels.iter().all(|l| inside(l.at)));
        // Left to right: inputs, Σ, activation, output.
        assert!(at("x1")[0] < at("sum")[0] && at("sum")[0] < at("act")[0]);
        assert!(at("act")[0] < at("y")[0]);
        // Top to bottom: x₁ above x₂ above x₃ above the bias.
        assert!(at("x1")[1] < at("x2")[1] && at("x2")[1] < at("x3")[1]);
        assert!(at("x3")[1] < at("b")[1]);
        assert_eq!(p.edges.len(), 6);
        assert!(p.edges.iter().filter(|e| e.arrow).count() == 2);
    }

    #[test]
    fn projection_matches_the_view_target() {
        // The orbit target is the centre of the slide.
        let view = OrbitView {
            yaw: -1.2,
            pitch: 0.3,
            distance: 18.0,
            target: Vec3::new(0.0, 0.0, 0.0),
        };
        let cam = SlideCamera::new(&view, 16.0 / 9.0);
        let c = cam.point(Vec3::ZERO).unwrap();
        assert!((c[0] - 0.5).abs() < 1e-5 && (c[1] - 0.5).abs() < 1e-5);
        // Up in the world is up on the slide (smaller y).
        assert!(cam.point(Vec3::Z).unwrap()[1] < c[1]);
    }
}
