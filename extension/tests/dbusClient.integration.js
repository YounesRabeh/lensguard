// SPDX-License-Identifier: GPL-3.0-or-later

import GLib from 'gi://GLib';

import {DbusClient} from '../src/dbusClient.js';
import {
    FakeCameraService,
    createSessionTuple,
} from './fixtures/fakeCameraService.js';

function assert(condition, message) {
    if (!condition)
        throw new Error(message);
}

function assertEqual(actual, expected, message) {
    if (actual !== expected)
        throw new Error(`${message}: expected ${expected}, got ${actual}`);
}

function delay(milliseconds) {
    return new Promise(resolve => {
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, milliseconds, () => {
            resolve();
            return GLib.SOURCE_REMOVE;
        });
    });
}

async function waitFor(predicate, message) {
    for (let attempt = 0; attempt < 200; attempt++) {
        if (predicate())
            return;
        await delay(10);
    }

    throw new Error(message);
}

const discord = createSessionTuple({
    sessionId: 'discord',
    applicationId: 'com.discordapp.Discord',
    applicationName: 'Discord',
    deviceId: 'integrated-camera',
    deviceName: 'Integrated Camera',
    processId: 100,
});
const meet = createSessionTuple({
    sessionId: 'meet',
    applicationId: 'org.mozilla.firefox',
    applicationName: 'Firefox (Google Meet)',
    deviceId: 'usb-camera',
    deviceName: 'USB Webcam',
    processId: 200,
});
const restarted = createSessionTuple({
    sessionId: 'restarted',
    applicationName: 'Camera after restart',
    deviceName: 'Restarted Camera',
});

const service = new FakeCameraService();
const states = [];
const errors = [];
const client = new DbusClient(state => states.push(state), {
    onError: error => {
        errors.push(error);
        print(`Fake-service client error: ${error.message}`);
    },
});

try {
    client.start();
    assertEqual(states.at(-1).status, 'service-unavailable',
        'client is fail-safe before service appears');

    await service.start([discord]);
    await waitFor(() => states.at(-1).status === 'active',
        'initial service snapshot was not synchronized');
    assertEqual(states.at(-1).sessions.length, 1,
        'initial synchronization includes one session');
    assertEqual(states.at(-1).sessions[0].applicationName, 'Discord',
        'initial synchronization preserves application identity');

    service.startSession(discord);
    service.startSession(discord);
    await delay(50);
    assertEqual(states.at(-1).sessions.length, 1,
        'repeated start events do not duplicate sessions');

    service.startSession(meet);
    await waitFor(() => states.at(-1).sessions.length === 2,
        'second session did not reach the client');
    assert(states.at(-1).cameraActive,
        'multiple sessions keep the camera state active');

    service.stopSession('discord');
    await waitFor(() => states.at(-1).sessions.length === 1,
        'first stop did not update the session list');
    assert(states.at(-1).cameraActive,
        'camera state cleared before the final session stopped');

    service.stopSession('meet');
    await waitFor(() => states.at(-1).status === 'inactive',
        'final stop did not hide the active state');
    assert(!states.at(-1).panelIconVisible,
        'final stop did not hide the panel icon');

    service.startSession(discord);
    await waitFor(() => states.at(-1).status === 'active',
        'pre-restart session did not activate');
    service.stop();
    await waitFor(() => states.at(-1).status === 'service-unavailable',
        'service disappearance did not clear the client');
    assertEqual(states.at(-1).sessions.length, 0,
        'service disappearance retained a stale session');

    await service.start([restarted]);
    await waitFor(() =>
        states.at(-1).sessions[0]?.sessionId === 'restarted',
    'daemon restart did not resynchronize sessions');
    assertEqual(states.at(-1).sessions[0].applicationName,
        'Camera after restart', 'restart synchronized the wrong session');

    service.setObserverAvailable(false);
    await waitFor(() => states.at(-1).status === 'observer-unavailable',
        'backend failure state did not reach the client');
    assert(!states.at(-1).cameraActive,
        'backend failure incorrectly claims camera activity');

    assertEqual(errors.length, 0, 'integration lifecycle produced client errors');
} finally {
    client.stop();
    service.stop();
}

print('D-Bus client fake-service integration tests passed.');
