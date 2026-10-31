SHELL := /bin/zsh
export INSTA_WORKSPACE_ROOT := $(CURDIR)
export PATH := /opt/homebrew/bin:/opt/homebrew/opt/rustup/bin:$(HOME)/.cargo/bin:$(PATH)
CARGO ?= /opt/homebrew/opt/rustup/bin/cargo

.PHONY: fmt clippy test okf deny api-gen api-check gate ci

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
