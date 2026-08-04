# Terminal camera-access checks

These scripts open a camera from a terminal, without GNOME Camera or a graphical preview. They
discard all frames and never create a recording.

## Direct V4L2 access (tests the driver)

Run a single camera client for 30 seconds:

```bash
./tests/manual/shell-camera-access.sh /dev/video0 30
```

Run two simultaneous direct-V4L2 clients for 30 seconds:

```bash
./tests/manual/shell-camera-access-multiple.sh /dev/video0 2 30
```

Use a different `/dev/videoN` path when the integrated camera is exposed under another node.
Start `camera-monitor watch-pipewire` or open LensGuard Quick Settings before running either
script, then confirm the reported active session count and app rows.

Some camera drivers allow only one direct V4L2 streaming client. In that case the multiple-client
script exits with a clear message; that is a driver limitation, not an application failure. Test
multiple cameras by running the single-client script once per device instead.

LensGuard's current backend observes PipeWire graph sessions. A direct V4L2 process that does not
create a PipeWire session is useful for checking the device and driver, but will not appear in
LensGuard until direct-V4L2 monitoring is implemented.
