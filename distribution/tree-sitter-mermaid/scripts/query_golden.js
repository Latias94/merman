'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const Parser = require('tree-sitter');
const Mermaid = require('../bindings/node');

const packageRoot = path.join(__dirname, '..');
const goldenRoot = path.join(
  packageRoot,
  'test',
  'queries',
  'portable',
  'highlights',
);
const querySource = fs.readFileSync(
  path.join(packageRoot, 'queries', 'portable', 'highlights.scm'),
  'utf8',
);

function normalizeCaptures(captures, source) {
  const compareText = (left, right) => (left < right ? -1 : left > right ? 1 : 0);
  return captures
    .map(({ name, node }) => {
      const text = source.slice(node.startIndex, node.endIndex);
      return {
        name,
        text,
        startByte: Buffer.byteLength(source.slice(0, node.startIndex)),
        endByte: Buffer.byteLength(source.slice(0, node.endIndex)),
      };
    })
    .sort((left, right) => (
      left.startByte - right.startByte
      || left.endByte - right.endByte
      || compareText(left.name, right.name)
      || compareText(left.text, right.text)
    ));
}

function sourceFiles() {
  return fs.readdirSync(goldenRoot)
    .filter((name) => name.endsWith('.captures.json'))
    .sort()
    .map((name) => path.join(goldenRoot, name));
}

function verifyGolden(goldenPath, parser, query) {
  const golden = JSON.parse(fs.readFileSync(goldenPath, 'utf8'));
  assert.equal(golden.schemaVersion, 1, `${goldenPath}: schemaVersion`);
  assert.equal(golden.profile, 'portable', `${goldenPath}: profile`);
  assert.equal(golden.surface, 'highlights', `${goldenPath}: surface`);
  assert.equal(typeof golden.source, 'string', `${goldenPath}: source`);
  assert.ok(!golden.source.includes('/') && !golden.source.includes('\\'), `${goldenPath}: source path`);
  const sourcePath = path.join(path.dirname(goldenPath), golden.source);
  assert.equal(path.dirname(sourcePath), path.dirname(goldenPath), `${goldenPath}: source escapes golden directory`);
  const source = fs.readFileSync(sourcePath, 'utf8');
  const tree = parser.parse(source);
  assert.equal(tree.rootNode.type, 'source_file', goldenPath);
  assert.equal(tree.rootNode.hasError, false, `${goldenPath}: source has parse errors`);
  const roots = tree.rootNode.namedChildren.filter((node) => node.type.endsWith('_diagram'));
  assert.equal(roots.length, 1, `${goldenPath}: expected one family root`);
  assert.ok(Array.isArray(golden.captures) && golden.captures.length > 0, `${goldenPath}: captures`);
  const actual = normalizeCaptures(query.captures(tree.rootNode), source);
  assert.deepEqual(actual, golden.captures, `${goldenPath}: capture golden drifted`);
  const fragmentPath = goldenPath.replace(/\.captures\.json$/, '.scm');
  if (fs.existsSync(fragmentPath)) {
    const fragment = new Parser.Query(Mermaid, fs.readFileSync(fragmentPath, 'utf8'));
    const fragmentActual = normalizeCaptures(fragment.captures(tree.rootNode), source);
    assert.deepEqual(
      fragmentActual,
      golden.captures,
      `${fragmentPath}: family query fragment drifted from its golden`,
    );
  }
  assert.ok(
    actual.some(({ name }) => name !== 'keyword' && name !== 'comment' && name !== 'attribute'),
    `${goldenPath}: golden must include a family-owned or structural capture`,
  );
}

function main() {
  const files = sourceFiles();
  assert.ok(files.length > 0, 'portable query golden directory must contain a golden');
  const parser = new Parser();
  parser.setLanguage(Mermaid);
  const query = new Parser.Query(Mermaid, querySource);
  for (const file of files) verifyGolden(file, parser, query);
  process.stdout.write(`portable query goldens: ${files.length} passed\n`);
}

main();
