use ascent_mcp::AscentMcp;
use rmcp::{
    model::{
        CallToolRequestParams, CancelTaskParams, ClientInfo, ClientRequest, GetTaskParams,
        GetTaskPayloadParams, Request, ServerResult, TaskMetadata, TaskStatus, TaskSupport,
    },
    ClientHandler, ServiceExt,
};
use serde_json::{json, Value};

#[derive(Debug, Clone, Default)]
struct TestClient;

impl ClientHandler for TestClient {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

fn arguments(value: Value) -> serde_json::Map<String, Value> {
    value.as_object().expect("object arguments").clone()
}

async fn sdk_pair() -> (
    rmcp::service::RunningService<rmcp::RoleClient, TestClient>,
    tokio::task::JoinHandle<()>,
) {
    let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
    let server = AscentMcp::new();
    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await.expect("server starts");
        service.waiting().await.expect("server closes");
    });
    let client = TestClient
        .serve(client_transport)
        .await
        .expect("client starts");
    (client, server_handle)
}

#[tokio::test]
async fn sdk_lists_exactly_the_five_public_tools() {
    let (client, server) = sdk_pair().await;
    let listed = client.list_tools(None).await.expect("tools/list");
    let names: Vec<_> = listed.tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(
        names,
        [
            "apply_proposal",
            "get_document",
            "propose_commands",
            "read_evidence",
            "run_study",
        ]
    );
    let run_study = listed
        .tools
        .iter()
        .find(|tool| tool.name == "run_study")
        .expect("run_study is listed");
    assert_eq!(run_study.task_support(), TaskSupport::Optional);
    client.cancel().await.expect("client closes");
    server.await.expect("server task joins");
}

#[tokio::test]
async fn study_id_overflow_is_a_visible_tool_error() {
    let (client, server) = sdk_pair().await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("run_study")
                .with_arguments(arguments(json!({ "study_id": 4_294_967_296_u64 }))),
        )
        .await
        .expect("tool call returns a result");
    assert_eq!(result.is_error, Some(true));
    let text = result.content[0].as_text().expect("text error");
    assert!(text.text.contains("out of range"), "{}", text.text);
    client.cancel().await.expect("client closes");
    server.await.expect("server task joins");
}

#[tokio::test]
async fn propose_and_apply_report_studies_made_stale() {
    let (client, server) = sdk_pair().await;
    let created = client
        .call_tool(
            CallToolRequestParams::new("apply_proposal").with_arguments(arguments(json!({
                "lines": ["create-study \"spread\" native 42 {\"kind\":\"dispersion\",\"flights\":5}"]
            }))),
        )
        .await
        .expect("create study");
    assert_eq!(created.is_error, Some(false));

    let ran = client
        .call_tool(
            CallToolRequestParams::new("run_study")
                .with_arguments(arguments(json!({ "study_id": 1 }))),
        )
        .await
        .expect("run study");
    assert_eq!(ran.is_error, Some(false));

    let proposal = client
        .call_tool(
            CallToolRequestParams::new("propose_commands").with_arguments(arguments(json!({
                "lines": ["set-part-param 3 span_m 0.05"]
            }))),
        )
        .await
        .expect("propose edit");
    let structured = proposal.structured_content.expect("structured proposal");
    assert_eq!(structured["valid"], true);
    assert_eq!(structured["diff"]["studies_made_stale"], json!([1]));

    client.cancel().await.expect("client closes");
    server.await.expect("server task joins");
}

#[tokio::test]
async fn sdk_task_call_has_a_real_working_to_completed_lifecycle() {
    let (client, server) = sdk_pair().await;
    client
        .call_tool(
            CallToolRequestParams::new("apply_proposal").with_arguments(arguments(json!({
                "lines": ["create-study \"task\" native 7 {\"kind\":\"dispersion\",\"flights\":5}"]
            }))),
        )
        .await
        .expect("create study");

    let response = client
        .send_request(ClientRequest::CallToolRequest(Request::new(
            CallToolRequestParams::new("run_study")
                .with_arguments(arguments(json!({ "study_id": 1 })))
                .with_task(TaskMetadata::new()),
        )))
        .await
        .expect("task-augmented tools/call");
    let ServerResult::CreateTaskResult(created) = response else {
        panic!("expected task creation response");
    };
    assert_eq!(created.task.status, TaskStatus::Working);

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let response = client
            .send_request(ClientRequest::GetTaskRequest(Request::new(
                GetTaskParams::new(created.task.task_id.clone()),
            )))
            .await
            .expect("tasks/get");
        let ServerResult::GetTaskResult(task) = response else {
            panic!("expected task status response");
        };
        if task.task.status == TaskStatus::Completed {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "task did not complete"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let response = client
        .send_request(ClientRequest::GetTaskPayloadRequest(Request::new(
            GetTaskPayloadParams::new(created.task.task_id),
        )))
        .await
        .expect("tasks/result");
    let payload = match response {
        ServerResult::GetTaskPayloadResult(result) => result.0,
        ServerResult::CustomResult(result) => result.0,
        ServerResult::CallToolResult(result) => {
            result.structured_content.expect("structured task result")
        }
        other => panic!("expected task payload response, got {other:?}"),
    };
    let studies = payload.get("structuredContent").unwrap_or(&payload)["studies"].clone();
    assert!(studies[0]["results"].is_object());

    client.cancel().await.expect("client closes");
    server.await.expect("server task joins");
}

#[tokio::test]
async fn sdk_task_cancellation_stops_work_before_results_land() {
    let (client, server) = sdk_pair().await;
    client
        .call_tool(
            CallToolRequestParams::new("apply_proposal").with_arguments(arguments(json!({
                "lines": ["create-study \"cancel\" native 7 {\"kind\":\"dispersion\",\"flights\":20000}"]
            }))),
        )
        .await
        .expect("create study");

    let response = client
        .send_request(ClientRequest::CallToolRequest(Request::new(
            CallToolRequestParams::new("run_study")
                .with_arguments(arguments(json!({ "study_id": 1 })))
                .with_task(TaskMetadata::new()),
        )))
        .await
        .expect("task-augmented tools/call");
    let ServerResult::CreateTaskResult(created) = response else {
        panic!("expected task creation response");
    };

    let response = client
        .send_request(ClientRequest::CancelTaskRequest(Request::new(
            CancelTaskParams::new(created.task.task_id),
        )))
        .await
        .expect("tasks/cancel");
    let cancelled = match response {
        ServerResult::CancelTaskResult(result) => result.task,
        // RMCP 2.2 decodes the wire-identical task payload through the
        // generic task-status result variant on its client side.
        ServerResult::GetTaskResult(result) => result.task,
        other => panic!("expected cancellation response, got {other:?}"),
    };
    assert_eq!(cancelled.status, TaskStatus::Cancelled);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let document = client
        .call_tool(CallToolRequestParams::new("get_document"))
        .await
        .expect("get document")
        .structured_content
        .expect("structured document");
    assert!(document["studies"][0]["results"].is_null());

    client.cancel().await.expect("client closes");
    server.await.expect("server task joins");
}

#[test]
fn production_features_and_startup_are_stdio_only() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("\"server\", \"macros\", \"transport-io\""));
    for forbidden in ["server-side-http", "reqwest", "transport-streamable-http"] {
        assert!(
            !manifest.contains(forbidden),
            "enabled forbidden feature: {forbidden}"
        );
    }
    let main = include_str!("../src/main.rs");
    assert!(main.contains("rmcp::transport::stdio()"));
    assert!(!main.contains("TcpListener"));
    assert!(!main.contains("Http"));
}
