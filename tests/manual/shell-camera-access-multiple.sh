#!/usr/bin/env bash
# SPDX-License-Identifier: MIT

# Start multiple terminal V4L2 clients against the same camera.
# Usage: ./tests/manual/shell-camera-access-multiple.sh [/dev/video0] [clients] [seconds]

set -euo pipefail

device=${1:-/dev/video0}
client_count=${2:-2}
duration=${3:-30}
pids=()

cleanup() {
    for pid in "${pids[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    for pid in "${pids[@]}"; do
        wait "$pid" 2>/dev/null || true
    done
}
trap cleanup EXIT INT TERM

if ! command -v gst-launch-1.0 >/dev/null; then
    printf '%s\n' 'gst-launch-1.0 is required (install gstreamer1-plugins-base).' >&2
    exit 1
fi

if [[ ! -c "$device" ]]; then
    printf 'Camera device is not available: %s\n' "$device" >&2
    exit 1
fi

if ! [[ "$client_count" =~ ^[2-9][0-9]*$ ]]; then
    printf 'Client count must be at least 2: %s\n' "$client_count" >&2
    exit 1
fi

if ! [[ "$duration" =~ ^[1-9][0-9]*$ ]]; then
    printf 'Duration must be a positive number of seconds: %s\n' "$duration" >&2
    exit 1
fi

printf 'Starting %s terminal camera clients on %s for %s seconds.\n' \
    "$client_count" "$device" "$duration"
for index in $(seq 1 "$client_count"); do
    gst-launch-1.0 -q v4l2src device="$device" ! fakesink sync=false &
    pids+=("$!")
    printf 'Started client %s (PID %s).\n' "$index" "$!"
done

sleep 2
running=0
for pid in "${pids[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then
        ((running += 1))
    fi
done

if ((running < client_count)); then
    printf '%s\n' \
        'One or more clients exited: this camera driver does not allow concurrent direct V4L2 capture.' >&2
    printf '%s\n' \
        'This is a hardware/driver limit, not a LensGuard failure.' >&2
    exit 1
fi

printf 'All %s clients are capturing. Press Ctrl+C to stop early.\n' "$client_count"
remaining=$((duration > 2 ? duration - 2 : 0))
sleep "$remaining"
