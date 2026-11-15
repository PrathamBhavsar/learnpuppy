---
type: spec
title: Alvar Method Visual & Verify Skills
description: The learn-visual self-reviewing SVG loop (draw, look, fix, look again) and the learn-verify fact-checking subagent protocol (claim, search primary sources, return verdict). Trust is engineered, not assumed.
tags: [alvar-method, visual, verify, svg, fact-check, trust, subagent]
timestamp: 2026-11-04
---

# Alvar Method Visual & Verify Skills

Back to [Alvar Method Index](/alvar-method/index.md) | [Brain Index](/index.md). Related: [The Process](/alvar-method/process.md), [Teaching Protocols](/alvar-method/teaching-protocols.md), [Skills Reference](/alvar-method/skills-reference.md).

> Trust is engineered. These two skills are how the single-interface system earns it: one makes pictures that actually show the idea, the other makes sure claims are true before they are taught as fact.

## learn-visual — Self-Reviewing Teaching Diagrams

**Trigger:** `/learn-visual`, `/skill:learn-visual`, or "figure, diagram, SVG, sketch, visualization, picture of a concept."

**Philosophy:** One idea, one picture. The picture exists so the learner can accept the step — not as decoration.

### Output
Write an SVG file (`.alvar/visuals/<slug>-<n>.svg` or `<log-dir>/assets/<slug>-<n>.svg`). Embed or link it in the session file.

Prefer SVG. Use another format only if the harness cannot preview SVG.

### The Loop (do not skip)
1. State the claim the picture must make, in one sentence.
2. Draw the smallest picture that makes that claim.
3. **Look at the file** (image/read tool). If you cannot view it, say so and keep the SVG simple enough to audit as text.
4. Fix labels, overlap, wrong arrows, missing units, or a picture that does not match the claim.
5. Look again. Stop after a clean pass, not after the first draft.

### Design rules
- One claim. No collage of the whole course.
- Large labels. High contrast. No tiny legend the learner needs a second lesson to read.
- If the idea is algebraic, show the objects (arrows, planes, machines), not a screenshot of the equation.
- No decorative gradients, watermarks, or "AI art" backgrounds.

### Failures to catch on the look pass
- Arrow direction disagrees with the prose
- Two symbols for the same object
- 3D that hides the relation
- Cropped text
- A picture of a *different* special case than the one just taught

### Why subagents (from the video)
> "The reason that these are done in sub agents is to obviously preserve context but also the sub agents will look at the image to verify whether it actually looks correct. It wrote an SVG and now it's going to view it. It's going to edit it a bit, look at it again."

### After delivery
Tell the learner what to look at first ("the two arrows, then the pairing number"). Do not re-teach the whole node unless they ask.

### Teacher verification gate
The teacher reads the rendered PNG as the verification gate — for a diagram, labels and arrows; for a figure, it must be a chart (axes + data), not body text. If the picture is wrong, send the correction back to a new run. Embed by relative path only AFTER the PNG check passes.

## learn-verify — Fact-Checking Before Teaching

**Trigger:** `/learn-verify`, `/skill:learn-verify`, or "fact-check, verify this, is this true, source this, don't hallucinate."

**Philosophy:** Trust is engineered. Check the claim before it is taught as fact.

### When to run
- Empirical, historical, bibliographic, or API/tool claims
- Named theorems, identities, or "standard facts" you cannot reconstruct
- Anything you were about to present with unearned certainty

Skip a full search only when you can derive the statement in-session and the learner does not need an external citation. Still say that it was derived, not sourced.

### Method
1. Write the claim in one falsifiable sentence.
2. Fetch or search primary-ish sources (paper, textbook, official docs, standard reference). Do not cite a URL you did not open.
3. Quote or paraphrase the supporting line. Note edition/year if it matters.
4. Mark disagreements. Prefer the source the field actually uses.
5. Return a verdict.

### Verdict format

```markdown
## Claim
…

## Verdict
confirmed | qualified | contradicted | unknown

## Sources
- <title> — <url or citation> — <what it says>

## Teach as
<one sentence the teacher may now say, with any hedge>
```

- `qualified` = true under stated assumptions (dimension, characteristic, gauge, version).
- `unknown` = do not teach it as fact. Say you could not verify.

### Rules
- No invented papers, quotes, or page numbers.
- One claim per run. Batch only if they are the same fact in different words.
- Write the verdict into the session file if a teach session is open.

### From the video
> "We have to have reliable verification and fact-checking is one, correct information. Obviously, we absolutely don't want the AI to hallucinate false information. But also two, because it makes learning easier when we know that the system is reliable."

### Timing in a teach session
- Launch at plan time.
- Launch again on resume if none is running.
- Launch a follow-up the moment a new definition, number, or named fact appears that was not in the last batch.
- Do not wait until session end. A plan-batch check from earlier does not cover a new explanation about to be given.
- The fact-check itself never appears in the session log — it runs, and the plan corrects itself when a claim fails. The only trace is the corrected teaching.

## Textbook Figures (Textbook Mode Extension)

In textbook mode, the teacher never extracts or crops a figure — a subagent does it.

1. Locate the page with `find-textbook-figure BOOK.pdf "FIGURE 2.4"` (searches the text layer for the caption, never `printed_page + offset`).
2. Crop the complete printed figure: title block, chart, source line. No body text.
3. Write a new filename so the live view cannot keep a stale file.
4. The teacher reads the crop. Embed only if it is a chart (axes + data). If it is body text, the page is wrong — send it back.

> Printed page numbers are not PDF page numbers. The difference is the front matter, it is different in every book, and getting it wrong teaches forty confidently wrong pages while everything looks healthy.
