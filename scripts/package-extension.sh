#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output_arg=${1:-dist}

if [[ $output_arg = /* ]]; then
    output_dir=$output_arg
else
    output_dir=$repo_root/$output_arg
fi

mkdir -p -- "$output_dir"
gnome-extensions pack \
    --force \
    --quiet \
    --extra-source="$repo_root/extension/icons" \
    --extra-source="$repo_root/extension/src" \
    --out-dir "$output_dir" \
    "$repo_root/extension"

archive=$output_dir/lensguard@younesrabeh.github.io.shell-extension.zip
if [[ ! -s $archive ]]; then
    printf '%s\n' "extension package was not created at $archive" >&2
    exit 1
fi

archive_entries=$(unzip -Z1 "$archive")
for required_file in \
    metadata.json \
    extension.js \
    icons/camera-active.png \
    src/dbusClient.js \
    src/dbusPayload.js \
    src/indicator.js \
    src/sessionModel.js; do
    if ! grep -Fxq "$required_file" <<<"$archive_entries"; then
        printf '%s\n' "extension package is missing $required_file" >&2
        exit 1
    fi
done

printf '%s\n' "Created $archive"
