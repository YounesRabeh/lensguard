// SPDX-License-Identifier: GPL-3.0-or-later

import {
    PreferenceKey,
    applyPreferencesToViewState,
    normalizePreferences,
    readPreferences,
} from '../src/preferences.js';
import {
    applyInstallationConflictToViewState,
    createViewState,
} from '../src/sessionModel.js';

function assert(condition, message) {
    if (!condition)
        throw new Error(message);
}

function assertEqual(actual, expected, message) {
    if (actual !== expected)
        throw new Error(`${message}: expected ${expected}, got ${actual}`);
}

const backendFailure = createViewState({
    serviceAvailable: true,
    backendAvailable: false,
    sessions: [],
});

const defaults = applyPreferencesToViewState(backendFailure);
assert(defaults.backendWarningVisible, 'warnings default to enabled');
assert(defaults.panelIconVisible, 'failure indicator defaults to visible');
assertEqual(defaults.panelIconName, 'dialog-warning-symbolic',
    'default failure icon is a warning');

const quietFailure = applyPreferencesToViewState(backendFailure, {
    showBackendUnavailableWarning: false,
    showIndicatorDuringBackendFailure: true,
});
assert(!quietFailure.backendWarningVisible,
    'warning preference disables warning presentation');
assert(quietFailure.panelIconVisible,
    'warning language and panel visibility are independent');
assertEqual(quietFailure.panelIconName, 'dialog-information-symbolic',
    'disabled warnings use a neutral status icon');
assert(!quietFailure.subtitle.toLowerCase().includes('safe'),
    'failure state does not make a misleading safety claim');

const hiddenFailure = applyPreferencesToViewState(backendFailure, {
    showBackendUnavailableWarning: true,
    showIndicatorDuringBackendFailure: false,
});
assert(!hiddenFailure.panelIconVisible,
    'indicator preference hides the backend-failure panel icon');
assert(hiddenFailure.backendWarningVisible,
    'hiding the panel icon does not erase the menu warning');

const serviceFailure = createViewState({serviceAvailable: false});
const quietServiceFailure = applyPreferencesToViewState(serviceFailure, {
    showBackendUnavailableWarning: false,
    showIndicatorDuringBackendFailure: true,
});
assert(!quietServiceFailure.backendWarningVisible,
    'warning preference also applies when the daemon service is absent');
assertEqual(quietServiceFailure.panelIconName, 'dialog-information-symbolic',
    'service absence uses a neutral icon when warnings are disabled');

const installationConflict = applyInstallationConflictToViewState(
    createViewState({backendAvailable: true}), true);
const quietConflict = applyPreferencesToViewState(installationConflict, {
    showBackendUnavailableWarning: false,
});
assertEqual(quietConflict.status, 'installation-conflict',
    'monitoring preferences do not hide an installation conflict');
assertEqual(quietConflict.subtitle, 'Multiple extension copies installed',
    'installation conflict keeps its actionable message');

const active = createViewState({
    backendAvailable: true,
    sessions: [{
        sessionId: 'camera',
        applicationName: 'Camera',
        cameraName: 'Integrated Camera',
    }],
});
const hiddenPanel = applyPreferencesToViewState(active, {
    showPanelIndicator: false,
});
assert(!hiddenPanel.panelIconVisible,
    'panel preference hides the icon while retaining active camera state');
assert(hiddenPanel.cameraActive,
    'hiding the panel icon does not clear camera activity');
assert(applyPreferencesToViewState(active, {
    showBackendUnavailableWarning: false,
    showIndicatorDuringBackendFailure: false,
}) === active, 'backend preferences do not alter active camera state');

const values = new Map([
    [PreferenceKey.SHOW_PANEL_INDICATOR, false],
    [PreferenceKey.SHOW_BACKEND_UNAVAILABLE_WARNING, false],
    [PreferenceKey.SHOW_INDICATOR_DURING_BACKEND_FAILURE, true],
]);
const loaded = readPreferences({
    get_boolean(key) {
        return values.get(key);
    },
});
assertEqual(loaded.showPanelIndicator, false,
    'panel indicator preference is read from settings');
assertEqual(loaded.showBackendUnavailableWarning, false,
    'warning preference is read from settings');
assertEqual(loaded.showIndicatorDuringBackendFailure, true,
    'indicator preference is read from settings');

const malformed = normalizePreferences({
    showBackendUnavailableWarning: null,
    showIndicatorDuringBackendFailure: 0,
});
assert(malformed.showBackendUnavailableWarning,
    'malformed warning preference falls back safely');
assert(malformed.showIndicatorDuringBackendFailure,
    'malformed indicator preference falls back safely');

print('Preference-to-view-model tests passed.');
