# Release packaging

LensGuard is packaged as a system package containing the Rust daemon,
systemd user unit, D-Bus activation file, and GNOME extension. Keeping these runtime components in
one package prevents an extension/daemon protocol mismatch. A standalone extension ZIP is also
produced for extension installation tests and development, but it still needs a compatible daemon.

## Supported environment

Pull requests and every push to `main` run the read-only CI workflow. Repository settings should
require its `CI / Check on Ubuntu` status before merging to `main`.

Strict semantic tags such as `v1.3.0` run the release workflow. It verifies that the tag version
matches `Cargo.toml`, is newer than the latest published stable release, and is reachable from
`main`. It then repeats full CI and builds DEB, RPM, Arch, extension, daemon, source, licence, and
manifest artifacts.

Each native package is metadata-linted, installed in its target distribution container, checked
for installed paths and permissions, cold-activated over D-Bus, upgraded from the previous stable
package when one exists, and uninstalled. Upgrade tests also verify that the panel-indicator
preference is preserved.

The workflow creates a **draft** GitHub release only. Reviewing and publishing that draft—and any
later GNOME Extensions submission—remain deliberate publishing steps.

The container base images are pinned by digest. Distribution repositories inside those containers
remain current compatibility inputs, so release reruns are controlled but are not guaranteed to be
bit-for-bit identical after repository metadata changes.

## Build release artifacts

Install the development prerequisites from `docs/development.md`, including `rpm-build`, then run:

```bash
make release-candidate
```

Artifacts are written under `dist/release/<version>/`:

- `lensguard-<version>-*.rpm`: Fedora binary package;
- `lensguard-<version>-*.src.rpm`: Fedora package source;
- `lensguard-<version>.spec`: concrete, independently reusable Fedora spec;
- `lensguard-debuginfo-<version>-*.rpm` and `lensguard-debugsource-<version>-*.rpm`: Fedora debugging packages;
- `lensguard-extension-<version>.zip`: standalone GNOME extension;
- `camera-monitor-<version>-<architecture>`: release daemon diagnostic artifact;
- `lensguard-<version>-source.tar.gz`: reproducible source snapshot;
- `lensguard-<version>-dependency-licenses.tsv`: locked dependency attribution report;
- `SHA256SUMS`: checksums for every release artifact.

`Cargo.toml` under `[workspace.package]` is the version source. Workspace crates inherit it, the
daemon and D-Bus `Version` property compile it in, extension packaging writes it into
`metadata.json`, and release filenames include it. `scripts/check-release.sh` rejects version
mismatches.

The RPM is compiled during `rpmbuild` from its source archive. Cargo dependencies, including the
patched bindgen revision, are vendored into that archive and the RPM build runs Cargo offline. The
source RPM therefore contains the sources needed to reproduce the binary build rather than wrapping
a precompiled daemon.

## Install the Fedora package

Inspect and verify the artifacts before installation:

```bash
version=$(./scripts/project-version.sh)
cd "dist/release/$version"
sha256sum --check SHA256SUMS
rpm -qpl "lensguard-$version"-*.x86_64.rpm
sudo dnf install "./lensguard-$version"-*.x86_64.rpm
```

The package installs only under `/usr`:

- `/usr/libexec/lensguard/camera-monitor`
- `/usr/lib/systemd/user/camera-monitor.service`
- `/usr/share/dbus-1/services/io.github.younesrabeh.CameraMonitor.service`
- `/usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io/`

The daemon is activated on demand through the user session bus. It is not enabled as a system
service and does not run as root. Log out and back in after the first system installation so GNOME
Shell discovers the extension, then enable **Lens Guard** in Extensions.

## Upgrade

Install the newer RPM with DNF. The RPM user-service scriptlets notify user managers and restart a
running daemon after upgrade. LensGuard settings are stored in the user's dconf database and are
not owned or removed by the package. Log out and back in when GNOME Shell retains old extension
code or metadata in memory.

For a per-user development installation, run `make install-local` again. The installer replaces
only its owned binaries and extension directory; GSettings values remain intact.

## Uninstall

Remove a Fedora package with:

```bash
sudo dnf remove lensguard
```

Package-owned files are removed. User preferences may remain in dconf so reinstalling does not
silently reset them. Remove a per-user development install with `make uninstall-local`.

## Known detection limitation

LensGuard observes PipeWire camera relationships. Applications that open `/dev/video*` directly
without creating a PipeWire session are not detected in this release candidate. Direct V4L2
monitoring is explicitly deferred until after the stable-release work.
