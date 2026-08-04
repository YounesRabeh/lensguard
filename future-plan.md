# Future plan: direct V4L2 camera-access detection

## Timing

Do not begin this work before Plan.md Step 14 is complete, the v1.0 release is stable, and its
PipeWire-based detection has been validated in normal daily use. This is post-v1.0 work, not a
replacement for the current PipeWire monitor.

## Motivation

LensGuard currently observes camera sessions represented in the PipeWire graph. An application
that opens `/dev/videoN` directly through V4L2 can bypass PipeWire, so LensGuard cannot currently
report it. The terminal scripts in `tests/manual/` demonstrate that limitation.

## Goal

Detect direct V4L2 camera access by unprivileged applications while preserving the existing
PipeWire detection path, privacy guarantees, and GNOME Shell user experience.

## Design requirements

- Do not open camera devices, read frames, or change device configuration.
- Do not require root, a system-wide daemon, kernel patches, or broad process tracing by default.
- Treat device existence as distinct from active capture; opening a device alone must not produce a
  false “camera in use” result.
- Associate direct access with a process and desktop application only when the identity can be
  established safely.
- Deduplicate one physical capture that is visible through both PipeWire and V4L2.
- Preserve the current unavailable-state warning if a direct-V4L2 backend cannot be started.
- Keep all monitoring local, metadata-only, and free of telemetry or persistent history.

## Investigation phase

1. Evaluate unprivileged Linux mechanisms for detecting V4L2 streaming state, including sysfs,
   V4L2 event/IOCTL capabilities, PipeWire/WirePlumber metadata, and narrowly scoped eBPF or
   audit-based options where available.
2. Document what each mechanism can prove: device open, stream-on, stream-off, process identity,
   and device identity.
3. Build a compatibility matrix for UVC webcams, integrated cameras, libcamera devices, virtual
   cameras, Flatpaks, browsers, and applications that access `/dev/videoN` directly.
4. Reject approaches that require reading frames, create false positives for a mere device probe,
   or need privileged always-on monitoring without an explicit product decision.

## Implementation outline

1. Define a backend-neutral direct-V4L2 event contract in `camera-core`.
2. Implement the selected direct-V4L2 observer behind a feature flag and explicit availability
   reporting.
3. Resolve process and application identity using the existing resolver, with a safe unknown-app
   fallback.
4. Correlate direct-V4L2 and PipeWire events into one camera session when they describe the same
   physical capture.
5. Expose the source backend in diagnostics only; keep the normal Quick Settings UI focused on
   the application and camera.
6. Make the feature opt-in until it has passed compatibility and privacy review.

## Required tests before enabling by default

- Unit tests for direct-access state transitions, process identity fallback, deduplication, and
  backend loss.
- Integration tests with synthetic direct-V4L2 start/stop events and mixed PipeWire/V4L2 events.
- Manual terminal checks using `tests/manual/shell-camera-access.sh` and
  `tests/manual/shell-camera-access-multiple.sh`.
- Manual checks for a browser, Discord/Meet-like client, native V4L2 client, Flatpak client,
  virtual camera, camera removal, daemon restart, and user logout/login.
- Privacy review confirming no frames, command lines, or persistent application history are read
  or stored.
- Performance and battery measurements while the camera is idle and while several clients access
  it.

## Release criteria

Enable direct-V4L2 monitoring by default only after it reliably detects real streaming activity,
does not duplicate PipeWire sessions, remains unprivileged, passes the full test suite, and has
clear user-facing documentation of any remaining blind spots.
