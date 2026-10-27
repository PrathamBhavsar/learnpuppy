use super::*;
use crate::agy::parse_agy;
use lp_core::ports::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

#[derive(Default)]
struct Mem {
    cursors: Mutex<Vec<IngestCursor>>,
    sessions: Mutex<Vec<CodingSession>>,
}

impl TrackerStore for Mem {
    fn cursor(&self, key: &str) -> StoreResult<Option<IngestCursor>> {
        Ok(self
            .cursors
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.source_key == key)
            .cloned())
    }
    fn set_cursor(&self, cursor: &IngestCursor) -> StoreResult<()> {
        let mut cs = self.cursors.lock().unwrap();
        cs.retain(|c| c.source_key != cursor.source_key);
        cs.push(cursor.clone());
        Ok(())
    }
    fn upsert_session(&self, s: &CodingSession) -> StoreResult<()> {
        let mut ss = self.sessions.lock().unwrap();
        ss.retain(|x| x.id != s.id);
        ss.push(s.clone());
        Ok(())
    }
    fn session(&self, id: &str) -> StoreResult<Option<CodingSession>> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.id == id)
            .cloned())
    }
    fn upsert_decision(&self, _: &DecisionRecord) -> StoreResult<()> {
        Ok(())
    }
    fn decisions(&self) -> StoreResult<Vec<DecisionRecord>> {
        Ok(vec![])
    }
    fn upsert_book_chunk(&self, _: &BookChunk) -> StoreResult<()> {
        Ok(())
    }
    fn upsert_card(&self, _: &FlashCard) -> StoreResult<()> {
        Ok(())
    }
    fn cards(&self) -> StoreResult<Vec<FlashCard>> {
        Ok(vec![])
    }
    fn append_review(&self, _: &FsrsReviewLog) -> StoreResult<()> {
        Ok(())
    }
    fn reviews(&self) -> StoreResult<Vec<FsrsReviewLog>> {
        Ok(vec![])
    }
    fn append_sync(&self, _: &str, _: &serde_json::Value) -> StoreResult<()> {
        Ok(())
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

const T0: &str = "2026-05-01T10:00:00Z";
const T0_MS: i64 = 1_777_629_600_000;

fn claude_fixture(cwd: &str) -> String {
    format!(
        r#"{{"type":"user","cwd":"{cwd}","gitBranch":"main","timestamp":"{T0}","message":{{"content":"add a cursor store <system-reminder>secret ctx</system-reminder>"}}}}
{{"type":"assistant","timestamp":"{T0}","message":{{"content":[{{"type":"text","text":"Done."}},{{"type":"tool_use","name":"Edit","input":{{"file_path":"src/main.rs"}}}},{{"type":"tool_use","name":"Bash","input":{{"command":"curl -H 'Authorization: Bearer sk-secret' x"}}}}]}}}}
{{"type":"user","timestamp":"{T0}","message":{{"content":[{{"type":"tool_result","content":"ok"}}]}}}}
not json
"#
    )
}

fn codex_fixture(cwd: &str) -> String {
    format!(
        r#"{{"type":"session_meta","timestamp":"{T0}","payload":{{"id":"cx1","cwd":"{cwd}","git":{{"branch":"dev"}}}}}}
{{"type":"response_item","timestamp":"{T0}","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"<environment_context>x</environment_context>"}}]}}}}
{{"type":"response_item","timestamp":"{T0}","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"explain fsrs"}}]}}}}
{{"type":"response_item","timestamp":"{T0}","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"it schedules"}}]}}}}
"#
    )
}

fn agy_fixture(root: &Path, cwd: &str) {
    std::fs::create_dir_all(root).unwrap();
    let db = rusqlite::Connection::open(root.join("conversation_summaries.db")).unwrap();
    db.execute_batch(
        "CREATE TABLE conversation_summaries(conversation_id TEXT,title TEXT,preview TEXT,workspace_uris TEXT,last_modified_time TEXT)",
    )
    .unwrap();
    db.execute(
        "INSERT INTO conversation_summaries VALUES('c1','Snapshot','copy the wal',?1,?2)",
        [format!(r#"["file://{cwd}"]"#), T0.to_string()],
    )
    .unwrap();
    write(&root.join("brain/c1/plan.md"), "# plan\nsnapshot first");
}

struct Env {
    home: tempfile::TempDir,
    repo: PathBuf,
}

fn env() -> Env {
    let home = tempfile::tempdir().unwrap();
    let repo = home.path().join("code/tracker");
    std::fs::create_dir_all(&repo).unwrap();
    let cwd = repo.to_str().unwrap();
    write(
        &home.path().join(".claude/projects/p/s1.jsonl"),
        &claude_fixture(cwd),
    );
    write(
        &home
            .path()
            .join(".claude-accounts/work2/projects/p/s2.jsonl"),
        &claude_fixture(cwd),
    );
    write(
        &home.path().join(".claude/projects/p/subagents/s3.jsonl"),
        &claude_fixture(cwd),
    );
    write(
        &home.path().join(".claude/projects/p/outside.jsonl"),
        &claude_fixture("/elsewhere"),
    );
    write(
        &home.path().join(".codex/sessions/2026/05/01/rollout.jsonl"),
        &codex_fixture(cwd),
    );
    agy_fixture(&home.path().join(".gemini/antigravity-cli"), cwd);
    Env { home, repo }
}

#[test]
fn ingests_claude_multi_account_codex_and_agy() {
    let e = env();
    let store = Mem::default();
    let rep = ingest(
        &store,
        &Roots::from_home(e.home.path()),
        std::slice::from_ref(&e.repo),
        0,
    );
    assert_eq!(rep.errors, Vec::<String>::new());
    let mut ids: Vec<_> = store
        .sessions
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.id.to_string())
        .collect();
    ids.sort();
    // subagents/ and out-of-allowlist cwd are skipped
    assert_eq!(ids, ["agy:c1", "claude:s1", "claude:s2", "codex:cx1"]);

    let ss = store.sessions.lock().unwrap();
    let claude = ss.iter().find(|s| s.id == "claude:s1").unwrap();
    assert_eq!(claude.git_branch.as_deref(), Some("main"));
    assert_eq!(claude.started_at, T0_MS);
    assert_eq!(
        claude.transcript,
        "U: add a cursor store\nA: Done.\nT: [tool: Edit src/main.rs]\nT: [tool: Bash curl]\n"
    );
    let codex = ss.iter().find(|s| s.source == "codex").unwrap();
    assert_eq!(codex.git_branch.as_deref(), Some("dev"));
    assert_eq!(codex.transcript, "U: explain fsrs\nA: it schedules\n");
    let agy = ss.iter().find(|s| s.source == "agy").unwrap();
    assert_eq!(
        agy.transcript,
        "U: copy the wal\nA: plan.md: # plan\nsnapshot first\n"
    );
}

#[test]
fn unchanged_files_are_skipped_and_changed_ones_reingested() {
    let e = env();
    let store = Mem::default();
    let roots = Roots::from_home(e.home.path());
    let allow = [e.repo.clone()];
    assert_eq!(ingest(&store, &roots, &allow, 0).files, 5);
    let again = ingest(&store, &roots, &allow, 0);
    assert_eq!((again.files, again.sessions), (0, 0));

    let p = e
        .home
        .path()
        .join(".codex/sessions/2026/05/01/rollout.jsonl");
    let grown = format!("{}{}", codex_fixture(e.repo.to_str().unwrap()), "\n");
    write(&p, &grown);
    assert_eq!(ingest(&store, &roots, &allow, 0).files, 1);
}

#[test]
fn since_filters_old_sessions() {
    let e = env();
    let store = Mem::default();
    let rep = ingest(
        &store,
        &Roots::from_home(e.home.path()),
        std::slice::from_ref(&e.repo),
        T0_MS + 1,
    );
    assert_eq!(rep.sessions, 0);
}

fn git(repo: &Path, date: &str, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .status()
        .unwrap()
        .success();
    assert!(ok);
}

#[test]
fn commits_match_session_time_window() {
    let repo = tempfile::tempdir().unwrap();
    let r = repo.path();
    git(r, "2026-05-01T10:00:00Z", &["init", "-q"]);
    let commit = |file: &str, date: &str, msg: &str| {
        write(&r.join(file), "x\n");
        git(r, date, &["add", "."]);
        git(r, date, &["commit", "-q", "-m", msg]);
    };
    commit("early.txt", "2026-05-01T09:00:00Z", "too early");
    commit("in.txt", "2026-05-01T10:20:00Z", "in window");
    commit("late.txt", "2026-05-01T11:00:00Z", "too late");

    let session = CodingSession {
        id: "claude:s".into(),
        source: "claude".into(),
        cwd: r.join("sub").to_string_lossy().into_owned(),
        git_branch: None,
        started_at: T0_MS,
        transcript: String::new(),
    };
    let commits = commits_for_session(&session, T0_MS, &[r.to_path_buf()]);
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].subject, "in window");
    assert_eq!(commits[0].files, ["in.txt"]);
    assert!(commits[0].diff.contains("+x"));
    assert!(commits_for_session(&session, T0_MS, &[PathBuf::from("/nope")]).is_empty());
}

#[test]
fn secrets_never_reach_the_store() {
    let e = env();
    let cwd = e.repo.to_str().unwrap();
    let line = |kind: &str, text: &str| {
        format!(
            r#"{{"type":"{kind}","cwd":"{cwd}","timestamp":"{T0}","message":{{"content":"{text}"}}}}"#
        )
    };
    write(
        &e.home.path().join(".claude/projects/p/leaky.jsonl"),
        &format!(
            "{}\n{}\n",
            line("user", "use OPENAI_API_KEY=sk-live-abc12345 please"),
            line("user", "header was Authorization: Bearer tok_abcdef123456")
        ),
    );
    let store = Mem::default();
    ingest(
        &store,
        &Roots::from_home(e.home.path()),
        std::slice::from_ref(&e.repo),
        0,
    );
    let ss = store.sessions.lock().unwrap();
    let s = ss.iter().find(|s| s.id == "claude:leaky").unwrap();
    assert_eq!(
        s.transcript,
        "U: use OPENAI_API_KEY=[REDACTED] please\nU: header was Authorization: Bearer [REDACTED]\n"
    );
}

#[test]
fn commit_diffs_are_redacted() {
    let repo = tempfile::tempdir().unwrap();
    let r = repo.path();
    git(r, "2026-05-01T10:00:00Z", &["init", "-q"]);
    write(&r.join(".env"), "API_TOKEN=abc123secret\n");
    git(r, "2026-05-01T10:20:00Z", &["add", "."]);
    git(
        r,
        "2026-05-01T10:20:00Z",
        &["commit", "-q", "-m", "add env"],
    );
    let session = CodingSession {
        id: "claude:s".into(),
        source: "claude".into(),
        cwd: r.to_string_lossy().into_owned(),
        git_branch: None,
        started_at: T0_MS,
        transcript: String::new(),
    };
    let commits = commits_for_session(&session, T0_MS, &[r.to_path_buf()]);
    assert!(commits[0].diff.contains("API_TOKEN=[REDACTED]"));
    assert!(!commits[0].diff.contains("abc123secret"));
}

#[test]
fn widening_since_or_the_allowlist_rereads_skipped_sessions() {
    let e = env();
    let store = Mem::default();
    let roots = Roots::from_home(e.home.path());
    let allow = [e.repo.clone()];
    assert_eq!(ingest(&store, &roots, &allow, T0_MS + 1).sessions, 0);
    assert_eq!(ingest(&store, &roots, &allow, 0).sessions, 4);

    let other = Mem::default();
    assert_eq!(
        ingest(&other, &roots, &[PathBuf::from("/nope")], 0).sessions,
        0
    );
    assert_eq!(ingest(&other, &roots, &allow, 0).sessions, 4);
}

#[test]
fn since_spec_parses_without_panicking() {
    assert_eq!(
        since_ms("2d", 1_000_000_000),
        Ok(1_000_000_000 - 48 * 3_600_000)
    );
    assert!(since_ms("7天", 0).is_err());
    assert!(since_ms("", 0).is_err());
    assert!(since_ms("9223372036854775807w", 0).is_err());
}

#[test]
fn invalid_utf8_line_does_not_abort_the_transcript() {
    let e = env();
    let cwd = e.repo.to_str().unwrap();
    let mut bytes = claude_fixture(cwd).into_bytes();
    bytes.extend_from_slice(b"\n{\"type\":\"user\",\"x\":\"\xff\xfe\"}\n");
    let p = e.home.path().join(".claude/projects/p/bad.jsonl");
    std::fs::write(&p, bytes).unwrap();
    let store = Mem::default();
    let rep = ingest(
        &store,
        &Roots::from_home(e.home.path()),
        std::slice::from_ref(&e.repo),
        0,
    );
    assert_eq!(rep.errors, Vec::<String>::new());
    assert!(store
        .sessions
        .lock()
        .unwrap()
        .iter()
        .any(|s| s.id == "claude:bad"));
}

#[test]
fn agy_artifacts_are_redacted_before_clipping() {
    let e = env();
    let root = e.home.path().join(".gemini/antigravity-cli");
    // the key sits across the 1500-char clip boundary
    let md = format!("{}OPENAI_API_KEY=sk-abcdefghijklmnop", "a".repeat(1490));
    write(&root.join("brain/c1/plan.md"), &md);
    let s = &parse_agy(&root).unwrap()[0];
    assert!(!s.transcript.contains("sk-abc"), "{}", s.transcript);
}

#[test]
fn git_helpers_never_run_repo_configured_diff_programs() {
    let repo = tempfile::tempdir().unwrap();
    let r = repo.path();
    git(r, "2026-05-01T10:00:00Z", &["init", "-q"]);
    let marker = r.join("ran");
    let tool = r.join("ext.sh");
    write(&tool, &format!("#!/bin/sh\ntouch {}\n", marker.display()));
    git(
        r,
        "2026-05-01T10:00:00Z",
        &["config", "diff.x.textconv", tool.to_str().unwrap()],
    );
    git(
        r,
        "2026-05-01T10:00:00Z",
        &["config", "core.hooksPath", "/dev/null"],
    );
    Command::new("chmod").arg("+x").arg(&tool).status().unwrap();
    write(&r.join(".gitattributes"), "*.txt diff=x\n");
    write(&r.join("a.txt"), "x\n");
    git(r, "2026-05-01T10:20:00Z", &["add", "."]);
    git(r, "2026-05-01T10:20:00Z", &["commit", "-q", "-m", "one"]);
    write(&r.join("a.txt"), "y\n");
    git(r, "2026-05-01T10:21:00Z", &["commit", "-qam", "two"]);
    let s = CodingSession {
        id: "claude:s".into(),
        source: "claude".into(),
        cwd: r.to_string_lossy().into_owned(),
        git_branch: None,
        started_at: T0_MS,
        transcript: String::new(),
    };
    let commits = commits_for_session(&s, T0_MS, &[r.to_path_buf()]);
    assert_eq!(commits.len(), 2);
    assert!(!marker.exists());
}
