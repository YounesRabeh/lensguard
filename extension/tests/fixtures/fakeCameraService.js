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
      <arg name="sessions" type="a(ssssstu)" direction="out"/>
    </method>
    <method name="Ping">
      <arg name="response" type="s" direction="out"/>
    </method>
    <property name="Active" type="b" access="read"/>
    <property name="ActiveSessionCount" type="u" access="read"/>
    <property name="ObserverAvailable" type="b" access="read"/>
    <property name="ObserverAvailability" type="s" access="read"/>
    <property name="ObserverStatusDetail" type="s" access="read"/>
    <property name="UnknownCameraActivity" type="b" access="read"/>
    <property name="SuppressedBrokerEvents" type="t" access="read"/>
    <property name="SuppressedUnknownEvents" type="t" access="read"/>
    <property name="Version" type="s" access="read"/>
    <signal name="StateChanged">
      <arg name="active" type="b"/>
      <arg name="active_session_count" type="u"/>
      <arg name="observer_available" type="b"/>
    </signal>
    <signal name="SessionStarted">
      <arg name="session" type="(ssssstu)"/>
    </signal>
    <signal name="SessionStopped">
      <arg name="session_id" type="s"/>
    </signal>
    <signal name="ObserverStatusChanged">
      <arg name="availability" type="s"/>
    </signal>
  </interface>
</node>`;

export function createSessionTuple({
    sessionId,
    applicationId = '',
    applicationName,
    deviceId = 'camera',
    deviceName,
    startedAtMonotonicNs = 1n,
    processId = 0,
}) {
    return [
        sessionId,
        applicationId,
        applicationName,
        deviceId,
        deviceName,
        startedAtMonotonicNs,
        processId,
    ];
}

export class FakeCameraService {
    constructor() {
        this.sessions = [];
        this.observerAvailable = true;
        this.observerAvailability = 'available';
        this.unknownCameraActivity = false;
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

    get ObserverAvailable() {
        return this.observerAvailable;
    }

    get ObserverAvailability() {
        return this.observerAvailability;
    }

    get ObserverStatusDetail() {
        return '';
    }

    get UnknownCameraActivity() {
        return this.unknownCameraActivity;
    }

    get SuppressedBrokerEvents() {
        return 0n;
    }

    get SuppressedUnknownEvents() {
        return this.unknownCameraActivity ? 1n : 0n;
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
        this.observerAvailable = true;
        this.observerAvailability = 'available';
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
            new GLib.Variant('((ssssstu))', [tuple]));
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

    setObserverAvailable(available) {
        const previous = this._propertySnapshot();
        this.observerAvailable = available;
        this.observerAvailability = available ? 'available' : 'backend-lost';
        if (!available)
            this.sessions = [];
        this._emitPropertyChanges(previous);
        this._exportedObject.emit_signal(
            'ObserverStatusChanged',
            new GLib.Variant('(s)', [this.observerAvailability]));
        this.emitStateChanged();
    }

    emitStateChanged() {
        this._exportedObject.emit_signal(
            'StateChanged',
            new GLib.Variant('(bub)', [
                this.Active,
                this.ActiveSessionCount,
                this.ObserverAvailable,
            ]));
    }

    _propertySnapshot() {
        return {
            active: this.Active,
            activeSessionCount: this.ActiveSessionCount,
            observerAvailable: this.ObserverAvailable,
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
        if (previous.observerAvailable !== this.ObserverAvailable) {
            this._exportedObject.emit_property_changed(
                'ObserverAvailable',
                GLib.Variant.new_boolean(this.ObserverAvailable));
            this._exportedObject.emit_property_changed(
                'ObserverAvailability',
                GLib.Variant.new_string(this.ObserverAvailability));
        }
    }
}
