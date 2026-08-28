# User-session D-Bus API

[< Back to LensGuard](../README.md)

- Bus: `io.github.younesrabeh.CameraMonitor`
- Object: `/io/github/younesrabeh/CameraMonitor`
- Interface: `io.github.younesrabeh.CameraMonitor1`

The interface is implemented only by the unprivileged `camera-monitor` daemon.

## Methods

- `Ping() → s`: returns `pong`.
- `GetActiveSessions() → a(ssssstu)`: confirmed direct V4L2 sessions.

Each session tuple contains, in order: session ID, application ID, application display name,
device ID, camera display name, monotonic start timestamp in nanoseconds, and PID. IDs are opaque.

## Properties

- `Active` (`b`) and `ActiveSessionCount` (`u`)
- `ObserverAvailable` (`b`)
- `ObserverAvailability` (`s`): `available`, `disabled`, `not-installed`,
  `unsupported-kernel`, `missing-capability`, `blocked-by-policy`, `version-mismatch`,
  `connection-failed`, or `backend-lost`
- `ObserverStatusDetail` (`s`): bounded diagnostic text
- `UnknownCameraActivity` (`b`)
- `SuppressedBrokerEvents` and `SuppressedUnknownEvents` (`t`)
- `Version` (`s`)

Suppression properties are aggregate and non-identifying. Unknown activity never appears in
`GetActiveSessions`, never creates an application row, and never sets `Active`.

Signals are `StateChanged(bub)`, `SessionStarted((ssssstu))`, `SessionStopped(s)`, and
`ObserverStatusChanged(s)`. The authoritative introspection document is
[`dbus/io.github.younesrabeh.CameraMonitor1.xml`](../dbus/io.github.younesrabeh.CameraMonitor1.xml).
