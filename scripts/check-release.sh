#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
release_dir=${1:-}
version=$("$repo_root/scripts/project-version.sh")

[[ -n $release_dir ]] || {
    printf '%s\n' 'Usage: scripts/check-release.sh RELEASE_DIRECTORY' >&2
    exit 2
}
if [[ $release_dir != /* ]]; then
    release_dir=$PWD/$release_dir
fi
[[ -d $release_dir ]] || {
    printf 'check-release.sh: directory does not exist: %s\n' "$release_dir" >&2
    exit 1
}

for command_name in cargo jq rpm sha256sum tar unzip; do
    command -v "$command_name" >/dev/null || {
        printf 'check-release.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

binary=$(find "$release_dir" -maxdepth 1 -type f -name "camera-monitor-$version-*" -print -quit)
extension=$release_dir/lensguard-extension-v$version.zip
source_archive=$release_dir/lensguard-$version-source.tar.gz
license_report=$release_dir/lensguard-$version-dependency-licenses.tsv
for artifact in "$binary" "$extension" "$source_archive" "$license_report" \
    "$release_dir/RELEASE-MANIFEST.txt" "$release_dir/SHA256SUMS"; do
    [[ -n $artifact && -s $artifact ]] || {
        printf 'check-release.sh: missing release artifact: %s\n' "$artifact" >&2
        exit 1
    }
done

[[ $("$binary" --version) == "camera-monitor $version" ]]

archive_version=$(unzip -p "$extension" metadata.json | jq -r '."version-name"')
[[ $archive_version == "$version" ]] || {
    printf 'check-release.sh: extension version is %s, expected %s\n' \
        "$archive_version" "$version" >&2
    exit 1
}
unzip -p "$extension" metadata.json | jq -e '."shell-version" == ["50"]' >/dev/null
extension_entries=$(unzip -Z1 "$extension")
if rg -q '(^|/)(tests?|fixtures|mocks?)/|mockDataProvider|gnomeSmoke|gnomeScreenshot' \
    <<<"$extension_entries"; then
    printf '%s\n' 'check-release.sh: extension contains development-only files' >&2
    exit 1
fi
if rg -q '(^|/)(camera-monitor|lensguard-v4l2-observer)$|\.(so([.]|$)|a|o|node|wasm|bin)$' \
    <<<"$extension_entries"; then
    printf '%s\n' 'check-release.sh: extension ZIP contains a native binary or library' >&2
    exit 1
fi

source_entries=$(tar -tzf "$source_archive")
for required in \
    "lensguard-$version/Cargo.toml" \
    "lensguard-$version/Cargo.lock" \
    "lensguard-$version/LICENSE" \
    "lensguard-$version/packaging/rpm/lensguard.spec.in"; do
    rg -Fxq "$required" <<<"$source_entries" || {
        printf 'check-release.sh: source archive is missing %s\n' "$required" >&2
        exit 1
    }
done
if rg -q '(^|/)(target|dist|node_modules|\.git)/' <<<"$source_entries"; then
    printf '%s\n' 'check-release.sh: source archive contains generated files' >&2
    exit 1
fi

metadata_versions=$(cargo metadata --locked --offline --format-version 1 \
    --manifest-path "$repo_root/Cargo.toml" |
    jq -r --arg root "$repo_root/" \
        '.packages[] | select(.manifest_path | startswith($root)) | .version' |
    sort -u)
[[ $metadata_versions == "$version" ]] || {
    printf 'check-release.sh: workspace package versions are not synchronized:\n%s\n' \
        "$metadata_versions" >&2
    exit 1
}
source_extension_version=$(jq -r '."version-name"' "$repo_root/extension/metadata.json")
[[ $source_extension_version == "$version" ]] || {
    printf 'check-release.sh: source extension version is %s, expected %s\n' \
        "$source_extension_version" "$version" >&2
    exit 1
}

(
    cd "$release_dir"
    sha256sum --check --strict SHA256SUMS
)

rpm_count=0
while IFS= read -r rpm_file; do
    [[ -n $rpm_file ]] || continue
    ((rpm_count += 1))
    [[ $(rpm -qp --queryformat '%{VERSION}' "$rpm_file") == "$version" ]]
    rpm -qpl "$rpm_file" | rg -Fxq '/usr/libexec/lensguard/camera-monitor'
    rpm -qpl "$rpm_file" | rg -Fxq '/usr/lib/systemd/user/camera-monitor.service'
    rpm -qpl "$rpm_file" | rg -Fxq \
        '/usr/share/dbus-1/services/io.github.younesrabeh.CameraMonitor.service'
    rpm -qpl "$rpm_file" | rg -Fxq \
        '/usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io/metadata.json'
done < <(find "$release_dir" -maxdepth 1 -type f -name "lensguard-$version-*.rpm" ! -name '*.src.rpm')

service_rpm_count=0
while IFS= read -r rpm_file; do
    [[ -n $rpm_file ]] || continue
    ((service_rpm_count += 1))
    [[ $(rpm -qp --queryformat '%{VERSION}' "$rpm_file") == "$version" ]]
    rpm -qpl "$rpm_file" | rg -Fxq '/usr/libexec/lensguard/camera-monitor'
    if rpm -qpl "$rpm_file" | rg -q '/usr/share/gnome-shell/extensions/'; then
        printf '%s\n' 'check-release.sh: service RPM contains GNOME extension files' >&2
        exit 1
    fi
done < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-service-$version-*.rpm" ! -name '*.src.rpm')

printf 'Release checks passed for LensGuard %s (%s full RPMs, %s service RPMs).\n' \
    "$version" "$rpm_count" "$service_rpm_count"
