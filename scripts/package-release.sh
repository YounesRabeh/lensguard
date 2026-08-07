#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
version=$("$repo_root/scripts/project-version.sh")
release_tag=v$version
output_dir=${1:-$repo_root/dist/release/$version}
require_rpm=false
skip_rpm=false

if [[ ${2:-} == --require-rpm ]]; then
    require_rpm=true
elif [[ ${2:-} == --skip-rpm ]]; then
    skip_rpm=true
elif (($# > 1)); then
    printf 'Usage: scripts/package-release.sh [OUTPUT_DIRECTORY] [--require-rpm|--skip-rpm]\n' >&2
    exit 2
fi

if [[ $output_dir != /* ]]; then
    output_dir=$PWD/$output_dir
fi
mkdir -p -- "$output_dir"

for command_name in cargo git gzip sha256sum tar unzip; do
    command -v "$command_name" >/dev/null || {
        printf 'package-release.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

work_dir=$(mktemp -d --tmpdir lensguard-release.XXXXXX)
cleanup() {
    rm -rf -- "$work_dir"
}
trap cleanup EXIT

extension_dir=$work_dir/extension
"$repo_root/scripts/package-extension.sh" "$extension_dir"
cp -- "$extension_dir/lensguard@younesrabeh.github.io.shell-extension.zip" \
    "$output_dir/lensguard-extension-$release_tag.zip"

cargo build --locked --release -p camera-monitor --manifest-path "$repo_root/Cargo.toml"
cargo_target=${CARGO_TARGET_DIR:-$repo_root/target}
if [[ $cargo_target != /* ]]; then
    cargo_target=$repo_root/$cargo_target
fi
architecture=$(uname -m)
install -m 0755 -- "$cargo_target/release/camera-monitor" \
    "$output_dir/camera-monitor-$version-$architecture"

source_epoch=${SOURCE_DATE_EPOCH:-$(git -C "$repo_root" log -1 --format=%ct)}
source_list=$work_dir/source-files
while IFS= read -r -d '' source_path; do
    if [[ -e $repo_root/$source_path || -L $repo_root/$source_path ]]; then
        printf '%s\0' "$source_path"
    fi
done < <(git -C "$repo_root" ls-files --cached --others --exclude-standard -z) > "$source_list"
source_archive=$output_dir/lensguard-$version-source.tar.gz
tar \
    --directory "$repo_root" \
    --null \
    --verbatim-files-from \
    --files-from "$source_list" \
    --sort=name \
    --mtime="@$source_epoch" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    --transform="s,^,lensguard-$version/," \
    --create --file=- | gzip -n > "$source_archive"

"$repo_root/scripts/audit-licenses.sh" \
    "$output_dir/lensguard-$version-dependency-licenses.tsv"

if $skip_rpm; then
    printf '%s\n' 'RPM build intentionally skipped; native release workflow supplies it separately.'
elif command -v rpmbuild >/dev/null; then
    "$repo_root/scripts/package-rpm.sh"
    find "$repo_root/dist/packages/rpm" -maxdepth 1 -type f \
        \( -name "lensguard-$version-*.rpm" \
        -o -name "lensguard-$version-*.src.rpm" \
        -o -name "lensguard-debuginfo-$version-*.rpm" \
        -o -name "lensguard-debugsource-$version-*.rpm" \
        -o -name "lensguard-$version.spec" \) \
        -exec cp -- {} "$output_dir/" \;
    if $require_rpm &&
        ! find "$output_dir" -maxdepth 1 -type f \
            -name "lensguard-$version-*.rpm" ! -name '*.src.rpm' | rg -q .; then
        printf '%s\n' 'package-release.sh: RPM build produced no binary package' >&2
        exit 1
    fi
elif $require_rpm; then
    printf '%s\n' 'package-release.sh: rpmbuild is required for this release build' >&2
    exit 1
else
    printf '%s\n' 'rpmbuild is unavailable; release directory will not contain an RPM' >&2
fi

cat > "$work_dir/manifest" <<EOF
LensGuard release candidate $version
Architecture: $architecture
Supported GNOME Shell: 50
Packaging: one system package contains the daemon and extension; the extension ZIP is also shipped for testing.
EOF
install -m 0644 -- "$work_dir/manifest" "$output_dir/RELEASE-MANIFEST.txt"

(
    cd "$output_dir"
    find . -maxdepth 1 -type f ! -name SHA256SUMS -printf '%P\n' |
        sort |
        xargs -r sha256sum > "$work_dir/SHA256SUMS"
)
install -m 0644 -- "$work_dir/SHA256SUMS" "$output_dir/SHA256SUMS"

"$repo_root/scripts/check-release.sh" "$output_dir"
printf 'Created LensGuard %s release candidate in %s\n' "$version" "$output_dir"
