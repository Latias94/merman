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

test('portable highlights compile and execute in the source binding', () => {
  const profile = Mermaid.queryProfiles.portable.highlights;
  const query = new Parser.Query(Mermaid, profile.source);
  const parser = new Parser();
  parser.setLanguage(Mermaid);
  const tree = parser.parse('flowchart TD\n  A --> B\n');
  const captures = query.captures(tree.rootNode);

  assert.ok(captures.some((capture) => capture.name === 'keyword'));
});

test('portable highlights match their artifact receipt profile', () => {
  const profile = Mermaid.queryProfiles.portable.highlights;
  assert.equal(profile.relativePath, 'queries/portable/highlights.scm');
  const receiptProfile = Mermaid.artifactReceipt.queryProfiles.find(
    (item) =>
      item.profile === 'portable' &&
      item.surface === 'highlights' &&
      item.path === profile.relativePath,
  );
  assert.ok(receiptProfile);
  const digest = crypto.createHash('sha256').update(fs.readFileSync(profile.path)).digest('hex');
  assert.equal(receiptProfile.sha256, digest);
});
