# Step 11 robustness report

Date: 2026-08-04  
LensGuard version: 1.1.0  
Host: Fedora 44, GNOME Shell/Mutter 50.3, one integrated V4L2 camera

## Result

Step 11 passes. Backend loss, stale events, process exit, camera removal, identifier changes,
malformed metadata, resolver stalls, repeated D-Bus generations, and multi-camera graphs converge
to deterministic privacy state without manual recovery.

## Fault-injection coverage

- Out-of-order and duplicate registry events reconcile without duplicate sessions.
- Camera-node or application-node removal emits one stop even when link cleanup arrives late.
- A missed remove followed by backend disconnect clears the old snapshot; reconnect accepts only
  the complete fresh snapshot.
- A one-second production metadata-resolution timeout preserves PipeWire identity. Only one
  blocking resolver worker may remain active, preventing detached-worker accumulation.
- Application exit or inaccessible process metadata does not clear camera activity before the
  PipeWire relationship ends.
- Malformed numeric relationships are rejected without poisoning the correlation engine.
- Retained PipeWire properties are allowlisted and limited to 512 characters per value. Resolver
  identity text is limited to 256 characters, the production resolution cache remains bounded,
  and backend failure text is limited before logging or publication.

## Multi-device behavior

The UI model is one row per camera-device to PipeWire application-node relationship. Tests cover:

- two cameras and one application;
- one camera and two applications;
- two cameras and two applications (four relationships);
- cameras with identical display/model names but distinct stable node identities;
- two process nodes with the same desktop application ID;
- unplug while active and replug with changed raw PipeWire IDs.

This host exposes one physical camera, so real two-camera hardware validation was not available.
The graph combinations above use the same raw registry and correlation boundaries as the live
adapter, with deterministic synthetic events.

## Resource and lifecycle checks

- 2,000 start/stop cycles completed with task growth of 2 and resident-memory growth of 3,148 KiB,
  within the test limits of 3 tasks and 32 MiB.
- Twenty-five D-Bus service disappear/reappear generations disconnected both proxy signal
  subscriptions each time and resynchronized the current snapshot.
- Twelve GNOME Shell extension disable/enable cycles retained exactly one indicator while enabled
  and zero after disable.
- The PipeWire inspector now releases its registry callback reference before returning the raw
  graph; the live inspection completed without retained internal graph references.

## Verification

- `CC=/usr/bin/gcc make check`: pass (format, Clippy with warnings denied, 82 Rust tests,
  extension checks, isolated D-Bus integration, workspace build, extension package).
- `CC=/usr/bin/gcc make smoke-extension`: pass on headless GNOME Shell 50.3.
- Live `inspect-pipewire`: pass; 11 nodes, 25 ports, one integrated camera candidate.
- Live `watch-pipewire` with GNOME Camera: one matching `START` and `STOP` lifecycle.
- ShellCheck was not installed, so the optional shell lint remained skipped as documented by the
  project check target.

## Known limitations

The detailed limitations and ambiguity policy are maintained in `docs/troubleshooting.md`.
Direct `/dev/video*` access, unsupported multi-hop/virtual graphs, and graphs missing required
ownership or direction metadata are not guessed. Backend loss is shown as unavailable rather than
inactive.
