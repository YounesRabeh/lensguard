# D-Bus service smoke test

Environment:

- Fedora 44, D-Bus 1.16.2, GLib `gdbus`, and LensGuard 0.6.0.
- An isolated session bus created by `dbus-run-session`.

Preconditions:

- `cargo build -p camera-monitor` passes.
- `dbus-run-session` and `gdbus` are installed.
- No production user-session bus or installed LensGuard service is required.

Steps:

1. Start an isolated bus with `dbus-run-session`.
2. Start `target/debug/camera-monitor serve-dbus` inside it and wait for `Ping` to respond.
3. Run `gdbus introspect` against `io.github.younesrabeh.CameraMonitor` at
   `/io/github/younesrabeh/CameraMonitor`.
4. Read `Active` through `org.freedesktop.DBus.Properties.Get`.
5. Call `io.github.younesrabeh.CameraMonitor1.GetActiveSessions`.
6. Send SIGINT to the daemon and wait for a clean exit.

Expected result:

- Standard introspection shows the versioned interface, its methods, properties, and signals.
- `Active` is `false` and `GetActiveSessions` returns an empty array for the Step 6 standalone
  endpoint.
- The daemon reports readiness and exits successfully after SIGINT.

Actual result:

- Pass on 2026-08-04. `gdbus introspect` displayed the versioned interface with both methods,
  all four properties, and all four signals. The property read returned `(<false>,)` and the
  session call returned `(@a(sssssstu) [],)`. SIGINT produced a successful daemon exit.

Logs collected:

- Startup: `D-Bus camera monitor ready at io.github.younesrabeh.CameraMonitor
  /io/github/younesrabeh/CameraMonitor; press Ctrl+C to stop`.
- Ping: `('pong',)`.
- Introspection reported `GetActiveSessions(out a(sssssstu) arg_0)`, `readonly b Active = false`,
  `readonly u ActiveSessionCount = 0`, `readonly b BackendAvailable = true`, and
  `readonly s Version = '0.6.0'`.

Pass/fail:

- Pass.

Notes:

- The standalone endpoint is intentionally not connected to PipeWire until Step 7.
- This test uses metadata only and does not access camera frames.
