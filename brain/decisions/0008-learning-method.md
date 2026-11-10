---
type: adr
title: "ADR-0008: Learning Method Enforced In Code, Numbers As Settings"
description: The Alvar loop and Huberman session conditions run as server-enforced rules on the cheap tutor, and every number from the sources ships as a tunable setting rather than a fact.
tags: [adr, learning-method, bout, settings, enforcement]
timestamp: 2026-11-10
---

# ADR-0008: Learning Method Enforced In Code, Numbers As Settings

Back to [Decisions Index](/decisions/index.md) | [Brain Index](/index.md). Related: [Learning Method Plan](/learning-method/index.md), [Task DAG](/learning-method/task-dag.md), [ADR-0007](/decisions/0007-course-as-data.md).

## Status

**Accepted** (2026-11-10), recording what M1 to M13 built. This is the M0 record, written after the code because the defaults it names already shipped as settings. Gate G3 result: see [G3 result](/learning-method/g3-result.md). G4 is open and owner-only.

## Context

1. Alvar decides what happens in a session and when something counts as learned. Huberman decides the conditions of a session: length, rest, difficulty, pauses.
2. The sources disagree in six places and the Huberman numbers are partly unverified (see Evidence limits in the [Plan](/learning-method/index.md)).
3. A method that lives only in prompts or only in the UI is skipped the first time it is inconvenient.

## Decision

1. **Enforcement lives in code, on the server.** `lp-core` decides the next step and rejects actions that break a rule. The server refuses `POST /courses/{id}/step` with `409 conflict` and a stable `details.gate` code when there is no open bout (`bout_required`), when the clock is inside the gap (`gap`) or the day is spent (`daily_limit`). A bout that ran out of time may only be closed: `step` then returns `close`. The web app shows the same states (check-in, ready gate, lock, clock, gap pauses) but a client that skips them still gets nothing.
2. **Every number is a setting.** 15 settings, each with a default and a range, are readable and writable per workspace through `GET/POST /workspaces/{ws}/learning/settings` and the Study method section of workspace settings. The route validates range and keeps the error band consistent (`error_band_low_pct` below `error_band_high_pct`). The teach loop reads the workspace's values, not the built-in defaults.
3. **The six conflicts ship the defaults in the [Plan](/learning-method/index.md)**: parking after 3 failed attempts (`attempts_before_parking`), a hard 90-minute lock with one logged override a day (`bout_limit_minutes`), and the rest as fixed rules.
4. **Bouts are workspace-wide.** Minutes today count across courses; the clock is folded from the log, so a refresh or restart cannot reset it.

## Consequences

- Changing a number needs no code change, and a wrong number is a one-click fix.
- The lock can be escaped only through the typed-reason override, which is logged.
- Self-reported inputs (sleep, phone away) are recorded, never verified.
- The sources' numbers stay unverified until G4 says this owner learns better with them. A gate that annoys without helping is cut.
