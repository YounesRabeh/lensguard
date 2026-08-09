#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
uuid=lensguard@younesrabeh.github.io
bus_name=io.github.younesrabeh.CameraMonitor
interface_file=$bus_name'1.xml'
stage_root=
daemon_path=
include_extension=true
package_name=lensguard

usage() {
    printf '%s\n' \
        'Usage: scripts/package/stage-system-package.sh --root DIRECTORY --daemon-path PATH [--service-only] [--package-name NAME]' \
        '' \
        'Builds the release daemon and stages LensGuard below DIRECTORY.' \
        '--service-only omits the GNOME extension for the Store-extension installation path.' \
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
        --service-only)
            include_extension=false
            shift
            ;;
        --package-name)
            (($# >= 2)) || die '--package-name requires a value'
            package_name=$2
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
[[ $package_name =~ ^[a-z0-9][a-z0-9+.-]*$ ]] ||
    die '--package-name must be a lowercase package identifier'
for command_name in cargo install mktemp sed; do
    command -v "$command_name" >/dev/null || die "required command not found: $command_name"
done

if [[ $stage_root != /* ]]; then
    stage_root=$PWD/$stage_root
fi
relative_daemon_path=${daemon_path#/}
daemon_destination=$stage_root/$relative_daemon_path
systemd_destination=$stage_root/usr/lib/systemd/user/camera-monitor.service
dbus_destination=$stage_root/usr/share/dbus-1/services/$bus_name.service

cargo build --locked --release -p camera-monitor --manifest-path "$repo_root/Cargo.toml"
cargo_target=${CARGO_TARGET_DIR:-$repo_root/target}
if [[ $cargo_target != /* ]]; then
    cargo_target=$repo_root/$cargo_target
fi
daemon_artifact=$cargo_target/release/camera-monitor
[[ -x $daemon_artifact ]] || die "release daemon was not created: $daemon_artifact"

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
    "$stage_root/usr/share/doc/$package_name" \
    "$stage_root/usr/share/licenses/$package_name"
install -m 0755 -- "$daemon_artifact" "$daemon_destination"
sed "s|@EXECUTABLE@|$escaped_daemon_path|g" \
    "$repo_root/systemd/camera-monitor.service.in" > "$systemd_destination"
sed "s|@EXECUTABLE@|$escaped_daemon_path|g" \
    "$repo_root/systemd/$bus_name.service.in" > "$dbus_destination"
if $include_extension; then
    for command_name in gnome-extensions glib-compile-schemas unzip; do
        command -v "$command_name" >/dev/null || die "required command not found: $command_name"
    done
    extension_destination=$stage_root/usr/share/gnome-shell/extensions/$uuid
    extension_work=$(mktemp -d --tmpdir lensguard-package-extension.XXXXXX)
    cleanup() {
        rm -rf -- "$extension_work"
    }
    trap cleanup EXIT
    "$repo_root/scripts/package/package-extension.sh" "$extension_work"
    archive=$extension_work/$uuid.shell-extension.zip
    install -d -m 0755 -- "$extension_destination"
    unzip -q "$archive" -d "$extension_destination"
    glib-compile-schemas --strict "$extension_destination/schemas"
fi
install -m 0644 -- "$repo_root/README.md" "$stage_root/usr/share/doc/$package_name/README.md"
install -m 0644 -- "$repo_root/docs/installation.md" \
    "$stage_root/usr/share/doc/$package_name/installation.md"
install -m 0644 -- "$repo_root/dbus/$interface_file" \
    "$stage_root/usr/share/doc/$package_name/$interface_file"
install -m 0644 -- "$repo_root/LICENSE" "$stage_root/usr/share/licenses/$package_name/LICENSE"

printf '%s\n' "Staged LensGuard system package root at $stage_root"
