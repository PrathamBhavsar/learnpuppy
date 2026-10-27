SHELL := /bin/zsh
export PATH := /opt/homebrew/opt/rustup/bin:$(HOME)/.cargo/bin:$(PATH)
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
	@command -v cargo-deny >/dev/null && $(CARGO) deny check || echo 'cargo-deny not installed, skipping'

gate: fmt clippy test okf

ci: gate deny
