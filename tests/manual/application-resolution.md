# Application-resolution smoke test

Run this check from a terminal in the active GNOME session after `make check` passes.

## Native application

1. Run `cargo run -p camera-monitor -- watch-pipewire`.
2. Start camera preview in a native camera application.
3. Confirm the `START` event contains a recognizable human-readable application name.
4. Stop preview and confirm the matching `STOP` event.

## Flatpak application, when available

1. Run `flatpak list --app` and choose an installed application that can capture the camera.
2. Start the watcher and camera capture in that application.
3. Confirm the `START` event reports its application ID or desktop-entry display name.
4. Stop capture and confirm the matching `STOP` event.

## Actual result

- Native application: pass on 2026-08-04. Starting GNOME Camera (Snapshot) preview emitted
  `START session=pipewire-5122fe9b6394fedf application="Camera" camera="Integrated Camera
  (V4L2)"`; closing it emitted a matching `STOP`.
- Flatpak application: not run. Flatpak applications are installed, but no camera capture could be
  started non-interactively in the available applications; this remains a manual check.

Do not record camera frames, full command lines, or unrelated process metadata in this file.
