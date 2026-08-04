#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
archive=${1:-$repo_root/dist/lensguard@younesrabeh.github.io.shell-extension.zip}
output_dir=${2:-$repo_root/docs/images}
log_file=$(mktemp --tmpdir lensguard-screenshots.XXXXXX.log)
trap 'rm -f -- "$log_file"' EXIT

if [[ ! -s $archive ]]; then
    printf '%s\n' "extension archive not found: $archive" >&2
    exit 1
fi

mkdir -p -- "$output_dir"
dbus-run-session -- env \
    GSETTINGS_BACKEND=memory \
    LENSGUARD_SCREENSHOT_DIR="$output_dir" \
    gnome-shell-test-tool \
        --headless \
        --disable-animations \
        --extension "$archive" \
        "$repo_root/extension/tests/gnomeScreenshot.js" 2>&1 | tee "$log_file"

if rg -q 'Script failed:|Failed to load extension|Extension lensguard.*: Error|Extension .* had error' "$log_file"; then
    printf '%s\n' 'GNOME screenshot log contains an extension or automation failure.' >&2
    exit 1
fi

for screenshot in \
    step10-active-sessions.png \
    step10-backend-unavailable.png; do
    if [[ ! -s $output_dir/$screenshot ]]; then
        printf '%s\n' "screenshot was not created: $output_dir/$screenshot" >&2
        exit 1
    fi
done

printf '%s\n' "Created Step 10 screenshots in $output_dir"
