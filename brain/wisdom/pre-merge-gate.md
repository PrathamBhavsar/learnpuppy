---
type: guide
title: Mandatory Pre-Merge Verification Gate
description: Exact verification checklist required before merging any worker branch into staging or fast-forwarding prod, including cargo, OKF, tester-army/e2e, and alibaba/open-code-review (ocr).
tags: [wisdom, gate, ci, testing, e2e, ocr, okf, web]
timestamp: 2026-10-28
---

# Mandatory Pre-Merge Verification Gate

Back to [Wisdom Index](/wisdom/index.md). Related: [Task DAG](/tasks/index.md), [Anti-Patterns](/wisdom/anti-patterns.md), [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md).

Under `/ponytail` and `/orchestration` rules, **no worker branch (`pratham404/<task-id>`) may be merged into `staging`** until every command below exits `0`:

## 1. Per-Task Pre-Merge Gate (`make gate`)

```sh
export PATH="/opt/homebrew/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

# 1. Formatting & Zero-Warning Clippy across all 7 crates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# 2. Unit, Property (proptest), and Integration Tests
cargo test --workspace --all-targets

# 3. License & Security Advisory Check
cargo deny check

# 4. Strict OKF Brain Validation (0 errors, 0 warnings)
python3 /Users/pratham/.claude/skills/okf/scripts/validate.py /Users/pratham/code/learnpuppy/brain --strict
```

## 2. Git Merge Protocol (`staging` Integration)

1. Verify working tree is clean (`git status --porcelain` is empty) and no secrets/tokens or `.tmp` files are tracked.
2. Commit with short, descriptive lowercase message: `feat: <what you did>`, `fix: <what you did>`, `test: <what you did>`, or `docs: <what you did>`.
3. From `/Users/pratham/code/learnpuppy` (on `staging`):
   ```sh
   git merge --no-ff pratham404/<task-id> -m "merge: <task-id> into staging"
   ```
4. Append a 2–4 line summary entry to `[/log.md](/log.md)` and update task status in `[/tasks/index.md](/tasks/index.md)`.

## 3. Final Release Gate (Archived: `D60` macOS Dioxus Release)

> Archived: `lp-ui`/`lp-app` and the root `e2e.config.ts` harness were removed by [ADR-0006](/decisions/0006-web-ui-over-dioxus.md). The web release gate (Playwright under `web/e2e/`, `make e2e`) is defined in `docs/web/07-testing-and-quality.md` and `docs/web/10-acceptance.md`. The list below is history.

Before fast-forwarding `prod` to `staging`:
1. **Dioxus SSR & Component Tree Tests (`cargo test -p lp-ui -p lp-app`)**:
   - Verifies every `lp-ui` component (`Button`, `Badge`, `Card`, `Dialog`, `Tabs`, `Progress`, `Slider`, `CommandPalette`, SVG icons) and every `lp-app` screen (`GraphCanvasView`, `ClassroomView`, `DigestFsrsView`, `AccountsModal`) renders valid HTML/SVG with `dioxus-ssr` and handles state transitions.
2. **End-to-End Visual & Agentic Suite (`tester-army/e2e` + `e2e.config.ts`)**:
   - Launches `learnpuppy-app` in E2E web-harness mode (`LP_E2E_PORT`) and runs `npx e2e run` + `crates/lp-app/tests/e2e_desktop.rs` verifying:
     - Quick-action Icon Rail navigation (`Graph Home`, `Classroom`, `FSRS & Digest`, `CLI Accounts`).
     - Interactive SVG Knowledge Graph rendering (`n_...` nodes, `arg_...` junctions, zoom/pan controls, 3-level Chain Table toggle, node provenance drawer).
     - Complete Classroom flow (`Probe -> probe_done -> plan_init -> Plan DAG rings -> Teach quiz -> Graded verdict`).
     - Multi-account CLI pool (`claude #1: pratham.bhavsar501@gmail.com`, `claude #2: q97159745@gmail.com`, `codex`, `agy`) and FSRS 4-choice flashcard grading.
3. **Alibaba Open Code Review (`ocr`)**:
   - Run `ocr scan` / `ocr delegate` across `crates/lp-ui` and `crates/lp-app` and `git diff prod..staging`, resolving all P0/P1/P2 findings.
4. **Claude Opus High Sign-Off (`claude-opus-4-6-thinking --effort high`)**:
   - Final architectural review confirming 100% parity with `docs/REVERSE_ENGINEERING.md` and `docs/PLAN.md`.
