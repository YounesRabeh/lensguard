// SPDX-License-Identifier: MIT

import Gio from 'gi://Gio';

const SCHEMA_ID = 'org.gnome.shell.extensions.lensguard';

function assert(condition, message) {
    if (!condition)
        throw new Error(message);
}

if (ARGV.length !== 1)
    throw new Error('expected a compiled schema directory');

const source = Gio.SettingsSchemaSource.new_from_directory(
    ARGV[0], Gio.SettingsSchemaSource.get_default(), false);
const schema = source.lookup(SCHEMA_ID, false);
assert(schema, `compiled schema ${SCHEMA_ID} was not found`);

const expectedDefaults = new Map([
    ['show-panel-indicator', true],
    ['show-backend-unavailable-warning', true],
    ['show-indicator-during-backend-failure', true],
]);
for (const [key, expected] of expectedDefaults) {
    assert(schema.has_key(key), `schema is missing ${key}`);
    const schemaKey = schema.get_key(key);
    assert(schemaKey.get_default_value().unpack() === expected,
        `${key} has an unexpected default`);
    assert(schemaKey.get_summary().trim().length > 0,
        `${key} does not document its default behavior`);
    assert(schemaKey.get_description().trim().length > 0,
        `${key} does not have a useful description`);
}

print('GSettings schema defaults and documentation tests passed.');
