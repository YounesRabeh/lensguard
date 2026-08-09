#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
version=$("$repo_root/scripts/project-version.sh")

for command_name in find rg stat; do
    command -v "$command_name" >/dev/null || {
        printf 'test-system-package.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

test_root=$(mktemp -d --tmpdir lensguard-system-package.XXXXXX)
cleanup() {
    rm -rf -- "$test_root"
}
trap cleanup EXIT

stage_root=$test_root/root
"$repo_root/scripts/stage-system-package.sh" \
    --root "$stage_root" \
    --daemon-path /usr/libexec/lensguard/camera-monitor >/dev/null

daemon=$stage_root/usr/libexec/lensguard/camera-monitor
unit=$stage_root/usr/lib/systemd/user/camera-monitor.service
activation=$stage_root/usr/share/dbus-1/services/io.github.younesrabeh.CameraMonitor.service
extension=$stage_root/usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io

[[ -x $daemon ]]
[[ $("$daemon" --version) == "camera-monitor $version" ]]
[[ $(stat --format='%a' "$daemon") == 755 ]]
[[ $(stat --format='%a' "$unit") == 644 ]]
[[ $(stat --format='%a' "$activation") == 644 ]]
[[ -f $extension/metadata.json ]]
[[ -f $extension/schemas/gschemas.compiled ]]
rg -Fq 'ExecStart="/usr/libexec/lensguard/camera-monitor" --log-level info run' "$unit"
rg -Fq 'Exec="/usr/libexec/lensguard/camera-monitor" --log-level info run' "$activation"
rg -Fq 'SystemdService=camera-monitor.service' "$activation"

if find "$extension" -type f | rg -q \
    '/(tests?|fixtures|mocks?)/|mockDataProvider|gnomeSmoke|gnomeScreenshot'; then
    printf '%s\n' 'staged extension contains development-only files' >&2
    exit 1
fi
if find "$stage_root" -mindepth 1 -maxdepth 1 ! -name usr | rg -q .; then
    printf '%s\n' 'system package stages files outside /usr' >&2
    exit 1
fi

printf '%s\n' 'System-package paths, modes, versions, and production contents passed.'

service_root=$test_root/service-root
"$repo_root/scripts/stage-system-package.sh" \
    --root "$service_root" \
    --daemon-path /usr/libexec/lensguard/camera-monitor \
    --package-name lensguard-service \
    --service-only >/dev/null
[[ -x $service_root/usr/libexec/lensguard/camera-monitor ]]
[[ -f $service_root/usr/lib/systemd/user/camera-monitor.service ]]
[[ -f $service_root/usr/share/dbus-1/services/io.github.younesrabeh.CameraMonitor.service ]]
if [[ -e $service_root/usr/share/gnome-shell || \
      -L $service_root/usr/share/gnome-shell ]]; then
    printf '%s\n' 'service-only package unexpectedly contains GNOME extension files' >&2
    exit 1
fi

printf '%s\n' 'Service-only package boundary passed.'
