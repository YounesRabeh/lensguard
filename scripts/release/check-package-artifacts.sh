#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
release_dir=${1:-}
version=$("$repo_root/scripts/util/project-version.sh")

[[ -n $release_dir ]] || {
    printf '%s\n' 'Usage: scripts/release/check-package-artifacts.sh RELEASE_DIRECTORY' >&2
    exit 2
}
if [[ $release_dir != /* ]]; then
    release_dir=$PWD/$release_dir
fi
[[ -d $release_dir ]] || {
    printf 'check-package-artifacts.sh: directory does not exist: %s\n' "$release_dir" >&2
    exit 1
}

for command_name in dpkg-deb rpm; do
    command -v "$command_name" >/dev/null || {
        printf 'check-package-artifacts.sh: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

if command -v bsdtar >/dev/null; then
    archive_reader=bsdtar
elif command -v tar >/dev/null && command -v zstd >/dev/null; then
    archive_reader=tar
else
    printf '%s\n' \
        'check-package-artifacts.sh: bsdtar or GNU tar with zstd support is required' >&2
    exit 1
fi

mapfile -t deb_packages < <(find "$release_dir" -maxdepth 1 -type f -name "lensguard_${version}_*.deb")
mapfile -t service_deb_packages < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-service_${version}_*.deb")
mapfile -t rpm_packages < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-$version-*.rpm" ! -name '*.src.rpm')
mapfile -t source_rpm_packages < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-$version-*.src.rpm")
mapfile -t service_rpm_packages < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-service-$version-*.rpm" ! -name '*.src.rpm' \
    ! -name '*-debuginfo-*' ! -name '*-debugsource-*')
mapfile -t service_source_rpm_packages < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-service-$version-*.src.rpm")
mapfile -t rpm_debuginfo_packages < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-debuginfo-$version-*.rpm")
mapfile -t rpm_debugsource_packages < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-debugsource-$version-*.rpm")
mapfile -t arch_packages < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-$version-*.pkg.tar.*")
mapfile -t service_arch_packages < <(find "$release_dir" -maxdepth 1 -type f \
    -name "lensguard-service-$version-*.pkg.tar.*")

[[ ${#deb_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one DEB, found %s\n' "${#deb_packages[@]}" >&2
    exit 1
}
[[ ${#service_deb_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one service DEB, found %s\n' \
        "${#service_deb_packages[@]}" >&2
    exit 1
}
[[ ${#rpm_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one binary RPM, found %s\n' "${#rpm_packages[@]}" >&2
    exit 1
}
[[ ${#source_rpm_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one source RPM, found %s\n' \
        "${#source_rpm_packages[@]}" >&2
    exit 1
}
[[ ${#service_rpm_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one service RPM, found %s\n' \
        "${#service_rpm_packages[@]}" >&2
    exit 1
}
[[ ${#service_source_rpm_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one service source RPM, found %s\n' \
        "${#service_source_rpm_packages[@]}" >&2
    exit 1
}
[[ ${#rpm_debuginfo_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one RPM debuginfo package, found %s\n' \
        "${#rpm_debuginfo_packages[@]}" >&2
    exit 1
}
[[ ${#rpm_debugsource_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one RPM debugsource package, found %s\n' \
        "${#rpm_debugsource_packages[@]}" >&2
    exit 1
}
[[ ${#arch_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one Arch package, found %s\n' "${#arch_packages[@]}" >&2
    exit 1
}
[[ ${#service_arch_packages[@]} -eq 1 ]] || {
    printf 'check-package-artifacts.sh: expected one service Arch package, found %s\n' \
        "${#service_arch_packages[@]}" >&2
    exit 1
}

deb=${deb_packages[0]}
service_deb=${service_deb_packages[0]}
rpm_package=${rpm_packages[0]}
source_rpm_package=${source_rpm_packages[0]}
service_rpm_package=${service_rpm_packages[0]}
service_source_rpm_package=${service_source_rpm_packages[0]}
rpm_debuginfo_package=${rpm_debuginfo_packages[0]}
rpm_debugsource_package=${rpm_debugsource_packages[0]}
arch=${arch_packages[0]}
service_arch=${service_arch_packages[0]}
rendered_spec=$release_dir/lensguard-$version.spec
rendered_service_spec=$release_dir/lensguard-service-$version.spec

[[ -s $rendered_spec ]] || {
    printf 'check-package-artifacts.sh: rendered RPM spec is missing: %s\n' "$rendered_spec" >&2
    exit 1
}
[[ -s $rendered_service_spec ]] || {
    printf 'check-package-artifacts.sh: rendered service RPM spec is missing: %s\n' \
        "$rendered_service_spec" >&2
    exit 1
}
grep -Fxq "Version:        $version" "$rendered_spec"
if grep -Fq '@VERSION@' "$rendered_spec"; then
    printf '%s\n' 'check-package-artifacts.sh: rendered RPM spec contains a version placeholder' >&2
    exit 1
fi
grep -Fxq "Version:        $version" "$rendered_service_spec"
if grep -Fq '@VERSION@' "$rendered_service_spec"; then
    printf '%s\n' 'check-package-artifacts.sh: rendered service RPM spec contains a version placeholder' >&2
    exit 1
fi

[[ $(dpkg-deb --field "$deb" Version) == "$version" ]]
[[ $(dpkg-deb --field "$deb" Package) == lensguard ]]
grep -Eq '(^|, )lensguard-service([ ,]|$)' <<<"$(dpkg-deb --field "$deb" Conflicts)"
deb_dependencies=$(dpkg-deb --field "$deb" Depends)
deb_contents=$(dpkg-deb --contents "$deb")
for dependency in dbus gnome-shell systemd; do
    grep -Eq "(^|, )$dependency([ ,]|$)" <<<"$deb_dependencies"
done
grep -Fq './usr/lib/lensguard/camera-monitor' <<<"$deb_contents"
grep -Fq './usr/lib/lensguard/lensguard-v4l2-observer' <<<"$deb_contents"
grep -Fq './usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io/metadata.json' \
    <<<"$deb_contents"

[[ $(dpkg-deb --field "$service_deb" Version) == "$version" ]]
[[ $(dpkg-deb --field "$service_deb" Package) == lensguard-service ]]
grep -Eq '(^|, )lensguard([ ,]|$)' <<<"$(dpkg-deb --field "$service_deb" Conflicts)"
service_deb_contents=$(dpkg-deb --contents "$service_deb")
grep -Fq './usr/lib/lensguard/camera-monitor' <<<"$service_deb_contents"
grep -Fq './usr/lib/lensguard/lensguard-v4l2-observer' <<<"$service_deb_contents"
if grep -Fq './usr/share/gnome-shell/extensions/' <<<"$service_deb_contents"; then
    printf '%s\n' 'service DEB contains GNOME extension files' >&2
    exit 1
fi

[[ $(rpm -qp --queryformat '%{VERSION}' "$rpm_package") == "$version" ]]
grep -Fxq 'lensguard-service' <<<"$(rpm -qp --conflicts "$rpm_package")"
[[ $(rpm -qp --queryformat '%{VERSION}' "$source_rpm_package") == "$version" ]]
[[ $(rpm -qp --queryformat '%{VERSION}' "$rpm_debuginfo_package") == "$version" ]]
[[ $(rpm -qp --queryformat '%{VERSION}' "$rpm_debugsource_package") == "$version" ]]
rpm_dependencies=$(rpm -qp --requires "$rpm_package")
rpm_contents=$(rpm -qpl "$rpm_package")
for dependency in dbus gnome-shell systemd; do
    grep -Eq "^$dependency([[:space:]<>=].*)?$" <<<"$rpm_dependencies"
done
grep -Fxq '/usr/libexec/lensguard/camera-monitor' <<<"$rpm_contents"
grep -Fxq '/usr/libexec/lensguard/lensguard-v4l2-observer' <<<"$rpm_contents"
grep -Fxq '/usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io/metadata.json' \
    <<<"$rpm_contents"

[[ $(rpm -qp --queryformat '%{VERSION}' "$service_rpm_package") == "$version" ]]
[[ $(rpm -qp --queryformat '%{VERSION}' "$service_source_rpm_package") == "$version" ]]
grep -Fxq 'lensguard' <<<"$(rpm -qp --conflicts "$service_rpm_package")"
service_rpm_contents=$(rpm -qpl "$service_rpm_package")
grep -Fxq '/usr/libexec/lensguard/camera-monitor' <<<"$service_rpm_contents"
grep -Fxq '/usr/libexec/lensguard/lensguard-v4l2-observer' <<<"$service_rpm_contents"
if grep -Fq '/usr/share/gnome-shell/extensions/' <<<"$service_rpm_contents"; then
    printf '%s\n' 'service RPM contains GNOME extension files' >&2
    exit 1
fi

package_info=$("$archive_reader" -xOf "$arch" .PKGINFO)
arch_contents=$("$archive_reader" -tf "$arch")
grep -Fxq 'pkgname = lensguard' <<<"$package_info"
grep -Fxq "pkgver = $version-1" <<<"$package_info"
grep -Fxq 'conflict = lensguard-service' <<<"$package_info"
for dependency in dbus gnome-shell systemd; do
    grep -Eq "^depend = $dependency([<>=].*)?$" <<<"$package_info"
done
grep -Fxq 'usr/lib/lensguard/camera-monitor' <<<"$arch_contents"
grep -Fxq 'usr/lib/lensguard/lensguard-v4l2-observer' <<<"$arch_contents"
grep -Fxq 'usr/share/gnome-shell/extensions/lensguard@younesrabeh.github.io/metadata.json' \
    <<<"$arch_contents"

service_package_info=$("$archive_reader" -xOf "$service_arch" .PKGINFO)
service_arch_contents=$("$archive_reader" -tf "$service_arch")
grep -Fxq 'pkgname = lensguard-service' <<<"$service_package_info"
grep -Fxq "pkgver = $version-1" <<<"$service_package_info"
grep -Fxq 'conflict = lensguard' <<<"$service_package_info"
grep -Fxq 'usr/lib/lensguard/camera-monitor' <<<"$service_arch_contents"
grep -Fxq 'usr/lib/lensguard/lensguard-v4l2-observer' <<<"$service_arch_contents"
if grep -Fq 'usr/share/gnome-shell/extensions/' <<<"$service_arch_contents"; then
    printf '%s\n' 'service Arch package contains GNOME extension files' >&2
    exit 1
fi

printf 'Full and service-only DEB, RPM, and Arch artifacts passed synchronized metadata checks for %s.\n' "$version"
