//! End to end: an MCP client drives a running bevaru app through
//! `bevaru-mcp --app`, over Bevy Remote Protocol on localhost. The app runs
//! headless in a background thread, with the real HTTP server.

use std::process::Stdio;
use std::time::{Duration, Instant};

use bevaru::BevaruPlugin;
use bevaru::lobby::LobbyPlugin;
use bevaru::remote::RemoteHooksPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt, service::RunningService};
use serde_json::{Value, json};

fn start_app() -> u16 {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    std::thread::spawn(move || {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .add_plugins((BevaruPlugin, LobbyPlugin::default()))
            .add_plugins(RemoteHooksPlugin { port: Some(port) });
        loop {
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    port
}

async fn call(
    client: &RunningService<RoleClient, ()>,
    name: &'static str,
    args: Value,
) -> CallToolResult {
    let Value::Object(args) = args else { panic!() };
    client
        .call_tool(CallToolRequestParams::new(name).with_arguments(args))
        .await
        .unwrap()
}

fn text(r: &CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect()
}

/// Poll `app_state` until `done` holds.
async fn wait_for(
    client: &RunningService<RoleClient, ()>,
    what: &str,
    done: impl Fn(&Value) -> bool,
) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let r = call(client, "app_state", json!({})).await;
        if let Some(state) = r.structured_content.clone().filter(|s| done(s)) {
            return state;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {}",
            text(&r)
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn an_agent_drives_a_running_app() {
    let port = start_app();
    let url = format!("http://127.0.0.1:{port}");
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_bevaru-mcp"));
    command.arg("--app").arg(&url);
    let (transport, _) = TokioChildProcess::builder(command)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let client = ().serve(transport).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|t| t.name == "app_enter_experience"));

    // The app comes up in the lobby.
    wait_for(&client, "the lobby", |s| s["screen"] == "lobby").await;

    let r = call(
        &client,
        "app_enter_experience",
        json!({"id": "loss-curves"}),
    )
    .await;
    assert_ne!(r.is_error, Some(true), "{}", text(&r));
    let state = wait_for(&client, "loss-curves running", |s| {
        s["screen"] == "running"
            && s["active_experience"] == "loss-curves"
            && !s["experiment"].is_null()
    })
    .await;
    assert_eq!(state["experiment"]["panes"][0]["trainer"]["loss"], "hinge");

    let r = call(&client, "app_playback", json!({"command": "pause"})).await;
    assert_ne!(r.is_error, Some(true), "{}", text(&r));
    let before = call(&client, "app_state", json!({}))
        .await
        .structured_content
        .unwrap()["playback"]["step"]
        .as_u64()
        .unwrap();
    call(&client, "app_playback", json!({"command": "step"})).await;
    wait_for(&client, "one more step", |s| {
        s["playback"]["step"].as_u64() == Some(before + 1)
    })
    .await;

    // Unknown ids are tool errors listing the valid ones; the app doesn't move.
    let r = call(&client, "app_enter_experience", json!({"id": "nonsense"})).await;
    assert_eq!(r.is_error, Some(true));
    assert!(text(&r).contains("iris-svm"), "{}", text(&r));

    call(&client, "app_leave_experience", json!({})).await;
    let state = wait_for(&client, "back in the lobby", |s| s["screen"] == "lobby").await;
    assert!(state["experiment"].is_null());

    client.cancel().await.unwrap();
}
