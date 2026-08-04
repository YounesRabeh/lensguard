// SPDX-License-Identifier: MIT

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {DbusClient} from './src/dbusClient.js';
import {LensGuardIndicator} from './src/indicator.js';
import {
    applyPreferencesToViewState,
    readPreferences,
} from './src/preferences.js';

export default class LensGuardExtension extends Extension {
    enable() {
        if (this._indicator)
            return;

        this._settings = this.getSettings();
        this._settingsChangedId = this._settings.connect(
            'changed', () => this._render());

        this._indicator = new LensGuardIndicator(
            () => this.openPreferences());
        Main.panel.statusArea.quickSettings.addExternalIndicator(this._indicator);

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

        if (this._settingsChangedId)
            this._settings?.disconnect(this._settingsChangedId);
        this._settingsChangedId = 0;
        this._settings = null;
        this._baseState = null;
    }

    _render() {
        if (!this._indicator || !this._settings || !this._baseState)
            return;

        this._indicator.render(applyPreferencesToViewState(
            this._baseState,
            readPreferences(this._settings)));
    }
}
