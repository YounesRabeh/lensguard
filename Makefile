.PHONY: format format-check lint test check build sync-version license-audit package-rpm package-deb package-arch package-all package-release release-artifacts release-check release-package-check release-candidate smoke-extension smoke-real-camera capture-extension-screenshots test-local-installation test-system-package install-local uninstall-local bootstrap clean

format:
	cargo fmt --all

format-check:
	cargo fmt --all --check

lint:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
	./scripts/test/check-extension.sh
	@if command -v shellcheck >/dev/null 2>&1; then find scripts tests/manual -type f -name '*.sh' -print0 | xargs -0 shellcheck; else echo "shellcheck not installed; skipping optional shell lint"; fi

test:
	cargo test --workspace --locked
	./scripts/test/test-extension-dbus.sh
	./scripts/test/test-local-installation.sh

build:
	cargo build --workspace --locked
	./scripts/package/package-extension.sh dist

sync-version:
	./scripts/util/sync-version.sh

license-audit:
	./scripts/release/audit-licenses.sh

package-rpm:
	./scripts/package/package-rpm.sh

package-deb:
	./scripts/package/package-deb.sh

package-arch:
	./scripts/package/package-arch.sh

package-all:
	./scripts/release/package-all.sh

package-release:
	./scripts/release/package-release.sh

release-artifacts:
	./scripts/release/package-latest-release.sh

release-check:
	./scripts/release/check-release.sh dist/release/$(shell ./scripts/util/project-version.sh)

release-package-check:
	./scripts/release/check-package-artifacts.sh dist/release/$(shell ./scripts/util/project-version.sh)

release-candidate:
	./scripts/release/package-release.sh dist/release/$(shell ./scripts/util/project-version.sh) --require-rpm

smoke-extension: build
	./scripts/dev/smoke-extension.sh dist/lensguard@younesrabeh.github.io.shell-extension.zip

smoke-real-camera: build
	./scripts/dev/smoke-real-camera.sh dist/lensguard@younesrabeh.github.io.shell-extension.zip

capture-extension-screenshots: build
	./scripts/dev/capture-extension-screenshots.sh dist/lensguard@younesrabeh.github.io.shell-extension.zip

test-local-installation: build
	./scripts/test/test-local-installation.sh

test-system-package:
	./scripts/test/test-system-package.sh

install-local:
	./scripts/install/install-local.sh

uninstall-local:
	./scripts/install/uninstall-local.sh

check: format-check lint test build test-system-package

bootstrap:
	./scripts/dev/bootstrap-dev.sh

clean:
	cargo clean
