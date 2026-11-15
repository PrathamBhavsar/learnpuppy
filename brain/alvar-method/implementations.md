---
type: guide
title: Alvar Method Community Implementations
description: "Survey of five open-source implementations of Eero Alvar's AI learning method: Alvarmethod (portable skills), agentic-learning (OpenCode), EmilioFarias/teach (full skill + textbook mode), Mathesis (code-enforced state machine), and learning-with-copilot (Copilot + Anki)."
tags: [alvar-method, implementations, github, open-source, skills, opencode, claude-code, codex]
timestamp: 2026-11-04
---

# Alvar Method Community Implementations

Back to [Alvar Method Index](/alvar-method/index.md) | [Brain Index](/index.md). Related: [Skills Reference](/alvar-method/skills-reference.md).

> Eero Alvar's original demo used Pi Agent with Obsidian. The community has since ported the method to multiple agent harnesses as installable skills. Each implementation adds detail the video does not surface.

## 1. Alvarmethod (vasanthsreeram)

**Repo:** [github.com/vasanthsreeram/Alvarmethod](https://github.com/vasanthsreeram/Alvarmethod)
**Install:** `npx skills add vasanthsreeram/Alvarmethod -g --all`
**Agents:** Claude Code, Codex, Grok, Pi, OpenCode, Cursor

The canonical portable implementation. Five skills: `teach`, `probe`, `learn-profile`, `learn-visual`, `learn-verify`. Ships the cleaned video transcript in `source/`.

### Key contributions
- Portable across all skills-CLI agents via `npx skills`.
- Quiz UI mapping table per harness (Claude Code → `AskUserQuestion`, Grok → `ask_user_question`, etc.).
- `.alvar/` file structure standard (LEARNER.md, maps/, sessions/, visuals/).
- The mermaid flowchart: Probe → Plan → Teach → Lock-in quiz → (pass: next node / fail: insert prereq).

### Install paths

| Agent | Global path |
|-------|-------------|
| Codex | `~/.codex/skills` + `~/.agents/skills` |
| Claude Code | `~/.claude/skills` |
| Grok | `~/.grok/skills` |
| Pi | `~/.pi/agent/skills` |
| OpenCode | `~/.config/opencode/skills` |

## 2. agentic-learning (pacchio1)

**Repo:** [github.com/pacchio1/agentic-learning](https://github.com/pacchio1/agentic-learning)
**Agent:** OpenCode (native features)

An OpenCode-native implementation with custom tools and plugins.

### Key contributions
- **Quiz extension** (`.opencode/tools/quiz.ts`): graded MCQs with `quiz_grade`, `quiz_summary`; history in `.learn/quiz-history.jsonl`.
- **MD-log extension** (`.opencode/tools/note.ts` + `.opencode/plugins/md-log.ts`): links a markdown file to the session, auto-writes everything (LaTeX included).
- **SVG-maker subagent** (`.opencode/agent/svg-maker.md`): draws educational SVG figures and looks at the rendered result to fix them.
- **SVG-checker subagent** (`.opencode/agent/svg-checker.md`): independent figure QA (PASS / FIX).
- **Fact-checker subagent** (`.opencode/agent/fact-checker.md`): verifies every non-obvious claim before teaching.
- Obsidian vault integration via symlink for live LaTeX/Mermaid/figure rendering.

### Commands
`/teach <topic>`, `/note <file.md>`, `/setup-profile`

## 3. EmilioFarias/teach

**Repo:** [github.com/EmilioFarias/teach](https://github.com/EmilioFarias/teach)
**Agents:** Cursor, Claude Code, Codex (anything reading `~/.agents/skills/`)

The most complete implementation. Adds everything "school actually needs" on top of the video's demo.

### Key additions beyond the video
- **Mastery bar** — correct MC + correct free recall in own words on separate turns. Self-report does not count.
- **Textbook mode** — map printed pages to PDF pages, extract the chapter's own objectives, teach from the book, quiz every objective. Progress tracked in a course ledger.
- **Spaced-review queue** — the learner dates reviews, run at the start of the next session. Ladder: 1 day, 3, 7, 14, 30.
- **Misconception register** — written at the moment of a miss, not at session end.
- **Notes gate** ("yap note") — before every quiz, the learner types thinking/questions.
- **Confidence tracking** — "sure / think so / guessing" folded into every quiz. Reports "answered sure and was wrong: N of M."
- **No-reveal rule** — on a miss, never state the correct option. Re-teach from a different angle, re-quiz.
- **Graded questions** — 3 content options + "I don't know" + Other (thought process). "I don't know" is a gap, not a miss.
- **Active time line** — idle gaps over 10 minutes move the timer back.
- **Faded worked examples** — 3 rungs: full example → middle blanked → alone.
- **Interleaving** — every third quiz is a mixed pass from earlier steps.
- **Feynman pass** — explain to a ten-year-old on load-bearing concepts.
- **Prediction before reveal** — commit to a wrong answer, then see the right one.

### File structure
```
skills/teach/
  SKILL.md           setup, probe, plan, teach, close
  protocols.md       mastery bar, quizzes, review ladder, teaching moves
  textbook-mode.md   map the book, extract objectives, quiz all of them
templates/           profile, review queue, ledger/map formats
bin/
  svg2png            render a diagram so the agent can check its own drawing
  find-textbook-figure  find a figure by caption (printed page ≠ PDF page)
```

## 4. Mathesis (tdsiar)

**Repo:** [github.com/tdsiar/Mathesis](https://github.com/tdsiar/Mathesis)
**Agent:** OpenCode or any AGENTS.md harness (Claude Code reads CLAUDE.md symlink)
**Interface:** Obsidian (HOME.md, per-subject dashboards, native Canvas)

The key upgrade: **the enforcement lives in code, not in prompt rules.**

### Key contributions
- **Code-enforced state machine** (`.opencode/plugins/tutor.ts`) — tool-call blocks, not suggestions:
  - Mark a node done with no quiz file → **blocked**
  - Change a mastery status with no recorded evidence → **blocked**
  - Enter plan/teach without a committed DAG → **blocked**
  - Mark done without a diagram for the concept → **blocked**
  - Malformed evidence → **blocked**
  - Edit state.json through shell redirection → **blocked**
  - Context lost to compaction → state summary auto-reinjected
- **Machine-generated interface** — HOME.md, per-subject dashboards, Obsidian Canvas (path.canvas) where cards light up green as you master them.
- **State.json** — single source of truth: phase + mastery ledger per subject.
- Every concept gets a self-reviewed diagram.

### Layout
```
AGENTS.md                     the tutor's constitution — loaded every session
protocol/
  probe.md                    how to map the edge
  plan.md                     how to build the committed DAG
  teach.md                    the teach → exercise → quiz loop
.opencode/plugins/tutor.ts    gates + dashboard/canvas generation
.opencode/agent/visual.md     self-reviewing diagram agent
subjects/<subject>/
  subject.md                  curriculum + scope decisions
  state.json                  phase + mastery ledger
  probe-findings.md            what the probe heard, with gaps
  plan.mmd                    the committed DAG
  lessons/  quizzes/          one artifact pair per node
```

## 5. learning-with-copilot (MMostafa-Hub)

**Repo:** [github.com/MMostafa-Hub/learning-with-copilot](https://github.com/MMostafa-Hub/learning-with-copilot)
**Agent:** GitHub Copilot

Ports the Alvarmethod skills to `~/.copilot/skills/`. Adds a `keep` skill.

### Key addition
- **`keep` skill** — turns a finished lesson into an Anki deck. Requires Anki + an Anki MCP server (Anki MCP add-on or AnkiMCP). Creates decks and notes programmatically.

## Comparison Table

| Feature | Alvarmethod | agentic-learning | EmilioFarias | Mathesis | copilot |
|---------|:-:|:-:|:-:|:-:|:-:|
| Probe → Plan → Teach | ✓ | ✓ | ✓ | ✓ | ✓ |
| Mermaid DAG | ✓ | ✓ | ✓ | ✓ | ✓ |
| Graded MCQ via UI | ✓ | ✓ | ✓ | ✓ | ✓ |
| Fact-check subagent | ✓ | ✓ | ✓ | — | ✓ |
| Visual subagent (self-review) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Visual checker subagent | — | ✓ | — | — | — |
| Mastery bar (MC + free recall) | — | — | ✓ | ✓ | — |
| Spaced review queue | — | — | ✓ | ✓ | — |
| Misconception register | — | — | ✓ | — | — |
| Confidence tracking | — | — | ✓ | — | — |
| No-reveal rule | — | — | ✓ | — | — |
| Notes gate | — | — | ✓ | — | — |
| Prediction before reveal | — | — | ✓ | — | — |
| Faded worked examples | — | — | ✓ | — | — |
| Interleaving | — | — | ✓ | — | — |
| Feynman pass | — | — | ✓ | — | — |
| Textbook mode | — | — | ✓ | — | — |
| Code-enforced gates | — | — | — | ✓ | — |
| Anki export | — | — | — | — | ✓ |
| Obsidian integration | optional | ✓ (symlink) | ✓ (native) | ✓ (Canvas) | — |
| Portable across agents | ✓ | OpenCode only | ✓ (agents dir) | AGENTS.md | Copilot |

## Relevance to LearnPuppy

LearnPuppy implements the Alvar Method as a Rust workspace rather than agent skills. The mapping:

| Alvar Method skill | LearnPuppy crate |
|--------------------|------------------|
| `teach` | `lp-agent` `TeacherAgent` + `lp-core` `Teach` phase |
| `probe` | `lp-agent` `ProberAgent` + `lp-core` `Probe` phase |
| `learn-visual` | `lp-agent` `visual` subagent |
| `learn-verify` | `lp-agent` `research` subagent |
| `learn-profile` | `lp-service` settings/profiles |
| Spaced review | `lp-store` `tracker` FSRS v5 engine |
| Session persistence | `lp-store` JSONL ledgers |
| Mastery bar | `lp-agent` `classroom_view` + verdict system |
| Quiz UI | `lp-server` SSE → `web/` React SPA |

The EmilioFarias implementation is the closest analog to LearnPuppy's design — both have a mastery bar, spaced review, and session persistence. LearnPuppy adds the hybrid JSONL+SQLite storage layer and multi-account CLI pooling that none of the skill-based implementations have.
