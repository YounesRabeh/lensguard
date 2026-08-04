#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
archive=${1:-$repo_root/dist/lensguard@younesrabeh.github.io.shell-extension.zip}
daemon=${2:-$repo_root/target/debug/camera-monitor}
log_file=$(mktemp --tmpdir lensguard-real-camera.XXXXXX.log)
trap 'rm -f -- "$log_file"' EXIT

if [[ ! -x $daemon ]]; then
    printf '%s\n' "daemon executable not found: $daemon" >&2
    exit 1
fi

if [[ ! -s $archive ]]; then
    printf '%s\n' "extension archive not found: $archive" >&2
    exit 1
fi

dbus-run-session -- bash -c '
    set -euo pipefail
    daemon=$1
    archive=$2
    test_script=$3
    log_file=$4

    "$daemon" --log-level info run &
    daemon_pid=$!
    trap '\''kill -TERM "$daemon_pid" 2>/dev/null || true; wait "$daemon_pid" 2>/dev/null || true'\'' EXIT

    env GSETTINGS_BACKEND=memory gnome-shell-test-tool \
        --headless \
        --disable-animations \
        --extension "$archive" \
        "$test_script" 2>&1 | tee "$log_file"
' lensguard-real-camera \
    "$daemon" \
    "$archive" \
    "$repo_root/extension/tests/gnomeRealCameraSmoke.js" \
    "$log_file"

if rg -q 'Script failed:|Failed to load extension|Extension lensguard.*: Error|Extension .* had error' "$log_file"; then
    printf '%s\n' 'GNOME real-camera log contains an extension or automation failure.' >&2
    exit 1
fi

if ! rg -q 'LENSGUARD_REAL_CAMERA_ACTIVE' "$log_file" ||
    ! rg -q 'LENSGUARD_REAL_CAMERA_INACTIVE' "$log_file"; then
    printf '%s\n' 'GNOME real-camera test did not observe both camera transitions.' >&2
    exit 1
fi

printf '%s\n' 'GNOME Shell real-camera smoke test passed.'
