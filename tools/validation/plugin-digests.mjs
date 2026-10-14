import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '../../examples/plugins');
let checked = 0;
function verify(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) verify(file);
    else if (entry.name === 'webdeck.json') {
      const manifest = JSON.parse(fs.readFileSync(file, 'utf8'));
      const source = fs.readFileSync(path.resolve(directory, manifest.entry));
      const digest = createHash('sha256').update(source).digest('hex');
      assert.equal(digest, manifest.digest, `Plugin digest mismatch: ${file}`);
      checked++;
    }
  }
}
verify(root);
assert.ok(checked > 0, 'No example plugin manifests found');
console.log(`Verified ${checked} example plugin digests`);
