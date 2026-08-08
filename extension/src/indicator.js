// SPDX-License-Identifier: GPL-3.0-or-later

import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as QuickSettings from 'resource:///org/gnome/shell/ui/quickSettings.js';

import {formatSessionLabel} from './sessionModel.js';

const LensGuardToggle = GObject.registerClass(
class LensGuardToggle extends QuickSettings.QuickMenuToggle {
    constructor(openPreferences) {
        super({
            title: 'LensGuard',
            subtitle: 'No camera in use',
            iconName: 'camera-web-symbolic',
            toggleMode: false,
            menuButtonAccessibleName: 'Show active camera applications',
        });

        this.name = 'lensguard-quick-toggle';
        this._sessionSection = new PopupMenu.PopupMenuSection();
        this.menu.addMenuItem(this._sessionSection);

        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        const preferencesItem = new PopupMenu.PopupMenuItem('Preferences');
        preferencesItem.name = 'lensguard-preferences';
        preferencesItem.accessible_name = 'Open LensGuard preferences';
        preferencesItem.connect('activate', () => openPreferences());
        this.menu.addMenuItem(preferencesItem);
    }

    render(state) {
        this.title = state.title;
        this.subtitle = state.subtitle;
        this.iconName = state.panelIconName;
        this.checked = state.cameraActive;
        this.accessible_name = state.accessibleLabel;
        this.menu.setHeader(
            state.panelIconName,
            state.title,
            state.subtitle);

        this._sessionSection.removeAll();
        if (state.status === 'service-unavailable') {
            this._addInformationItem(
                state.backendWarningVisible
                    ? 'The LensGuard camera monitor service is not running.'
                    : 'Camera status is currently unavailable.');
            return;
        }

        if (state.status === 'backend-unavailable') {
            this._addInformationItem(
                state.backendWarningVisible
                    ? 'Camera use cannot be determined while monitoring is unavailable.'
                    : 'Camera status is currently unavailable.');
            return;
        }

        if (state.sessions.length === 0) {
            this._addInformationItem('No applications are using a camera.');
            return;
        }

        state.sessions.forEach((session, index) => {
            const item = new PopupMenu.PopupMenuItem(
                formatSessionLabel(session),
                {reactive: false, can_focus: false});
            item.name = `lensguard-session-${index}`;
            item.accessible_name =
                `${session.applicationName}, camera ${session.cameraName}`;
            this._sessionSection.addMenuItem(item);
        });
    }

    _addInformationItem(text) {
        const item = new PopupMenu.PopupMenuItem(
            text,
            {reactive: false, can_focus: false});
        item.name = 'lensguard-status-message';
        item.accessible_name = text;
        this._sessionSection.addMenuItem(item);
    }
});

export const LensGuardIndicator = GObject.registerClass(
class LensGuardIndicator extends QuickSettings.SystemIndicator {
    constructor(openPreferences) {
        super();

        this.name = 'lensguard-indicator';
        this.track_hover = true;

        this._statusIcon = this._addIndicator();
        this._statusIcon.name = 'lensguard-panel-icon';
        this._statusIcon.icon_name = 'camera-web-symbolic';
        this._statusIcon.accessible_name = 'LensGuard camera status';
        this._statusIcon.add_style_class_name('privacy-indicator');
        this._statusIcon.hide();

        this._toggle = new LensGuardToggle(openPreferences);
        this.quickSettingsItems.push(this._toggle);

        this._tooltip = new St.Label({
            style_class: 'dash-label lensguard-tooltip',
            text: 'LensGuard camera status',
            visible: false,
        });
        Main.uiGroup.add_child(this._tooltip);
        this.connect('notify::hover', () => this._syncTooltip());
    }

    render(state) {
        this._statusIcon.icon_name = state.panelIconName;
        this._statusIcon.accessible_name = state.accessibleLabel;
        this._statusIcon.visible = state.panelIconVisible;
        this._tooltip.text = state.tooltip;
        this._toggle.render(state);
        this._syncTooltip();
    }

    _syncTooltip() {
        if (!this.hover || !this._statusIcon.visible) {
            this._tooltip.hide();
            return;
        }

        this._tooltip.show();
        const [actorX, actorY] = this.get_transformed_position();
        const [actorWidth, actorHeight] = this.get_transformed_size();
        const [, tooltipWidth] = this._tooltip.get_preferred_width(-1);
        const monitor = Main.layoutManager.findMonitorForActor(this) ??
            Main.layoutManager.primaryMonitor;
        const unclampedX = actorX + (actorWidth - tooltipWidth) / 2;
        const minimumX = monitor?.x ?? 0;
        const maximumX = monitor
            ? monitor.x + monitor.width - tooltipWidth
            : unclampedX;
        const tooltipX = Math.clamp(unclampedX, minimumX, maximumX);

        this._tooltip.set_position(Math.round(tooltipX), actorY + actorHeight + 6);
    }

    destroy() {
        this._tooltip?.destroy();
        this._tooltip = null;
        this.quickSettingsItems.forEach(item => item.destroy());
        this.quickSettingsItems = [];
        super.destroy();
    }
});
