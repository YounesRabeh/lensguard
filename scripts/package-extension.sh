#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output_arg=${1:-dist}

workspace_version=$("$repo_root/scripts/project-version.sh")
if [[ -z $workspace_version ]]; then
    printf '%s\n' 'could not read [workspace.package] version from Cargo.toml' >&2
    exit 1
fi

if [[ $output_arg = /* ]]; then
    output_dir=$output_arg
else
    output_dir=$repo_root/$output_arg
fi

mkdir -p -- "$output_dir"
package_source=$(mktemp -d --tmpdir lensguard-extension-source.XXXXXX)
cleanup() {
    rm -rf -- "$package_source"
}
trap cleanup EXIT
cp -a -- "$repo_root/extension/." "$package_source/"
sed -i -E \
    "s|(\"version-name\":\s*\")[^\"]*(\")|\1$workspace_version\2|" \
    "$package_source/metadata.json"
gnome-extensions pack \
    --force \
    --quiet \
    --extra-source="$package_source/icons" \
    --extra-source="$package_source/src" \
    --out-dir "$output_dir" \
    "$package_source"

archive=$output_dir/lensguard@younesrabeh.github.io.shell-extension.zip
if [[ ! -s $archive ]]; then
    printf '%s\n' "extension package was not created at $archive" >&2
    exit 1
fi

archive_version=$(unzip -p "$archive" metadata.json |
    sed -nE 's/.*"version-name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p')
if [[ $archive_version != "$workspace_version" ]]; then
    printf '%s\n' \
        "extension version mismatch: archive=$archive_version workspace=$workspace_version" >&2
    exit 1
fi

archive_entries=$(unzip -Z1 "$archive")
for required_file in \
    metadata.json \
    extension.js \
    prefs.js \
    icons/camera-active.png \
    schemas/org.gnome.shell.extensions.lensguard.gschema.xml \
    src/dbusClient.js \
    src/dbusPayload.js \
    src/indicator.js \
    src/preferences.js \
    src/sessionModel.js; do
    if ! grep -Fxq "$required_file" <<<"$archive_entries"; then
        printf '%s\n' "extension package is missing $required_file" >&2
        exit 1
    fi
done

if grep -Eq '(^|/)(tests?|fixtures|mocks?)/|mockDataProvider|gnomeSmoke|gnomeScreenshot' \
    <<<"$archive_entries"; then
    printf '%s\n' 'extension package contains development-only files' >&2
    exit 1
fi

printf '%s\n' "Created $archive"
