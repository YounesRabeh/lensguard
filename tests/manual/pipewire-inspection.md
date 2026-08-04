# PipeWire inspection smoke test

Run these checks from an ordinary terminal in the active GNOME user session. They cannot be
validated from a sandbox that denies access to `/run/user/$UID/pipewire-0`.

## Initial graph and no-camera case

1. Close applications that may capture video.
2. Run `cargo run -p camera-monitor -- inspect-pipewire`.
3. Confirm the command exits successfully and prints a concise graph summary.
4. It is valid for the summary to contain a camera-source candidate while no capture is active;
   this step observes available objects rather than active sessions.

## Camera and application candidates

1. Open a known camera application and begin preview/capture.
2. Run the inspection command again.
3. Confirm a `Video/Source` camera appears under `camera-source`.
4. Confirm the capturing application's `Stream/Input/Video` node appears under
   `application-input` when the session manager exposes it.
5. Stop capture and confirm the diagnostic still exits cleanly.

## Unavailable backend

Run against a deliberately nonexistent remote:

```sh
PIPEWIRE_REMOTE=lensguard-does-not-exist \
  cargo run -p camera-monitor -- inspect-pipewire
```

Confirm it exits unsuccessfully with an actionable connection error and does not panic.
