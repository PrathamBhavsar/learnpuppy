"""Weighted quota ceiling for the G4 gate. Pure and deterministic, no model calls.

Tokens count by model weight: Sonnet = 1, Opus = 5, Haiku or Flash = 1/3
(Opus is 15x Haiku). The gate fails loudly when a cheap role runs on a strong
model, or when the weighted total passes the ceiling.

    python3 bench/cheap_tutor/quota.py     # runs the self tests
"""
from __future__ import annotations

import unittest

# Sonnet-equivalent tokens per study hour. A normal hour (2 Opus author calls
# plus 22 cheap calls, under 100,000 raw tokens) weighs about 71,000.
WEIGHTED_CEILING = 150_000
CHEAP_ROLES = {"distiller", "placer", "follow_up", "extractor", "auditor"}


def weight(model: str) -> float:
    m = model.lower()
    if "opus" in m:
        return 5.0
    if "haiku" in m or "flash" in m:
        return 1 / 3
    return 1.0  # Sonnet and anything unknown


def is_strong(model: str) -> bool:
    return weight(model) > 1.0


def check(calls: list[dict]) -> list[str]:
    """calls: {role, model, tokens}. Returns failures, empty when the gate passes."""
    fails = []
    total = 0.0
    for c in calls:
        total += c["tokens"] * weight(c["model"])
        if c["role"] in CHEAP_ROLES and is_strong(c["model"]):
            fails.append(f"role leak: {c['role']} ran on {c['model']}")
    if total > WEIGHTED_CEILING:
        fails.append(f"weighted quota {total:.0f} over ceiling {WEIGHTED_CEILING}")
    return fails


class QuotaTests(unittest.TestCase):
    def hour(self):
        calls = [{"role": "author", "model": "claude-opus-5-5", "tokens": 8000}] * 2
        roles = ["distiller", "placer", "follow_up"] * 7 + ["placer"]
        calls += [{"role": r, "model": "claude-haiku-5", "tokens": 1500} for r in roles]
        return calls

    def test_normal_hour_passes(self):
        self.assertEqual(check(self.hour()), [])

    def test_role_leak_fails(self):
        calls = self.hour() + [{"role": "distiller", "model": "claude-opus-5-5", "tokens": 100}]
        self.assertTrue(any("role leak" in f for f in check(calls)))

    def test_ceiling_fails(self):
        calls = [{"role": "author", "model": "claude-opus-5-5", "tokens": 40000}] * 2
        self.assertTrue(any("ceiling" in f for f in check(calls)))


if __name__ == "__main__":
    unittest.main()
