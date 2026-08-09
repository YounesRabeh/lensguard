# Scripts

Use the repository `Makefile` for common tasks. The scripts below are grouped by responsibility so
internal helpers are easy to distinguish from commands intended for developers.

| Directory | Purpose |
| --- | --- |
| `dev/` | Development bootstrap, GNOME smoke tests, and screenshot capture |
| `install/` | Per-user source installation and removal |
| `package/` | Extension and native distribution package builders |
| `release/` | Release assembly, validation, and dependency-license auditing |
| `test/` | Repository, D-Bus, installer, and installed-package test harnesses |
| `util/` | Small shared utilities such as project-version lookup and synchronization |

Common entry points:

```bash
make bootstrap
make check
make install-local
make uninstall-local
make package-deb
make package-rpm
make package-arch
make release-candidate
```

Scripts may still be run directly when debugging. Run them from any directory; each script resolves
the repository root from its own location.
