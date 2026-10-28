---
name: dioxus-desktop-ui
description: Mandatory architecture, Dioxus 0.7 patterns, shadcn-style lp-ui component rules, and things to avoid when building LearnPuppy's native macOS desktop UI (lp-ui and lp-app).
---

# Dioxus 0.7 Desktop & `lp-ui` Engineering Rules

Use this skill whenever creating or modifying code in `crates/lp-ui` or `crates/lp-app`.

## 1. How To Use Dioxus 0.7 Properly

1. **Pure Rust State & Reactivity (`use_signal`, `use_memo`, `use_resource`, `use_context`)**:
   - All modal open states, tab selections, dropdown visibility, focus states, and canvas pan/zoom transforms (`pan_x`, `pan_y`, `zoom`) MUST live in Dioxus `Signal<T>` or `Memo<T>`.
   - Share global app handles (`Arc<DesktopBackend>`, `Arc<AppState>`) via `use_context_provider` at the root `App` component.
2. **Async Background Bridge (`spawn` + `tokio::task::spawn_blocking`)**:
   - The Dioxus VirtualDOM runs on the main OS thread (`!Send`).
   - Run async `LlmEngine` streaming turns inside `spawn(async move { ... })` and push incremental deltas into `Signal`s.
   - Run synchronous `rusqlite` or `std::fs` calls inside `tokio::task::spawn_blocking` before updating signals so the macOS WKWebView frame rate stays locked at 60fps.
3. **Component Architecture (`lp-ui`)**:
   - Follow the `DioxusLabs/components` (`dioxus-primitives`) and `shadcn/ui` dark-mode token architecture (`--background`, `--foreground`, `--card`, `--primary`, `--muted`, `--accent`, `--border`, `--ring`).
   - Use strongly-typed Rust enums for variants and sizes (`ButtonVariant`, `ButtonSize`, `BadgeVariant`) with `#[props(extends = GlobalAttributes)]` for clean attribute forwarding.
   - Use inline SVG icon components (`crates/lp-ui/src/icons.rs`, Hugeicons paths) with `width`/`height`/`stroke="currentColor"`.
4. **Interactive SVG Viewport for Knowledge Graph & Plan DAG**:
   - Render the Knowledge Graph and Plan DAG using a pure-Rust interactive `<svg>` canvas (`onwheel` zoom, `onmousedown`/`onmousemove`/`onmouseup` pan, `onclick` node selection, cubic-bezier edges, cluster hulls, mastery rings).
5. **Dual-Target Dev & E2E Harness (`LP_E2E_PORT`)**:
   - `lp-app` supports both native macOS desktop (`dioxus::desktop::launch`) and local web server (`LP_E2E_PORT=<port>`) so `tester-army/e2e` (`@e2e-dev/web` + Playwright) can drive every screen deterministically.
   - Attach stable `data-testid` and semantic ARIA roles (`role="dialog"`, `role="tab"`, `role="status"`, `aria-label`) to every interactive control.

## 2. Things To Avoid Completely (Hard Bans)

1. **NEVER use `script { dangerous_inner_html: ... }` or `setTimeout` DOM polling** (the `rust-ui/dioxus-ui` anti-pattern) to wire clicks, dialogs, tabs, or Escape listeners.
2. **NEVER import `wasm_bindgen`, `web_sys`, `js_sys`, or `gloo_timers`** inside `lp-ui` or `lp-app`. They fail to compile on native macOS desktop targets (`aarch64-apple-darwin`).
3. **NEVER allow body/window double scrollbars**: Root shell must be `h-screen w-screen overflow-hidden select-none flex flex-col bg-background text-foreground`. Constrain scrolling to internal panes (`overflow-y-auto`).
4. **NEVER omit `event.prevent_default()` on `<form onsubmit>`**: Unhandled form submits cause WKWebView to reload the page and wipe app state.
5. **NEVER depend on `lp-tui` (`ratatui`/`crossterm`) from `lp-ui` or `lp-app`**: The legacy TUI is isolated on branch `legacy/tui-cli`.
