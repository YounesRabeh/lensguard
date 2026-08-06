.PHONY: format format-check lint test check build smoke-extension smoke-real-camera capture-extension-screenshots test-local-installation install-local uninstall-local bootstrap clean

format:
	cargo fmt --all

format-check:
	cargo fmt --all --check

lint:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
	./scripts/check-extension.sh
	@if command -v shellcheck >/dev/null 2>&1; then shellcheck scripts/*.sh; else echo "shellcheck not installed; skipping optional shell lint"; fi

test:
	cargo test --workspace --locked
	./scripts/test-extension-dbus.sh
	./scripts/test-local-installation.sh

build:
	cargo build --workspace --locked
	./scripts/package-extension.sh dist

smoke-extension: build
	./scripts/smoke-extension.sh dist/lensguard@younesrabeh.github.io.shell-extension.zip

smoke-real-camera: build
	./scripts/smoke-real-camera.sh dist/lensguard@younesrabeh.github.io.shell-extension.zip

capture-extension-screenshots: build
	./scripts/capture-extension-screenshots.sh dist/lensguard@younesrabeh.github.io.shell-extension.zip

test-local-installation: build
	./scripts/test-local-installation.sh

install-local:
	./scripts/install-local.sh

uninstall-local:
	./scripts/uninstall-local.sh

check: format-check lint test build

bootstrap:
	./scripts/bootstrap-dev.sh

clean:
	cargo clean
