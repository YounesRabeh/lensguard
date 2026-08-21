# Troubleshooting

## Service unavailable

Confirm the native package and both units exist:

```bash
systemctl status lensguard-v4l2-observer.service
systemctl --user status camera-monitor.service
camera-monitor inspect-v4l2
```

`not-installed` usually means the Unix socket is absent. `missing-capability` means the system
service could not load or attach BPF. `unsupported-kernel` means a required tracepoint or kernel
facility is absent. `blocked-by-policy` covers a missing/malformed broker policy or a kernel
security denial. `version-mismatch` requires upgrading the extension and native service together.

Inspect logs with:

```bash
journalctl -u lensguard-v4l2-observer.service -b
journalctl --user -u camera-monitor.service -b
```

Do not run `camera-monitor` as root. Only the separately packaged observer is privileged.

## Direct application is not shown

- Confirm the application actually opens `/dev/videoN` directly. If it uses the trusted desktop
  camera broker, GNOME owns that indication and LensGuard suppresses it.
- Confirm it uses streaming I/O and successfully calls `VIDIOC_STREAMON`. Device opens, capability
  queries, failed stream starts, and V4L2 read-I/O do not activate this release.
- Check `ObserverAvailability` and `UnknownCameraActivity`. An inaccessible executable, ambiguous
  broker lookalike, device-resolution race, or policy failure is suppressed rather than guessed.
- Sandboxed browsers may select different capture paths depending on portal and permission state.

## Unknown camera activity

This stable diagnostic contains no application, PID, device, path, or event history. It means the
observer saw a confirmed candidate but could not safely prove direct ownership. Review observer
logs and package policy integrity; never weaken the broker policy to force a label.
