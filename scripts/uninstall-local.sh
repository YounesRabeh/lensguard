#!/usr/bin/env bash
set -euo pipefail

uuid=lensguard@younesrabeh.github.io
bus_name=io.github.younesrabeh.CameraMonitor
unit_name=camera-monitor.service
ownership_marker=lensguard-local-install-v1
use_user_manager=true

usage() {
    printf '%s\n' \
        'Usage: scripts/uninstall-local.sh [--no-user-manager]' \
        '' \
        'Removes only files owned by the LensGuard per-user installer.' \
        '--no-user-manager    Remove staged files without calling the live user manager.'
}

while (($#)); do
    case $1 in
        --no-user-manager)
            use_user_manager=false
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            printf 'uninstall-local.sh: unknown argument: %s\n' "$1" >&2
            exit 1
            ;;
    esac
done

[[ -n ${HOME:-} && $HOME = /* ]] || {
    printf '%s\n' 'uninstall-local.sh: HOME must be an absolute path' >&2
    exit 1
}

data_home=${XDG_DATA_HOME:-$HOME/.local/share}
config_home=${XDG_CONFIG_HOME:-$HOME/.config}
libexec_dir=$HOME/.local/libexec/lensguard
systemd_dir=$config_home/systemd/user
dbus_service_dir=$data_home/dbus-1/services
extension_dir=$data_home/gnome-shell/extensions/$uuid

if $use_user_manager && command -v systemctl >/dev/null; then
    systemctl --user stop "$unit_name" >/dev/null 2>&1 || true
fi
if $use_user_manager && command -v gnome-extensions >/dev/null; then
    gnome-extensions disable "$uuid" >/dev/null 2>&1 || true
fi

rm -f -- \
    "$systemd_dir/$unit_name" \
    "$dbus_service_dir/$bus_name.service"

if [[ -f $libexec_dir/.lensguard-owned ]] &&
    [[ $(<"$libexec_dir/.lensguard-owned") == "$ownership_marker" ]]; then
    rm -f -- "$libexec_dir/camera-monitor" "$libexec_dir/.lensguard-owned"
    rmdir -- "$libexec_dir" 2>/dev/null || true
fi

if [[ -f $extension_dir/.lensguard-owned ]] &&
    [[ $(<"$extension_dir/.lensguard-owned") == "$ownership_marker" ]]; then
    rm -rf -- "$extension_dir"
fi

if $use_user_manager && command -v systemctl >/dev/null; then
    systemctl --user daemon-reload
    if command -v gdbus >/dev/null; then
        gdbus call --session \
            --dest org.freedesktop.DBus \
            --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.ReloadConfig >/dev/null 2>&1 || true
    fi
    systemctl --user reset-failed "$unit_name" >/dev/null 2>&1 || true
fi

rmdir -- "$systemd_dir" "$config_home/systemd" 2>/dev/null || true
rmdir -- "$dbus_service_dir" "$data_home/dbus-1" 2>/dev/null || true

printf '%s\n' 'LensGuard local per-user installation removed.'
