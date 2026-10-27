//! Claude and Codex JSONL transcript parsing into `CodingSession`s.
use chrono::DateTime;
use lp_core::ports::CodingSession;
use serde_json::Value;
use std::io::BufRead;
use std::path::{Path, PathBuf};

const JUNK: [&str; 9] = [
    "<local-command",
    "<command-",
    "<user-prompt",
    "<environment_context",
    "<permissions",
    "<instructions",
    "<skill",
    "# agents.md",
    "<system-reminder",
];

/// Drops `<system-reminder>…</system-reminder>` blocks (an unterminated one eats the rest).
fn strip_reminders(s: &str) -> String {
    let (mut out, mut rest) = (String::new(), s);
    while let Some(i) = rest.find("<system-reminder>") {
        out.push_str(&rest[..i]);
        match rest[i..].find("</system-reminder>") {
            Some(j) => rest = &rest[i + j + "</system-reminder>".len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| matches!(b["type"].as_str(), Some("text" | "input_text" | "output_text")))
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Accumulates `U:`/`A:`/`T:` lines; empty and boilerplate messages are dropped.
#[derive(Default)]
struct Lines {
    out: String,
    first_ts: Option<i64>,
}

impl Lines {
    fn add(&mut self, ts: Option<i64>, kind: char, raw: &str, cap: usize) {
        let text = strip_reminders(raw).split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() || JUNK.iter().any(|j| text.to_lowercase().starts_with(j)) {
            return;
        }
        if self.first_ts.is_none() {
            self.first_ts = ts;
        }
        self.out.push_str(&format!("{kind}: {}\n", clip(&text, cap)));
    }
}

fn ts_ms(v: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(v.as_str()?).ok().map(|d| d.timestamp_millis())
}

fn for_each_line(path: &Path, mut f: impl FnMut(Value)) -> std::io::Result<()> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut buf = String::new();
    loop {
        buf.clear();
        if reader.read_line(&mut buf)? == 0 {
            return Ok(());
        }
        if let Ok(v) = serde_json::from_str(&buf) {
            f(v);
        }
    }
}

fn finish(source: &str, id: String, cwd: String, branch: Option<String>, lines: Lines) -> Option<CodingSession> {
    Some(CodingSession {
        id: format!("{source}:{id}").into(),
        source: source.into(),
        cwd,
        git_branch: branch,
        started_at: lines.first_ts?,
        transcript: lines.out,
    })
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub(crate) fn parse_claude(path: &Path) -> std::io::Result<Option<CodingSession>> {
    let (mut cwd, mut branch, mut lines) = (String::new(), None, Lines::default());
    for_each_line(path, |d| {
        let kind = d["type"].as_str().unwrap_or("");
        if kind != "user" && kind != "assistant" {
            return;
        }
        if cwd.is_empty() {
            cwd = d["cwd"].as_str().unwrap_or("").into();
            branch = d["gitBranch"].as_str().map(Into::into);
        }
        let ts = ts_ms(&d["timestamp"]);
        let content = &d["message"]["content"];
        if kind == "user" {
            lines.add(ts, 'U', &text_of(content), 500);
            return;
        }
        for b in content.as_array().into_iter().flatten() {
            match b["type"].as_str() {
                Some("text") => lines.add(ts, 'A', b["text"].as_str().unwrap_or(""), 420),
                Some("tool_use") => {
                    let name = b["name"].as_str().unwrap_or("");
                    let target = b["input"]["file_path"].as_str().or_else(|| b["input"]["command"].as_str());
                    if let Some(t) = target {
                        lines.add(ts, 'T', &format!("[tool: {name} {t}]"), 200);
                    }
                }
                _ => {}
            }
        }
    })?;
    Ok(finish("claude", stem(path), cwd, branch, lines))
}

pub(crate) fn parse_codex(path: &Path) -> std::io::Result<Option<CodingSession>> {
    let (mut id, mut cwd, mut branch, mut lines) = (stem(path), String::new(), None, Lines::default());
    for_each_line(path, |d| {
        let p = &d["payload"];
        let ts = ts_ms(&d["timestamp"]);
        match (d["type"].as_str(), p["type"].as_str()) {
            (Some("session_meta"), _) => {
                id = p["id"].as_str().map_or(id.clone(), Into::into);
                cwd = p["cwd"].as_str().unwrap_or("").into();
                branch = p["git"]["branch"].as_str().map(Into::into);
            }
            (Some("response_item"), Some("message")) => match p["role"].as_str() {
                Some("user") => lines.add(ts, 'U', &text_of(&p["content"]), 500),
                Some("assistant") => lines.add(ts, 'A', &text_of(&p["content"]), 420),
                _ => {}
            },
            _ => {}
        }
    })?;
    Ok(finish("codex", id, cwd, branch, lines))
}

/// All `*.jsonl` under `dir`, skipping `subagents/` directories.
pub(crate) fn jsonl_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                if e.file_name() != "subagents" {
                    stack.push(p);
                }
            } else if p.extension().is_some_and(|x| x == "jsonl") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}
