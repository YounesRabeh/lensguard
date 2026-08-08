// SPDX-License-Identifier: GPL-3.0-or-later

const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f-\u009f]/g;
const HAS_CONTROL_CHARACTERS = /[\u0000-\u001f\u007f-\u009f]/;
const MAX_DISPLAY_LENGTH = 160;
const MAX_ID_LENGTH = 512;

function normalizeDisplayText(value, fallback) {
    if (typeof value !== 'string')
        return fallback;

    const normalized = value
        .replace(CONTROL_CHARACTERS, ' ')
        .replace(/\s+/g, ' ')
        .trim();
    if (!normalized)
        return fallback;

    return [...normalized].slice(0, MAX_DISPLAY_LENGTH).join('');
}

function normalizeIdentifier(value) {
    if (typeof value !== 'string')
        return null;

    const normalized = value.trim();
    if (!normalized || normalized.length > MAX_ID_LENGTH ||
        HAS_CONTROL_CHARACTERS.test(normalized))
        return null;

    return normalized;
}

function isUint64(value) {
    return (typeof value === 'bigint' && value >= 0n) ||
        (Number.isSafeInteger(value) && value >= 0);
}

function normalizeSessionTuple(tuple) {
    if (!Array.isArray(tuple) || tuple.length !== 8)
        return null;

    const [
        rawSessionId,
        rawApplicationId,
        rawApplicationName,
        rawDeviceId,
        rawDeviceName,
        rawBackend,
        startedAtUnixMs,
        processId,
    ] = tuple;
    const sessionId = normalizeIdentifier(rawSessionId);
    const deviceId = normalizeIdentifier(rawDeviceId);
    const backend = normalizeIdentifier(rawBackend);
    if (!sessionId || !deviceId || !backend ||
        !isUint64(startedAtUnixMs) ||
        !Number.isInteger(processId) || processId < 0 || processId > 0xffffffff)
        return null;

    const applicationId = normalizeDisplayText(rawApplicationId, '');
    const applicationName = normalizeDisplayText(
        rawApplicationName,
        applicationId || 'Unknown application');
    const cameraName = normalizeDisplayText(rawDeviceName, 'Unknown camera');

    return {
        sessionId,
        applicationName,
        cameraName,
    };
}

export function normalizeDbusSessions(methodReply) {
    if (!Array.isArray(methodReply) || methodReply.length !== 1 ||
        !Array.isArray(methodReply[0]))
        return [];

    const uniqueSessions = new Map();
    for (const tuple of methodReply[0]) {
        const session = normalizeSessionTuple(tuple);
        if (session && !uniqueSessions.has(session.sessionId))
            uniqueSessions.set(session.sessionId, session);
    }

    return [...uniqueSessions.values()];
}

export function normalizeDbusSnapshot(properties, methodReply) {
    if (properties?.serviceAvailable !== true) {
        return {
            serviceAvailable: false,
            backendAvailable: false,
            sessions: [],
        };
    }

    return {
        serviceAvailable: true,
        backendAvailable: properties.backendAvailable === true,
        reportedActive: properties.active === true,
        reportedSessionCount: Number.isInteger(properties.activeSessionCount)
            ? properties.activeSessionCount
            : 0,
        sessions: normalizeDbusSessions(methodReply),
    };
}
