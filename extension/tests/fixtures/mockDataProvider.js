// SPDX-License-Identifier: MIT

import {createViewState} from '../../src/sessionModel.js';

const MOCK_STATES = Object.freeze({
    inactive: {
        backendAvailable: true,
        sessions: [],
    },
    'active-one': {
        backendAvailable: true,
        sessions: [{
            sessionId: 'discord-camera',
            applicationName: 'Discord',
            cameraName: 'Integrated Camera',
        }],
    },
    'active-multiple': {
        backendAvailable: true,
        sessions: [{
            sessionId: 'meet-camera',
            applicationName: 'Firefox (Google Meet)',
            cameraName: 'USB Webcam',
        }, {
            sessionId: 'discord-camera',
            applicationName: 'Discord',
            cameraName: 'Integrated Camera',
        }],
    },
    'backend-unavailable': {
        backendAvailable: false,
        sessions: [],
    },
});

export class MockDataProvider {
    constructor(settings, onStateChanged) {
        this._settings = settings;
        this._onStateChanged = onStateChanged;
        this._signalId = 0;
    }

    start() {
        if (this._signalId)
            return;

        this._signalId = this._settings.connect(
            'changed::mock-state',
            () => this._publish());
        this._publish();
    }

    stop() {
        if (this._signalId)
            this._settings.disconnect(this._signalId);

        this._signalId = 0;
        this._settings = null;
        this._onStateChanged = null;
    }

    _publish() {
        const stateName = this._settings.get_string('mock-state');
        const snapshot = MOCK_STATES[stateName] ?? MOCK_STATES.inactive;
        this._onStateChanged(createViewState(snapshot));
    }
}
