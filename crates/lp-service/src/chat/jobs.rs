//! The job store a workspace's workbench writes through: every create and update is also reported
//! (for `job.updated` events and the wake pump), and each job carries a title taken from the tool
//! call that asked for it.

use std::collections::HashMap;
use std::sync::Arc;

use lp_agent::loops::{Msg, Session};
use lp_agent::workbench::job_id;
use lp_core::ports::{JobKind, JobRecord, JobStore, StoreResult};
use parking_lot::Mutex;
use serde_json::{json, Value};

const TITLE_CHARS: usize = 80;

/// Wire name of a job kind.
pub fn kind_name(k: JobKind) -> &'static str {
    match k {
        JobKind::Research => "research",
        JobKind::Pdf => "pdf",
        JobKind::Visual => "visual",
        JobKind::Demo => "demo",
        JobKind::Textbook => "textbook",
    }
}

/// The title a job shows: its stored title, else "Research job" etc.
pub fn job_title(j: &JobRecord) -> String {
    j.result
        .as_ref()
        .and_then(|r| r.get("title"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let k = kind_name(j.kind);
            format!("{}{} job", k[..1].to_uppercase(), &k[1..])
        })
}

/// The artifact a job produced: an explicit `artifactId`, else the file stem of `artifact`.
pub fn job_artifact(j: &JobRecord) -> Option<String> {
    let r = j.result.as_ref()?;
    if let Some(id) = r.get("artifactId").and_then(Value::as_str) {
        return Some(id.to_owned());
    }
    let path = r.get("artifact").and_then(Value::as_str)?;
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
}

/// A one-line title from a tool call's brief.
pub fn title_of(kind: JobKind, args: &Value) -> String {
    let key = match kind {
        JobKind::Research | JobKind::Pdf => "question",
        JobKind::Visual | JobKind::Demo => "description",
        JobKind::Textbook => "title",
    };
    let brief = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if brief.is_empty() {
        return job_title(&JobRecord {
            id: "".into(),
            chat_id: "".into(),
            kind,
            status: lp_core::ports::JobStatus::Running,
            delivery: lp_core::ports::DeliveryStatus::Pending,
            created_at: 0,
            result: None,
            usage: vec![],
        });
    }
    if brief.chars().count() <= TITLE_CHARS {
        return brief;
    }
    let cut: String = brief.chars().take(TITLE_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

fn tool_kind(name: &str) -> Option<JobKind> {
    Some(match name {
        "research" => JobKind::Research,
        "pdf" => JobKind::Pdf,
        "visual" => JobKind::Visual,
        "demo" => JobKind::Demo,
        _ => return None,
    })
}

/// Wraps a workspace's job store.
pub struct ObservedJobs<J> {
    inner: Arc<J>,
    titles: Mutex<HashMap<String, String>>,
    notify: Arc<dyn Fn(JobRecord) + Send + Sync>,
}

impl<J: JobStore> ObservedJobs<J> {
    pub fn new(inner: Arc<J>, notify: Arc<dyn Fn(JobRecord) + Send + Sync>) -> Self {
        Self {
            inner,
            titles: Mutex::default(),
            notify,
        }
    }

    /// Titles for the jobs `s` is about to launch, from their tool calls.
    pub fn name_requests(&self, s: &Session) {
        let ids = s.requests_ids();
        if ids.is_empty() {
            return;
        }
        let mut titles = self.titles.lock();
        for m in &s.messages {
            let Msg::Assistant { calls, .. } = m else {
                continue;
            };
            for c in calls {
                let Some(kind) = tool_kind(&c.name) else {
                    continue;
                };
                let id = job_id(&s.id, &c.id);
                if ids.contains(&id) {
                    titles.insert(id, title_of(kind, &c.args));
                }
            }
        }
    }

    fn titled(&self, job: &JobRecord) -> JobRecord {
        let mut job = job.clone();
        if let Some(t) = self.titles.lock().get(job.id.as_str()) {
            let mut r = job.result.take().unwrap_or_else(|| json!({}));
            if let Value::Object(m) = &mut r {
                m.entry("title").or_insert_with(|| Value::String(t.clone()));
            }
            job.result = Some(r);
        }
        job
    }
}

impl<J: JobStore> JobStore for ObservedJobs<J> {
    fn create_job(&self, job: &JobRecord) -> StoreResult<()> {
        let job = self.titled(job);
        self.inner.create_job(&job)?;
        (self.notify)(job);
        Ok(())
    }

    fn update_job(&self, job: &JobRecord) -> StoreResult<()> {
        let job = self.titled(job);
        self.inner.update_job(&job)?;
        (self.notify)(job);
        Ok(())
    }

    fn claim_steers(&self, chat_id: &str) -> StoreResult<Vec<JobRecord>> {
        self.inner.claim_steers(chat_id)
    }

    fn interrupted_jobs(&self) -> StoreResult<Vec<JobRecord>> {
        self.inner.interrupted_jobs()
    }

    fn claimed_jobs(&self) -> StoreResult<Vec<JobRecord>> {
        self.inner.claimed_jobs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_core::ports::{DeliveryStatus, JobStatus};

    fn rec(result: Option<Value>) -> JobRecord {
        JobRecord {
            id: "job_1".into(),
            chat_id: "c".into(),
            kind: JobKind::Visual,
            status: JobStatus::Done,
            delivery: DeliveryStatus::Pending,
            created_at: 1,
            result,
            usage: vec![],
        }
    }

    #[test]
    fn titles_and_artifacts_read_from_results() {
        assert_eq!(job_title(&rec(None)), "Visual job");
        assert_eq!(job_title(&rec(Some(json!({"title": "Poll"})))), "Poll");
        assert_eq!(
            job_artifact(&rec(Some(json!({"artifact": "/x/artifacts/job_1.svg"})))).as_deref(),
            Some("job_1")
        );
        assert_eq!(
            job_artifact(&rec(Some(json!({"artifactId": "art"})))).as_deref(),
            Some("art")
        );
        assert_eq!(job_artifact(&rec(Some(json!({"error": "x"})))), None);
        let long = "word ".repeat(40);
        let t = title_of(JobKind::Research, &json!({ "question": long }));
        assert!(t.ends_with('…') && t.chars().count() <= TITLE_CHARS, "{t}");
        assert_eq!(title_of(JobKind::Demo, &json!({})), "Demo job");
    }
}
