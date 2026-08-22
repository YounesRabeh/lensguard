# Getting started

Install the native `lensguard-service` package, which enables and starts
`lensguard-v4l2-observer.service`, then install one copy of the GNOME extension. Open Quick Settings
to verify “No camera in use” and no warning icon.

Start an application configured for direct V4L2 streaming capture. After successful stream-on,
LensGuard shows one row with the resolved application and camera. Ending capture removes it.

An observer warning means coverage is unavailable, not that the camera is safe. “Unknown camera
activity” means ownership could not be classified and intentionally does not create a normal
application row. Camera use through a trusted desktop broker is left to GNOME's privacy indicator.

See [troubleshooting](troubleshooting.md) for availability categories and capture-path checks.
