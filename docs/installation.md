# Per-user installation

LensGuard installs entirely into the current user's home directory. It does not use `sudo`,
Polkit, a system service, or privileged camera access.

## Install

From the repository root:

```sh
make install-local
gnome-extensions enable lensguard@younesrabeh.github.io
```

The installer performs a locked release build, packages the extension, compiles its GSettings
schema, and installs these project-owned paths:

| Component | Local path |
| --- | --- |
| Daemon | `~/.local/libexec/lensguard/camera-monitor` |
| systemd user unit | `${XDG_CONFIG_HOME:-~/.config}/systemd/user/camera-monitor.service` |
| D-Bus activation | `${XDG_DATA_HOME:-~/.local/share}/dbus-1/services/io.github.younesrabeh.CameraMonitor.service` |
| GNOME extension | `${XDG_DATA_HOME:-~/.local/share}/gnome-shell/extensions/lensguard@younesrabeh.github.io` |

Installation is idempotent. An active daemon is restarted only after its replacement files are
ready; an inactive daemon stays stopped. The user D-Bus broker is asked to reload its activation
configuration so a newly installed service is normally available without logout.

GNOME Shell may require a logout/login before it discovers a newly installed extension on
Wayland. This is a Shell extension-discovery limitation, not a daemon activation requirement.

## Activation and service lifecycle

The daemon is not enabled for eager login startup. Requesting
`io.github.younesrabeh.CameraMonitor` activates `camera-monitor.service`; systemd considers it
ready only after the process owns that D-Bus name. For example:

```sh
gdbus call --session \
  --dest io.github.younesrabeh.CameraMonitor \
  --object-path /io/github/younesrabeh/CameraMonitor \
  --method io.github.younesrabeh.CameraMonitor1.Ping
```

An unexpected failure restarts after two seconds, with systemd start-rate limiting. SIGTERM from
`systemctl --user stop` is a clean shutdown and does not restart. The service belongs to the
graphical session and stops when that session ends.

The extension is only one possible D-Bus client. If it is absent or disabled, another user-session
client can activate the same daemon and use the documented API. When no client requests the bus
name, the service remains dormant; once activated it stays available for the rest of the session
unless explicitly stopped.

Useful service commands are:

```sh
systemctl --user status camera-monitor.service
systemctl --user restart camera-monitor.service
systemctl --user stop camera-monitor.service
journalctl --user -u camera-monitor.service -f
```

## Uninstall

```sh
make uninstall-local
```

Uninstall stops the service, disables the exact LensGuard extension UUID when possible, removes
the two exact integration files, and removes daemon/extension directories only when their
installer ownership markers match. Repeated uninstall is harmless. Other extensions, user units,
D-Bus services, settings, and home-directory files are not removed.

## Distribution packaging

The checked-in service templates use `@EXECUTABLE@` rather than embedding a home directory.
Distribution packages should substitute an appropriate path such as
`/usr/libexec/lensguard/camera-monitor`, install the unit under the distribution's systemd user
unit directory, and install the D-Bus service under its session-service directory.

Two package variants are produced:

- `lensguard-service` contains only the daemon and service integration for users of the GNOME
  Store extension.
- `lensguard` contains both the daemon and a system extension for a fully package-managed install.

The package variants conflict because their daemon files overlap. A per-user Store extension and
the system extension must not coexist; Lens Guard displays a conflict warning when both UUID paths
are present.

The repository provides package build commands for the main Linux distribution families:

```sh
make package-rpm   # Fedora, RHEL, openSUSE
make package-deb   # Debian, Ubuntu, Linux Mint, Pop!_OS
make package-arch  # Arch, Manjaro, EndeavourOS
```

Each command creates full and service-only artifacts under `dist/packages/` and builds the release daemon for the
current architecture. Run the command in a matching distribution environment (or its container or
CI runner): a Fedora-built daemon is not guaranteed to run on an older Debian or Ubuntu release.
The required native builders are `rpmbuild`, `dpkg-deb`, and `makepkg`, respectively.

Pushing a version tag in the form `v<workspace-version>` (for example, `v1.3.0`) runs the release
workflow. It verifies the full check suite, builds the extension bundle plus DEB, RPM, and Arch
packages in their matching environments, then attaches the artifacts to a generated GitHub Release.

`scripts/install-local.sh --artifact PATH --no-user-manager` is available for staged tests and
packaging validation; ordinary users should use `make install-local`.
