//! Managed job runner (v0.3 Step 5): long computations leave the IPC
//! thread. Jobs are queued onto one worker thread, report progress
//! through an event sink, and cancel cooperatively. Completion lands
//! results through the document's command dispatcher — a job's output is
//! a journaled `SetStudyResults` like any other mutation, stamped with
//! the input hash of the exact snapshot it computed from. A cancelled or
//! failed job dispatches nothing: the study is untouched.
//!
//! The sink is a plain closure so the runner is fully testable headless;
//! the Tauri layer's sink forwards to `job-progress` / `job-done` events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::design::Design;
use crate::dispersion_ipc::{self, DispersionRequest};
use crate::document::Document;
use crate::study::{study_input_hash, Study, StudyId, StudyKind, StudyResults};
use crate::Command;
use ascent_domain::vehicle::Vehicle;
use ascent_sim::{Variation, VaryParam};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Cancelled,
    Failed,
}

/// What the sink receives. `Progress` is throttled (every
/// `PROGRESS_STRIDE` flights plus completion); `Done` is always final
/// and exactly once per job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum JobEvent {
    Progress {
        job_id: JobId,
        study_id: StudyId,
        completed: u32,
        total: u32,
    },
    Done {
        job_id: JobId,
        study_id: StudyId,
        status: JobStatus,
        error: Option<String>,
    },
}

/// Snapshot DTO for the jobs panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobView {
    pub id: JobId,
    pub study_id: StudyId,
    pub status: JobStatus,
    pub error: Option<String>,
}

const PROGRESS_STRIDE: u32 = 50;

struct JobRecord {
    study_id: StudyId,
    status: JobStatus,
    error: Option<String>,
    cancel: Arc<AtomicBool>,
}

struct Ticket {
    id: JobId,
    study_id: StudyId,
    cancel: Arc<AtomicBool>,
}

pub struct JobRunner {
    doc: Arc<Mutex<Document>>,
    queue: mpsc::Sender<Ticket>,
    jobs: Arc<Mutex<HashMap<u64, JobRecord>>>,
    next_id: Mutex<u64>,
}

fn lock<'a, T>(m: &'a Mutex<T>) -> std::sync::MutexGuard<'a, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl JobRunner {
    pub fn new(
        doc: Arc<Mutex<Document>>,
        sink: impl Fn(JobEvent) + Send + 'static,
    ) -> Arc<JobRunner> {
        let (queue, receiver) = mpsc::channel::<Ticket>();
        let jobs: Arc<Mutex<HashMap<u64, JobRecord>>> = Arc::new(Mutex::new(HashMap::new()));
        let runner = Arc::new(JobRunner {
            doc: doc.clone(),
            queue,
            jobs: jobs.clone(),
            next_id: Mutex::new(1),
        });
        std::thread::spawn(move || {
            // Worker: one job at a time, in enqueue order. The loop ends
            // when the runner (the only sender) is dropped.
            while let Ok(ticket) = receiver.recv() {
                run_one(&doc, &jobs, &sink, ticket);
            }
        });
        runner
    }

    /// Queue a run of the given study. Fails fast if the study doesn't
    /// exist or its kind isn't runnable yet — a job that can only fail
    /// should never enter the queue.
    pub fn enqueue(&self, study_id: StudyId) -> Result<JobId, String> {
        let doc = lock(&self.doc);
        let study = doc
            .studies
            .iter()
            .find(|s| s.id == study_id)
            .ok_or_else(|| format!("no study {}", study_id.0))?;
        runnable(study)?;
        drop(doc);

        let mut next = lock(&self.next_id);
        let id = JobId(*next);
        *next += 1;
        drop(next);

        let cancel = Arc::new(AtomicBool::new(false));
        lock(&self.jobs).insert(
            id.0,
            JobRecord {
                study_id,
                status: JobStatus::Queued,
                error: None,
                cancel: cancel.clone(),
            },
        );
        self.queue
            .send(Ticket {
                id,
                study_id,
                cancel,
            })
            .map_err(|_| "job worker has shut down".to_string())?;
        Ok(id)
    }

    /// Request cooperative cancellation. Queued jobs never start; running
    /// jobs stop at the next flight boundary. Finished jobs ignore it.
    pub fn cancel(&self, job_id: JobId) -> Result<(), String> {
        let jobs = lock(&self.jobs);
        let record = jobs
            .get(&job_id.0)
            .ok_or_else(|| format!("no job {}", job_id.0))?;
        record.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn jobs(&self) -> Vec<JobView> {
        let mut views: Vec<JobView> = lock(&self.jobs)
            .iter()
            .map(|(id, r)| JobView {
                id: JobId(*id),
                study_id: r.study_id,
                status: r.status,
                error: r.error.clone(),
            })
            .collect();
        views.sort_by_key(|v| v.id.0);
        views
    }
}

fn runnable(study: &Study) -> Result<(), String> {
    match &study.kind {
        StudyKind::Dispersion { .. } => Ok(()),
        other => Err(format!(
            "study kind {} is not runnable yet",
            serde_json::to_value(other)
                .ok()
                .and_then(|v| v.get("kind").cloned())
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "unknown".into())
        )),
    }
}

/// The standard dispersion variation set for study jobs. Fixed by
/// convention for now — it is part of the hashed study config the moment
/// StudyKind grows per-study variations.
/// Synchronous study run for single-threaded clients (CLI, MCP): the same
/// snapshot → dispersion → `SetStudyResults` landing as the background
/// worker, without the queue. Deterministic: same document, same bytes.
pub fn run_study_now(doc: &mut Document, study_id: StudyId) -> Result<(), String> {
    let study = doc
        .studies
        .iter()
        .find(|s| s.id == study_id)
        .ok_or_else(|| format!("no study {}", study_id.0))?
        .clone();
    runnable(&study)?;
    let input_hash = study_input_hash(&doc.vehicle, &doc.design, &study);
    let StudyKind::Dispersion { flights } = study.kind else {
        return Err("study kind is not runnable yet".into());
    };
    let request = dispersion_request(study.seed, flights);
    let summary = dispersion_ipc::run_observed(&doc.vehicle, &doc.design, &request, |_, _| true)?
        .ok_or("run cancelled")?;
    let data = serde_json::to_value(&summary).map_err(|e| format!("summary serialize: {e}"))?;
    doc.dispatch(Command::SetStudyResults {
        id: study_id,
        results: Some(StudyResults { input_hash, data }),
    })
}

fn dispersion_request(seed: u64, flights: u32) -> DispersionRequest {
    DispersionRequest {
        seed,
        samples: flights,
        vary: vec![
            Variation {
                param: VaryParam::ThrustPct,
                sigma: 3.0,
            },
            Variation {
                param: VaryParam::WindSpeedMs,
                sigma: 1.5,
            },
        ],
        base_wind_ms: 3.0,
    }
}

fn run_one(
    doc: &Arc<Mutex<Document>>,
    jobs: &Arc<Mutex<HashMap<u64, JobRecord>>>,
    sink: &impl Fn(JobEvent),
    ticket: Ticket,
) {
    let set_status = |status: JobStatus, error: Option<String>| {
        if let Some(record) = lock(jobs).get_mut(&ticket.id.0) {
            record.status = status;
            record.error = error.clone();
        }
        sink(JobEvent::Done {
            job_id: ticket.id,
            study_id: ticket.study_id,
            status,
            error,
        });
    };

    if ticket.cancel.load(Ordering::Relaxed) {
        set_status(JobStatus::Cancelled, None);
        return;
    }
    if let Some(record) = lock(jobs).get_mut(&ticket.id.0) {
        record.status = JobStatus::Running;
    }

    // Snapshot the inputs once. The run computes from this snapshot; the
    // stamped hash describes it, so edits made mid-run correctly leave
    // the landed results marked stale.
    let (vehicle, design, study): (Vehicle, Design, Study) = {
        let doc = lock(doc);
        let Some(study) = doc.studies.iter().find(|s| s.id == ticket.study_id) else {
            drop(doc);
            set_status(JobStatus::Failed, Some("study was deleted".into()));
            return;
        };
        (doc.vehicle.clone(), doc.design.clone(), study.clone())
    };
    let input_hash = study_input_hash(&vehicle, &design, &study);

    let StudyKind::Dispersion { flights } = study.kind else {
        set_status(JobStatus::Failed, Some("study kind is not runnable yet".into()));
        return;
    };
    let request = dispersion_request(study.seed, flights);

    let cancel = ticket.cancel.clone();
    let outcome = dispersion_ipc::run_observed(&vehicle, &design, &request, |completed, total| {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        if completed % PROGRESS_STRIDE == 0 || completed == total {
            sink(JobEvent::Progress {
                job_id: ticket.id,
                study_id: ticket.study_id,
                completed,
                total,
            });
        }
        true
    });

    match outcome {
        Err(e) => set_status(JobStatus::Failed, Some(e)),
        Ok(None) => set_status(JobStatus::Cancelled, None),
        Ok(Some(summary)) => {
            let data = match serde_json::to_value(&summary) {
                Ok(v) => v,
                Err(e) => {
                    set_status(JobStatus::Failed, Some(format!("summary serialize: {e}")));
                    return;
                }
            };
            let landed = lock(doc).dispatch(Command::SetStudyResults {
                id: ticket.study_id,
                results: Some(StudyResults { input_hash, data }),
            });
            match landed {
                Ok(()) => set_status(JobStatus::Done, None),
                Err(e) => set_status(JobStatus::Failed, Some(e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    fn doc_with_dispersion_study(flights: u32, seed: u64) -> Arc<Mutex<Document>> {
        let mut doc = Document::default();
        doc.dispatch(Command::CreateStudy {
            name: "spread".into(),
            kind: StudyKind::Dispersion { flights },
            engine: "native".into(),
            seed,
        })
        .unwrap();
        Arc::new(Mutex::new(doc))
    }

    fn wait_done(events: &mpsc::Receiver<JobEvent>) -> (JobStatus, Option<String>, Vec<(u32, u32)>) {
        let mut progress = Vec::new();
        loop {
            match events
                .recv_timeout(Duration::from_secs(30))
                .expect("job must finish")
            {
                JobEvent::Progress { completed, total, .. } => progress.push((completed, total)),
                JobEvent::Done { status, error, .. } => return (status, error, progress),
            }
        }
    }

    #[test]
    fn thousand_flight_dispersion_runs_in_background_with_progress() {
        let doc = doc_with_dispersion_study(1000, 42);
        let (tx, rx) = channel();
        let runner = JobRunner::new(doc.clone(), move |e| {
            let _ = tx.send(e);
        });
        let job = runner.enqueue(StudyId(1)).unwrap();

        // The IPC thread stays free while the job runs: the document
        // lock is held only for snapshot and landing, so this acquires
        // immediately instead of blocking behind 1000 flights.
        drop(doc.lock().unwrap());

        let (status, error, progress) = wait_done(&rx);
        assert_eq!(status, JobStatus::Done, "{error:?}");
        assert!(
            progress.len() >= 20,
            "1000 flights at stride 50 must report ≥20 progress points, got {}",
            progress.len()
        );
        assert_eq!(progress.last(), Some(&(1000, 1000)));
        let increasing = progress.windows(2).all(|w| w[0].0 < w[1].0);
        assert!(increasing, "progress must be monotonic: {progress:?}");

        // Completion landed through the journal with the snapshot's hash.
        let doc = doc.lock().unwrap();
        let study = &doc.studies[0];
        let results = study.results.as_ref().expect("results landed");
        assert_eq!(
            results.input_hash,
            study_input_hash(&doc.vehicle, &doc.design, study)
        );
        assert!(!study.is_stale(&doc.vehicle, &doc.design));
        assert_eq!(results.data["samples"], 1000);
        assert_eq!(runner.jobs()[0].status, JobStatus::Done);
        assert_eq!(job, JobId(1));
    }

    #[test]
    fn cancel_leaves_the_study_untouched() {
        let doc = doc_with_dispersion_study(2000, 7);
        let (tx, rx) = channel();
        let runner = JobRunner::new(doc.clone(), move |e| {
            let _ = tx.send(e);
        });
        let job = runner.enqueue(StudyId(1)).unwrap();

        // Cancel on the first progress report — the worker checks the
        // flag at every flight boundary after that.
        let mut progress_seen = 0;
        let (status, _, _) = loop {
            match rx
                .recv_timeout(Duration::from_secs(30))
                .expect("job must finish")
            {
                JobEvent::Progress { .. } => {
                    progress_seen += 1;
                    runner.cancel(job).unwrap();
                }
                JobEvent::Done { status, error, .. } => break (status, error, ()),
            }
        };
        assert!(progress_seen >= 1);
        assert_eq!(status, JobStatus::Cancelled);

        let doc = doc.lock().unwrap();
        assert!(
            doc.studies[0].results.is_none(),
            "a cancelled job must dispatch nothing"
        );
        assert!(!doc.state().can_redo);
        drop(doc);
        assert_eq!(runner.jobs()[0].status, JobStatus::Cancelled);
    }

    #[test]
    fn same_seed_is_byte_identical_through_the_job_path() {
        let run_once = || {
            let doc = doc_with_dispersion_study(200, 99);
            let (tx, rx) = channel();
            let runner = JobRunner::new(doc.clone(), move |e| {
                let _ = tx.send(e);
            });
            runner.enqueue(StudyId(1)).unwrap();
            let (status, error, _) = wait_done(&rx);
            assert_eq!(status, JobStatus::Done, "{error:?}");
            let doc = doc.lock().unwrap();
            serde_json::to_string(&doc.studies[0].results).unwrap()
        };
        assert_eq!(run_once(), run_once());
    }

    #[test]
    fn unrunnable_and_missing_studies_are_rejected_at_enqueue() {
        let doc = Arc::new(Mutex::new(Document::default()));
        doc.lock()
            .unwrap()
            .dispatch(Command::CreateStudy {
                name: "single".into(),
                kind: StudyKind::SingleFlight,
                engine: "native".into(),
                seed: 0,
            })
            .unwrap();
        let runner = JobRunner::new(doc, |_| {});
        let err = runner.enqueue(StudyId(1)).unwrap_err();
        assert!(err.contains("not runnable"), "{err}");
        let err = runner.enqueue(StudyId(9)).unwrap_err();
        assert!(err.contains("no study"), "{err}");
        assert!(runner.jobs().is_empty(), "rejected jobs never enter the queue");
    }
}
