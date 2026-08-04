#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

if [[ ! -x "$repo_root/node_modules/.bin/eslint" ]]; then
    printf '%s\n' 'JavaScript dependencies are missing; run pnpm install.' >&2
    exit 1
fi

"$repo_root/node_modules/.bin/eslint" "$repo_root/extension" "$repo_root/eslint.config.js"

while IFS= read -r module; do
    node --check "$repo_root/$module"
done < <(cd "$repo_root" && rg --files extension -g '*.js')

gjs -m "$repo_root/extension/tests/validateMetadata.js" "$repo_root/extension/metadata.json"
gjs -m "$repo_root/extension/tests/sessionModel.test.js"
gjs -m "$repo_root/extension/tests/mockDataProvider.test.js"
gjs -m "$repo_root/extension/tests/dbusPayload.test.js"
gjs -m "$repo_root/extension/tests/dbusClient.test.js"

printf '%s\n' 'GNOME extension JavaScript and metadata checks passed.'
