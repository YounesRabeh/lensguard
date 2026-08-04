#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
archive=${1:-dist/lensguard@younesrabeh.github.io.shell-extension.zip}
log_file=$(mktemp --tmpdir lensguard-gnome-smoke.XXXXXX.log)
trap 'rm -f -- "$log_file"' EXIT

if [[ ! -s $archive ]]; then
    printf '%s\n' "extension archive not found: $archive" >&2
    exit 1
fi

set +e
dbus-run-session -- env GSETTINGS_BACKEND=memory gnome-shell-test-tool \
    --headless \
    --disable-animations \
    --extension "$archive" \
    "$repo_root/extension/tests/gnomeSmoke.js" 2>&1 | tee "$log_file"
test_status=${PIPESTATUS[0]}
set -e

if ((test_status != 0)); then
    printf '%s\n' "GNOME smoke process failed with status $test_status" >&2
    exit "$test_status"
fi

if rg -q 'Script failed:|Failed to load extension|Extension lensguard.*: Error|Extension .* had error' "$log_file"; then
    printf '%s\n' 'GNOME smoke log contains an extension or automation failure.' >&2
    exit 1
fi

printf '%s\n' 'GNOME Shell D-Bus UI smoke test passed.'
