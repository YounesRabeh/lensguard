# LensGuard

LensGuard is a GNOME Shell privacy indicator for applications that access a camera **directly
through V4L2**. It is intentionally complementary to GNOME's brokered-camera indicator: trusted
desktop broker capture is suppressed so the same camera use is never shown twice.

LensGuard reports a normal camera session only after a successful `VIDIOC_STREAMON`. Opening or
probing `/dev/videoN`, a failed capture request, an unverified process, or an event from a trusted
broker cannot activate the normal indicator.

## Components

- `lensguard-v4l2-observer`: a small privileged system service with narrowly scoped eBPF
  tracepoints. It records operation metadata only—never frames, buffers, command lines, process
  memory, networking, or persistent history.
- `camera-monitor`: an unprivileged per-user daemon that validates observer messages, resolves
  application and physical-device metadata, owns session state, and publishes user-session D-Bus.
- `lensguard@younesrabeh.github.io`: the unprivileged GNOME Shell extension.

The extension ZIP never contains either native binary, eBPF code, capabilities, systemd units, or
policy files. Install the matching `lensguard-service` native package alongside the Store
extension, or install the distribution's combined `lensguard` package.

## Coverage

Streaming V4L2 applications—including direct camera access from browsers and Electron clients
such as Chrome, Firefox, Discord, and Telegram when they bypass the desktop broker—are detected
when their driver successfully accepts `VIDIOC_STREAMON`. Applications may choose a brokered path;
that path remains GNOME's responsibility and is deliberately absent from LensGuard.

V4L2 read-I/O capture is not enabled in this release because tracing generic `read(2)` would
broaden collection beyond the reviewed camera-only boundary. LensGuard reports observer or owner
uncertainty explicitly instead of guessing.

See [installation](docs/installation.md), [architecture](docs/architecture.md), and
[troubleshooting](docs/troubleshooting.md).

## Development

```bash
make check
cargo run -p camera-monitor -- inspect-v4l2
```

The second command expects the packaged system observer. See [development](docs/development.md)
for a local workflow and security notes.

LensGuard is licensed under GPL-3.0-or-later.
