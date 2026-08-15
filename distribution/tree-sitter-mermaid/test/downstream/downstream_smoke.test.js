'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const {
  canonicalPlatformKey,
  computeSha256,
  parseLatestTag,
  readTomlArray,
  readTomlString,
  resolveAsset,
} = require('../../scripts/downstream_smoke');

const matrix = JSON.parse(fs.readFileSync(
  path.join(__dirname, 'matrix.json'),
  'utf8',
));

test('platform keys use release-asset vocabulary', () => {
  assert.equal(canonicalPlatformKey('darwin', 'arm64'), 'darwin-arm64');
  assert.equal(canonicalPlatformKey('linux', 'x64'), 'linux-x64');
  assert.equal(canonicalPlatformKey('win32', 'x64'), 'win32-x64');
  assert.throws(() => canonicalPlatformKey('freebsd', 'x64'), /unsupported platform/);
});

test('asset resolution is explicit and rejects unsupported combinations', () => {
  const nvim = resolveAsset(matrix.editors.neovim, 'darwin-arm64', 'Neovim');
  assert.equal(nvim.sha256.length, 64);
  assert.match(nvim.url, /\/v0\.12\.4\//);
  assert.throws(
    () => resolveAsset(matrix.editors.helix, 'win32-arm64', 'Helix'),
    /no fixed Helix asset/,
  );
});

test('sha256 and latest redirect parsing are deterministic', () => {
  assert.equal(
    computeSha256(Buffer.from('mermaid\n')),
    '0caf9a7e62691e0707f4dc9e5203074f7abfa3f5c32476743773b83b090d94fd',
  );
  assert.equal(
    parseLatestTag('https://github.com/neovim/neovim/releases/tag/v0.12.4'),
    'v0.12.4',
  );
  assert.equal(
    parseLatestTag('https://github.com/zed-industries/zed/releases/tag/v1.15.0'),
    'v1.15.0',
  );
  assert.equal(parseLatestTag('https://example.com/no-tag'), null);
});

test('narrow TOML reader extracts the Zed fixture contract', () => {
  const extension = fs.readFileSync(
    path.join(__dirname, 'zed', 'extension.toml'),
    'utf8',
  );
  const language = fs.readFileSync(
    path.join(__dirname, 'zed', 'languages', 'mermaid', 'config.toml'),
    'utf8',
  );
  assert.equal(readTomlString(extension, 'id'), 'mermaid');
  assert.match(readTomlString(extension, 'rev', 'grammars.mermaid'), /^[0-9a-f]{40}$/);
  assert.equal(readTomlString(extension, 'path', 'grammars.mermaid'), 'distribution/tree-sitter-mermaid');
  assert.equal(readTomlString(language, 'grammar'), 'mermaid');
  assert.deepEqual(readTomlArray(language, 'path_suffixes'), ['mmd', 'mermaid']);
});
