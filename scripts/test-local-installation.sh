#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
artifact=${1:-$repo_root/target/debug/camera-monitor}
uuid=lensguard@younesrabeh.github.io
bus_name=io.github.younesrabeh.CameraMonitor
object_path=/io/github/younesrabeh/CameraMonitor
interface_name=io.github.younesrabeh.CameraMonitor1

for command_name in dbus-run-session gdbus glib-compile-schemas sha256sum timeout; do
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
[[ -x $installed_binary ]]
[[ -f $installed_unit ]]
[[ -f $installed_activation ]]
[[ -f $installed_extension/schemas/gschemas.compiled ]]
[[ -f $installed_extension/.lensguard-owned ]]
grep -Fq 'Type=dbus' "$installed_unit"
grep -Fq 'BusName=io.github.younesrabeh.CameraMonitor' "$installed_unit"
grep -Fq 'Restart=on-failure' "$installed_unit"
grep -Fq 'SystemdService=camera-monitor.service' "$installed_activation"
if grep -RqE '(^|[[:space:]])(sudo|pkexec)([[:space:]]|$)' \
    "$repo_root/scripts/install-local.sh" "$repo_root/scripts/uninstall-local.sh"; then
    printf '%s\n' 'local integration scripts must not request root privileges' >&2
    exit 1
fi

sha256sum "$installed_binary" "$installed_unit" "$installed_activation" \
    > "$test_root/first-install.sha256"
run_installer >/dev/null
sha256sum "$installed_binary" "$installed_unit" "$installed_activation" \
    > "$test_root/second-install.sha256"
cmp "$test_root/first-install.sha256" "$test_root/second-install.sha256"
[[ $(find "$data_home/gnome-shell/extensions" -mindepth 1 -maxdepth 1 -type d | wc -l) -eq 1 ]]

activate_once() {
    env \
        HOME="$test_home" \
        XDG_DATA_HOME="$data_home" \
        XDG_CONFIG_HOME="$config_home" \
        # shellcheck disable=SC2016 # The inner bash process expands these expressions.
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
[[ ! -e $installed_activation ]]
[[ ! -e $installed_extension ]]
[[ -f $data_home/unrelated-sentinel ]]
run_uninstaller >/dev/null
[[ -f $data_home/unrelated-sentinel ]]

printf '%s\n' \
    'Fresh and repeated local installation tests passed.' \
    'Cold D-Bus activation and bus-name acquisition passed in two fresh sessions.' \
    'Fresh and repeated uninstall tests passed without removing unrelated files.'
