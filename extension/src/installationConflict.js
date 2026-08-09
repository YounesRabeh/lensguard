// SPDX-License-Identifier: GPL-3.0-or-later

export function extensionInstallPaths(uuid, userDataDir, systemDataDirs = []) {
    const paths = [userDataDir, ...systemDataDirs]
        .filter(path => typeof path === 'string' && path.length > 0)
        .map(path => `${path}/gnome-shell/extensions/${uuid}`);
    return [...new Set(paths)];
}

export function findInstalledExtensionCopies(
    uuid,
    userDataDir,
    systemDataDirs,
    isDirectory
) {
    return extensionInstallPaths(uuid, userDataDir, systemDataDirs)
        .filter(path => isDirectory(path));
}
