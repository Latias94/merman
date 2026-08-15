'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const PROFILE_NAMES = new Set(['portable', 'neovim', 'helix', 'zed']);
const SURFACE_NAMES = new Set([
  'highlights',
  'folds',
  'indents',
  'injections',
  'locals',
  'tags',
  'brackets',
  'outline',
  'textobjects',
]);

function loadQueryProfiles(packageRoot, receipt) {
  assert.ok(Array.isArray(receipt.queryProfiles), 'artifact receipt must list query profiles');
  const result = {};

  for (const item of receipt.queryProfiles) {
    assert.ok(PROFILE_NAMES.has(item.profile), `unknown query profile ${item.profile}`);
    assert.ok(SURFACE_NAMES.has(item.surface), `unknown query surface ${item.surface}`);
    const expectedPath = `queries/${item.profile}/${item.surface}.scm`;
    assert.equal(item.path, expectedPath, `query profile path for ${item.profile}/${item.surface}`);
    assert.equal(result[item.profile]?.[item.surface], undefined, `duplicate query profile ${item.path}`);

    const queryPath = path.resolve(packageRoot, item.path);
    assert.ok(
      queryPath.startsWith(`${path.resolve(packageRoot)}${path.sep}`),
      `query profile escapes package root: ${item.path}`,
    );
    const source = fs.readFileSync(queryPath, 'utf8');
    assert.equal(Buffer.byteLength(source), item.bytes, `query profile byte length: ${item.path}`);
    const digest = crypto.createHash('sha256').update(source).digest('hex');
    assert.equal(digest, item.sha256, `query profile digest: ${item.path}`);

    result[item.profile] ??= {};
    result[item.profile][item.surface] = Object.freeze({
      relativePath: item.path,
      path: queryPath,
      source,
    });
  }

  for (const [profile, surfaces] of Object.entries(result)) {
    result[profile] = Object.freeze(surfaces);
  }
  return Object.freeze(result);
}

module.exports = { loadQueryProfiles };
