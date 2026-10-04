//! The catalog of agent tools: what the MCP server exposes, as data.
//!
//! The docs describe tools from this list, and the
//! `bevaru-mcp` server must implement exactly these names (a test checks
//! both directions), so a tool can't exist without being documented.

use serde::{Deserialize, Serialize};

use super::api;

/// Tool groups, in display order.
pub const GROUPS: [(&str, &str); 7] = [
    (
        "describe",
        "Discover what bevaru offers: losses, models, datasets, experiences, messages, and schemas.",
    ),
    ("losses", "Evaluate loss functions and their gradients."),
    ("datasets", "Build datasets and project them for display."),
    (
        "training",
        "Train linear models step by step and inspect the trajectory.",
    ),
    (
        "sweeps",
        "Sweep a hyperparameter, training to convergence at each value.",
    ),
    ("charts", "Render ruviz charts as PNG images."),
    (
        "app-control",
        "Drive a running bevaru window (needs the app started with --remote).",
    ),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub group: String,
    pub description: String,
    /// Name of the request type; its JSON Schema is in the manifest's
    /// `schemas` (see [`ToolInfo::input_schema`]).
    pub input_type: String,
    /// Only available when connected to a running app.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub requires_app: bool,
}

impl ToolInfo {
    /// The JSON Schema of this tool's input.
    pub fn input_schema(&self) -> serde_json::Value {
        api::schemas()
            .into_iter()
            .find(|(name, _)| *name == self.input_type)
            .map(|(_, schema)| schema)
            .unwrap_or_else(|| panic!("no schema for {}", self.input_type))
    }
}

fn tool<T>(name: &str, group: &str, description: &str) -> ToolInfo {
    debug_assert!(
        GROUPS.iter().any(|(g, _)| *g == group),
        "unknown group {group}"
    );
    let full = std::any::type_name::<T>();
    ToolInfo {
        name: name.into(),
        group: group.into(),
        description: description.into(),
        input_type: full.rsplit("::").next().unwrap_or(full).into(),
        requires_app: group == "app-control",
    }
}

/// Every agent tool, in display order.
pub fn tools() -> Vec<ToolInfo> {
    use api::*;
    vec![
        tool::<DescribeRequest>(
            "describe",
            "describe",
            "Return bevaru's capability manifest, or one section of it (losses, models, datasets, views, loss_shapes, sweep_parameters, experiences, messages, tools, schemas).",
        ),
        tool::<NoArguments>(
            "list_experiences",
            "describe",
            "List the registered experiences: id, title, summary, category, kind, and requirements.",
        ),
        tool::<LossEvalRequest>(
            "evaluate_losses",
            "losses",
            "Evaluate losses and their (sub)gradients at points: residuals r = ŷ − y for regression losses, margins m = y·f(x) for classification losses. At most 10000 points.",
        ),
        tool::<LossShapeRequest>(
            "sample_loss_shape",
            "losses",
            "Sample a 3-D loss-shape view (see the manifest's loss_shapes) on a grid of at most 101 × 101: the axes, every loss value, which values were clipped at the cap, the caption, and the slice that equals the 2-D loss curve.",
        ),
        tool::<DatasetViewRequest>(
            "build_dataset",
            "datasets",
            "Build a dataset and return its displayed coordinates, class labels, axis names, and (for PCA) explained variance. At most 5000 points are returned.",
        ),
        tool::<TrainRequest>(
            "train",
            "training",
            "Train one model on a dataset until it converges or reaches its step budget (at most 100000 steps). Returns the loss trajectory (at most 500 points), final weights and bias, status, and support vectors for SVMs.",
        ),
        tool::<SweepToolRequest>(
            "sweep",
            "sweeps",
            "Train to convergence at each value of one hyperparameter (2 to 50 values) and return each solution's weights, bias, objective, and support-vector count.",
        ),
        tool::<LossChartRequest>(
            "render_loss_chart",
            "charts",
            "Render losses against their argument as a PNG chart.",
        ),
        tool::<LossShapeRenderRequest>(
            "render_loss_shape",
            "charts",
            "Render a 3-D loss-shape view as a PNG surface, coloured cool to warm by height, with labelled axes and an optional camera azimuth and elevation.",
        ),
        tool::<TrainRequest>(
            "render_training_chart",
            "charts",
            "Train one model and render its training objective by step as a PNG chart.",
        ),
        tool::<NoArguments>(
            "app_list_experiences",
            "app-control",
            "List the experiences registered in the running app.",
        ),
        tool::<EnterRequest>(
            "app_enter_experience",
            "app-control",
            "Start an experience in the running app by id, leaving the current one.",
        ),
        tool::<NoArguments>(
            "app_leave_experience",
            "app-control",
            "Return the running app to its lobby, cleaning up the current experience.",
        ),
        tool::<AppPlaybackRequest>(
            "app_playback",
            "app-control",
            "Send a playback command (play, pause, toggle, step, reset, seek) to every pane of the running experiment.",
        ),
        tool::<SweepRequest>(
            "app_start_sweep",
            "app-control",
            "Start a hyperparameter sweep in the running experiment.",
        ),
        tool::<NoArguments>("app_stop_sweep", "app-control", "Stop the running sweep."),
        tool::<NoArguments>(
            "app_state",
            "app-control",
            "Read the running app's state: screen, active experience, panes and their configurations, step, objective, and sweep progress.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_are_unique_and_grouped() {
        let tools = tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), tools.len());
        for (group, _) in GROUPS {
            assert!(
                tools.iter().any(|t| t.group == group),
                "group {group} has no tools"
            );
        }
        assert!(
            tools
                .iter()
                .filter(|t| t.requires_app)
                .all(|t| t.name.starts_with("app_"))
        );
        // Every input type has a published schema, and (as MCP requires)
        // every tool's input is a JSON object.
        for t in &tools {
            assert_eq!(
                t.input_schema()["type"],
                "object",
                "{} input must be an object",
                t.name
            );
        }
    }
}
