use ascent_mcp::AscentMcp;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, ClientInfo, ClientRequest, GetTaskParams,
        GetTaskPayloadParams, Request, ServerResult, TaskMetadata, TaskStatus,
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

async fn call_tool(
    client: &rmcp::service::RunningService<rmcp::RoleClient, TestClient>,
    name: &'static str,
    args: Value,
) -> CallToolResult {
    client
        .call_tool(CallToolRequestParams::new(name).with_arguments(arguments(args)))
        .await
        .unwrap_or_else(|error| panic!("{name} request failed: {error}"))
}

async fn tool(
    client: &rmcp::service::RunningService<rmcp::RoleClient, TestClient>,
    name: &'static str,
    args: Value,
) -> Value {
    let result = call_tool(client, name, args).await;
    assert_eq!(result.is_error, Some(false), "{name} returned {result:?}");
    result
        .structured_content
        .unwrap_or_else(|| panic!("{name} returned no structured content"))
}

async fn run_study_task(
    client: &rmcp::service::RunningService<rmcp::RoleClient, TestClient>,
    study_id: u64,
) -> Value {
    let response = client
        .send_request(ClientRequest::CallToolRequest(Request::new(
            CallToolRequestParams::new("run_study")
                .with_arguments(arguments(json!({ "study_id": study_id })))
                .with_task(TaskMetadata::new()),
        )))
        .await
        .expect("task-augmented run_study");
    let ServerResult::CreateTaskResult(created) = response else {
        panic!("expected task creation response");
    };

    // Guard against a hung task, not a perf bound: matches the 30s job-wait
    // convention in jobs.rs so a real study running under saturated parallel
    // test load cannot trip a too-tight deadline.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
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
    payload.get("structuredContent").cloned().unwrap_or(payload)
}

async fn seed_study(client: &rmcp::service::RunningService<rmcp::RoleClient, TestClient>) {
    tool(
        client,
        "apply_proposal",
        json!({"lines":["create-study \"agent eval\" native 42 {\"kind\":\"dispersion\",\"flights\":5}"]}),
    )
    .await;
    tool(client, "run_study", json!({"study_id": 1})).await;
}

async fn close(
    client: rmcp::service::RunningService<rmcp::RoleClient, TestClient>,
    server: tokio::task::JoinHandle<()>,
) {
    client.cancel().await.expect("client closes");
    server.await.expect("server task joins");
}

#[tokio::test]
async fn fin_widen_stales_verified_study_matches_golden() {
    let golden: Value = serde_json::from_str(include_str!(
        "fixtures/evals/fin_widen_stales_verified_study.json"
    ))
    .unwrap();
    let (client, server) = sdk_pair().await;
    seed_study(&client).await;

    let proposal = tool(
        &client,
        "propose_commands",
        json!({"lines":["set-part-param 3 span_m 0.05"]}),
    )
    .await;
    assert_eq!(
        proposal["diff"]["studies_made_stale"],
        golden["studies_made_stale"]
    );

    let state = tool(
        &client,
        "apply_proposal",
        json!({"lines":["set-part-param 3 span_m 0.05"]}),
    )
    .await;
    assert_eq!(
        state["vehicle"]["parts"][1]["children"][0]["kind"]["span_m"],
        golden["span_m"]
    );

    let task = run_study_task(&client, 1).await;
    assert_eq!(
        task["studies"][0]["results"]["input_hash"]
            .as_str()
            .unwrap()
            .len(),
        golden["hash_length"]
    );

    let evidence = tool(&client, "read_evidence", json!({"study_id":1})).await;
    assert_eq!(evidence["links"][0]["relation"], golden["relation"]);
    close(client, server).await;
}

#[tokio::test]
async fn invalid_batch_is_atomic_then_agent_recovers_matches_golden() {
    let golden: Value = serde_json::from_str(include_str!(
        "fixtures/evals/invalid_batch_is_atomic_then_agent_recovers.json"
    ))
    .unwrap();
    let (client, server) = sdk_pair().await;
    let before = tool(&client, "get_document", json!({})).await;

    let rejected = call_tool(
        &client,
        "apply_proposal",
        json!({"lines":["set-part-param 999 span_m 0.06"]}),
    )
    .await;
    assert_eq!(rejected.is_error, Some(true));
    assert!(rejected.content[0]
        .as_text()
        .expect("text error")
        .text
        .contains(golden["error_contains"].as_str().unwrap()));
    assert_eq!(tool(&client, "get_document", json!({})).await, before);

    let after = tool(
        &client,
        "apply_proposal",
        json!({"lines":["set-part-param 3 span_m 0.05"]}),
    )
    .await;
    assert_eq!(
        after["vehicle"]["parts"][1]["children"][0]["kind"]["span_m"],
        golden["span_m"]
    );
    close(client, server).await;
}

#[tokio::test]
async fn task_results_and_evidence_hash_links_match_golden() {
    let golden: Value = serde_json::from_str(include_str!(
        "fixtures/evals/task_results_and_evidence_hash_links.json"
    ))
    .unwrap();
    let (client, server) = sdk_pair().await;
    tool(
        &client,
        "apply_proposal",
        json!({"lines":["create-study \"evidence\" native 42 {\"kind\":\"dispersion\",\"flights\":5}"]}),
    )
    .await;

    let result = run_study_task(&client, 1).await;
    assert_eq!(
        result["studies"][0]["results"]["input_hash"]
            .as_str()
            .unwrap()
            .len(),
        golden["hash_length"]
    );
    let evidence = tool(&client, "read_evidence", json!({"study_id": 1})).await;
    assert_eq!(evidence["study"]["id"], golden["study_id"]);
    assert_eq!(evidence["links"][0]["relation"], golden["relation"]);
    close(client, server).await;
}

#[test]
fn eval_harness_rejects_a_changed_stale_warning() {
    let golden: Value = serde_json::from_str(include_str!(
        "fixtures/evals/fin_widen_stales_verified_study.json"
    ))
    .unwrap();
    let changed = json!([]);
    assert!(
        !golden_field_matches(&changed, &golden["studies_made_stale"]),
        "the eval comparator must reject a changed seam warning"
    );
}

fn golden_field_matches(actual: &Value, expected: &Value) -> bool {
    actual == expected
}
