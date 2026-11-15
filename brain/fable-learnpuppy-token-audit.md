---
type: guide
title: Fable Direction and Token Audit Prompt
description: Ready-to-run prompt that has Fable produce one full report on LearnPuppy - feature comparison with learnharness, audit of top-level decisions, token optimization, test plan, and whether it can be a learner's only tutor.
tags: [prompt, audit, tokens, cost, learnharness, decisions, tutor, report]
timestamp: 2026-11-11
---

# Fable Direction and Token Audit Prompt

Back to [Brain Index](/index.md). **Run this only after the UI polish merges are complete** (see [UI Polish Plan](/ui-polish/index.md) and its [task DAG](/ui-polish/tasks.md)), so the report judges the product as built, not as planned. Copy everything below the line into Fable.

---

<role>
You are the lead research analyst for LearnPuppy. You think, delegate, and synthesize. You do not implement. You are honest: no flattery of the project, no flattery of the comparison repo. A finding with no evidence is not a finding.
</role>

<vision>
The owner's goal: a second brain that actually teaches, and constantly tries to be replaced by the learner's own brain. It should be the owner's only tutor for everything from a big topic (a whole book like a PDF of "Designing Data-Intensive Applications") to a small definition. The product should take the best of `amosblomqvist/learnharness` (output quality, UI polish, interactive teaching) and beat it on cost, enforcement and retention. The owner wants to know: are we going in the right direction, and what must change to get there?
</vision>

<context>
Repo: /Users/pratham/code/learnpuppy (branch `staging`). Rust workspace plus a Vite/React SPA, local-only server on 127.0.0.1. Orientation: `brain/index.md` first, then `brain/overview.md`. Do not wander beyond what workers report.

What LearnPuppy is: visual lessons, a cross-session Knowledge Graph, FSRS spaced repetition, multiple isolated workspaces, a course-as-data "cheap tutor" (ADR-0007), and a learning-method layer (ADR-0008) enforced in Rust: probe with `why` and 3 strands, plan, teach loop, mastery levels, 90-minute bout limit, brain dump, close report, review dates, fact-check ledger, gap pauses, difficulty control.

Known state to verify, not to trust: the real study hour (T9, G2, M14, G4) has never been run, so there is no real-learner evidence yet that the method teaches better. Real-model runs are a manual script, not CI. Textbook mode (F37: PDF to chapters to objectives to cards, "check that I read this chapter") is being built after a reviewed spec; check how far it got. The UI polish work (stream-safe markdown, lazy Mermaid, type ladder with a reading font, event cards, think-out-loud box with opt-in dictation, KaTeX chain glyphs) is merged or in progress; check what actually shipped.

LearnPuppy has NO API key. Every model call shells out to a local CLI (`claude` multi-account pool, `codex`, `agy`), so "token cost" means subscription quota burned per learning hour, plus latency. Calls are routed by role in `crates/lp-service/src/engine.rs` (`CliPoolEngine`) into `crates/lp-cli-driver/src/engine/subprocess.rs` (`run_turn`, `run_structured`). Two tiers: strong and cheap. The G4 budget is in `brain/learning-method/real-hour.md`: under 100,000 tokens, at most 2 strong and 22 cheap calls per real hour, never hit a provider limit.

Token surfaces (start here): per-turn agents `crates/lp-agent/src/loops/`; classroom loop `crates/lp-service/src/chat/runtime.rs`; course creation `crates/lp-service/src/course/`, `crates/lp-agent/src/course/`; Extractor, Auditor, Merger in `crates/lp-agent/src/extract/`; workbench sub-agents `crates/lp-agent/src/workbench/`; ingest and digest `crates/lp-ingest`, `crates/lp-service/src/digest*`; subprocess overhead (what each spawned CLI loads by default); existing measurement: `/courses/{id}/usage` meter, `scripts/real_smoke.py`, `bench/cheap_tutor/`, `FauxEngine`.

Sources of intent (the three methods): `brain/alvar-method/`, `brain/huberman-learning/`, `brain/gornik-learning/`. Decisions: `brain/decisions/` (ADR-0001 to ADR-0009), `docs/web/00-decisions.md`, `docs/web/09-grill-and-risks.md`, `brain/wisdom/`. Plans: `brain/learning-method/`, `brain/cheap-tutor/`, `brain/ui-polish/`.

The comparison repo: https://github.com/amosblomqvist/learnharness (AGPL-3.0, so read for ideas, never copy code). A worker clones it shallow into the scratchpad. Known from an earlier code read: it uses `streamdown` for half-streamed markdown with math, Mermaid, code and CJK; a typographic ladder with a bundled reading font; one card per tool event; thinking fields with dictation; a drawing pad, coding questions with a runner and flashcards as plugins; an Electron desktop build; about 420 test files. It runs an agent loop with prompts (`src/agent/prompts`, subagents, tools) rather than course-as-data.
</context>

<questions>
The report must answer all of these, each with evidence or an explicit "unknown, needs measurement X":

A. **Inventory.** What have we actually built, per area (platform, method features F01 to F38, UI, textbook mode, ingestion, review)? Mark each: built and tested, built and untested, partial, planned, parked. Give counts. Do not rely on docs alone; a worker confirms against code and tests.

B. **Comparison with learnharness.** Feature by feature and layer by layer (teaching loop and prompts, probing, practice, knowledge graph, question types, markdown and math rendering, visuals and demos, files and PDFs, UI polish, extensibility, packaging, tests, cost behaviour). For each: who does it better, why, and what exactly to adopt, adapt or reject (licence rule applies). Quantify where possible (lines of prompt, calls per lesson, bundle size, test count). Be realistic: list what learnharness does better and what we do better, without boasting.

C. **Top-level decision audit.** Review every ADR and the big decisions (web-only product, local CLI pool instead of API keys, course-as-data, enforcement in code, numbers as settings, Rust core plus TypeScript renderer, workspaces, FSRS, graph as stored truth, Mermaid and plan DAG, textbook mode design, no plugin API). For each: is it still right, what is its strongest counter-argument, what evidence would flip it, and if sound, how to optimize it. Name the three decisions most likely to be wrong and why.

D. **Can this be the only tutor?** Test the vision against the product. Cover: big topics (a book) down to small definitions; cold start on a new topic; coverage of what the learner already knows; keeping retention honest (does it measure, not assume, learning); handling wrong or invented model content (fact-check, citations); when the model is rate-limited; support for maths, code, diagrams, handwriting, speech; **fading itself out**: does the design actively move the learner toward needing the tutor less (retrieval before explanation, hint ladder that shrinks, graph showing owned versus borrowed knowledge, scheduled "no tutor" tests)? List concrete gaps between today's product and a tutor that tries to be replaced by the learner's own brain, ranked by impact.

E. **Best of both worlds.** A battle-tested target product: which learnharness behaviours we adopt, which of our strengths stay, which new features the three methods imply that neither product has. Output a prioritized roadmap (now, next, later), each item with effort, expected learning benefit, token cost, and the test that proves it worked.

F. **Token economics.** Where do tokens go today (ranked by share of a real learning hour, by call site, role and tier)? What can be cut or shifted to cheaper tiers without hurting learning? Each change with file and line, saving, quality risk and effort. Include the cost of the new features (textbook import, think-out-loud notes, any per-turn UI features) and whether the G4 budget is itself right. The target: learnharness-level output quality at a fraction of its token spend.

G. **How to test it.** A repeatable method that separates token cost from learning quality, so an optimization or feature is accepted only if both hold. Include a battle-test plan for the whole product: a golden scenario on `FauxEngine` for deterministic call counts, a live cheap-tier run for real numbers, a quality guard (pre and post question sets, pinned judge model or rubric), the owner's real hour (T9 baseline versus M14 bout) with sample sizes and the minimum effect worth acting on, a one-week retention retest, failure drills (rate limits, malformed output, bad PDF), and CI-level assertions (max calls per phase, prompt size ceilings, prompt prefix snapshots).
</questions>

<hard_budget_rules>
Fable tokens are the scarce resource. Obey all of these:
- Effort: medium. Use high only for section C and D synthesis if a worker table conflicts. No deliberation loops, no re-deriving what this prompt states.
- Do NOT read source in bulk. Fable reads at most 12 files directly, only to confirm a claim a worker reported. All broad reading, grepping, counting, cloning and measuring goes to cheap workers via /tribe.
- Workers return distilled tables (never raw file dumps) and cite file and line. Tell every worker this, and tell them to keep each table under 60 rows.
- No web research beyond the find-skills step and at most 3 official-docs lookups (prompt caching, CLI flags, token counting).
- Final report: at most 3,500 words plus tables. No process narration, no repeated context.
- Stop when every question in `<questions>` has an evidence-backed answer or an explicit "unknown, needs measurement X". Do not pad.
- Ground every number in a tool result or a worker's measurement. Mark estimates as estimates.
</hard_budget_rules>

<tool_protocol>
<find_skills>
Use the `find-skills` skill (vercel-labs/skills, installed globally). Follow its steps in order:
1. Understand need: LLM token auditing, prompt caching and compression, CLI-agent cost control, LLM eval harness, learning-quality evaluation, spaced repetition, PDF and textbook ingestion, streaming markdown.
2. Check the skills.sh leaderboard first.
3. `npx skills find <query>` for each of: `token optimization`, `prompt caching`, `llm cost audit`, `eval harness`, `context compression`, `rust profiling`, `pdf extraction`, `spaced repetition`. Max 8 searches.
4. Verify quality before recommending: install count (prefer 1K+), source reputation, GitHub stars. Skip anything under 100 installs or from unknown authors.
5. Report at most 5 candidates: name, source, installs, what it would give LearnPuppy, why it earns its place.
6. Do NOT install anything. Installing is the owner's decision.
</find_skills>

<tribe>
Use `/tribe` (skill at ~/.claude/skills/tribe; read `references/model_routing.md` once). You are the Architect; workers do the legwork. Max 4 workers at once. Codex login may be expired: if a worker fails to start, route to `claude1`, `claude2` or `agy` and do not stop to ask.
Post at most 7 read-only tasks:
- W1 (AGY or Codex, medium): **inventory.** Per area, feature, test: built, tested, partial, planned, parked; counts; cite tests. Include git log summary since 2026-10-30, and merged versus planned UI polish packages U0 to U9 and textbook mode U7 and U8.
- W2 (AGY, medium): **learnharness deep read.** Clone to the scratchpad (shallow). Table of its teaching loop, prompts (size, structure), subagents, tools, question types, plugins, rendering stack, UI patterns, tests, and any token-saving or token-wasting behaviour. Ideas only, no code copying.
- W3 (Codex or AGY, medium): **call-site and prompt anatomy.** Every model call site: file:line, role, tier, effort flag, trigger, calls per learning hour, what prompt content is rebuilt and resent each turn, fixed versus dynamic versus history sizes, dead or duplicate text.
- W4 (AGY, medium): **subprocess overhead.** What each spawned `claude`, `codex`, `agy` call loads by default (tools, MCP, hooks, skills, settings, system prompt) and the flags that shrink it. Measure with `FauxEngine` or one cheap live call.
- W5 (Codex, medium): **test and measurement inventory.** What the usage meter, `real_smoke.py`, `bench/cheap_tutor`, `FauxEngine`, e2e and CI measure; the gaps; how to log per-call tokens by role and phase.
- W6 (free tier or low effort): **decision digest.** One line per ADR and `docs/web/00-decisions.md` entry: decision, date, stated reason, stated risk, whether any evidence since has supported or contradicted it.
- W7 (AGY, medium): **tutor-readiness probe.** Walk the running product (faux engine) as a learner would for: a one-line definition, a chapter of a book via PDF upload, a new topic from scratch. Record every point where it breaks, stalls, costs a call, hides the answer badly, or fails to push the learner toward independence. Short table with steps and verdict.
Workers must not edit files, commit, push, merge, or run `make ci`. Live model calls: at most 4 total across all workers, cheap tier only. Each worker writes its table to the board and finishes. You read only the board summaries. Delete finished workers and any scratch clones when done (`tribe clean`).
</tribe>

<advisor>
Use `/advisor` exactly three times, never more:
1. After the workers' tables are in, before drafting sections C and D: send the decision digest and your three most doubtful decisions and ask the advisor to attack them.
2. Before drafting section F: send the ranked token table plus your top 5 optimizations and ask what is missing, what is overestimated, which cut would damage learning.
3. On the finished draft: send sections C to E and the test plan and ask for the single biggest flaw.
Incorporate each point or say in one line why not. If `/advisor` is unavailable, say so once at the top and do not substitute extra self-review loops.
</advisor>
</tool_protocol>

<optimization_lenses>
Check each lens and report only those with real evidence:
- History resend: stateless turns resend full history; summarize, window or resume sessions where the CLI supports it.
- Prompt caching: stable prefix first, dynamic content last; is the prefix identical across calls and roles?
- Tier routing: which strong calls can run on cheap at an equal quality bar; which cheap calls are unnecessary.
- Call elimination: deterministic code replacing a model call (grading, option shuffle, FSRS, strand scoring, verdict wording, objective and card text stored at import).
- Batching: merge small sequential calls (probe questions, card generation, extractor stages, per-chapter import).
- Structured output: short JSON schemas, no prose wrappers, output caps per role.
- Extractor, Auditor, Merger: sample or skip the audit stage when confidence is high.
- Workbench sub-agents: lazy start, caps, reuse across lessons.
- Subprocess overhead: strip tools, MCP, hooks, skills, settings from spawned CLIs; effort per role.
- Precompute and reuse: lessons, visuals and cards generated once and stored.
- Retries and failover: do rate-limit or malformed-JSON retries double-spend?
- Rendering and UI features: confirm none of them cause model calls.
</optimization_lenses>

<constraints>
- Read-only investigation. No code edits, commits, pushes, merges, installs, or `brain/` updates. Write only to the Fable scratchpad.
- Licence: learnharness is AGPL-3.0. Ideas only.
- Do not propose removing a learning-method gate unless evidence shows it is pointless; the owner decides that via G4. If you doubt a gate, name the measurement that would settle it.
- Where a question needs the real hour or live quota, say what to measure and how, instead of guessing.
- Never flatter. If the product is behind learnharness somewhere, say so plainly; if learnharness is wasteful somewhere, say so plainly.
</constraints>

<output_format>
Single markdown report, in this order:
1. **Verdict** (8 lines max): are we heading the right way; the biggest gap; the biggest safe win; can it be the only tutor today (yes, no, almost, with the three blockers); realistic token saving range.
2. **Inventory** (section A): table with counts and statuses.
3. **Comparison** (section B): table; who wins, evidence, adopt, adapt or reject.
4. **Decision audit** (section C): table per decision (right, doubtful, wrong), strongest counter-argument, flip evidence, optimization. Then the three riskiest decisions in one paragraph each.
5. **Only-tutor readiness** (section D): scenario results from W7, the fading-out gaps, ranked.
6. **Best-of-both roadmap** (section E): now, next, later, each with effort, benefit, token cost, proving test.
7. **Token map and optimization table** (section F): ranked token map (call site, role, tier, calls per hour, tokens per call, share; measured or estimated), then optimizations (id, change, file:line, saving, quality risk, effort, confidence) sorted by saving divided by risk.
8. **Do-not-cut list**: features that carry learning quality.
9. **Battle-test plan** (section G).
10. **Skills found via find-skills** (table, not installed).
11. **Open questions** that need the real hour or owner input.
12. **Advisor notes**: one line each for the three consultations.
Save to `token-and-direction-report.md` in the Fable scratchpad directory and reply with the full path plus a 6-line summary.
</output_format>

<success_criteria>
- Every table row traces to a worker table, a file and line, or a measurement.
- At least 5 token optimizations have file and line plus a saving estimate with its basis.
- Section C names at least 3 decisions with a concrete counter-argument and the evidence that would change the decision.
- Section D lists the exact steps where a real learner would be blocked today.
- A different engineer can reproduce the baseline versus optimized comparison from section G with no extra questions.
- Fable's own spend stayed within the hard budget rules, and no worker or scratch clone is left running or on disk.
</success_criteria>
