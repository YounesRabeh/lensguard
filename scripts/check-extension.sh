#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

gjs -m "$repo_root/extension/tests/validateMetadata.js" "$repo_root/extension/metadata.json"
gjs -m "$repo_root/extension/extension.js"

printf '%s\n' 'GNOME extension metadata and module syntax are valid.'
