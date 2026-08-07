#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output_dir=$repo_root/dist/packages/rpm

for command_name in cargo git gzip install mktemp rpmbuild sed tar; do
    command -v "$command_name" >/dev/null || {
        printf 'package-rpm.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

version=$("$repo_root/scripts/project-version.sh")
[[ -n $version ]] || {
    printf '%s\n' 'package-rpm.sh: could not read package version' >&2
    exit 1
}

top_dir=$(mktemp -d --tmpdir lensguard-rpm.XXXXXX)
cleanup() {
    rm -rf -- "$top_dir"
}
trap cleanup EXIT

mkdir -p -- \
    "$top_dir/BUILD" \
    "$top_dir/BUILDROOT" \
    "$top_dir/RPMS" \
    "$top_dir/SOURCES" \
    "$top_dir/SPECS" \
    "$top_dir/SRPMS"

source_name=lensguard-$version
source_root=$top_dir/$source_name
source_files=$top_dir/source-files
mkdir -p -- "$source_root/.cargo"
while IFS= read -r -d '' source_path; do
    if [[ -e $repo_root/$source_path || -L $repo_root/$source_path ]]; then
        printf '%s\0' "$source_path"
    fi
done < <(git -C "$repo_root" ls-files --cached --others --exclude-standard -z) > "$source_files"
[[ -s $source_files ]] || {
    printf '%s\n' 'package-rpm.sh: no source files were selected for the RPM source archive' >&2
    exit 1
}
tar -C "$repo_root" -cf - --null --verbatim-files-from --files-from "$source_files" |
    tar -C "$source_root" -xf -

(
    cd "$source_root"
    cargo vendor --locked --versioned-dirs vendor > .cargo/config.toml
)

source_epoch=${SOURCE_DATE_EPOCH:-$(git -C "$repo_root" log -1 --format=%ct)}
tar \
    --directory "$top_dir" \
    --sort=name \
    --mtime="@$source_epoch" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    --create --file=- \
    "$source_name" | gzip -n > "$top_dir/SOURCES/$source_name.tar.gz"

rendered_spec=$top_dir/SPECS/lensguard.spec
sed "s|@VERSION@|$version|g" \
    "$repo_root/packaging/rpm/lensguard.spec.in" > "$rendered_spec"
rpmbuild -ba "$rendered_spec" --define "_topdir $top_dir"
mkdir -p -- "$output_dir"
find "$top_dir/RPMS" -type f -name '*.rpm' -exec cp -t "$output_dir" {} +
find "$top_dir/SRPMS" -type f -name '*.src.rpm' -exec cp -t "$output_dir" {} +
install -m 0644 -- "$rendered_spec" "$output_dir/lensguard-$version.spec"
printf '%s\n' "Created source-built RPM package(s) in $output_dir"
