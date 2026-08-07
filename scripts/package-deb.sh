#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output_dir=$repo_root/dist/packages/deb

for command_name in awk date dpkg-deb du git gzip install mktemp sed; do
    command -v "$command_name" >/dev/null || {
        printf 'package-deb.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

version=$("$repo_root/scripts/project-version.sh")
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

stage_root=$work_dir/root
"$repo_root/scripts/stage-system-package.sh" \
    --root "$stage_root" \
    --daemon-path /usr/lib/lensguard/camera-monitor
install -d -m 0755 -- "$stage_root/DEBIAN"
install -m 0644 -- "$repo_root/LICENSE" "$stage_root/usr/share/doc/lensguard/copyright"
source_epoch=${SOURCE_DATE_EPOCH:-$(git -C "$repo_root" log -1 --format=%ct)}
release_date=$(date --date="@$source_epoch" --rfc-email)
printf '%s\n' \
    "lensguard ($version) stable; urgency=medium" \
    '' \
    "  * Release LensGuard $version." \
    '' \
    " -- Younes Rabeh <younesrabeh@users.noreply.github.com>  $release_date" \
    > "$work_dir/changelog.Debian"
gzip -n -9 < "$work_dir/changelog.Debian" \
    > "$stage_root/usr/share/doc/lensguard/changelog.Debian.gz"
installed_size=$(du -sk "$stage_root/usr" | awk '{print $1}')
sed \
    -e "s|@VERSION@|$version|g" \
    -e "s|@ARCHITECTURE@|$architecture|g" \
    -e "s|@INSTALLED_SIZE@|$installed_size|g" \
    "$repo_root/packaging/deb/control.in" > "$stage_root/DEBIAN/control"

mkdir -p -- "$output_dir"
package_path=$output_dir/lensguard_${version}_${architecture}.deb
dpkg-deb --root-owner-group --build "$stage_root" "$package_path"
printf '%s\n' "Created $package_path"
