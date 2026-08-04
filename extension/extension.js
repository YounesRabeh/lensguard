// SPDX-License-Identifier: MIT

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {DbusClient} from './src/dbusClient.js';
import {LensGuardIndicator} from './src/indicator.js';

export default class LensGuardExtension extends Extension {
    enable() {
        if (this._indicator)
            return;

        this._indicator = new LensGuardIndicator();
        Main.panel.statusArea.quickSettings.addExternalIndicator(this._indicator);

        this._client = new DbusClient(
            state => this._indicator?.render(state));
        this._client.start();
    }

    disable() {
        this._client?.stop();
        this._client = null;

        this._indicator?.destroy();
        this._indicator = null;
    }
}
