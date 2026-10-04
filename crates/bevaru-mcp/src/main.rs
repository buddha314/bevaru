//! `bevaru-mcp`: makes bevaru easy for AI agents to use.
//!
//! - `bevaru-mcp` (no arguments) serves the Model Context Protocol over
//!   stdio: headless tools for bevaru's losses, datasets, training, sweeps,
//!   and charts, plus its agent docs as resources. No window or GPU needed.
//! - `bevaru-mcp --app <url>` adds `app_*` tools that drive a running bevaru
//!   window started with `--remote` (Bevy Remote Protocol, JSON-RPC over HTTP).
//! - `bevaru-mcp gen-docs` regenerates the reference files in `docs/agents/`
//!   from the code (`tests/agent_docs.rs` checks they're current).
//!
//! Tool names, descriptions, and input schemas come from
//! [`bevaru::agent::tools`]; this binary only dispatches them to
//! [`bevaru::agent::run`], so the docs and the server can't disagree.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use base64::Engine as _;
use bevaru::agent::api::{self, ApiError};
use bevaru::agent::{Manifest, builtin_manifest, docs::generated_files, run, tools};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    JsonObject, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
    ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
    ResourceContents, ServerCapabilities, ServerConfig, Tool, ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, transport::stdio};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Agent docs served as resources, embedded so they match this binary.
const RESOURCES: [(&str, &str, &str, &str); 7] = [
    (
        "bevaru://capabilities.json",
        "Capability manifest (JSON)",
        "application/json",
        include_str!("../../../docs/agents/capabilities.json"),
    ),
    (
        "bevaru://capabilities.md",
        "Capability reference",
        "text/markdown",
        include_str!("../../../docs/agents/capabilities.md"),
    ),
    (
        "bevaru://docs/guide.md",
        "Agent guide: building with bevaru",
        "text/markdown",
        include_str!("../../../docs/agents/guide.md"),
    ),
    (
        "bevaru://docs/mcp.md",
        "Using the bevaru MCP server",
        "text/markdown",
        include_str!("../../../docs/agents/mcp.md"),
    ),
    (
        "bevaru://docs/recipes/train-headlessly.md",
        "Recipe: train a model headlessly",
        "text/markdown",
        include_str!("../../../docs/agents/recipes/train-headlessly.md"),
    ),
    (
        "bevaru://docs/recipes/add-an-experience.md",
        "Recipe: add an experience to the lobby",
        "text/markdown",
        include_str!("../../../docs/agents/recipes/add-an-experience.md"),
    ),
    (
        "bevaru://docs/recipes/drive-a-running-app.md",
        "Recipe: drive a running app",
        "text/markdown",
        include_str!("../../../docs/agents/recipes/drive-a-running-app.md"),
    ),
];

const INSTRUCTIONS: &str = "bevaru visualizes machine-learning loss functions and linear models. \
These tools run headlessly: evaluate losses, build datasets, train linear regression / SVM / \
logistic regression and inspect the trajectory, sweep a hyperparameter, and render charts as PNG. \
Call `describe` (or read bevaru://capabilities.md) first for every id, range, and schema. \
Invalid input returns a tool error naming the field and its valid range.";

/// Every tool this server can run (the app-control tools need a running app
/// and arrive with the remote hooks).
const SERVED: [&str; 8] = [
    "describe",
    "list_experiences",
    "evaluate_losses",
    "build_dataset",
    "train",
    "sweep",
    "render_loss_chart",
    "render_training_chart",
];

/// App-control tools and the remote method each one calls.
const APP_TOOLS: [(&str, &str); 7] = [
    ("app_list_experiences", "bevaru.experiences.list"),
    ("app_enter_experience", "bevaru.experiences.enter"),
    ("app_leave_experience", "bevaru.experiences.leave"),
    ("app_playback", "bevaru.playback"),
    ("app_start_sweep", "bevaru.sweep.start"),
    ("app_stop_sweep", "bevaru.sweep.stop"),
    ("app_state", "bevaru.state"),
];

/// Where a running app usually listens (`bevaru --remote`).
const DEFAULT_APP_URL: &str = "http://127.0.0.1:15702";

/// What a tool produced.
enum Output {
    Json(Value),
    Png(Vec<u8>),
}

fn parse<T: DeserializeOwned>(args: JsonObject) -> Result<T, ApiError> {
    serde_json::from_value(Value::Object(args))
        .map_err(|e| ApiError::new("arguments", e.to_string()))
}

fn json<T: serde::Serialize>(v: T) -> Result<Output, ApiError> {
    Ok(Output::Json(
        serde_json::to_value(v).expect("results serialize"),
    ))
}

/// Run one tool. Blocking: call it off the async runtime.
fn dispatch(manifest: &Manifest, name: &str, args: JsonObject) -> Result<Output, ApiError> {
    match name {
        "describe" => {
            let req: api::DescribeRequest = parse(args)?;
            let value = run::describe(manifest, &req)?;
            match req.section {
                // MCP structured content must be an object.
                Some(section) => json(serde_json::json!({ section: value })),
                None => json(value),
            }
        }
        "list_experiences" => {
            let _: api::NoArguments = parse(args)?;
            json(serde_json::json!({ "experiences": &manifest.experiences }))
        }
        "evaluate_losses" => {
            json(serde_json::json!({ "evaluations": api::evaluate_losses(&parse(args)?)? }))
        }
        "build_dataset" => json(run::build_dataset(&parse(args)?)?),
        "train" => json(run::train(&parse(args)?)?),
        "sweep" => json(run::sweep(&parse(args)?)?),
        "render_loss_chart" => Ok(Output::Png(run::render_loss_chart(&parse(args)?)?)),
        "render_training_chart" => Ok(Output::Png(run::render_training_chart(&parse(args)?)?)),
        other => Err(ApiError::new("name", format!("unknown tool {other:?}"))),
    }
}

/// Call a `bevaru.*` method on a running app. Blocking.
fn call_app(url: &str, tool: &str, args: JsonObject) -> Result<Output, ApiError> {
    let method = APP_TOOLS
        .iter()
        .find(|(name, _)| *name == tool)
        .map(|(_, method)| *method)
        .ok_or_else(|| ApiError::new("name", format!("unknown tool {tool:?}")))?;
    // Validate here so mistakes come back as tool errors with field names.
    let params = match tool {
        "app_enter_experience" => {
            Some(serde_json::to_value(parse::<api::EnterRequest>(args)?).expect("serializes"))
        }
        "app_playback" => Some(
            serde_json::to_value(parse::<api::AppPlaybackRequest>(args)?.command)
                .expect("serializes"),
        ),
        "app_start_sweep" => {
            Some(serde_json::to_value(parse::<api::SweepRequest>(args)?).expect("serializes"))
        }
        _ => {
            let _: api::NoArguments = parse(args)?;
            None
        }
    };
    let request =
        serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let unreachable = |e: &dyn std::fmt::Display| {
        ApiError::new(
            "app",
            format!(
                "no bevaru app is answering at {url} ({e}). Start one with `cargo run --features remote -- --remote`, or pass its address with --app"
            ),
        )
    };
    let mut response = ureq::post(url)
        .send_json(&request)
        .map_err(|e| unreachable(&e))?;
    let reply: Value = response
        .body_mut()
        .read_json()
        .map_err(|e| unreachable(&e))?;
    if let Some(error) = reply.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(ApiError::new("app", format!("{method}: {message}")));
    }
    let result = reply.get("result").cloned().unwrap_or(Value::Null);
    Ok(Output::Json(result))
}

#[derive(Clone)]
struct BevaruServer {
    manifest: Arc<Manifest>,
    tools: Arc<Vec<Tool>>,
    /// A running app to drive; enables the `app_*` tools.
    app: Option<Arc<str>>,
}

impl BevaruServer {
    fn new(app: Option<String>) -> Self {
        let manifest = builtin_manifest();
        let tools = tools::tools()
            .into_iter()
            .filter(|t| {
                SERVED.contains(&t.name.as_str())
                    || (app.is_some() && APP_TOOLS.iter().any(|(name, _)| *name == t.name))
            })
            .map(|t| {
                let Value::Object(schema) = t.input_schema() else {
                    panic!("schema for {} is not an object", t.name);
                };
                // Headless tools only compute; app tools change the window.
                let annotations = if t.requires_app {
                    ToolAnnotations::new()
                        .read_only(t.name == "app_state" || t.name == "app_list_experiences")
                        .destructive(false)
                        .idempotent(false)
                        .open_world(false)
                } else {
                    ToolAnnotations::new()
                        .read_only(true)
                        .destructive(false)
                        .idempotent(true)
                        .open_world(false)
                };
                Tool::new(t.name.clone(), t.description.clone(), Arc::new(schema))
                    .with_annotations(annotations)
            })
            .collect();
        Self {
            manifest: Arc::new(manifest),
            tools: Arc::new(tools),
            app: app.map(Into::into),
        }
    }
}

impl ServerHandler for BevaruServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new("bevaru-mcp", env!("CARGO_PKG_VERSION")))
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items((*self.tools).clone()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let name = request.name.to_string();
        if !self.tools.iter().any(|t| t.name == name) {
            return Err(ErrorData::invalid_params(
                format!("unknown tool {name:?}"),
                None,
            ));
        }
        let args = request.arguments.unwrap_or_default();
        let manifest = self.manifest.clone();
        let app = self.app.clone();
        let outcome = tokio::task::spawn_blocking(move || match &app {
            Some(url) if name.starts_with("app_") => call_app(url, &name, args),
            _ => dispatch(&manifest, &name, args),
        })
        .await
        .map_err(|e| ErrorData::internal_error(format!("tool panicked: {e}"), None))?;
        let result = match outcome {
            // MCP requires structured content to be an object.
            Ok(Output::Json(value @ Value::Object(_))) => CallToolResult::structured(value),
            Ok(Output::Json(value)) => {
                CallToolResult::structured(serde_json::json!({ "result": value }))
            }
            Ok(Output::Png(bytes)) => CallToolResult::success(vec![ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(bytes),
                "image/png",
            )]),
            // Errors the caller can fix are tool results, not protocol errors.
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_string())]),
        };
        Ok(result.into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(
            RESOURCES
                .iter()
                .map(|(uri, name, mime, _)| Resource::new(*uri, *name).with_mime_type(*mime))
                .collect(),
        ))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let Some((uri, _, mime, text)) = RESOURCES.iter().find(|r| r.0 == request.uri) else {
            return Err(ErrorData::resource_not_found(
                format!("no resource {}", request.uri),
                None,
            ));
        };
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(*text, *uri).with_mime_type(*mime),
        ])
        .into())
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gen_docs(root: &Path) -> std::io::Result<()> {
    for (path, contents) in generated_files(root) {
        let target = root.join(path);
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let unchanged = std::fs::read_to_string(&target).is_ok_and(|old| old == contents);
        if !unchanged {
            std::fs::write(&target, contents)?;
        }
        println!(
            "{} {path}",
            if unchanged { "unchanged" } else { "wrote    " }
        );
    }
    Ok(())
}

async fn serve(app: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // stdout carries the protocol; anything else goes to stderr.
    let service = BevaruServer::new(app).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["--app", _] | ["--app"] => {
            let app = match args.get(1) {
                Some(url) => Some(url.clone()),
                None if args.is_empty() => None,
                None => Some(DEFAULT_APP_URL.to_string()),
            };
            let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
            match runtime.block_on(serve(app)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("bevaru-mcp: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        ["gen-docs"] => match gen_docs(&repo_root()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("gen-docs failed: {e}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!(
                "usage: bevaru-mcp                serve MCP over stdio\n       bevaru-mcp --app [<url>]  also drive a running app (default {DEFAULT_APP_URL})\n       bevaru-mcp gen-docs       regenerate docs/agents/ reference files"
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_tool_is_served_somewhere() {
        let mut served: Vec<&str> = SERVED
            .iter()
            .chain(APP_TOOLS.iter().map(|(t, _)| t))
            .copied()
            .collect();
        let catalog = tools::tools();
        let mut expected: Vec<&str> = catalog.iter().map(|t| t.name.as_str()).collect();
        served.sort_unstable();
        expected.sort_unstable();
        assert_eq!(served, expected);
    }

    #[test]
    fn app_tools_only_with_an_app() {
        assert!(
            BevaruServer::new(None)
                .tools
                .iter()
                .all(|t| !t.name.starts_with("app_"))
        );
        let with_app = BevaruServer::new(Some(DEFAULT_APP_URL.into()));
        assert_eq!(
            with_app
                .tools
                .iter()
                .filter(|t| t.name.starts_with("app_"))
                .count(),
            APP_TOOLS.len()
        );
    }

    #[test]
    fn unreachable_app_is_a_clear_tool_error() {
        // Port 9 (discard) is essentially never an HTTP server.
        let err = call_app("http://127.0.0.1:9", "app_state", JsonObject::new())
            .err()
            .unwrap();
        assert_eq!(err.field, "app");
        assert!(err.message.contains("--remote"), "{}", err.message);
    }

    #[test]
    fn served_tools_are_exactly_the_headless_catalog() {
        let catalog: Vec<String> = tools::tools()
            .into_iter()
            .filter(|t| !t.requires_app)
            .map(|t| t.name)
            .collect();
        let mut served: Vec<String> = SERVED.iter().map(|s| s.to_string()).collect();
        let mut expected = catalog.clone();
        served.sort();
        expected.sort();
        assert_eq!(
            served, expected,
            "SERVED must list every headless catalog tool"
        );
        // Every served tool dispatches (bad arguments, but not "unknown tool").
        let manifest = builtin_manifest();
        for name in SERVED {
            let err = dispatch(
                &manifest,
                name,
                serde_json::Map::from_iter([("bogus".into(), Value::Null)]),
            );
            if let Err(e) = err {
                assert_ne!(e.field, "name", "{name} is not dispatched");
            }
        }
    }

    #[test]
    fn every_agent_doc_is_served_as_a_resource() {
        let root = repo_root();
        let mut docs = Vec::new();
        let mut dirs = vec![root.join("docs/agents")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = entry.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if matches!(p.extension().and_then(|e| e.to_str()), Some("md" | "json")) {
                    docs.push(p);
                }
            }
        }
        for doc in docs {
            let text = std::fs::read_to_string(&doc).unwrap();
            assert!(
                RESOURCES.iter().any(|r| r.3 == text),
                "{} is not served (or is stale; rebuild bevaru-mcp)",
                doc.display()
            );
        }
    }
}
