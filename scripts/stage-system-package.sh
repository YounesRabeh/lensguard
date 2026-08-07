#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
uuid=lensguard@younesrabeh.github.io
bus_name=io.github.younesrabeh.CameraMonitor
interface_file=$bus_name'1.xml'
stage_root=
daemon_path=

usage() {
    printf '%s\n' \
        'Usage: scripts/stage-system-package.sh --root DIRECTORY --daemon-path PATH' \
        '' \
        'Builds the release daemon and stages LensGuard below DIRECTORY.' \
        'PATH must be the absolute installed daemon path, such as /usr/libexec/lensguard/camera-monitor.'
}

die() {
    printf 'stage-system-package.sh: %s\n' "$1" >&2
    exit 1
}

while (($#)); do
    case $1 in
        --root)
            (($# >= 2)) || die '--root requires a directory'
            stage_root=$2
            shift 2
            ;;
        --daemon-path)
            (($# >= 2)) || die '--daemon-path requires a path'
            daemon_path=$2
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1"
            ;;
    esac
done

[[ -n $stage_root ]] || die '--root is required'
[[ -n $daemon_path && $daemon_path = /* ]] || die '--daemon-path must be absolute'
for command_name in cargo gnome-extensions glib-compile-schemas install mktemp sed unzip; do
    command -v "$command_name" >/dev/null || die "required command not found: $command_name"
done

if [[ $stage_root != /* ]]; then
    stage_root=$PWD/$stage_root
fi
relative_daemon_path=${daemon_path#/}
daemon_destination=$stage_root/$relative_daemon_path
systemd_destination=$stage_root/usr/lib/systemd/user/camera-monitor.service
dbus_destination=$stage_root/usr/share/dbus-1/services/$bus_name.service
extension_destination=$stage_root/usr/share/gnome-shell/extensions/$uuid

cargo build --locked --release -p camera-monitor --manifest-path "$repo_root/Cargo.toml"
cargo_target=${CARGO_TARGET_DIR:-$repo_root/target}
if [[ $cargo_target != /* ]]; then
    cargo_target=$repo_root/$cargo_target
fi
daemon_artifact=$cargo_target/release/camera-monitor
[[ -x $daemon_artifact ]] || die "release daemon was not created: $daemon_artifact"

extension_work=$(mktemp -d --tmpdir lensguard-package-extension.XXXXXX)
cleanup() {
    rm -rf -- "$extension_work"
}
trap cleanup EXIT

"$repo_root/scripts/package-extension.sh" "$extension_work"
archive=$extension_work/$uuid.shell-extension.zip

escape_template_value() {
    local value=$1
    value=${value//\\/\\\\}
    value=${value//&/\\&}
    value=${value//|/\\|}
    value=${value//\"/\\\"}
    printf '%s' "$value"
}

escaped_daemon_path=$(escape_template_value "$daemon_path")
install -d -m 0755 -- \
    "$(dirname -- "$daemon_destination")" \
    "$(dirname -- "$systemd_destination")" \
    "$(dirname -- "$dbus_destination")" \
    "$extension_destination" \
    "$stage_root/usr/share/doc/lensguard" \
    "$stage_root/usr/share/licenses/lensguard"
install -m 0755 -- "$daemon_artifact" "$daemon_destination"
sed "s|@EXECUTABLE@|$escaped_daemon_path|g" \
    "$repo_root/systemd/camera-monitor.service.in" > "$systemd_destination"
sed "s|@EXECUTABLE@|$escaped_daemon_path|g" \
    "$repo_root/systemd/$bus_name.service.in" > "$dbus_destination"
unzip -q "$archive" -d "$extension_destination"
glib-compile-schemas --strict "$extension_destination/schemas"
install -m 0644 -- "$repo_root/README.md" "$stage_root/usr/share/doc/lensguard/README.md"
install -m 0644 -- "$repo_root/docs/installation.md" \
    "$stage_root/usr/share/doc/lensguard/installation.md"
install -m 0644 -- "$repo_root/dbus/$interface_file" \
    "$stage_root/usr/share/doc/lensguard/$interface_file"
install -m 0644 -- "$repo_root/LICENSE" "$stage_root/usr/share/licenses/lensguard/LICENSE"

printf '%s\n' "Staged LensGuard system package root at $stage_root"
