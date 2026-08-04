# Camera Access Indicator for GNOME — Implementation Plan

> A staged implementation plan for a GNOME Shell camera-privacy indicator backed by a Rust monitoring daemon.
>
> This file is intended to be given directly to Codex. Codex must execute one step at a time, run the required checks, report the result, and stop until the user explicitly approves moving to the next step.

---

## 1. Project Summary

Build a Linux desktop privacy tool that displays a GNOME Shell panel indicator whenever an application actively uses a camera.

The system consists of:

1. A **Rust user daemon** that observes PipeWire, identifies active camera sessions, resolves the responsible application, and exposes state through D-Bus.
2. A **GNOME Shell extension written in GJS/JavaScript** that displays the indicator and presents active camera sessions.
3. **Packaging and user-session integration** through systemd and D-Bus activation.

### MVP behavior

- Detect active camera capture through PipeWire.
- Distinguish camera devices from application video-input streams.
- Correlate each active application stream with the camera device it uses.
- Expose active sessions over the user D-Bus.
- Show a GNOME top-panel camera indicator while at least one session is active.
- Show active application and device information in the indicator menu.
- Recover cleanly when PipeWire, D-Bus, the daemon, or the extension restarts.

### Explicit MVP non-goals

Do not implement these until the core MVP is complete:

- eBPF monitoring.
- Kernel modules.
- Direct `/dev/video*` blocking.
- Camera kill switch.
- Permanent access history.
- Cloud services or telemetry.
- Mobile or non-GNOME desktop support.
- Polkit or root privileges.

---

## 2. Architecture

```text
┌──────────────────────────────────────────────┐
│ GNOME Shell extension                       │
│                                              │
│ GJS / JavaScript                            │
│ - panel privacy indicator                   │
│ - active-session menu                       │
│ - notifications and preferences             │
└──────────────────────┬───────────────────────┘
                       │ User-session D-Bus
┌──────────────────────▼───────────────────────┐
│ Rust daemon                                  │
│                                              │
│ - daemon orchestration                       │
│ - session state                              │
│ - application resolution                    │
│ - D-Bus service                              │
└──────────────────────┬───────────────────────┘
                       │ Port / adapter boundary
┌──────────────────────▼───────────────────────┐
│ PipeWire adapter                             │
│                                              │
│ - registry observation                       │
│ - node, port, and link tracking              │
│ - camera-session correlation                 │
└──────────────────────────────────────────────┘
```

### Dependency direction

Use a ports-and-adapters structure.

```text
GNOME extension ──D-Bus──> daemon application layer
                                │
                                ▼
                           domain/core
                                ▲
                                │
                 PipeWire and process adapters
```

Rules:

- The domain/core crate must not depend on PipeWire, zbus, GJS, GNOME Shell, or systemd.
- PipeWire-specific objects must not leak into the domain API.
- D-Bus DTOs must be separate from internal domain entities.
- The extension must communicate with the daemon only through the documented D-Bus interface.
- Infrastructure failures must be converted into typed application errors.

---

## 3. Proposed Repository Layout

```text
camera-access-indicator/
├── Cargo.toml                         # Rust workspace
├── Cargo.lock
├── Makefile
├── README.md
├── LICENSE
├── Plan.md
├── .gitignore
├── .editorconfig
├── rustfmt.toml
├── deny.toml                         # optional cargo-deny policy
├── crates/
│   ├── camera-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── model.rs
│   │       ├── event.rs
│   │       ├── state.rs
│   │       ├── ports.rs
│   │       └── error.rs
│   ├── camera-pipewire/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── registry.rs
│   │       ├── graph.rs
│   │       ├── mapper.rs
│   │       └── error.rs
│   ├── camera-app-resolver/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── procfs.rs
│   │       ├── desktop_entry.rs
│   │       ├── flatpak.rs
│   │       └── error.rs
│   └── camera-dbus/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── dto.rs
│           ├── service.rs
│           └── error.rs
├── daemon/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── application.rs
│   │   ├── config.rs
│   │   ├── runtime.rs
│   │   └── shutdown.rs
│   └── tests/
│       ├── fake_backend.rs
│       ├── lifecycle.rs
│       └── dbus_contract.rs
├── extension/
│   ├── extension.js
│   ├── metadata.json
│   ├── stylesheet.css
│   ├── prefs.js
│   ├── src/
│   │   ├── indicator.js
│   │   ├── menu.js
│   │   ├── dbusClient.js
│   │   ├── sessionModel.js
│   │   ├── notifications.js
│   │   └── settings.js
│   ├── schemas/
│   │   └── org.gnome.shell.extensions.camera-access-indicator.gschema.xml
│   └── tests/
│       ├── sessionModel.test.js
│       └── fixtures.js
├── dbus/
│   └── io.github.younesrabeh.CameraMonitor1.xml
├── systemd/
│   ├── camera-monitor.service
│   └── io.github.younesrabeh.CameraMonitor.service
├── scripts/
│   ├── bootstrap-dev.sh
│   ├── check.sh
│   ├── install-local.sh
│   ├── uninstall-local.sh
│   ├── run-daemon.sh
│   ├── package-extension.sh
│   └── smoke-test.sh
├── packaging/
│   └── rpm/
│       ├── camera-access-indicator.spec
│       └── README.md
├── tests/
│   ├── fixtures/
│   │   ├── pipewire/
│   │   └── desktop-files/
│   └── manual/
│       ├── camera-session.md
│       ├── multiple-apps.md
│       └── restart-recovery.md
└── docs/
    ├── architecture.md
    ├── development.md
    ├── dbus-api.md
    ├── testing.md
    ├── troubleshooting.md
    └── release.md
```

Codex may adjust file names when justified, but it must preserve the module boundaries and dependency direction.

---

## 4. Technology Stack

### Rust daemon

- Stable Rust, edition selected from the currently installed stable toolchain.
- Cargo workspace.
- `pipewire` Rust bindings for PipeWire observation.
- `zbus` for user-session D-Bus.
- Tokio for asynchronous daemon orchestration and channels.
- `serde` for internal serialization where useful.
- `tracing` and `tracing-subscriber` for structured logs.
- `thiserror` for library errors.
- `anyhow` only at the binary/application boundary when helpful.

### GNOME extension

- GJS JavaScript ES modules.
- GNOME Shell APIs.
- `Gio.DBusProxy` for daemon communication.
- GSettings for extension preferences.
- Symbolic GNOME icons.

### System integration

- User-session D-Bus.
- systemd user service.
- D-Bus activation.
- PipeWire and WirePlumber already supplied by the desktop session.

### Quality tooling

- `cargo fmt`.
- `cargo clippy` with warnings denied in CI/check scripts.
- `cargo test --workspace`.
- JavaScript linting appropriate to GJS.
- Shell scripts checked with ShellCheck when installed.
- Markdown checked manually or with a configured linter when available.

Do not pin dependency versions by guessing. Codex must select current compatible versions, record them in `Cargo.lock`, and document any important minimum runtime requirements.

---

## 5. Core Domain Model

Use stable internal identifiers and avoid coupling domain entities to PipeWire IDs.

Suggested entities:

```rust
pub struct CameraDevice {
    pub id: DeviceId,
    pub display_name: String,
    pub node_name: Option<String>,
}

pub struct ApplicationIdentity {
    pub pid: Option<u32>,
    pub app_id: Option<String>,
    pub display_name: String,
    pub binary: Option<String>,
}

pub struct CameraSession {
    pub id: SessionId,
    pub application: ApplicationIdentity,
    pub device: CameraDevice,
    pub started_at_unix_ms: u64,
    pub backend: DetectionBackend,
}

pub enum MonitorEvent {
    SessionStarted(CameraSession),
    SessionUpdated(CameraSession),
    SessionStopped(SessionId),
    BackendUnavailable { reason: String },
    BackendRecovered,
}
```

Required state behavior:

- Events are idempotent.
- Duplicate start events do not create duplicate sessions.
- Unknown stop events do not crash the daemon.
- Session replacement and metadata enrichment are supported.
- Active state is derived from `active_sessions.is_empty()`.
- Public snapshots are deterministic and consistently ordered.

---

## 6. D-Bus Contract

Use the user-session bus.

Suggested names:

```text
Bus name:   io.github.younesrabeh.CameraMonitor
Object:     /io/github/younesrabeh/CameraMonitor
Interface:  io.github.younesrabeh.CameraMonitor1
```

Minimum API:

### Read-only properties

- `Active: bool`
- `ActiveSessionCount: u32`
- `BackendAvailable: bool`
- `Version: string`

### Methods

- `GetActiveSessions() -> array<session>`
- `Ping() -> string`

### Signals

- `StateChanged`
- `SessionStarted(session)`
- `SessionStopped(session_id)`
- `BackendAvailabilityChanged(available)`

The exact D-Bus signature must be documented in the introspection XML and `docs/dbus-api.md`. Prefer simple, stable D-Bus types. Avoid exposing JSON as the primary contract.

---

## 7. Definition of Done for Every Step

A step is complete only when all applicable items are satisfied:

- The step goal is implemented without knowingly implementing later-step scope.
- Code follows the architecture and dependency rules.
- New public behavior has tests.
- Existing tests pass.
- Formatting and lint checks pass.
- No unexplained warnings remain.
- Errors contain actionable context.
- Relevant documentation is updated.
- Manual verification instructions are written when automation is impossible.
- Codex reports files changed, commands run, test results, limitations, and remaining risks.
- Codex stops and waits for explicit approval before starting the next step.

---

## 8. Codex Execution Rules

Codex must follow this procedure for every requested step.

### Before coding

1. Read this entire `Plan.md`.
2. Inspect the current repository state.
3. Confirm the requested step and its dependencies.
4. Do not start a later step.
5. State a brief implementation approach before modifying files.

### During coding

- Prefer small, reviewable changes.
- Keep domain logic pure and testable.
- Avoid global mutable state.
- Avoid `unwrap()` and `expect()` in production paths unless an invariant is proven and documented.
- Do not suppress compiler or linter warnings without justification.
- Do not introduce placeholder production implementations presented as complete.
- Do not add unrelated refactors.
- Preserve backwards compatibility with established contracts unless the user approves a change.

### At the end of a step

Codex must provide:

```text
Step completed:
Summary:
Files changed:
Commands run:
Automated tests:
Manual tests still required:
Known limitations:
Architecture deviations:
Recommended review points:
```

Then stop. Do not continue automatically.

---

# Iterative Implementation Steps

---

## Step 1 — Repository Bootstrap and Development Baseline

### Goal

Create a buildable repository skeleton with verified local prerequisites, quality commands, module boundaries, and no application behavior yet.

### Systems to develop

- Cargo workspace.
- Empty Rust crates and daemon binary.
- GNOME extension skeleton.
- Build/check scripts.
- Initial documentation.
- Local environment discovery.

### Checklist

- [x] Detect and document the installed GNOME Shell version.
- [x] Detect and document the installed Rust toolchain.
- [x] Verify PipeWire, WirePlumber, GJS, D-Bus tools, and systemd user-session availability.
- [x] Create the repository structure described above.
- [x] Create a Cargo workspace with compiling placeholder library crates.
- [x] Add a daemon binary that starts, logs its version, and exits successfully with `--version`.
- [x] Add extension metadata using the locally installed GNOME Shell compatibility value.
- [x] Add a minimal extension that can be enabled and disabled without adding a panel item.
- [x] Add `.editorconfig`, `.gitignore`, formatting configuration, and license.
- [x] Add `Makefile` or equivalent task entry points:
  - [x] `make format`
  - [x] `make lint`
  - [x] `make test`
  - [x] `make check`
  - [x] `make build`
- [x] Add `scripts/bootstrap-dev.sh` that checks dependencies without making unsafe system changes.
- [x] Add `docs/development.md` with Fedora-focused setup and generic fallback guidance.
- [x] Add a root README with project purpose, status, architecture summary, and quick-start placeholders.

### Minimum testing requirements

**Unit tests**

- [x] At least one trivial Rust test per library crate to prove test discovery.

**Smoke tests**

- [x] `cargo build --workspace` succeeds.
- [x] Daemon `--version` command succeeds.
- [x] Extension package contains valid required metadata files.
- [x] Extension can be parsed by GJS without syntax errors.

**Static checks**

- [x] `cargo fmt --check` passes.
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes.

### Deliverables

- Buildable repository skeleton.
- Environment report in `docs/development.md`.
- One-command `make check` baseline.

### Exit criteria

- A clean checkout can run the documented bootstrap and check commands.
- No camera detection, D-Bus service, or visible indicator is implemented yet.

---

## Step 2 — Pure Domain Model and Session State Machine

### Goal

Implement the infrastructure-independent core model that owns camera sessions and applies monitor events deterministically.

### Systems to develop

- `camera-core` models.
- Typed identifiers.
- Monitor events.
- State reducer/session registry.
- Port traits.
- Domain error types.

### Checklist

- [x] Define `CameraDevice`, `ApplicationIdentity`, `CameraSession`, and typed IDs.
- [x] Define `DetectionBackend` with `PipeWire` and future-safe variants.
- [x] Define `MonitorEvent`.
- [x] Implement `MonitorState` with active-session lookup and ordered snapshots.
- [x] Implement event application as a pure operation.
- [x] Handle duplicate starts idempotently.
- [x] Handle unknown stops safely.
- [x] Support metadata enrichment through session updates.
- [x] Define a `CameraEventSource` port used by future adapters.
- [x] Define a `SessionObserver` or event-sink port if needed by the application layer.
- [x] Keep the crate free of async runtime and infrastructure dependencies unless a small abstraction requires otherwise.
- [x] Document invariants in Rust documentation comments.

### Minimum testing requirements

**Unit tests**

- [x] Start event adds one session.
- [x] Duplicate start remains one session.
- [x] Stop removes the matching session.
- [x] Unknown stop is harmless.
- [x] Update enriches metadata without changing session identity.
- [x] Multiple simultaneous sessions are supported.
- [x] Snapshot ordering is deterministic.
- [x] `Active` becomes false only after the final session stops.

**Property or table-driven tests**

- [x] Reapplying an identical event sequence produces the same state.
- [x] Applying duplicate starts does not change session count.

**Static checks**

- [x] No PipeWire, zbus, GNOME, or systemd dependency appears in `camera-core`.

### Deliverables

- Fully tested pure domain crate.
- Domain model documented in `docs/architecture.md`.

### Exit criteria

- All state behavior can be tested without a desktop session, D-Bus, or camera.

---

## Step 3 — PipeWire Registry Observation

### Goal

Connect to the current user PipeWire instance and observe relevant registry objects without yet declaring active camera sessions.

### Systems to develop

- PipeWire connection lifecycle.
- Registry listeners.
- Internal raw graph objects.
- Node, port, and link discovery.
- Structured diagnostic logging.

### Checklist

- [x] Implement PipeWire initialization and connection.
- [x] Observe global object addition and removal.
- [x] Capture properties required to classify nodes, ports, devices, and links.
- [x] Store raw PipeWire IDs only inside the adapter crate.
- [x] Create internal adapter models for nodes, ports, and links.
- [x] Detect candidate camera-source nodes.
- [x] Detect candidate application video-input stream nodes.
- [x] Handle incomplete metadata and late-arriving properties.
- [x] Handle PipeWire disconnect without panicking.
- [x] Expose adapter events through a testable internal abstraction.
- [x] Add debug logging that can print a concise relevant graph summary.
- [x] Add a CLI diagnostic mode such as `camera-monitor inspect-pipewire`.

### Minimum testing requirements

**Unit tests**

- [x] Node classification from property maps.
- [x] Port classification from property maps.
- [x] Link parsing from synthetic properties.
- [x] Missing or malformed property handling.
- [x] Object removal cleans adapter graph state.

**Fixture tests**

- [x] Parse sanitized PipeWire fixture data for at least:
  - [x] one physical camera;
  - [x] one camera application stream;
  - [x] unrelated audio nodes;
  - [x] unrelated video-output nodes.

**Smoke tests**

- [x] Diagnostic command connects to PipeWire in the current desktop session.
- [x] Command exits cleanly when no camera is present.
- [x] Command reports a useful error when PipeWire is unavailable.

### Deliverables

- PipeWire graph observer.
- Diagnostic command and fixture set.
- Troubleshooting notes for missing PipeWire metadata.

### Exit criteria

- The project can reliably observe and classify candidate objects but does not yet emit domain camera sessions.

---

## Step 4 — PipeWire Graph Correlation and Camera Session Events

### Goal

Convert the observed PipeWire graph into stable start, update, and stop events for active camera sessions.

### Systems to develop

- Graph correlation engine.
- Camera-device to application-stream relationship mapping.
- Session identity strategy.
- Event deduplication.
- Link lifecycle handling.

### Checklist

- [x] Define the minimum graph conditions that mean a camera is actively consumed.
- [x] Correlate camera source nodes, ports, links, and application input streams.
- [x] Create stable domain session IDs from adapter-owned relationships.
- [x] Emit `SessionStarted` only once per active relationship.
- [x] Emit `SessionUpdated` when useful metadata improves.
- [x] Emit `SessionStopped` when the active relationship disappears.
- [x] Handle object removal in any order.
- [x] Support one application using multiple cameras.
- [x] Support multiple applications using one camera.
- [x] Ignore unrelated video playback and virtual nodes unless they satisfy camera-source criteria.
- [x] Avoid false stop/start churn caused by harmless metadata updates.
- [x] Document assumptions and known PipeWire limitations.

### Minimum testing requirements

**Unit tests**

- [x] Link creation emits one session start.
- [x] Duplicate link information emits no duplicate start.
- [x] Link removal emits one stop.
- [x] Node removal before link removal still emits one stop.
- [x] Late application metadata emits an update.
- [x] Audio graph changes emit no camera events.
- [x] Video-output/playback streams emit no camera events.

**Integration tests with synthetic graph adapter**

- [x] Single camera and single application lifecycle.
- [x] Two applications sharing one camera.
- [x] One application switching cameras.
- [x] PipeWire backend restart and graph rebuild.

**Manual smoke test**

- [x] Start a known camera application and observe a start event.
- [x] Stop camera capture and observe a stop event.
- [x] Confirm no event when the application is open but not capturing.

### Deliverables

- PipeWire backend producing domain events.
- Manual test record in `tests/manual/camera-session.md`.

### Exit criteria

- Camera session lifecycle is correct at the Rust event boundary before application-name enrichment.

---

## Step 5 — Application Identity Resolution

### Goal

Resolve useful application identity from PipeWire metadata and process information while degrading safely when information is unavailable.

### Systems to develop

- PipeWire metadata extraction.
- `/proc` process inspection.
- Desktop-entry resolution.
- Flatpak-aware application ID resolution.
- Caching and invalidation.

### Checklist

- [x] Prefer trusted PipeWire application metadata when present.
- [x] Resolve PID from PipeWire metadata where available.
- [x] Read permitted `/proc/<pid>` fields safely.
- [x] Resolve executable and process name.
- [x] Inspect cgroup/process metadata for sandboxed application identity when practical.
- [x] Resolve `.desktop` files from standard user and system locations.
- [x] Return application ID and display name without returning arbitrary icon file paths.
- [x] Define deterministic fallback display names.
- [x] Add a bounded cache keyed by relevant identity inputs.
- [x] Avoid treating process command-line content as trusted UI markup.
- [x] Handle exited processes and permission errors without failing the session.
- [x] Keep resolution off latency-sensitive PipeWire callback paths.

### Minimum testing requirements

**Unit tests**

- [x] Metadata-only identity resolution.
- [x] PID and executable fallback.
- [x] Desktop-entry match.
- [x] Flatpak-style ID resolution from fixtures.
- [x] Missing process handling.
- [x] Permission-denied handling.
- [x] Safe fallback naming.
- [x] Cache hit and invalidation behavior.

**Integration tests**

- [x] Resolve a spawned test process.
- [x] Resolve desktop entries from a temporary fixture directory.

**Manual smoke test**

- [x] Camera session reports a recognizable application name for at least one native application.
- [ ] Test one sandboxed/Flatpak application when available.

### Deliverables

- `camera-app-resolver` crate.
- Resolver behavior documented in architecture and troubleshooting docs.

### Exit criteria

- Sessions contain human-readable application information when available and safe fallback values otherwise.

---

## Step 6 — D-Bus Interface and Service Implementation

### Goal

Expose daemon state through a stable, documented user-session D-Bus API independent of the GNOME extension.

### Systems to develop

- Introspection XML.
- Rust D-Bus DTOs.
- D-Bus object implementation.
- Properties, methods, and signals.
- Contract tests.

### Checklist

- [x] Finalize D-Bus names and signatures.
- [x] Write canonical introspection XML.
- [x] Implement DTO conversion from domain entities.
- [x] Implement `Active`, `ActiveSessionCount`, `BackendAvailable`, and `Version` properties.
- [x] Implement `GetActiveSessions` and `Ping` methods.
- [x] Emit state and session signals.
- [x] Ensure property-change notifications are emitted correctly.
- [x] Sort returned session arrays deterministically.
- [x] Do not expose raw PipeWire IDs as stable public identifiers.
- [x] Return typed D-Bus errors with useful names and messages.
- [x] Add `docs/dbus-api.md` with examples using standard D-Bus CLI tools.

### Minimum testing requirements

**Unit tests**

- [x] Domain-to-D-Bus DTO conversion.
- [x] Empty, single-session, and multiple-session snapshots.
- [x] Stable field ordering and values where applicable.

**D-Bus integration tests**

Run in an isolated session bus when possible.

- [x] Service acquires the expected bus name.
- [x] `Ping` returns the expected response.
- [x] Initial properties are correct.
- [x] Applying a start event updates properties and emits signals.
- [x] Applying a stop event updates properties and emits signals.
- [x] `GetActiveSessions` matches current domain state.
- [x] Service exits cleanly and releases its bus name.

**Smoke tests**

- [x] Introspection works with a standard D-Bus inspection command.
- [x] A shell command can read `Active` and call `GetActiveSessions`.

### Deliverables

- Stable D-Bus contract.
- Automated contract tests.
- API documentation.

### Exit criteria

- A non-GNOME client can reliably observe camera state through D-Bus.

---

## Step 7 — Daemon Orchestration, Lifecycle, and Recovery

### Goal

Assemble the PipeWire adapter, resolver, domain state, and D-Bus service into a reliable long-running user daemon.

### Systems to develop

- Application orchestration.
- Async channels and task ownership.
- Graceful shutdown.
- Backend availability state.
- Retry and reconnection logic.
- Runtime configuration.

### Checklist

- [ ] Define explicit task ownership and channel flow.
- [ ] Keep PipeWire callback work minimal.
- [ ] Route events to the application/state layer.
- [ ] Resolve application identity asynchronously.
- [ ] Publish state changes through D-Bus.
- [ ] Handle SIGINT and SIGTERM gracefully.
- [ ] Handle PipeWire startup delay.
- [ ] Retry recoverable PipeWire connection failures with bounded backoff.
- [ ] Clear or reconcile stale sessions after backend loss.
- [ ] Set `BackendAvailable` correctly.
- [ ] Prevent unbounded queues and task leaks.
- [ ] Add structured logging with configurable verbosity.
- [ ] Add `--log-level`, `--version`, and diagnostic options.
- [ ] Ensure normal operation requires no root privileges.

### Minimum testing requirements

**Unit tests**

- [ ] Backoff policy.
- [ ] Configuration parsing.
- [ ] Event-to-state-to-publication flow using fakes.

**Integration tests**

- [ ] Fake backend start/update/stop reaches D-Bus.
- [ ] Backend failure updates availability.
- [ ] Backend recovery restores observation.
- [ ] Shutdown completes within a bounded duration.
- [ ] Slow resolver does not block event ingestion.

**Process smoke tests**

- [ ] Daemon remains running in a normal user session.
- [ ] SIGTERM produces a clean exit status.
- [ ] Restarting PipeWire or using a simulated backend loss does not crash the daemon.

### Deliverables

- Functional daemon binary.
- Runtime and logging documentation.

### Exit criteria

- The daemon can run unattended and expose live camera state through D-Bus.

---

## Step 8 — GNOME Extension Foundation with Mockable Client

### Goal

Implement the GNOME Shell UI structure using a mockable data client before connecting to the real daemon.

### Systems to develop

- Extension lifecycle.
- Panel system indicator.
- Menu layout.
- Session view model.
- Mock data provider.
- GNOME-safe cleanup.

### Checklist

- [ ] Implement `enable()` and `disable()` with complete resource cleanup.
- [ ] Add a symbolic camera icon to the appropriate GNOME panel/privacy area.
- [ ] Hide the icon when inactive.
- [ ] Show the icon when mock state is active.
- [ ] Implement a menu listing active application and camera names.
- [ ] Support multiple active sessions.
- [ ] Show a backend-unavailable status without claiming camera use.
- [ ] Separate pure session-model code from GNOME-specific widgets.
- [ ] Ensure repeated enable/disable cycles do not duplicate UI elements or signal handlers.
- [ ] Avoid blocking operations in GNOME Shell.
- [ ] Use accessible labels and tooltips.
- [ ] Add a development-only mock mode controlled through a documented mechanism.

### Minimum testing requirements

**JavaScript unit tests**

- [ ] Session list transformation.
- [ ] Duplicate-session handling.
- [ ] Active/inactive derived state.
- [ ] Stable display ordering.
- [ ] Backend-unavailable view state.

**Static checks**

- [ ] JavaScript lint passes.
- [ ] GJS module syntax check passes.
- [ ] Metadata and schema validation pass.

**GNOME smoke tests**

- [ ] Extension enables successfully.
- [ ] Mock inactive state hides the icon.
- [ ] Mock active state shows the icon.
- [ ] Menu renders one and multiple sessions.
- [ ] Extension disables without GNOME Shell errors.

### Deliverables

- Working mock-driven extension UI.
- Manual UI test instructions.

### Exit criteria

- GNOME UI behavior is reviewable without requiring the Rust daemon or a physical camera.

---

## Step 9 — Live D-Bus Integration Between Extension and Daemon

### Goal

Replace mock state with a resilient D-Bus client and provide complete live indicator behavior.

### Systems to develop

- `Gio.DBusProxy` client.
- Initial state synchronization.
- D-Bus signal handling.
- Daemon appearance/disappearance handling.
- Reconnection behavior.

### Checklist

- [ ] Create a dedicated `dbusClient.js` abstraction.
- [ ] Read the initial property snapshot after proxy creation.
- [ ] Fetch active sessions on startup.
- [ ] Subscribe to property changes and session/state signals.
- [ ] Coalesce redundant refreshes.
- [ ] Handle daemon not installed, not running, starting, stopping, and restarting.
- [ ] Avoid stale active indicators after daemon loss.
- [ ] Do not issue synchronous D-Bus calls from GNOME Shell UI paths.
- [ ] Validate and normalize D-Bus data before rendering.
- [ ] Return all signal subscriptions and cancellables during `disable()`.
- [ ] Disable mock mode in production builds.
- [ ] Add a useful menu message when the service is unavailable.

### Minimum testing requirements

**JavaScript unit tests**

- [ ] D-Bus payload normalization using fixtures.
- [ ] Initial sync behavior.
- [ ] Repeated state events do not duplicate sessions.
- [ ] Service disappearance clears stale state.

**Integration tests**

- [ ] Extension client works against a fake D-Bus service.
- [ ] Start event shows indicator.
- [ ] Stop event hides indicator when no sessions remain.
- [ ] Multiple sessions remain visible until the final stop.
- [ ] Daemon restart triggers resynchronization.

**End-to-end smoke tests**

- [ ] Start real daemon and extension.
- [ ] Open camera capture in an application.
- [ ] Indicator appears within an acceptable short delay.
- [ ] Menu shows the responsible application.
- [ ] Stop capture and verify the indicator disappears.

### Deliverables

- First complete vertical MVP slice.
- End-to-end manual test record.

### Exit criteria

- Real camera access is reflected correctly in GNOME Shell from PipeWire through D-Bus.

---

## Step 10 — Preferences, Notifications, and UX Refinement

### Goal

Add user-facing preferences and polished privacy-indicator behavior without expanding the detection scope.

### Systems to develop

- GSettings schema.
- Preferences window.
- Start/stop notifications.
- Indicator and menu presentation rules.
- Accessibility text.

### Checklist

- [ ] Add a preference to enable or disable notifications.
- [ ] Add a preference to show or hide application names in notifications.
- [ ] Add a preference controlling backend-unavailable warnings.
- [ ] Add a preference controlling whether the indicator remains visible during backend failure.
- [ ] Implement start notifications with deduplication.
- [ ] Avoid noisy stop notifications unless explicitly enabled.
- [ ] Group rapid session changes where appropriate.
- [ ] Use symbolic icons and GNOME-consistent spacing.
- [ ] Escape or safely render all external application strings.
- [ ] Add clear accessible names.
- [ ] Do not use misleading “safe” language or colors.
- [ ] Keep settings owned by the extension unless the daemon genuinely needs them.

### Minimum testing requirements

**Unit tests**

- [ ] Notification deduplication.
- [ ] Preference-to-view-model behavior.
- [ ] Safe rendering of unusual application names.

**Schema tests**

- [ ] GSettings schema compiles.
- [ ] Defaults are documented and sensible.

**GNOME integration tests**

- [ ] Preference changes apply without restarting GNOME Shell when feasible.
- [ ] Notifications respect settings.
- [ ] Re-enabling the extension preserves settings.

**Manual UX review**

- [ ] One session.
- [ ] Multiple sessions.
- [ ] Long application names.
- [ ] Missing application metadata.
- [ ] Backend unavailable.

### Deliverables

- Preferences UI.
- Notification behavior.
- Updated screenshots and usage documentation.

### Exit criteria

- The MVP is understandable and usable without command-line interaction after installation.

---

## Step 11 — Robustness, Multi-Camera Cases, and Fault Injection

### Goal

Harden the system against real-world race conditions, restarts, malformed metadata, multiple devices, and partial failures.

### Systems to develop

- Fault-injection test helpers.
- Session reconciliation.
- Resource leak checks.
- Multi-camera validation.
- Error-state UX.

### Checklist

- [ ] Reconcile a complete backend snapshot after reconnect.
- [ ] Prevent stale sessions after missed remove events.
- [ ] Handle application exit before PipeWire cleanup.
- [ ] Handle camera unplug during active capture.
- [ ] Handle camera replug with changed PipeWire IDs.
- [ ] Handle two cameras with identical display names.
- [ ] Handle two processes belonging to the same desktop application.
- [ ] Define whether sessions are grouped in UI by process, application, or device.
- [ ] Cap logs and in-memory caches.
- [ ] Check for leaked D-Bus subscriptions and GNOME objects.
- [ ] Add timeout boundaries around external metadata resolution.
- [ ] Confirm malformed metadata cannot crash either component.
- [ ] Document unsupported or ambiguous graph cases.

### Minimum testing requirements

**Fault-injection integration tests**

- [ ] Out-of-order events.
- [ ] Duplicate events.
- [ ] Missed event followed by snapshot reconciliation.
- [ ] Backend disconnect during active session.
- [ ] Resolver timeout.
- [ ] D-Bus client disconnect and reconnect.

**Multi-device tests**

- [ ] Two cameras, one application.
- [ ] One camera, two applications.
- [ ] Two cameras, two applications.

**Longevity smoke test**

- [ ] Run the daemon for an extended test period while repeatedly starting and stopping camera applications.
- [ ] Verify memory and task counts remain stable enough for a user-session daemon.

### Deliverables

- Fault-injection suite.
- Updated known-limitations section.
- Robustness test report.

### Exit criteria

- Common desktop lifecycle disruptions do not leave incorrect privacy state or require manual recovery.

---

## Step 12 — systemd User Service and D-Bus Activation

### Goal

Integrate the daemon into the user session so it starts on demand and behaves correctly across login, logout, restart, and extension lifecycle events.

### Systems to develop

- systemd user unit.
- D-Bus activation service file.
- Install/uninstall scripts.
- Runtime paths.
- Logging instructions.

### Checklist

- [ ] Create a systemd user service with correct dependencies and restart policy.
- [ ] Use `Type=dbus` when appropriate for the final service design.
- [ ] Add D-Bus activation.
- [ ] Ensure paths work for local development installation.
- [ ] Ensure paths can be replaced cleanly by system packaging.
- [ ] Add safe idempotent local install and uninstall scripts.
- [ ] Run schema compilation and extension packaging during installation.
- [ ] Do not require root for local per-user installation.
- [ ] Document logs through the user journal.
- [ ] Ensure uninstall removes only project-owned files.
- [ ] Define behavior when the extension is absent but another D-Bus client activates the daemon.

### Minimum testing requirements

**Installation tests**

- [ ] Fresh local install succeeds.
- [ ] Repeated install succeeds without duplication.
- [ ] Uninstall succeeds.
- [ ] Repeated uninstall is harmless.

**Activation tests**

- [ ] D-Bus call starts the daemon automatically.
- [ ] Bus name is acquired.
- [ ] Service restart policy works for unexpected failure.
- [ ] Normal shutdown is not treated as a crash loop.

**Session smoke tests**

- [ ] Log out and back in, then confirm normal operation.
- [ ] Disable and re-enable the extension.
- [ ] Restart the user service during active desktop use.

### Deliverables

- Production-shaped user-session integration.
- Installation and troubleshooting documentation.

### Exit criteria

- A user can install, use, inspect, restart, and uninstall the application predictably.

---

## Step 13 — Packaging and Release Candidate

### Goal

Produce reproducible artifacts for the GNOME extension and Fedora-oriented daemon packaging.

### Systems to develop

- Extension ZIP packaging.
- RPM spec or equivalent Fedora package source.
- Version propagation.
- Release checks.
- License and attribution audit.

### Checklist

- [ ] Define one project version source or a checked propagation process.
- [ ] Include version in daemon, D-Bus property, extension metadata, and release artifacts.
- [ ] Produce a GNOME extension ZIP with only required files.
- [ ] Produce Fedora package sources for daemon, D-Bus, and systemd files.
- [ ] Decide whether the extension is packaged separately from the daemon.
- [ ] Add package install, upgrade, and uninstall script behavior where required.
- [ ] Verify file ownership and install paths.
- [ ] Audit dependency licenses.
- [ ] Add changelog and release notes template.
- [ ] Add checksums for release artifacts.
- [ ] Ensure development mocks and fixtures are excluded from production artifacts.
- [ ] Document supported GNOME Shell versions based on actual testing.

### Minimum testing requirements

**Artifact tests**

- [ ] Build artifacts from a clean checkout.
- [ ] Inspect package contents.
- [ ] Install extension ZIP locally.
- [ ] Build RPM in a clean or isolated build environment when available.

**Upgrade tests**

- [ ] Upgrade from the previous local version without losing settings.
- [ ] Restart daemon and extension successfully after upgrade.

**Release smoke test**

- [ ] Clean installation on a representative Fedora/GNOME environment.
- [ ] End-to-end camera start/stop behavior.
- [ ] Clean uninstall.

### Deliverables

- Release candidate artifacts.
- `docs/release.md`.
- Release checklist.

### Exit criteria

- Another user can install and test the release candidate from documented artifacts.

---

## Step 14 — Final Hardening, Documentation, and v1.0 Release

### Goal

Close critical defects, complete documentation, verify privacy behavior, and prepare the first stable release.

### Systems to develop

- Final end-to-end suite.
- Security and privacy review.
- User documentation.
- Contributor documentation.
- Release automation where justified.

### Checklist

- [ ] Review all open known limitations and classify release blockers.
- [ ] Review all uses of process metadata and external strings.
- [ ] Confirm the application sends no telemetry or network traffic.
- [ ] Confirm no root privileges are requested.
- [ ] Confirm no camera frames are captured or processed by this application.
- [ ] Confirm logs do not unnecessarily expose sensitive command-line data.
- [ ] Confirm history is not stored in MVP.
- [ ] Complete README installation, usage, architecture, and troubleshooting sections.
- [ ] Add contributor workflow and code-review expectations.
- [ ] Add issue templates for false positives, false negatives, and compatibility reports.
- [ ] Record tested GNOME, PipeWire, Rust, and Fedora versions.
- [ ] Produce final release notes.
- [ ] Tag v1.0 only after all release gates pass.

### Minimum testing requirements

**Full automated suite**

- [ ] Rust unit and integration tests pass.
- [ ] D-Bus contract tests pass.
- [ ] JavaScript unit and static checks pass.
- [ ] Packaging checks pass.

**Final manual matrix**

- [ ] Native camera application.
- [ ] Browser camera access.
- [ ] Flatpak application when available.
- [ ] Capture denied by the user.
- [ ] Application open without active capture.
- [ ] Camera unplug/replug.
- [ ] Multiple concurrent camera users.
- [ ] Daemon restart.
- [ ] PipeWire restart or simulated backend recovery.
- [ ] GNOME extension disable/re-enable.
- [ ] Login/logout cycle.

**Security/privacy review**

- [ ] No privilege escalation path introduced.
- [ ] No shell-command construction from untrusted metadata.
- [ ] D-Bus API exposes only required information.
- [ ] External strings are treated as untrusted.
- [ ] Dependency audit has no unresolved critical issue.

### Deliverables

- v1.0 source tag.
- Release artifacts.
- Complete documentation.
- Final test and compatibility report.

### Exit criteria

- The application is stable enough for daily use and its known detection limitations are explicit.

---

# Optional Post-v1.0 Steps

These steps must not be mixed into MVP work.

---

## Optional Step A — Direct V4L2 Fallback Detection

### Goal

Detect applications that bypass PipeWire and directly hold `/dev/video*` devices.

### Constraints

- Start with an unprivileged, clearly labeled fallback where feasible.
- Do not claim complete coverage.
- Do not introduce eBPF or root requirements without a separate approved design.
- Deduplicate sessions detected by both PipeWire and V4L2 backends.

### Minimum testing requirements

- Unit tests for device-path and process-FD correlation.
- Integration tests with a controlled process holding a test device where feasible.
- Permission-denied behavior.
- Cross-backend deduplication tests.
- False-positive review for metadata-only devices.

---

## Optional Step B — Camera Access History

### Goal

Provide opt-in local access history with strict retention and privacy controls.

### Constraints

- Disabled by default.
- Clear retention period.
- Clear-data control.
- No command-line capture.
- No network synchronization.
- Document the privacy trade-off.

### Minimum testing requirements

- Persistence and migration tests.
- Retention cleanup tests.
- Disable-history behavior.
- Clear-history behavior.
- File-permission tests.

---

## Optional Step C — Camera Disable Control

### Goal

Offer a safe user control for disabling camera access.

### Constraints

This requires a separate technical and security design. Do not implement a misleading button that merely hides the indicator. The control must have a documented, reversible enforcement mechanism and must not destabilize active desktop sessions.

---

# Testing Strategy Summary

## Unit tests

Use unit tests for:

- Pure domain transitions.
- Property-map classification.
- Graph correlation.
- Application resolution.
- DTO conversion.
- JavaScript view-model behavior.
- Configuration and backoff rules.

## Integration tests

Use integration tests for:

- Synthetic PipeWire graph lifecycles.
- Daemon orchestration with fake backends.
- Isolated D-Bus service/client interactions.
- Application resolver against temporary fixtures.
- Extension client against a fake D-Bus service.
- systemd and activation behavior where the environment permits.

## Smoke tests

Use smoke tests for:

- Real PipeWire connection.
- Real D-Bus introspection.
- GNOME extension enable/disable.
- End-to-end camera start/stop.
- Install, restart, login, upgrade, and uninstall flows.

## Manual tests

Every manual test document must include:

```text
Environment:
Preconditions:
Steps:
Expected result:
Actual result:
Logs collected:
Pass/fail:
Notes:
```

---

# Quality Gates

The following commands should eventually be represented by `make check` or equivalent:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

# Project-defined commands
make lint-js
make test-js
make validate-extension
make validate-dbus
make smoke
```

Rules:

- Do not merge with failing tests.
- Do not ignore flaky tests; isolate and fix the cause.
- Do not disable a quality gate to complete a step.
- Manual-only checks must be clearly labeled and reproducible.

---

# Compatibility Policy

- Detect the local GNOME Shell version during Step 1.
- Add only GNOME Shell versions that are actually tested.
- Keep version-specific GNOME code behind small compatibility adapters when necessary.
- Record PipeWire properties observed on tested systems without assuming they exist everywhere.
- Treat missing metadata as a normal case.
- Prefer graceful degradation over false certainty.

---

# Privacy and Security Requirements

- The application observes metadata only; it must never access camera frames.
- No telemetry.
- No remote service.
- No permanent history in MVP.
- No root requirement in MVP.
- No arbitrary shell execution from application metadata.
- Do not log full process command lines by default.
- D-Bus is limited to the user session.
- External application names and IDs are untrusted input.
- Failures must never create a false “camera inactive” claim without indicating backend unavailability.

--

# Step Status Tracker

Update this table only after the user accepts a completed step.

| Step | Name | Status | Accepted commit/tag | Notes |
|---:|---|---|---|---|
| 1 | Repository bootstrap | Not started | — | — |
| 2 | Domain model and state | Not started | — | — |
| 3 | PipeWire registry observation | Not started | — | — |
| 4 | Graph correlation | Not started | — | — |
| 5 | Application resolution | Not started | — | — |
| 6 | D-Bus service | Not started | — | — |
| 7 | Daemon orchestration | Not started | — | — |
| 8 | GNOME UI foundation | Not started | — | — |
| 9 | Live extension integration | Not started | — | — |
| 10 | Preferences and UX | Not started | — | — |
| 11 | Robustness hardening | Not started | — | — |
| 12 | systemd and activation | Not started | — | — |
| 13 | Packaging and release candidate | Not started | — | — |
| 14 | v1.0 hardening and release | Not started | — | — |

---

# Final Product Acceptance Criteria

The v1.0 project is acceptable only when:

- Camera capture through supported PipeWire paths causes the indicator to appear.
- The responsible application is identified when metadata permits.
- Multiple simultaneous sessions are represented correctly.
- Ending the final session hides the active indicator.
- Backend failure is distinguishable from “camera inactive.”
- Restarting the daemon, extension, or PipeWire does not leave stale state.
- The system runs as the current user without elevated privileges.
- Installation and removal are documented and reproducible.
- Automated and manual release checks pass.
- Known detection limitations are stated plainly.
