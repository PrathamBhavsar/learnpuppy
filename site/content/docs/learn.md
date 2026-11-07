---
title: Learn
group: Use
order: 5
description: Probe, plan, teach, practice. What each phase does and the question types.
---

# Learn

A Learn chat has a conversation on the left and a **Plan** panel on the right.

## Probe

The tutor asks up to **8 questions** to find what you already know, as free text or choices. A counter shows "Question 3 of 8". Nothing is taught before this ends.

## Plan

The plan appears as a graph of steps. Steps are styled as current, visited or upcoming. Click a step to jump to where it was taught, or to read its summary. A stepper above it shows Probe, Plan, Teach.

## Teach

The tutor teaches one step at a time and asks you to think. It asks with these tools:

| Type | What you do |
| --- | --- |
| Quiz | Pick from options A to D, with keys `a` to `d` or `1` to `4`. A confidence slider (1 to 5) appears when the quiz asks for it. The verdict and an explanation follow. |
| Recall | Answer in free text, or press "I don't know". |
| Explain | Write the idea in your own words. |
| Problem | Solve a statement in one of three modes: recall, discrimination or composition. Ask for a hint and the ladder goes one rung up. After it closes you see the outcome and what kind of miss it was. |
| Free question | Answer a question with options, free text or both. |

An answered question stays readable as a recap: every option, which was right, which you picked, and your confidence.

## Practice

A **Practice** chat starts from nodes you select in the [graph](#/graph) instead of a topic.

## Workbench jobs

Research, visual and PDF jobs run beside the chat. Attach a PDF in the composer to start a PDF job. Results open in an artifact viewer: markdown inline, images as images, and HTML in a sandboxed iframe with no scripts.

## Control

- `Enter` sends, `Shift+Enter` adds a line.
- `Esc` stops a running turn and keeps the partial answer.
- **Steer** lets you guide a turn while it runs.
- If the server restarted mid-turn, the chat offers **Resume**.
- If every model account is busy, a banner shows the wait with a countdown and a Cancel button.
- A failed turn shows Retry and "Copy diagnostics".

## What was verified

Probe, Plan, Teach, Practice and the workbench jobs were verified against real CLIs. See the [FAQ](#/faq) for the full list of limits.
