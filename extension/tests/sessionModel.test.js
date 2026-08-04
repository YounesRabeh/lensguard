// SPDX-License-Identifier: MIT

import {
    ViewStatus,
    createViewState,
    normalizeSessions,
} from '../src/sessionModel.js';

function assert(condition, message) {
    if (!condition)
        throw new Error(message);
}

function assertEqual(actual, expected, message) {
    if (actual !== expected)
        throw new Error(`${message}: expected ${expected}, got ${actual}`);
}

function testSessionTransformation() {
    const sessions = normalizeSessions([{
        sessionId: 'one',
        applicationName: '  Discord  ',
        cameraName: '',
    }, null, {sessionId: ''}]);

    assertEqual(sessions.length, 1, 'invalid sessions are removed');
    assertEqual(sessions[0].applicationName, 'Discord', 'names are trimmed');
    assertEqual(sessions[0].cameraName, 'Unknown camera', 'camera fallback is used');
}

function testDuplicateHandling() {
    const sessions = normalizeSessions([{
        sessionId: 'same',
        applicationName: 'First',
        cameraName: 'Camera A',
    }, {
        sessionId: 'same',
        applicationName: 'Duplicate',
        cameraName: 'Camera B',
    }]);

    assertEqual(sessions.length, 1, 'duplicate session IDs are removed');
    assertEqual(sessions[0].applicationName, 'First', 'first duplicate wins');
}

function testDerivedState() {
    const inactive = createViewState({backendAvailable: true, sessions: []});
    assertEqual(inactive.status, ViewStatus.INACTIVE, 'empty state is inactive');
    assert(!inactive.cameraActive, 'empty state does not claim camera use');
    assert(!inactive.panelIconVisible, 'inactive panel icon is hidden');

    const active = createViewState({
        backendAvailable: true,
        sessions: [{
            sessionId: 'active',
            applicationName: 'Discord',
            cameraName: 'Webcam',
        }],
    });
    assertEqual(active.status, ViewStatus.ACTIVE, 'session state is active');
    assert(active.cameraActive, 'active state claims camera use');
    assert(active.panelIconVisible, 'active panel icon is visible');
}

function testStableOrdering() {
    const input = [{
        sessionId: 'z',
        applicationName: 'Zoom',
        cameraName: 'Camera B',
    }, {
        sessionId: 'd2',
        applicationName: 'discord',
        cameraName: 'Camera B',
    }, {
        sessionId: 'd1',
        applicationName: 'Discord',
        cameraName: 'Camera A',
    }];
    const first = normalizeSessions(input);
    const second = normalizeSessions([...input].reverse());
    const expectedIds = 'd1,d2,z';

    assertEqual(first.map(session => session.sessionId).join(','), expectedIds,
        'sessions have stable display order');
    assertEqual(second.map(session => session.sessionId).join(','), expectedIds,
        'input order does not affect display order');
}

function testBackendUnavailable() {
    const state = createViewState({
        backendAvailable: false,
        sessions: [{
            sessionId: 'stale',
            applicationName: 'Stale application',
            cameraName: 'Stale camera',
        }],
    });

    assertEqual(state.status, ViewStatus.BACKEND_UNAVAILABLE,
        'backend failure has explicit status');
    assert(!state.cameraActive, 'backend failure does not claim camera use');
    assertEqual(state.sessions.length, 0, 'backend failure discards stale sessions');
    assert(state.panelIconVisible, 'backend failure remains visible as a warning');
    assertEqual(state.panelIconName, 'dialog-warning-symbolic',
        'backend failure does not use the camera-active icon');
}

testSessionTransformation();
testDuplicateHandling();
testDerivedState();
testStableOrdering();
testBackendUnavailable();

print('Session model tests passed.');
