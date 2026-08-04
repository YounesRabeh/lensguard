# Development

LensGuard has completed Step 12's user-session integration scope. The daemon supervises
PipeWire with bounded retry, resolves application identity off native callbacks, reconciles stale
sessions after backend loss, and publishes the resulting state over the user-session D-Bus. The
extension uses only asynchronous D-Bus calls, validates daemon payloads, and resynchronizes across
daemon loss and restart. It now provides an Adwaita preferences window, live backend-failure
presentation settings, bounded and sanitized external display strings, and accessible Quick
Settings labels. Production packages contain no mock setting or mock data provider, and
notifications remain intentionally unimplemented.

## Recorded local environment

The baseline was captured on 2026-08-04 on Fedora Linux 44 Workstation:

| Component | Detected version or status |
| --- | --- |
| GNOME Shell | 50.3; extension metadata targets compatibility value `50` |
| GJS | 1.88.1 |
| Node.js, pnpm, ESLint | 24.18.0, 11.18.0, 9.39.5 |
| Rust compiler and Cargo | 1.97.1, Fedora packages; workspace edition 2024 |
| `rustfmt` | 1.9.0 (installed before final Step 1 verification) |
| Clippy | 0.1.97 (installed before final Step 1 verification) |
| PipeWire | 1.6.8, executable and client library present |
| WirePlumber | 0.5.14, executable and client library present |
| D-Bus tools | `dbus-send`, `gdbus`, and `busctl` present |
| systemd | 259; user manager not reachable from the sandboxed development terminal |
| ShellCheck | Not installed; lint integration treats it as optional |

The user D-Bus address and runtime directory were present, but sandbox permissions prevented
connecting to the session bus. Run session-dependent checks from an ordinary terminal in the
GNOME login session. No Step 1 code requires a camera, PipeWire connection, D-Bus connection,
or running systemd user manager.

## Fedora setup

Install the development tools with Fedora's package manager. Package names can change between
Fedora releases; use `dnf search` if a listed package is unavailable.

```sh
sudo dnf install cargo rust rustfmt clippy clang pkgconf-pkg-config make gjs gnome-shell \
  glib2 nodejs pnpm ripgrep pipewire pipewire-devel wireplumber dbus-tools systemd unzip
```

ShellCheck is optional but recommended:

```sh
sudo dnf install ShellCheck
```

The project requires Rust 1.85 or newer because it uses the Rust 2024 edition. A current stable
Rust toolchain installed through rustup is also supported; add the `rustfmt` and `clippy`
components when using that distribution.

## Project version

The single project version source is `version` under `[workspace.package]` in the root
`Cargo.toml`. Every workspace package uses `version.workspace = true`, and internal dependencies
are inherited from `[workspace.dependencies]` without duplicating the project version. Changing
the root value therefore updates every LensGuard crate and the daemon together.

## Other Linux distributions

Install equivalent packages providing stable Rust 1.85+, Cargo, rustfmt, Clippy, Clang/libclang,
pkg-config, GNU Make, GJS, GNOME Shell extension tooling, PipeWire tools and development headers,
WirePlumber tools, D-Bus command-line tools, systemd, and unzip. GNOME compatibility is
deliberately recorded in `extension/metadata.json`; update and test that value before using the
extension on another major GNOME release.

## Commands

Install the locked JavaScript development tools, then run the bootstrap check. The bootstrap
command only reports prerequisites; it does not install packages or alter the system:

```sh
pnpm install --frozen-lockfile
make bootstrap
```

Run the complete baseline, or its individual parts:

```sh
make format
make lint
make test
make build
make check
make smoke-extension
make capture-extension-screenshots
make test-local-installation
make install-local
make uninstall-local
```

`make build` writes Rust artifacts to `target/` and a development extension bundle to `dist/`.
Both directories are ignored by Git. To inspect the daemon bootstrap executable:

```sh
cargo run -p camera-monitor -- --version
cargo run -p camera-monitor -- --log-level info run
cargo run -p camera-monitor -- inspect-pipewire
cargo run -p camera-monitor -- watch-pipewire
cargo run -p camera-monitor -- serve-dbus
```

The watcher resolves application names on the daemon consumer thread. A typical event is
`START ... application="Snapshot" ...`; if metadata is unavailable it uses a safe process-based
name or `Unknown application` without dropping the session.

No command and the explicit `run` command both start the functional daemon. `--log-level` accepts
`trace`, `debug`, `info`, `warn`, or `error` and can appear before or after the command. SIGINT and
SIGTERM both trigger graceful shutdown. `inspect-pipewire`, `watch-pipewire`, and `serve-dbus`
remain focused diagnostics; use `--help` to list the syntax.

The daemon acquires `io.github.younesrabeh.CameraMonitor`, initially reports the backend as
unavailable while connecting, and then publishes live state. Failed connections retry after
250 ms, doubling to a maximum of 30 seconds. Internal queues are bounded at 256 adapter events and
128 events for each application stage. Isolated D-Bus integration tests use `dbus-daemon`;
environments that forbid Unix socket creation must run those tests outside that sandbox.

## Troubleshooting application identity

- If the application remains `Unknown application`, inspect the application input node with
  `inspect-pipewire` and check whether it exposes `application.process.id`, `application.id`,
  `application.name`, or `application.process.binary`.
- A process may exit before procfs is read, or sandbox permissions may deny process metadata.
  These are expected non-fatal fallbacks.
- Confirm the matching `.desktop` file is beneath `$XDG_DATA_HOME/applications` or an
  `$XDG_DATA_DIRS` `applications` directory and contains a non-empty `Name`.
- Flatpak matching uses `X-Flatpak`, `.flatpak-info`, or recognizable systemd cgroup metadata.
  Portal topology may still prevent the application PID from being exposed.
- Do not paste complete process command lines into reports. The resolver intentionally never
  reads them, and it does not return desktop-entry icon file paths.

See [the application-resolution smoke test](../tests/manual/application-resolution.md) for live
verification.

See [the daemon lifecycle smoke test](../tests/manual/daemon-lifecycle.md) for reproducible normal,
signal, and backend-loss process checks.

## GNOME live D-Bus UI

Run the isolated GNOME 50 smoke test without changing the live desktop. It starts a fake D-Bus
service and verifies initial synchronization, one and multiple sessions, final stop, daemon loss,
restart resynchronization, and repeated extension reloads:

```sh
make smoke-extension
```

The fake service is restricted to test fixtures. The packaged extension always connects to
`io.github.younesrabeh.CameraMonitor` on the user session bus.

## Extension preferences

Open the LensGuard submenu in Quick Settings and select **Preferences**, or use the preferences
button in GNOME Extensions. Both settings are extension-owned, apply without restarting GNOME
Shell, and default to the most visible failure reporting:

| Setting | Default | Effect |
| --- | --- | --- |
| Show monitoring warnings | On | Uses explicit warning language and `dialog-warning-symbolic` while the PipeWire backend is unavailable. |
| Keep the status icon visible | On | Keeps a failure status icon in the panel while camera use cannot be determined. |

Turning warning presentation off uses neutral unavailable-state language and
`dialog-information-symbolic`; it never claims the camera is safe or inactive. Turning the failure
icon off only hides that panel status icon. It does not hide an active-camera indicator and does
not erase the unavailable state from the LensGuard Quick Settings menu.

The schema defaults and descriptions are tested from a freshly compiled temporary schema. The
GNOME Shell smoke suite changes both preferences live and verifies that they survive an extension
disable/enable cycle using an isolated in-memory settings backend.

For a physical-camera check, build first, open GNOME Camera, then run the hardware-gated smoke test
from another terminal. Stop capture within 30 seconds after the active marker appears:

```sh
make build
snapshot
# In another terminal:
make smoke-real-camera
```

The real-camera test loads the packaged extension in an isolated headless Shell, but the real Rust
daemon connects to the current user's PipeWire graph. This avoids changing the live desktop while
still exercising the complete PipeWire to D-Bus to GNOME actor path. Exact expected results and
the recorded Step 9 run are in
[the end-to-end camera test](../tests/manual/end-to-end-camera.md). The former
[Step 8 mock test](../tests/manual/gnome-mock-ui.md) is retained only as a historical record.
Step 10's full visual review and current screenshots are recorded in
[the GNOME UX review](../tests/manual/gnome-ux-review.md).
