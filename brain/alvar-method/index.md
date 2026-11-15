# Alvar Method — AI-Powered One-to-One Learning

Back to [Brain Index](/index.md). Related: [Core Domain Contracts & Algorithms](/architecture/core-domain.md), [Classroom & Async Workbench Engine](/architecture/classroom-engine.md), [Adaptive Practice Sessions & FSRS v5](/architecture/practice-fsrs.md).

> **Source:** Eero Alvar, *How I Use AI to Learn Things* (2026-08-14, 18:58) — [youtu.be/kzcI5F4tGiU](https://youtu.be/kzcI5F4tGiU). Companion video: *How I Learn Difficult Things* — [youtu.be/ciC6ffUqI8k](https://youtu.be/ciC6ffUqI8k).

## What This Is

The Alvar Method is a one-to-one AI tutoring system. One teacher (the AI), one student (the learner). The AI aggregates many sources through one interface, engineers trust via fact-checking, and concentrates all cognitive struggle into the material itself — absorbing logistics, planning, sourcing, verification, and note-keeping.

LearnPuppy's `lp-core` 3-phase fold (`Probe → Plan → Teach`), `lp-agent` classroom engine, and `lp-store` FSRS review are direct implementations of this method. These documents capture the original source so the implementation stays grounded.

## Documents

- [Philosophy: One-to-One Teaching](/alvar-method/philosophy.md) — Why many-to-many learning wastes effort, the two principles (optimized teaching + optimized mental resource allocation), and the source/interface distinction.
- [The Process: Probe → Plan → Teach → Close](/alvar-method/process.md) — The 4-phase loop, how each phase works, what the system absorbs vs what the learner handles.
- [Teaching Protocols](/alvar-method/teaching-protocols.md) — Mastery bar, graded quiz format, prediction-before-reveal, faded worked examples, interleaving, Feynman pass, spaced review, misconception register, confidence tracking, no-reveal rule.
- [Visual & Verify Skills](/alvar-method/visual-verify.md) — The `learn-visual` self-reviewing SVG loop and the `learn-verify` fact-checking subagent protocol.
- [Skills Reference](/alvar-method/skills-reference.md) — The 5 core skills (`teach`, `probe`, `learn-profile`, `learn-visual`, `learn-verify`), their triggers, file structures, and quiz UI mapping per agent harness.
- [Community Implementations](/alvar-method/implementations.md) — Survey of Alvarmethod, agentic-learning, EmilioFarias/teach, Mathesis, and learning-with-copilot.
- [Full Video Transcript](/alvar-method/transcript.md) — Cleaned auto-captions of the original video with chapter timestamps.

## How I Watched This Video

Since I cannot play video directly, I used the following approach — applicable to any YouTube learning source:

1. **Metadata** via YouTube's oembed API (`/oembed?url=...&format=json`) — title, author, thumbnail, channel URL.
2. **Transcript** — the Alvarmethod repo ships a cleaned auto-caption file (`source/eero-alvar-how-i-use-ai-to-learn-things.md`), which I fetched in full.
3. **Visuals** — the video thumbnail (`https://i.ytimg.com/vi/kzcI5F4tGiU/hqdefault.jpg`) was checked via the oembed response. The demo section references Obsidian as the visual interface (LaTeX rendering, mermaid diagrams, embedded SVGs).
4. **Cross-referencing** — I fetched 5 community implementations (GitHub repos) that encode the video's spoken method as code, each adding detail the transcript alone does not surface (mastery bar, spaced review, textbook mode, code-enforced state machine).
5. **Companion video** — the second video *How I Learn Difficult Things* (ciC6ffUqI8k) was identified via oembed and summarized from implementation descriptions: "Start from facts the brain can accept without hedging, then build by motivated discovery at the edge of what you already hold."

## Relevance to LearnPuppy

| Alvar Method concept | LearnPuppy implementation |
|---|---|
| Probe (graded MCQ, binary-search edge) | `lp-core` `Probe` phase, `ProberAgent` in `lp-agent` |
| Plan (mermaid DAG, fact-check) | `lp-core` `Plan` phase, `plan_init` parser, `research` subagent |
| Teach (one reasoning step, quiz lock-in) | `lp-core` `Teach` phase, `TeacherAgent`, `quiz`/`explain` tools |
| Mastery bar (MC + free recall) | `lp-agent` `classroom_view`, verdict system |
| Spaced review (FSRS ladder) | `lp-store` `tracker` FSRS v5 engine |
| Visual subagent (self-reviewed SVG) | `lp-agent` `visual` subagent |
| Fact-check subagent | `lp-agent` `research` subagent |
| Learner profile | `lp-service` settings/profiles |
| Session log persistence | `lp-store` JSONL ledgers |
