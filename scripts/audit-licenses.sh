#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output=${1:-}

for command_name in cargo jq; do
    command -v "$command_name" >/dev/null || {
        printf 'audit-licenses.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

metadata=$(mktemp --tmpdir lensguard-license-metadata.XXXXXX.json)
report=$(mktemp --tmpdir lensguard-license-report.XXXXXX.tsv)
cleanup() {
    rm -f -- "$metadata" "$report"
}
trap cleanup EXIT

cargo metadata --locked --offline --format-version 1 \
    --manifest-path "$repo_root/Cargo.toml" > "$metadata"
jq -r '
    .packages[]
    | select(.source != null)
    | [.name, .version, (.license // "MISSING"), .source]
    | @tsv
' "$metadata" | sort -u > "$report"

if rg -n $'\tMISSING\t' "$report"; then
    printf '%s\n' 'audit-licenses.sh: dependency without declared license' >&2
    exit 1
fi

allowed='^(Apache-2\.0|Apache-2\.0/MIT|Apache-2\.0 OR MIT|Apache-2\.0 WITH LLVM-exception|Apache-2\.0 WITH LLVM-exception OR Apache-2\.0 OR MIT|BSD-3-Clause|ISC|MIT|MIT/Apache-2\.0|MIT OR Apache-2\.0|\(MIT OR Apache-2\.0\) AND Unicode-3\.0|MIT OR Apache-2\.0 OR LGPL-2\.1-or-later|Unlicense OR MIT)$'
unexpected=$(cut -f3 "$report" | sort -u | rg -v "$allowed" || true)
if [[ -n $unexpected ]]; then
    printf '%s\n%s\n' \
        'audit-licenses.sh: unreviewed dependency license expression(s):' \
        "$unexpected" >&2
    exit 1
fi

if [[ -n $output ]]; then
    if [[ $output != /* ]]; then
        output=$PWD/$output
    fi
    mkdir -p -- "$(dirname -- "$output")"
    cp -- "$report" "$output"
fi

printf 'Dependency license audit passed for %s external Rust packages.\n' \
    "$(wc -l < "$report")"
printf '%s\n' 'The extension has no production JavaScript dependencies.'
