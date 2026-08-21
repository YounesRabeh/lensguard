// SPDX-License-Identifier: GPL-3.0-or-later

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {DbusClient} from './src/dbusClient.js';
import {LensGuardIndicator} from './src/indicator.js';
import {findInstalledExtensionCopies} from './src/installationConflict.js';
import {applyInstallationConflictToViewState} from './src/sessionModel.js';
import {
    applyPreferencesToViewState,
    readPreferences,
} from './src/preferences.js';

export default class LensGuardExtension extends Extension {
    enable() {
        this._installationConflict = findInstalledExtensionCopies(
            this.uuid,
            GLib.get_user_data_dir(),
            GLib.get_system_data_dirs(),
            path => Gio.File.new_for_path(path).query_file_type(
                Gio.FileQueryInfoFlags.NONE, null) === Gio.FileType.DIRECTORY
        ).length > 1;

        this._settings = this.getSettings();
        this._settings.connectObject(
            'changed', () => this._render(), this);

        this._indicator = new LensGuardIndicator(
            () => this.openPreferences());
        const quickSettings = Main.panel.statusArea.quickSettings;
        quickSettings.addExternalIndicator(this._indicator);
        // External indicators are appended by default. Keep the app in the
        // first (far-left in LTR layouts) status-cluster slot.
        quickSettings._indicators.set_child_at_index(this._indicator, 0);

        this._client = new DbusClient(
            state => {
                this._baseState = state;
                this._render();
            });
        this._client.start();
    }

    disable() {
        this._client?.stop();
        this._client = null;

        this._indicator?.destroy();
        this._indicator = null;

        this._settings?.disconnectObject(this);
        this._settings = null;
        this._baseState = null;
        this._installationConflict = false;
    }

    _render() {
        if (!this._indicator || !this._settings || !this._baseState)
            return;

        const baseState = applyInstallationConflictToViewState(
            this._baseState, this._installationConflict);
        this._indicator.render(applyPreferencesToViewState(
            baseState,
            readPreferences(this._settings)));
    }
}
