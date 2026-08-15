'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const Parser = require('tree-sitter');

const packageRoot = path.resolve(__dirname, '..', '..', '..');
const queryRoot = path.join(packageRoot, 'queries', 'helix');
const goldenRoot = path.join(packageRoot, 'test', 'queries', 'helix');
const portableFixtureRoot = path.join(
  packageRoot,
  'test',
  'queries',
  'portable',
  'highlights',
);

// This profile-local verifier compiles against the built language directly.
// Canonical package bindings additionally enforce artifact-receipt hashes.
const Mermaid = require(path.join(
  packageRoot,
  'build',
  'Release',
  'tree_sitter_mermaid_binding.node',
));

const SURFACES = [
  'highlights',
  'folds',
  'indents',
  'injections',
  'locals',
  'tags',
  'brackets',
  'outline',
  'textobjects',
];
const LOADED_SURFACES = [
  'highlights',
  'injections',
  'locals',
  'indents',
  'textobjects',
];
const NATIVE_NA_SURFACES = ['folds', 'tags', 'brackets', 'outline'];
const HELIX_HIGHLIGHT_CAPTURES = new Set([
  'attribute',
  'comment',
  'comment.block.documentation',
  'constant',
  'constant.builtin.boolean',
  'constant.character.escape',
  'constant.numeric',
  'function',
  'function.macro',
  'keyword',
  'keyword.operator',
  'namespace',
  'operator',
  'punctuation.bracket',
  'punctuation.delimiter',
  'punctuation.special',
  'string',
  'string.special',
  'type',
  'type.builtin',
  'variable',
  'variable.other.member',
]);
const HELIX_INDENT_CAPTURES = new Set([
  'indent',
  'outdent',
  'indent.always',
  'outdent.always',
  'align',
  'anchor',
  'extend',
  'extend.prevent-once',
]);
const HELIX_TEXTOBJECT_CAPTURES = new Set([
  'function.inside',
  'function.around',
  'class.inside',
  'class.around',
  'test.inside',
  'test.around',
  'parameter.inside',
  'parameter.around',
  'comment.inside',
  'comment.around',
  'entry.inside',
  'entry.around',
]);

function capturesInSource(querySource) {
  return new Set([...querySource.matchAll(/@([A-Za-z0-9_.-]+)/g)].map((match) => match[1]));
}

function assertCaptureVocabulary(surface, source) {
  const captures = capturesInSource(source);
  assert.ok(captures.size > 0, `${surface}: query must declare captures`);
  if (surface === 'highlights') {
    for (const capture of captures) {
      assert.ok(HELIX_HIGHLIGHT_CAPTURES.has(capture), `${surface}: unsupported @${capture}`);
    }
  } else if (surface === 'injections') {
    assert.deepEqual(captures, new Set(['injection.content']));
  } else if (surface === 'locals') {
    for (const capture of captures) {
      assert.ok(capture.startsWith('local.'), `${surface}: unsupported @${capture}`);
    }
  } else if (surface === 'indents') {
    for (const capture of captures) {
      assert.ok(HELIX_INDENT_CAPTURES.has(capture), `${surface}: unsupported @${capture}`);
    }
  } else if (surface === 'textobjects') {
    for (const capture of captures) {
      assert.ok(HELIX_TEXTOBJECT_CAPTURES.has(capture), `${surface}: unsupported @${capture}`);
    }
  }
}

function normalizeCaptures(captures, source) {
  const compareText = (left, right) => (left < right ? -1 : left > right ? 1 : 0);
  return captures
    .map(({ name, node }) => ({
      name,
      text: source.slice(node.startIndex, node.endIndex),
      startByte: Buffer.byteLength(source.slice(0, node.startIndex)),
      endByte: Buffer.byteLength(source.slice(0, node.endIndex)),
    }))
    .sort((left, right) => (
      left.startByte - right.startByte
      || left.endByte - right.endByte
      || compareText(left.name, right.name)
      || compareText(left.text, right.text)
    ));
}

function loadQueries() {
  const queries = {};
  for (const surface of LOADED_SURFACES) {
    const queryPath = path.join(queryRoot, `${surface}.scm`);
    assert.ok(fs.existsSync(queryPath), `${surface}: Helix query file is missing`);
    const source = fs.readFileSync(queryPath, 'utf8');
    assertCaptureVocabulary(surface, source);
    queries[surface] = new Parser.Query(Mermaid, source);
  }
  for (const surface of NATIVE_NA_SURFACES) {
    assert.ok(
      !fs.existsSync(path.join(queryRoot, `${surface}.scm`)),
      `${surface}: an empty or unused Helix query file would create a false contract`,
    );
  }
  return queries;
}

function verifyApplicability() {
  const contract = JSON.parse(fs.readFileSync(path.join(goldenRoot, 'applicability.json'), 'utf8'));
  assert.equal(contract.schemaVersion, 1);
  assert.equal(contract.profile, 'helix');
  assert.equal(contract.consumer.version, '25.07.1');
  assert.deepEqual(contract.consumer.loadedSurfaces, LOADED_SURFACES);
  assert.equal(contract.families.length, 35);
  assert.equal(new Set(contract.families.map(({ publicId }) => publicId)).size, 35);
  for (const family of contract.families) {
    assert.deepEqual(Object.keys(family.surfaces).sort(), [...SURFACES].sort(), family.publicId);
    for (const surface of SURFACES) {
      const cell = family.surfaces[surface];
      assert.ok(['applicable', 'not_applicable'].includes(cell.status), `${family.publicId}/${surface}`);
      if (cell.status === 'applicable') {
        assert.equal(cell.query, `queries/helix/${surface}.scm`, `${family.publicId}/${surface}`);
      } else {
        assert.ok(cell.rationale.length >= 24, `${family.publicId}/${surface}: N/A rationale`);
      }
    }
    for (const surface of NATIVE_NA_SURFACES) {
      assert.equal(family.surfaces[surface].status, 'not_applicable', `${family.publicId}/${surface}`);
    }
  }
  return contract;
}

function verifyGoldens(parser, queries) {
  const goldenPaths = [];
  for (const surface of LOADED_SURFACES) {
    const surfaceRoot = path.join(goldenRoot, surface);
    for (const name of fs.readdirSync(surfaceRoot).filter((entry) => entry.endsWith('.captures.json')).sort()) {
      goldenPaths.push(path.join(surfaceRoot, name));
    }
  }
  assert.ok(goldenPaths.length >= LOADED_SURFACES.length);
  for (const goldenPath of goldenPaths) {
    const golden = JSON.parse(fs.readFileSync(goldenPath, 'utf8'));
    assert.equal(golden.schemaVersion, 1, goldenPath);
    assert.equal(golden.profile, 'helix', goldenPath);
    assert.equal(golden.consumerVersion, '25.07.1', goldenPath);
    assert.ok(LOADED_SURFACES.includes(golden.surface), goldenPath);
    const sourcePath = path.join(path.dirname(goldenPath), golden.source);
    assert.equal(path.dirname(sourcePath), path.dirname(goldenPath), `${goldenPath}: source escapes directory`);
    const source = fs.readFileSync(sourcePath, 'utf8');
    const tree = parser.parse(source);
    assert.equal(tree.rootNode.hasError, false, `${goldenPath}: source has parse errors`);
    const actual = normalizeCaptures(queries[golden.surface].captures(tree.rootNode), source);
    assert.deepEqual(actual, golden.captures, `${goldenPath}: capture golden drifted`);
  }
  return goldenPaths.length;
}

function coverageSource(family, surface) {
  if (family.publicId === 'state' && surface === 'indents') {
    return fs.readFileSync(path.join(goldenRoot, 'indents', 'state.mmd'), 'utf8');
  }
  return fs.readFileSync(path.join(portableFixtureRoot, `${family.fixtureSlug}.mmd`), 'utf8');
}

function verifyFamilyCoverage(parser, queries, contract) {
  let applicableCells = 0;
  let notApplicableCells = 0;
  for (const family of contract.families) {
    for (const surface of SURFACES) {
      const cell = family.surfaces[surface];
      if (cell.status === 'not_applicable') {
        notApplicableCells += 1;
        continue;
      }
      applicableCells += 1;
      let source = coverageSource(family, surface);
      if (surface === 'injections') {
        source = `---\ntitle: Helix coverage\n---\n${source}`;
      }
      const tree = parser.parse(source);
      assert.equal(tree.rootNode.hasError, false, `${family.publicId}/${surface}: parse errors`);
      const names = queries[surface].captures(tree.rootNode).map(({ name }) => name);
      assert.ok(names.length > 0, `${family.publicId}/${surface}: no captures`);
      if (surface === 'highlights') {
        assert.ok(names.includes('keyword'), `${family.publicId}/highlights: no keyword`);
      } else if (surface === 'injections') {
        assert.ok(names.includes('injection.content'), `${family.publicId}/injections`);
      } else if (surface === 'locals') {
        assert.ok(names.includes('local.scope'), `${family.publicId}/locals: no scope`);
        assert.ok(names.some((name) => name.startsWith('local.definition')), `${family.publicId}/locals: no definition`);
        assert.ok(names.includes('local.reference'), `${family.publicId}/locals: no reference`);
      } else if (surface === 'indents') {
        assert.ok(names.includes('indent'), `${family.publicId}/indents: no indent`);
        assert.ok(names.includes('outdent'), `${family.publicId}/indents: no outdent`);
      } else if (surface === 'textobjects') {
        assert.ok(names.includes('class.around'), `${family.publicId}/textobjects: no around`);
        assert.ok(names.includes('class.inside'), `${family.publicId}/textobjects: no inside`);
        assert.ok(names.includes('entry.around'), `${family.publicId}/textobjects: no entry`);
      }
    }
  }
  assert.equal(applicableCells + notApplicableCells, 35 * SURFACES.length);
  return { applicableCells, notApplicableCells };
}

function main() {
  const parser = new Parser();
  parser.setLanguage(Mermaid);
  const queries = loadQueries();
  const contract = verifyApplicability();
  const goldenCount = verifyGoldens(parser, queries);
  const cells = verifyFamilyCoverage(parser, queries, contract);
  process.stdout.write(
    `helix profile: ${LOADED_SURFACES.length} queries compiled, ${goldenCount} goldens, `
      + `${cells.applicableCells} applicable cells, ${cells.notApplicableCells} N/A cells passed\n`,
  );
}

main();
