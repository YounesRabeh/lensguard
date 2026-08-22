#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
output_dir=${1:-$repo_root/dist/packages/deb}

if (($# > 1)); then
    printf 'Usage: scripts/package/package-deb.sh [OUTPUT_DIRECTORY]\n' >&2
    exit 2
fi

if [[ $output_dir != /* ]]; then
    output_dir=$PWD/$output_dir
fi

for command_name in awk date dpkg-deb du git gzip install mktemp sed strip; do
    command -v "$command_name" >/dev/null || {
        printf 'package-deb.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

version=$("$repo_root/scripts/util/project-version.sh")
[[ -n $version ]] || { printf '%s\n' 'package-deb.sh: could not read package version' >&2; exit 1; }

case $(uname -m) in
    x86_64) architecture=amd64 ;;
    aarch64) architecture=arm64 ;;
    *) printf 'package-deb.sh: unsupported architecture: %s\n' "$(uname -m)" >&2; exit 1 ;;
esac

work_dir=$(mktemp -d --tmpdir lensguard-deb.XXXXXX)
cleanup() {
    rm -rf -- "$work_dir"
}
trap cleanup EXIT

source_epoch=${SOURCE_DATE_EPOCH:-$(git -C "$repo_root" log -1 --format=%ct)}
release_date=$(date --date="@$source_epoch" --rfc-email)
mkdir -p -- "$output_dir"

build_package() {
    local package_name=$1
    local control_template=$2
    local service_only=$3
    local package_work=$work_dir/$package_name
    local stage_root=$package_work/root
    local stage_args=(
        --root "$stage_root"
        --daemon-path /usr/lib/lensguard/camera-monitor
        --package-name "$package_name"
    )
    if [[ $service_only == true ]]; then
        stage_args+=(--service-only)
    fi

    "$repo_root/scripts/package/stage-system-package.sh" "${stage_args[@]}"
    strip --strip-unneeded "$stage_root/usr/lib/lensguard/camera-monitor"
    strip --strip-unneeded "$stage_root/usr/lib/lensguard/lensguard-v4l2-observer"
    install -d -m 0755 -- "$stage_root/DEBIAN"
    install -m 0755 -- "$repo_root/packaging/deb/postinst.in" "$stage_root/DEBIAN/postinst"
    install -m 0755 -- "$repo_root/packaging/deb/prerm.in" "$stage_root/DEBIAN/prerm"
    install -m 0755 -- "$repo_root/packaging/deb/postrm.in" "$stage_root/DEBIAN/postrm"
    install -m 0644 -- "$repo_root/packaging/deb/copyright.in" \
        "$stage_root/usr/share/doc/$package_name/copyright"
    printf '%s\n' \
        "$package_name ($version) stable; urgency=medium" \
        '' \
        "  * Release Lens Guard $version." \
        '' \
        " -- Younes Rabeh <younesrabeh@users.noreply.github.com>  $release_date" \
        > "$package_work/changelog"
    gzip -n -9 < "$package_work/changelog" \
        > "$stage_root/usr/share/doc/$package_name/changelog.gz"
    local installed_size
    installed_size=$(du -sk "$stage_root/usr" | awk '{print $1}')
    sed \
        -e "s|@VERSION@|$version|g" \
        -e "s|@ARCHITECTURE@|$architecture|g" \
        -e "s|@INSTALLED_SIZE@|$installed_size|g" \
        "$control_template" > "$stage_root/DEBIAN/control"

    local package_path=$output_dir/${package_name}_${version}_${architecture}.deb
    dpkg-deb --root-owner-group --build "$stage_root" "$package_path"
    printf '%s\n' "Created $package_path"
}

build_package lensguard "$repo_root/packaging/deb/control.in" false
build_package lensguard-service "$repo_root/packaging/deb/service-control.in" true
