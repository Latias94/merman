'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const Parser = require('tree-sitter');

const packageRoot = path.join(__dirname, '..');
const Mermaid = require('node-gyp-build')(packageRoot);
const portableGoldenRoot = path.join(
  packageRoot,
  'test',
  'queries',
  'portable',
);
const portableQueryRoot = path.join(packageRoot, 'queries', 'portable');
const writeGoldens = process.argv.slice(2).includes('--write');
const QUERY_SURFACES = [
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

const HIGHLIGHT_WHITESPACE_NODE_ALLOWLIST = new Set([
  'railroad_ebnf_special_text',
  'state_note_line',
  'timeline_event_text',
  'timeline_period',
  'tree_view_box_prefix',
]);

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

function goldenFiles(goldenRoot) {
  if (!fs.existsSync(goldenRoot)) return [];
  return fs.readdirSync(goldenRoot)
    .filter((name) => name.endsWith('.captures.json'))
    .sort()
    .map((name) => path.join(goldenRoot, name));
}

function assertSourceInsidePortableRoot(goldenPath, sourcePath) {
  const relative = path.relative(portableGoldenRoot, sourcePath);
  assert.ok(
    relative && !relative.startsWith('..') && !path.isAbsolute(relative),
    `${goldenPath}: source escapes portable golden root`,
  );
}

function assertSurfaceContract(surface, captures, source, goldenPath) {
  if (surface === 'highlights') {
    for (const { name, node } of captures) {
      const text = source.slice(node.startIndex, node.endIndex);
      assert.ok(
        !/^\s|\s$/.test(text) || HIGHLIGHT_WHITESPACE_NODE_ALLOWLIST.has(node.type),
        `${goldenPath}: ${name} capture ${node.type} owns structural whitespace`,
      );
    }
    assert.ok(
      captures.some(({ name }) => (
        name !== 'keyword' && name !== 'comment' && name !== 'attribute'
      )),
      `${goldenPath}: golden must include a family-owned or structural capture`,
    );
    return;
  }

  if (surface === 'injections') {
    assert.ok(
      captures.some(({ name }) => name === 'injection.content'),
      `${goldenPath}: injection.content capture`,
    );
    return;
  }

  if (surface === 'locals') {
    assert.ok(
      captures.some(({ name }) => name === 'local.definition'),
      `${goldenPath}: local.definition capture`,
    );
    assert.ok(
      captures.some(({ name }) => name === 'local.reference'),
      `${goldenPath}: local.reference capture`,
    );
    return;
  }

  if (surface === 'tags') {
    assert.ok(
      captures.some(({ name }) => name === 'name'),
      `${goldenPath}: tag name capture`,
    );
    assert.ok(
      captures.some(({ name }) => name.startsWith('definition.')),
      `${goldenPath}: tag definition capture`,
    );
  }
}

function assertInjectionProperties(query, rootNode, goldenPath) {
  const matches = query.matches(rootNode);
  assert.ok(matches.length > 0, `${goldenPath}: injection match`);
  for (const match of matches) {
    assert.equal(
      typeof match.setProperties?.['injection.language'],
      'string',
      `${goldenPath}: injection.language property`,
    );
  }
}

function verifyGolden(goldenPath, parser, query, surface) {
  const golden = JSON.parse(fs.readFileSync(goldenPath, 'utf8'));
  assert.equal(golden.schemaVersion, 1, `${goldenPath}: schemaVersion`);
  assert.equal(golden.profile, 'portable', `${goldenPath}: profile`);
  assert.equal(golden.surface, surface, `${goldenPath}: surface`);
  assert.equal(typeof golden.source, 'string', `${goldenPath}: source`);
  const sourcePath = path.resolve(path.dirname(goldenPath), golden.source);
  assertSourceInsidePortableRoot(goldenPath, sourcePath);
  const source = fs.readFileSync(sourcePath, 'utf8');
  const tree = parser.parse(source);
  assert.equal(tree.rootNode.type, 'source_file', goldenPath);
  assert.equal(tree.rootNode.hasError, false, `${goldenPath}: source has parse errors`);
  const roots = tree.rootNode.namedChildren.filter((node) => node.type.endsWith('_diagram'));
  assert.equal(roots.length, 1, `${goldenPath}: expected one family root`);
  assert.ok(Array.isArray(golden.captures), `${goldenPath}: captures`);
  const rawCaptures = query.captures(tree.rootNode);
  assertSurfaceContract(surface, rawCaptures, source, goldenPath);
  if (surface === 'injections') {
    assertInjectionProperties(query, tree.rootNode, goldenPath);
  }
  const actual = normalizeCaptures(rawCaptures, source);
  if (writeGoldens && JSON.stringify(actual) !== JSON.stringify(golden.captures)) {
    golden.captures = actual;
    fs.writeFileSync(goldenPath, `${JSON.stringify(golden, null, 2)}\n`);
  } else {
    assert.deepEqual(actual, golden.captures, `${goldenPath}: capture golden drifted`);
  }
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
}

function packageSource(relative, context) {
  assert.equal(typeof relative, 'string', `${context}: source`);
  const resolved = path.resolve(packageRoot, relative);
  assert.ok(
    resolved.startsWith(`${path.resolve(packageRoot)}${path.sep}`),
    `${context}: source escapes package`,
  );
  assert.ok(fs.existsSync(resolved), `${context}: missing ${relative}`);
  return fs.readFileSync(resolved, 'utf8');
}

function verifyApplicability(parser, queries) {
  const matrixPath = path.join(portableGoldenRoot, 'applicability.json');
  const matrix = JSON.parse(fs.readFileSync(matrixPath, 'utf8'));
  const support = JSON.parse(fs.readFileSync(
    path.join(packageRoot, 'metadata', 'support.json'),
    'utf8',
  ));
  assert.equal(matrix.schemaVersion, 1, 'portable applicability schemaVersion');
  assert.equal(matrix.profile, 'portable', 'portable applicability profile');
  assert.deepEqual(matrix.surfaces, QUERY_SURFACES, 'portable query surfaces');
  assert.equal(matrix.families.length, 35, 'portable family count');
  assert.deepEqual(
    matrix.families.map(({ publicId }) => publicId).sort(),
    support.families.map(({ publicId }) => publicId).sort(),
    'portable family catalog',
  );

  const roots = new Map(support.families.map(({ publicId, rootNode }) => [
    publicId,
    rootNode,
  ]));
  const referencedQueries = new Set();
  let applicable = 0;
  let notApplicable = 0;
  for (const family of matrix.families) {
    assert.deepEqual(
      Object.keys(family.surfaces).sort(),
      [...QUERY_SURFACES].sort(),
      `${family.publicId}: explicit portable surfaces`,
    );
    for (const surface of QUERY_SURFACES) {
      const context = `${family.publicId}/${surface}`;
      const cell = family.surfaces[surface];
      assert.ok(
        cell.status === 'applicable' || cell.status === 'not_applicable',
        `${context}: status`,
      );
      if (cell.status === 'not_applicable') {
        notApplicable += 1;
        assert.equal(typeof cell.rationale, 'string', `${context}: rationale`);
        assert.ok(cell.rationale.trim().length >= 20, `${context}: rationale too short`);
        assert.equal(cell.query, undefined, `${context}: N/A query`);
        assert.equal(cell.requiredCaptures, undefined, `${context}: N/A captures`);
        continue;
      }

      applicable += 1;
      const expectedQuery = `queries/portable/${surface}.scm`;
      assert.equal(cell.query, expectedQuery, `${context}: query`);
      referencedQueries.add(expectedQuery);
      assert.ok(queries.has(surface), `${context}: missing packaged query`);
      assert.ok(
        Array.isArray(cell.requiredCaptures) && cell.requiredCaptures.length > 0,
        `${context}: required captures`,
      );
      assert.equal(
        new Set(cell.requiredCaptures).size,
        cell.requiredCaptures.length,
        `${context}: duplicate required capture`,
      );
      const source = packageSource(cell.source || family.source, context);
      const tree = parser.parse(source);
      assert.equal(tree.rootNode.hasError, false, `${context}: parse error`);
      const diagrams = tree.rootNode.namedChildren.filter(({ type }) => type.endsWith('_diagram'));
      assert.equal(diagrams.length, 1, `${context}: diagram count`);
      assert.equal(diagrams[0].type, roots.get(family.publicId), `${context}: family root`);
      const captures = new Set(
        queries.get(surface).captures(tree.rootNode).map(({ name }) => name),
      );
      for (const capture of cell.requiredCaptures) {
        assert.ok(captures.has(capture), `${context}: missing @${capture}`);
      }
    }
  }
  assert.equal(applicable + notApplicable, 35 * QUERY_SURFACES.length);
  assert.deepEqual(
    [...referencedQueries].sort(),
    [...queries.keys()].map((surface) => `queries/portable/${surface}.scm`).sort(),
    'every portable query serves an applicable family cell',
  );
  process.stdout.write(
    `portable applicability: ${applicable} applicable, ${notApplicable} N/A cells passed\n`,
  );
}

function main() {
  const parser = new Parser();
  parser.setLanguage(Mermaid);
  const surfaces = fs.readdirSync(portableQueryRoot)
    .filter((name) => name.endsWith('.scm'))
    .map((name) => name.slice(0, -4))
    .sort();
  assert.ok(surfaces.length > 0, 'portable queries must contain a surface');

  const queries = new Map();
  let total = 0;
  for (const surface of surfaces) {
    const querySource = fs.readFileSync(
      path.join(portableQueryRoot, `${surface}.scm`),
      'utf8',
    );
    const query = new Parser.Query(Mermaid, querySource);
    queries.set(surface, query);
    const files = goldenFiles(path.join(portableGoldenRoot, surface));
    assert.ok(files.length > 0, `${surface}: portable surface requires a golden`);
    for (const file of files) verifyGolden(file, parser, query, surface);
    total += files.length;
    process.stdout.write(`portable ${surface} goldens: ${files.length} passed\n`);
  }
  verifyApplicability(parser, queries);
  process.stdout.write(`portable query goldens: ${total} passed\n`);
}

main();
