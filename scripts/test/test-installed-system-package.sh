#!/usr/bin/env bash
set -euo pipefail

version=
daemon_path=
expect_removed=false
service_only=false
uuid=lensguard@younesrabeh.github.io
bus_name=io.github.younesrabeh.CameraMonitor
object_path=/io/github/younesrabeh/CameraMonitor
interface_name=io.github.younesrabeh.CameraMonitor1

usage() {
    printf '%s\n' \
        'Usage: scripts/test/test-installed-system-package.sh --version VERSION --daemon-path PATH [--service-only] [--expect-removed]' \
        '' \
        'Validates an installed LensGuard system package and cold D-Bus activation.' \
        '--expect-removed    Assert that package-owned runtime files are absent instead.'
}

die() {
    printf 'test-installed-system-package.sh: %s\n' "$1" >&2
    exit 1
}

while (($#)); do
    case $1 in
        --version)
            (($# >= 2)) || die '--version requires a value'
            version=$2
            shift 2
            ;;
        --daemon-path)
            (($# >= 2)) || die '--daemon-path requires a value'
            daemon_path=$2
            shift 2
            ;;
        --expect-removed)
            expect_removed=true
            shift
            ;;
        --service-only)
            service_only=true
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

[[ -n $version ]] || die '--version is required'
[[ -n $daemon_path && $daemon_path = /* ]] || die '--daemon-path must be absolute'

unit_path=/usr/lib/systemd/user/camera-monitor.service
activation_path=/usr/share/dbus-1/services/$bus_name.service
extension_path=/usr/share/gnome-shell/extensions/$uuid
metadata_path=$extension_path/metadata.json

if $expect_removed; then
    owned_paths=(
        "$daemon_path" \
        "$unit_path" \
        "$activation_path"
    )
    if ! $service_only; then
        owned_paths+=("$extension_path")
    fi
    for owned_path in "${owned_paths[@]}"; do
        [[ ! -e $owned_path && ! -L $owned_path ]] || \
            die "package-owned path remains after uninstall: $owned_path"
    done
    printf '%s\n' 'System-package uninstall cleanup passed.'
    exit 0
fi

for command_name in dbus-run-session gdbus jq stat timeout; do
    command -v "$command_name" >/dev/null || die "required command not found: $command_name"
done

[[ -x $daemon_path ]] || die "daemon is not executable: $daemon_path"
[[ -f $unit_path ]] || die "user unit is missing: $unit_path"
[[ -f $activation_path ]] || die "D-Bus activation file is missing: $activation_path"
if $service_only; then
    [[ ! -e $extension_path && ! -L $extension_path ]] || \
        die "service-only package contains extension files: $extension_path"
else
    [[ -f $metadata_path ]] || die "extension metadata is missing: $metadata_path"
fi

[[ $(stat -c '%a' "$daemon_path") == 755 ]] || die 'daemon mode is not 0755'
[[ $(stat -c '%a' "$unit_path") == 644 ]] || die 'user-unit mode is not 0644'
[[ $(stat -c '%a' "$activation_path") == 644 ]] || die 'D-Bus service mode is not 0644'
if ! $service_only; then
    [[ $(stat -c '%a' "$metadata_path") == 644 ]] || \
        die 'extension metadata mode is not 0644'
fi

[[ $($daemon_path --version) == "camera-monitor $version" ]] || \
    die 'installed daemon version does not match the package version'
if ! $service_only; then
    [[ $(jq -r '."version-name"' "$metadata_path") == "$version" ]] || \
        die 'installed extension version does not match the package version'
fi

grep -Fq "ExecStart=\"$daemon_path\" --log-level info run" "$unit_path" || \
    die 'user unit does not launch the packaged daemon'
grep -Fq "Exec=\"$daemon_path\" --log-level info run" "$activation_path" || \
    die 'D-Bus activation file does not launch the packaged daemon'
grep -Fq 'SystemdService=camera-monitor.service' "$activation_path" || \
    die 'D-Bus activation file does not name the user unit'

if ! $service_only && find "$extension_path" -type f \
    \( -path '*/tests/*' -o -path '*/fixtures/*' -o -path '*/mocks/*' \) \
    -print -quit | grep -q .; then
    die 'installed extension contains development-only files'
fi

test_root=$(mktemp -d --tmpdir lensguard-package-activation.XXXXXX)
cleanup() {
    rm -rf -- "$test_root"
}
trap cleanup EXIT
mkdir -p -- "$test_root/home" "$test_root/config" "$test_root/data"

# The child shell expands its positional parameters supplied below.
# shellcheck disable=SC2016
env \
    HOME="$test_root/home" \
    XDG_CONFIG_HOME="$test_root/config" \
    XDG_DATA_HOME="$test_root/data" \
    XDG_DATA_DIRS=/usr/local/share:/usr/share \
    timeout 15 dbus-run-session -- bash -c '
        set -euo pipefail
        response=$(gdbus call --session \
            --dest "$1" \
            --object-path "$2" \
            --method "$3.Ping")
        [[ $response == *pong* ]]
    ' lensguard-package-activation "$bus_name" "$object_path" "$interface_name"

printf 'Installed system package %s passed file, version, and D-Bus activation tests.\n' "$version"
