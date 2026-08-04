# Development

LensGuard has completed Step 5 application identity resolution. A live adapter emits domain
camera-session events enriched from trusted PipeWire hints, procfs, standard desktop entries, and
Flatpak metadata. D-Bus and UI behavior do not exist yet.

## Recorded local environment

The baseline was captured on 2026-08-04 on Fedora Linux 44 Workstation:

| Component | Detected version or status |
| --- | --- |
| GNOME Shell | 50.3; extension metadata targets compatibility value `50` |
| GJS | 1.88.1 |
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
  pipewire pipewire-devel wireplumber dbus-tools systemd unzip
```

ShellCheck is optional but recommended:

```sh
sudo dnf install ShellCheck
```

The project requires Rust 1.85 or newer because it uses the Rust 2024 edition. A current stable
Rust toolchain installed through rustup is also supported; add the `rustfmt` and `clippy`
components when using that distribution.

## Other Linux distributions

Install equivalent packages providing stable Rust 1.85+, Cargo, rustfmt, Clippy, Clang/libclang,
pkg-config, GNU Make, GJS, GNOME Shell extension tooling, PipeWire tools and development headers,
WirePlumber tools, D-Bus command-line tools, systemd, and unzip. GNOME compatibility is
deliberately recorded in `extension/metadata.json`; update and test that value before using the
extension on another major GNOME release.

## Commands

The bootstrap command only reports prerequisites; it does not install packages or alter the
system:

```sh
make bootstrap
```

Run the complete baseline, or its individual parts:

```sh
make format
make lint
make test
make build
make check
```

`make build` writes Rust artifacts to `target/` and a development extension bundle to `dist/`.
Both directories are ignored by Git. To inspect the daemon bootstrap executable:

```sh
cargo run -p camera-monitor -- --version
cargo run -p camera-monitor -- inspect-pipewire
cargo run -p camera-monitor -- watch-pipewire
```

The watcher resolves application names on the daemon consumer thread. A typical event is
`START ... application="Snapshot" ...`; if metadata is unavailable it uses a safe process-based
name or `Unknown application` without dropping the session.

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

Step 1 intentionally has no live GNOME enable/disable test automation. To verify lifecycle
loading manually, install the generated bundle in a disposable GNOME user session, enable and
disable it with the Extensions application, and confirm the Shell journal has no LensGuard
errors. The extension adds no panel item at this stage.
