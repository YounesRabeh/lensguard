#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later

# Open a V4L2 camera from a terminal without using GNOME Camera.
# Usage: ./tests/manual/shell-camera-access.sh [/dev/video0] [seconds]

set -euo pipefail

device=${1:-/dev/video0}
duration=${2:-30}

if ! command -v gst-launch-1.0 >/dev/null; then
    printf '%s\n' 'gst-launch-1.0 is required (install gstreamer1-plugins-base).' >&2
    exit 1
fi

if [[ ! -c "$device" ]]; then
    printf 'Camera device is not available: %s\n' "$device" >&2
    exit 1
fi

if ! [[ "$duration" =~ ^[1-9][0-9]*$ ]]; then
    printf 'Duration must be a positive number of seconds: %s\n' "$duration" >&2
    exit 1
fi

printf 'Opening %s for %s seconds. Press Ctrl+C to stop early.\n' \
    "$device" "$duration"
set +e
timeout --foreground "$duration" \
    gst-launch-1.0 -q v4l2src device="$device" ! fakesink sync=false
status=$?
set -e
if [[ $status -eq 0 ]]; then
    exit 0
fi
if [[ $status -eq 124 || $status -eq 130 ]]; then
    exit 0
fi

printf 'Could not capture from %s (the device may already be in use).\n' \
    "$device" >&2
exit "$status"
