'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { loadQueryProfiles } = require('./query-profiles');

function fixture() {
  const packageRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'tree-sitter-mermaid-query-'));
  const relativePath = 'queries/portable/highlights.scm';
  const queryPath = path.join(packageRoot, relativePath);
  const source = '(source_file) @root\n';
  fs.mkdirSync(path.dirname(queryPath), { recursive: true });
  fs.writeFileSync(queryPath, source);
  return {
    packageRoot,
    receipt: {
      queryProfiles: [{
        profile: 'portable',
        surface: 'highlights',
        path: relativePath,
        sha256: crypto.createHash('sha256').update(source).digest('hex'),
        bytes: Buffer.byteLength(source),
      }],
    },
  };
}

test('receipt-bound query profiles load as immutable source records', (context) => {
  const { packageRoot, receipt } = fixture();
  context.after(() => fs.rmSync(packageRoot, { recursive: true, force: true }));

  const profiles = loadQueryProfiles(packageRoot, receipt);
  assert.equal(profiles.portable.highlights.source, '(source_file) @root\n');
  assert.ok(Object.isFrozen(profiles));
  assert.ok(Object.isFrozen(profiles.portable));
  assert.ok(Object.isFrozen(profiles.portable.highlights));
});

test('query profile paths and digests are verified before exposure', (context) => {
  const { packageRoot, receipt } = fixture();
  context.after(() => fs.rmSync(packageRoot, { recursive: true, force: true }));

  receipt.queryProfiles[0].sha256 = '0'.repeat(64);
  assert.throws(() => loadQueryProfiles(packageRoot, receipt), /query profile digest/);

  receipt.queryProfiles[0].path = '../outside.scm';
  assert.throws(() => loadQueryProfiles(packageRoot, receipt), /query profile path/);
});
