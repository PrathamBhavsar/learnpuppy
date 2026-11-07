---
title: Accounts
group: Setup
order: 8
description: The CLI account pool, role slots and failover.
---

# Accounts

LearnPuppy has no login of its own. It runs the agent CLIs already installed and logged in on your machine, and spreads work across them.

## The pool

- **Claude:** every profile it finds, `~/.claude` plus `~/.claude-accounts/*`, and any extra folders you add in Accounts. Each profile is its own rate-limit bucket.
- **codex** and **agy:** one login each.
- **kiro-cli** and **opencode** are picked up too when installed and logged in.

The Accounts screen shows each account's status, usage and cooldown, with Enable and Disable. For an account that needs a login it shows the exact command to run, with a copy button. Rescan re-reads the logins.

## Failover

Failures are classified, and each kind has its own reaction:

| Failure | Reaction |
| --- | --- |
| Not installed, or not logged in | The account is marked missing or needs login. |
| Rate limited | The account cools down until its reset time, or a backoff of 60 seconds to 1 hour when unknown. Others keep serving. |
| No such model | That one model is benched. Sibling models keep serving. |
| Offline | A global backoff of 5 to 120 seconds. No account is cooled. |
| Timed out, other | No state change. |

When every account is busy, the chat shows a banner with a live countdown.

## Role slots

Each role chooses provider, model, effort and a fallback: prober, teacher, practice, extractor, auditor, and the workbench roles pdf, research, visual and demo. A Test button sends a one-token ping through the chosen profile. Per-workspace overrides live in each workspace's settings, defaulting to inherit global.

## Concurrency

Settings has a concurrency cap from 1 to 6.

## Known limit

agy was out of quota when this was verified, so agy as the teacher, and its token counts, are untested. Failover away from agy is tested.
