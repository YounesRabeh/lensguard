# Troubleshooting

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
