<a href="https://github.com/YounesRabeh/lensguard"><img src="docs/images/lensguard-banner.png" alt="LensGuard" width="100%"></a>

<div align="center">

  <p align="center">
    <img src="https://img.shields.io/badge/Platform-Linux-1793D1?style=for-the-badge&amp;logo=linux&amp;logoColor=white" alt="Platform: Linux">
    <img src="https://img.shields.io/badge/GNOME_Shell-50-4A86CF?style=for-the-badge&amp;logo=gnome&amp;logoColor=white" alt="GNOME Shell 50">
    <img src="https://img.shields.io/badge/License-GPL--3.0--or--later-3DA639?style=for-the-badge" alt="GPL-3.0-or-later license">
    <a href="https://github.com/YounesRabeh/lensguard/releases/latest"><img src="https://img.shields.io/badge/Download-Latest_Release-2EA44F?style=for-the-badge&amp;logo=github&amp;logoColor=white" alt="Download the latest release"></a>
    <a href="docs/README.md"><img src="https://img.shields.io/badge/Open-Documentation_Hub-0969DA?style=for-the-badge&amp;logo=readthedocs&amp;logoColor=white" alt="Open the documentation hub"></a>
  </p>

  <p>A GNOME Shell privacy indicator for applications using cameras directly through V4L2.</p>

  <p align="center">
    <a href="#features">Features</a> •
    <a href="#screenshots">Screenshots</a> •
    <a href="#quick-start">Quick Start</a> •
    <a href="#development">Development</a> •
    <a href="#guides">Guides</a>
  </p>
</div>

---

## Features

- 📷 Detects confirmed direct V4L2 streaming after a successful `VIDIOC_STREAMON`.
- 🛡️ Shows the active application and physical camera in GNOME Quick Settings.
- 🤝 Leaves trusted desktop-broker sessions to GNOME, preventing duplicate indicators.
- ⚠️ Makes monitoring failures and unknown activity visible instead of claiming the camera is idle.
- 🔒 Handles metadata only, never frames, video buffers, command lines, or persistent history.

## Screenshots

| Active direct-camera session | Quick Settings details |
| --- | --- |
| ![LensGuard showing a direct camera session](.github/assets/app-showcase.png) | ![LensGuard Quick Settings menu](.github/assets/menu-showcase.png) |

| Application identification | Monitoring availability warning |
| --- | --- |
| ![LensGuard identifying Discord camera use](.github/assets/discord-example.png) | ![LensGuard monitoring warning](.github/assets/dbus-error.png) |

## Quick start

On Fedora, install the complete package from COPR:

```bash
sudo dnf copr enable younesrabeh/lensguard
sudo dnf install lensguard
```

Log out and back in once, then enable the extension:

```bash
gnome-extensions enable lensguard@younesrabeh.github.io
```

LensGuard can also be installed from extensions.gnome.org with the separate `lensguard-service`
package. See the [installation guide](docs/installation.md) for Fedora, Debian/Ubuntu, Arch,
local packages, upgrades, removal, and the difference between the two package routes.

## Development

Requires Rust 1.85+, Clang with the BPF target, GNU Make, GNOME development tools, pnpm,
ripgrep, shellcheck, and unzip. Bootstrap a development environment, then run the full check:

```bash
./scripts/dev/bootstrap-dev.sh
make check
```

| Need | Command |
| --- | --- |
| Format code | `make format` |
| Run tests | `make test` |
| Build the workspace and extension | `make build` |
| Run all checks | `make check` |
| Build release packages | `make package-all` |

See the [development guide](docs/development.md) for the full setup and observer testing workflow.

## Guides

Choose a guide by task, or browse the complete [documentation hub](docs/README.md):

| I want to… | Start here |
| --- | --- |
| Start using LensGuard | [Getting started](docs/getting-started.md) · [Installation](docs/installation.md) |
| Understand its privacy model | [Architecture and privacy boundary](docs/architecture.md) · [D-Bus API](docs/dbus-api.md) |
| Diagnose an issue | [Troubleshooting](docs/troubleshooting.md) |
| Develop, test, or release it | [Development](docs/development.md) · [Release packaging](docs/release.md) · [Release checklist](docs/release-checklist.md) |


## Tech stack

<p align="left">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-2024-000000?style=for-the-badge&amp;logo=rust&amp;logoColor=white" alt="Rust 2024"></a>
  <a href="https://developer.mozilla.org/en-US/docs/Web/JavaScript"><img src="https://img.shields.io/badge/JavaScript-ES2022-F7DF1E?style=for-the-badge&amp;logo=javascript&amp;logoColor=black" alt="JavaScript"></a>
  <a href="https://gjs.guide/"><img src="https://img.shields.io/badge/GJS-GNOME_JavaScript-4A86CF?style=for-the-badge&amp;logo=gnome&amp;logoColor=white" alt="GJS GNOME JavaScript"></a>
  <a href="https://www.gnome.org/"><img src="https://img.shields.io/badge/GNOME_Shell-50-4A86CF?style=for-the-badge&amp;logo=gnome&amp;logoColor=white" alt="GNOME Shell 50"></a>
  <a href="https://ebpf.io/"><img src="https://img.shields.io/badge/eBPF-Tracepoints-F15A24?style=for-the-badge&amp;logo=linux&amp;logoColor=white" alt="eBPF tracepoints"></a>
  <a href="https://www.freedesktop.org/wiki/Software/dbus/"><img src="https://img.shields.io/badge/D--Bus-Session_IPC-8B5CF6?style=for-the-badge" alt="D-Bus session IPC"></a>
  <a href="https://systemd.io/"><img src="https://img.shields.io/badge/systemd-Services-5B7C99?style=for-the-badge&amp;logo=systemd&amp;logoColor=white" alt="systemd services"></a>
  <a href="https://www.gnu.org/software/make/"><img src="https://img.shields.io/badge/GNU_Make-Build-427819?style=for-the-badge&amp;logo=gnu&amp;logoColor=white" alt="GNU Make"></a>
  <a href="https://pnpm.io/"><img src="https://img.shields.io/badge/pnpm-11-F69220?style=for-the-badge&amp;logo=pnpm&amp;logoColor=white" alt="pnpm 11"></a>
</p>

---

## License

Distributed under the [GPL-3.0-or-later](LICENSE) license.
