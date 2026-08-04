// SPDX-License-Identifier: MIT

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {LensGuardIndicator} from './src/indicator.js';
import {MockDataProvider} from './src/mockDataProvider.js';

export default class LensGuardExtension extends Extension {
    enable() {
        if (this._indicator)
            return;

        this._indicator = new LensGuardIndicator();
        Main.panel.statusArea.quickSettings.addExternalIndicator(this._indicator);

        this._provider = new MockDataProvider(
            this.getSettings(),
            state => this._indicator?.render(state));
        this._provider.start();
    }

    disable() {
        this._provider?.stop();
        this._provider = null;

        this._indicator?.destroy();
        this._indicator = null;
    }
}
