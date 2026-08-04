// SPDX-License-Identifier: MIT

import {MockDataProvider} from './fixtures/mockDataProvider.js';

class FakeSettings {
    constructor(value) {
        this._value = value;
        this._handlers = new Map();
        this.connectCount = 0;
        this.disconnectCount = 0;
    }

    connect(signal, callback) {
        if (signal !== 'changed::mock-state')
            throw new Error(`unexpected signal ${signal}`);

        this.connectCount++;
        const id = this.connectCount;
        this._handlers.set(id, callback);
        return id;
    }

    disconnect(id) {
        if (!this._handlers.delete(id))
            throw new Error(`unknown signal ID ${id}`);
        this.disconnectCount++;
    }

    get_string(key) {
        if (key !== 'mock-state')
            throw new Error(`unexpected key ${key}`);
        return this._value;
    }

    setMockState(value) {
        this._value = value;
        [...this._handlers.values()].forEach(callback => callback());
    }
}

function assertEqual(actual, expected, message) {
    if (actual !== expected)
        throw new Error(`${message}: expected ${expected}, got ${actual}`);
}

const settings = new FakeSettings('inactive');
const states = [];
const provider = new MockDataProvider(settings, state => states.push(state));

provider.start();
provider.start();
assertEqual(settings.connectCount, 1, 'start is idempotent');
assertEqual(states.length, 1, 'start publishes exactly one initial state');
assertEqual(states[0].status, 'inactive', 'initial state is inactive');

settings.setMockState('active-multiple');
assertEqual(states.length, 2, 'settings change publishes once');
assertEqual(states[1].sessions.length, 2, 'multiple mock sessions are published');

provider.stop();
assertEqual(settings.disconnectCount, 1, 'stop disconnects the signal');
settings.setMockState('active-one');
assertEqual(states.length, 2, 'stopped provider receives no changes');

print('Mock data provider lifecycle tests passed.');
