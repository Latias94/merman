'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const test = require('node:test');
const Parser = require('tree-sitter');
const Mermaid = require('./index.js');
const familyFixtures = require('../../metadata/fixtures/family-roots.json');

test('source binding loads the ABI-14 language and parses all public families', () => {
  const parser = new Parser();
  parser.setLanguage(Mermaid);

  assert.equal(familyFixtures.length, 35);
  for (const fixture of familyFixtures) {
    const tree = parser.parse(fixture.source);
    assert.equal(tree.rootNode.type, 'source_file', fixture.publicId);
    assert.equal(tree.rootNode.hasError, false, fixture.publicId);
    const roots = tree.rootNode.namedChildren.filter((node) => node.type.endsWith('_diagram'));
    assert.equal(roots.length, 1, fixture.publicId);
    assert.equal(roots[0].type, fixture.root, fixture.publicId);
  }
  assert.equal(Mermaid.name, 'mermaid');
  assert.ok(Array.isArray(Mermaid.nodeTypeInfo));
  assert.equal(Mermaid.artifactReceipt.language.abi, 14);
});

test('portable queries compile and highlights execute in the source binding', () => {
  for (const [surfaceName, profile] of Object.entries(Mermaid.queryProfiles.portable)) {
    assert.doesNotThrow(
      () => new Parser.Query(Mermaid, profile.source),
      `portable/${surfaceName}`,
    );
  }

  const profile = Mermaid.queryProfiles.portable.highlights;
  const query = new Parser.Query(Mermaid, profile.source);
  const parser = new Parser();
  parser.setLanguage(Mermaid);
  const tree = parser.parse('flowchart TD\n  A --> B\n');
  const captures = query.captures(tree.rootNode);

  assert.ok(captures.some((capture) => capture.name === 'keyword'));
});

test('all source-binding query profiles match their artifact receipt entries', () => {
  const exposed = [];
  for (const [profileName, surfaces] of Object.entries(Mermaid.queryProfiles)) {
    for (const [surfaceName, profile] of Object.entries(surfaces)) {
      exposed.push(`${profileName}/${surfaceName}`);
      const receiptProfile = Mermaid.artifactReceipt.queryProfiles.find(
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
  assert.equal(exposed.length, Mermaid.artifactReceipt.queryProfiles.length);
});
