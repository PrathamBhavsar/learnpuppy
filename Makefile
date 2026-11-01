SHELL := /bin/zsh
export INSTA_WORKSPACE_ROOT := $(CURDIR)
export PATH := /opt/homebrew/bin:/opt/homebrew/opt/rustup/bin:$(HOME)/.cargo/bin:$(PATH)
CARGO ?= /opt/homebrew/opt/rustup/bin/cargo

.PHONY: fmt clippy test okf deny api-gen api-check gate ci disk-guard e2e perf release install

OPENAPI := web/openapi/openapi.json

fmt:
	$(CARGO) fmt --all -- --check

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

test:
	$(CARGO) test --workspace --all-targets

okf:
	python3 /Users/pratham/.claude/skills/okf/scripts/validate.py brain --strict

deny:
	@if command -v cargo-deny >/dev/null; then $(CARGO) deny check; else echo 'cargo-deny not installed, skipping'; fi

# Regenerate the committed OpenAPI document from the server's utoipa definitions.
api-gen:
	@mkdir -p $(dir $(OPENAPI))
	$(CARGO) run -q -p lp-server -- openapi --out $(OPENAPI)

# Fails when the committed (or staged) OpenAPI document differs from the server's.
api-check: api-gen
	@git ls-files --error-unmatch $(OPENAPI) >/dev/null 2>&1 || { echo '$(OPENAPI) is not tracked: run make api-gen and git add it'; exit 1; }
	@git diff --exit-code -- $(OPENAPI) || { echo '$(OPENAPI) is stale: run make api-gen and commit it'; exit 1; }

gate: fmt clippy test okf api-check

ci: gate deny

LP_WEB_PORT ?= 5173
export LP_WEB_PORT

disk-guard:
	@scripts/disk-guard.sh

# Full Playwright: mock-mode suites (journeys, visual, a11y, offline) + real-server suites (auth, learn, offline).
# Touching assets.rs forces rust-embed to pick up the freshly built web/dist.
e2e: disk-guard
	cd web && bun install --frozen-lockfile && bun run build
	touch crates/lp-server/src/assets.rs
	$(CARGO) build -p lp-server
	cd web && bunx playwright test
	cd web && LP_BIN=$(CURDIR)/target/debug/learnpuppy bunx playwright test -c playwright.serve.config.ts

# Budgets from docs/web/07: bundle size, graph 5k/10k frame times, LCP, long tasks, heap growth.
perf: disk-guard
	cd web && bun install --frozen-lockfile && bun run build && bun run bundle-size
	cd web && bunx playwright test graph.spec -g "performance budgets"
	cd web && bunx playwright test -c playwright.perf.config.ts

# Single self-contained binary: the SPA (web/dist) is embedded at compile time, so build it first.
release: disk-guard
	cd web && bun install --frozen-lockfile && bun run build
	$(CARGO) build --release -p lp-server
	@echo "built target/release/learnpuppy"

install: disk-guard
	cd web && bun install --frozen-lockfile && bun run build
	$(CARGO) install --path crates/lp-server --locked
