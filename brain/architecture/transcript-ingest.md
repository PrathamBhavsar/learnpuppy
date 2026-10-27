---
type: spec
title: Local CLI Transcript & Git Commit Ingestor (lp-ingest)
description: Incremental mtime+size ingestion of Claude (all accounts), Codex, and Antigravity sessions, Git commit matching, decision distillation, and MC card quality gates.
tags: [ingest, transcripts, claude, codex, agy, git, decisions, tracker]
timestamp: 2026-10-27
---

# Local CLI Transcript & Git Commit Ingestor (`lp-ingest`)

Back to [Architecture Index](/architecture/index.md). Related: [Practice & FSRS](/architecture/practice-fsrs.md), [Multi-Account CLI Pool](/architecture/cli-auth-pool.md).

## 1. Four Local Ingestion Sources

`lp-ingest` scans local agent histories incrementally using `ingest_cursors (source_key PRIMARY KEY, mtime_ms INTEGER, size_bytes INTEGER)`:

1. **Claude Code (Across All Discovered Accounts)**:
   - Walks `~/.claude/projects/**/*.jsonl`, `~/.claude-accounts/*/projects/**/*.jsonl`, and `~/.claude-work/projects/**/*.jsonl` (skipping `subagents/`).
   - Tags each ingested session with both `agent = "claude"` and the specific `account_id` (`claude:pratham.bhavsar501@gmail.com` vs `claude:q97159745@gmail.com`).
   - Extracts `cwd`, `gitBranch`, `sessionId`, user prompts (stripping `<system-reminder>`), assistant explanations, and compact tool summaries (`[tool: Edit crates/lp-core/src/graph.rs]`).
2. **Codex CLI**:
   - Walks `~/.codex/sessions/YYYY/MM/DD/*.jsonl`.
   - Parses `session_meta` (`id`, `cwd`, `git.branch`) and `response_item` user/assistant turns, filtering out `AGENTS.md` and `<environment_context>` headers.
3. **Antigravity (`agy`)**:
   - **Snapshot Safety Law**: Never opens `~/.gemini/antigravity-cli/conversation_summaries.db` in place. `with_snapshot` copies `conversation_summaries.db`, `conversation_summaries.db-wal`, and `conversation_summaries.db-shm` into a `TempDir` before querying `summaries` and reading `~/.gemini/antigravity-cli/brain/<conversation_id>/*.md` artifacts.
4. **Git Commit & Unified Diff Matcher**:
   - Resolves the session's `cwd` to its git repo root via `git rev-parse --show-toplevel`.
   - Checks against the project allowlist (defaulting to `/Users/pratham/code/tracker`, `/Users/pratham/code/bunfiiliate`, `/Users/pratham/code/learnpuppy`, plus any user-added repos).
   - Runs `git log` over `[started_at - 5 min, ended_at + 30 min]` and extracts commit SHAs, subjects, changed file lists, and unified diffs (capped at 6,000 chars per commit).

## 2. Decision Distillation & Knowledge Graph Bridge

For each ingested session with substantive engineering activity:
1. **`distill_session_decisions`**: Calls `LlmEngine::complete_structured<DistilledSession>` to extract up to 8 `DecisionRecord`s (`title`, `what`, `why`, `basis: Stated | Inferred`, `alternatives`, `risk`, `files`, `commit_sha`).
2. **Knowledge Graph Bridge (`fold_decisions_to_graph`)**: Converts generalizable System Design and Rust insights from `DecisionRecord`s into `SessionFindings` (`propositions` + `chains`) and feeds them through the Auditor -> Merger pipeline so practical coding lessons populate the Knowledge Graph!

## 3. Multiple-Choice Card Builder & Hard Quality Gates (`cards.rs`)

Generates 4-option MC cards (`options: [String; 4]`, `correct_index: 0..3`, `rationale`) and enforces `validate_card_shape`:
- **Context-Independence Gate**: Rejects any prompt matching `/\b(in this (project|session|repo|codebase|commit)|why did (we|you|the user))\b/i` unless the concrete setup is included inline.
- **Option Length-Ratio Gate**: Computes `correct_len` and `mean_distractor_len`. Rejects any card where `correct_len as f64 > 1.6 * mean_distractor_len + 10.0` so the correct option is never noticeably longer than distractors.
