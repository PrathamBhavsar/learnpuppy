//! Git commits inside a session's time window, for allowlisted repos.
use lp_core::ports::CodingSession;
use std::path::{Path, PathBuf};
use std::process::Command;

const BEFORE_MS: i64 = 5 * 60_000;
const AFTER_MS: i64 = 30 * 60_000;
const DIFF_CAP: usize = 6000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitCommit {
    pub sha: String,
    pub subject: String,
    pub body: String,
    pub files: Vec<String>,
    /// Unified diff capped at 6,000 chars.
    pub diff: String,
}

fn git(repo: &Path, args: &[&str]) -> String {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Commits of the allowlisted repo containing the session's `cwd`, from 5 min before the
/// session start to 30 min after it. `end_ms` is the session's last activity.
pub fn commits_for_session(session: &CodingSession, end_ms: i64, allow: &[PathBuf]) -> Vec<GitCommit> {
    let Some(repo) = allow.iter().find(|a| Path::new(&session.cwd).starts_with(a)) else {
        return vec![];
    };
    let since = format!("--since=@{}", (session.started_at - BEFORE_MS) / 1000);
    let until = format!("--until=@{}", (end_ms.max(session.started_at) + AFTER_MS) / 1000);
    let shas = git(repo, &["log", &since, &until, "--no-merges", "--format=%H"]);
    shas.lines()
        .map(|sha| {
            let meta = git(repo, &["show", "-s", "--format=%s%x1f%b", "--end-of-options", sha]);
            let (subject, body) = meta.split_once('\x1f').unwrap_or((&meta, ""));
            let files = git(repo, &["show", "--format=", "--name-only", "--end-of-options", sha]);
            let diff = git(repo, &["show", "--format=", "--unified=2", "--no-color", "--end-of-options", sha]);
            GitCommit {
                sha: sha.into(),
                subject: subject.trim().into(),
                body: body.trim().into(),
                files: files.lines().map(Into::into).collect(),
                diff: diff.chars().take(DIFF_CAP).collect(),
            }
        })
        .collect()
}
