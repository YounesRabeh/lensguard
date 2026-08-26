#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
archive=${1:-$repo_root/dist/lensguard@younesrabeh.github.io.shell-extension.zip}
workspace_version=$("$repo_root/scripts/util/project-version.sh")

if [[ ! -s $archive ]]; then
    printf '%s\n' "extension archive not found: $archive" >&2
    exit 1
fi

command -v unzip >/dev/null 2>&1 || {
    printf '%s\n' 'unzip is required for the extension archive smoke test.' >&2
    exit 1
}

unzip -tq -- "$archive"
entries=$(unzip -Z1 -- "$archive")
metadata=$(unzip -p -- "$archive" metadata.json)

grep -Fq '"uuid": "lensguard@younesrabeh.github.io"' <<<"$metadata" || {
    printf '%s\n' 'extension archive has an unexpected UUID.' >&2
    exit 1
}
grep -Fq "\"version-name\": \"$workspace_version\"" <<<"$metadata" || {
    printf '%s\n' "extension archive version is not $workspace_version." >&2
    exit 1
}

for required_file in \
    metadata.json extension.js prefs.js \
    schemas/org.gnome.shell.extensions.lensguard.gschema.xml \
    src/dbusClient.js src/indicator.js src/preferences.js src/sessionModel.js; do
    grep -Fxq "$required_file" <<<"$entries" || {
        printf '%s\n' "extension archive is missing $required_file" >&2
        exit 1
    }
done

if grep -Eq '(^|/)(tests?|fixtures|mocks?)/|gnomeSmoke|gnomeRealCameraSmoke' <<<"$entries";
then
    printf '%s\n' 'extension archive contains smoke-test or fixture files.' >&2
    exit 1
fi

if grep -Eq '(^|/)(camera-monitor|lensguard-v4l2-observer)$|\.(so|a|o|node|wasm|bin)$' <<<"$entries";
then
    printf '%s\n' 'extension archive contains a native binary or library.' >&2
    exit 1
fi

extract_dir=$(mktemp -d --tmpdir lensguard-extension-smoke.XXXXXX)
cleanup() { rm -rf -- "$extract_dir"; }
trap cleanup EXIT
unzip -qq -- "$archive" -d "$extract_dir"

while IFS= read -r module; do
    node --check "$extract_dir/$module"
done < <(cd "$extract_dir" && find . -type f -name '*.js' -printf '%P\n')

schema_dir="$extract_dir/schemas"
glib-compile-schemas --strict "$schema_dir"

printf '%s\n' 'Extension archive smoke test passed.'
