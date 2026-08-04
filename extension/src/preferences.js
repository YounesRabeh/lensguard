// SPDX-License-Identifier: MIT

export const PreferenceKey = Object.freeze({
    SHOW_BACKEND_UNAVAILABLE_WARNING: 'show-backend-unavailable-warning',
    SHOW_INDICATOR_DURING_BACKEND_FAILURE:
        'show-indicator-during-backend-failure',
});

export const DEFAULT_PREFERENCES = Object.freeze({
    showBackendUnavailableWarning: true,
    showIndicatorDuringBackendFailure: true,
});

export function normalizePreferences(values = {}) {
    return {
        showBackendUnavailableWarning:
            values.showBackendUnavailableWarning !== false,
        showIndicatorDuringBackendFailure:
            values.showIndicatorDuringBackendFailure !== false,
    };
}

export function readPreferences(settings) {
    return normalizePreferences({
        showBackendUnavailableWarning: settings.get_boolean(
            PreferenceKey.SHOW_BACKEND_UNAVAILABLE_WARNING),
        showIndicatorDuringBackendFailure: settings.get_boolean(
            PreferenceKey.SHOW_INDICATOR_DURING_BACKEND_FAILURE),
    });
}

export function applyPreferencesToViewState(state, values = {}) {
    const preferences = normalizePreferences(values);
    const monitoringFailure = state.status === 'backend-unavailable' ||
        state.status === 'service-unavailable';
    if (!monitoringFailure)
        return state;

    if (preferences.showBackendUnavailableWarning) {
        return {
            ...state,
            backendWarningVisible: true,
            panelIconVisible:
                preferences.showIndicatorDuringBackendFailure,
        };
    }

    return {
        ...state,
        backendWarningVisible: false,
        panelIconVisible: preferences.showIndicatorDuringBackendFailure,
        panelIconName: 'dialog-information-symbolic',
        subtitle: 'Camera status unavailable',
        accessibleLabel: 'LensGuard camera status is unavailable',
        tooltip: 'LensGuard: camera status unavailable',
    };
}
