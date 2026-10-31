# B21 verification report

Verified 2026-10-31 in the B21 worktree.

- `CARGO_TARGET_DIR=/Users/pratham/code/learnpuppy/target CARGO_INCREMENTAL=0 make gate`: passed. 383 tests passed, 0 failed, 1 existing ignored real-data migration test. Formatting and clippy with `-D warnings` passed; strict OKF: 27 files, 0 errors, 0 lints; regenerated OpenAPI passed the drift check.
- New HTTP/SSE scenarios cover Probe → Plan → Teach, encoded arguments, PracticeGate and conclude feedback, stale answers, active/completed idempotent requests, cancellation with partial text, cooldown recovery and deadline, engine crash/retry, restart/resume, boot job recovery, PDF job completion/abort, upload magic/size rejection, sandboxed artifacts and symlink confinement, combined text/thinking throttling, coalescing, sequence order/replay, global concurrency, the eight-question probe cap and compaction.
- Production handlers in the new route files contain no `unwrap()`.

## Live server run

Seeded a disposable medium fixture with `learnpuppy seed --fixture medium --out <scratch>/medium`: 800 nodes, 1,200 chains, 6 chats, 120 cards and 30 decisions. Launched:

`LP_FAUX_SCRIPT=crates/lp-service/tests/faux/teach.json learnpuppy serve --fixture <scratch>/medium --engine faux --port 0 --no-open`

The server bound 127.0.0.1:53862. Exchanged the launch token for its session cookie (303), opened a real HTTP SSE listener, created chat_43b12fc235d9, answered its probe and quiz, and refetched the Classroom view after each rest. All three command responses were 202. HTTP calls used Node fetch because AGENTS.md prohibits curl. No credentials were printed or retained in this report.

Observed events, in order:

```text
chat.created
chat.updated
chat.delta(thinking,seq=1)
chat.delta(text,seq=2)
chat.toolCall(probe)
chat.rest
chat.updated
chat.toolCall(probe_done)
chat.toolCall(plan_init)
chat.delta(text,seq=1)
chat.toolCall(quiz)
chat.rest
chat.updated
chat.delta(text,seq=1)
chat.toolCall(explain)
chat.rest
```

The final question was `explain`, with `own=visited` and `borrow=current`. View versions increased: 1793464009949 → 1793464009974 → 1793464009995. Pending probe/quiz answers remained hidden. SIGTERM shutdown exited 0. The disposable fixture was removed.

## Integration and remaining boundary

Minimal integration glue registers the routes/OpenAPI, supplies live turn state to existing reads, serializes rename/delete with turns, recovers jobs at startup, and waits for interrupted turns to save on shutdown. Existing read-API assertions now expect startup recovery of seeded jobs.

A separate live FauxEngine run confirmed an existing defect outside B21 ownership in `crates/lp-agent/src/loops/tools.rs::conclude`: after the agent rejects a second `problem` while one is active, it selects the most recent problem call's modelSolution even when that call failed. Reproduce by changing the second problem's modelSolution in `practice.json` to `REJECTED_SOLUTION`, then answer the first problem. Observed: `Model solution: REJECTED_SOLUTION`; expected the first committed problem's solution. The coordinator should fix that agent handler to select the active successful problem and add a regression with different solutions. The Practice scenario here verifies the rejection gate using a repeated solution.

Real CLI account/subprocess smoke was not run; all model behavior used FauxEngine.
