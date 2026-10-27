//! Git commits inside a session's time window, for allowlisted repos.
use super::redact::redact;
use lp_core::ports::CodingSession;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

const BEFORE_MS: i64 = 5 * 60_000;
const AFTER_MS: i64 = 30 * 60_000;
const DIFF_CAP: usize = 6000;
/// Per-invocation cap so a hung `git` cannot stall ingest.
const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_COMMITS: &str = "--max-count=50";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitCommit {
    pub sha: String,
    pub subject: String,
    pub body: String,
    pub files: Vec<String>,
    /// Unified diff capped at 6,000 chars.
    pub diff: String,
}

async fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let run = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(GIT_TIMEOUT, run)
        .await
        .map_err(|_| format!("git {} timed out", args[0]))?
        .map_err(|e| format!("git {}: {e}", args[0]))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git {} failed: {}", args[0], err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Commits of the allowlisted repo containing the session's `cwd`, from 5 min before the
/// session start to 30 min after it. `end_ms` is the session's last activity.
pub async fn commits_for_session(
    session: &CodingSession,
    end_ms: i64,
    allow: &[PathBuf],
) -> Result<Vec<GitCommit>, String> {
    let Some(repo) = allow
        .iter()
        .find(|a| Path::new(&session.cwd).starts_with(a))
    else {
        return Ok(vec![]);
    };
    let since = format!("--since=@{}", (session.started_at - BEFORE_MS) / 1000);
    let until = format!(
        "--until=@{}",
        (end_ms.max(session.started_at) + AFTER_MS) / 1000
    );
    let shas = git(
        repo,
        &[
            "log",
            &since,
            &until,
            "--no-merges",
            MAX_COMMITS,
            "--format=%H",
        ],
    )
    .await?;
    let mut commits = vec![];
    for sha in shas.lines() {
        let meta = git(
            repo,
            &["show", "-s", "--format=%s%x1f%b", "--end-of-options", sha],
        )
        .await?;
        let (subject, body) = meta.split_once('\x1f').unwrap_or((&meta, ""));
        let files = git(
            repo,
            &["show", "--format=", "--name-only", "--end-of-options", sha],
        )
        .await?;
        let diff = git(
            repo,
            &[
                "show",
                "--format=",
                "--unified=2",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--end-of-options",
                sha,
            ],
        )
        .await?;
        commits.push(GitCommit {
            sha: sha.into(),
            subject: redact(subject.trim()),
            body: redact(body.trim()),
            files: files.lines().map(Into::into).collect(),
            diff: redact(&diff).chars().take(DIFF_CAP).collect(),
        });
    }
    Ok(commits)
}
