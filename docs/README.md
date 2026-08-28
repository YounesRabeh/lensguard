# LensGuard documentation

[< Back to LensGuard](../README.md)

Use these guides to install, operate, develop, and release LensGuard.

| I want to… | Start here |
| --- | --- |
| Get familiar with LensGuard | [Getting started](getting-started.md) |
| Install, upgrade, or remove it | [Installation](installation.md) |
| Understand detection and privacy boundaries | [Architecture](architecture.md) |
| Integrate with the user-session service | [D-Bus API](dbus-api.md) |
| Diagnose missing or unexpected activity | [Troubleshooting](troubleshooting.md) |
| Set up a contributor environment or test changes | [Development](development.md) |
| Build or publish packages | [Release packaging](release.md) · [Release checklist](release-checklist.md) |

LensGuard detects confirmed direct V4L2 streaming. It deliberately leaves trusted desktop-broker
camera sessions to GNOME's own privacy indicator and never captures frames or video buffers.
