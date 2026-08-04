#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
uuid=lensguard@younesrabeh.github.io
bus_name=io.github.younesrabeh.CameraMonitor
unit_name=camera-monitor.service
ownership_marker=lensguard-local-install-v1
artifact=
use_user_manager=true

usage() {
    printf '%s\n' \
        'Usage: scripts/install-local.sh [--artifact PATH] [--no-user-manager]' \
        '' \
        'Builds and installs LensGuard for the current user without root.' \
        '--artifact PATH      Install an existing camera-monitor binary instead of building release.' \
        '--no-user-manager    Stage files without calling the live user manager (tests/packaging).'
}

die() {
    printf 'install-local.sh: %s\n' "$1" >&2
    exit 1
}

while (($#)); do
    case $1 in
        --artifact)
            (($# >= 2)) || die '--artifact requires a path'
            artifact=$2
            shift 2
            ;;
        --no-user-manager)
            use_user_manager=false
            shift
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

[[ -n ${HOME:-} && $HOME = /* ]] || die 'HOME must be an absolute path'
for command_name in cargo glib-compile-schemas install mktemp sed unzip; do
    command -v "$command_name" >/dev/null || die "required command not found: $command_name"
done
if $use_user_manager; then
    command -v systemctl >/dev/null || die 'systemctl is required for a live local install'
fi

if [[ -z $artifact ]]; then
    cargo build --locked --release -p camera-monitor --manifest-path "$repo_root/Cargo.toml"
    cargo_target=${CARGO_TARGET_DIR:-$repo_root/target}
    if [[ $cargo_target != /* ]]; then
        cargo_target=$repo_root/$cargo_target
    fi
    artifact=$cargo_target/release/camera-monitor
elif [[ $artifact != /* ]]; then
    artifact=$PWD/$artifact
fi
[[ -x $artifact ]] || die "daemon artifact is not executable: $artifact"

data_home=${XDG_DATA_HOME:-$HOME/.local/share}
config_home=${XDG_CONFIG_HOME:-$HOME/.config}
libexec_dir=$HOME/.local/libexec/lensguard
executable=$libexec_dir/camera-monitor
systemd_dir=$config_home/systemd/user
dbus_service_dir=$data_home/dbus-1/services
extension_parent=$data_home/gnome-shell/extensions
extension_dir=$extension_parent/$uuid

stage=$(mktemp -d --tmpdir lensguard-install.XXXXXX)
cleanup() {
    rm -rf -- "$stage"
}
trap cleanup EXIT

"$repo_root/scripts/package-extension.sh" "$stage/package"
archive=$stage/package/$uuid.shell-extension.zip
mkdir -p -- "$stage/extension"
unzip -q "$archive" -d "$stage/extension"
glib-compile-schemas --strict "$stage/extension/schemas"
printf '%s\n' "$ownership_marker" > "$stage/extension/.lensguard-owned"

escape_template_value() {
    local value=$1
    value=${value//\\/\\\\}
    value=${value//&/\\&}
    value=${value//|/\\|}
    value=${value//\"/\\\"}
    printf '%s' "$value"
}

escaped_executable=$(escape_template_value "$executable")
sed "s|@EXECUTABLE@|$escaped_executable|g" \
    "$repo_root/systemd/$unit_name.in" > "$stage/$unit_name"
sed "s|@EXECUTABLE@|$escaped_executable|g" \
    "$repo_root/systemd/$bus_name.service.in" > "$stage/$bus_name.service"

install -d -m 0755 -- "$libexec_dir" "$systemd_dir" "$dbus_service_dir" "$extension_parent"
install -m 0755 -- "$artifact" "$stage/camera-monitor"
mv -f -- "$stage/camera-monitor" "$executable"
printf '%s\n' "$ownership_marker" > "$libexec_dir/.lensguard-owned"
install -m 0644 -- "$stage/$unit_name" "$systemd_dir/$unit_name"
install -m 0644 -- "$stage/$bus_name.service" "$dbus_service_dir/$bus_name.service"

if [[ -L $extension_dir ]]; then
    die "refusing to replace symbolic-link extension directory: $extension_dir"
fi
rm -rf -- "$extension_dir"
mv -- "$stage/extension" "$extension_dir"

if $use_user_manager; then
    systemctl --user daemon-reload
    if command -v gdbus >/dev/null; then
        gdbus call --session \
            --dest org.freedesktop.DBus \
            --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.ReloadConfig >/dev/null 2>&1 || true
    fi
    systemctl --user reset-failed "$unit_name" >/dev/null 2>&1 || true
    systemctl --user try-restart "$unit_name"
fi

printf '%s\n' \
    "Installed LensGuard daemon: $executable" \
    "Installed user unit: $systemd_dir/$unit_name" \
    "Installed D-Bus activation: $dbus_service_dir/$bus_name.service" \
    "Installed extension: $extension_dir" \
    '' \
    "If it was not already active, the daemon remains stopped until $bus_name is requested." \
    "Enable the extension with: gnome-extensions enable $uuid"
