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
  assert.ok(actualPages >= memory.initialPages);
  assert.ok(actualPages <= memory.maxPeakPages);
}

function commonShortStatementFlowchart(targetBytes) {
  const header = 'flowchart TD\n';
  const statement = '  A --> B\n';
  return header + statement.repeat(
    Math.floor((targetBytes - Buffer.byteLength(header)) / Buffer.byteLength(statement)),
  );
}

function longLabelFlowchart(targetBytes) {
  const statement = `A[${'x'.repeat(1000)}]\n`;
  let source = 'flowchart TD\n';
  while (Buffer.byteLength(source) + Buffer.byteLength(statement) <= targetBytes) {
    source += statement;
  }
  const remaining = targetBytes - Buffer.byteLength(source);
  if (remaining >= 4) source += `A[${'x'.repeat(remaining - 4)}]\n`;
  return source;
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
  const commonShortStatements = commonShortStatementFlowchart(256 * 1024);
  assert.ok(Buffer.byteLength(commonShortStatements) >= 256 * 1024 - 16);
  const commonShortTree = parser.parse(commonShortStatements);
  assert.equal(commonShortTree.rootNode.hasError, false);
  commonShortTree.delete();

  const doublingStress = longLabelFlowchart(1024 * 1024);
  assert.equal(Buffer.byteLength(doublingStress), 1024 * 1024);
  const doublingTree = parser.parse(doublingStress);
  assert.equal(doublingTree.rootNode.hasError, false);
  doublingTree.delete();
  parser.delete();
  assertRuntimeMemoryPages();
});

test('portable queries compile and highlights execute against the language WASM', async () => {
  const language = await languagePromise;
  for (const [surfaceName, profile] of Object.entries(wasmBinding.queryProfiles.portable)) {
    const candidate = new Query(language, profile.source);
    candidate.delete();
    assert.ok(profile.source.length > 0, `portable/${surfaceName}`);
  }

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

test('all WASM query profiles match their artifact receipt entries', () => {
  let exposed = 0;
  for (const [profileName, surfaces] of Object.entries(wasmBinding.queryProfiles)) {
    for (const [surfaceName, profile] of Object.entries(surfaces)) {
      exposed += 1;
      const receiptProfile = wasmBinding.artifactReceipt.queryProfiles.find(
        (item) =>
          item.profile === profileName &&
          item.surface === surfaceName &&
          item.path === profile.relativePath,
      );
      assert.ok(receiptProfile, `${profileName}/${surfaceName}`);
      const digest = crypto.createHash('sha256').update(fs.readFileSync(profile.path)).digest('hex');
      assert.equal(receiptProfile.sha256, digest, `${profileName}/${surfaceName}`);
    }
  }
  assert.equal(exposed, wasmBinding.artifactReceipt.queryProfiles.length);
});
