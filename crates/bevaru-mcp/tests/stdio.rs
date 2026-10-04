//! End to end: start the real `bevaru-mcp` binary and talk MCP to it over
//! stdio, as an agent's client would.

use std::process::Stdio;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ReadResourceRequestParams, ResourceContents,
};
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt, service::RunningService};
use serde_json::{Value, json};

async fn connect() -> RunningService<RoleClient, ()> {
    let (transport, _stderr) = TokioChildProcess::builder(tokio::process::Command::new(env!(
        "CARGO_BIN_EXE_bevaru-mcp"
    )))
    .stderr(Stdio::null())
    .spawn()
    .expect("start bevaru-mcp");
    ().serve(transport).await.expect("initialize")
}

async fn call(
    client: &RunningService<RoleClient, ()>,
    name: &'static str,
    args: Value,
) -> CallToolResult {
    let Value::Object(args) = args else {
        panic!("arguments must be an object")
    };
    client
        .call_tool(CallToolRequestParams::new(name).with_arguments(args))
        .await
        .expect("call_tool")
}

fn text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn an_agent_session() {
    let client = connect().await;

    // Tools match the headless catalog, each with an object input schema.
    let tools = client.list_all_tools().await.unwrap();
    let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    names.sort();
    let mut expected: Vec<String> = bevaru::agent::tools::tools()
        .into_iter()
        .filter(|t| !t.requires_app)
        .map(|t| t.name)
        .collect();
    expected.sort();
    assert_eq!(names, expected);
    assert!(
        tools
            .iter()
            .all(|t| t.input_schema.get("type") == Some(&json!("object")))
    );

    // Hinge loss at margins 1.5 and 0.25 (the spec's example).
    let r = call(
        &client,
        "evaluate_losses",
        json!({"losses": ["hinge"], "points": [1.5, 0.25], "margin": 1}),
    )
    .await;
    assert_ne!(r.is_error, Some(true), "{}", text(&r));
    let out = r.structured_content.expect("structured result");
    assert!(out.is_object(), "MCP structured content must be an object");
    assert_eq!(out["evaluations"][0]["values"], json!([0.0, 0.75]));
    assert_eq!(out["evaluations"][0]["gradients"], json!([0.0, -1.0]));

    // Every structured result is an object, including list-shaped ones.
    for (tool, args) in [
        ("list_experiences", json!({})),
        ("describe", json!({"section": "losses"})),
        (
            "build_dataset",
            json!({"dataset": {"kind": "separable-blobs"}}),
        ),
    ] {
        let r = call(&client, tool, args).await;
        assert!(
            r.structured_content.as_ref().is_some_and(Value::is_object),
            "{tool}: {}",
            text(&r)
        );
    }

    // Train an SVM on separable blobs.
    let r = call(
        &client,
        "train",
        json!({"dataset": {"kind": "separable-blobs"}, "trainer": {"loss": "hinge", "c": 1, "max_steps": 200}}),
    )
    .await;
    let out = r.structured_content.expect("structured result");
    assert_eq!(out["model"], "svm");
    assert!(out["steps"].as_u64().unwrap() <= 200);
    assert_eq!(out["params"]["weights"].as_array().unwrap().len(), 2);
    assert!(!out["support_vectors"].as_array().unwrap().is_empty());

    // An invalid hyperparameter is a tool error naming the field; the server keeps serving.
    let r = call(
        &client,
        "train",
        json!({"dataset": {"kind": "regression", "features": 1, "outlier_fraction": 0.1},
               "trainer": {"loss": "huber", "huber_delta": 0}}),
    )
    .await;
    assert_eq!(r.is_error, Some(true));
    assert!(text(&r).contains("trainer.huber_delta"), "{}", text(&r));

    // Charts arrive as PNG images.
    let r = call(
        &client,
        "render_loss_chart",
        json!({"losses": ["hinge", "logistic", "zero-one"]}),
    )
    .await;
    let image = r.content[0].as_image().expect("image content");
    assert_eq!(image.mime_type, "image/png");
    assert!(
        image.data.starts_with("iVBORw0KGgo"),
        "base64 of the PNG signature"
    );

    // The manifest resource is the generated file.
    let rr = client
        .read_resource(ReadResourceRequestParams::new("bevaru://capabilities.json"))
        .await
        .unwrap();
    let ResourceContents::TextResourceContents { text: served, .. } = &rr.contents[0] else {
        panic!("text resource expected");
    };
    let generated = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/agents/capabilities.json"
    ))
    .unwrap();
    assert_eq!(served, &generated);

    client.cancel().await.unwrap();
}
