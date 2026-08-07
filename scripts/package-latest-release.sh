#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output_dir=${1:-$repo_root/dist/release-artifacts}

if (($# > 1)); then
    printf 'Usage: scripts/package-latest-release.sh [OUTPUT_DIRECTORY]\n' >&2
    exit 2
fi

if [[ $output_dir != /* ]]; then
    output_dir=$PWD/$output_dir
fi

work_dir=$(mktemp -d --tmpdir lensguard-latest-release.XXXXXX)
cleanup() {
    rm -rf -- "$work_dir"
}
trap cleanup EXIT

"$repo_root/scripts/sync-version.sh"
"$repo_root/scripts/package-release.sh" "$work_dir/release-artifacts" --skip-rpm
"$repo_root/scripts/check-release.sh" "$work_dir/release-artifacts"

rm -rf -- "$output_dir"
mkdir -p -- "$(dirname -- "$output_dir")"
mv -- "$work_dir/release-artifacts" "$output_dir"

printf 'Created verified LensGuard %s release artifacts in %s\n' \
    "$("$repo_root/scripts/project-version.sh")" "$output_dir"
