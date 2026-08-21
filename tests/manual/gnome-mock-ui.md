# GNOME mock UI test

> Historical Step 8 record. Step 9 removed the production mock setting and provider; these live
> `gsettings` commands no longer apply to current packages. Mock fixtures now exist only under
> `extension/tests/`.

Environment:

- Fedora Linux 44 Workstation, GNOME Shell 50.3, GJS 1.88.1.
- Automated run used `gnome-shell-test-tool` with a headless 1280x720 virtual monitor and an
  isolated D-Bus session on 2026-08-04.

Preconditions:

- Run `pnpm install --frozen-lockfile` and `make build` from the repository root.
- For visual review, use a GNOME 50 user session and install the generated extension archive.
- The Rust daemon and a physical camera are not required; Step 8 uses mock data only.

Steps:

1. Run the isolated automated UI check:

   ```sh
   ./scripts/dev/smoke-extension.sh \
     dist/lensguard@younesrabeh.github.io.shell-extension.zip
   ```

2. For live visual review, install and enable the archive:

   ```sh
   gnome-extensions install --force \
     dist/lensguard@younesrabeh.github.io.shell-extension.zip
   gnome-extensions enable lensguard@younesrabeh.github.io
   ```

3. Define the installed extension schema location:

   ```sh
   lensguard_schema_dir="$HOME/.local/share/gnome-shell/extensions/lensguard@younesrabeh.github.io/schemas"
   ```

4. Exercise each state from a terminal in the GNOME session:

   ```sh
   GSETTINGS_SCHEMA_DIR="$lensguard_schema_dir" gsettings set \
     org.gnome.shell.extensions.lensguard mock-state inactive
   GSETTINGS_SCHEMA_DIR="$lensguard_schema_dir" gsettings set \
     org.gnome.shell.extensions.lensguard mock-state active-one
   GSETTINGS_SCHEMA_DIR="$lensguard_schema_dir" gsettings set \
     org.gnome.shell.extensions.lensguard mock-state active-multiple
   GSETTINGS_SCHEMA_DIR="$lensguard_schema_dir" gsettings set \
     org.gnome.shell.extensions.lensguard mock-state observer-unavailable
   ```

5. Open Quick Settings after each change. Hover the visible LensGuard top-bar icon to inspect its
   tooltip. In active states, open the LensGuard tile's submenu.
6. Disable and enable the extension three times, then disable it once more:

   ```sh
   for cycle in 1 2 3; do
     gnome-extensions disable lensguard@younesrabeh.github.io
     gnome-extensions enable lensguard@younesrabeh.github.io
   done
   gnome-extensions disable lensguard@younesrabeh.github.io
   ```

7. Inspect current-boot Shell logs:

   ```sh
   journalctl --user -b -o cat | rg -i 'lensguard|extension.*error'
   ```

Expected result:

- The extension enables without an error.
- `inactive` hides the top-bar icon and the tile says `No camera in use`.
- `active-one` shows the symbolic privacy camera icon and lists
  `Discord — Integrated Camera`.
- `active-multiple` lists both Discord and `Firefox (Google Meet) — USB Webcam` in stable order.
- `observer-unavailable` shows a warning icon and explicitly says monitoring is unavailable; the
  camera-active state is not checked.
- Hover text and accessible labels describe the current state.
- Repeated lifecycle cycles produce exactly one indicator and signal handler, and final disable
  removes the icon, tile, tooltip, settings signal, and menu without Shell errors.

Actual result:

- The automated GNOME Shell 50.3 test passed all inactive, one-session, multi-session,
  observer-unavailable, and three repeated enable/disable assertions.
- It verified exact app/camera menu labels, symbolic icon names, warning semantics, absence of
  duplicate indicators, and complete final removal.
- Visual review commands are documented above; the automated run inspected the live Shell actor
  tree rather than modifying the user's desktop session.

Logs collected:

- `scripts/dev/smoke-extension.sh` captured the isolated Shell log and rejected `Script failed`,
  extension load failures, and extension error states.
- Final line: `GNOME Shell mock UI smoke test passed.`
- Expected headless-session service warnings were present; no LensGuard or automation error was
  present in the passing run.

Pass/fail:

PASS

Notes:

- Step 8 is intentionally mock-only. Live daemon state and D-Bus reconnection behavior begin in
  Step 9.
- The top-bar icon is inserted through GNOME's Quick Settings external-indicator API next to the
  built-in privacy indicators.
