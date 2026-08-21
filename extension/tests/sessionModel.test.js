// SPDX-License-Identifier: GPL-3.0-or-later

import {
    ViewStatus,
    applyInstallationConflictToViewState,
    createViewState,
    formatSessionLabel,
    normalizeSessions,
    sanitizeDisplayText,
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
    const inactive = createViewState({observerAvailable: true, sessions: []});
    assertEqual(inactive.status, ViewStatus.INACTIVE, 'empty state is inactive');
    assert(!inactive.cameraActive, 'empty state does not claim camera use');
    assert(!inactive.panelIconVisible, 'inactive panel icon is hidden');

    const active = createViewState({
        observerAvailable: true,
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

function testObserverUnavailable() {
    const state = createViewState({
        observerAvailable: false,
        observerAvailability: 'missing-capability',
        sessions: [{
            sessionId: 'stale',
            applicationName: 'Stale application',
            cameraName: 'Stale camera',
        }],
    });

    assertEqual(state.status, ViewStatus.OBSERVER_UNAVAILABLE,
        'observer failure has explicit status');
    assert(!state.cameraActive, 'observer failure does not claim camera use');
    assertEqual(state.sessions.length, 0, 'observer failure discards stale sessions');
    assert(state.panelIconVisible, 'observer failure remains visible as a warning');
    assertEqual(state.panelIconName, 'dialog-warning-symbolic',
        'observer failure does not use the camera-active icon');
}

function testServiceUnavailable() {
    const state = createViewState({
        serviceAvailable: false,
        observerAvailable: true,
        sessions: [{
            sessionId: 'stale',
            applicationName: 'Stale application',
            cameraName: 'Stale camera',
        }],
    });

    assertEqual(state.status, ViewStatus.SERVICE_UNAVAILABLE,
        'service loss has explicit status');
    assert(!state.cameraActive, 'service loss does not claim camera use');
    assertEqual(state.sessions.length, 0, 'service loss clears stale sessions');
}

function testUnknownActivityNeverCreatesDirectSessionState() {
    const state = createViewState({
        observerAvailable: true,
        unknownCameraActivity: true,
        sessions: [],
    });
    assertEqual(state.status, ViewStatus.UNKNOWN_ACTIVITY,
        'unknown ownership has a stable diagnostic state');
    assert(!state.cameraActive,
        'unknown ownership never creates normal camera-active state');
    assertEqual(state.sessions.length, 0,
        'unknown ownership never creates an application row');
    assertEqual(state.panelIconName, 'dialog-warning-symbolic',
        'unknown ownership is distinguishable from direct capture');
}

function testInstallationConflict() {
    const normal = createViewState({observerAvailable: true, sessions: []});
    const conflict = applyInstallationConflictToViewState(normal, true);
    assertEqual(conflict.status, ViewStatus.INSTALLATION_CONFLICT,
        'duplicate extension copies produce an explicit conflict state');
    assertEqual(conflict.subtitle, 'Multiple extension copies installed',
        'conflict state explains the duplicate installation');
    assert(!conflict.cameraActive,
        'installation conflict does not claim camera activity');
    assertEqual(conflict.sessions.length, 0,
        'installation conflict does not expose stale sessions');
    assert(applyInstallationConflictToViewState(normal, false) === normal,
        'no conflict preserves the original state');
}

function testSafeUnusualApplicationNames() {
    const sessions = normalizeSessions([{
        sessionId: 'unusual',
        applicationName: '<b>Camera</b>\u202e\n meeting',
        cameraName: 'USB <script>alert(1)</script>',
    }]);

    assertEqual(sessions[0].applicationName, '<b>Camera</b> meeting',
        'markup remains literal while directional and control text is removed');
    assertEqual(
        formatSessionLabel(sessions[0]),
        '<b>Camera</b> meeting — USB <script>alert(1)</script>',
        'session label preserves safe literal external text');

    const longName = 'x'.repeat(200);
    assertEqual([...sanitizeDisplayText(longName, 'fallback')].length, 160,
        'external display text is bounded');
    assertEqual(sanitizeDisplayText('\u202e\n', 'Unknown application'),
        'Unknown application', 'unsafe-only names use the documented fallback');
}

testSessionTransformation();
testDuplicateHandling();
testDerivedState();
testStableOrdering();
testObserverUnavailable();
testServiceUnavailable();
testUnknownActivityNeverCreatesDirectSessionState();
testInstallationConflict();
testSafeUnusualApplicationNames();

print('Session model tests passed.');
