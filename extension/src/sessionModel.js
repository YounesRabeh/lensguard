// SPDX-License-Identifier: GPL-3.0-or-later

export const ViewStatus = Object.freeze({
    ACTIVE: 'active',
    INACTIVE: 'inactive',
    OBSERVER_UNAVAILABLE: 'observer-unavailable',
    UNKNOWN_ACTIVITY: 'unknown-activity',
    SERVICE_UNAVAILABLE: 'service-unavailable',
    INSTALLATION_CONFLICT: 'installation-conflict',
});

const UNSAFE_DISPLAY_CHARACTERS =
    /[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069\ufeff]/g;
const MAX_DISPLAY_LENGTH = 160;

function normalizedIdentifier(value) {
    if (typeof value !== 'string')
        return '';

    return value.trim();
}

export function sanitizeDisplayText(value, fallback) {
    if (typeof value !== 'string')
        return fallback;

    const normalized = value
        .replace(UNSAFE_DISPLAY_CHARACTERS, ' ')
        .replace(/\s+/g, ' ')
        .trim();
    if (!normalized)
        return fallback;

    return [...normalized].slice(0, MAX_DISPLAY_LENGTH).join('');
}

function compareText(left, right) {
    const foldedLeft = left.toLowerCase();
    const foldedRight = right.toLowerCase();

    if (foldedLeft < foldedRight)
        return -1;
    if (foldedLeft > foldedRight)
        return 1;
    if (left < right)
        return -1;
    if (left > right)
        return 1;
    return 0;
}

function compareSessions(left, right) {
    return compareText(left.applicationName, right.applicationName) ||
        compareText(left.cameraName, right.cameraName) ||
        compareText(left.sessionId, right.sessionId);
}

export function normalizeSessions(sessions) {
    if (!Array.isArray(sessions))
        return [];

    const uniqueSessions = new Map();
    for (const session of sessions) {
        if (!session || typeof session !== 'object')
            continue;

        const sessionId = normalizedIdentifier(session.sessionId);
        if (!sessionId || uniqueSessions.has(sessionId))
            continue;

        uniqueSessions.set(sessionId, {
            sessionId,
            applicationName: sanitizeDisplayText(
                session.applicationName, 'Unknown application'),
            cameraName: sanitizeDisplayText(
                session.cameraName, 'Unknown camera'),
        });
    }

    return [...uniqueSessions.values()].sort(compareSessions);
}

export function formatSessionLabel(session) {
    return `${session.applicationName} — ${session.cameraName}`;
}

export function applyInstallationConflictToViewState(state, conflict) {
    if (!conflict)
        return state;

    return {
        status: ViewStatus.INSTALLATION_CONFLICT,
        serviceAvailable: false,
        observerAvailable: false,
        cameraActive: false,
        panelIconVisible: true,
        panelIconName: 'dialog-warning-symbolic',
        observerWarningVisible: true,
        title: 'Lens Guard conflict',
        subtitle: 'Multiple extension copies installed',
        accessibleLabel: 'Lens Guard has conflicting extension installations',
        tooltip: 'Lens Guard: conflicting extension installations',
        sessions: [],
    };
}

export function createViewState(snapshot = {}) {
    const serviceAvailable = snapshot.serviceAvailable !== false;
    if (!serviceAvailable) {
        return {
            status: ViewStatus.SERVICE_UNAVAILABLE,
            serviceAvailable: false,
            observerAvailable: false,
            cameraActive: false,
            panelIconVisible: true,
            panelIconName: 'dialog-warning-symbolic',
            observerWarningVisible: true,
            title: 'LensGuard',
            subtitle: 'Camera monitor service unavailable',
            accessibleLabel: 'LensGuard camera monitor service is unavailable',
            tooltip: 'LensGuard: camera monitor service unavailable',
            sessions: [],
        };
    }

    const observerAvailable = snapshot.observerAvailable === true;
    const sessions = observerAvailable
        ? normalizeSessions(snapshot.sessions)
        : [];
    const cameraActive = observerAvailable && sessions.length > 0;
    const unknownCameraActivity = observerAvailable &&
        snapshot.unknownCameraActivity === true;

    if (!observerAvailable) {
        return {
            status: ViewStatus.OBSERVER_UNAVAILABLE,
            serviceAvailable: true,
            observerAvailable: false,
            cameraActive: false,
            panelIconVisible: true,
            panelIconName: 'dialog-warning-symbolic',
            title: 'LensGuard',
            subtitle: 'Direct V4L2 monitoring unavailable',
            accessibleLabel: 'LensGuard camera monitoring is unavailable',
            tooltip: 'LensGuard: camera monitoring unavailable',
            observerWarningVisible: true,
            observerAvailability: snapshot.observerAvailability ??
                'connection-failed',
            observerStatusDetail: snapshot.observerStatusDetail ?? '',
            unknownCameraActivity: false,
            sessions,
        };
    }

    if (!cameraActive && unknownCameraActivity) {
        return {
            status: ViewStatus.UNKNOWN_ACTIVITY,
            serviceAvailable: true,
            observerAvailable: true,
            cameraActive: false,
            panelIconVisible: true,
            panelIconName: 'dialog-warning-symbolic',
            title: 'LensGuard',
            subtitle: 'Unknown camera activity',
            accessibleLabel: 'LensGuard: unknown camera activity',
            tooltip: 'LensGuard: unknown camera activity',
            observerWarningVisible: false,
            unknownCameraActivity: true,
            sessions,
        };
    }

    if (!cameraActive) {
        return {
            status: ViewStatus.INACTIVE,
            serviceAvailable: true,
            observerAvailable: true,
            cameraActive: false,
            panelIconVisible: false,
            panelIconName: 'camera-web-symbolic',
            observerWarningVisible: false,
            unknownCameraActivity: false,
            title: 'LensGuard',
            subtitle: 'No camera in use',
            accessibleLabel: 'LensGuard: no camera is in use',
            tooltip: 'LensGuard: no camera is in use',
            sessions,
        };
    }

    const count = sessions.length;
    return {
        status: ViewStatus.ACTIVE,
        serviceAvailable: true,
        observerAvailable: true,
        cameraActive: true,
        panelIconVisible: true,
        panelIconName: 'camera-web-symbolic',
        observerWarningVisible: false,
        unknownCameraActivity,
        title: 'Camera in use',
        subtitle: count === 1
            ? '1 active application'
            : `${count} active applications`,
        accessibleLabel: count === 1
            ? 'LensGuard: camera in use by 1 application'
            : `LensGuard: camera in use by ${count} applications`,
        tooltip: count === 1
            ? 'Camera in use by 1 application'
            : `Camera in use by ${count} applications`,
        sessions,
    };
}
