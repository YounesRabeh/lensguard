# LensGuard

LensGuard is a planned GNOME Shell camera-privacy indicator backed by an unprivileged Rust user
daemon. It will observe active camera capture through PipeWire and expose session state to a
GNOME Shell extension over the user D-Bus.

## Architecture

The project has completed its pure-domain Step 2. It does not yet connect to `PipeWire`, run a
D-Bus service, or display a camera indicator.

The repository follows ports and adapters:

- `camera-core` owns pure domain models, idempotent event reduction, ordered snapshots, and port
  traits without desktop dependencies.
- `camera-pipewire` will adapt PipeWire graph events to the domain boundary.
- `camera-app-resolver` will resolve process and desktop application identity.
- `camera-dbus` will translate internal state into a stable user-session D-Bus API.
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
```

See [docs/development.md](docs/development.md) for Fedora setup, recorded local versions, generic
distribution guidance, and the current manual GNOME lifecycle check. Implementation sequencing
and scope are defined in [Plan.md](Plan.md). Domain invariants and dependency rules are documented
in [docs/architecture.md](docs/architecture.md).

## License

MIT. See [LICENSE](LICENSE).
