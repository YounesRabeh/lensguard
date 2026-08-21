# V4L2 observer implementation review

## Decision

LensGuard uses syscall tracepoints with in-kernel request/result correlation for streaming V4L2.
This mechanism proves successful `VIDIOC_STREAMON` and `VIDIOC_STREAMOFF`, tracks confirmed FDs
for close, records process exit, and supplies PID/TGID, UID, FD, monotonic time, operation, and
result. Userspace resolves and validates character-device and physical-device identity before IPC.

The BPF object contains BTF metadata but uses stable tracepoint layouts rather than kernel-structure
offsets, reducing CO-RE relocation requirements. Supported systems must expose the syscall and
scheduler tracepoints plus BPF perf-event output.

## Mechanism matrix

| Mechanism | Success result | FD/process | Camera scoped | Decision |
|---|---:|---:|---:|---|
| syscall enter/exit tracepoints, filtered ioctl numbers | yes | yes | userspace major/minor + sysfs validation | selected |
| V4L2 fentry/fexit | yes | file pointer, FD mapping harder | yes | rejected for kernel-symbol portability |
| kprobe/kretprobe | yes | architecture-sensitive arguments | potentially | rejected for portability |
| open-FD scanning | no | yes | yes | rejected: open is not capture |
| audit | attempt/result correlation varies | yes | filterable | rejected: broader persistent audit surface |
| generic read tracepoint | yes | yes | not safely in-kernel without broader FD tracking | deferred |

## Threat controls

- The loader accepts no runtime tracing targets, paths, commands, scripts, plugins, or BPF objects.
- Kernel code emits no frame pointer, buffer, command line, path, or application string.
- The broker policy is installed read-only with the observer; a parse failure fails closed.
- Broker identity requires an exact policy path plus root ownership and non-writable executable
  mode. Same-name lookalikes become unknown.
- IPC is size-limited, schema/version negotiated, strict-deserialized, peer-executable
  authenticated, and routed by peer/event UID.
- The daemon independently rejects non-direct owner values, failed operations, open-only
  confidence, stale ordering, PID reuse, invalid enums, and oversized identity.
- Systemd constrains capabilities, filesystems, networking, namespaces, and syscalls. Process death
  closes all BPF links; normal shutdown explicitly drops the loader before removing the socket.

## Compatibility and limits

| Capture type | Status |
|---|---|
| UVC/integrated/media-controller node using streaming I/O | supported when it reaches a V4L2 video node |
| direct browser/Electron/native/Flatpak access | supported when it successfully issues `VIDIOC_STREAMON` |
| trusted broker capture | classified and suppressed |
| virtual `v4l2loopback` node | supported when sysfs exposes it as video4linux |
| V4L2 read-I/O | not supported; generic read tracing was rejected as too broad |
| libcamera without a V4L2 video-node stream | not supported |
| hidden procfs or denied BPF/perf access | explicit unavailable/unknown state |

Automated tests cover validation, failed/open-only/non-direct suppression, every session end path,
duplicates, ordering, PID reuse, malformed/oversized/versioned IPC, synthetic start/stop, observer
loss, D-Bus diagnostics, UI unknown activity, clean object construction, and package boundaries.
Hardware/kernel validation still requires a supported host and camera; use
[`tests/manual/shell-camera-access.md`](../tests/manual/shell-camera-access.md).
