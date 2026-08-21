// SPDX-License-Identifier: GPL-3.0-or-later

import {
    normalizeDbusSessions,
    normalizeDbusSnapshot,
} from '../src/dbusPayload.js';

function assert(condition, message) {
    if (!condition)
        throw new Error(message);
}

function assertEqual(actual, expected, message) {
    if (actual !== expected)
        throw new Error(`${message}: expected ${expected}, got ${actual}`);
}

const VALID_SESSION = [
    'session-1',
    'com.example.Video',
    '  Example\nVideo  ',
    'camera-1',
    ' Integrated\tCamera ',
    1234n,
    42,
];

function testValidPayload() {
    const sessions = normalizeDbusSessions([[VALID_SESSION]]);
    assertEqual(sessions.length, 1, 'valid session is retained');
    assertEqual(sessions[0].applicationName, 'Example Video',
        'untrusted app text is normalized');
    assertEqual(sessions[0].cameraName, 'Integrated Camera',
        'untrusted camera text is normalized');
}

function testMalformedAndDuplicatePayload() {
    const sessions = normalizeDbusSessions([[
        VALID_SESSION,
        [...VALID_SESSION],
        ['too-short'],
        ['bad\nid', '', 'Bad', 'camera', 'Camera', 1n, 2],
        ['bad-time', '', 'Bad', 'camera', 'Camera', -1, 2],
        ['bad-pid', '', 'Bad', 'camera', 'Camera', 1n, -2],
    ]]);

    assertEqual(sessions.length, 1,
        'duplicates and malformed session tuples are removed');
}

function testFallbacksAndLengthLimit() {
    const longName = 'x'.repeat(300);
    const sessions = normalizeDbusSessions([[ [
        'session-2',
        'org.example.Fallback',
        '',
        'camera-2',
        longName,
        0,
        0,
    ] ]]);

    assertEqual(sessions[0].applicationName, 'org.example.Fallback',
        'application ID is the safe name fallback');
    assertEqual([...sessions[0].cameraName].length, 160,
        'display values are bounded');
}

function testSnapshotValidation() {
    const available = normalizeDbusSnapshot({
        serviceAvailable: true,
        observerAvailable: true,
        observerAvailability: 'available',
        active: true,
        activeSessionCount: 1,
    }, [[VALID_SESSION]]);
    assert(available.observerAvailable, 'valid observer property is retained');
    assertEqual(available.sessions.length, 1, 'valid snapshot includes sessions');

    const unavailable = normalizeDbusSnapshot({
        serviceAvailable: false,
        observerAvailable: true,
    }, [[VALID_SESSION]]);
    assert(!unavailable.observerAvailable, 'service loss forces observer unavailable');
    assertEqual(unavailable.sessions.length, 0, 'service loss rejects stale payload');

    assertEqual(normalizeDbusSessions([]).length, 0,
        'unexpected method reply shape is rejected');
}

testValidPayload();
testMalformedAndDuplicatePayload();
testFallbacksAndLengthLimit();
testSnapshotValidation();

print('D-Bus payload normalization tests passed.');
