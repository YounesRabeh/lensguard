#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

test_root=$(mktemp -d --tmpdir lensguard-extension-dbus.XXXXXX)
cleanup() {
    rm -rf -- "$test_root"
}
trap cleanup EXIT

mkdir -p -- \
    "$test_root/home" \
    "$test_root/config" \
    "$test_root/data" \
    "$test_root/system-data"

env \
    HOME="$test_root/home" \
    XDG_CONFIG_HOME="$test_root/config" \
    XDG_DATA_HOME="$test_root/data" \
    XDG_DATA_DIRS="$test_root/system-data" \
    dbus-run-session -- \
    gjs -m "$repo_root/extension/tests/dbusClient.integration.js"
