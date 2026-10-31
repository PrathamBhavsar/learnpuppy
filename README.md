# LearnPuppy

Local learning web app: a visual learning harness, a cross-session Knowledge Graph, and FSRS spaced repetition, with multiple isolated workspaces.

`learnpuppy serve` runs a Rust (axum) server on `127.0.0.1` and serves a Vite + React + shadcn SPA from `web/`.

## Quick start
```sh
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo run -p lp-server -- serve     # open the printed 127.0.0.1 URL
make gate                           # fmt, clippy, test, OKF, OpenAPI check
```

## Where to read
- [`CLAUDE.md`](./CLAUDE.md) / [`AGENTS.md`](./AGENTS.md): project map and rules.
- [`docs/web/README.md`](./docs/web/README.md): the web UI spec.
- [`brain/index.md`](./brain/index.md): OKF knowledge bundle (architecture, ADRs, tasks).

The Dioxus desktop app and Ratatui TUI were removed (ADR-0006); TUI history is on branch `legacy/tui-cli`.
