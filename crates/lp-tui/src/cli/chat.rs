//! `learnpuppy chat new | step | show | jobs`.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context};
use clap::{Args, Subcommand};
use lp_agent::loops::{Kind, Msg, Session};
use lp_agent::workbench::{HttpWeb, PdftotextPdf, Web, Workbench};
use lp_cli_driver::engine::LlmEngine;
use lp_core::plan::Phase;
use lp_core::ports::{ChatStore, JobKind, JobRecord, JobStatus};
use lp_store::FsJobStore;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::session::{classroom_view, pending_call, render_answer, run_to_rest};
use crate::state::now_ms;
use crate::views::classroom::{Answer, Prompt};

#[derive(Subcommand)]
pub enum Cmd {
    /// Start a session on a topic and run its first turn.
    New {
        topic: String,
        #[command(flatten)]
        mode: Mode,
        #[command(flatten)]
        wait: Wait,
    },
    /// Answer the pending question of a session and run the next turn.
    /// A session whose last turn failed re-runs that turn instead (the answer is then unused).
    Step {
        id: String,
        answer: String,
        /// Confidence 1..5.
        #[arg(long, default_value_t = 3)]
        confidence: u8,
        #[command(flatten)]
        wait: Wait,
    },
    /// Phase, plan, pending question, jobs and the last assistant text of a session.
    Show { id: String },
    /// Background jobs (research, visual, demo, pdf) of a session.
    Jobs { id: String },
}

#[derive(Args)]
pub struct Mode {
    #[arg(long, conflicts_with = "practice")]
    teach: bool,
    #[arg(long)]
    practice: bool,
}

#[derive(Args)]
pub struct Wait {
    /// Seconds to wait for the background jobs a turn requested; 0 leaves them queued, not started.
    #[arg(long = "wait-jobs", value_name = "SECONDS", default_value_t = 480)]
    wait_jobs: u64,
}

const STREAM_DELTA_CAP: usize = 512;

/// Production wiring: real web and PDF readers, artifacts under the state dir.
pub async fn run<E: LlmEngine + 'static>(
    chats: &dyn ChatStore,
    jobs: Arc<FsJobStore>,
    engine_for: &dyn Fn(&str) -> Arc<E>,
    state: &Path,
    root: &Path,
    cmd: Cmd,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let wb = Workbench::routed(
        |r| engine_for(r),
        jobs.clone(),
        Arc::new(HttpWeb::default()) as Arc<dyn Web>,
        Arc::new(PdftotextPdf),
        state.join("artifacts"),
    );
    run_with(chats, &jobs, &wb, engine_for, root, cmd, out).await
}

async fn run_with<E: LlmEngine + 'static>(
    chats: &dyn ChatStore,
    jobs: &FsJobStore,
    wb: &Arc<Workbench<E, FsJobStore>>,
    engine_for: &dyn Fn(&str) -> Arc<E>,
    root: &Path,
    cmd: Cmd,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let wait = match &cmd {
        Cmd::Show { id } => {
            let s = Session::load(chats, id).with_context(|| format!("chat {id}"))?;
            let title = chats.load_chat(id)?.title;
            return show(&s, &title, &chat_jobs(jobs, id)?, out);
        }
        Cmd::Jobs { id } => {
            Session::load(chats, id).with_context(|| format!("chat {id}"))?;
            return print_jobs(&chat_jobs(jobs, id)?, out);
        }
        Cmd::New { wait, .. } | Cmd::Step { wait, .. } => wait.wait_jobs,
    };
    let mut s = match &cmd {
        Cmd::New { topic, mode, .. } => {
            let kind = if mode.practice {
                Kind::Practice
            } else {
                Kind::Teach
            };
            let mut s = Session::new(format!("c_{:x}", now_ms()), kind);
            s.push_user(topic);
            s.save(chats, now_ms() / 1000)?;
            let mut rec = chats.load_chat(&s.id)?;
            rec.title.clone_from(topic);
            chats.save_chat(&rec)?;
            s
        }
        Cmd::Step {
            id,
            answer,
            confidence,
            ..
        } => {
            let mut s = Session::load(chats, id).with_context(|| format!("chat {id}"))?;
            if let Some(call_id) = pending_call(&s).map(|c| c.id.clone()) {
                let text = render_answer(&Answer {
                    value: String::new(),
                    confidence: (*confidence).clamp(1, 5),
                    reading_thinking: String::new(),
                    thinking: answer.clone(),
                });
                s.answer(&call_id, text);
            } else if matches!(
                s.messages.last(),
                Some(Msg::User(_) | Msg::Answer { .. } | Msg::Steer { .. })
            ) {
                // The last turn failed before the model replied; run it again.
                writeln!(out, "re-running the last turn of chat {id}")?;
            } else {
                bail!("chat {id} has no pending question");
            }
            // Results of finished jobs reach the model with this turn.
            wb.deliver(&mut s)?;
            s
        }
        Cmd::Show { .. } | Cmd::Jobs { .. } => unreachable!("handled above"),
    };
    let (tx, mut rx) = mpsc::channel(STREAM_DELTA_CAP);
    // Deltas are not printed live; draining keeps the bounded channel from stalling the turn.
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let res = run_to_rest(engine_for, root, &mut s, &tx, &CancellationToken::new()).await;
    drop(tx);
    let _ = drain.await;
    let (mut handles, mut launched) = (Vec::new(), Vec::new());
    if res.is_ok() && wait > 0 {
        launched = s.requests_ids();
        handles = wb.launch(&mut s)?;
    }
    s.save(chats, now_ms() / 1000)?;
    if let Err(e) = res {
        bail!(
            "{e}\nchat {id} is saved; run `learnpuppy chat step {id} <answer>` to re-run the turn",
            id = s.id
        );
    }

    let view = classroom_view(&s, "");
    writeln!(out, "chat {}", s.id)?;
    write!(out, "phase {:?}", s.phase())?;
    if s.phase() == Phase::Probe {
        let max = lp_agent::loops::MAX_PROBE_QUESTIONS;
        write!(out, ", probe question {}/{max}", s.probe_questions())?;
    }
    writeln!(out)?;
    if let Some(last) = view.transcript.last() {
        writeln!(out, "{last}")?;
    }
    match view.prompt {
        Some(Prompt::Quiz { input, .. }) => {
            writeln!(out, "\n? {}", input.question)?;
            for o in input.options {
                writeln!(out, "  {} ({})", o.label, o.value)?;
            }
        }
        Some(Prompt::Free { question, .. }) => writeln!(out, "\n? {question}")?,
        None => {}
    }
    if wait == 0 && !s.requests_ids().is_empty() {
        writeln!(
            out,
            "\n{} job(s) queued, not started (--wait-jobs 0); the next chat step starts them",
            s.requests_ids().len()
        )?;
    }
    if !handles.is_empty() {
        let all = async {
            for h in handles {
                let _ = h.await;
            }
        };
        let done = tokio::time::timeout(Duration::from_secs(wait), all)
            .await
            .is_ok();
        let mut mine = chat_jobs(jobs, &s.id)?;
        mine.retain(|j| launched.iter().any(|id| id.as_str() == j.id.as_str()));
        writeln!(out)?;
        print_jobs(&mine, out)?;
        if !done {
            writeln!(
                out,
                "jobs still running after {wait}s; learnpuppy marks them interrupted on its next start"
            )?;
        }
    }
    Ok(())
}

fn chat_jobs(jobs: &FsJobStore, chat: &str) -> anyhow::Result<Vec<JobRecord>> {
    let (all, _) = jobs.all_report()?;
    Ok(all.into_iter().filter(|j| j.chat_id == chat).collect())
}

fn print_jobs(jobs: &[JobRecord], out: &mut dyn Write) -> anyhow::Result<()> {
    if jobs.is_empty() {
        writeln!(out, "no jobs")?;
    }
    for j in jobs {
        let result = j.result.clone().unwrap_or_default();
        let status = match (j.status, result.get("error").and_then(|e| e.as_str())) {
            (_, Some(e)) => format!("failed: {e}"),
            (JobStatus::Done, None) => "done".into(),
            (JobStatus::Failed, None) => "failed".into(),
            (JobStatus::Running, None) => "running".into(),
        };
        writeln!(out, "job {} {:?} {status}", j.id, j.kind)?;
        if let Some(path) = result.get("artifact").and_then(|a| a.as_str()) {
            writeln!(out, "  artifact {path}")?;
        }
        if let (JobKind::Visual, Some(ascii)) = (j.kind, result["ascii_fallback"].as_str()) {
            writeln!(out, "{ascii}")?;
        }
    }
    Ok(())
}

fn show(s: &Session, title: &str, jobs: &[JobRecord], out: &mut dyn Write) -> anyhow::Result<()> {
    let view = classroom_view(s, "");
    writeln!(out, "chat {} {:?} {title:?}", s.id, s.kind)?;
    write!(out, "phase {:?}", s.phase())?;
    if s.phase() == Phase::Probe {
        let max = lp_agent::loops::MAX_PROBE_QUESTIONS;
        write!(out, ", probe question {}/{max}", s.probe_questions())?;
    }
    writeln!(out)?;
    match &s.plan {
        None => writeln!(out, "plan: none yet")?,
        Some(plan) => {
            writeln!(out, "plan: {} nodes", plan.nodes.len())?;
            for n in &plan.nodes {
                let st = &view.statuses;
                let mark = if st.current.as_deref() == Some(n.id.as_str()) {
                    "current"
                } else if st.visited.contains(&n.id) {
                    "done"
                } else {
                    "todo"
                };
                writeln!(out, "  [{mark}] {} - {}", n.id, n.title)?;
            }
        }
    }
    match view.prompt {
        Some(Prompt::Quiz { input, .. }) => writeln!(out, "pending: {}", input.question)?,
        Some(Prompt::Free { question, .. }) => writeln!(out, "pending: {question}")?,
        None => writeln!(out, "pending: none")?,
    }
    print_jobs(jobs, out)?;
    let last = s.messages.iter().rev().find_map(|m| match m {
        Msg::Assistant { text, .. } if !text.trim().is_empty() => Some(text.as_str()),
        _ => None,
    });
    writeln!(out, "last assistant: {}", last.unwrap_or("(none)"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use lp_agent::workbench::{Hit, Pdf};
    use lp_cli_driver::engine::FauxEngine;
    use lp_store::FsChatStore;
    use serde_json::json;

    struct NoWeb;

    #[async_trait]
    impl Web for NoWeb {
        async fn search(&self, _: &str) -> Result<Vec<Hit>, String> {
            Ok(vec![])
        }
        async fn fetch(&self, _: &str) -> Result<String, String> {
            Err("offline".into())
        }
    }

    struct NoPdf;

    impl Pdf for NoPdf {
        fn pages(&self, _: &str) -> Result<Vec<String>, String> {
            Ok(vec![])
        }
    }

    struct Rig {
        dir: tempfile::TempDir,
        chats: FsChatStore,
        jobs: Arc<FsJobStore>,
        faux: Arc<FauxEngine>,
        wb: Arc<Workbench<FauxEngine, FsJobStore>>,
    }

    impl Rig {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let faux = Arc::new(FauxEngine::new());
            let jobs = Arc::new(FsJobStore::open(dir.path().join("jobs")).unwrap());
            let f = faux.clone();
            let wb = Workbench::routed(
                move |_| f.clone(),
                jobs.clone(),
                Arc::new(NoWeb),
                Arc::new(NoPdf),
                dir.path().join("artifacts"),
            );
            Self {
                chats: FsChatStore::open(dir.path()).unwrap(),
                dir,
                jobs,
                faux,
                wb,
            }
        }

        async fn go(&self, cmd: Cmd) -> anyhow::Result<String> {
            let faux = self.faux.clone();
            let engine_for = move |_: &str| faux.clone();
            let mut out = Vec::new();
            run_with(
                &self.chats,
                &self.jobs,
                &self.wb,
                &engine_for,
                self.dir.path(),
                cmd,
                &mut out,
            )
            .await?;
            Ok(String::from_utf8(out).unwrap())
        }

        fn id(&self) -> String {
            self.chats.list_chats().unwrap()[0].id.to_string()
        }
    }

    fn new_cmd(wait_jobs: u64) -> Cmd {
        Cmd::New {
            topic: "ownership".into(),
            mode: Mode {
                teach: true,
                practice: false,
            },
            wait: Wait { wait_jobs },
        }
    }

    fn step_cmd(id: &str, wait_jobs: u64) -> Cmd {
        Cmd::Step {
            id: id.into(),
            answer: "a".into(),
            confidence: 9,
            wait: Wait { wait_jobs },
        }
    }

    fn quiz() -> serde_json::Value {
        json!({
            "question": "Who owns it?",
            "options": [{"label": "One", "value": "a"}, {"label": "Two", "value": "b"}],
            "correctAnswer": "a",
            "explanation": "One owner."
        })
    }

    #[tokio::test]
    async fn new_then_step_persists_and_prints_the_question() {
        let r = Rig::new();
        r.faux.faux_tool_call("probe", quiz());
        let text = r.go(new_cmd(5)).await.unwrap();
        assert!(text.contains("? Who owns it?") && text.contains("One (a)"));
        assert!(text.contains("phase Probe, probe question 1/8"), "{text}");
        let id = r.id();
        assert_eq!(r.chats.load_chat(&id).unwrap().title, "ownership");

        r.faux.faux_text("Good.");
        r.go(step_cmd(&id, 5)).await.unwrap();
        let s = Session::load(&r.chats, &id).unwrap();
        assert!(pending_call(&s).is_none());
        let err = r.go(step_cmd(&id, 5)).await.unwrap_err();
        assert!(err.to_string().contains("no pending question"), "{err}");
    }

    #[tokio::test]
    async fn failed_turn_is_resumable_by_the_next_step() {
        let r = Rig::new();
        // Nothing scripted: the first turn fails and the chat keeps the learner's topic.
        let err = r.go(new_cmd(5)).await.unwrap_err().to_string();
        assert!(err.contains("chat step"), "{err}");
        let id = r.id();

        r.faux.faux_tool_call("probe", quiz());
        let text = r.go(step_cmd(&id, 5)).await.unwrap();
        assert!(
            text.contains("re-running") && text.contains("? Who owns it?"),
            "{text}"
        );

        // A failed turn after an answer is re-run, not refused as "no pending question".
        let err = r.go(step_cmd(&id, 5)).await.unwrap_err().to_string();
        assert!(err.contains("chat step"), "{err}");
        r.faux.faux_text("Good.");
        r.go(step_cmd(&id, 5)).await.unwrap();
    }

    #[tokio::test]
    async fn requested_job_is_launched_waited_for_and_printed() {
        let r = Rig::new();
        r.faux.faux_tool_call("research", json!({"question": "q"}));
        r.faux.faux_tool_call("probe", quiz());
        r.faux.faux_tool_call(
            "submit",
            json!({"brief": "b", "sources": [{"title": "t", "url": "https://example.test/x"}]}),
        );
        let text = r.go(new_cmd(30)).await.unwrap();
        assert!(text.contains("Research done"), "{text}");
        assert!(
            text.contains("artifact ") && text.contains("artifacts/job_"),
            "{text}"
        );
        let id = r.id();
        let jobs = r.go(Cmd::Jobs { id: id.clone() }).await.unwrap();
        assert!(jobs.contains("Research done"), "{jobs}");
        let shown = r.go(Cmd::Show { id }).await.unwrap();
        assert!(
            shown.contains("phase Probe") && shown.contains("pending: Who owns it?"),
            "{shown}"
        );
        assert!(
            shown.contains("plan: none yet") && shown.contains("Research done"),
            "{shown}"
        );
    }

    #[tokio::test]
    async fn wait_jobs_zero_leaves_requests_queued_until_a_later_step() {
        let r = Rig::new();
        r.faux.faux_tool_call("research", json!({"question": "q"}));
        r.faux.faux_tool_call("probe", quiz());
        let text = r.go(new_cmd(0)).await.unwrap();
        assert!(text.contains("1 job(s) queued, not started"), "{text}");
        let id = r.id();
        assert!(r.go(Cmd::Jobs { id }).await.unwrap().contains("no jobs"));
    }

    #[test]
    fn visual_job_prints_its_diagram_and_artifact() {
        let job = JobRecord {
            id: "job_1".into(),
            chat_id: "c".into(),
            kind: JobKind::Visual,
            status: JobStatus::Done,
            delivery: lp_core::ports::DeliveryStatus::Pending,
            created_at: 0,
            result: Some(json!({"ascii_fallback": "[a]-[b]", "artifact": "/x/job_1.svg"})),
            usage: vec![],
        };
        let mut out = Vec::new();
        print_jobs(&[job], &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("job job_1 Visual done") && text.contains("[a]-[b]"));
        assert!(text.contains("artifact /x/job_1.svg"), "{text}");
    }
}
