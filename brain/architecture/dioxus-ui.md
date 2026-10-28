---
type: spec
title: Dioxus 0.7 & lp-ui Visual Desktop Architecture
description: Deep-dive specification for LearnPuppy's native macOS Dioxus 0.7 desktop UI (lp-app) and shadcn/Radix-style Rust component library (lp-ui), including library usage rules and banned patterns.
tags: [architecture, dioxus, lp-ui, lp-app, shadcn, desktop, macos, svg, graph]
timestamp: 2026-10-28
---

# Dioxus 0.7 & `lp-ui` Visual Desktop Architecture

Back to [Architecture Index](/architecture/index.md) | [Brain Index](/index.md). Related: [ADR-0005 Dioxus Desktop Over TUI](/decisions/0005-dioxus-desktop-over-tui.md), [Anti-Patterns](/wisdom/anti-patterns.md), [Knowledge Graph](/architecture/knowledge-graph.md).

## 1. Platform Rollout & Branch Isolation Strategy

- **Legacy CLI/TUI Preserved in Isolation**: The completed 6-crate Ratatui TUI + CLI implementation is frozen and preserved on git branch `legacy/tui-cli`.
- **Target 1 (Immediate Deliverable — Active)**: **macOS Native Desktop (`aarch64-apple-darwin`)** built with `dioxus = { version = "0.7", features = ["desktop"] }` (`wry` / macOS `WKWebView`), plus a dual-mode `--e2e-port` local web harness for `tester-army/e2e` browser/agentic verification.
- **Target 2 (Next in Roadmap — `TODO`)**: **Windows Desktop (`x86_64-pc-windows-msvc` / WebView2)** (`T70`).
- **Target 3 (Final in Roadmap — `TODO`)**: **iOS & Android Mobile Companion (`dioxus` `mobile` feature)** (`T80`), syncing `nodes.jsonl`, `chains.jsonl`, and `sync-<device>.jsonl` from the desktop over LAN/folder sync so users can review FSRS cards and explore their Knowledge Graph on mobile.

---

## 2. Two-Crate UI Split: `lp-ui` vs `lp-app`

To make LearnPuppy a standout Rust portfolio piece, the UI is split into two focused crates:

### 2.1 `crates/lp-ui` — Pure-Rust `shadcn/ui` + `dioxus-primitives` Component Library
Synthesizes the best parts of `DioxusLabs/components` (`dioxus-primitives` controlled/uncontrolled signal patterns and ARIA attributes) and `rust-ui/dioxus-ui` (OKLCH dark-mode design tokens and shadcn class ergonomics) while eliminating all inline-JS and `wasm-bindgen` anti-patterns:

- **`theme.rs`**: Injects the embedded dark-first OKLCH stylesheet (`LP_THEME_CSS`) defining `--background`, `--foreground`, `--card`, `--popover`, `--primary`, `--secondary`, `--muted`, `--accent`, `--destructive`, `--success`, `--warning`, `--info`, `--border`, `--ring`, and mastery ring tokens (`--mastery-novice`, `--mastery-learning`, `--mastery-mastered`).
- **`icons.rs`**: Zero-dependency pure-Rust SVG icon components (`IconGraph`, `IconClassroom`, `IconDigest`, `IconSparkles`, `IconCheck`, `IconX`, `IconChevronRight`, `IconChevronDown`, `IconSearch`, `IconSettings`, `IconTerminal`, `IconBook`, `IconBrain`, `IconRefresh`, `IconPlay`, `IconShield`, `IconPlus`, `IconTrash`) rendering crisp 24x24 `stroke="currentColor"` Lucide paths.
- **Primitive & Composite Components**:
  1. `Button` (`ButtonVariant::{Primary, Secondary, Outline, Ghost, Destructive, Link}`, `ButtonSize::{Xs, Sm, Default, Lg, Icon}`)
  2. `Badge` (`BadgeVariant::{Default, Secondary, Success, Warning, Destructive, Outline, Phase}`)
  3. `Card`, `CardHeader`, `CardTitle`, `CardDescription`, `CardContent`, `CardFooter`
  4. `Dialog`, `DialogContent`, `DialogHeader`, `DialogTitle`, `DialogDescription`, `DialogFooter` (pure Rust `Signal<bool>` open state + `onkeydown` `Escape` handler + backdrop click dismiss)
  5. `Tabs`, `TabsList`, `TabsTrigger`, `TabsContent` (pure Rust `Signal<String>` active tab context)
  6. `Tooltip` (CSS group-hover + ARIA `role="tooltip"` zero-JS popover)
  7. `Progress` & `MasteryRing` (SVG circular and linear progress indicators)
  8. `Slider` (1..5 Confidence selector & parameter explorer slider)
  9. `Switch` & `Checkbox` (controlled `Signal<bool>` toggles)
  10. `ScrollArea` (thin dark-mode custom scroll container)
  11. `SplitPane` (resizable horizontal/vertical desktop workspace divider)
  12. `CommandPalette` (`Cmd+K` / `Ctrl+K` fuzzy command and proposition search modal)
  13. `CodeBlock` (`pulldown-cmark` + `syntect` syntax-highlighted HTML/RSX renderer with copy button and language badge)

### 2.2 `crates/lp-app` — Native macOS Visual Learning Application
Connects `lp-ui` directly to `lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, and `lp-ingest`:

1. **Left Icon Rail & Quick-Action Toolbar**:
   - One-click visual navigation with icons + tooltips + shortcut badges:
     - `Graph Home` (`Cmd+1`) — Interactive Knowledge Graph & Chain Table
     - `Classroom` (`Cmd+2`) — Prober/Teacher Split View, Mermaid Plan DAG, & Workbench Sub-Agents
     - `FSRS & Digest` (`Cmd+3`) — 4-Choice Spaced Repetition & Local CLI Transcript Decisions
     - `CLI Accounts & Roles` (`Cmd+4` / `Cmd+A`) — Live Multi-Account Claude (`#1`, `#2`), Codex, and AGY status pills
2. **Interactive SVG Knowledge Graph Canvas (`views/graph_canvas.rs`)**:
   - The visual centerpiece of LearnPuppy. Renders `GraphSnapshot` propositions (`n_...`), implication junctions (`arg_...`), directed cubic-bezier edges, and birth-session cluster hulls inside a hardware-accelerated `<svg>` viewport.
   - Supports smooth mouse wheel zoom (`onwheel`), click-and-drag pan (`onmousedown`/`onmousemove`/`onmouseup`), node click provenance inspection, multi-node selection basket (`Space` or checkbox click) to launch an Adaptive Practice Session, and toggleable 3-level Collapsible Chain Table (`Cluster -> Premise A ∧ Premise B ⟹ Conclusion C -> Statements`).
3. **Visual Classroom & Plan DAG Split View (`views/classroom.rs`)**:
   - **Left Session Drawer**: Filterable `Teaching | Practice | All` sessions with phase badges (`PROBE`, `PLAN`, `TEACH`, `PRACTICE`) and live Sub-Agent workbench cards (`researcher`, `pdf-reader`, `visual-maker`, `demo-builder`).
   - **Center Lesson Stream**: Concise visual blocks (headings, tables, syntax-highlighted Rust/System Design code, inline SVG diagrams) followed by the **Interactive Question Dock** (clickable `[A]..[D]` option cards, `[I] I don't know` pill, `1..5` Confidence selector, dual `Reading Notes` / `Answer Reasoning` tabs, and instant graded verdict banner).
   - **Right Plan DAG Viewport**: Renders the session's `PlanTree` dependency graph as an interactive visual node-link diagram with glowing status rings (`● Current` in cyan, `✓ Visited` in emerald, `○ Upcoming` in slate).
4. **FSRS 4-Choice Review & CLI Transcript Digest (`views/digest_fsrs.rs`)**:
   - **FSRS Review Deck**: Visual flashcard runner with mastery rings, clickable 4-option cards, rationale reveal, and 1-click FSRS grading (`Again`, `Hard`, `Good`, `Easy`).
   - **CLI Coding Digest**: Browse ingested sessions from `claude` (Account 1 & Account 2), `codex`, and `agy` across `/Users/pratham/code/tracker` and `/Users/pratham/code/bunfiiliate`, inspect distilled architectural `DecisionRecord`s, and click **"Fold into Graph"** or **"Mint Cards"**.

---

## 3. How To Use Dioxus 0.7 vs What To Avoid Completely

| Pattern | DO (Production `lp-ui` / `lp-app` Rule) | NEVER DO (Banned Anti-Pattern) |
|---|---|---|
| **Component Interactivity** | Drive all state via Dioxus `use_signal`, `use_memo`, and `use_context`. Handle `onclick`, `onkeydown`, `oninput`, `onwheel` in Rust closures. | **NEVER** emit `script { dangerous_inner_html: "..." }` with `setTimeout` DOM polling (used in `rust-ui/dioxus-ui`). |
| **Target Portability** | Use `dioxus::prelude::*` events (`MouseEvent`, `KeyboardEvent`, `WheelEvent`, `FormEvent`) that compile cleanly on `desktop` (`aarch64-apple-darwin`), `web`, and `mobile`. | **NEVER** import `wasm_bindgen`, `web_sys`, `js_sys`, or `gloo_timers` in `lp-ui` or `lp-app` — they fail to compile on native macOS desktop! |
| **Async & SQLite Calls** | Spawn async `LlmEngine` turns via `spawn(async move { ... })` and run blocking `rusqlite` / `std::fs` work inside `tokio::task::spawn_blocking`. | **NEVER** block the main Dioxus VirtualDOM thread with synchronous SQLite queries or subprocess waits. |
| **Window Layout & Scroll** | Root `div` uses `h-screen w-screen overflow-hidden select-none flex bg-background text-foreground`; scroll only inside child `ScrollArea` panes. | **NEVER** let `html` or `body` scroll on desktop (causes double scrollbars and rubber-banding in macOS WKWebView). |
| **Form Submissions** | Always call `evt.prevent_default()` on `onsubmit` handlers. | **NEVER** allow raw HTML `<form>` submission to trigger a WKWebView navigation reload. |
