// SPDX-License-Identifier: GPL-3.0-or-later

import {
    extensionInstallPaths,
    findInstalledExtensionCopies,
} from '../src/installationConflict.js';

function assertEqual(actual, expected, message) {
    if (actual !== expected)
        throw new Error(`${message}: expected ${expected}, got ${actual}`);
}

const uuid = 'lensguard@example.test';
const paths = extensionInstallPaths(
    uuid,
    '/home/test/.local/share',
    ['/usr/local/share', '/usr/share', '/usr/share']);
assertEqual(paths.length, 3, 'duplicate data directories are removed');

const installed = new Set([
    `/home/test/.local/share/gnome-shell/extensions/${uuid}`,
    `/usr/share/gnome-shell/extensions/${uuid}`,
]);
const copies = findInstalledExtensionCopies(
    uuid,
    '/home/test/.local/share',
    ['/usr/local/share', '/usr/share'],
    path => installed.has(path));
assertEqual(copies.length, 2,
    'user and system extension copies are both detected');

print('Extension installation-conflict tests passed.');
