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
make package-all
make release-candidate
```

`make package-all` builds fresh DEB, RPM, and Arch package sets, a complete latest-artifact
directory, and the current version's release candidate. It replaces `dist/packages/{deb,rpm,arch}`
and `dist/release-artifacts`, replaces only `dist/release/<current-version>`, and preserves all
other versioned release directories. If `makepkg` is unavailable, the command builds the Arch
packages in the digest-pinned Arch Linux CI image through Podman or Docker.

Scripts may still be run directly when debugging. Run them from any directory; each script resolves
the repository root from its own location.
