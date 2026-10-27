//! Async workbench sub-agents: "ack now, steer later". A tool call only queues a request; `launch`
//! starts it as a background job, and the wake pump delivers its result as a steer.

mod adapters;
mod agent;
mod jobs;
mod tools;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lp_cli_driver::engine::{LlmEngine, StreamDelta};
use lp_core::ports::{
    DeliveryStatus, JobKind, JobRecord, JobStatus, JobStore, StoreError, StoreResult,
};
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::loops::{run_turn, Session, TurnError, TurnOutcome};

pub use adapters::{HttpWeb, PdftotextPdf};
pub use agent::{run_submit_agent, AgentError, NoTools, Run, Tools};
pub use jobs::{readable, Hit, Pdf, Slider, TuiDemoSpec, Web};
pub use tools::{
    exec, job_id, GraphHandle, JobRequest, DEMO, LIST_PROPOSITIONS, LIST_SESSIONS, PDF, RESEARCH,
    VISUAL,
};

/// Process-wide cap on concurrent background model calls, so foreground turns never starve.
const BG_LLM_PERMITS: usize = 3;

#[derive(Debug, thiserror::Error)]
pub enum PumpError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Turn(#[from] TurnError),
}

pub struct Workbench<E, J> {
    engine: Arc<E>,
    jobs: Arc<J>,
    web: Arc<dyn Web>,
    pdf: Arc<dyn Pdf>,
    artifacts: PathBuf,
    bg_llm: Semaphore,
    running: Mutex<HashMap<String, CancellationToken>>,
}

impl<E: LlmEngine + 'static, J: JobStore + 'static> Workbench<E, J> {
    pub fn new(
        engine: Arc<E>,
        jobs: Arc<J>,
        web: Arc<dyn Web>,
        pdf: Arc<dyn Pdf>,
        artifacts: impl Into<PathBuf>,
    ) -> Arc<Self> {
        Arc::new(Self {
            engine,
            jobs,
            web,
            pdf,
            artifacts: artifacts.into(),
            bg_llm: Semaphore::new(BG_LLM_PERMITS),
            running: Mutex::default(),
        })
    }

    /// Start every job the finished turn requested. The tool already acked, so the model has moved on.
    pub fn launch(self: &Arc<Self>, s: &mut Session) -> StoreResult<Vec<JoinHandle<()>>> {
        let mut handles = Vec::new();
        for req in std::mem::take(&mut s.requests) {
            let id = job_id(&s.id, &req.call_id);
            let rec = JobRecord {
                id: id.as_str().into(),
                chat_id: s.id.as_str().into(),
                kind: req.kind,
                status: JobStatus::Running,
                delivery: DeliveryStatus::Pending,
                created_at: chrono::Utc::now().timestamp_millis(),
                result: None,
                usage: Vec::new(),
            };
            self.jobs.create_job(&rec)?;
            let cancel = CancellationToken::new();
            self.running.lock().insert(id, cancel.clone());
            let wb = self.clone();
            handles.push(tokio::spawn(
                async move { wb.run(rec, req.args, cancel).await },
            ));
        }
        Ok(handles)
    }

    /// Learner abort: the job stops and its result is never delivered.
    pub fn abort(&self, job_id: &str) {
        if let Some(cancel) = self.running.lock().get(job_id) {
            cancel.cancel();
        }
    }

    async fn run(&self, mut rec: JobRecord, args: Value, cancel: CancellationToken) {
        let run = Run {
            engine: self.engine.as_ref(),
            bg_llm: &self.bg_llm,
            cancel: &cancel,
        };
        let done = match rec.kind {
            JobKind::Research => jobs::research(&run, self.web.as_ref(), &args).await,
            JobKind::Pdf => jobs::pdf(&run, self.pdf.as_ref(), &args).await,
            JobKind::Visual => jobs::visual(&run, &args).await,
            JobKind::Demo => jobs::demo(&run, &args).await,
        };
        let result = match done {
            Ok(d) => self.write_artifact(&rec.id, d).await,
            Err(e) => Err(e),
        };
        (rec.status, rec.delivery) = if cancel.is_cancelled() {
            (JobStatus::Failed, DeliveryStatus::Claimed)
        } else {
            (JobStatus::Done, DeliveryStatus::Pending)
        };
        // A failure is still delivered, so the teacher can tell the learner.
        rec.result = Some(result.unwrap_or_else(|e| json!({ "error": e })));
        if let Err(e) = self.jobs.update_job(&rec) {
            tracing::warn!("job {} result not stored: {e}", rec.id);
        }
        self.running.lock().remove(rec.id.as_str());
    }

    async fn write_artifact(&self, id: &str, d: jobs::Done) -> Result<Value, String> {
        let (ext, body) = d.artifact;
        let path = self.artifacts.join(format!("{id}.{ext}"));
        write_atomic(&path, body)
            .await
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut result = d.result;
        result["artifact"] = json!(path);
        Ok(result)
    }

    /// Crash recovery: jobs left `Running` by a dead process become delivered errors.
    pub fn recover(&self) -> StoreResult<usize> {
        let jobs = self.jobs.interrupted_jobs()?;
        for mut rec in jobs.iter().cloned() {
            rec.status = JobStatus::Done;
            rec.delivery = DeliveryStatus::Pending;
            rec.result = Some(json!({ "error": "interrupted by a restart" }));
            self.jobs.update_job(&rec)?;
        }
        Ok(jobs.len())
    }

    /// Idle wake gate: while a question waits nothing is claimed; otherwise finished jobs are claimed
    /// and delivered as `steer_<jobId>`. True if the transcript changed.
    pub fn deliver(&self, s: &mut Session) -> StoreResult<bool> {
        if s.pending_question() {
            return Ok(false);
        }
        for job in self.jobs.claim_steers(&s.id)? {
            s.queue_steer(format!("steer_{}", job.id), steer_text(&job));
        }
        Ok(s.wake())
    }

    /// Deliver finished jobs and, if that woke the session, run the continuation turn.
    /// The caller holds `&mut Session`, so no turn is in flight.
    pub async fn pump(
        self: &Arc<Self>,
        root: &Path,
        s: &mut Session,
        events: &mpsc::Sender<StreamDelta>,
        cancel: &CancellationToken,
    ) -> Result<Option<TurnOutcome>, PumpError> {
        if !self.deliver(s)? {
            return Ok(None);
        }
        let out = run_turn(self.engine.as_ref(), root, s, events, cancel).await?;
        self.launch(s)?;
        Ok(Some(out))
    }
}

fn steer_text(job: &JobRecord) -> String {
    let result = job.result.clone().unwrap_or(Value::Null);
    format!(
        "Background {:?} job {} finished: {result}",
        job.kind, job.id
    )
}

/// Write to a sibling temp file, then rename over the target.
async fn write_atomic(path: &Path, body: String) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    tokio::fs::write(&tmp, body).await?;
    tokio::fs::rename(&tmp, path).await
}
