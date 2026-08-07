#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
artifact=${1:-$repo_root/target/debug/camera-monitor}
uuid=lensguard@younesrabeh.github.io
bus_name=io.github.younesrabeh.CameraMonitor
object_path=/io/github/younesrabeh/CameraMonitor
interface_name=io.github.younesrabeh.CameraMonitor1

for command_name in dbus-run-session gdbus glib-compile-schemas gsettings sha256sum timeout; do
    command -v "$command_name" >/dev/null || {
        printf 'test-local-installation.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done
[[ -x $artifact ]] || {
    printf 'test-local-installation.sh: artifact is not executable: %s\n' "$artifact" >&2
    exit 1
}

test_root=$(mktemp -d --tmpdir lensguard-install-test.XXXXXX)
cleanup() {
    rm -rf -- "$test_root"
}
trap cleanup EXIT

test_home=$test_root/home
data_home=$test_root/data
config_home=$test_root/config
mkdir -p -- "$test_home" "$data_home" "$config_home"
printf '%s\n' 'must survive LensGuard uninstall' > "$data_home/unrelated-sentinel"

run_installer() {
    env \
        HOME="$test_home" \
        XDG_DATA_HOME="$data_home" \
        XDG_CONFIG_HOME="$config_home" \
        "$repo_root/scripts/install-local.sh" \
        --artifact "$artifact" \
        --no-user-manager
}

run_uninstaller() {
    env \
        HOME="$test_home" \
        XDG_DATA_HOME="$data_home" \
        XDG_CONFIG_HOME="$config_home" \
        "$repo_root/scripts/uninstall-local.sh" \
        --no-user-manager
}

run_installer >/dev/null

installed_binary=$test_home/.local/libexec/lensguard/camera-monitor
installed_unit=$config_home/systemd/user/camera-monitor.service
installed_activation=$data_home/dbus-1/services/$bus_name.service
installed_extension=$data_home/gnome-shell/extensions/$uuid
installed_unit_marker=$installed_unit.lensguard-owned
installed_activation_marker=$installed_activation.lensguard-owned
[[ -x $installed_binary ]]
[[ -f $installed_unit ]]
[[ -f $installed_unit_marker ]]
[[ -f $installed_activation ]]
[[ -f $installed_activation_marker ]]
[[ -f $installed_extension/schemas/gschemas.compiled ]]
[[ -f $installed_extension/.lensguard-owned ]]
version=$("$repo_root/scripts/project-version.sh")
[[ $("$installed_binary" --version) == "camera-monitor $version" ]]
grep -Fq '"version-name": "'"$version"'"' "$installed_extension/metadata.json"
grep -Fq 'Type=dbus' "$installed_unit"
grep -Fq 'BusName=io.github.younesrabeh.CameraMonitor' "$installed_unit"
grep -Fq 'Restart=on-failure' "$installed_unit"
grep -Fq 'SystemdService=camera-monitor.service' "$installed_activation"
if grep -RqE '(^|[[:space:]])(sudo|pkexec)([[:space:]]|$)' \
    "$repo_root/scripts/install-local.sh" "$repo_root/scripts/uninstall-local.sh"; then
    printf '%s\n' 'local integration scripts must not request root privileges' >&2
    exit 1
fi

env \
    HOME="$test_home" \
    XDG_CONFIG_HOME="$config_home" \
    GSETTINGS_BACKEND=keyfile \
    GSETTINGS_SCHEMA_DIR="$installed_extension/schemas" \
    gsettings set org.gnome.shell.extensions.lensguard show-panel-indicator false

sha256sum "$installed_binary" "$installed_unit" "$installed_activation" \
    > "$test_root/first-install.sha256"
# Older local installs only marked the daemon and extension directories. Verify
# they can upgrade once and receive the new per-service ownership markers.
rm -f -- "$installed_unit_marker" "$installed_activation_marker"
run_installer >/dev/null
[[ -f $installed_unit_marker ]]
[[ -f $installed_activation_marker ]]
sha256sum "$installed_binary" "$installed_unit" "$installed_activation" \
    > "$test_root/second-install.sha256"
cmp "$test_root/first-install.sha256" "$test_root/second-install.sha256"
saved_preference=$(env \
    HOME="$test_home" \
    XDG_CONFIG_HOME="$config_home" \
    GSETTINGS_BACKEND=keyfile \
    GSETTINGS_SCHEMA_DIR="$installed_extension/schemas" \
    gsettings get org.gnome.shell.extensions.lensguard show-panel-indicator)
[[ $saved_preference == false ]]
[[ $(find "$data_home/gnome-shell/extensions" -mindepth 1 -maxdepth 1 -type d | wc -l) -eq 1 ]]

activate_once() {
    # shellcheck disable=SC2016 # The inner bash process expands these expressions.
    env \
        HOME="$test_home" \
        XDG_DATA_HOME="$data_home" \
        XDG_CONFIG_HOME="$config_home" \
        timeout 15 dbus-run-session -- bash -c '
            set -euo pipefail
            response=$(gdbus call --session \
                --dest "$1" \
                --object-path "$2" \
                --method "$3.Ping")
            [[ $response == *pong* ]]
            owner=$(gdbus call --session \
                --dest org.freedesktop.DBus \
                --object-path /org/freedesktop/DBus \
                --method org.freedesktop.DBus.NameHasOwner "$1")
            [[ $owner == *true* ]]
        ' lensguard-activation "$bus_name" "$object_path" "$interface_name"
}

# Two fresh private buses model independent login sessions and verify cold activation each time.
activate_once
activate_once

run_uninstaller >/dev/null
[[ ! -e $installed_binary ]]
[[ ! -e $installed_unit ]]
[[ ! -e $installed_unit_marker ]]
[[ ! -e $installed_activation ]]
[[ ! -e $installed_activation_marker ]]
[[ ! -e $installed_extension ]]
[[ -f $data_home/unrelated-sentinel ]]
run_uninstaller >/dev/null
[[ -f $data_home/unrelated-sentinel ]]

mkdir -p -- "$installed_extension" "$(dirname -- "$installed_unit")" "$(dirname -- "$installed_activation")"
printf '%s\n' 'unowned extension data' > "$installed_extension/sentinel"
printf '%s\n' 'unowned user unit' > "$installed_unit"
printf '%s\n' 'unowned D-Bus service' > "$installed_activation"

if run_installer >"$test_root/unowned-install.stdout" 2>"$test_root/unowned-install.stderr"; then
    printf '%s\n' 'installer replaced an unowned installation' >&2
    exit 1
fi
grep -Fq 'refusing to replace unowned user unit' "$test_root/unowned-install.stderr"
run_uninstaller >/dev/null
grep -Fxq 'unowned extension data' "$installed_extension/sentinel"
grep -Fxq 'unowned user unit' "$installed_unit"
grep -Fxq 'unowned D-Bus service' "$installed_activation"

printf '%s\n' \
    'Fresh and repeated local installation tests passed.' \
    'Upgrade preserved the extension preference.' \
    'Cold D-Bus activation and bus-name acquisition passed in two fresh sessions.' \
    'Fresh and repeated uninstall tests passed without removing unrelated files.' \
    'Install and uninstall preserved unowned conflicting files.'
