'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const test = require('node:test');
const { Language, Parser, Query } = require('web-tree-sitter');
const familyFixtures = require('../../metadata/fixtures/family-roots.json');
const mechanicsMetrics = require('../../metadata/metrics/u2-mechanics.json');
const wasmBinding = require('../wasm');

const runtimeMemory = new WebAssembly.Memory({ initial: 512, maximum: 32768 });
const languagePromise = (async () => {
  await Parser.init({ wasmMemory: runtimeMemory });
  return Language.load(wasmBinding.languagePath);
})();

function assertRuntimeMemoryPages() {
  const memory = mechanicsMetrics.observed.wasmRuntimeMemoryPages;
  const actualPages = runtimeMemory.buffer.byteLength / 65536;
  assert.equal(memory.initialPages, 512);
  assert.equal(actualPages, memory.observedPeakPages);
  assert.ok(actualPages <= memory.maxPeakPages);
}

test('language WASM loads with ABI 14 and parses all public families', async () => {
  const wasmPath = wasmBinding.languagePath;
  const language = await languagePromise;
  assert.equal(language.abiVersion, 14);

  const receipt = wasmBinding.artifactReceipt;
  assert.equal(receipt.receiptId.length, 64);
  const recordedWasm = receipt.artifacts.find(
    (artifact) => artifact.path === 'wasm/tree-sitter-mermaid.wasm',
  );
  const wasmDigest = crypto.createHash('sha256').update(fs.readFileSync(wasmPath)).digest('hex');
  assert.equal(wasmDigest, recordedWasm.sha256);

  const parser = new Parser();
  parser.setLanguage(language);
  assert.equal(familyFixtures.length, 35);
  for (const fixture of familyFixtures) {
    const tree = parser.parse(fixture.source);
    assert.equal(tree.rootNode.type, 'source_file', fixture.publicId);
    assert.equal(tree.rootNode.hasError, false, fixture.publicId);
    const roots = tree.rootNode.namedChildren.filter((node) => node.type.endsWith('_diagram'));
    assert.equal(roots.length, 1, fixture.publicId);
    assert.equal(roots[0].type, fixture.root, fixture.publicId);
    tree.delete();
  }
  const stressHeader = 'flowchart TD\n';
  const stressStatement = '  A --> B\n';
  const stressCount = Math.ceil(
    (1024 * 1024 - Buffer.byteLength(stressHeader)) /
      Buffer.byteLength(stressStatement),
  );
  const stress = stressHeader + stressStatement.repeat(stressCount);
  assert.equal(Buffer.byteLength(stress), 1048583);
  const stressTree = parser.parse(stress);
  assert.equal(stressTree.rootNode.hasError, false);
  stressTree.delete();
  parser.delete();
  assertRuntimeMemoryPages();
});

test('portable highlights compile and execute against the language WASM', async () => {
  const language = await languagePromise;
  const profile = wasmBinding.queryProfiles.portable.highlights;
  const query = new Query(language, profile.source);
  const parser = new Parser();
  parser.setLanguage(language);
  const tree = parser.parse('flowchart TD\n  A --> B\n');
  const captures = query.captures(tree.rootNode);

  assert.ok(captures.some((capture) => capture.name === 'keyword'));
  tree.delete();
  parser.delete();
  query.delete();
  assertRuntimeMemoryPages();
});

test('WASM portable highlights match their artifact receipt profile', () => {
  const profile = wasmBinding.queryProfiles.portable.highlights;
  const receiptProfile = wasmBinding.artifactReceipt.queryProfiles.find(
    (item) =>
      item.profile === 'portable' &&
      item.surface === 'highlights' &&
      item.path === profile.relativePath,
  );
  assert.ok(receiptProfile);
  const digest = crypto.createHash('sha256').update(fs.readFileSync(profile.path)).digest('hex');
  assert.equal(receiptProfile.sha256, digest);
});
