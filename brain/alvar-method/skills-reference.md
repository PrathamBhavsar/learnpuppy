---
type: reference
title: Alvar Method Skills Reference
description: The 5 core skills (teach, probe, learn-profile, learn-visual, learn-verify), their triggers, file structures, and the quiz UI tool mapping per agent harness (Claude Code, Codex, Grok, OpenCode, Pi).
tags: [alvar-method, skills, reference, quiz-ui, agent-harness, file-structure]
timestamp: 2026-11-04
---

# Alvar Method Skills Reference

Back to [Alvar Method Index](/alvar-method/index.md) | [Brain Index](/index.md). Related: [The Process](/alvar-method/process.md), [Teaching Protocols](/alvar-method/teaching-protocols.md), [Visual & Verify](/alvar-method/visual-verify.md).

> The Alvar Method is implemented as a set of agent skills. Each skill is a SKILL.md file in the agent's skills directory. The skills compose: `teach` calls `probe`, `plan` calls `learn-verify`, `teach` calls `learn-visual`.

## Skill Overview

| Skill | Job | Trigger |
|-------|-----|---------|
| `teach` | Full loop: probe → plan → one-step teaching | `/teach`, "teach me", "I want to learn", "introduce me to" |
| `probe` | Understanding map only | `/probe`, "quiz me", "what do I already know", "find the edge" |
| `learn-profile` | Write learner profile | `/learn-profile`, "how I learn", "learning philosophy", "set up tutoring" |
| `learn-visual` | One SVG, look at it, fix it | `/learn-visual`, "figure", "diagram", "visualization" |
| `learn-verify` | Fact-check a claim before teaching | `/learn-verify`, "fact-check", "verify this", "is this true" |

## teach

The primary skill. Full loop in order:
1. Restate the goal in one sentence. Confirm.
2. Load or create learner files.
3. Probe (or reuse a fresh map).
4. Plan + mermaid. Pause for confirmation.
5. Teach node 1. One reasoning step per turn. Quiz. Advance on lock-in.

### Hard rules
- Struggle stays in the material. The system absorbs logistics.
- Do not reteach `known`. Do not start in `unknown` with no ramp.
- One reasoning step per turn. Stop. Quiz that step. Advance only on lock-in.
- Show the mermaid plan **before** teaching. Do not skip the graph.
- Never dump the whole explanation in one message.
- Do not invent citations. Verify or mark uncertainty.
- Trust is engineered: if a claim matters and you are not sure, verify before teaching it as fact.

### When they interrupt
Answer the question. Do not "just finish the slide." Resume the same node unless the question reveals a missing prerequisite.

### Done for now
Write what locked, what is still `edge`, and the next node. Leave the session file in a state another agent can resume.

## probe

Diagnostic questions only. No lectures. Measures the edge of understanding.

- Start wide, then binary-search every strand the lesson will depend on.
- 1–3 MCQs per turn. Wait for the tool.
- Always include "I don't know" as an option.
- If they already listed solid ground, skip those strands.
- Write the map to `.alvar/maps/<slug>.md`.

### Scoring

| Result | Status Move |
|--------|-------------|
| correct + sound reason | `known` |
| correct, thin reason | `edge` |
| wrong, near-miss | `edge` |
| wrong, foundation missing | `unknown` |
| I don't know | `blocked` or `unknown` |

### End
Show the table. Say which strands a teacher should start from. Do not start the lesson unless they ask.

## learn-profile

Interview the learner and write `.alvar/LEARNER.md` so one teacher can fit one mind.

### Interview (one cluster at a time, 2–3 questions per turn)
1. **Solid ground** — what they already hold thoroughly (subjects, notations).
2. **Goal** — what "done" looks like for the next stretch.
3. **Pace** — default is one reasoning step, then a quiz.
4. **Struggle** — they keep the hard thinking; the system keeps logistics.
5. **Voice** — density, tone, hated LLM habits, language.
6. **Artifacts** — Obsidian / markdown / no files.

Do not ask all six in one message. Use the harness quiz UI, not a markdown list.

### LEARNER.md template

```markdown
# Learner

## How I learn
- Pace: one reasoning step at a time
- Struggle: keep it in the material; I will do the problems
- Explanations I want: _
- Explanations I do not want: rushed surveys, unearned "now you know"

## Voice
- Density: compact
- Tone: _
- Notation I already use: _

## Solid ground
- Topics I hold thoroughly: _

## Goals
- Current: _

## Artifacts
- Write maps and sessions under `.alvar/`
- Visuals: SVG when a picture would lock the idea
- I read session markdown in: editor / Obsidian / other
```

## Quiz UI — Harness Tool Mapping

**Never print A/B/C/D in the chat transcript.** Every probe and lock-in quiz must go through the interactive question tool the agent already has. Wait for the tool result before scoring.

| Tool | Harness | Call |
|------|---------|------|
| `AskUserQuestion` | **Claude Code** | `AskUserQuestion` |
| `ask_user_question` | **Codex** / **Grok** | `ask_user_question` |
| `question` | **OpenCode** | `question` |
| `quiz` | **Pi** (quiz extension) | `quiz` |
| `ask_user` / `askUserQuestion` | **Pi** (ask-user extension) | that tool |

No match → say which tool is missing and stop. Do not fall back to pasted multiple choice.

### Shared quiz shape
- 1–3 questions per call. Wait.
- One right answer. `multiSelect` = false.
- Options: 3 content choices + "I don't know" (last, never shuffled with content).
- Do not mark the correct option `(Recommended)` or put it first.
- Free-text / Other is for talk-through. Treat a typed reason as signal when scoring.
- Header / strand tag: short (≤12 chars if the tool requires it).

## File Structure

### Alvarmethod / agentic-learning style (`.alvar/`)

```
.alvar/
  LEARNER.md                 # how this mind wants to be taught
  maps/<slug>.md             # probe results for one goal
  sessions/<date>-<slug>.md  # plan + steps + quizzes
  visuals/<slug>-<n>.svg     # diagrams from learn-visual
```

### EmilioFarias/teach style (`~/learning/`)

```
~/learning/
  profile.md
  review-queue.md
  sessions/YYYY-MM-DD-<topic-slug>.md
  sessions/<folder>/assets/
  courses/<course>/mastery.md
  textbooks/<book>/map.md
```

### Map file format

```markdown
# Map — <goal>

Updated: <ISO date>
Goal: <one sentence>

## Strands
| strand | status | evidence |
|--------|--------|----------|
| line integrals | known | Q2 correct, explained work |
| Stokes | edge | recognized statement, missed Faraday link |
| differential forms | unknown | said so |
| SR field mix | blocked | answered "I don't know" |

## Quiz log
- Q1 [line integrals] C — correct
```

### Session file format

```markdown
# Session — <goal>
Date:
Model:
Goal:

## Plan
\`\`\`mermaid
graph TD
  A[covector] --> B[1-form]
  B --> C[wedge]
\`\`\`

## Log
### Node: covector
- taught:
- visual:
- quiz:
- result: lock-in | retry | insert-prereq
```
