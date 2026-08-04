.PHONY: format format-check lint test check build bootstrap clean

format:
	cargo fmt --all

format-check:
	cargo fmt --all --check

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	./scripts/check-extension.sh
	@if command -v shellcheck >/dev/null 2>&1; then shellcheck scripts/*.sh; else echo "shellcheck not installed; skipping optional shell lint"; fi

test:
	cargo test --workspace

build:
	cargo build --workspace
	./scripts/package-extension.sh dist

check: format-check lint test build

bootstrap:
	./scripts/bootstrap-dev.sh

clean:
	cargo clean
