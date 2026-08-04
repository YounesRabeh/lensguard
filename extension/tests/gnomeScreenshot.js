// SPDX-License-Identifier: MIT

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

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

function indicator() {
    const matches = Main.panel.statusArea.quickSettings._indicators
        .get_children()
        .filter(actor => actor.name === 'lensguard-indicator');
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

function sessionLabels() {
    return indicator()._toggle._sessionSection
        ._getMenuItems()
        .map(item => item.label.text);
}

async function captureScreenshot(path) {
    const file = Gio.File.new_for_path(path);
    const stream = file.replace(
        null, false, Gio.FileCreateFlags.REPLACE_DESTINATION, null);
    const screenshot = new Shell.Screenshot();

    await new Promise((resolve, reject) => {
        screenshot.screenshot(false, stream, (_source, result) => {
            try {
                screenshot.screenshot_finish(result);
                stream.close(null);
                resolve();
            } catch (error) {
                reject(error);
            }
        });
    });
}

async function openLensGuardMenu() {
    Main.panel.statusArea.quickSettings.menu.open();
    await Scripting.sleep(100);
    indicator()._toggle.menu.open();
    await Scripting.sleep(150);
}

export async function run() {
    const outputDir = GLib.getenv('LENSGUARD_SCREENSHOT_DIR');
    assert(outputDir, 'LENSGUARD_SCREENSHOT_DIR is not set');

    const service = new FakeCameraService();
    const sessions = [
        createSessionTuple({
            sessionId: 'camera',
            applicationName: 'Camera',
            deviceName: 'Integrated Camera (V4L2)',
        }),
        createSessionTuple({
            sessionId: 'meeting',
            applicationName: 'Firefox — Weekly design review with a deliberately long meeting title that remains bounded in the menu',
            deviceName: 'USB Webcam',
        }),
        createSessionTuple({
            sessionId: 'unknown',
            applicationName: '',
            deviceName: '',
        }),
    ];

    try {
        const extension = Main.extensionManager.lookup(UUID);
        assert(extension,
            'GNOME Shell did not discover the packaged LensGuard extension');
        await waitFor(() => {
            try {
                return indicator()._toggle !== null;
            } catch {
                return false;
            }
        }, 'extension did not add its indicator');

        await service.start(sessions);
        await waitFor(() => sessionLabels().length === 3,
            'multi-session screenshot state did not render');
        assert(sessionLabels().some(label =>
            label === 'Unknown application — Unknown camera'),
        'missing metadata fallback did not render');
        await openLensGuardMenu();
        await captureScreenshot(`${outputDir}/step10-active-sessions.png`);

        service.setBackendAvailable(false);
        await waitFor(() =>
            indicator()._toggle.subtitle === 'Camera monitoring unavailable',
        'backend screenshot state did not render');
        await openLensGuardMenu();
        await captureScreenshot(`${outputDir}/step10-backend-unavailable.png`);

        Main.panel.statusArea.quickSettings.menu.close();
        assert(Main.extensionManager.disableExtension(UUID),
            'final extension disable failed');
    } finally {
        service.stop();
    }
}

export function finish() {}
