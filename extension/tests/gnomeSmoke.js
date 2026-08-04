// SPDX-License-Identifier: MIT

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Scripting from 'resource:///org/gnome/shell/ui/scripting.js';

import {
    FakeCameraService,
    createSessionTuple,
} from './fixtures/fakeCameraService.js';

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
    for (let attempt = 0; attempt < 100; attempt++) {
        if (predicate())
            return;
        await Scripting.sleep(50);
    }

    throw new Error(message);
}

function sessionLabels(lensGuardIndicator) {
    return lensGuardIndicator._toggle._sessionSection
        ._getMenuItems()
        .map(item => item.label.text);
}

export async function run() {
    const service = new FakeCameraService();
    let settings = null;
    const discord = createSessionTuple({
        sessionId: 'discord',
        applicationName: 'Discord',
        deviceId: 'integrated-camera',
        deviceName: 'Integrated Camera',
    });
    const meet = createSessionTuple({
        sessionId: 'meet',
        applicationName: 'Firefox (Google Meet)',
        deviceId: 'usb-camera',
        deviceName: 'USB Webcam',
    });

    try {
        const extension = Main.extensionManager.lookup(UUID);
        assert(extension,
            'GNOME Shell did not discover the packaged LensGuard extension');
        await waitFor(() => indicators().length === 1,
            'extension did not add its Quick Settings indicator ' +
            `(state=${extension.state}, error=${extension.error ?? 'none'})`);
        settings = extension.stateObj?._settings;
        assert(settings,
            'loaded extension did not retain its GSettings instance');
        settings.reset('show-backend-unavailable-warning');
        settings.reset('show-indicator-during-backend-failure');
        const preferencesItem = indicator()._toggle.menu
            ._getMenuItems()
            .find(item => item.name === 'lensguard-preferences');
        assert(preferencesItem,
            'Quick Settings menu does not expose Preferences');
        assert(preferencesItem.accessible_name ===
            'Open LensGuard preferences',
        'Preferences action does not have a clear accessible name');
        assert(indicator()._statusIcon.visible,
            'missing service warning is not visible');
        assert(indicator()._statusIcon.icon_name === 'dialog-warning-symbolic',
            'missing service incorrectly uses the active camera icon');
        assert(sessionLabels(indicator())[0] ===
            'The LensGuard camera monitor service is not running.',
        'missing service menu message is not useful');

        await service.start();
        await waitFor(() => indicator()._toggle.subtitle === 'No camera in use',
            'initial empty D-Bus snapshot did not render inactive state');
        assert(!indicator()._statusIcon.visible,
            'inactive D-Bus state did not hide the top-bar icon');

        service.startSession(discord);
        await waitFor(() => indicator()._statusIcon.visible,
            'D-Bus start event did not show the top-bar icon');
        let labels = sessionLabels(indicator());
        assert(labels.length === 1, 'one-session D-Bus menu did not render one item');
        assert(labels[0] === 'Discord — Integrated Camera',
            `unexpected D-Bus session label: ${labels[0]}`);

        service.startSession(discord);
        service.startSession(discord);
        await Scripting.sleep(100);
        assert(sessionLabels(indicator()).length === 1,
            'repeated D-Bus state events duplicated the session');

        service.startSession(meet);
        await waitFor(() => sessionLabels(indicator()).length === 2,
            'multi-session D-Bus menu did not render two items');
        labels = sessionLabels(indicator());
        assert(labels[0] === 'Discord — Integrated Camera',
            'multi-session D-Bus menu order is not stable');
        assert(labels[1] === 'Firefox (Google Meet) — USB Webcam',
            'multi-session D-Bus menu omitted the second app/camera');

        service.stopSession('discord');
        await waitFor(() => sessionLabels(indicator()).length === 1,
            'first D-Bus stop event did not remove its session');
        assert(indicator()._statusIcon.visible,
            'first D-Bus stop hid the icon while another session remained');

        service.stopSession('meet');
        await waitFor(() => !indicator()._statusIcon.visible,
            'final D-Bus stop event did not hide the icon');

        service.setBackendAvailable(false);
        await waitFor(() =>
            indicator()._toggle.subtitle === 'Camera monitoring unavailable',
        'backend failure did not use the default warning presentation');
        assert(indicator()._statusIcon.visible,
            'backend warning icon is not visible by default');
        assert(indicator()._statusIcon.icon_name === 'dialog-warning-symbolic',
            'backend failure did not use the warning icon by default');

        settings.set_boolean(
            'show-indicator-during-backend-failure', false);
        await waitFor(() => !indicator()._statusIcon.visible,
            'indicator visibility preference did not apply live');
        assert(indicator()._toggle.subtitle === 'Camera monitoring unavailable',
            'hiding the panel icon incorrectly erased the menu warning');

        settings.set_boolean('show-backend-unavailable-warning', false);
        settings.set_boolean('show-indicator-during-backend-failure', true);
        await waitFor(() =>
            indicator()._statusIcon.icon_name ===
                'dialog-information-symbolic' &&
            indicator()._statusIcon.visible,
        'warning preference did not apply live');
        assert(indicator()._toggle.subtitle === 'Camera status unavailable',
            'disabled warnings did not use neutral status language');
        assert(sessionLabels(indicator())[0] ===
            'Camera status is currently unavailable.',
        'disabled warnings retained the warning menu message');

        assert(Main.extensionManager.disableExtension(UUID),
            'disable failed during preference persistence test');
        await waitFor(() => indicators().length === 0,
            'indicator survived preference persistence disable');
        assert(Main.extensionManager.enableExtension(UUID),
            'enable failed during preference persistence test');
        await waitFor(() => indicators().length === 1,
            'indicator missing after preference persistence enable');
        await waitFor(() =>
            indicator()._toggle.subtitle === 'Camera status unavailable',
        're-enabling the extension did not preserve preferences');
        assert(indicator()._statusIcon.visible,
            'persisted backend indicator preference was lost');

        settings.set_boolean('show-backend-unavailable-warning', true);
        settings.set_boolean('show-indicator-during-backend-failure', true);
        service.setBackendAvailable(true);
        await waitFor(() => indicator()._toggle.subtitle === 'No camera in use',
            'restoring the backend did not return to inactive state');

        service.startSession(discord);
        await waitFor(() => indicator()._statusIcon.visible,
            'pre-restart session did not activate');
        service.stop();
        await waitFor(() =>
            indicator()._toggle.subtitle === 'Camera monitor service unavailable',
        'daemon disappearance did not clear stale UI');
        assert(indicator()._statusIcon.icon_name === 'dialog-warning-symbolic',
            'daemon disappearance retained the active camera icon');

        await service.start([meet]);
        await waitFor(() =>
            sessionLabels(indicator())[0] ===
                'Firefox (Google Meet) — USB Webcam',
        'daemon restart did not resynchronize the session list');

        for (let cycle = 0; cycle < 3; cycle++) {
            assert(Main.extensionManager.disableExtension(UUID),
                `disable failed in lifecycle cycle ${cycle + 1}`);
            await waitFor(() => indicators().length === 0,
                `indicator survived disable cycle ${cycle + 1}`);

            assert(Main.extensionManager.enableExtension(UUID),
                `enable failed in lifecycle cycle ${cycle + 1}`);
            await waitFor(() => indicators().length === 1,
                `indicator missing after enable cycle ${cycle + 1}`);
            await waitFor(() => indicator()._statusIcon.visible,
                `D-Bus state did not resync after enable cycle ${cycle + 1}`);
        }

        assert(Main.extensionManager.disableExtension(UUID),
            'final extension disable failed');
        await waitFor(() => indicators().length === 0,
            'indicator survived final disable');
    } finally {
        settings?.reset('show-backend-unavailable-warning');
        settings?.reset('show-indicator-during-backend-failure');
        service.stop();
    }
}

export function finish() {}
