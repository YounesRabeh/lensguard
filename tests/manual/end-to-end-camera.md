# End-to-end camera test

Environment:

- Fedora Linux 44 Workstation, GNOME Shell 50.3, GJS 1.88.1.
- Integrated camera exposed as `/dev/video0` and `/dev/video1` and as
  `Integrated Camera (V4L2)` in PipeWire.
- GNOME Camera (`org.gnome.Snapshot`, version 50.0) used as the capture application.
- Test recorded on 2026-08-04 in the active Wayland user session.

Preconditions:

- Build the daemon and extension with `make build`.
- Run commands from the graphical user's session so `XDG_RUNTIME_DIR`, PipeWire, and the user
  D-Bus are reachable.
- Ensure no other process owns `io.github.younesrabeh.CameraMonitor`.
- A physical camera must be available and permitted for GNOME Camera.

Steps:

1. Start a diagnostic live daemon:

   ```sh
   target/debug/camera-monitor --log-level debug run
   ```

2. Start GNOME Camera and wait for its preview:

   ```sh
   snapshot --debug
   ```

3. Confirm the live D-Bus properties and session list:

   ```sh
   gdbus call --session \
     --dest io.github.younesrabeh.CameraMonitor \
     --object-path /io/github/younesrabeh/CameraMonitor \
     --method org.freedesktop.DBus.Properties.GetAll \
     io.github.younesrabeh.CameraMonitor1

   gdbus call --session \
     --dest io.github.younesrabeh.CameraMonitor \
     --object-path /io/github/younesrabeh/CameraMonitor \
     --method io.github.younesrabeh.CameraMonitor1.GetActiveSessions
   ```

4. Stop the diagnostic daemon, keep camera capture active, and run the full Shell test:

   ```sh
   ./scripts/smoke-real-camera.sh
   ```

5. After `LENSGUARD_REAL_CAMERA_ACTIVE` appears, stop or suspend camera capture within 30 seconds.
6. Close GNOME Camera and confirm the live daemon, if still running, reports `Active=false`, count
   zero, and an empty session list.

Expected result:

- The daemon publishes one session while capture is active.
- The top-bar camera icon appears within a short delay.
- The LensGuard menu identifies both the responsible application and camera.
- Stopping the final capture removes the icon and changes the tile to `No camera in use`.
- The extension disables cleanly and the daemon exits cleanly.

Actual result:

- The live daemon published `Active=true`, `ActiveSessionCount=1`, and one session with application
  ID `org.gnome.Snapshot`, display name `Camera`, and device `Integrated Camera (V4L2)`.
- The daemon emitted its relationship start about 73 ms after GNOME Camera realized the viewfinder.
- During the complete isolated-Shell run, the PipeWire start was logged at 19:43:59.321 and the
  extension logged `LENSGUARD_REAL_CAMERA_ACTIVE Camera — Integrated Camera (V4L2)` at
  19:43:59.337, about 16 ms later.
- Camera suspension removed the PipeWire link at 19:44:03.363; the extension logged
  `LENSGUARD_REAL_CAMERA_INACTIVE` at 19:44:03.378, about 15 ms later, and hid the icon.
- After GNOME Camera was closed, D-Bus reported `Active=false`, count zero, and an empty session
  array. GNOME Camera and both test daemons exited cleanly.

Logs collected:

- Real daemon debug log containing PipeWire node/link and relationship transitions.
- GNOME Camera debug log confirming portal access and a playing camera pipeline.
- Headless GNOME Shell log containing both `LENSGUARD_REAL_CAMERA_ACTIVE` and
  `LENSGUARD_REAL_CAMERA_INACTIVE`.
- Final line: `GNOME Shell real-camera smoke test passed.`

Pass/fail:

PASS

Notes:

- A newly copied local extension is discovered by the live GNOME Shell after the next login; the
  isolated Shell is used for repeatable automation without ending the user's session.
- The display name `Camera` comes from GNOME Camera's desktop entry. The stable application ID in
  the D-Bus payload remains `org.gnome.Snapshot`.
