// SPDX-License-Identifier: MIT

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Scripting from 'resource:///org/gnome/shell/ui/scripting.js';

const UUID = 'lensguard@younesrabeh.github.io';

export const METRICS = {};

function assert(condition, message) {
    if (!condition)
        throw new Error(message);
}

function indicators() {
    return Main.panel.statusArea.quickSettings._indicators
        .get_children()
        .filter(actor => actor.name === 'lensguard-indicator');
}

function indicator() {
    const matches = indicators();
    assert(matches.length === 1,
        `expected exactly one LensGuard indicator, found ${matches.length}`);
    return matches[0];
}

function sessionLabels() {
    return indicator()._toggle._sessionSection
        ._getMenuItems()
        .map(item => item.label.text);
}

async function waitFor(predicate, message, attempts = 600) {
    for (let attempt = 0; attempt < attempts; attempt++) {
        if (predicate())
            return;
        await Scripting.sleep(50);
    }

    throw new Error(message);
}

export async function run() {
    const extension = Main.extensionManager.lookup(UUID);
    assert(extension,
        'GNOME Shell did not discover the packaged LensGuard extension');

    await waitFor(() => indicators().length === 1,
        'extension did not add its Quick Settings indicator ' +
        `(state=${extension.state}, error=${extension.error ?? 'none'})`);
    await waitFor(() => indicator()._statusIcon.visible,
        'real camera session did not show the top-bar icon');
    await waitFor(() => sessionLabels().some(label =>
        label.includes('Camera') && label.includes('Integrated Camera')),
    `real camera menu did not identify Camera and the device: ${sessionLabels()}`);

    console.log(`LENSGUARD_REAL_CAMERA_ACTIVE ${sessionLabels().join(' | ')}`);

    await waitFor(() => !indicator()._statusIcon.visible,
        'top-bar icon did not hide after real camera capture stopped');
    assert(indicator()._toggle.subtitle === 'No camera in use',
        `unexpected final subtitle: ${indicator()._toggle.subtitle}`);

    console.log('LENSGUARD_REAL_CAMERA_INACTIVE');
    assert(Main.extensionManager.disableExtension(UUID),
        'final extension disable failed');
    await waitFor(() => indicators().length === 0,
        'indicator survived final disable');
}

export function finish() {}
