// SPDX-License-Identifier: GPL-3.0-or-later

export const PreferenceKey = Object.freeze({
    SHOW_PANEL_INDICATOR: 'show-panel-indicator',
    SHOW_OBSERVER_UNAVAILABLE_WARNING: 'show-observer-unavailable-warning',
    SHOW_INDICATOR_DURING_OBSERVER_FAILURE:
        'show-indicator-during-observer-failure',
});

export const DEFAULT_PREFERENCES = Object.freeze({
    showPanelIndicator: true,
    showObserverUnavailableWarning: true,
    showIndicatorDuringObserverFailure: true,
});

export function normalizePreferences(values = {}) {
    return {
        showPanelIndicator: values.showPanelIndicator !== false,
        showObserverUnavailableWarning:
            values.showObserverUnavailableWarning !== false,
        showIndicatorDuringObserverFailure:
            values.showIndicatorDuringObserverFailure !== false,
    };
}

export function readPreferences(settings) {
    return normalizePreferences({
        showPanelIndicator: settings.get_boolean(
            PreferenceKey.SHOW_PANEL_INDICATOR),
        showObserverUnavailableWarning: settings.get_boolean(
            PreferenceKey.SHOW_OBSERVER_UNAVAILABLE_WARNING),
        showIndicatorDuringObserverFailure: settings.get_boolean(
            PreferenceKey.SHOW_INDICATOR_DURING_OBSERVER_FAILURE),
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
    const monitoringFailure = state.status === 'observer-unavailable' ||
        state.status === 'service-unavailable';
    if (!monitoringFailure) {
        if (!hidePanelIndicator)
            return state;
        return {...state, panelIconVisible: false};
    }

    if (preferences.showObserverUnavailableWarning) {
        return {
            ...state,
            observerWarningVisible: true,
            panelIconVisible:
                preferences.showIndicatorDuringObserverFailure &&
                !hidePanelIndicator,
        };
    }

    return {
        ...state,
        observerWarningVisible: false,
        panelIconVisible: preferences.showIndicatorDuringObserverFailure &&
            !hidePanelIndicator,
        panelIconName: 'dialog-information-symbolic',
        subtitle: 'Camera status unavailable',
        accessibleLabel: 'LensGuard camera status is unavailable',
        tooltip: 'LensGuard: camera status unavailable',
    };
}
