# Installation

LensGuard needs a native service because verified V4L2 operation results require a small
privileged eBPF observer. The GNOME extension cannot and does not install or start privileged
code.

## GNOME Extensions website

1. Install `lensguard-service` with your distribution package manager.
2. Enable its system observer:

   ```bash
   sudo systemctl enable --now lensguard-v4l2-observer.service
   ```

3. Install the LensGuard extension from `extensions.gnome.org`.

The service package provides `/usr/lib*/lensguard/camera-monitor`,
`lensguard-v4l2-observer`, the system and user units, D-Bus activation, and the versioned broker
policy. The Store ZIP contains only GJS, preferences, schemas, metadata, and CSS.

## Native combined package

Install the distribution's `lensguard` package and enable the observer as above. Do not install a
second copy of the extension from the GNOME website; both delivery paths use the same UUID.

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
