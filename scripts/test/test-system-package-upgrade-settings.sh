#!/usr/bin/env bash
set -euo pipefail

mode=${1:-}
state_home=${2:-}
schema_id=org.gnome.shell.extensions.lensguard
schema_dir=${LENSGUARD_SCHEMA_DIR:-/usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io/schemas}

if [[ $mode != write && $mode != check ]] || [[ -z $state_home || $state_home != /* ]]; then
    printf '%s\n' \
        'Usage: scripts/test/test-system-package-upgrade-settings.sh {write|check} ABSOLUTE_STATE_HOME' >&2
    exit 2
fi

[[ -f $schema_dir/org.gnome.shell.extensions.lensguard.gschema.xml ]] || {
    printf 'upgrade settings test: schema is missing from %s\n' "$schema_dir" >&2
    exit 1
}

mkdir -p -- "$state_home/config"
run_gsettings() {
    env \
        HOME="$state_home" \
        XDG_CONFIG_HOME="$state_home/config" \
        GSETTINGS_BACKEND=keyfile \
        GSETTINGS_SCHEMA_DIR="$schema_dir" \
        gsettings "$@"
}

if [[ $mode == write ]]; then
    run_gsettings set "$schema_id" show-panel-indicator false
    [[ $(run_gsettings get "$schema_id" show-panel-indicator) == false ]]
    printf '%s\n' 'Stored pre-upgrade LensGuard preference.'
else
    [[ $(run_gsettings get "$schema_id" show-panel-indicator) == false ]] || {
        printf '%s\n' 'LensGuard preference did not survive package upgrade' >&2
        exit 1
    }
    printf '%s\n' 'Package upgrade preserved the LensGuard preference.'
fi
