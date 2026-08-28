# Development

[< Back to LensGuard](../README.md)

Install Rust 1.85 or newer, Clang with the BPF target, GNU Make, D-Bus tools, GJS, GNOME Shell,
`glib-compile-schemas`, pnpm, ripgrep, shellcheck, and unzip.

```bash
./scripts/dev/bootstrap-dev.sh
make check
```

`observer/build.rs` compiles `observer/bpf/v4l2.bpf.c` into the privileged binary. Normal unit and
synthetic IPC tests do not load BPF and need no privileges. The complete workspace test suite uses
private D-Bus sockets.

Useful commands:

```bash
cargo run -p camera-monitor -- --version
cargo run -p camera-monitor -- inspect-v4l2
cargo run -p camera-monitor -- watch-v4l2
```

The inspect/watch commands connect only to the fixed packaged observer socket. The observer accepts
no user-supplied path, tracing target, command, or BPF program.

## Testing the real observer

Build and install a native package, start `lensguard-v4l2-observer.service`, then run
`tests/manual/shell-camera-access.sh` with a streaming-I/O V4L2 client. Confirm that an open/query
does nothing, a successful stream-on creates exactly one session, stream-off/close ends it, and a
brokered camera app creates no LensGuard session.

Changes to the kernel hooks, raw event fields, capabilities, peer authentication, or broker policy
are security-boundary changes and should receive focused review.
