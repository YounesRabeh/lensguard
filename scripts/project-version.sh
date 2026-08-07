#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

awk '
    /^\[workspace\.package\]$/ { inside = 1; next }
    inside && /^\[/ { exit }
    inside && $1 == "version" {
        gsub(/"/, "", $3)
        print $3
        found = 1
        exit
    }
    END { if (!found) exit 1 }
' "$repo_root/Cargo.toml"
