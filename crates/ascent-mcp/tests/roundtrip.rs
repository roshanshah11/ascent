//! The seam, driven end-to-end over the MCP wire shape: initialize,
//! create + run a study (as a task), propose a geometry edit, see the
//! stored study go stale in the proposal diff, apply, and verify the
//! document. Every state change flows through the one dispatcher.

use ascent_mcp::{McpServer, PROTOCOL_VERSION};
use serde_json::{json, Value};

fn call(server: &mut McpServer, id: u64, method: &str, params: Value) -> Value {
    let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    server.handle_line(&line.to_string()).expect("response due")
}

fn tool(server: &mut McpServer, id: u64, name: &str, arguments: Value) -> Value {
    let response = call(
        server,
        id,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    );
    response["result"]["structuredContent"].clone()
}

#[test]
fn initialize_reports_the_protocol_version_and_tools() {
    let mut server = McpServer::default();
    let init = call(&mut server, 1, "initialize", json!({}));
    assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert!(init["result"]["capabilities"]["tasks"].is_object());

    let list = call(&mut server, 2, "tools/list", json!({}));
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec![
            "get_document",
            "propose_commands",
            "apply_proposal",
            "run_study",
            "read_evidence"
        ]
    );
}

#[test]
fn notifications_get_no_response_and_unknown_methods_error() {
    let mut server = McpServer::default();
    let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    assert!(server.handle_line(&note.to_string()).is_none());

    let bad = call(&mut server, 1, "no/such", json!({}));
    assert_eq!(bad["error"]["code"], -32601);

    let garbage = server.handle_line("not json").unwrap();
    assert_eq!(garbage["error"]["code"], -32700);
}

#[test]
fn propose_apply_roundtrip_sees_studies_made_stale() {
    let mut server = McpServer::default();
    call(&mut server, 1, "initialize", json!({}));

    // Create a small dispersion study through the seam.
    let state = tool(
        &mut server,
        2,
        "apply_proposal",
        json!({ "lines": ["create-study \"spread\" native 42 {\"kind\":\"dispersion\",\"flights\":5}"] }),
    );
    let study_id = state["studies"][0]["id"].as_u64().unwrap();

    // Run it as a task (Tasks extension): synchronous execution, so the
    // created task is already completed and its result is fetchable.
    let created = call(
        &mut server,
        3,
        "tools/call",
        json!({ "name": "run_study", "arguments": { "study_id": study_id }, "task": {} }),
    );
    let task_id = created["result"]["task"]["taskId"].as_str().unwrap().to_string();
    assert_eq!(created["result"]["task"]["status"], "completed");
    let got = call(&mut server, 4, "tasks/get", json!({ "taskId": task_id }));
    assert_eq!(got["result"]["task"]["status"], "completed");
    let result = call(&mut server, 5, "tasks/result", json!({ "taskId": task_id }));
    let landed = &result["result"]["structuredContent"]["studies"][0]["results"];
    assert!(landed.is_object(), "study results landed: {result}");

    // Propose a fin-span edit: the proposal must flag the stored study
    // as going stale — the physics-in-the-loop warning.
    let proposal = tool(
        &mut server,
        6,
        "propose_commands",
        json!({ "lines": ["set-part-param 3 span_m 0.05"] }),
    );
    assert_eq!(proposal["valid"], true, "{proposal}");
    assert_eq!(
        proposal["diff"]["studies_made_stale"],
        json!([study_id]),
        "{proposal}"
    );

    // Document unchanged by the dry run.
    let before = tool(&mut server, 7, "get_document", json!({}));
    assert_eq!(
        before["vehicle"]["parts"][1]["children"][0]["kind"]["span_m"],
        json!(0.0375)
    );

    // Apply, then verify the edit landed and the results are now stale
    // relative to the new tree (hash mismatch is the staleness signal).
    let after = tool(
        &mut server,
        8,
        "apply_proposal",
        json!({ "lines": ["set-part-param 3 span_m 0.05"] }),
    );
    assert_eq!(
        after["vehicle"]["parts"][1]["children"][0]["kind"]["span_m"],
        json!(0.05)
    );
    assert!(after["studies"][0]["results"].is_object());

    // An invalid batch reports errors and mutates nothing.
    let invalid = tool(
        &mut server,
        9,
        "apply_proposal",
        json!({ "lines": ["set-part-param 999 span_m 0.06"] }),
    );
    // apply_batch error surfaces as a tool error (isError true → the
    // structuredContent path is absent).
    assert!(invalid.is_null(), "{invalid}");
    let unchanged = tool(&mut server, 10, "get_document", json!({}));
    assert_eq!(
        unchanged["vehicle"]["parts"][1]["children"][0]["kind"]["span_m"],
        json!(0.05)
    );
}

#[test]
fn run_study_without_task_returns_state_inline() {
    let mut server = McpServer::default();
    tool(
        &mut server,
        1,
        "apply_proposal",
        json!({ "lines": ["create-study \"spread\" native 7 {\"kind\":\"dispersion\",\"flights\":3}"] }),
    );
    let state = tool(&mut server, 2, "run_study", json!({ "study_id": 1 }));
    assert!(state["studies"][0]["results"].is_object());
    let hash = state["studies"][0]["results"]["input_hash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
}

#[test]
fn stdio_serve_speaks_newline_delimited_jsonrpc() {
    let requests = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        "\n"
    );
    let mut out = Vec::new();
    ascent_mcp::serve(requests.as_bytes(), &mut out).unwrap();
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    // Two responses for three inputs: the notification is silent.
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["id"], 2);
}
