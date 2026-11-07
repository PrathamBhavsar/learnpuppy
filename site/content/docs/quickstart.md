---
title: Quickstart
group: Start
order: 3
description: From a built binary to your first learning chat and first review.
---

# Quickstart

## 1. Check your setup

```bash
learnpuppy doctor
```

It reports the data folder, port, disk space and which CLI logins were found. You need at least one login. If none is found, log in to a CLI first, for example `claude login`.

## 2. Start the server

```bash
learnpuppy serve
```

It prints a one-time launch URL and opens your browser. Open that URL, not the bare address: the launch token is what lets the browser in. A second `serve` reuses the running server and prints the same URL. Ctrl-C stops it, and jobs that were running resume on the next start.

## 3. First run

The home screen walks you through three steps on one page:

1. **Accounts.** The CLI logins that were detected. If there are none, it shows the command to run, with a Rescan button.
2. **Name your first workspace.** Pick an icon and a color.
3. **Choose sources.** Detected project folders to ingest from. You can skip this.

"Create and start ingest" scans those sources for past AI sessions and git history.

## 4. Learn something

Open **Learn**, start a **Teach** chat and type a topic. The chat probes first, then shows the plan, then teaches. See [Learn](#/learn).

## 5. Review

When a chat finishes, its cards join the queue. Open **Review**, press Start review, and rate each card with keys 1 to 4. See [Review](#/review).

## Try it without an AI account

```bash
learnpuppy serve --dev
```

Development mode uses a scripted engine and no launch token. It is for trying the interface and for tests, not for real study, and any local process can reach its API.
