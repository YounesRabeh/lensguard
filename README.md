# LensGuard

LensGuard is a GNOME Shell privacy indicator for applications that access cameras directly through
V4L2. It complements GNOME's PipeWire camera indicator without reporting the same session twice.

![LensGuard showing direct camera activity](.github/assets/app-showcase.png)

## What it does

- Detects successful V4L2 streaming capture from apps such as browsers and Electron clients.
- Shows the application and physical camera in GNOME Quick Settings.
- Reports monitoring failures clearly instead of presenting an uncertain “camera idle” state.
- Observes metadata only—never frames, video buffers, command lines, or persistent history.

![LensGuard Quick Settings menu](.github/assets/menu-showcase.png)

Direct Discord capture:

![LensGuard identifying Discord camera use](.github/assets/discord-example.png)

Monitoring failures remain visible:

![LensGuard monitoring warning](.github/assets/dbus-error.png)

## Install

On Fedora, install the complete package from COPR:

```bash
sudo dnf copr enable younesrabeh/lensguard
sudo dnf install lensguard
```

The package enables the privileged metadata observer automatically and installs the GNOME
extension system-wide. Log out and back in once, then enable LensGuard:

```bash
gnome-extensions enable lensguard@younesrabeh.github.io
```

Alternatively, install the extension from extensions.gnome.org and pair it with the service-only
`lensguard-service` package. Do not install both native packages together.

See the [installation guide](docs/installation.md) for Fedora, Debian/Ubuntu, Arch Linux, local
packages, upgrades, and removal.

## Detection boundary

LensGuard reports direct V4L2 streaming only after a successful `VIDIOC_STREAMON`. Camera access
through a trusted desktop broker remains GNOME's responsibility. V4L2 read-I/O is not detected in
this release.

## Documentation

- [Getting started](docs/getting-started.md)
- [Installation](docs/installation.md)
- [Architecture and privacy boundary](docs/architecture.md)
- [Troubleshooting](docs/troubleshooting.md)
- [D-Bus API](docs/dbus-api.md)
- [Development](docs/development.md)
- [Release process](docs/release.md)

## Development

```bash
make check
```

LensGuard is licensed under [GPL-3.0-or-later](LICENSE).
