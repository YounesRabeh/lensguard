import GLib from 'gi://GLib';

function fail(message) {
  throw new Error(`metadata validation failed: ${message}`);
}

if (ARGV.length !== 1)
  fail('expected a metadata.json path');

const [loaded, contents] = GLib.file_get_contents(ARGV[0]);
if (!loaded)
  fail(`could not read ${ARGV[0]}`);

let metadata;
try {
  metadata = JSON.parse(new TextDecoder().decode(contents));
} catch (error) {
  fail(`invalid JSON: ${error.message}`);
}

for (const field of ['uuid', 'name', 'description', 'shell-version']) {
  if (!(field in metadata))
    fail(`missing required field: ${field}`);
}

if (!Array.isArray(metadata['shell-version']) || metadata['shell-version'].length === 0)
  fail('shell-version must be a non-empty array');

if (metadata['settings-schema'] !== 'org.gnome.shell.extensions.lensguard')
  fail('missing or unexpected settings-schema');
