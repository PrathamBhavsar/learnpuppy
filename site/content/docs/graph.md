---
title: Knowledge Graph
group: Use
order: 6
description: The cross-session graph, how to explore it, and practice from a selection.
---

# Knowledge Graph

Every learning chat and every ingest run adds propositions (short statements of what you learned) and chains (how they connect) to one graph per workspace. It spans sessions, so a new topic can link to an old one.

## Explore

- Search is full-text. Matches highlight and everything else dims. `Enter` flies to the first match.
- Filters: source (claude, codex, agy, git, chat), cluster or session, and minimum degree.
- Toggle cluster outlines, labels (auto, always, never) and a minimap.
- Pan by dragging, zoom with the wheel or pinch, select by clicking. `Shift` or `Cmd` click multi-selects, and `Shift` drag draws a marquee.
- Keyboard: arrows move to the nearest neighbor, `Tab` cycles, `Enter` opens the drawer, `Esc` clears, `f` fits, `+` and `-` zoom.
- Below the canvas, a focusable list of nodes mirrors the selection for screen readers.

URLs keep your place: `?node=`, `?q=`, `?src=` and `?cluster=`.

## Node drawer

Select a node to read its full text, where it came from and when, the chat it was learned in, its incoming and outgoing chains, and its linked cards. Node text is read-only: the graph is append-only, so you change it by learning, or delete it.

## Chain table

A table view with three levels (groups, chains, propositions). Sort it, scope it to the selection, and export CSV.

## Practice from selection

Select nodes and press Practice. LearnPuppy creates a Practice chat on exactly those ideas and opens it.

## Deleting

Delete shows a preview of what would be removed, including orphaned chains, and requires an acknowledgement when chains would be orphaned.

## Known limit

The legend's claim, concept, procedure and decision kinds are cosmetic. The data carries only the source kind.
