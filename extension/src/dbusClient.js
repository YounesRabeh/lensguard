// SPDX-License-Identifier: MIT

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {normalizeDbusSnapshot} from './dbusPayload.js';
import {createViewState} from './sessionModel.js';

export const SERVICE_NAME = 'io.github.younesrabeh.CameraMonitor';
export const OBJECT_PATH = '/io/github/younesrabeh/CameraMonitor';
export const INTERFACE_NAME = 'io.github.younesrabeh.CameraMonitor1';

const STATE_SIGNALS = new Set([
    'StateChanged',
    'SessionStarted',
    'SessionStopped',
    'BackendAvailabilityChanged',
]);
const STATE_PROPERTIES = new Set([
    'Active',
    'ActiveSessionCount',
    'BackendAvailable',
]);

function defaultWatchName(onAppeared, onVanished) {
    return Gio.bus_watch_name(
        Gio.BusType.SESSION,
        SERVICE_NAME,
        // The user service is on-demand activated through its D-Bus
        // .service file.  Request activation when we begin watching so a
        // fresh installation does not remain stuck in service-unavailable.
        Gio.BusNameWatcherFlags.AUTO_START,
        onAppeared,
        onVanished);
}

function defaultCreateProxy(connection, nameOwner, cancellable) {
    return new Promise((resolve, reject) => {
        Gio.DBusProxy.new(
            connection,
            Gio.DBusProxyFlags.DO_NOT_AUTO_START,
            null,
            nameOwner,
            OBJECT_PATH,
            INTERFACE_NAME,
            cancellable,
            (_source, result) => {
                try {
                    resolve(Gio.DBusProxy.new_finish(result));
                } catch (error) {
                    reject(error);
                }
            });
    });
}

function defaultScheduleIdle(callback) {
    return GLib.idle_add_once(GLib.PRIORITY_DEFAULT_IDLE, callback);
}

function defaultCallProxy(proxy, cancellable) {
    return new Promise((resolve, reject) => {
        proxy.call(
            'GetActiveSessions',
            null,
            Gio.DBusCallFlags.NONE,
            -1,
            cancellable,
            (_source, result) => {
                try {
                    resolve(proxy.call_finish(result));
                } catch (error) {
                    reject(error);
                }
            });
    });
}

function isCancelled(error) {
    return error?.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED) ?? false;
}

function readCachedProperty(proxy, name, signature, fallback) {
    const value = proxy.get_cached_property(name);
    if (!value || value.get_type_string() !== signature)
        return fallback;

    return value.unpack();
}

export class DbusClient {
    constructor(onStateChanged, dependencies = {}) {
        this._onStateChanged = onStateChanged;
        this._watchName = dependencies.watchName ?? defaultWatchName;
        this._unwatchName = dependencies.unwatchName ?? Gio.bus_unwatch_name;
        this._createProxy = dependencies.createProxy ?? defaultCreateProxy;
        this._callProxy = dependencies.callProxy ?? defaultCallProxy;
        this._scheduleIdle = dependencies.scheduleIdle ?? defaultScheduleIdle;
        this._cancelSource = dependencies.cancelSource ?? GLib.Source.remove;
        this._onError = dependencies.onError ??
            (error => console.warn(`LensGuard D-Bus client: ${error.message}`));

        this._watchId = 0;
        this._proxy = null;
        this._proxySignalIds = [];
        this._cancellable = null;
        this._generation = 0;
        this._refreshSourceId = 0;
        this._refreshInFlight = false;
        this._refreshAgain = false;
        this._lastStateKey = null;
        this._running = false;
    }

    start() {
        if (this._running)
            return;

        this._running = true;
        this._publishUnavailable();
        this._watchId = this._watchName(
            (connection, _name, nameOwner) => {
                void this._onNameAppeared(connection, nameOwner);
            },
            () => this._onNameVanished());
    }

    stop() {
        if (!this._running)
            return;

        this._running = false;
        this._generation++;
        if (this._watchId)
            this._unwatchName(this._watchId);
        this._watchId = 0;

        this._detachProxy();
        this._cancelScheduledRefresh();
        this._refreshInFlight = false;
        this._refreshAgain = false;
        this._onStateChanged = null;
        this._lastStateKey = null;
    }

    async _onNameAppeared(connection, nameOwner) {
        if (!this._running)
            return;

        this._generation++;
        const generation = this._generation;
        this._detachProxy();
        this._cancelScheduledRefresh();
        this._refreshInFlight = false;
        this._refreshAgain = false;

        this._cancellable = new Gio.Cancellable();
        const cancellable = this._cancellable;
        this._publish(createViewState({
            serviceAvailable: true,
            backendAvailable: false,
            sessions: [],
        }));

        try {
            const proxy = await this._createProxy(
                connection, nameOwner, cancellable);
            if (!this._isCurrent(generation) || cancellable.is_cancelled())
                return;

            this._proxy = proxy;
            this._proxySignalIds = [
                proxy.connect('g-properties-changed',
                    (_proxy, changed, invalidated) =>
                        this._onPropertiesChanged(changed, invalidated)),
                proxy.connect('g-signal',
                    (_proxy, _sender, signalName, _parameters) =>
                        this._onSignal(signalName)),
            ];
            this._scheduleRefresh();
        } catch (error) {
            if (!this._isCurrent(generation) || isCancelled(error))
                return;

            this._onError(error);
            this._publishUnavailable();
        }
    }

    _onNameVanished() {
        if (!this._running)
            return;

        this._generation++;
        this._detachProxy();
        this._cancelScheduledRefresh();
        this._refreshInFlight = false;
        this._refreshAgain = false;
        this._publishUnavailable();
    }

    _onPropertiesChanged(changed, invalidated) {
        const changedNames = Object.keys(changed.deepUnpack());
        if (changedNames.some(name => STATE_PROPERTIES.has(name)) ||
            invalidated.some(name => STATE_PROPERTIES.has(name)))
            this._scheduleRefresh();
    }

    _onSignal(signalName) {
        if (STATE_SIGNALS.has(signalName))
            this._scheduleRefresh();
    }

    _scheduleRefresh() {
        if (!this._running || !this._proxy)
            return;

        if (this._refreshInFlight) {
            this._refreshAgain = true;
            return;
        }

        if (this._refreshSourceId)
            return;

        this._refreshSourceId = this._scheduleIdle(() => {
            this._refreshSourceId = 0;
            void this._refresh(this._generation);
        });
    }

    async _refresh(generation) {
        if (!this._isCurrent(generation) || !this._proxy)
            return;

        const proxy = this._proxy;
        const cancellable = this._cancellable;
        this._refreshInFlight = true;
        this._refreshAgain = false;

        // Reading all three cached properties is part of every synchronization.
        this._readProperties(proxy);
        try {
            const reply = await this._callProxy(proxy, cancellable);
            if (!this._isCurrent(generation) || proxy !== this._proxy)
                return;

            const properties = this._readProperties(proxy);
            const snapshot = normalizeDbusSnapshot(
                properties,
                reply.recursiveUnpack());
            this._publish(createViewState(snapshot));
        } catch (error) {
            if (!this._isCurrent(generation) || isCancelled(error))
                return;

            this._onError(error);
            this._publish(createViewState({
                serviceAvailable: true,
                backendAvailable: false,
                sessions: [],
            }));
        } finally {
            if (this._isCurrent(generation) && proxy === this._proxy) {
                this._refreshInFlight = false;
                if (this._refreshAgain)
                    this._scheduleRefresh();
            }
        }
    }

    _readProperties(proxy) {
        return {
            serviceAvailable: true,
            active: readCachedProperty(proxy, 'Active', 'b', false),
            activeSessionCount: readCachedProperty(
                proxy, 'ActiveSessionCount', 'u', 0),
            backendAvailable: readCachedProperty(
                proxy, 'BackendAvailable', 'b', false),
        };
    }

    _publishUnavailable() {
        this._publish(createViewState({
            serviceAvailable: false,
            backendAvailable: false,
            sessions: [],
        }));
    }

    _publish(state) {
        const stateKey = JSON.stringify(state);
        if (stateKey === this._lastStateKey)
            return;

        this._lastStateKey = stateKey;
        this._onStateChanged?.(state);
    }

    _isCurrent(generation) {
        return this._running && generation === this._generation;
    }

    _cancelScheduledRefresh() {
        if (this._refreshSourceId)
            this._cancelSource(this._refreshSourceId);
        this._refreshSourceId = 0;
    }

    _detachProxy() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._disconnectProxy(this._proxy);
        this._proxy = null;
        this._proxySignalIds = [];
    }

    _disconnectProxy(proxy) {
        if (!proxy)
            return;

        for (const signalId of this._proxySignalIds) {
            if (signalId)
                proxy.disconnect(signalId);
        }
    }
}
