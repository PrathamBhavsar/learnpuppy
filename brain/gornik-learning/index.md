---
type: guide
title: Gornik Method — Anti-Brain-Rot AI Learning for Engineering
description: Aleks Gornik's 4-method AI learning workflow from "How I use AI to learn engineering (anti brain rot studying)": visualize concepts, filter resources, find the 20%, and find the best resources — using AI as a filter, not a replacement for thinking.
tags: [gornik, learning, ai, engineering, visualization, filtering, 80-20, resources, brain-rot]
timestamp: 2026-11-04
---

# Gornik Method — Anti-Brain-Rot AI Learning for Engineering

Back to [Brain Index](/index.md). Related: [Alvar Method](/alvar-method/index.md), [Huberman Learning](/huberman-learning/index.md), [Core Domain Contracts & Algorithms](/architecture/core-domain.md).

> **Source:** Aleks Gornik, *How I use AI to learn engineering (anti brain rot studying)* — [youtu.be/GPM5MgFEkpk](https://youtu.be/GPM5MgFEkpk). Channel: [youtube.com/@aleksgornik](https://www.youtube.com/@aleksgornik). Website: [aleksgornik.com](https://aleksgornik.com).

## What This Is

Aleks Gornik (recent EE graduate, software engineer at an AI startup working on LLM training) presents 4 practical methods for using AI to learn hard engineering concepts fast — without the cognitive degradation ("brain rot") that comes from letting AI do the thinking for you.

The thesis: AI gets you unstuck in minutes, not by doing the work for you, but by **showing concepts visually, filtering noise from signal, identifying the essential 20%, and finding the best resources**. AI is a filter and accelerator, not a replacement for your own thinking.

This is the practical engineering counterpart to the [Alvar Method](/alvar-method/index.md) (which is the tutoring loop) and the [Huberman Learning](/huberman-learning/index.md) (which is the neuroscience). Where Alvar provides the pedagogy and Huberman provides the biology, Gornik provides the tooling.

## Documents

- [The Four AI Methods](/gornik-learning/four-methods.md) — Detailed breakdown of all 4 methods with examples, prompts, and warnings.
- [Full Transcript](/gornik-learning/transcript.md) — Complete auto-caption transcript with timestamps.

## How I Watched This Video

1. **Metadata** via YouTube oembed — title ("How I use AI to learn engineering (anti brain rot studying)"), author (Aleks Gornik), channel (youtube.com/@aleksgornik).
2. **Transcript** — fetched via `youtube-transcript-api` Python package (installed on this machine). Full auto-caption transcript with timestamps.
3. **Visuals** — the video is a screen-recording demo format. Aleks shows live examples of Claude Code generating 3D models, interactive wiring dashboards, and sub-agent internet crawls. Thumbnail checked via oembed.
4. **Context** — fetched Aleks's website (aleksgornik.com) and his kit.com blog posts to establish background: EE graduate, internships in AI/hardware/software, now Member of Technical Staff at an AI startup working on LLM training.

## The Four Methods (Summary)

### 1. Make It Visual
Use AI (Claude Code, GPT Codex) to generate interactive 3D models and presentations. Examples: robotics project 3D model, STM32 wiring dashboard. Better models (more expensive tokens) are much better at 3D.

### 2. Filter Resources with AI
Use AI to decide what's actually worth reading. University: 5 books × 1000 pages → which chapters match your syllabus? Work: 5 papers → which is best to read? Saves hours.

**The prompt:** "Here's my syllabus, here is the reading list. Which books and chapters are actually worth my time considering my specific curriculum and exams? Which tutorial problems are worth doing?"

### 3. Find the Essential 20%
You don't need to know everything. Use AI to identify the 20% that gets you unstuck and moving. Story: graph neural network thesis — broke "deep learning" down to "just get a training loop going in PyTorch." Didn't know backprop deeply, but it was enough to get going.

### 4. Find the Best Resources
Use AI sub-agents to crawl the internet for high-quality resources. CAD example: found NASA 3D models of Mars rovers, satellite .stl files — all free, open-sourced. 10 sub-agents crawling the internet beats 10 hours of manual searching.

**Warning:** AI hallucinates — always click links and double-check that resources are legit.

## Relevance to LearnPuppy

| Gornik method | LearnPuppy implementation |
|---|---|
| Visualize concepts (3D models, dashboards) | `lp-agent` `visual` subagent (self-reviewed SVG) |
| Filter resources (which to read) | `lp-agent` `research` subagent (fact-checking) |
| Find the 20% (essential to get going) | `lp-core` `Probe` phase (find the edge of understanding) |
| Find the best resources (sub-agents) | `lp-agent` `research` subagent + `lp-ingest` transcript ingestor |
| AI as filter, not replacement | Alvar Method: struggle stays in the material, system absorbs logistics |
| 80/20 principle | `lp-core` `Plan` phase (DAG: one reasoning step per node, not the whole field) |
| Better models for 3D | `lp-cli-driver` multi-account pool (route visual tasks to stronger models) |
| AI hallucination warning | Alvar Method `learn-verify` skill (fact-check before teaching) |

## Relationship to Alvar Method and Huberman Learning

| Gornik | Alvar Method | Huberman Learning |
|--------|-------------|------------------|
| Make it visual | `learn-visual` skill (self-reviewed SVG) | Visual focus → mental focus (Step 2 of Super-Protocol) |
| Filter resources | `learn-verify` skill (fact-check) | Self-testing offsets forgetting (familiarity ≠ mastery) |
| Find the 20% | Probe phase (find the edge) | Effort at the edge (~15% error rate optimal) |
| Find the best resources | Plan phase (DAG, fact-check subagents) | Testing soon after first exposure |
| AI as filter not replacement | "Struggle stays in the material" | "Effort is the cornerstone of learning" |
| Get unstuck fast | One reasoning step per turn | Gap effects (10-sec pauses, 20-30x neural reps) |
