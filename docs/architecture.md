# Architecture

LensGuard uses ports and adapters. The `camera-core` crate owns domain meaning; desktop and
transport integrations translate their native data before crossing that boundary.

```text
PipeWire adapter ─┐
                  ├─ MonitorEvent → MonitorState → MonitorSnapshot ─┐
Other adapters ───┘                                                  ├─ application observers
                                                                     └─ D-Bus adapter
```

## Dependency direction

`camera-core` has no Cargo dependencies and does not know about `PipeWire`, D-Bus, GJS, GNOME
Shell, systemd, process inspection, or an async runtime. Adapter crates may depend on the core;
the core must never depend on an adapter. Raw backend object IDs cannot cross into the domain.

The current Step 2 crate modules are:

- `model`: stable typed IDs, camera devices, application identity, sessions, and detection
  backend identity;
- `event`: the complete set of state-reducer inputs;
- `state`: the active-session registry and deterministic public snapshots;
- `ports`: synchronous source and observer traits that future application code can bridge to an
  async runtime;
- `error`: typed validation errors for domain values.

## Identity and session invariants

`SessionId` and `DeviceId` are non-empty strings whose values are assigned above the domain
boundary. They are intentionally unrelated to transient `PipeWire` numeric IDs. A session ID
defines the identity of one application-to-camera relationship; metadata may be replaced or
enriched while that ID remains unchanged.

`MonitorState` maintains these rules:

1. A `BTreeMap` stores at most one session per ID and produces snapshots sorted by ID.
2. Repeating an identical start or update is a no-op.
3. A changed start replaces metadata for its existing ID; a changed update enriches an existing
   session.
4. An update or stop for an unknown ID is ignored safely.
5. Active state is computed from whether the registry is empty, never stored separately.
6. Backend availability changes are idempotent and retain an actionable unavailable reason.

Backend loss does not clear sessions in the pure reducer. Reconnection and stale-session
reconciliation require adapter lifecycle knowledge and belong to the later daemon orchestration
step.

## Port boundaries

`CameraEventSource` yields domain events and `SessionObserver` consumes complete ordered
snapshots. Both expose associated typed errors and are synchronous by design. This keeps runtime
selection and channel ownership outside the domain while still allowing fake implementations in
tests.

D-Bus data-transfer objects are defined separately in `camera-dbus`; the domain entities in this
document are not the public wire contract.

## PipeWire registry adapter

`camera-pipewire` owns all transient numeric `PipeWire` IDs. Registry and bound-object callbacks
are copied immediately into owned string property maps and represented as `RegistryEvent` values.
The same events drive synthetic unit and fixture tests, so graph behavior is independent of a
desktop session.

The raw graph stores nodes, ports, and links in ordered maps. Node info and port/link info events
merge late metadata into the original registry properties. Removing a node also removes known
child ports and connected links; later removal callbacks are harmless.

Step 3 classification follows the session-manager media classes:

- `Video/Source` is a camera-source candidate;
- `Stream/Input/Video` is an application video-input candidate;
- audio classes and video output/playback classes are unrelated.

These classifications remain candidates until the Step 4 correlation conditions below hold.

`camera-monitor inspect-pipewire` performs two `PipeWire` synchronization barriers, prints a
deterministically ordered summary, and exits. Core errors are converted into typed errors rather
than panics. Long-running retry and reconnection policy remains daemon-orchestration scope.

## PipeWire camera relationship correlation

Step 4 converts raw graph changes to `camera-core::MonitorEvent` values. A relationship is active
only when all of these conditions hold:

1. A complete PipeWire link identifies both endpoint nodes and ports.
2. The output node has `media.class=Video/Source`.
3. The input node has `media.class=Stream/Input/Video`.
4. Both endpoint ports exist, belong to the linked nodes, and have output/input direction
   respectively.

The adapter deduplicates relationships by camera-node/application-node pair rather than link ID.
This makes duplicate links and harmless link replacement invisible to the domain. An opaque,
deterministic hash of that pair becomes the domain session ID; raw PipeWire IDs never cross the
adapter boundary. The original start timestamp is retained when metadata improves.

Registry callbacks and native objects live on a dedicated PipeWire main-loop thread. The public
`PipeWireEventSource` implements the synchronous `CameraEventSource` port and passes only domain
events over a channel. A backend failure first ends known sessions and then emits
`BackendUnavailable`, avoiding a false claim that the camera is inactive.

Direct links are the supported Step 4 topology. Graphs containing converter or portal nodes
between the camera and application may require multi-hop correlation later. Candidate source
classification also cannot distinguish every physical camera from virtual or screen sources.
Application metadata enters the domain from PipeWire before the Step 5 enrichment stage described
below.

## Application identity resolution

`camera-app-resolver` enriches `SessionStarted` and `SessionUpdated` events in the daemon after
they leave the dedicated PipeWire callback thread. Resolution therefore cannot add procfs or
desktop-file latency to native PipeWire callbacks.

The resolver uses this deterministic precedence:

1. trusted PipeWire application ID, display name, binary, and PID hints;
2. a matching desktop entry's `Name`, especially when the backend name is a machine identifier;
3. permitted `/proc/<pid>` process name and executable basename;
4. Flatpak identity from `/proc/<pid>/root/.flatpak-info` or a recognizable cgroup unit;
5. a normalized backend/process identifier, then `Unknown application`.

Desktop entries are indexed from the standard XDG user and system `applications` directories.
Only the desktop-file ID, `Name`, `Exec` basename, and `X-Flatpak` ID are retained. Icon paths and
process command lines are deliberately excluded from the trust boundary. Newline and control
characters are normalized before a fallback is displayed.

Process exit and permission failures are non-fatal: the session is preserved with the best
remaining identity. Resolutions use a bounded cache keyed by all identity inputs. Exact entries
can be invalidated, and the complete cache can be cleared when desktop-entry state changes.

## D-Bus adapter

`camera-dbus` owns the public transport contract. It converts `MonitorSnapshot` sessions into a
fixed `(sssssstu)` DTO and never exports backend-native objects, raw PipeWire IDs, executable
paths, node names, or icon paths. Returned arrays are sorted by the opaque domain session ID.

The service owns `io.github.younesrabeh.CameraMonitor` on the user session bus and exports the
versioned `io.github.younesrabeh.CameraMonitor1` interface. Domain events are reduced through
`MonitorState` before the adapter emits semantic signals and standard property-change
notifications. Duplicate domain events therefore produce no public notification. The canonical
contract is in `dbus/io.github.younesrabeh.CameraMonitor1.xml` and client semantics are documented
in `docs/dbus-api.md`.

The `serve-dbus` command publishes an empty standalone state so the transport can be tested without
PipeWire. The normal daemon path is described below.

## Daemon orchestration and recovery

`camera-monitor` owns three explicitly separated stages:

```text
PipeWire main-loop thread
  → bounded adapter queue (256)
  → blocking backend supervisor
  → bounded application queue (128)
  → resolver/state task
  → bounded publication queue (128)
  → D-Bus publisher task
```

Native callbacks copy and correlate metadata on the dedicated PipeWire thread and use a
non-blocking bounded send. Queue overload ends that observation so the supervisor can reconnect
and reconcile rather than grow memory or silently retain stale state. The supervisor polls the
source with a cancellation interval, reports availability events, and retries initialization or
disconnects with exponential delays from 250 ms through a 30-second maximum.

Application identity lookup runs on Tokio's blocking pool. The bounded input queue continues
accepting backend events while the ordered application task awaits procfs or desktop metadata. It
owns an independent `MonitorState` reducer. Before publishing a backend-unavailable
event, it emits deterministic stop events for every active session; a later connection emits
`BackendRecovered` and rebuilds observation from a fresh PipeWire graph.

The publisher is the only owner that mutates the exported D-Bus state. SIGINT and SIGTERM set a
shared cancellation flag, stop and join the backend, drain both bounded queues, release the D-Bus
name, and enforce a five-second deadline for asynchronous task cleanup. Normal operation uses
only user-session PipeWire and D-Bus resources and requires no elevated privileges.
