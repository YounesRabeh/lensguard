# Future plan: V4L2-only camera-access detection

## Product decision

LensGuard will become a **direct-V4L2-only** camera monitor. Direct V4L2 capture is the sole
supported source of camera-use state. The current PipeWire-based detector is deprecated and will
be removed; it must not remain as a fallback, secondary signal, or requirement for normal camera
detection.

This replacement is intentionally a product migration, not an additional backend. Until the V4L2
implementation meets the release criteria in this document, LensGuard must clearly report that
monitoring is unavailable rather than combining incomplete V4L2 results with the retired backend.

## PipeWire retirement scope

The V4L2 migration is complete only when the repository and shipped packages no longer contain
the PipeWire monitoring implementation. Remove:

- the `camera-pipewire` crate and its workspace membership;
- PipeWire, WirePlumber, SPA, and bindgen dependencies used solely for graph monitoring;
- graph classifiers, relationship tracking, reconnect logic, and backend-specific D-Bus fields;
- PipeWire diagnostic commands, fixtures, tests, manual test instructions, and screenshots that
  describe graph detection;
- PipeWire runtime and development dependencies from local installers, DEB/RPM/Arch packages,
  CI images, and release validation; and
- user-facing claims that LensGuard detects PipeWire sessions.

Keep the unprivileged user daemon, application resolver, user-session D-Bus interface, GNOME
extension, packaging discipline, and privacy guarantees, but adapt them to consume only validated
direct-V4L2 observer events.

## Product model

LensGuard will use one camera-detection path:

### Direct V4L2 coverage: narrowly privileged eBPF observer

- Detects applications that access `/dev/videoN` directly through V4L2.
- Uses a narrowly scoped eBPF program and the minimum privileges required by supported kernels and distributions.
- Reports only metadata required to determine active camera capture.
- Emits process identity, device identity, capture operation, timestamp, and operation result.
- Never reads image frames, capture buffers, process memory, complete command lines, or unrelated system activity.
- Runs as a small, separately reviewable system component.
- Communicates with the unprivileged LensGuard user daemon through a restricted local IPC boundary.
- Must not be bundled inside the GNOME Extensions ZIP.
- Must report clearly when the observer is unavailable because of missing privileges, unsupported kernels, BPF restrictions, service failure, or version mismatch.

## Motivation

Applications may access Linux camera devices directly through V4L2 by opening `/dev/videoN` and issuing V4L2 operations.

LensGuard must distinguish between:

- a process merely opening or probing a camera device; and
- a process that successfully starts capture.

Scanning open file descriptors is insufficient because an open camera node does not prove that image capture is active.

## Goal

Detect successful direct V4L2 capture activity, associate it with a process and physical camera where safely possible, and expose one accurate camera session to the GNOME Shell extension.

The implementation must remain metadata-only, local, reviewable, and narrowly privileged.

## Non-goals

- Using any graph-based or media-session-manager detection backend.
- Combining V4L2 events with a second camera-detection source.
- Making the GNOME Shell extension privileged.
- Reporting a camera as active solely because `/dev/videoN` is open.
- Reading, copying, hashing, decoding, mapping, or storing camera frames.
- Reading complete command lines, process memory, or unrelated syscall activity.
- Adding telemetry, cloud synchronization, or persistent camera-access history.
- Installing privileged components from the GNOME Extensions website.
- Supporting arbitrary tracing, plugins, shell execution, or user-provided BPF programs.

## Architecture

```text
┌──────────────────────────────────────────────┐
│ GNOME Shell extension                       │
│                                              │
│ - Quick Settings indicator                  │
│ - active application and camera UI          │
│ - observer availability diagnostics         │
└──────────────────────┬───────────────────────┘
                       │ user-session D-Bus
┌──────────────────────▼───────────────────────┐
│ LensGuard user daemon                       │
│                                              │
│ - application resolver                      │
│ - physical-device resolver                  │
│ - V4L2 session state machine                │
│ - observer health monitoring                │
│ - D-Bus API                                 │
└──────────────────────┬───────────────────────┘
                       │ restricted local IPC
┌──────────────────────▼───────────────────────┐
│ Privileged eBPF V4L2 observer               │
│                                              │
│ - narrowly scoped kernel hooks              │
│ - successful capture start/stop detection   │
│ - PID/TGID and device metadata only         │
│ - no frames, buffers, history, or networking│
└──────────────────────────────────────────────┘
```

The user daemon is the only component that communicates with the GNOME Shell extension.

The eBPF observer must not expose a second UI-facing API.

## Security boundary

The eBPF observer is a privileged, security-sensitive component.

Requirements:

- Keep its source tree, binary, BPF objects, service definition, IPC protocol, and packaging separate from the unprivileged daemon.
- Use the minimum capabilities required to load and attach the selected BPF programs.
- Do not assume full root access is necessary without measuring and documenting the deployment model.
- Drop unnecessary privileges after initialization where technically possible.
- Restrict observed operations to V4L2 capture-relevant file descriptors, devices, syscalls, and ioctl requests.
- Restrict IPC access to the LensGuard user daemon through documented socket ownership or policy.
- Validate every event crossing the privileged/unprivileged boundary.
- Do not accept arbitrary paths, filters, commands, tracing targets, or BPF programs from the user daemon.
- Do not provide networking, plugin loading, scripting, shell execution, or persistence.
- Fail closed: malformed, unverifiable, or unauthorized events must not create a camera session.
- Produce audit-friendly logs without exposing sensitive process data.
- Detach all BPF programs cleanly during shutdown, upgrade, failure, or uninstall.

## Detection semantics

### Confirmed capture

A V4L2 session may be considered active only after observing a successful capture operation.

Primary start signals:

- successful `VIDIOC_STREAMON` for streaming I/O;
- successful capture reads for clients using V4L2 read I/O, when the read can be identified as capture without examining the returned frame contents.

Primary stop signals:

- successful `VIDIOC_STREAMOFF`;
- final close of the confirmed capture file descriptor;
- process exit;
- device removal;
- observer shutdown or backend loss.

Observer shutdown or backend loss must terminate the session as interrupted or unknown, not as a normal successful stop.

### Open-only state

Observing that a process has `/dev/videoN` open does not prove active capture.

Open-only information may be used internally for:

- mapping file descriptors to devices;
- preparing process identity;
- diagnostics;
- correlation between subsequent operations.

Open-only information must not:

- activate the normal panel indicator;
- create a confirmed camera session;
- generate a “camera in use” notification.

### Confidence model

Use explicit confidence internally:

```rust
pub enum CaptureConfidence {
    Confirmed,
    OpenOnly,
    Unknown,
}
```

Only `Confirmed` sessions may drive the normal user-facing camera-in-use state.

## Observer availability model

V4L2 observer availability must be explicit.

Example diagnostic state:

```text
Camera monitoring: unavailable
V4L2 observer: missing required capability
```

Possible states:

```rust
pub enum ObserverAvailability {
    Available,
    Disabled,
    NotInstalled,
    UnsupportedKernel,
    MissingCapability,
    BlockedByPolicy,
    VersionMismatch,
    ConnectionFailed,
    BackendLost,
}
```

Rules:

- Observer unavailability must never be shown as a successful monitoring state.
- Missing privileges, disabled BPF, unsupported kernels, SELinux denial, Secure Boot restrictions, absent packaging, service failure, or rejected IPC must be reported accurately.
- The normal UI should remain focused on active applications and cameras.
- Detailed observer status belongs in preferences, diagnostics, logs, or an expandable status area.
- The GNOME extension must not attempt privilege escalation or service installation.

## Event contract

Define the observer event contract in `camera-core` before implementing eBPF details.

Example event:

```rust
pub struct DirectCaptureEvent {
    pub schema_version: u16,
    pub timestamp_monotonic_ns: u64,
    pub process_id: u32,
    pub thread_group_id: u32,
    pub file_descriptor: i32,
    pub device: DeviceIdentity,
    pub operation: DirectCaptureOperation,
    pub result: i64,
    pub confidence: CaptureConfidence,
}
```

```rust
pub enum DirectCaptureOperation {
    DeviceOpened,
    StreamStarted,
    StreamStopped,
    CaptureRead,
    DeviceClosed,
    ProcessExited,
    DeviceRemoved,
    BackendLost,
}
```

Contract requirements:

- Version the IPC message schema.
- Reject unknown versions and invalid enum values safely.
- Enforce strict message-size and field-length limits.
- Use monotonic timestamps for event ordering.
- Include syscall or ioctl result information so failed operations cannot become confirmed sessions.
- Keep display names, desktop-file metadata, icons, and GNOME-specific concepts out of the privileged observer.
- Perform application enrichment only in the unprivileged user daemon.
- Treat the observer as untrusted input despite being locally installed.

## Physical-device identity

Do not identify cameras solely by `/dev/videoN`, because device numbers may change and one physical camera may expose multiple nodes.

Build the strongest available identity from:

- character-device major/minor;
- udev syspath;
- media-controller parent;
- USB parent path;
- serial number when available;
- vendor and product identifiers;
- bus information;
- driver name;
- libcamera camera identifier where available.

Example:

```rust
pub struct PhysicalCameraId {
    pub major: u32,
    pub minor: u32,
    pub udev_syspath: Option<PathBuf>,
    pub media_device: Option<PathBuf>,
    pub serial: Option<String>,
    pub hardware_path: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
}
```

Identity resolution requirements:

- tolerate incomplete metadata;
- preserve the kernel major/minor identity;
- distinguish multiple V4L2 nodes from the same physical device;
- expose uncertainty rather than forcing an incorrect physical-device merge;
- avoid opening the camera device from the user daemon.

## Session model

LensGuard should maintain one logical session per confirmed capture context.

A session may be keyed by:

- process or thread group;
- confirmed capture file descriptor;
- physical camera identity;
- capture lifetime.

Example:

```rust
pub struct CameraSession {
    pub session_id: SessionId,
    pub process_id: u32,
    pub thread_group_id: u32,
    pub application: ResolvedApplication,
    pub camera: PhysicalCameraId,
    pub started_at_monotonic_ns: u64,
    pub last_observed_at_monotonic_ns: u64,
    pub state: CameraSessionState,
    pub confidence: CaptureConfidence,
}
```

```rust
pub enum CameraSessionState {
    Active,
    Stopped,
    Interrupted,
    Unknown,
}
```

Rules:

- Multiple active file descriptors from one process may represent separate sessions unless proven to be one capture.
- Multiple processes using the same camera must remain distinguishable.
- One application using multiple cameras must produce distinguishable sessions.
- A failed `VIDIOC_STREAMON` must never produce an active session.
- Repeated successful start events must be idempotent.
- Out-of-order stop events must not create phantom sessions.
- Process exit, final close, device removal, or observer loss must end active sessions safely.

## Process and application identity

The privileged observer should emit only PID/TGID and device metadata.

The unprivileged daemon may resolve:

- executable path when permitted;
- process name;
- desktop application ID;
- Flatpak application ID;
- application display name;
- icon name.

Requirements:

- use the existing resolver where possible;
- handle processes that exit before enrichment completes;
- provide a safe “Unknown application” fallback;
- do not store complete command lines;
- do not persist application history;
- do not expose unrelated process environment data;
- do not trust process identity after PID reuse without validating timestamps or process start time.

## Investigation phase

Do not begin production implementation until the investigation report is approved.

### Mechanisms to evaluate

- V4L2 ioctl tracing for successful `VIDIOC_STREAMON` and `VIDIOC_STREAMOFF`.
- Capture through V4L2 read I/O.
- Mapping a traced file descriptor to a V4L2 character-device identity.
- Tracking file close, process exit, and device removal.
- eBPF tracepoints, fentry/fexit, kprobes/kretprobes, LSM hooks, and other relevant attachment options.
- Capability-based deployment and system-service deployment.
- Kernel BTF and CO-RE portability.
- Secure Boot, kernel lockdown, SELinux, AppArmor, and BPF restrictions.
- Udev and sysfs metadata for physical-device identity.
- Audit-based tracing only as a comparison, not as the preferred production backend.

### Required investigation outputs

1. A mechanism matrix documenting whether each approach can prove:
   - device open;
   - successful stream start;
   - successful stream stop;
   - successful capture read;
   - process identity;
   - file-descriptor identity;
   - physical-device identity;
   - required privilege or capability;
   - supported kernels and distributions.
2. A threat model for:
   - the privileged observer;
   - BPF loading and attachment;
   - the IPC boundary;
   - event validation;
   - service installation and upgrade.
3. A compatibility matrix for:
   - UVC webcams;
   - integrated cameras;
   - media-controller devices;
   - libcamera-backed devices;
   - virtual cameras such as `v4l2loopback`;
   - browsers using direct V4L2;
   - native V4L2 clients;
   - Flatpak clients with device access;
   - streaming I/O;
   - read I/O.
4. CPU, memory, wake-up, event-latency, and battery measurements.
5. A written product decision approving or rejecting the eBPF observer architecture.

### Rejection criteria

Reject an approach if it:

- reads, maps, copies, hashes, or inspects frame buffers unnecessarily;
- reports a device open or capability probe as active capture;
- traces unrelated ioctls, devices, or processes broadly;
- requires unrestricted root execution for the lifetime of a large daemon;
- allows the user daemon to load arbitrary BPF programs;
- cannot distinguish successful operations from syscall attempts;
- cannot cleanly identify device and process ownership;
- produces unacceptable idle wake-ups, CPU use, or battery cost;
- cannot be packaged and reviewed independently;
- creates an undocumented or overly broad privilege boundary.

## Implementation modules

### 1. `camera-core`

Add V4L2-focused types and state machines:

- `DirectCaptureEvent`;
- `DirectCaptureOperation`;
- `CaptureConfidence`;
- `ObserverAvailability`;
- physical-camera identity;
- process identity;
- confirmed-capture state transitions;
- interrupted-session handling;
- versioned IPC schema.

The module must not depend on:

- eBPF libraries;
- D-Bus;
- GNOME;
- systemd;
- a specific IPC transport.

### 2. `camera-monitor`

The unprivileged user daemon must provide:

- observer client;
- strict event validation;
- process/application enrichment;
- physical-device enrichment;
- V4L2 session state;
- observer health reporting;
- user-session D-Bus API;
- safe behavior when the observer disconnects or fails;
- no camera-device opening;
- no privileged operations.

### 3. `lensguard-v4l2-observer`

Create a separate privileged component:

- minimal Rust userspace loader or service;
- narrowly scoped eBPF programs;
- V4L2 file-descriptor tracking;
- successful operation filtering;
- kernel event-to-device resolution;
- restricted local IPC;
- capability and kernel-feature detection;
- clean shutdown and BPF detachment;
- no UI;
- no application-name resolution;
- no persistent history;
- no networking;
- no arbitrary configuration.

### 4. GNOME Shell extension

Keep the extension unprivileged and minimal:

- show the camera indicator for confirmed sessions;
- show application and camera information from the user daemon;
- expose observer availability in diagnostics or preferences;
- display a clear warning when monitoring is unavailable;
- never perform privilege escalation;
- never install, configure, or restart the observer;
- never read `/dev/videoN`;
- never communicate directly with the privileged observer.

### 5. Packaging

Package the privileged observer separately from the GNOME extension ZIP.

Possible artifacts:

```text
lensguard-extension.zip
lensguard-camera-monitor
lensguard-v4l2-observer
lensguard-v4l2-observer.service
lensguard-v4l2-observer.socket
lensguard-v4l2-observer.policy-or-capability-config
lensguard-v4l2-observer.bpf.o
```

Packaging requirements:

- explicit privileged-component documentation;
- reproducible builds;
- signed release artifacts where supported;
- runtime version compatibility checks;
- correct service, socket, capability, and policy permissions;
- clean uninstall of services, BPF objects, capabilities, sockets, and policy;
- extension package contains no privileged binary;
- GNOME extension store listing clearly states that the extension requires an externally installed LensGuard service.

## Iteration plan

Each iteration must be completed, tested, documented, and explicitly approved before moving to the next one.

### Iteration V1 — Research and feasibility report

**Goal:** determine whether a narrowly privileged eBPF observer can reliably detect successful direct V4L2 capture.

**Checklist**

- [ ] Document V4L2 streaming-I/O and read-I/O capture paths.
- [ ] Identify viable kernel hooks and attachment types.
- [ ] Determine how to map traced file descriptors to V4L2 devices.
- [ ] Confirm how syscall and ioctl return values will be observed.
- [ ] Evaluate required capabilities on supported Fedora releases and other target distributions.
- [ ] Document kernel BTF and CO-RE requirements.
- [ ] Document Secure Boot, lockdown, SELinux, AppArmor, and BPF policy effects.
- [ ] Build an initial compatibility and security matrix.
- [ ] Record rejected mechanisms and reasons.

**Minimum tests**

- [ ] Prototype observes successful `VIDIOC_STREAMON` from a controlled native client.
- [ ] Failed `VIDIOC_STREAMON` does not create a confirmed event.
- [ ] Merely opening and querying `/dev/videoN` does not create a confirmed event.
- [ ] Process identity and device major/minor are captured.
- [ ] `VIDIOC_STREAMOFF` ends a confirmed session.
- [ ] Process exit and final close end a confirmed session.
- [ ] Prototype teardown removes all hooks cleanly.

**Exit criteria**

- A written decision confirms that the approach is technically viable, sufficiently narrow, and acceptable for further development.

### Iteration V2 — Core event contract and state machine

**Goal:** add V4L2 event and session concepts without production eBPF code.

**Checklist**

- [ ] Define versioned observer-event schema.
- [ ] Define confidence and observer-health models.
- [ ] Define physical-camera identity.
- [ ] Define process identity and PID-reuse protection.
- [ ] Add state transitions for start, stop, close, process exit, device removal, and backend loss.
- [ ] Add interrupted-session semantics.
- [ ] Define idempotency and out-of-order behavior.

**Minimum tests**

- [ ] Unit tests for every capture transition.
- [ ] Invalid-event and invalid-version rejection tests.
- [ ] Duplicate-event tests.
- [ ] Out-of-order-event tests.
- [ ] Backend-loss behavior tests.
- [ ] Open-only events never activate confirmed camera state.
- [ ] Failed operations never activate confirmed camera state.
- [ ] PID-reuse tests.

**Exit criteria**

- `camera-core` supports the complete V4L2 session model with no dependency on eBPF, D-Bus, GNOME, systemd, or a concrete IPC transport.

### Iteration V3 — Synthetic observer integration

**Goal:** integrate the user daemon with an unprivileged synthetic observer before using real privileged events.

**Checklist**

- [ ] Implement observer-client interface.
- [ ] Implement a test transport and synthetic event source.
- [ ] Add strict schema validation.
- [ ] Add version negotiation.
- [ ] Add observer-availability reporting.
- [ ] Add disabled, unavailable, and backend-lost states.
- [ ] Keep the production observer disabled until explicitly configured.

**Minimum tests**

- [ ] Integration tests for synthetic start/stop events.
- [ ] Malformed-message tests.
- [ ] Oversized-message tests.
- [ ] Replayed-message tests.
- [ ] Unknown-version tests.
- [ ] Observer disconnect/reconnect tests.
- [ ] Observer absence produces an explicit unavailable state.
- [ ] No synthetic event can bypass confirmation rules.

**Exit criteria**

- The user daemon can consume validated V4L2 events and expose correct session state without privileged code being present.

### Iteration V4 — eBPF observer prototype

**Goal:** create the minimal privileged observer and prove metadata-only capture detection.

**Checklist**

- [ ] Implement narrowly scoped BPF hooks.
- [ ] Track relevant V4L2 file descriptors.
- [ ] Filter for capture-relevant devices and operations.
- [ ] Observe operation return values.
- [ ] Emit only approved event fields.
- [ ] Implement clean attach/detach lifecycle.
- [ ] Add kernel and capability detection.
- [ ] Implement restricted local IPC.
- [ ] Add security-oriented structured logging.

**Minimum tests**

- [ ] Unit tests for userspace serialization and validation.
- [ ] Integration tests using a controlled V4L2 client.
- [ ] Negative tests for unrelated ioctls.
- [ ] Negative tests for unrelated character devices.
- [ ] Failed capture operations do not become confirmed sessions.
- [ ] Device-open-only behavior does not activate the indicator.
- [ ] No frame payload appears in events or logs.
- [ ] No command-line data appears in events or logs.
- [ ] Service restart and crash-cleanup tests.
- [ ] All BPF links detach after shutdown.

**Exit criteria**

- The observer detects confirmed direct capture using a narrowly documented privilege model and no prohibited data collection.

### Iteration V5 — Identity resolution and session correctness

**Goal:** produce accurate application and camera sessions from V4L2 events.

**Checklist**

- [ ] Reuse or implement the process/application resolver.
- [ ] Add a safe unknown-application fallback.
- [ ] Implement physical-camera identity resolution.
- [ ] Distinguish multiple cameras exposed by one physical device.
- [ ] Handle helper processes and browser process models.
- [ ] Protect against PID reuse.
- [ ] Handle duplicate FDs and repeated start/stop operations.
- [ ] Preserve ambiguity in diagnostics.

**Minimum tests**

- [ ] Unit tests for exact, partial, ambiguous, and conflicting identity matches.
- [ ] One process using one camera produces one active session.
- [ ] Two processes using one camera produce distinguishable sessions.
- [ ] One application using two cameras produces distinguishable sessions.
- [ ] Repeated start events do not create duplicates.
- [ ] Final close ends the correct session.
- [ ] Process exit ends all sessions owned by that process.
- [ ] PID reuse cannot inherit an old session.

**Exit criteria**

- Session identity is reliable enough for normal user-facing application and camera labels.

### Iteration V6 — GNOME Shell integration

**Goal:** connect the V4L2-only daemon state to the GNOME Shell extension.

**Checklist**

- [ ] Expose active sessions through user-session D-Bus.
- [ ] Expose observer availability and error state.
- [ ] Show the indicator only for confirmed sessions.
- [ ] Show application and camera details.
- [ ] Show monitoring-unavailable diagnostics.
- [ ] Handle daemon restart and D-Bus reconnection.
- [ ] Keep the extension free of privileged operations.
- [ ] Keep the extension independent of observer IPC details.

**Minimum tests**

- [ ] Extension enable/disable smoke test.
- [ ] Confirmed synthetic session activates the indicator.
- [ ] Open-only event does not activate the indicator.
- [ ] Session stop hides the indicator.
- [ ] Daemon restart recovers state.
- [ ] Observer loss changes diagnostics and terminates active sessions safely.
- [ ] Extension does not crash when the daemon is absent.
- [ ] No direct privileged-observer connection exists from GJS.

**Exit criteria**

- GNOME Shell accurately presents V4L2-confirmed camera use and observer availability.

### Iteration V7 — Compatibility, privacy, security, and performance validation

**Goal:** validate real-world behavior before release.

**Checklist**

- [ ] Run the full device/application compatibility matrix.
- [ ] Complete privacy review.
- [ ] Complete observer threat-model review.
- [ ] Review service, socket, capability, and policy permissions.
- [ ] Measure idle CPU, memory, wake-ups, and battery impact.
- [ ] Measure active overhead with one and multiple clients.
- [ ] Test observer absence, denial, restart, upgrade, and version mismatch.
- [ ] Document remaining blind spots.

**Minimum tests**

- [ ] `tests/manual/shell-camera-access.sh`.
- [ ] `tests/manual/shell-camera-access-multiple.sh`.
- [ ] Native V4L2 client using streaming I/O.
- [ ] Native V4L2 client using read I/O where supported.
- [ ] Browser using direct V4L2 where applicable.
- [ ] Flatpak client with direct device access.
- [ ] Virtual camera.
- [ ] UVC webcam.
- [ ] Integrated camera.
- [ ] Camera removal during capture.
- [ ] Daemon restart during capture.
- [ ] Observer restart during capture.
- [ ] User logout/login.
- [ ] Failed capture request.
- [ ] Device open and query without capture.

**Exit criteria**

- Compatibility, privacy, security, and performance reports are approved with no unresolved critical issue.

### Iteration V8 — Release and packaging

**Goal:** publish LensGuard as a V4L2-only monitor with a separately packaged privileged observer.

**Checklist**

- [ ] Package the user daemon.
- [ ] Package the observer separately.
- [ ] Add installation and removal documentation.
- [ ] Add privilege and privacy documentation.
- [ ] Add supported-kernel and distribution documentation.
- [ ] Add clear unavailable-state documentation.
- [ ] Add rollback and version-compatibility instructions.
- [ ] Confirm the GNOME extension ZIP contains no privileged binary.
- [ ] Confirm the extension-store description explains the external service requirement.

**Minimum tests**

- [ ] Clean installation on each supported distribution.
- [ ] Clean removal of extension, user daemon, observer, BPF objects, capabilities, sockets, and policy.
- [ ] Upgrade from a previous LensGuard version.
- [ ] Observer/user-daemon version mismatch behavior.
- [ ] Packaging policy validation.
- [ ] Reproducible build verification.
- [ ] Full unit, integration, smoke, security, privacy, and performance suites.

**Exit criteria**

- LensGuard can be installed, upgraded, used, and removed safely with clear documentation of its privileged observer and remaining blind spots.

## Required test categories

### Unit tests

- V4L2 capture state transitions.
- Successful versus failed operation handling.
- Open-only confidence behavior.
- Duplicate and out-of-order events.
- Process identity fallback.
- PID reuse.
- Physical-device identity matching.
- Final close and process exit.
- Device removal.
- Backend loss and recovery.
- IPC schema validation.

### Integration tests

- Synthetic V4L2 start/stop events.
- Controlled real V4L2 clients.
- Streaming-I/O capture.
- Read-I/O capture where supported.
- Observer disconnect/reconnect.
- Version negotiation and incompatibility.
- Restricted IPC authorization.
- Device removal.
- Process termination.
- Daemon restart.
- Observer restart.

### Smoke tests

- Observer installed and available.
- Observer not installed.
- Observer disabled.
- Observer missing required capability.
- Observer blocked by kernel or security policy.
- GNOME extension enable/disable.
- User daemon restart.
- Observer restart.
- Login/logout.

### Security tests

- Malformed and oversized IPC payloads.
- Unauthorized IPC client attempts.
- Arbitrary BPF program injection is impossible.
- Arbitrary trace-target injection is impossible.
- Arbitrary device-path injection is impossible.
- Observer emits no frame data, process memory, complete command lines, or unrelated syscall metadata.
- Capabilities are no broader than documented.
- Service and socket permissions are correct.
- BPF objects detach cleanly after shutdown or crash.
- User daemon treats observer input as untrusted.

### Privacy review

Confirm that LensGuard does not read or store:

- image frames;
- frame-buffer contents;
- complete command lines;
- process memory;
- persistent application-access history;
- network identifiers;
- telemetry.

Allowed metadata must be documented and limited to:

- process ID/TGID;
- file descriptor where needed for state tracking;
- stable device identity;
- capture operation and result;
- monotonic timestamp;
- observer availability;
- application identity resolved locally by the unprivileged daemon.

### Performance tests

Measure:

- idle CPU utilization;
- idle wake-ups;
- resident memory;
- event latency;
- active overhead with one client;
- active overhead with several clients;
- battery impact on a laptop;
- recovery cost after daemon or observer restart.

## Release criteria

LensGuard may be released when it:

- reliably detects successful direct V4L2 capture on documented supported systems;
- does not report a mere device open or probe as active capture;
- distinguishes active sessions by process and camera;
- uses a separately packaged, narrowly privileged observer;
- reads no frames, buffers, complete command lines, or process memory;
- passes the full unit, integration, smoke, security, privacy, compatibility, and performance test suites;
- reports observer unavailability clearly;
- has clear installation, removal, capability, kernel-support, and blind-spot documentation;
- contains no privileged binary inside the GNOME extension ZIP.

## Final product statement

LensGuard's supported architecture is:

```text
Detection:
direct native direct V4L2 capture monitoring only

Kernel observation:
narrowly privileged eBPF observer

Desktop integration:
unprivileged user daemon and GNOME Shell extension
```

LensGuard reports only confirmed V4L2 capture activity. It does not use PipeWire or WirePlumber, does not treat a device open as active capture, does not read image data, and keeps privileged kernel observation isolated from the unprivileged desktop UI.
