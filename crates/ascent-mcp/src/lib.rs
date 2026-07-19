//! MCP stdio server over the copilot seam (docs/COPILOT_INTERFACE.md).
//!
//! Public surface: [`McpServer`] (one document, one protocol state
//! machine; `handle_line` maps one JSON-RPC request line to at most one
//! response line) and [`serve`] (the blocking stdio loop `main` runs).
//!
//! The server is a thin adapter: every tool call lands on the same
//! `Document` dispatcher every other client uses — `get_document`,
//! `propose_commands`, `apply_proposal` and `read_evidence` add zero
//! mutation logic, and `run_study` reuses the job runner's synchronous
//! path. No network anywhere: stdin in, stdout out, that is the whole
//! transport. Long-running studies adopt the 2026-07-28 Tasks extension
//! shape (`task` argument on `tools/call`, `tasks/get`, `tasks/result`,
//! `tasks/list`, `tasks/cancel`); execution itself stays synchronous and
//! deterministic, so a created task is already `completed` when the
//! client first polls it.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};

use ascent_app::{
    apply_batch, evidence_for, propose_batch, run_study_now, Command, Document, StudyId,
};
use serde_json::{json, Value};

pub const PROTOCOL_VERSION: &str = "2026-07-28";

pub struct McpServer {
    doc: Document,
    initialized: bool,
    tasks: BTreeMap<String, Value>,
    next_task: u64,
}

impl Default for McpServer {
    fn default() -> Self {
        Self {
            doc: Document::default(),
            initialized: false,
            tasks: BTreeMap::new(),
            next_task: 1,
        }
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Tool output per the spec: JSON payload as text content plus
/// `structuredContent` for clients that read it.
fn tool_ok(id: Value, payload: Value) -> Value {
    rpc_result(
        id,
        json!({
            "content": [{ "type": "text", "text": payload.to_string() }],
            "structuredContent": payload,
            "isError": false
        }),
    )
}

fn tool_err(id: Value, message: &str) -> Value {
    rpc_result(
        id,
        json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true
        }),
    )
}

fn parse_lines(arguments: &Value) -> Result<Vec<Command>, String> {
    let lines = arguments
        .get("lines")
        .and_then(Value::as_array)
        .ok_or("missing 'lines' array")?;
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let text = line.as_str().ok_or(format!("line {} is not a string", i + 1))?;
            Command::parse_text(text).map_err(|e| format!("line {}: {e}", i + 1))
        })
        .collect()
}

impl McpServer {
    /// Handle one newline-delimited JSON-RPC message; `None` means no
    /// response is due (notifications, parse-level garbage has an error).
    pub fn handle_line(&mut self, line: &str) -> Option<Value> {
        let message: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return Some(rpc_error(Value::Null, -32700, "parse error")),
        };
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let params = message.get("params").cloned().unwrap_or(json!({}));

        // Notifications get no response.
        let Some(id) = id else {
            return None;
        };

        Some(match method {
            "initialize" => {
                self.initialized = true;
                rpc_result(
                    id,
                    json!({
                        "protocolVersion": PROTOCOL_VERSION,
                        "capabilities": { "tools": {}, "tasks": {} },
                        "serverInfo": {
                            "name": "ascent-mcp",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                )
            }
            "ping" => rpc_result(id, json!({})),
            "tools/list" => rpc_result(id, json!({ "tools": tool_catalog() })),
            "tools/call" => self.tools_call(id, &params),
            "tasks/list" => {
                let tasks: Vec<&Value> = self.tasks.values().collect();
                rpc_result(id, json!({ "tasks": tasks }))
            }
            "tasks/get" => match self.task_for(&params) {
                Ok(task) => rpc_result(id, json!({ "task": task.get("task") })),
                Err(e) => rpc_error(id, -32602, &e),
            },
            "tasks/result" => match self.task_for(&params) {
                Ok(task) => rpc_result(id, task.get("result").cloned().unwrap_or(json!({}))),
                Err(e) => rpc_error(id, -32602, &e),
            },
            "tasks/cancel" => match self.task_for(&params) {
                // Synchronous execution: every stored task already
                // completed, and completed tasks cannot be cancelled.
                Ok(_) => rpc_error(id, -32602, "task already completed"),
                Err(e) => rpc_error(id, -32602, &e),
            },
            _ => rpc_error(id, -32601, &format!("method not found: {method}")),
        })
    }

    fn task_for(&self, params: &Value) -> Result<&Value, String> {
        let task_id = params
            .get("taskId")
            .and_then(Value::as_str)
            .ok_or("missing taskId")?;
        self.tasks
            .get(task_id)
            .ok_or(format!("unknown task: {task_id}"))
    }

    fn tools_call(&mut self, id: Value, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
        let as_task = params.get("task").is_some();

        let outcome: Result<Value, String> = match name {
            "get_document" => serde_json::to_value(self.doc.state()).map_err(|e| e.to_string()),
            "propose_commands" => parse_lines(&arguments).and_then(|commands| {
                serde_json::to_value(propose_batch(&self.doc, &commands)).map_err(|e| e.to_string())
            }),
            "apply_proposal" => parse_lines(&arguments).and_then(|commands| {
                apply_batch(&mut self.doc, &commands)?;
                serde_json::to_value(self.doc.state()).map_err(|e| e.to_string())
            }),
            "run_study" => {
                let study_id = arguments.get("study_id").and_then(Value::as_u64);
                match study_id {
                    None => Err("missing 'study_id'".into()),
                    Some(sid) => run_study_now(&mut self.doc, StudyId(sid as u32)).and_then(|()| {
                        serde_json::to_value(self.doc.state()).map_err(|e| e.to_string())
                    }),
                }
            }
            "read_evidence" => evidence_for(&self.doc.design)
                .and_then(|report| serde_json::to_value(report).map_err(|e| e.to_string())),
            _ => return tool_err(id, &format!("unknown tool: {name}")),
        };

        match outcome {
            Err(e) => tool_err(id, &e),
            Ok(payload) => {
                if as_task {
                    // Tasks extension: synchronous execution means the
                    // task is complete the moment it exists.
                    let task_id = format!("task-{}", self.next_task);
                    self.next_task += 1;
                    let result = json!({
                        "content": [{ "type": "text", "text": payload.to_string() }],
                        "structuredContent": payload,
                        "isError": false
                    });
                    self.tasks.insert(
                        task_id.clone(),
                        json!({
                            "task": { "taskId": task_id, "status": "completed" },
                            "result": result
                        }),
                    );
                    rpc_result(id, json!({ "task": { "taskId": task_id, "status": "completed" } }))
                } else {
                    tool_ok(id, payload)
                }
            }
        }
    }
}

fn tool_catalog() -> Value {
    let lines_schema = json!({
        "type": "object",
        "properties": {
            "lines": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Journal-grammar command lines (docs/JOURNAL_FORMAT.md)"
            }
        },
        "required": ["lines"]
    });
    json!([
        {
            "name": "get_document",
            "description": "Read the full document state: vehicle tree, design, studies with results and hashes, undo/redo flags.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "propose_commands",
            "description": "Dry-run a batch of journal-grammar command lines. Mutates nothing; returns per-command errors and, when valid, a diff summary including studies_made_stale.",
            "inputSchema": lines_schema
        },
        {
            "name": "apply_proposal",
            "description": "Re-validate and apply a command batch atomically; any error leaves the document untouched. Applied commands journal like GUI edits.",
            "inputSchema": lines_schema
        },
        {
            "name": "run_study",
            "description": "Run a study by id against the current document and land its hash-stamped results. Supports task-augmented calls for long dispersions.",
            "inputSchema": {
                "type": "object",
                "properties": { "study_id": { "type": "integer" } },
                "required": ["study_id"]
            }
        },
        {
            "name": "read_evidence",
            "description": "Deterministic evidence report for the current design: scorecard, input hashes, convergence.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

/// Blocking stdio loop: one JSON-RPC message per line in, one per line
/// out. The entire transport — no sockets, no network.
pub fn serve(input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    let mut server = McpServer::default();
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_line(&line) {
            writeln!(output, "{response}")?;
            output.flush()?;
        }
    }
    Ok(())
}
