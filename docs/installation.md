# Installation

LensGuard needs a native service because verified V4L2 operation results require a small
privileged eBPF observer. The GNOME extension cannot install or start privileged code; the native
package enables and starts the observer during installation.

## GNOME Extensions website

Use this installation route when the LensGuard GNOME extension comes from
`extensions.gnome.org`. The native package must be installed separately because a GNOME
extension is not allowed to install privileged system services.

### 1. Install `lensguard-service`

`lensguard-service` is the service-only native package. It installs:

- the unprivileged `camera-monitor` user daemon and its D-Bus activation file;
- the privileged, metadata-only `lensguard-v4l2-observer`;
- the system and user systemd units; and
- the trusted-broker policy.

It deliberately does **not** install the GNOME extension. Do not install both
`lensguard-service` and the combined `lensguard` package; they provide the same native files and
are declared as conflicting packages.

If `lensguard` is already installed, do not use `--skip-broken`: the full package already contains
the native service. Choose one of these routes instead:

- Keep `lensguard`, skip this service-package step, and use its system-installed GNOME extension.
- To use the extension from `extensions.gnome.org`, replace `lensguard` with
  `lensguard-service`. On Fedora, use `dnf swap` so removal and installation happen in one
  transaction:

  ```bash
  sudo dnf swap lensguard lensguard-service
  ```

  When installing a locally built RPM, use the RPM path as the second argument instead of the
  repository package name.

If a configured package repository provides LensGuard, install it normally:

```bash
# Fedora
sudo dnf install lensguard-service

# Ubuntu or Debian
sudo apt install lensguard-service

# Arch Linux
sudo pacman -S lensguard-service
```

If the package is not published in a configured repository, build the package for the current
distribution from this repository and install the generated service-only artifact. Each `find`
command below selects the newest non-debug binary package in `dist/packages`:

```bash
# Fedora
make package-rpm
service_rpm=$(find dist/packages/rpm -maxdepth 1 -type f \
    -name 'lensguard-service-[0-9]*.rpm' ! -name '*.src.rpm' | sort -V | tail -n 1)
if rpm -q lensguard >/dev/null 2>&1; then
    sudo dnf swap lensguard "$service_rpm"
else
    sudo dnf install "$service_rpm"
fi
```

```bash
# Ubuntu or Debian
make package-deb
service_deb=$(find dist/packages/deb -maxdepth 1 -type f \
    -name 'lensguard-service_[0-9]*_*.deb' | sort -V | tail -n 1)
sudo apt install "$service_deb"
```

```bash
# Arch Linux
make package-arch
service_arch=$(find dist/packages/arch -maxdepth 1 -type f \
    -name 'lensguard-service-[0-9]*.pkg.tar.*' | sort -V | tail -n 1)
sudo pacman -U "$service_arch"
```

The build command requires the distribution's normal package-building tools in addition to the
development requirements described in [development.md](development.md). A downloaded release
artifact can be installed with the same `dnf install ./file.rpm`, `apt install ./file.deb`, or
`pacman -U ./file.pkg.tar.zst` form without building it locally.

Do not substitute `make install-local` for this step. That target installs only an unprivileged
development copy under the current user's home directory and cannot install the V4L2 observer.

### 2. Verify the observer

On a fresh package installation, the observer is enabled and started automatically. Verify it with:

```bash
systemctl status lensguard-v4l2-observer.service
```

If an older installation or a local system policy left it disabled, enable it manually:

```bash
sudo systemctl enable --now lensguard-v4l2-observer.service
```

To opt out of privileged monitoring, disable it explicitly with
`sudo systemctl disable --now lensguard-v4l2-observer.service`. LensGuard will then display its
monitoring-unavailable warning.

### 3. Install the extension

After installing `lensguard-service`, install the unprivileged extension using either of these
methods.

From `extensions.gnome.org`:

1. Open <https://extensions.gnome.org> in a browser, or open an extension-manager application.
2. Search for **Lens Guard** and install it.

To install the extension ZIP built from this repository instead:

```bash
./scripts/package/package-extension.sh dist
gnome-extensions install --force \
    dist/lensguard@younesrabeh.github.io.shell-extension.zip
```

The ZIP installation is per-user and is appropriate beside the system-wide
`lensguard-service` package. Do not use it beside the combined `lensguard` package, which already
installs the same extension UUID system-wide.

On Wayland, log out and back in after the first ZIP installation or after replacing a previously
loaded copy. GNOME Shell does not discover a new extension directory during the existing session;
running `gnome-extensions enable` too early reports that the extension does not exist even though
the files were installed successfully.

After Shell has discovered the extension, turn it on in the Extensions application or enable it
from a terminal:

```bash
gnome-extensions enable lensguard@younesrabeh.github.io
```

Verify which extension copy GNOME Shell sees:

```bash
gnome-extensions info lensguard@younesrabeh.github.io
```

The command should show the expected path and version. If it still shows an old copy, log out and
back in again before troubleshooting the extension files or compiled settings schema.

The Store ZIP contains only GJS, preferences, schemas, metadata, and CSS.

## Native combined package

Install the distribution's `lensguard` package and enable the observer as above. Do not install a
second copy of the extension from the GNOME website; both delivery paths use the same UUID.

### Quickly replace only the extension

When the combined `lensguard` package is installed and only the extension has changed, rebuild the
ZIP and overwrite the system extension in place:

```bash
./scripts/package/package-extension.sh dist
sudo unzip -o \
    dist/lensguard@younesrabeh.github.io.shell-extension.zip \
    -d /usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io
sudo glib-compile-schemas \
    /usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io/schemas
```

This replaces only the GJS extension, preferences, metadata, CSS, and schema. It does not rebuild,
replace, or restart the daemon or V4L2 observer. Log out and back in to load the new extension on
Wayland.

Do not also run `gnome-extensions install` in this setup: that creates a second per-user copy with
the same UUID. These files are owned by the combined package, so reinstalling `lensguard` later
restores the packaged extension.

## Developer-only user install

`make install-local` installs only the unprivileged daemon and extension for development. It
cannot install the observer and therefore correctly shows monitoring as unavailable unless a
matching system `lensguard-service` is already installed.

## Removal

Disable and remove the chosen extension copy, then remove the native package. Package removal must
stop and remove the observer unit, binary, BPF object embedded in that binary, runtime socket, and
trusted-broker policy. Per-user Store extension files are never modified by native package removal.

## Requirements

- Linux with eBPF tracepoint and perf-event support
- systemd system and user managers
- a kernel policy that permits the package's bounded capabilities
- GNOME Shell 50 or newer for the extension

Secure Boot lockdown, SELinux/AppArmor policy, containers, or disabled unprivileged/perf BPF
facilities can make the observer unavailable. LensGuard exposes the specific availability category
instead of falling back to another detector.
