#!/usr/bin/env bash
set -euo pipefail

missing=0

check_command() {
    local command_name=$1
    local purpose=$2

    if command -v "$command_name" >/dev/null 2>&1; then
        printf '%-18s %s\n' "$command_name" "found ($purpose)"
    else
        printf '%-18s %s\n' "$command_name" "MISSING ($purpose)" >&2
        missing=1
    fi
}

printf '%s\n' 'LensGuard development prerequisite check'
check_command cargo 'Rust builds and tests'
check_command rustc 'Rust compiler'
check_command rustfmt 'Rust formatting'
check_command clang 'PipeWire binding generation'
check_command pkg-config 'native PipeWire library discovery'
check_command gnome-shell 'GNOME Shell compatibility discovery'
check_command gnome-extensions 'extension packaging'
check_command gjs 'extension syntax checks'
check_command pipewire 'PipeWire runtime'
check_command pw-cli 'PipeWire diagnostics'
check_command wireplumber 'WirePlumber runtime'
check_command wpctl 'WirePlumber diagnostics'
check_command dbus-send 'D-Bus diagnostics'
check_command gdbus 'D-Bus diagnostics'
check_command busctl 'D-Bus diagnostics'
check_command systemctl 'systemd user services'
check_command make 'quality command entry points'
check_command unzip 'extension package inspection'

if cargo clippy --version >/dev/null 2>&1; then
    printf '%-18s %s\n' 'cargo clippy' 'found (Rust linting)'
else
    printf '%-18s %s\n' 'cargo clippy' 'MISSING (Rust linting)' >&2
    missing=1
fi

if pkg-config --exists libpipewire-0.3; then
    printf '%-18s %s\n' 'libpipewire-0.3' "found ($(pkg-config --modversion libpipewire-0.3))"
else
    printf '%-18s %s\n' 'libpipewire-0.3' 'MISSING (PipeWire development files)' >&2
    missing=1
fi

printf '\nDetected versions:\n'
rustc --version 2>/dev/null || true
cargo --version 2>/dev/null || true
gnome-shell --version 2>/dev/null || true
gjs --version 2>/dev/null || true
pipewire --version 2>/dev/null || true
wireplumber --version 2>/dev/null || true
systemctl --version 2>/dev/null | head -n 1 || true

if systemctl --user is-system-running >/dev/null 2>&1; then
    printf '%s\n' 'systemd user manager: reachable'
else
    printf '%s\n' 'systemd user manager: not reachable from this terminal (check inside the desktop session)'
fi

if [[ $missing -ne 0 ]]; then
    printf '\n%s\n' 'One or more required tools are missing. See docs/development.md.' >&2
    exit 1
fi

printf '\n%s\n' 'All required command-line prerequisites were found.'
