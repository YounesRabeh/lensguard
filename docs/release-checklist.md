# Release checklist

Release version: `<workspace version>`  
Target packages: Ubuntu 26.04, Fedora 44, Arch Linux; GNOME Shell 50; x86_64

## Version and source

- [ ] Working tree contents reviewed.
- [ ] Root version, Cargo packages, daemon, D-Bus property, extension metadata, and artifacts match.
- [ ] `Cargo.lock` is current and `--locked` builds pass.
- [ ] Changelog and release notes are current.

## Automated gates

- [ ] The tagged commit is reachable from `main` and its version is newer than the latest stable release.
- [ ] Clean-source `make check` passes with the pinned Rust toolchain.
- [ ] Dependency license audit passes.
- [ ] Extension ZIP contains runtime files only.
- [ ] DEB, source-built RPM/SRPM, and Arch packages build in native distribution environments.
- [ ] `lintian`, `rpmlint`, and `namcap` report no errors.
- [ ] Package paths, ownership, modes, dependencies, and scriptlets are inspected.
- [ ] Release artifact checksums verify.

## Install and upgrade

- [ ] Standalone extension ZIP installs in the isolated GNOME smoke environment.
- [ ] Fresh per-user install and cold D-Bus activation pass.
- [ ] Every native package installs, cold-activates over D-Bus, and uninstalls cleanly.
- [ ] Upgrade from the previous stable package preserves preferences.
- [ ] Daemon and extension restart successfully after upgrade.
- [ ] Clean uninstall removes owned files and preserves unrelated files.

## Manual release smoke

- [ ] Representative Fedora/GNOME session tested.
- [ ] Camera start produces the active indicator and application row.
- [ ] Camera stop clears the final session and indicator.
- [ ] Known direct-V4L2 limitation remains documented.

## Approval

- [ ] The GitHub Release is still a draft and its complete artifact set was reviewed.
- [ ] Publishing the GitHub draft and GNOME Extensions submission were approved separately.
