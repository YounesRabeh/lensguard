#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
version=$("$repo_root/scripts/project-version.sh")

sed -i -E \
    "s|(^[[:space:]]*\"version-name\":[[:space:]]*\")[^\"]*(\".*)$|\\1$version\\2|" \
    "$repo_root/extension/metadata.json"
if [[ -f $repo_root/CHANGELOG.md ]]; then
    sed -i -E \
        "s|^(## \[)[0-9]+\.[0-9]+\.[0-9]+(\] - )|\\1$version\\2|" \
        "$repo_root/CHANGELOG.md"
fi

printf 'Synchronized extension metadata to LensGuard %s.\n' "$version"
