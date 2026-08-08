// SPDX-License-Identifier: GPL-3.0-or-later

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {
    INTERFACE_NAME,
    OBJECT_PATH,
    SERVICE_NAME,
} from '../../src/dbusClient.js';

const INTERFACE_XML = `
<node>
  <interface name="${INTERFACE_NAME}">
    <method name="GetActiveSessions">
      <arg name="sessions" type="a(sssssstu)" direction="out"/>
    </method>
    <method name="Ping">
      <arg name="response" type="s" direction="out"/>
    </method>
    <property name="Active" type="b" access="read"/>
    <property name="ActiveSessionCount" type="u" access="read"/>
    <property name="BackendAvailable" type="b" access="read"/>
    <property name="Version" type="s" access="read"/>
    <signal name="StateChanged">
      <arg name="active" type="b"/>
      <arg name="active_session_count" type="u"/>
      <arg name="backend_available" type="b"/>
    </signal>
    <signal name="SessionStarted">
      <arg name="session" type="(sssssstu)"/>
    </signal>
    <signal name="SessionStopped">
      <arg name="session_id" type="s"/>
    </signal>
    <signal name="BackendAvailabilityChanged">
      <arg name="available" type="b"/>
    </signal>
  </interface>
</node>`;

export function createSessionTuple({
    sessionId,
    applicationId = '',
    applicationName,
    deviceId = 'camera',
    deviceName,
    startedAtUnixMs = 1n,
    processId = 0,
}) {
    return [
        sessionId,
        applicationId,
        applicationName,
        deviceId,
        deviceName,
        'pipewire',
        startedAtUnixMs,
        processId,
    ];
}

export class FakeCameraService {
    constructor() {
        this.sessions = [];
        this.backendAvailable = true;
        this.getActiveSessionsCalls = 0;
        this._ownerId = 0;
        this._exportedObject = null;
        this._stopping = false;
    }

    get Active() {
        return this.sessions.length > 0;
    }

    get ActiveSessionCount() {
        return this.sessions.length;
    }

    get BackendAvailable() {
        return this.backendAvailable;
    }

    get Version() {
        return 'fake-step-9';
    }

    GetActiveSessions() {
        this.getActiveSessionsCalls++;
        return this.sessions;
    }

    Ping() {
        return 'pong';
    }

    start(initialSessions = []) {
        if (this._ownerId)
            throw new Error('fake camera service is already running');

        this.sessions = initialSessions.map(tuple => [...tuple]);
        this.backendAvailable = true;
        this._stopping = false;

        return new Promise((resolve, reject) => {
            this._ownerId = Gio.bus_own_name(
                Gio.BusType.SESSION,
                SERVICE_NAME,
                Gio.BusNameOwnerFlags.NONE,
                connection => {
                    this._exportedObject =
                        Gio.DBusExportedObject.wrapJSObject(INTERFACE_XML, this);
                    this._exportedObject.export(connection, OBJECT_PATH);
                },
                () => resolve(),
                () => {
                    if (!this._stopping)
                        reject(new Error('fake camera service could not own its name'));
                });
        });
    }

    stop() {
        if (!this._ownerId)
            return;

        this._stopping = true;
        const ownerId = this._ownerId;
        this._ownerId = 0;
        Gio.bus_unown_name(ownerId);
        this._exportedObject?.unexport();
        this._exportedObject = null;
    }

    startSession(tuple) {
        const previous = this._propertySnapshot();
        const sessionId = tuple[0];
        const existingIndex = this.sessions.findIndex(
            session => session[0] === sessionId);
        if (existingIndex >= 0)
            this.sessions[existingIndex] = [...tuple];
        else
            this.sessions.push([...tuple]);

        this._emitPropertyChanges(previous);
        this._exportedObject.emit_signal(
            'SessionStarted',
            new GLib.Variant('((sssssstu))', [tuple]));
        this.emitStateChanged();
    }

    stopSession(sessionId) {
        const previous = this._propertySnapshot();
        this.sessions = this.sessions.filter(session => session[0] !== sessionId);
        this._emitPropertyChanges(previous);
        this._exportedObject.emit_signal(
            'SessionStopped',
            new GLib.Variant('(s)', [sessionId]));
        this.emitStateChanged();
    }

    setBackendAvailable(available) {
        const previous = this._propertySnapshot();
        this.backendAvailable = available;
        if (!available)
            this.sessions = [];
        this._emitPropertyChanges(previous);
        this._exportedObject.emit_signal(
            'BackendAvailabilityChanged',
            new GLib.Variant('(b)', [available]));
        this.emitStateChanged();
    }

    emitStateChanged() {
        this._exportedObject.emit_signal(
            'StateChanged',
            new GLib.Variant('(bub)', [
                this.Active,
                this.ActiveSessionCount,
                this.BackendAvailable,
            ]));
    }

    _propertySnapshot() {
        return {
            active: this.Active,
            activeSessionCount: this.ActiveSessionCount,
            backendAvailable: this.BackendAvailable,
        };
    }

    _emitPropertyChanges(previous) {
        if (previous.active !== this.Active) {
            this._exportedObject.emit_property_changed(
                'Active', GLib.Variant.new_boolean(this.Active));
        }
        if (previous.activeSessionCount !== this.ActiveSessionCount) {
            this._exportedObject.emit_property_changed(
                'ActiveSessionCount',
                GLib.Variant.new_uint32(this.ActiveSessionCount));
        }
        if (previous.backendAvailable !== this.BackendAvailable) {
            this._exportedObject.emit_property_changed(
                'BackendAvailable',
                GLib.Variant.new_boolean(this.BackendAvailable));
        }
    }
}
