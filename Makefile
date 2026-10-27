export PATH := /opt/homebrew/opt/rustup/bin:$(HOME)/.cargo/bin:$(PATH)

.PHONY: fmt clippy test okf gate ci

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace --all-targets

okf:
	python3 /Users/pratham/.claude/skills/okf/scripts/validate.py brain --strict

gate: fmt clippy test okf

ci: gate
