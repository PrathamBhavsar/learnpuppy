---
title: FAQ
group: Reference
order: 11
description: Status, what was verified, limits, and the hero animation.
---

# FAQ

## Is it ready to use?

It is pre-release: version 0.1.0, no tagged release, and the repository is currently private. Build from source today. The one-line installer and release downloads start working when the repo is public.

## What leaves my machine?

Nothing, except the calls your own `claude`, `codex` and `agy` CLIs make. See [Security](#/security).

## Do I need an API key?

You need at least one logged-in agent CLI. LearnPuppy has no login or key of its own.

## What is verified and what is not?

Verified on real CLIs: Probe, Plan, Teach, Practice, workbench jobs (research, visual, PDF), ingest and Digest.

Not verified, or limited:

- `/review-all` was only checked against the scripted engine and mocks.
- agy was out of quota during verification, so agy as the teacher and its token counts are untested. Failover away from it is tested.
- The graph legend's claim, concept, procedure and decision kinds are cosmetic.
- codex and agy turns are read-only sandboxed, which still lets them read your files.

## What is the animation on the home page?

A **synthetic illustration**. The curve is made up: recall decays, then jumps back up at each review, and the gaps between reviews grow. The percentage printed beside it comes from that made-up curve. It is not measured data and not a result from LearnPuppy. FSRS is the real scheduler; the curve only shows the idea.

## Which scheduling algorithm?

FSRS v5 with default weights, a 0.9 target retention, short-term steps on and fuzz off.

## Which platforms?

macOS and Linux. Releases target arm64 and x86_64 on both.

## Where is my data?

In `~/.learnpuppy`, or `LEARNPUPPY_HOME`. Deleted workspaces go to `.trash` first, and you can export any workspace as a zip.

## Which license?

MIT, as set in the workspace `Cargo.toml`. The repository has no separate license file yet.
