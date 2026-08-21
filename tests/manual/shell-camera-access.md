# Direct V4L2 camera-access checks

These scripts use GStreamer's `v4l2src` streaming path and discard frames without recording them.

```bash
camera-monitor watch-v4l2
./tests/manual/shell-camera-access.sh /dev/video0 30
./tests/manual/shell-camera-access-multiple.sh /dev/video0 2 30
```

Verify one direct session per successful client, no session before stream-on, and prompt removal
after stream-off or close. Some devices reject concurrent clients; the multiple-client script
reports that hardware/driver limitation explicitly.

Also verify negative cases:

- `v4l2-ctl --device /dev/video0 --all` opens and queries the node but creates no session.
- a capture attempt against an already-busy device creates no new confirmed session.
- a desktop camera application using the trusted broker creates no LensGuard session or app row.
- stopping or restarting the system observer clears active sessions and shows observer
  unavailability.
