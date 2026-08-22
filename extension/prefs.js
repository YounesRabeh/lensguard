// SPDX-License-Identifier: GPL-3.0-or-later

import Adw from 'gi://Adw';
import Gio from 'gi://Gio';

import {ExtensionPreferences} from
    'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

import {PreferenceKey} from './src/preferences.js';

export default class LensGuardPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const page = new Adw.PreferencesPage({
            title: 'Monitoring',
            icon_name: 'camera-web-symbolic',
        });
        const failureGroup = new Adw.PreferencesGroup({
            title: 'Monitoring failures',
            description: 'Choose how LensGuard reports that camera use ' +
                'cannot currently be determined.',
        });
        const warningRow = new Adw.SwitchRow({
            title: 'Show monitoring warnings',
            subtitle: 'Use warning language and a warning icon when the ' +
                'privileged V4L2 observer is unavailable.',
        });
        const indicatorRow = new Adw.SwitchRow({
            title: 'Keep the status icon visible',
            subtitle: 'Show a panel status icon while camera monitoring ' +
                'is unavailable.',
        });

        failureGroup.add(warningRow);
        failureGroup.add(indicatorRow);
        page.add(failureGroup);

        const visibilityGroup = new Adw.PreferencesGroup({
            title: 'Shell visibility',
            description: 'Choose where LensGuard appears in GNOME Shell.',
        });
        const quickSettingsRow = new Adw.SwitchRow({
            title: 'Show the LensGuard Quick Settings tile',
            subtitle: 'Show camera status and the active application list in Quick Settings.',
        });
        const panelRow = new Adw.SwitchRow({
            title: 'Show the LensGuard panel icon',
            subtitle: 'Show the top-bar icon while direct camera use is detected.',
        });
        visibilityGroup.add(quickSettingsRow);
        visibilityGroup.add(panelRow);
        page.add(visibilityGroup);
        window.add(page);

        window._settings = this.getSettings();
        window._settings.bind(
            PreferenceKey.SHOW_QUICK_SETTINGS_TILE,
            quickSettingsRow,
            'active',
            Gio.SettingsBindFlags.DEFAULT);
        window._settings.bind(
            PreferenceKey.SHOW_PANEL_INDICATOR,
            panelRow,
            'active',
            Gio.SettingsBindFlags.DEFAULT);
        window._settings.bind(
            PreferenceKey.SHOW_OBSERVER_UNAVAILABLE_WARNING,
            warningRow,
            'active',
            Gio.SettingsBindFlags.DEFAULT);
        window._settings.bind(
            PreferenceKey.SHOW_INDICATOR_DURING_OBSERVER_FAILURE,
            indicatorRow,
            'active',
            Gio.SettingsBindFlags.DEFAULT);
    }
}
