#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

dbus-run-session -- \
    gjs -m "$repo_root/extension/tests/dbusClient.integration.js"
