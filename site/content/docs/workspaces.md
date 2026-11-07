---
title: Workspaces
group: Use
order: 4
description: Isolated knowledge bases for separate subjects.
---

# Workspaces

A workspace is an isolated knowledge base: "Rust async", "Distributed systems course", "Work repo X". Each has its own:

- Knowledge Graph
- chats, workbench jobs and artifacts
- spaced-repetition cards
- decisions and ingest cursors
- full-text search index

Shared across all workspaces: the CLI account pool and rate limits, role settings, and UI preferences.

## Managing them

Create, rename, change the icon and color, archive, delete and restore. Each workspace card on the home screen shows its nodes, chains and chats, and a badge for due cards.

- **Archive** makes a workspace read-only.
- **Delete** asks you to type the name, aborts running work, and moves the folder to `.trash` inside your data folder. Restore it from Settings, or purge it.
- **Export** produces a zip with the workspace metadata, graph, chats and a checksummed manifest. **Import** validates that zip and creates a new workspace. It never overwrites one.

## Switching

Use the workspace chip at the top of the sidebar, `Cmd 1` to `9`, or the command palette. The section you were in is remembered. Two browser tabs can sit on two workspaces independently.

## On disk

```text
~/.learnpuppy/
  state.json  settings.json  server.lock  logs/  .trash/
  workspaces/<id>/
    workspace.json  graph/  chats/  jobs/  artifacts/
```

Set `LEARNPUPPY_HOME` to use a different folder.

## Coming from the single-store version

If your data folder has `graph` or `chats` at its root and no `workspaces`, `learnpuppy migrate` moves them into a workspace named "Default". Entries are moved, not copied. `learnpuppy migrate --revert` undoes it. See [CLI](#/cli).
