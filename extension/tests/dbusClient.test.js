// SPDX-License-Identifier: MIT

import GLib from 'gi://GLib';

import {DbusClient} from '../src/dbusClient.js';

function assert(condition, message) {
    if (!condition)
        throw new Error(message);
}

function assertEqual(actual, expected, message) {
    if (actual !== expected)
        throw new Error(`${message}: expected ${expected}, got ${actual}`);
}

function delay(milliseconds) {
    return new Promise(resolve => {
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, milliseconds, () => {
            resolve();
            return GLib.SOURCE_REMOVE;
        });
    });
}

async function waitFor(predicate, message) {
    for (let attempt = 0; attempt < 100; attempt++) {
        if (predicate())
            return;
        await delay(5);
    }

    throw new Error(message);
}

class FakeProxy {
    constructor() {
        this.properties = {
            Active: true,
            ActiveSessionCount: 1,
            BackendAvailable: true,
        };
        this.sessions = [[
            'initial', '', 'Initial app', 'camera', 'Initial camera',
            'pipewire', 1n, 7,
        ]];
        this.callCount = 0;
        this.disconnectCount = 0;
        this._nextSignalId = 1;
        this._handlers = new Map();
    }

    connect(signalName, callback) {
        const signalId = this._nextSignalId++;
        this._handlers.set(signalId, {signalName, callback});
        return signalId;
    }

    disconnect(signalId) {
        assert(this._handlers.delete(signalId),
            `disconnect received unknown signal ${signalId}`);
        this.disconnectCount++;
    }

    get_cached_property(name) {
        const value = this.properties[name];
        if (name === 'Active' || name === 'BackendAvailable')
            return GLib.Variant.new_boolean(value);
        if (name === 'ActiveSessionCount')
            return GLib.Variant.new_uint32(value);
        return null;
    }

    async call(methodName, _parameters, _flags, _timeout, cancellable) {
        assertEqual(methodName, 'GetActiveSessions', 'client calls contract method');
        assert(!cancellable.is_cancelled(), 'active refresh is not cancelled');
        this.callCount++;
        return new GLib.Variant('(a(sssssstu))', [this.sessions]);
    }

    emitStateSignal() {
        for (const {signalName, callback} of this._handlers.values()) {
            if (signalName === 'g-signal')
                callback(this, ':1.fake', 'StateChanged', null);
        }
    }

    emitPropertyChange() {
        const changed = {
            deepUnpack: () => ({
                Active: GLib.Variant.new_boolean(this.properties.Active),
            }),
        };
        for (const {signalName, callback} of this._handlers.values()) {
            if (signalName === 'g-properties-changed')
                callback(this, changed, []);
        }
    }
}

let appearedCallback = null;
let vanishedCallback = null;
let unwatchCount = 0;
const proxy = new FakeProxy();
const states = [];
const errors = [];
const client = new DbusClient(state => states.push(state), {
    watchName: (appeared, vanished) => {
        appearedCallback = appeared;
        vanishedCallback = vanished;
        return 99;
    },
    unwatchName: watchId => {
        assertEqual(watchId, 99, 'client returns the name watch');
        unwatchCount++;
    },
    createProxy: async () => proxy,
    callProxy: (activeProxy, cancellable) => activeProxy.call(
        'GetActiveSessions', null, 0, -1, cancellable),
    onError: error => errors.push(error),
});

client.start();
client.start();
assertEqual(states.length, 1, 'start publishes one unavailable state');
assertEqual(states[0].status, 'service-unavailable',
    'client starts fail-safe while service is absent');

appearedCallback({}, 'io.github.younesrabeh.CameraMonitor', ':1.fake');
await waitFor(() => states.some(state => state.status === 'active'),
    'initial synchronization did not publish active state');
assertEqual(proxy.callCount, 1, 'initial synchronization fetches sessions once');
assertEqual(states.at(-1).sessions[0].applicationName, 'Initial app',
    'initial method payload reaches the view state');

proxy.emitStateSignal();
proxy.emitStateSignal();
proxy.emitPropertyChange();
await waitFor(() => proxy.callCount === 2,
    'signal burst did not trigger a refresh');
await delay(20);
assertEqual(proxy.callCount, 2, 'signal burst is coalesced into one refresh');

vanishedCallback();
assertEqual(states.at(-1).status, 'service-unavailable',
    'service disappearance publishes unavailable state');
assertEqual(states.at(-1).sessions.length, 0,
    'service disappearance clears stale sessions');
assertEqual(proxy.disconnectCount, 2,
    'service disappearance disconnects proxy subscriptions');

for (let cycle = 0; cycle < 25; cycle++) {
    const expectedCalls = proxy.callCount + 1;
    appearedCallback({}, 'io.github.younesrabeh.CameraMonitor',
        `:1.reconnect-${cycle}`);
    await waitFor(() => proxy.callCount === expectedCalls,
        `reconnect ${cycle + 1} did not synchronize`);
    vanishedCallback();
    assertEqual(proxy._handlers.size, 0,
        `reconnect ${cycle + 1} leaked proxy subscriptions`);
}
assertEqual(proxy.disconnectCount, 52,
    'each service generation disconnects both subscriptions');

client.stop();
client.stop();
assertEqual(unwatchCount, 1, 'stop returns the name watch exactly once');
assertEqual(errors.length, 0, 'normal lifecycle produces no client errors');

print('D-Bus client synchronization and lifecycle tests passed.');
