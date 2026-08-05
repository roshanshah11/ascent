//! Official RMCP stdio server over Ascent's single command dispatcher.
//!
//! The five tools are typed SDK tools, document state is server-owned, and
//! every mutation still passes through `Document::dispatch` via the existing
//! proposal and study seams. The only production transport is Tokio stdio.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use ascent_app::{
    apply_batch, evidence_for, evidence_for_study, propose_batch, run_study_now, Command, Document,
    DocumentState, JobId, JobRunner, JobStatus, StudyId,
};
use chrono::{SecondsFormat, Utc};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolRequestParams, CallToolResult, CancelTaskParams, CancelTaskResult,
        CreateTaskResult, GetTaskParams, GetTaskPayloadParams, GetTaskPayloadResult, GetTaskResult,
        ListTasksResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Task, TaskStatus,
        TasksCapability,
    },
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LinesRequest {
    /// Journal-grammar command lines from docs/JOURNAL_FORMAT.md.
    lines: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct StudyRequest {
    study_id: u64,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct EvidenceRequest {
    /// Optional completed study whose result hashes should be linked explicitly.
    study_id: Option<u64>,
}

#[derive(Clone)]
struct TaskRecord {
    task: Task,
    job_id: JobId,
    result: Option<Value>,
}

/// One RMCP server instance, owning one Ascent document and its task runner.
#[derive(Clone)]
pub struct AscentMcp {
    tool_router: ToolRouter<Self>,
    document: Arc<Mutex<Document>>,
    jobs: Arc<JobRunner>,
    tasks: Arc<Mutex<BTreeMap<String, TaskRecord>>>,
}

impl Default for AscentMcp {
    fn default() -> Self {
        Self::new()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn parse_lines(lines: &[String]) -> Result<Vec<Command>, String> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            Command::parse_text(line).map_err(|error| format!("line {}: {error}", index + 1))
        })
        .collect()
}

fn study_id(value: u64) -> Result<StudyId, String> {
    u32::try_from(value)
        .map(StudyId)
        .map_err(|_| format!("study_id {value} is out of range for a 32-bit study id"))
}

fn structured<T: serde::Serialize>(value: &T) -> Result<CallToolResult, String> {
    serde_json::to_value(value)
        .map(CallToolResult::structured)
        .map_err(|error| format!("serialize tool result: {error}"))
}

#[tool_router(router = tool_router)]
impl AscentMcp {
    pub fn new() -> Self {
        let document = Arc::new(Mutex::new(Document::default()));
        let jobs = JobRunner::new(document.clone(), |_| {});
        Self {
            tool_router: Self::tool_router(),
            document,
            jobs,
            tasks: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    #[tool(description = "Read the full Ascent document state without mutating it")]
    async fn get_document(&self) -> Result<CallToolResult, String> {
        structured(&lock(&self.document).state())
    }

    #[tool(
        description = "Dry-run journal-grammar commands and report validation and stale studies"
    )]
    async fn propose_commands(
        &self,
        Parameters(request): Parameters<LinesRequest>,
    ) -> Result<CallToolResult, String> {
        let commands = parse_lines(&request.lines)?;
        structured(&propose_batch(&lock(&self.document), &commands))
    }

    #[tool(description = "Atomically validate and apply journal-grammar commands")]
    async fn apply_proposal(
        &self,
        Parameters(request): Parameters<LinesRequest>,
    ) -> Result<CallToolResult, String> {
        let commands = parse_lines(&request.lines)?;
        let mut document = lock(&self.document);
        apply_batch(&mut document, &commands)?;
        structured(&document.state())
    }

    #[tool(
        description = "Run a study and land deterministic hash-stamped results",
        execution(task_support = "optional")
    )]
    async fn run_study(
        &self,
        Parameters(request): Parameters<StudyRequest>,
    ) -> Result<CallToolResult, String> {
        let id = study_id(request.study_id)?;
        let mut document = lock(&self.document);
        run_study_now(&mut document, id)?;
        structured(&document.state())
    }

    #[tool(
        description = "Read deterministic evidence; optional study_id links result and current-input hashes"
    )]
    async fn read_evidence(
        &self,
        Parameters(request): Parameters<EvidenceRequest>,
    ) -> Result<CallToolResult, String> {
        let document = lock(&self.document);
        let report = match request.study_id {
            Some(value) => {
                let id = study_id(value)?;
                let study = document
                    .studies
                    .iter()
                    .find(|study| study.id == id)
                    .ok_or_else(|| format!("no study {value}"))?;
                evidence_for_study(
                    &document.vehicle,
                    &document.design,
                    document.atmosphere.as_ref(),
                    study,
                )?
            }
            None => evidence_for(&document.design)?,
        };
        structured(&report)
    }

    fn refresh_task(&self, task_id: &str) -> Result<TaskRecord, McpError> {
        let job_id = lock(&self.tasks)
            .get(task_id)
            .map(|record| record.job_id)
            .ok_or_else(|| McpError::invalid_params(format!("unknown task: {task_id}"), None))?;
        let job = self
            .jobs
            .jobs()
            .into_iter()
            .find(|job| job.id == job_id)
            .ok_or_else(|| McpError::internal_error("task job disappeared", None))?;

        let mut tasks = lock(&self.tasks);
        let record = tasks
            .get_mut(task_id)
            .expect("task exists while its record lock is held");
        if matches!(
            record.task.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            return Ok(record.clone());
        }

        let (status, message) = match job.status {
            JobStatus::Queued => (TaskStatus::Working, Some("queued".to_string())),
            JobStatus::Running => (TaskStatus::Working, Some("running".to_string())),
            JobStatus::Done => {
                let state: DocumentState = lock(&self.document).state();
                let payload = serde_json::to_value(CallToolResult::structured(
                    serde_json::to_value(state).map_err(|error| {
                        McpError::internal_error(format!("serialize task state: {error}"), None)
                    })?,
                ))
                .map_err(|error| {
                    McpError::internal_error(format!("serialize task result: {error}"), None)
                })?;
                record.result = Some(payload);
                (TaskStatus::Completed, Some("completed".to_string()))
            }
            JobStatus::Cancelled => (TaskStatus::Cancelled, Some("cancelled".to_string())),
            JobStatus::Failed => (
                TaskStatus::Failed,
                Some(job.error.unwrap_or_else(|| "study failed".to_string())),
            ),
        };
        if record.task.status != status || record.task.status_message != message {
            record.task.status = status;
            record.task.status_message = message;
            record.task.last_updated_at = now();
        }
        Ok(record.clone())
    }

    fn task_ids(&self) -> Vec<String> {
        lock(&self.tasks).keys().cloned().collect()
    }

    fn enqueue_study_task(&self, study_id: StudyId) -> Result<CreateTaskResult, McpError> {
        let job_id = self
            .jobs
            .enqueue(study_id)
            .map_err(|error| McpError::invalid_params(error, None))?;
        let timestamp = now();
        let task = Task::new(
            format!("study-{}", job_id.0),
            TaskStatus::Working,
            timestamp.clone(),
            timestamp,
        )
        .with_status_message("queued")
        .with_poll_interval(50);
        lock(&self.tasks).insert(
            task.task_id.clone(),
            TaskRecord {
                task: task.clone(),
                job_id,
                result: None,
            },
        );
        Ok(CreateTaskResult::new(task))
    }

    fn cancel_task_record(&self, task_id: &str) -> Result<CancelTaskResult, McpError> {
        let record = self.refresh_task(task_id)?;
        if matches!(
            record.task.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            return Err(McpError::invalid_params(
                format!("task {task_id} is already terminal"),
                None,
            ));
        }
        self.jobs
            .cancel(record.job_id)
            .map_err(|error| McpError::invalid_params(error, None))?;
        let mut tasks = lock(&self.tasks);
        let record = tasks
            .get_mut(task_id)
            .expect("task exists while its record lock is held");
        record.task.status = TaskStatus::Cancelled;
        record.task.status_message = Some("cancellation requested".to_string());
        record.task.last_updated_at = now();
        Ok(CancelTaskResult::new(record.task.clone()))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AscentMcp {
    fn get_info(&self) -> ServerInfo {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_tasks_with(TasksCapability::server_default())
            .build();
        ServerInfo::new(capabilities).with_instructions(
            "Ascent is deterministic and offline. Mutations use journal-grammar command batches.",
        )
    }

    async fn enqueue_task(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<CreateTaskResult, McpError> {
        if request.name.as_ref() != "run_study" {
            return Err(McpError::invalid_params(
                format!("tool {} does not support task execution", request.name),
                None,
            ));
        }
        let arguments = request.arguments.unwrap_or_default();
        let parsed: StudyRequest = serde_json::from_value(Value::Object(arguments))
            .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        let id =
            study_id(parsed.study_id).map_err(|error| McpError::invalid_params(error, None))?;
        self.enqueue_study_task(id)
    }

    async fn list_tasks(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListTasksResult, McpError> {
        let mut tasks = Vec::new();
        for task_id in self.task_ids() {
            tasks.push(self.refresh_task(&task_id)?.task);
        }
        tasks.sort_by(|left, right| left.task_id.cmp(&right.task_id));
        Ok(ListTasksResult::new(tasks))
    }

    async fn get_task_info(
        &self,
        request: GetTaskParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<GetTaskResult, McpError> {
        Ok(GetTaskResult::new(
            self.refresh_task(&request.task_id)?.task,
        ))
    }

    async fn get_task_result(
        &self,
        request: GetTaskPayloadParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<GetTaskPayloadResult, McpError> {
        let record = self.refresh_task(&request.task_id)?;
        match (record.task.status, record.result) {
            (TaskStatus::Completed, Some(result)) => Ok(GetTaskPayloadResult::new(result)),
            (status, _) => Err(McpError::invalid_params(
                format!("task {} is not completed ({status:?})", request.task_id),
                None,
            )),
        }
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<CancelTaskResult, McpError> {
        self.cancel_task_record(&request.task_id)
    }
}
