# Camera-session lifecycle smoke test

Environment:

- Fedora 44 GNOME desktop session, PipeWire 1.6.8, WirePlumber 0.5.14.
- Run from a terminal belonging to the active graphical user session.

Preconditions:

- `make check` passes.
- A known camera application can preview the camera through PipeWire.
- No other application is capturing video.

Steps:

1. Start `cargo run -p camera-monitor -- watch-pipewire`.
2. Open the camera application without starting preview or capture. Wait five seconds.
3. Confirm no `START` event appears.
4. Begin preview or capture.
5. Confirm exactly one `START` appears with an opaque session ID, application fallback name, and
   camera name.
6. Leave capture active for five seconds and confirm no duplicate `START` appears.
7. Stop capture and confirm exactly one `STOP` appears with the same session ID.
8. Close the watcher with Ctrl+C.

Expected result:

- Opening the application alone emits nothing.
- Capture emits one start, optional metadata updates, and one matching stop.
- No raw PipeWire IDs, panic, or unrelated audio/playback events are printed.

Actual result:

- 2026-08-04: the watcher connected to the live PipeWire session. Starting GNOME Snapshot preview
  emitted exactly one `START`; stopping preview emitted exactly one `STOP` with the same session
  ID. Snapshot remained open without capturing for another five seconds and emitted no event.

Logs collected:

- Startup reported `PipeWire camera relationship monitor ready; press Ctrl+C to stop`.
- Start: `START session=pipewire-ebacf547791b79f7 application="org.gnome.Snapshot"
  camera="Integrated Camera (V4L2)"`.
- Stop: `STOP session=pipewire-ebacf547791b79f7`.
- No lifecycle event was emitted during the five-second open-but-inactive observation period.

Pass/fail:

- Pass.

Notes:

- If active capture emits nothing, save relevant sanitized `pw-dump` objects before reporting the
  topology. Do not include camera frames or full process command lines.
