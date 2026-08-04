# Daemon lifecycle smoke test

Environment:

- Fedora 44 GNOME user session, PipeWire 1.6.8, D-Bus 1.16.2, and LensGuard 0.7.0.
- A private D-Bus session created with `dbus-run-session`; the normal user PipeWire socket remains
  available to the daemon.

Preconditions:

- `cargo build -p camera-monitor` passes.
- The command runs as the ordinary desktop user without `sudo`.
- `gdbus` and `dbus-run-session` are installed.

Steps:

1. Start `target/debug/camera-monitor --log-level debug run` in a private D-Bus session.
2. Wait for `io.github.younesrabeh.CameraMonitor1.Ping` to return.
3. Leave the daemon running for one second, confirm the process still exists, then read
   `BackendAvailable` and `GetActiveSessions`.
4. Send SIGTERM, wait for the process, and record its exit status.
5. Repeat with `PIPEWIRE_REMOTE=lensguard-step7-unavailable` to simulate backend startup loss.
6. Confirm the daemon remains responsive, reports `BackendAvailable=false` and `Active=false`,
   and logs retry attempts rather than exiting.
7. Send SIGTERM and verify exit status 0.
8. Start the unavailable-backend case once more, send SIGINT, and verify exit status 0.

Expected result:

- The normal daemon remains running, connects to PipeWire, and serves its D-Bus contract.
- SIGINT and SIGTERM both drain owned tasks and exit successfully.
- Backend loss is visible over D-Bus, starts bounded retries, and does not crash the daemon or
  falsely claim that observation is available.

Actual result:

- Pass on 2026-08-04. The normal daemon returned `pong`, remained alive, reported
  `BackendAvailable=true`, and returned an empty active-session array while no camera was active.
- SIGTERM stopped the normal daemon with status 0.
- The simulated-loss daemon returned `pong`, remained alive, reported `BackendAvailable=false`
  and `Active=false`, and stopped with status 0.
- A separate SIGINT run also stopped with status 0.

Logs collected:

- Normal startup: `camera monitor daemon ready ... queue_capacity=128`, followed by
  `PipeWire backend connected`.
- Normal shutdown: `backend event channel drained` and `camera monitor daemon stopped cleanly`.
- Simulated failure logged retry attempts 0, 1, and 2 without process exit. The observed delays
  followed the configured 250 ms, 500 ms, then 1 s progression.
- Exit markers: `SIGTERM_EXIT=0`, `BACKEND_LOSS_SIGTERM_EXIT=0`, and `SIGINT_EXIT=0`.

Pass/fail:

- Pass.

Notes:

- Fake-backend integration tests separately verify live start/update/stop publication, stale
  session clearing, and recovery with renewed observation.
- This test observes metadata only and never opens or reads camera frames.
