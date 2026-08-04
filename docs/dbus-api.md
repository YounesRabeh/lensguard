# D-Bus API

LensGuard publishes a versioned, read-only interface on the user session bus. Clients must use
the interface name, not command-line output, as the compatibility boundary.

| Item | Value |
| --- | --- |
| Bus name | `io.github.younesrabeh.CameraMonitor` |
| Object path | `/io/github/younesrabeh/CameraMonitor` |
| Interface | `io.github.younesrabeh.CameraMonitor1` |
| Canonical XML | `dbus/io.github.younesrabeh.CameraMonitor1.xml` |

The `1` suffix is the major wire-interface version. Additive changes may retain it; incompatible
signature or semantic changes require a new interface name.

## Properties

| Name | Signature | Meaning |
| --- | --- | --- |
| `Active` | `b` | `true` when one or more camera relationships are active. |
| `ActiveSessionCount` | `u` | Number of active relationships, saturated at `u32::MAX`. |
| `BackendAvailable` | `b` | Whether observation is available. This is independent of `Active`. |
| `Version` | `s` | LensGuard workspace version. Constant for the service process. |

`Active`, `ActiveSessionCount`, and `BackendAvailable` emit standard
`org.freedesktop.DBus.Properties.PropertiesChanged` notifications only when their values change.
`Version` is constant. Clients should read initial properties after connecting and then watch for
changes; signals are not a replacement for initial synchronization.

## Methods

### `Ping() -> s`

Returns `pong`. This verifies that the object and versioned interface are responsive.

### `GetActiveSessions() -> a(sssssstu)`

Returns active sessions sorted lexicographically by `session_id`. Each element has this fixed
field order:

| Position | Field | Signature | Meaning |
| ---: | --- | --- | --- |
| 1 | `session_id` | `s` | Opaque identity for the active application-to-camera relationship. |
| 2 | `application_id` | `s` | Desktop/application ID, or an empty string when unknown. |
| 3 | `application_name` | `s` | Human-readable, untrusted display text. |
| 4 | `device_id` | `s` | Opaque camera identity assigned at the adapter boundary. |
| 5 | `device_name` | `s` | Human-readable, untrusted camera display text. |
| 6 | `backend` | `s` | Detection backend identifier; currently `pipewire`. |
| 7 | `started_at_unix_ms` | `t` | Relationship start time in Unix milliseconds. |
| 8 | `process_id` | `u` | Process ID, or `0` when unknown. |

IDs are opaque. The API does not expose raw numeric PipeWire node, port, or link IDs. Clients
must not parse IDs or assume they persist across service/backend restarts. Application and device
strings come from external metadata and must be rendered as untrusted text.

## Signals

| Name | Signature | Meaning |
| --- | --- | --- |
| `StateChanged` | `bub` | Active flag, session count, and backend availability after observable state changes. |
| `SessionStarted` | `(sssssstu)` | Full DTO for a newly active relationship. |
| `SessionStopped` | `s` | Opaque session ID that stopped. |
| `BackendAvailabilityChanged` | `b` | New backend availability value. |

Metadata-only session updates emit `StateChanged`; consumers should refresh
`GetActiveSessions()` to obtain current DTOs. Signals can be missed during reconnects, so a client
should always resynchronize properties and sessions after the service appears.

## Errors

Application failures use names below `io.github.younesrabeh.CameraMonitor.Error`, currently
including `io.github.younesrabeh.CameraMonitor.Error.Internal`. Messages include a concise reason
suitable for diagnostics. Standard D-Bus errors can still report unavailable services, unknown
methods, invalid arguments, or transport failures.

## Command-line examples

Start the functional daemon in one terminal:

```sh
cargo run -p camera-monitor -- --log-level info run
```

With no camera capture, this publishes an empty session list while keeping the PipeWire backend
under observation. `serve-dbus` remains available when a transport-only empty endpoint is useful.

Inspect and call it with GLib's standard D-Bus tool:

```sh
gdbus introspect --session \
  --dest io.github.younesrabeh.CameraMonitor \
  --object-path /io/github/younesrabeh/CameraMonitor

gdbus call --session \
  --dest io.github.younesrabeh.CameraMonitor \
  --object-path /io/github/younesrabeh/CameraMonitor \
  --method io.github.younesrabeh.CameraMonitor1.Ping

gdbus call --session \
  --dest io.github.younesrabeh.CameraMonitor \
  --object-path /io/github/younesrabeh/CameraMonitor \
  --method org.freedesktop.DBus.Properties.Get \
  io.github.younesrabeh.CameraMonitor1 Active

gdbus call --session \
  --dest io.github.younesrabeh.CameraMonitor \
  --object-path /io/github/younesrabeh/CameraMonitor \
  --method io.github.younesrabeh.CameraMonitor1.GetActiveSessions
```

Equivalent `busctl` checks are:

```sh
busctl --user introspect io.github.younesrabeh.CameraMonitor \
  /io/github/younesrabeh/CameraMonitor io.github.younesrabeh.CameraMonitor1
busctl --user get-property io.github.younesrabeh.CameraMonitor \
  /io/github/younesrabeh/CameraMonitor io.github.younesrabeh.CameraMonitor1 Active
busctl --user call io.github.younesrabeh.CameraMonitor \
  /io/github/younesrabeh/CameraMonitor io.github.younesrabeh.CameraMonitor1 GetActiveSessions
```
