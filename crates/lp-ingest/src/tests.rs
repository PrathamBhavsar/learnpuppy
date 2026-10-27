use super::*;
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
        Ok(self.cursors.lock().unwrap().iter().find(|c| c.source_key == key).cloned())
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
    fn upsert_decision(&self, _: &DecisionRecord) -> StoreResult<()> { Ok(()) }
    fn decisions(&self) -> StoreResult<Vec<DecisionRecord>> { Ok(vec![]) }
    fn upsert_book_chunk(&self, _: &BookChunk) -> StoreResult<()> { Ok(()) }
    fn upsert_card(&self, _: &FlashCard) -> StoreResult<()> { Ok(()) }
    fn cards(&self) -> StoreResult<Vec<FlashCard>> { Ok(vec![]) }
    fn append_review(&self, _: &FsrsReviewLog) -> StoreResult<()> { Ok(()) }
    fn reviews(&self) -> StoreResult<Vec<FsrsReviewLog>> { Ok(vec![]) }
    fn append_sync(&self, _: &str, _: &serde_json::Value) -> StoreResult<()> { Ok(()) }
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
    write(&home.path().join(".claude/projects/p/s1.jsonl"), &claude_fixture(cwd));
    write(&home.path().join(".claude-accounts/work2/projects/p/s2.jsonl"), &claude_fixture(cwd));
    write(&home.path().join(".claude/projects/p/subagents/s3.jsonl"), &claude_fixture(cwd));
    write(&home.path().join(".claude/projects/p/outside.jsonl"), &claude_fixture("/elsewhere"));
    write(&home.path().join(".codex/sessions/2026/05/01/rollout.jsonl"), &codex_fixture(cwd));
    agy_fixture(&home.path().join(".gemini/antigravity-cli"), cwd);
    Env { home, repo }
}

#[test]
fn ingests_claude_multi_account_codex_and_agy() {
    let e = env();
    let store = Mem::default();
    let rep = ingest(&store, &Roots::from_home(e.home.path()), std::slice::from_ref(&e.repo), 0);
    assert_eq!(rep.errors, Vec::<String>::new());
    let mut ids: Vec<_> = store.sessions.lock().unwrap().iter().map(|s| s.id.to_string()).collect();
    ids.sort();
    // subagents/ and out-of-allowlist cwd are skipped
    assert_eq!(ids, ["agy:c1", "claude:s1", "claude:s2", "codex:cx1"]);

    let ss = store.sessions.lock().unwrap();
    let claude = ss.iter().find(|s| s.id == "claude:s1").unwrap();
    assert_eq!(claude.git_branch.as_deref(), Some("main"));
    assert_eq!(claude.started_at, T0_MS);
    assert_eq!(claude.transcript, "U: add a cursor store\nA: Done.\nT: [tool: Edit src/main.rs]\nT: [tool: Bash curl]\n");
    let codex = ss.iter().find(|s| s.source == "codex").unwrap();
    assert_eq!(codex.git_branch.as_deref(), Some("dev"));
    assert_eq!(codex.transcript, "U: explain fsrs\nA: it schedules\n");
    let agy = ss.iter().find(|s| s.source == "agy").unwrap();
    assert_eq!(agy.transcript, "U: copy the wal\nA: plan.md: # plan\nsnapshot first\n");
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

    let p = e.home.path().join(".codex/sessions/2026/05/01/rollout.jsonl");
    let grown = format!("{}{}", codex_fixture(e.repo.to_str().unwrap()), "\n");
    write(&p, &grown);
    assert_eq!(ingest(&store, &roots, &allow, 0).files, 1);
}

#[test]
fn since_filters_old_sessions() {
    let e = env();
    let store = Mem::default();
    let rep = ingest(&store, &Roots::from_home(e.home.path()), std::slice::from_ref(&e.repo), T0_MS + 1);
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
