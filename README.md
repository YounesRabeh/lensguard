![Lens Guard](docs/images/lensguard-banner.png)

See which applications are using your camera, right from the GNOME desktop.

Lens Guard is a small, privacy-focused GNOME Shell extension for Linux. When your camera is
active, it shows a clear indicator and lets you see the application and camera involved from
Quick Settings. When nothing is using the camera, the menu stays quiet.

![Lens Guard showing active camera sessions](docs/images/step10-active-sessions.png)

## Why Lens Guard?

- Know when your camera is being used without opening another application.
- See the active application and camera in one place.
- Get a visible unavailable state when monitoring cannot be confirmed.
- Keep your camera information local to your user session, no account or cloud service is needed.

> [!NOTE]
> Lens Guard uses the same user session that GNOME and PipeWire use. It does not need root access, sudo, or privileged camera permissions.

## Get started

Install Lens Guard for your user account, then enable it in GNOME Extensions. The complete
installation and removal instructions are in the [Getting started guide](docs/getting-started.md).

If you are installing from the source repository, see the [installation guide](docs/installation.md).

## A note about detection

Lens Guard currently detects camera sessions that appear in PipeWire, which covers the normal
GNOME desktop and most modern applications. Direct applications that open `/dev/video*` without
PipeWire, and some virtual-camera or multi-hop setups, are not detected yet. See
[known limitations](docs/troubleshooting.md#known-limitations-and-ambiguous-graph-cases) for the
details and the planned direct-V4L2 work.

## Screenshots

![Lens Guard reporting that camera monitoring is unavailable](docs/images/step10-backend-unavailable.png)

Open the Lens Guard item in Quick Settings and choose **Preferences** to customize the indicator
and warning behavior. Notifications are intentionally not used.

## Learn more

- [Getting started](docs/getting-started.md) — install, enable, update, and remove Lens Guard.
- [Installation details](docs/installation.md) — service activation and distribution packages.
- [Troubleshooting](docs/troubleshooting.md) — fixes for common camera and service issues.
- [Development guide](docs/development.md) — build, test, and contribute locally.
- [Architecture](docs/architecture.md) — how the extension and user daemon fit together.
- [Release guide](docs/release.md) — maintainers' packaging and publishing process.

## License

Lens Guard is free software released under the [GNU General Public License v3.0 or later](LICENSE).
