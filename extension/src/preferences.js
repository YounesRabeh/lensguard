// SPDX-License-Identifier: GPL-3.0-or-later

export const PreferenceKey = Object.freeze({
    SHOW_PANEL_INDICATOR: 'show-panel-indicator',
    SHOW_BACKEND_UNAVAILABLE_WARNING: 'show-backend-unavailable-warning',
    SHOW_INDICATOR_DURING_BACKEND_FAILURE:
        'show-indicator-during-backend-failure',
});

export const DEFAULT_PREFERENCES = Object.freeze({
    showPanelIndicator: true,
    showBackendUnavailableWarning: true,
    showIndicatorDuringBackendFailure: true,
});

export function normalizePreferences(values = {}) {
    return {
        showPanelIndicator: values.showPanelIndicator !== false,
        showBackendUnavailableWarning:
            values.showBackendUnavailableWarning !== false,
        showIndicatorDuringBackendFailure:
            values.showIndicatorDuringBackendFailure !== false,
    };
}

export function readPreferences(settings) {
    return normalizePreferences({
        showPanelIndicator: settings.get_boolean(
            PreferenceKey.SHOW_PANEL_INDICATOR),
        showBackendUnavailableWarning: settings.get_boolean(
            PreferenceKey.SHOW_BACKEND_UNAVAILABLE_WARNING),
        showIndicatorDuringBackendFailure: settings.get_boolean(
            PreferenceKey.SHOW_INDICATOR_DURING_BACKEND_FAILURE),
    });
}

export function applyPreferencesToViewState(state, values = {}) {
    const preferences = normalizePreferences(values);
    const hidePanelIndicator = !preferences.showPanelIndicator;
    if (state.status === 'installation-conflict') {
        return hidePanelIndicator
            ? {...state, panelIconVisible: false}
            : state;
    }
    const monitoringFailure = state.status === 'backend-unavailable' ||
        state.status === 'service-unavailable';
    if (!monitoringFailure) {
        if (!hidePanelIndicator)
            return state;
        return {...state, panelIconVisible: false};
    }

    if (preferences.showBackendUnavailableWarning) {
        return {
            ...state,
            backendWarningVisible: true,
            panelIconVisible:
                preferences.showIndicatorDuringBackendFailure &&
                !hidePanelIndicator,
        };
    }

    return {
        ...state,
        backendWarningVisible: false,
        panelIconVisible: preferences.showIndicatorDuringBackendFailure &&
            !hidePanelIndicator,
        panelIconName: 'dialog-information-symbolic',
        subtitle: 'Camera status unavailable',
        accessibleLabel: 'LensGuard camera status is unavailable',
        tooltip: 'LensGuard: camera status unavailable',
    };
}
