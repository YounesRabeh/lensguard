# Release packaging

LensGuard supports two release installation paths:

- the binary-free GNOME extension ZIP plus a service-only `lensguard-service` package; or
- the full `lensguard` package containing the extension and service.

The two native package variants conflict because they own the same daemon and activation files.
The extension also reports a conflict when per-user and system copies of its UUID coexist.

## Supported environment

Pull requests and every push to `main` run the read-only CI workflow. Repository settings should
require its `CI / Check on Ubuntu` status before merging to `main`.

Strict semantic tags such as `v1.3.0` run the release workflow. It verifies that the tag version
matches `Cargo.toml`, is newer than the latest published stable release, and is reachable from
`main`. It requires successful CI on the tagged commit, then builds DEB, RPM, Arch, extension,
daemon, source, licence, and manifest artifacts.

Before any native package job starts, a dedicated release gate audits the locked Rust dependency
licenses and produces the dependency-license report. The DEB, RPM, Arch, and core artifact jobs all
depend on that gate. Routine `make check` runs intentionally omit this release-only audit.

The audit currently uses `scripts/release/audit-licenses.sh` and a reviewed SPDX-expression
allowlist.
Replace it with `cargo deny check licenses` only after a checked-in `deny.toml` reproduces the
policy, handles the pinned Git dependency, and produces an equivalent release artifact.

Each native package is metadata-linted, installed in its target distribution container, checked
for installed paths and permissions, cold-activated over D-Bus, upgraded from the previous stable
package when one exists, and uninstalled. Upgrade tests also verify that the panel-indicator
preference is preserved.

After the release workflow succeeds, the separate publish workflow creates a **draft** GitHub
release containing the standalone extension ZIP and both full and service-only DEB, binary RPM,
and Arch packages, plus checksums for those seven files. Rerunning that publish workflow replaces an existing
draft without rebuilding packages.
Reviewing and publishing the draft—and any later GNOME Extensions submission—remain deliberate
publishing steps.

To repeat publication without rerunning the release build, manually run **Publish release draft**
and provide the successful **Release candidate** workflow run ID. The publish workflow verifies the
source run and its checksums before replacing the draft.

The container base images are pinned by digest. Distribution repositories inside those containers
remain current compatibility inputs, so release reruns are controlled but are not guaranteed to be
bit-for-bit identical after repository metadata changes.

## Build release artifacts

Install the development prerequisites from `docs/development.md`, including `rpm-build`, then run:

```bash
make release-candidate
```

That target builds the core release artifacts and RPM packages for the current version. To build
every local distribution package and both release output forms with one command, run:

```bash
make package-all
```

`package-all` performs the following work in a temporary staging directory before changing the
published output directories:

1. synchronizes the extension metadata with the workspace version, verifies that `Cargo.lock`
   is current, and runs the dependency-license gate before expensive package builds;
2. builds full and service-only DEB, RPM, and Arch packages;
3. builds the extension ZIP, daemon artifact, source archive, dependency-license report, manifest,
   and checksums;
4. assembles and verifies a complete latest artifact set and versioned release candidate; and
5. replaces the package-format directories only after every build and verification succeeds.

The output policy is intentional:

- `dist/packages/deb`, `dist/packages/rpm`, and `dist/packages/arch` are fresh snapshots, so old
  packages do not remain mixed with the current version;
- `dist/release-artifacts` is replaced with the latest complete verified set;
- `dist/release/<current-version>` is replaced when rebuilding the same version; and
- every other directory under `dist/release/` is preserved as release history.

Package builders must run as the normal user. If an older artifact was created with `sudo`, repair
the generated-directory ownership before running `package-all`:

```bash
sudo chown -R "$USER:$USER" dist/packages dist/release-artifacts
```

The combined command requires the local DEB and RPM packaging tools, including `dpkg-deb` and
`rpmbuild`. When `makepkg` is installed, it builds Arch packages directly. Otherwise, it uses
Podman or Docker with the same digest-pinned Arch Linux image as CI; that fallback needs network
access to download the image and Arch build dependencies. Artifact validation accepts either
`bsdtar` or GNU `tar` with `zstd` support.

Artifacts are written under `dist/release/<version>/`:

- `lensguard-<version>-*.rpm`: Fedora binary package;
- `lensguard-service-<version>-*.rpm`: service-only Fedora package;
- `lensguard-<version>-*.src.rpm`: Fedora package source;
- `lensguard-<version>.spec`: concrete, independently reusable Fedora spec;
- `lensguard-service-<version>.spec`: service-only Fedora spec;
- `lensguard-debuginfo-<version>-*.rpm` and `lensguard-debugsource-<version>-*.rpm`: Fedora debugging packages;
- `lensguard-extension-v<version>.zip`: standalone GNOME extension;
- `camera-monitor-<version>-<architecture>`: release daemon diagnostic artifact;
- `lensguard-<version>-source.tar.gz`: reproducible source snapshot;
- `lensguard-<version>-dependency-licenses.tsv`: locked dependency attribution report;
- `SHA256SUMS`: checksums for every release artifact.

`Cargo.toml` under `[workspace.package]` is the version source. Workspace crates inherit it, the
daemon and D-Bus `Version` property compile it in, extension packaging writes it into
`metadata.json`, and release filenames include it. `scripts/release/check-release.sh` rejects version
mismatches.

The RPM is compiled during `rpmbuild` from its source archive. Cargo dependencies, including the
patched bindgen revision, are vendored into that archive and the RPM build runs Cargo offline. The
source RPM therefore contains the sources needed to reproduce the binary build rather than wrapping
a precompiled daemon.

## Install the Fedora package

Inspect and verify the artifacts before installation:

```bash
version=$(./scripts/util/project-version.sh)
cd "dist/release/$version"
sha256sum --check SHA256SUMS
rpm -qpl "lensguard-$version"-*.x86_64.rpm
sudo dnf install "./lensguard-$version"-*.x86_64.rpm
```

The full package installs only under `/usr`:

- `/usr/libexec/lensguard/camera-monitor`
- `/usr/lib/systemd/user/camera-monitor.service`
- `/usr/share/dbus-1/services/io.github.younesrabeh.CameraMonitor.service`
- `/usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io/`

The daemon is activated on demand through the user session bus. It is not enabled as a system
service and does not run as root. Log out and back in after the first system installation so GNOME
Shell discovers the extension, then enable **Lens Guard** in Extensions.

For an extension installed from extensions.gnome.org, install the matching
`lensguard-service` package instead. It installs the daemon, systemd user unit, and D-Bus
activation file, but nothing under `/usr/share/gnome-shell/extensions/`.

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

Store-extension users should remove the service with `sudo dnf remove lensguard-service`; this
does not remove or modify the GNOME Store extension.

## Known detection limitation

LensGuard observes confirmed direct V4L2 streaming capture. Brokered camera sessions are
intentionally left to GNOME's privacy indicator. V4L2 read-I/O
monitoring is explicitly deferred until after the stable-release work.
