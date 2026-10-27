---
type: guide
title: Mandatory Pre-Merge Verification Gate
description: Exact verification checklist required before merging any feat/* branch into staging or fast-forwarding prod.
tags: [wisdom, gate, ci, testing, e2e, ocr, okf]
timestamp: 2026-10-27
---

# Mandatory Pre-Merge Verification Gate

Back to [Wisdom Index](/wisdom/index.md). Related: [Task DAG](/tasks/index.md), [Anti-Patterns](/wisdom/anti-patterns.md).

Under `/ponytail` and `/orchestration` rules, **no worker branch (`feat/T<XX>-*`) may be merged into `staging`** until every command below exits `0`:

## 1. Per-Task Pre-Merge Gate (`make gate`)

```sh
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

# 1. Formatting & Zero-Warning Clippy
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# 2. Unit, Property (proptest), and Snapshot (insta) Tests
cargo test --workspace --all-targets

# 3. License & Security Advisory Check
cargo deny check

# 4. Strict OKF Brain Validation (0 errors, 0 warnings)
python3 /Users/pratham/.claude/skills/okf/scripts/validate.py /Users/pratham/code/learnpuppy/brain --strict
```

## 2. Git Merge Protocol (`staging` Integration)

1. Verify working tree is clean (`git status --porcelain` is empty) and no secrets/tokens or `.tmp` files are tracked.
2. Commit with short, descriptive lowercase message: `feat: <what you did>` or `fix: <what you did>`.
3. From `/Users/pratham/code/learnpuppy` (on `staging`):
   ```sh
   git merge --no-ff feat/T<XX>-<slug> -m "merge: feat/T<XX>-<slug> into staging"
   ```
4. Append a 2–4 line summary entry to `[/log.md](/log.md)` and update task status in `[/tasks/index.md](/tasks/index.md)`.

## 3. Final Phase 6 Release Gate (`T60`)

Before fast-forwarding `prod` to `staging`:
1. **E2E Suite (`tester-army/e2e` + `expectrl` PTY tests)**:
   - Verifies multi-account `claude` discovery (`#1` and `#2`), `codex`, and `agy`.
   - Verifies full `Probe -> probe_done -> plan_init -> Teach quiz -> Graph extraction` lifecycle.
   - Verifies `/Users/pratham/code/tracker` ingestion -> Decision distillation -> 4-choice FSRS card review.
2. **Alibaba Open Code Review (`ocr`)**:
   - Run `ocr` diff review against `prod..staging` and resolve all findings.
3. **Claude Opus High Sign-Off**:
   - Final architectural review confirming 100% parity with `docs/REVERSE_ENGINEERING.md` and `docs/PLAN.md`.
