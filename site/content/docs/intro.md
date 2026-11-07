---
title: Introduction
group: Start
order: 1
description: What LearnPuppy is, what it runs on, and what it is not.
---

# LearnPuppy

A local web app for learning with AI. It combines three things that usually live apart:

- **A tutor.** A chat that probes what you know, plans a route, then teaches with questions.
- **A Knowledge Graph.** Everything you learn is merged into one graph that spans sessions.
- **Spaced repetition.** FSRS schedules cards made from what you learned, so it comes back before it fades.

`learnpuppy serve` starts a Rust (axum) server on `127.0.0.1` and serves a React web app that is embedded in the binary. Nothing leaves your machine except the calls your own `claude`, `codex` and `agy` CLIs make.

> LearnPuppy is **pre-release**: version 0.1.0, no tagged release, and the repository is currently private. Release downloads work once the repo is public. Building from source works today. See the [FAQ](#/faq) for what has and has not been verified.

## What runs

| Part | Role |
| --- | --- |
| `learnpuppy` | One binary. `serve`, `doctor`, `migrate` and the embedded web app. |
| Your agent CLIs | `claude`, `codex`, `agy` do the AI work. kiro-cli and opencode are used too when installed. |
| `~/.learnpuppy` | Your data: per-workspace graph, chats, jobs, cards and search index. |

## Screens

| Screen | What it is for |
| --- | --- |
| Learn | Teach and Practice chats with the live plan beside them. |
| Graph | The Knowledge Graph: search, filter, select, practice. |
| Review | Due cards, the card browser, forecast and streak. |
| Digest | Decisions found in your past AI sessions, with their cards. |
| Ingest | Which sources to scan, and run history. |
| Accounts | The CLI account pool and role slots. |

Next: [install it](#/install).
