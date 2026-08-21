# Architecture

```text
GNOME Shell extension
        │ user-session D-Bus
camera-monitor (unprivileged)
        │ length-prefixed, versioned Unix IPC
lensguard-v4l2-observer (privileged, per-UID routing)
        │ eBPF tracepoints
successful V4L2 STREAMON / STREAMOFF / tracked close / process exit
```

## Detection boundary

The eBPF object filters ioctl traffic in-kernel to the `VIDIOC_STREAMON` and
`VIDIOC_STREAMOFF` request numbers and emits only successful results. Close events are emitted
only for file descriptors already confirmed by a successful stream start. Process-exit events are
emitted only for a process with tracked capture. Userspace resolves the file descriptor through
procfs, verifies that its character-device major/minor belongs to video4linux in sysfs, and builds
physical identity without opening the camera.

The observer never sees frame contents. It does not trace generic file opens, arbitrary ioctls, or
generic reads. This means streaming I/O is supported; read-I/O remains an explicit blind spot.

## Ownership and duplicate prevention

Before IPC, the observer classifies each confirmed candidate as `Direct`, `BrokerOwned`, or
`Unknown`. The versioned package policy identifies trusted broker executable paths. A matching
broker is accepted only when the executable is root-owned and not group- or world-writable. A
lookalike name at another path, an inaccessible process, a missing policy, or an unresolved device
becomes `Unknown`.

Only `Direct` events cross IPC as session-capable messages. Broker and unknown observations update
per-user aggregate counters without PID, path, application, device, or history. The daemon repeats
the owner check and rejects a non-direct injected event.

## IPC and session correctness

Messages use a four-byte big-endian length followed by strict JSON. Both peers negotiate schema
and workspace versions; frames over 16 KiB, unknown fields or variants, bad versions, invalid
results, invalid confidence, and oversized identity fields are rejected. The system observer
authenticates the peer PID and installed daemon executable and routes capture only to the matching
UID.

Sessions are keyed by TGID, procfs start time, file descriptor, device major/minor, and capture
lifetime. This protects against PID and FD reuse. Duplicate starts are idempotent; unknown or
out-of-order stops are harmless. Close, process exit, stream-off, device removal, and observer loss
end the matching sessions. Observer loss is treated as interruption and clears user-visible active
state before unavailability is published.

## Privilege boundary

The system service is separately built and packaged. Its systemd unit limits capabilities,
address families, namespaces, writable paths, and syscall families. BPF links and reader tasks are
owned by the observer process and detach on normal shutdown or process death. The user daemon and
extension never load BPF, open camera devices, elevate privileges, or communicate around this
boundary.
