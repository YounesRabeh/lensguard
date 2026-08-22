#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
dist_root=$repo_root/dist
package_root=$dist_root/packages
release_root=$dist_root/release
latest_root=$dist_root/release-artifacts

if (($# != 0)); then
    printf 'Usage: scripts/release/package-all.sh\n' >&2
    exit 2
fi

die() {
    printf 'package-all.sh: %s\n' "$1" >&2
    exit 1
}

for command_name in cargo cp find mkdir mktemp mv rm sha256sum sort tar xargs; do
    command -v "$command_name" >/dev/null ||
        die "required command not found: $command_name"
done

mkdir -p -- "$dist_root"
stage_root=$(mktemp -d --tmpdir="$dist_root" .lensguard-package-all.XXXXXX)
cleanup() {
    rm -rf -- "$stage_root"
}
trap cleanup EXIT

"$repo_root/scripts/util/sync-version.sh"
version=$("$repo_root/scripts/util/project-version.sh")
[[ -n $version ]] || die 'could not read the project version'

if ! cargo metadata \
    --manifest-path "$repo_root/Cargo.toml" \
    --format-version 1 \
    --locked \
    --offline \
    --no-deps >/dev/null; then
    die 'Cargo.lock is not synchronized; update it before packaging'
fi

stage_packages=$stage_root/packages
stage_candidate=$stage_root/release/$version
stage_latest=$stage_root/release-artifacts
license_report=$stage_candidate/lensguard-$version-dependency-licenses.tsv
mkdir -p -- \
    "$stage_packages/deb" \
    "$stage_packages/rpm" \
    "$stage_packages/arch" \
    "$stage_candidate"

"$repo_root/scripts/release/audit-licenses.sh" "$license_report"

build_arch_packages() {
    local container_runtime=
    local arch_image=

    if command -v makepkg >/dev/null; then
        "$repo_root/scripts/package/package-arch.sh" "$stage_packages/arch"
        return
    fi

    for candidate in podman docker; do
        if command -v "$candidate" >/dev/null; then
            container_runtime=$candidate
            break
        fi
    done
    [[ -n $container_runtime ]] ||
        die 'makepkg is unavailable and neither Podman nor Docker was found'

    arch_image='docker.io/library/archlinux:base-devel@sha256:c1829f370be8434135f43fb3acaef1256780804ac3b2d2eec90dfb1232e1ffdf'
    printf 'makepkg is unavailable; building Arch packages with %s.\n' "$container_runtime"

    "$container_runtime" run --rm \
        --security-opt label=disable \
        --volume "$repo_root:/src:ro" \
        "$arch_image" \
        bash -lc '
            set -euo pipefail
            pacman --sync --refresh --sysupgrade --noconfirm --needed \
                cargo rust clang pkgconf pipewire dbus gjs gnome-shell glib2 unzip git >&2
            useradd --create-home builder
            mkdir -p /build/lensguard
            cp -a /src/. /build/lensguard/
            chown -R builder:builder /build/lensguard
            runuser --user builder -- \
                /build/lensguard/scripts/package/package-arch.sh \
                /build/lensguard/dist/packages/arch >&2
            tar -C /build/lensguard/dist/packages/arch -cf - .
        ' | tar -C "$stage_packages/arch" -xf -
}

"$repo_root/scripts/package/package-deb.sh" "$stage_packages/deb"
"$repo_root/scripts/package/package-rpm.sh" "$stage_packages/rpm"
build_arch_packages

"$repo_root/scripts/release/package-release.sh" \
    "$stage_candidate" \
    --skip-rpm \
    --skip-license-audit
for format in deb rpm arch; do
    find "$stage_packages/$format" -maxdepth 1 -type f \
        -exec cp -- {} "$stage_candidate/" \;
done

checksum_file=$stage_root/SHA256SUMS
(
    cd "$stage_candidate"
    find . -maxdepth 1 -type f ! -name SHA256SUMS -printf '%P\n' |
        sort |
        xargs -r sha256sum
) > "$checksum_file"
mv -- "$checksum_file" "$stage_candidate/SHA256SUMS"

"$repo_root/scripts/release/check-release.sh" "$stage_candidate"
"$repo_root/scripts/release/check-package-artifacts.sh" "$stage_candidate"
cp -a -- "$stage_candidate" "$stage_latest"

replace_directory() {
    local source=$1
    local destination=$2

    [[ ! -L $destination ]] ||
        die "refusing to replace symbolic-link directory: $destination"
    if ! rm -rf -- "$destination"; then
        die "cannot replace $destination; check its ownership and permissions"
    fi
    mkdir -p -- "$(dirname -- "$destination")"
    mv -- "$source" "$destination"
}

preflight_destination() {
    local destination=$1
    local parent

    parent=$(dirname -- "$destination")
    mkdir -p -- "$parent"
    [[ ! -L $destination ]] ||
        die "refusing to replace symbolic-link directory: $destination"
    [[ -w $parent ]] ||
        die "cannot replace $destination; parent directory is not writable"
}

for destination in \
    "$package_root/deb" \
    "$package_root/rpm" \
    "$package_root/arch" \
    "$latest_root" \
    "$release_root/$version"; do
    preflight_destination "$destination"
done

replace_directory "$stage_packages/deb" "$package_root/deb"
replace_directory "$stage_packages/rpm" "$package_root/rpm"
replace_directory "$stage_packages/arch" "$package_root/arch"
replace_directory "$stage_latest" "$latest_root"
replace_directory "$stage_candidate" "$release_root/$version"

printf 'Created fresh LensGuard %s packages and release artifacts.\n' "$version"
printf 'Package directories: %s/{deb,rpm,arch}\n' "$package_root"
printf 'Latest artifacts: %s\n' "$latest_root"
printf 'Versioned candidate: %s/%s\n' "$release_root" "$version"
