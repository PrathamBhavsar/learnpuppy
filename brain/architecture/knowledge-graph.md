---
type: spec
title: Knowledge Graph & 3-Stage Extraction Pipeline
description: Chains-primary graph ledgers, SHA-256 message coverage fingerprints, digest-aware Extractor, Verdicts-Only Auditor, deterministic merger, and transitive simplify_chains.
tags: [graph, extraction, auditor, merger, simplify-chains, provenance]
timestamp: 2026-10-27
---

# Knowledge Graph & 3-Stage Extraction Pipeline

Back to [Architecture Index](/architecture/index.md). Related: [Core Domain](/architecture/core-domain.md), [Storage & Memory Model](/architecture/storage-memory.md).

## 1. Chains-Primary Data Model

The Knowledge Graph stores **world-facing true propositions** (`GraphNode`, `id: "n_<12hex>"`) and **jointly-entailing reasoning chains** (`GraphChain`, `id: "arg_<12hex>"`, `premises: Vec<NodeId>`, `conclusion: NodeId`).
- **Node Link-Appends**: In `nodes.jsonl`, the first line for an `id` defines `name`, `statement`, and `created_at`. Subsequent lines for the same `id` (written when the Auditor links a new proposition to an existing node) append their `statement` and `aliases` to `aliases` and union their `provenance`.
- **Chain Merging**: In `chains.jsonl`, lines with the same canonical key `sorted(premises).join("+") + ">" + conclusion` merge at read time under the first line's `arg_...` ID, unioning `provenance`.

## 2. Two Front Doors & Message Coverage Ledger (`coverage.jsonl`)

1. **Session Front Door**:
   - After every teacher turn (or quiz answer), `uncovered_messages` computes `sha256(message_view_lines)` for each teacher passage (`TEACHER [msg_id]`).
   - Passages whose `(chat_id, fp)` are absent from `coverage.jsonl` are grouped into batches (`<= 12_000` estimated tokens).
   - If a learner later answers a quiz attached to a message, that message's view lines change -> its SHA-256 fingerprint changes -> only that message automatically re-opens for extraction.
2. **Format-Agnostic Notes / Book Front Door (`src_<hash>`)**:
   - Imports Markdown files, PDFs, or distilled `tracker` decisions using deterministic source ID `"src_" + sha256(file_path)[..12]`.

## 3. Three-Stage Extraction Pipeline (`lp-agent::extraction`)

### Stage 1: Digest-Aware Extractor (`run_submit_agent`, max 4 turns)
- Per-source serial queue (`chat_id` or `src_id`).
- Prompt includes the uncovered passage batch PLUS `"THE GRAPH SO FAR"` (all propositions already extracted from this same source).
- Calls `submit({ propositions: Vec<ExtractedProposition>, chains: Vec<ExtractedChain> })`.
- May reference existing same-source node IDs (`n_...`) directly as premises or conclusions in `chains`.

### Stage 2: Verdicts-Only Auditor (`run_submit_agent`, max 4 turns, global FIFO mutex)
- Runs inside the global audit-merge critical section so two concurrent sessions can never race on duplicate node creation.
- **Never sees the raw transcript and never sees or mints chains.**
- Uses read-only graph tools (`find` via SQLite FTS5, `list_sessions`, `list_propositions`) to check if any newly extracted proposition is equivalent to an existing `GraphNode`.
- Calls `submit({ links: Vec<{ id: SmolStr, link_to: SmolStr }> })` with an **equivalence bias** (paraphrases of the same claim merge).

### Stage 3: Deterministic Merger & `simplify_chains` (`lp-core::graph`)
- Resolves each `ExtractedProposition`: if `link_to` points to a live node, appends a link line to `nodes.jsonl`; otherwise mints a new `n_<12hex>` node.
- Resolves `ExtractedChain` endpoints (dropping self-loops `premise == conclusion` and unresolvable IDs) and appends new chains to `chains.jsonl`.
- Runs **`simplify_chains`** over all chains grouped by `conclusion` in greedy `(premises.len(), created_at, key)` order:
  1. **Prefer-Fewest-Premises**: Drops any candidate chain whose premise set is a strict superset of an already-kept chain for the same conclusion.
  2. **Transitive Ancestry-Cover**: Computes the cycle-guarded premise ancestry closure across all chains (`A ⟹ X` makes `A` an ancestor of `X`). Drops any candidate chain whose premises are all contained in `kept.premises ∪ ancestors(kept.premises)`.
- Rewrites `chains.jsonl` atomically if any redundant chain was dropped, then appends `CoverageEntry` records to `coverage.jsonl` and refreshes the `ArcSwap<GraphSnapshot>` + FTS5 index.
