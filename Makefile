.PHONY: format format-check lint test check build sync-version license-audit package-rpm package-deb package-arch package-release release-artifacts release-check release-package-check release-candidate smoke-extension smoke-real-camera capture-extension-screenshots test-local-installation test-system-package install-local uninstall-local bootstrap clean

format:
	cargo fmt --all

format-check:
	cargo fmt --all --check

lint:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
	./scripts/check-extension.sh
	@if command -v shellcheck >/dev/null 2>&1; then shellcheck scripts/*.sh tests/manual/*.sh; else echo "shellcheck not installed; skipping optional shell lint"; fi

test:
	cargo test --workspace --locked
	./scripts/test-extension-dbus.sh
	./scripts/test-local-installation.sh

build:
	cargo build --workspace --locked
	./scripts/package-extension.sh dist

sync-version:
	./scripts/sync-version.sh

license-audit:
	./scripts/audit-licenses.sh

package-rpm:
	./scripts/package-rpm.sh

package-deb:
	./scripts/package-deb.sh

package-arch:
	./scripts/package-arch.sh

package-release:
	./scripts/package-release.sh

release-artifacts:
	./scripts/package-latest-release.sh

release-check:
	./scripts/check-release.sh dist/release/$(shell ./scripts/project-version.sh)

release-package-check:
	./scripts/check-package-artifacts.sh dist/release/$(shell ./scripts/project-version.sh)

release-candidate:
	./scripts/package-release.sh dist/release/$(shell ./scripts/project-version.sh) --require-rpm

smoke-extension: build
	./scripts/smoke-extension.sh dist/lensguard@younesrabeh.github.io.shell-extension.zip

smoke-real-camera: build
	./scripts/smoke-real-camera.sh dist/lensguard@younesrabeh.github.io.shell-extension.zip

capture-extension-screenshots: build
	./scripts/capture-extension-screenshots.sh dist/lensguard@younesrabeh.github.io.shell-extension.zip

test-local-installation: build
	./scripts/test-local-installation.sh

test-system-package:
	./scripts/test-system-package.sh

install-local:
	./scripts/install-local.sh

uninstall-local:
	./scripts/uninstall-local.sh

check: format-check lint test build license-audit test-system-package

bootstrap:
	./scripts/bootstrap-dev.sh

clean:
	cargo clean
