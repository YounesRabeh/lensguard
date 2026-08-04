# LensGuard

LensGuard is a planned GNOME Shell camera-privacy indicator backed by an unprivileged Rust user
daemon. It will observe active camera capture through PipeWire and expose session state to a
GNOME Shell extension over the user D-Bus.

## Architecture

The project has completed Step 6's stable user-session D-Bus contract. It observes and classifies
the current `PipeWire` graph, emits domain lifecycle events for complete camera-to-application
relationships, safely enriches them from process and desktop metadata, and can publish an
independently testable D-Bus endpoint. The long-running orchestration and panel indicator are not
implemented yet.

The repository follows ports and adapters:

- `camera-core` owns pure domain models, idempotent event reduction, ordered snapshots, and port
  traits without desktop dependencies.
- `camera-pipewire` adapts PipeWire graph events to the domain boundary.
- `camera-app-resolver` resolves process, desktop-entry, and Flatpak application identity.
- `camera-dbus` translates internal snapshots and events into a stable user-session D-Bus API.
- `camera-monitor` will be the daemon composition root.
- `extension/` contains the GNOME Shell client and must communicate with the daemon only over
  the documented D-Bus contract.

Infrastructure-specific types must not leak into `camera-core`, and D-Bus data-transfer objects
must remain separate from domain entities.

## Quick start

```sh
make bootstrap
make check
cargo run -p camera-monitor -- --version
cargo run -p camera-monitor -- inspect-pipewire
cargo run -p camera-monitor -- watch-pipewire
cargo run -p camera-monitor -- serve-dbus
```

See [docs/development.md](docs/development.md) for Fedora setup, recorded local versions, generic
distribution guidance, and the current manual GNOME lifecycle check. Implementation sequencing
and scope are defined in [Plan.md](Plan.md). Domain invariants and dependency rules are documented
in [docs/architecture.md](docs/architecture.md).

The inspection command prints a one-time raw graph summary. The watcher prints correlated and
identity-enriched domain start, update, and stop events. See
[tests/manual/camera-session.md](tests/manual/camera-session.md) for live-session checks.
The Step 6 `serve-dbus` command exposes an initially empty endpoint for independent clients; Step
7 will connect the PipeWire event stream to that service. The public wire contract and CLI
examples are in [docs/dbus-api.md](docs/dbus-api.md).

## License

MIT. See [LICENSE](LICENSE).
