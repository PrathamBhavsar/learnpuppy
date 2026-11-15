---
title: Review
group: Use
order: 7
description: FSRS spaced repetition, review sessions, the card browser and Digest.
---

# Review

Cards come from what you learned in chats and from decisions found by ingest. They are scheduled with FSRS v5: the default weights, a 0.9 target retention, short-term steps on and fuzz off. It is a port of the ts-fsrs build.

## A review session

Open Review and press Start review (the button shows how many are due). Sessions are full-screen focus mode.

1. Read the card and choose an answer with `1` to `4` or `a` to `d`.
2. See whether you were right and read the explanation.
3. Rate it: Again, Hard, Good or Easy. Each button previews the next interval. `Space` accepts the suggested rating.

`Esc` leaves, with a confirmation if you are mid-card. A summary shows counts by rating, time and accuracy.

## Stats and forecast

The strip at the top shows due now, new today, reviewed today and your streak. A 14-day forecast of due cards collapses away when you do not need it.

## Card browser

Search and filter cards by state (new, learning, review, relearning) and due date. Open one to see its review history, with stability and difficulty values under Details.

## Review all

Walks every workspace's due cards in turn, with a separator between workspaces. Note: review-all has been checked only against the scripted engine and mocks.

## Digest

Digest lists the decisions that [ingest](#/accounts) found in your past AI sessions, grouped by day. Open one to see what was chosen, the basis, the alternatives considered, related git commits and the cards generated from it. "Review these" starts a session on just those cards. "Regenerate cards" rebuilds them.

Ingest scans claude, codex and agy session transcripts and git history in folders you allow (an allow-list with a Detect projects helper), over the last 7, 30 or 90 days or all time. Secrets are redacted before anything is distilled. Run is disabled while the allow-list is empty, and overlaps with another workspace are flagged.
