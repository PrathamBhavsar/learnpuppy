SHELL := /bin/zsh
export INSTA_WORKSPACE_ROOT := $(CURDIR)
export PATH := /opt/homebrew/bin:/opt/homebrew/opt/rustup/bin:$(HOME)/.cargo/bin:$(PATH)
CARGO ?= /opt/homebrew/opt/rustup/bin/cargo

.PHONY: fmt clippy test okf deny gate ci

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

gate: fmt clippy test okf

ci: gate deny
