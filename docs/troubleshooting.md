# Troubleshooting

## D-Bus service is unavailable

Start `cargo run -p camera-monitor -- run` from a terminal in the same graphical user
session as the client. Confirm that `DBUS_SESSION_BUS_ADDRESS` is set, then call the `Ping` example
in `docs/dbus-api.md`. Sandboxes may expose the address while denying access to its Unix socket;
use an ordinary user-session terminal or an isolated `dbus-run-session` test in that case.

If startup reports that the name already exists, inspect its owner with:

```sh
busctl --user status io.github.younesrabeh.CameraMonitor
```

The `serve-dbus` diagnostic deliberately reports an empty inactive state. The normal `run` command
connects live PipeWire events; an empty session result while no application is capturing is not a
D-Bus failure.

## Backend remains unavailable

Run with `--log-level debug` and inspect the retry reason. The daemon initially reports
`BackendAvailable=false` while connecting and retries failures with bounded exponential backoff.
Check the active user's `XDG_RUNTIME_DIR`, `pipewire-0` socket, and `PIPEWIRE_REMOTE` value. A
backend failure also clears active sessions before publishing unavailability so clients do not
retain stale camera claims.

## PipeWire inspection cannot connect

Run the diagnostic from a terminal inside the active graphical login session:

```sh
cargo run -p camera-monitor -- inspect-pipewire
```

If it reports that the user instance is unavailable, check that `XDG_RUNTIME_DIR` belongs to the
current user, that its `pipewire-0` socket exists, and that PipeWire is running. Containers,
sandboxes, remote shells, and commands run as another user may see the socket but still be denied
permission to connect.

Useful comparisons are:

```sh
pw-cli info 0
wpctl status
```

## Candidate nodes are missing

The Step 3 classifier relies on the session-manager `media.class` property. A camera device is
normally `Video/Source`; an application capture stream is normally `Stream/Input/Video`. Inspect
the raw session graph with `pw-dump` when a vendor, virtual camera, sandbox, or older session
manager supplies different metadata.

Missing properties do not crash LensGuard. Objects begin as unclassified and may be reclassified
when bound-object info delivers later metadata. Malformed numeric node/link references are logged
and ignored rather than inserted as incorrect relationships.

## Candidate does not mean active camera access

The inspection command only discovers and classifies graph objects. A camera-source candidate may
exist while no application is capturing, and virtual or screen sources may also advertise a video
source class. Use the lifecycle watcher to see only correlated domain events:

```sh
cargo run -p camera-monitor -- watch-pipewire
```

An application merely being open should produce no output. Active direct capture produces
`START`, improved PipeWire metadata may produce `UPDATE`, and ending capture produces `STOP`.

## A camera application produces no lifecycle event

Compare `inspect-pipewire` output with `pw-dump`. Step 4 requires a direct complete link from a
`Video/Source` output port to a `Stream/Input/Video` input port. Some portals, virtual cameras, and
session-manager policies insert intermediate processing nodes; those multi-hop layouts are a
known limitation at this stage. Missing port ownership or direction metadata also keeps a
relationship unclassified rather than risking a false positive.

## Known limitations and ambiguous graph cases

LensGuard reports one row per active **camera-device to PipeWire application-node relationship**.
It intentionally does not merge rows by desktop application: two processes from the same app are
two rows, and one process using two cameras is also two rows. This preserves the privacy-relevant
device relationship even when application metadata is identical or incomplete.

Detection is currently limited to complete, direct PipeWire links from a `Video/Source` output to
a `Stream/Input/Video` input. Applications that bypass PipeWire and open `/dev/video*` directly
are outside the MVP and cannot be reported. Intermediate filters, virtual-camera chains,
session-manager-specific graph layouts, and portal graphs without the required ownership and
direction metadata can also be ambiguous and are ignored instead of guessed.

Device identity prefers a hardware serial, device name, or stable PipeWire node name. Two cameras
with the same display label still remain separate while their raw nodes are distinct, but a device
that exposes none of those stable properties may receive a new identifier after unplug/replug.
Application names may fall back to PipeWire metadata when a process exits early, `/proc` is hidden,
desktop-entry lookup times out, or sandbox metadata is absent. These fallbacks do not suppress the
active-camera warning.

Malformed numeric graph references are rejected and logged; oversized and unrelated properties
are not retained. A backend disconnect clears its complete prior snapshot and marks monitoring
unavailable before a fresh snapshot is accepted, so loss of observation is never presented as a
confident inactive state.
