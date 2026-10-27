# LearnPuppy Reverse-Engineering Specification (`learnharness` + `tracker`)

This document captures the complete clean-room reverse-engineering specification of:
1. **`amosblomqvist/learnharness`** (`v0.3.8`, inspected via [`morluto/rea`](https://github.com/morluto/rea) static analysis of `/Applications/LearnHarness.app` and full source audit of the upstream repository).
2. **`/Users/pratham/code/tracker` (`server/digest/`)** (local CLI session ingestion, decision distillation, MC card generation, and FSRS v5 spaced-repetition scheduler).

Every behavioral invariant, data contract, state machine, prompt composition rule, and deterministic algorithm documented here is preserved in **LearnPuppy**'s pure-Rust CLI/TUI port.

---

## 1. REA Static Analysis Summary of `LearnHarness.app` (`v0.3.8`)

Running `npx -y rea-agents@latest analyze-javascript-application /Applications/LearnHarness.app --json` against the installed macOS Electron bundle revealed:
- **Runtime**: Electron (`v35+`) wrapping a standalone Next.js 16 server (`app.asar/.next/standalone/server.js`) and spawning external Node/Playwright/Poppler/Whisper subprocesses.
- **Why CLI Logins Failed in Upstream LearnHarness**:
  - Upstream only wired three OAuth / Web-Cookie subscription bridges in `src/agent/subscription-connections.ts`:
    1. `chatgpt` (`https://auth.openai.com/oauth/authorize` PKCE flow on localhost port `1455` targeting `https://chatgpt.com/backend-api/codex/responses`).
    2. `github-copilot` (GitHub Device Flow `Iv1.b507a08c87ecfe98`).
    3. `grok` (Electron `BrowserWindow` scraping cookies from `https://grok.com`).
  - Upstream had **zero integration** with local CLI credentials (`~/.claude`, `~/.claude-accounts/*`, `~/.codex/auth.json`, `~/.gemini/oauth_creds.json`) and **zero support** for invoking authenticated local agent CLIs (`claude`, `codex`, `agy`).
- **Heavy Desktop Bloat**:
  - ~450 MB Electron + Chromium + Next.js bundle, polling HTTP/SSE endpoints (`GET /api/events`), spawning headless Chromium via Playwright (`demo-builder`, `visual-maker`), and reading/writing full JSON files on every turn.

---

## 2. Upstream `learnharness` Hexagonal Architecture & Import Law

Upstream enforces a 4-layer hexagonal dependency DAG (ported directly into our Rust workspace crate boundaries):

```
lp-core  <──  lp-store
   ^             ^
   │             │
lp-cli-driver ───┼── lp-agent ──> lp-ingest
                 │        ^
                 └────────┴────── lp-tui / learnpuppy (bin)
```

### Strict Layer Responsibilities
1. **`core` (`lp-core`)**: Pure domain types, state machines, parsers, and deterministic folds. Zero IO, zero async, zero filesystem or network access.
2. **`store` (`lp-store`)**: Persistence adapters implementing `core` port traits (`ChatStore`, `GraphStore`, `JobStore`, `SettingsStore`, `TrackerStore`).
3. **`agent` (`lp-agent`)**: Stateless per-turn agent builders (`ProberAgent`, `TeacherAgent`, `PracticeAgent`), tool execution + argument healing, background sub-agent runner, and knowledge-graph extraction pipeline (`Extractor -> Auditor -> Merger`).
4. **`tui` / `cli` (`lp-tui`, `learnpuppy`)**: Terminal presentation layer and CLI command router.

---

## 3. Core Domain Contracts & Invariants (`src/core/`)

### 3.1 Three-Phase Session Lifecycle (`src/core/phases.ts`)
Every teaching session (`ChatKind::Teach`) progresses through three event-derived phases computed purely from the transcript (`phase_at_3(messages)`):

| Phase | Entry Condition | Active Agent | Toolset |
|---|---|---|---|
| **`Probe`** | Initial state (before concluded `probe_done`) | `ProberAgent` | `probe` (choice) OR `recall` (free-text), `ask_user_question`, `probe_done`, `read`, `ls`, `find`, `grep`, `research`, `pdf`, optional `list_sessions` / `list_propositions` |
| **`Plan`** | After a succeeded `probe_done` tool result, before a succeeded `plan_init` | `TeacherAgent` (planning mode) | `plan_init`, `quiz` OR `explain`, `ask_user_question`, `read`, `ls`, `find`, `grep`, `research`, `demo`, `visual`, `pdf` |
| **`Teach`** | After a succeeded `plan_init` tool result | `TeacherAgent` (teaching mode) | Same as `Plan` (`plan_init` remains callable to rewrite the DAG if the learner redirects) |

**Critical Invariants**:
- **Distinct Quizzing Tool Names Across Phases**: In `Probe`, the quizzing tools are named `probe` (multiple-choice) and `recall` (free-text), which have **no `type` or `nodeId` fields**. In `Plan`/`Teach`, they are named `quiz` and `explain`, which **require** `type: "check" | "predict"` and `nodeId: string` validated against the live `PlanTree`. This prevents the LLM from carrying `nodeId`-less call habits from probing into teaching.
- **`probe_done` Gate**: `probe_done` is refused if there is an unanswered question in the transcript or if `probe_done` already succeeded. On success, its model-facing ack is fixed (`PROBE_DONE_ACK_TEXT`) to avoid duplicating the assessment in context, and it queues a deterministic handoff steer (`steer_probe_<chatId>`) with `HANDOFF_TEXT`.

### 3.2 Formal Plan DAG & Derived Position Statuses (`src/core/contracts/plan.ts`, `src/core/plan-tree.ts`)
- **Author-Once Mermaid Subset (`plan_init`)**:
  - Input is a Mermaid `graph TD` / `flowchart TD` string:
    - Node declaration: `slug["Title — 1-2 sentence summary"]` (em-dash `—` or `--` splits `title` and `summary`).
    - Edge declaration: `prereq-slug --> dependent-slug` (chains `a --> b --> c` and `&` fan-in/fan-out `a & b --> c` supported).
  - Validated for: non-empty nodes, valid slug syntax (`[a-z0-9-]+`), no undeclared edge endpoints, and **DAG acyclicity** (Kahn's algorithm / DFS cycle detection returning the exact cycle path `a -> b -> a` with line numbers).
- **Position-From-Calls Status Derivation (`derive_plan_statuses`)**:
  - `PlanTree` stores **structure only** (`nodes`, `edges`, `version`). It never stores completion flags.
  - Status (`Current`, `Visited`, `Upcoming`) is derived purely from `quiz` / `explain` tool **calls** (regardless of whether the learner answered right or wrong):
    - Scan assistant tool calls for `quiz` or `explain` whose `nodeId` exists in `tree.nodes`.
    - Collapse consecutive duplicate `nodeId`s into a run list `runs: Vec<String>`.
    - `current = runs.last()`.
    - `visited = runs[..runs.len()-1].iter().collect()`.
    - Every other node in `tree.nodes` is `upcoming`.

### 3.3 Question Core, Option Shuffling, & Argument Healing (`src/core/question-core.ts`, `src/core/argument-heal.ts`)
- **Graded Multiple-Choice (`probe` / `quiz`)**:
  - Fields: `question`, `options: Vec<{ label, value }>`, `correctAnswer: String | Vec<String>`, `explanation: String`, `multiSelect?: bool`, plus (`quiz` only) `type: "check" | "predict"` and `nodeId: String`.
  - Authoring validation (`quiz_input_problems`):
    - At least 2 options; unique non-empty `value`s and `label`s.
    - `correctAnswer` must reference valid option `value`(s).
    - **Uniform Option Length Law**: Prevents the "longest option is right" LLM tell.
- **Deterministic Seeded Fisher-Yates Shuffle (`answer-presentation.ts`)**:
  - Options are shuffled at display time using a deterministic hash seed of `toolCallId` so the correct answer is never biased toward option `a`, while remaining stable across re-renders and restarts.
  - An automatic `"I don't know"` option (`__idk__`) is appended at the bottom of the dock.
- **Learner Answer Payload**:
  - Carries: selected option(s), **Confidence rating** (`1..5` or percentage slider), and **Two "Think Out Loud" Channels**:
    1. `readingThinking`: Notes/questions typed while reading the teacher's explanation before answering.
    2. `thinking`: Reasoning attached to the submitted answer.
- **Argument Healing (`heal_encoded_arguments`)**:
  - Models occasionally double-encode arrays/objects as JSON strings or attach copy-syntax debris (`:node-id`, surrounding quotes). Every tool runs a deterministic pre-validation heal pass that parses stringified arrays/objects and strips value debris before schema validation.

### 3.4 Async Sub-Agents & "Ack Now, Steer Later" Wake Pipeline (`src/core/contracts/jobs.ts`, `src/server/turn-service.ts`)
- **Four Background Job Kinds**:
  1. `research`: Multi-step web/local search + page readability extraction -> `{ brief, sources: [{ title, url }] }`.
  2. `pdf`: Local PDF triage, page search, text/layout extraction -> `{ path, question, brief, pagesRead, pagesConsulted }`.
  3. `visual`: Diagram generator -> `{ svg, ascii_fallback, notes }`.
  4. `demo`: Interactive concept explorer -> in LearnPuppy TUI, produces both an interactive terminal parameter/plot model (`TuiDemoSpec`: sliders + live Unicode/Braille curve/state canvas) AND a self-contained HTML artifact (`~/.learnpuppy/artifacts/<jobId>.html`).
- **Concurrency Cap**: Process-wide semaphore of **3 permits** (`bg_llm_semaphore`) shared across all background sub-agent model calls so foreground teaching turns never starve.
- **Deterministic Steer Delivery**:
  - Mid-turn tool call (`research`, `demo`, `visual`, `pdf`) creates a `JobRecord` (`status: Running, delivery: { kind: Result, status: Pending }`) and immediately returns `"dispatched — continue your turn"`.
  - When the background worker finishes, it writes `result` and queues a `SteerMessage` with deterministic ID `steer_<jobId>` (or `steer_probe_<chatId>` for probe handoff).
  - **Idle Wake Gates**: If the session has no turn in flight, no unanswered question (`pendingQuestion == false`), and is not dismissed, the wake pump claims pending steers (`claim_steers`), appends the `SteerMessage` marker to the transcript, and runs a teacher continuation turn. If a turn IS in flight, the steer injects at the next agent step boundary.

### 3.5 Global Knowledge Graph & 3-Stage Extraction Pipeline (`src/core/graph.ts`, `src/agent/subagents/extraction.ts`)
- **Chains-Primary Data Model**:
  - **Stored Truth**:
    - `nodes.jsonl`: Append-only `GraphNode` lines (`id: "n_<12hex>"`, `name`, `statement`, `aliases`, `createdAt`, `provenance: [{ chatId, messageId }]`). First line for an `id` defines `name`, `statement`, `createdAt`; later lines for the same `id` are **link-appends** whose `statement` and `aliases` join `aliases` and whose `provenance` unions.
    - `chains.jsonl`: Append-only `GraphChain` assertions (`id: "arg_<12hex>"`, `premises: Vec<NodeId>`, `conclusion: NodeId`, `provenance`, `createdAt`). Merged at read time by canonical key `sorted(premises).join("+") + ">" + conclusion`.
    - `coverage.jsonl`: Per-message extraction ledger (`{ chatId, fp: sha256(messageViewLines), tokens, foldedAt }`). Only teacher prose + answered check pairs are fingerprinted; probe turns and practice turns are excluded.
    - `sessions.jsonl`: Checkpoints for one-shot markdown/file imports (`{ chatId: "src_<hash>", foldedAt }`).
  - **Derived View (`derive_graph_snapshot`)**:
    - Never stored; computed in memory from `nodes` + `chains`:
    - Creates one virtual implication junction node per multi-premise chain (`id = chain.id = "arg_..."`) and untyped `edges` (`premise -> imp -> conclusion`), plus `logicalEdges` (`premise -> conclusion` directly) for inference-depth calculation, and `clusters` via `birth_session_partition` (`MIN_CLUSTER_SIZE = 3`).
- **Three-Stage Pipeline (`Extractor -> Auditor -> Merger`)**:
  1. **Extractor (LLM via `run_submit_agent`, max 4 turns)**:
     - Sees the uncovered teacher passage batch (`TEACHER [msgId]` blocks) PLUS `"THE GRAPH SO FAR"` (digest of propositions already extracted from this same source).
     - Calls `submit({ propositions, chains })`. Can reference existing same-source node IDs (`n_...`) directly in `chains` to form cross-batch arguments.
  2. **Auditor (LLM via `run_submit_agent`, max 4 turns, serialized under global FIFO mutex)**:
     - **Never sees the raw transcript** and **never sees or mints chains**.
     - Sees only the fresh `propositions` and uses read-only graph tools (`find`, `list_sessions`, `list_propositions`) to emit **identity verdicts only**: `{ links: [{ id: "p1", linkTo: "n_abc123" }] }` with an **equivalence bias** (paraphrases of the same world-facing truth merge).
  3. **Merger (`merge_findings` + `simplify_chains`, pure deterministic Rust)**:
     - Resolves local IDs (`p1`) to `linkTo` (appending a link line) or mints a new `n_<12hex>` node.
     - Emits chains and runs **`simplify_chains`** over the full chain ledger:
       - **Rule 1 (Prefer-Fewest-Premises)**: For a given conclusion `C`, if kept chain `K` has `K.premises ⊂ Candidate.premises`, `Candidate` is dropped as redundant.
       - **Rule 2 (Transitive Ancestry-Cover)**: Build the full premise-ancestry closure over all chains (`A ⟹ X` makes `A` an ancestor of `X`). If every premise of `Candidate` is either in `K.premises` or is a transitive ancestor of `K.premises`, `Candidate` is dropped (`[A, B] ⟹ X` and `[X, C] ⟹ Y` makes `[A, B, C] ⟹ Y` redundant).
     - Appends `CoverageEntry` lines only after node and chain writes succeed.

### 3.6 Adaptive Practice Sessions (`src/agent/tools/practice.ts`)
- A session with `ChatKind::Practice` runs the `PracticeAgent` over selected Knowledge Graph propositions/chains (or a free-form topic).
- **Two Core Tools with Intra-Turn State Gate (`PracticeGate { problem_active: bool }`)**:
  1. `problem`: Poses one self-contained problem (`statement`, `mode: "recall" | "discrimination" | "composition"`, `target?: Vec<NodeId>`, `modelSolution: String`). Terminates the turn; `modelSolution` is committed before the learner sees the problem and hidden from the UI until `conclude`.
  2. `conclude`: Records `{ outcome: "solved" | "solvedWithHints" | "movedOn" | "surrendered" | "dismissed", verdict: String, rung: "R0".."R5" | null, missClass: "cue" | "discrimination" | "execution" | "careless" | "structural" | null, retest: bool }`. Reveals `modelSolution` verbatim and ends the turn waiting for `[NEXT PROBLEM]`.
- **Hint Ladder (`R0` to `R5`)**:
  - When the learner submits partial work or asks for a hint in the `"Your thinking"` lane, the agent climbs the minimum rung needed (`R0: Restate goal` -> `R1: Point to break` -> `R2: Cue missing proposition` -> `R3: Name the proposition` -> `R4: Setup step` -> `R5: Full worked step`).

### 3.7 Context Compaction & Money/Quota Channel (`src/core/compaction.ts`, `src/core/prices.ts`)
- **20,000-Token Retained Tail (`RETAINED_TAIL_TOKENS = 20_000`)**:
  - When a session's active context exceeds the threshold, background compaction summarizes the prefix before the 20k-token tail cut (snapping the cut backward so `[quiz_call, quiz_answer]` pairs are never split) and inserts a `LhCompactionMessage` marker (`COMPACTED_HISTORY_HEADER`).
- **Parallel Turn & Extraction Usage Logs**:
  - Token usage (`inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `reasoningTokens`) is stored in append-only parallel arrays (`turns`, `extractions`) on `ChatRecord` and `JobRecord` so message rewrites never clobber billing history.

---

## 4. `/Users/pratham/code/tracker` Digest & Spaced-Repetition Specification

LearnPuppy natively integrates `tracker`'s local CLI transcript learning and FSRS spaced-repetition system so the user can learn **System Design**, **Rust**, and **Engineering Decisions** directly from their own coding sessions (`claude`, `codex`, `agy`) and local repositories (`/Users/pratham/code/tracker`, `/Users/pratham/code/bunfiiliate`, etc.).

### 4.1 Local CLI Session & Git Ingestors (`server/digest/ingest.ts`)
1. **Claude Code (`~/.claude/projects/**/*.jsonl` AND `~/.claude-accounts/*/projects/**/*.jsonl` AND `~/.claude-work/projects/**/*.jsonl`)**:
   - Skips `subagents/` subdirectories.
   - Incremental cursor keyed by `source_key = "claude:<fullPath>"` with `{ mtimeMs, size }` fast-skip.
   - Extracts `user` and `assistant` messages (`cwd`, `sessionId`, `gitBranch`, text blocks, and compact tool summaries `[tool: Bash ls]`, `[tool: Edit src/main.rs]`), stripping `<system-reminder>` blocks.
2. **Codex CLI (`~/.codex/sessions/YYYY/MM/DD/*.jsonl`)**:
   - Reads `session_meta` (`payload.id`, `payload.cwd`, `payload.git.branch`) and `response_item` user/assistant message blocks, skipping `AGENTS.md` / `<environment_context>` boilerplate.
3. **Antigravity CLI (`~/.gemini/antigravity-cli/conversation_summaries.db` + `brain/<id>/`)**:
   - **Never opens the live SQLite DB in place**: copies `conversation_summaries.db` (+ `-wal`, `-shm`) into a temp directory (`with_snapshot`) before reading `summaries` table + `brain/<conversation_id>/*.md` artifacts.
4. **Git Commit & Diff Extractor (`collectGitCommits`)**:
   - For allowlisted project roots (e.g., `/Users/pratham/code/tracker`, `/Users/pratham/code/learnpuppy`, `/Users/pratham/code/bunfiiliate`), runs `git log` within the session window (`-5 min` to `+30 min`) and captures commit SHAs, subjects, bodies, changed files, and unified diffs (capped at 6,000 chars per commit).

### 4.2 Decision Distillation (`server/digest/distill.ts`)
- Synthesizes each coding session + its matched git commits into structured `DecisionRecord`s:
  - `title` (concrete subject, e.g., `"Snapshot copy for Antigravity SQLite WAL ingestion"`).
  - `what` (1–2 sentences naming real files/types/functions).
  - `why` (1–3 sentences explaining the tradeoff or constraint).
  - `basis`: `"stated"` (explicit in transcript) or `"inferred"` (derived from code diff).
  - `alternatives`: `Vec<{ option, rejected_because }>`.
  - `risk`: Caveat, failure mode, or follow-up trigger.
  - `files`: Touched file paths; `commit_sha`: Git commit hash.
- **Bridge into LearnPuppy Knowledge Graph**:
  - Every distilled `DecisionRecord` can also be emitted into LearnPuppy's Knowledge Graph as world-facing System Design / Rust propositions (`GraphNode`) and trade-off reasoning chains (`GraphChain`), connecting practical coding work directly to theoretical graph clusters!

### 4.3 Self-Contained Multiple-Choice Card Generation & Distractor Guards (`server/digest/cards.ts`)
- Generates 4-option MC cards (`options: [String; 4]`, `correct_index: 0..3`, `rationale`) across two tracks:
  1. **Decision Track**: `kind: "why" | "predict" | "bug"` from distilled coding decisions.
  2. **Book / Code / Rust Track**: `kind: "why" | "trace" | "ownership" | "bug" | "cloze"` from ingested book chapters or Rust source files.
- **Hard Quality Gates (`validate_card_shape`)**:
  - Rejects any question matching context-dependent phrases (`/\b(in this (project|session|repo|codebase|commit)|why did (we|you|the user))\b/i`) unless the prompt states the concrete setup inline.
  - **Length-Ratio Guard**: Rejects any card where `correct_option.len() > 1.6 * mean(distractor_lens) + 10` (prevents the "longest answer is right" tell).

### 4.4 Deterministic FSRS v5 Replay (`server/digest/fsrs.ts`)
- Uses FSRS parameters (`request_retention = 0.9`, `maximum_interval = 365`, `enable_fuzz = false`, `enable_short_term = true`).
- **Event-Sourced Review Log**: Card scheduling state (`due`, `stability`, `difficulty`, `reps`, `lapses`, `state`) is deterministically derived by replaying `(card_id, rating: Again|Hard|Good|Easy, reviewed_at)` events sorted by `(reviewed_at, id)`.
- Multi-device sync works without conflict via per-device append-only JSONL logs (`sync-<device>.jsonl`).
