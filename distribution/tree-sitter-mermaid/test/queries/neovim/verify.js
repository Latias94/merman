'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const Parser = require('tree-sitter');

const packageRoot = path.resolve(__dirname, '..', '..', '..');

function loadLanguage() {
  const override = process.env.MERMAID_NODE_BINDING;
  if (override) return require(path.resolve(override));
  return require(path.join(packageRoot, 'bindings', 'node'));
}

const Mermaid = loadLanguage();
const matrixPath = path.join(__dirname, 'applicability.json');
const matrix = JSON.parse(fs.readFileSync(matrixPath, 'utf8'));
const support = JSON.parse(fs.readFileSync(
  path.join(packageRoot, 'metadata', 'support.json'),
  'utf8',
));

function querySource(surface) {
  const adapter = fs.readFileSync(
    path.join(packageRoot, 'queries', 'neovim', `${surface}.scm`),
    'utf8',
  );
  const nodeCompatibleAdapter = adapter
    .split('\n')
    .filter((line) => !line.trimStart().startsWith('(#offset!'))
    .join('\n');
  if (surface !== 'highlights') return nodeCompatibleAdapter;
  const portable = fs.readFileSync(
    path.join(packageRoot, 'queries', 'portable', 'highlights.scm'),
    'utf8',
  );
  return `${portable}\n${nodeCompatibleAdapter}`;
}

function sourcePath(family, cell) {
  const relative = cell.source || family.source;
  assert.equal(typeof relative, 'string', `${family.publicId}: source`);
  assert.ok(!path.isAbsolute(relative), `${family.publicId}: source must be relative`);
  const resolved = path.resolve(packageRoot, relative);
  assert.ok(
    resolved.startsWith(`${packageRoot}${path.sep}`),
    `${family.publicId}: source escapes package`,
  );
  return resolved;
}

function captureNames(query, tree) {
  return new Set(query.captures(tree.rootNode).map(({ name }) => name));
}

function main() {
  assert.equal(matrix.schemaVersion, 1, 'schemaVersion');
  assert.equal(matrix.profile, 'neovim', 'profile');
  assert.match(matrix.consumer.version, /^\d+\.\d+\.\d+$/, 'fixed editor version');
  assert.equal(matrix.consumer.sourceRef, `v${matrix.consumer.version}`, 'consumer ref');
  assert.match(matrix.consumer.sourceCommit, /^[0-9a-f]{40}$/, 'consumer commit');
  assert.equal(matrix.surfaces.length, 9, 'surface count');
  assert.equal(new Set(matrix.surfaces).size, 9, 'surface uniqueness');
  assert.deepEqual(matrix.consumer.loadedSurfaces, matrix.surfaces, 'loaded surfaces');

  const queryFiles = fs.readdirSync(path.join(packageRoot, 'queries', 'neovim'))
    .filter((name) => name.endsWith('.scm'))
    .map((name) => name.replace(/\.scm$/, ''))
    .sort();
  assert.deepEqual(queryFiles, [...matrix.surfaces].sort(), 'query surface files');

  const expectedFamilies = support.families.map(({ publicId }) => publicId).sort();
  const actualFamilies = matrix.families.map(({ publicId }) => publicId).sort();
  assert.deepEqual(actualFamilies, expectedFamilies, 'family coverage');

  const roots = new Map(support.families.map(({ publicId, rootNode }) => [
    publicId,
    rootNode,
  ]));
  const parser = new Parser();
  parser.setLanguage(Mermaid);
  const parsedFixtures = new Map();
  function parsedFixture(family, cell) {
    const resolved = sourcePath(family, cell);
    let fixture = parsedFixtures.get(resolved);
    if (!fixture) {
      const source = fs.readFileSync(resolved, 'utf8');
      const tree = parser.parse(source);
      fixture = {
        tree,
        diagrams: tree.rootNode.namedChildren.filter(({ type }) => (
          type.endsWith('_diagram')
        )),
      };
      parsedFixtures.set(resolved, fixture);
    }
    return fixture;
  }
  const queries = new Map(matrix.surfaces.map((surface) => [
    surface,
    new Parser.Query(Mermaid, querySource(surface)),
  ]));
  const counts = Object.fromEntries(matrix.surfaces.map((surface) => [
    surface,
    { applicable: 0, not_applicable: 0 },
  ]));

  for (const family of matrix.families) {
    assert.deepEqual(
      Object.keys(family.surfaces).sort(),
      [...matrix.surfaces].sort(),
      `${family.publicId}: explicit surfaces`,
    );

    for (const surface of matrix.surfaces) {
      const cell = family.surfaces[surface];
      assert.ok(
        cell.status === 'applicable' || cell.status === 'not_applicable',
        `${family.publicId}/${surface}: status`,
      );
      counts[surface][cell.status] += 1;

      if (cell.status === 'not_applicable') {
        assert.equal(
          cell.query,
          undefined,
          `${family.publicId}/${surface}: N/A query path`,
        );
        assert.equal(
          typeof cell.rationale,
          'string',
          `${family.publicId}/${surface}: rationale`,
        );
        assert.ok(
          cell.rationale.trim().length >= 20,
          `${family.publicId}/${surface}: rationale too short`,
        );
        continue;
      }

      assert.equal(
        cell.query,
        `queries/neovim/${surface}.scm`,
        `${family.publicId}/${surface}: query path`,
      );

      assert.ok(
        Array.isArray(cell.requiredCaptures) && cell.requiredCaptures.length > 0,
        `${family.publicId}/${surface}: required captures`,
      );
      assert.equal(
        new Set(cell.requiredCaptures).size,
        cell.requiredCaptures.length,
        `${family.publicId}/${surface}: duplicate required capture`,
      );
      if (cell.requiredOffset) {
        assert.equal(surface, 'injections', `${family.publicId}: offset surface`);
        assert.equal(cell.requiredOffset.length, 4, `${family.publicId}: offset length`);
        assert.ok(
          cell.requiredOffset.every(Number.isInteger),
          `${family.publicId}: offset values`,
        );
      }

      const { tree, diagrams } = parsedFixture(family, cell);
      assert.equal(tree.rootNode.type, 'source_file', `${family.publicId}/${surface}: root`);
      assert.equal(tree.rootNode.hasError, false, `${family.publicId}/${surface}: parse error`);
      assert.equal(diagrams.length, 1, `${family.publicId}/${surface}: diagram count`);
      assert.equal(
        diagrams[0].type,
        roots.get(family.publicId),
        `${family.publicId}/${surface}: family root`,
      );

      const actual = captureNames(queries.get(surface), tree);
      for (const name of cell.requiredCaptures) {
        assert.ok(
          actual.has(name),
          `${family.publicId}/${surface}: missing @${name}; got ${[
            ...actual,
          ].sort().join(', ')}`,
        );
      }
    }
  }

  for (const surface of matrix.surfaces) {
    const { applicable, not_applicable: notApplicable } = counts[surface];
    assert.equal(applicable + notApplicable, 35, `${surface}: family count`);
    process.stdout.write(
      `${surface}: ${applicable} applicable, ${notApplicable} not_applicable\n`,
    );
  }
  process.stdout.write('Neovim query matrix: 315 cells passed\n');
}

main();
