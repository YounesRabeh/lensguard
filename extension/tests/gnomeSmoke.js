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

async function waitFor(predicate, message) {
    for (let attempt = 0; attempt < 40; attempt++) {
        if (predicate())
            return;
        await Scripting.sleep(50);
    }

    throw new Error(message);
}

async function setMockState(settings, state) {
    settings.set_string('mock-state', state);
    await Scripting.sleep(50);
}

function sessionLabels(lensGuardIndicator) {
    return lensGuardIndicator._toggle._sessionSection
        ._getMenuItems()
        .map(item => item.label.text);
}

export async function run() {
    await waitFor(() => indicators().length === 1,
        'extension did not add its Quick Settings indicator');
    const extension = Main.extensionManager.lookup(UUID);
    const settings = extension.stateObj._provider._settings;

    await setMockState(settings, 'inactive');
    assert(!indicator()._statusIcon.visible,
        'inactive state did not hide the top-bar icon');
    assert(indicator()._toggle.subtitle === 'No camera in use',
        'inactive tile status is incorrect');

    await setMockState(settings, 'active-one');
    assert(indicator()._statusIcon.visible,
        'active state did not show the top-bar icon');
    assert(indicator()._statusIcon.icon_name === 'camera-web-symbolic',
        'active state did not use the symbolic camera icon');
    let labels = sessionLabels(indicator());
    assert(labels.length === 1, 'one-session menu did not render one item');
    assert(labels[0] === 'Discord — Integrated Camera',
        `unexpected one-session label: ${labels[0]}`);

    await setMockState(settings, 'active-multiple');
    labels = sessionLabels(indicator());
    assert(labels.length === 2, 'multi-session menu did not render two items');
    assert(labels[0] === 'Discord — Integrated Camera',
        'multi-session menu order is not stable');
    assert(labels[1] === 'Firefox (Google Meet) — USB Webcam',
        'multi-session menu omitted the second app/camera');

    await setMockState(settings, 'backend-unavailable');
    assert(indicator()._statusIcon.visible,
        'backend warning is not visible');
    assert(indicator()._statusIcon.icon_name === 'dialog-warning-symbolic',
        'backend failure incorrectly claims active camera use');
    assert(!indicator()._toggle.checked,
        'backend failure left the camera state checked');

    for (let cycle = 0; cycle < 3; cycle++) {
        assert(Main.extensionManager.disableExtension(UUID),
            `disable failed in lifecycle cycle ${cycle + 1}`);
        await waitFor(() => indicators().length === 0,
            `indicator survived disable cycle ${cycle + 1}`);

        assert(Main.extensionManager.enableExtension(UUID),
            `enable failed in lifecycle cycle ${cycle + 1}`);
        await waitFor(() => indicators().length === 1,
            `indicator missing after enable cycle ${cycle + 1}`);
    }

    assert(Main.extensionManager.disableExtension(UUID),
        'final extension disable failed');
    await waitFor(() => indicators().length === 0,
        'indicator survived final disable');
}

export function finish() {}
