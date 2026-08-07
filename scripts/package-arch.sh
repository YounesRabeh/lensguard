#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output_dir=$repo_root/dist/packages/arch

for command_name in makepkg mktemp sed sha256sum tar; do
    command -v "$command_name" >/dev/null || {
        printf 'package-arch.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

version=$("$repo_root/scripts/project-version.sh")
[[ -n $version ]] || { printf '%s\n' 'package-arch.sh: could not read package version' >&2; exit 1; }

case $(uname -m) in
    x86_64|aarch64) architecture=$(uname -m) ;;
    *) printf 'package-arch.sh: unsupported architecture: %s\n' "$(uname -m)" >&2; exit 1 ;;
esac

work_dir=$(mktemp -d --tmpdir lensguard-arch.XXXXXX)
cleanup() {
    rm -rf -- "$work_dir"
}
trap cleanup EXIT

stage_root=$work_dir/root
"$repo_root/scripts/stage-system-package.sh" \
    --root "$stage_root" \
    --daemon-path /usr/lib/lensguard/camera-monitor
tar -C "$work_dir" -czf "$work_dir/lensguard-root.tar.gz" root
archive_sha256=$(sha256sum "$work_dir/lensguard-root.tar.gz" | awk '{ print $1 }')
sed \
    -e "s|@VERSION@|$version|g" \
    -e "s|@ARCHITECTURE@|$architecture|g" \
    -e "s|@SHA256@|$archive_sha256|g" \
    "$repo_root/packaging/arch/PKGBUILD.in" > "$work_dir/PKGBUILD"

(
    cd "$work_dir"
    makepkg --noconfirm --cleanbuild --nodeps
)
mkdir -p -- "$output_dir"
mapfile -t packages < <(find "$work_dir" -maxdepth 1 -type f \
    -name "lensguard-$version-*-$architecture.pkg.tar.*")
[[ ${#packages[@]} -eq 1 ]] || {
    printf 'package-arch.sh: expected one package, found %s\n' "${#packages[@]}" >&2
    exit 1
}
find "$output_dir" -maxdepth 1 -type f \
    \( -name "lensguard-$version-*.pkg.tar.*" \
    -o -name "lensguard-debug-$version-*.pkg.tar.*" \) -delete
install -m 0644 -- "${packages[0]}" "$output_dir/$(basename -- "${packages[0]}")"
install -m 0644 -- "$work_dir/PKGBUILD" "$output_dir/lensguard-$version-PKGBUILD"
printf '%s\n' "Created Arch package(s) in $output_dir"
