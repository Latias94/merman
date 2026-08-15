'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const Parser = require('tree-sitter');

const packageRoot = path.resolve(__dirname, '..', '..', '..');
const queryRoot = path.join(packageRoot, 'queries', 'zed');
const goldenRoot = path.join(packageRoot, 'test', 'queries', 'zed');

// The profile verifier compiles against the built language directly. The
// canonical package entry point additionally enforces artifact-receipt hashes.
const Mermaid = require(
  process.env.MERMAID_ZED_BINDING
    || path.join(packageRoot, 'build', 'Release', 'tree_sitter_mermaid_binding.node'),
);

const FAMILY_IDS = [
  'architecture',
  'block',
  'c4',
  'class',
  'cynefin',
  'er',
  'eventmodeling',
  'flowchart',
  'gantt',
  'gitgraph',
  'info',
  'ishikawa',
  'journey',
  'kanban',
  'mindmap',
  'packet',
  'pie',
  'quadrantchart',
  'radar',
  'railroad',
  'railroadAbnf',
  'railroadEbnf',
  'railroadPeg',
  'requirement',
  'sankey',
  'sequence',
  'state',
  'swimlane',
  'timeline',
  'treeView',
  'treemap',
  'venn',
  'wardley',
  'xychart',
  'zenuml',
];
const FIXTURE_SLUGS = {
  er: 'entity-relationship',
  eventmodeling: 'event-modeling',
  gitgraph: 'git-graph',
  quadrantchart: 'quadrant-chart',
  railroadAbnf: 'railroad-abnf',
  railroadEbnf: 'railroad-ebnf',
  railroadPeg: 'railroad-peg',
  treeView: 'tree-view',
};
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
  'brackets',
  'outline',
  'indents',
  'injections',
  'textobjects',
];
const NATIVE_NA_SURFACES = ['folds', 'locals', 'tags'];
const LOADER_PREFIXES = [
  'highlights',
  'brackets',
  'outline',
  'indents',
  'injections',
  'overrides',
  'redactions',
  'runnables',
  'debugger',
  'textobjects',
];
const QUERY_FILES = LOADED_SURFACES.map((surface) => `${surface}.scm`).sort();
const HIGHLIGHT_ADAPTER = {
  'comment.documentation': 'comment.doc',
  'function.macro': 'function',
  'keyword.operator': 'operator',
  namespace: 'type',
  'variable.member': 'property',
};

const CAPTURE_VOCABULARIES = {
  highlights: new Set([
    'attribute',
    'boolean',
    'comment',
    'comment.doc',
    'constant',
    'function',
    'keyword',
    'number',
    'operator',
    'property',
    'punctuation.bracket',
    'punctuation.delimiter',
    'punctuation.special',
    'string',
    'string.escape',
    'string.special',
    'type',
    'type.builtin',
    'variable',
  ]),
  brackets: new Set(['open', 'close']),
  outline: new Set([
    'item',
    'name',
    'context',
    'context.extra',
    'open',
    'close',
    'annotation',
  ]),
  indents: new Set(['indent', 'start', 'end', 'outdent']),
  injections: new Set([
    'language',
    'injection.language',
    'content',
    'injection.content',
  ]),
  textobjects: new Set([
    'function.inside',
    'function.around',
    'class.inside',
    'class.around',
    'comment.inside',
    'comment.around',
  ]),
};
const HIGHLIGHT_WHITESPACE_NODE_ALLOWLIST = new Set([
  'railroad_ebnf_special_text',
  'state_note_line',
  'timeline_event_text',
  'timeline_period',
  'tree_view_box_prefix',
]);

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function sorted(values) {
  return [...values].sort(compareText);
}

function assertExactKeys(value, expected, label) {
  assert.deepEqual(sorted(Object.keys(value)), sorted(expected), `${label}: keys`);
}

function captureNamesInSource(source) {
  return new Set(
    [...source.matchAll(/@([A-Za-z0-9_.-]+)/g)].map((match) => match[1]),
  );
}

function assertCaptureVocabulary(surface, source) {
  const actual = captureNamesInSource(source);
  assert.ok(actual.size > 0, `${surface}: query must declare captures`);
  for (const capture of actual) {
    const allowed = CAPTURE_VOCABULARIES[surface];
    assert.ok(
      allowed.has(capture) || (surface === 'indents' && capture.startsWith('start.')),
      `${surface}: unsupported Zed capture @${capture}`,
    );
  }
  if (surface === 'highlights') {
    assert.deepEqual(actual, CAPTURE_VOCABULARIES.highlights, 'highlights: adapter vocabulary');
  } else if (surface === 'brackets') {
    assert.deepEqual(actual, new Set(['open', 'close']));
  } else if (surface === 'outline') {
    assert.ok(actual.has('item') && actual.has('name'), 'outline: required captures');
  } else if (surface === 'indents') {
    assert.ok(actual.has('indent'), 'indents: required @indent capture');
  } else if (surface === 'injections') {
    assert.ok(
      actual.has('content') || actual.has('injection.content'),
      'injections: required content capture',
    );
  }
}

function normalizeCaptures(captures, source) {
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
  const actualFiles = fs.readdirSync(queryRoot)
    .filter((name) => name.endsWith('.scm'))
    .sort(compareText);
  assert.deepEqual(actualFiles, QUERY_FILES, 'Zed profile query files');

  const queries = {};
  for (const surface of LOADED_SURFACES) {
    const queryPath = path.join(queryRoot, `${surface}.scm`);
    const source = fs.readFileSync(queryPath, 'utf8');
    assertCaptureVocabulary(surface, source);
    queries[surface] = new Parser.Query(Mermaid, source);
  }

  for (const surface of NATIVE_NA_SURFACES) {
    assert.ok(
      !fs.existsSync(path.join(queryRoot, `${surface}.scm`)),
      `${surface}: an unused file would create a false Zed loader contract`,
    );
    assert.ok(
      !fs.existsSync(path.join(goldenRoot, surface)),
      `${surface}: N/A surfaces must not have executable fixture directories`,
    );
  }
  return queries;
}

function verifyApplicability() {
  const contractPath = path.join(goldenRoot, 'applicability.json');
  const contract = JSON.parse(fs.readFileSync(contractPath, 'utf8'));
  assert.equal(contract.schemaVersion, 1);
  assert.equal(contract.profile, 'zed');
  assertExactKeys(
    contract.consumer,
    [
      'name',
      'version',
      'sourceRef',
      'sourceCommit',
      'sourceDate',
      'loaderSource',
      'loaderPrefixes',
      'r9LoadedSurfaces',
    ],
    'consumer',
  );
  assert.equal(contract.consumer.name, 'Zed');
  assert.equal(contract.consumer.version, '1.16.0');
  assert.equal(contract.consumer.sourceRef, 'main');
  assert.equal(
    contract.consumer.sourceCommit,
    '38ca9106c5306ef93e52c35643df015a27f15b72',
  );
  assert.equal(contract.consumer.sourceDate, '2026-08-07');
  assert.equal(contract.consumer.loaderSource, 'crates/language_core/src/queries.rs');
  assert.deepEqual(contract.consumer.loaderPrefixes, LOADER_PREFIXES);
  assert.deepEqual(contract.consumer.r9LoadedSurfaces, LOADED_SURFACES);

  assert.deepEqual(contract.queryContract.profileFiles.sort(), QUERY_FILES);
  assert.deepEqual(contract.queryContract.absentFiles.sort(), NATIVE_NA_SURFACES.map(
    (surface) => `${surface}.scm`,
  ).sort());
  assert.deepEqual(contract.queryContract.highlightAdapter, HIGHLIGHT_ADAPTER);
  assert.equal(contract.queryContract.portableSemanticTokenMapping, false);

  assert.equal(contract.families.length, FAMILY_IDS.length);
  assert.deepEqual(contract.families.map(({ publicId }) => publicId), FAMILY_IDS);
  assert.equal(new Set(contract.families.map(({ publicId }) => publicId)).size, 35);

  for (const family of contract.families) {
    assertExactKeys(family, ['publicId', 'fixtureSlug', 'surfaces'], family.publicId);
    assert.equal(
      family.fixtureSlug,
      FIXTURE_SLUGS[family.publicId] || family.publicId,
      `${family.publicId}: fixtureSlug`,
    );
    assert.deepEqual(sorted(Object.keys(family.surfaces)), sorted(SURFACES), family.publicId);
    for (const surface of SURFACES) {
      const cell = family.surfaces[surface];
      const label = `${family.publicId}/${surface}`;
      assert.ok(cell && typeof cell === 'object', label);
      assert.ok(['applicable', 'not_applicable'].includes(cell.status), `${label}: status`);
      if (cell.status === 'applicable') {
        assertExactKeys(cell, ['status', 'query'], label);
        assert.ok(LOADED_SURFACES.includes(surface), `${label}: Zed does not load surface`);
        assert.equal(cell.query, `queries/zed/${surface}.scm`, `${label}: query path`);
      } else {
        assertExactKeys(cell, ['status', 'rationale'], label);
        assert.equal(typeof cell.rationale, 'string', `${label}: rationale type`);
        assert.ok(cell.rationale.trim().length >= 48, `${label}: rationale is too weak`);
      }
    }
    for (const surface of NATIVE_NA_SURFACES) {
      assert.equal(
        family.surfaces[surface].status,
        'not_applicable',
        `${family.publicId}/${surface}: native N/A surface`,
      );
    }
  }
  return contract;
}

function assertMatchContract(surface, query, tree, label) {
  const matches = query.matches(tree.rootNode);
  assert.ok(matches.length > 0, `${label}: no query matches`);
  for (const match of matches) {
    const names = new Set(match.captures.map(({ name }) => name));
    if (surface === 'brackets') {
      assert.ok(names.has('open') && names.has('close'), `${label}: bracket match`);
    } else if (surface === 'outline') {
      assert.ok(names.has('item') && names.has('name'), `${label}: outline match`);
    } else if (surface === 'indents') {
      assert.ok(names.has('indent'), `${label}: indent match`);
    } else if (surface === 'injections') {
      assert.ok(
        names.has('content') || names.has('injection.content'),
        `${label}: injection content match`,
      );
      assert.equal(
        typeof match.setProperties?.['injection.language'],
        'string',
        `${label}: injection.language property`,
      );
    } else if (surface === 'textobjects') {
      assert.ok(
        [...names].some((name) => CAPTURE_VOCABULARIES.textobjects.has(name)),
        `${label}: text object match`,
      );
    } else {
      assert.ok(match.captures.length > 0, `${label}: highlight match`);
    }
  }
}

function assertSurfaceContract(surface, rawCaptures, source, label) {
  const names = rawCaptures.map(({ name }) => name);
  for (const { name, node } of rawCaptures) {
    assert.ok(CAPTURE_VOCABULARIES[surface].has(name), `${label}: unsupported @${name}`);
    assert.ok(node.endIndex > node.startIndex, `${label}: empty @${name}`);
    const text = source.slice(node.startIndex, node.endIndex);
    if (surface === 'highlights') {
      assert.ok(
        !/^\s|\s$/.test(text) || HIGHLIGHT_WHITESPACE_NODE_ALLOWLIST.has(node.type),
        `${label}: @${name} ${node.type} owns structural whitespace`,
      );
    } else if (
      (surface === 'outline' && name === 'name')
      || (surface === 'brackets' && ['open', 'close'].includes(name))
      || (surface === 'indents' && name === 'end')
    ) {
      assert.ok(!/^\s|\s$/.test(text), `${label}: @${name} owns structural whitespace`);
    }
  }

  if (surface === 'highlights') {
    assert.ok(names.includes('keyword'), `${label}: no @keyword`);
    assert.ok(
      names.some((name) => !['keyword', 'comment', 'attribute'].includes(name)),
      `${label}: no family-owned or structural highlight`,
    );
  } else if (surface === 'brackets') {
    assert.equal(
      names.filter((name) => name === 'open').length,
      names.filter((name) => name === 'close').length,
      `${label}: unpaired bracket captures`,
    );
  } else if (surface === 'outline') {
    assert.ok(names.includes('item') && names.includes('name'), `${label}: outline captures`);
  } else if (surface === 'indents') {
    assert.ok(names.includes('indent'), `${label}: no @indent`);
  } else if (surface === 'injections') {
    assert.ok(names.includes('injection.content'), `${label}: no @injection.content`);
  } else if (surface === 'textobjects') {
    assert.ok(names.includes('class.around'), `${label}: no diagram-sized @class.around`);
    assert.ok(names.includes('class.inside'), `${label}: no diagram-body @class.inside`);
  }
}

function verifyApplicableCell(parser, queries, family, surface) {
  const label = `${family.publicId}/${surface}`;
  const surfaceRoot = path.join(goldenRoot, surface);
  const sourcePath = path.join(surfaceRoot, `${family.fixtureSlug}.mmd`);
  const goldenPath = path.join(surfaceRoot, `${family.fixtureSlug}.captures.json`);
  assert.ok(fs.existsSync(sourcePath), `${label}: source fixture`);
  assert.ok(fs.existsSync(goldenPath), `${label}: capture golden`);

  const source = fs.readFileSync(sourcePath, 'utf8');
  const tree = parser.parse(source);
  assert.equal(tree.rootNode.type, 'source_file', `${label}: source root`);
  assert.equal(tree.rootNode.hasError, false, `${label}: source has parse errors`);
  const roots = tree.rootNode.namedChildren.filter((node) => node.type.endsWith('_diagram'));
  assert.equal(roots.length, 1, `${label}: expected one family root`);

  const query = queries[surface];
  const rawCaptures = query.captures(tree.rootNode);
  assert.ok(rawCaptures.length > 0, `${label}: applicable cell has no captures`);
  assertMatchContract(surface, query, tree, label);
  assertSurfaceContract(surface, rawCaptures, source, label);

  const golden = JSON.parse(fs.readFileSync(goldenPath, 'utf8'));
  assertExactKeys(
    golden,
    ['schemaVersion', 'profile', 'surface', 'source', 'captures'],
    `${label}: golden`,
  );
  assert.equal(golden.schemaVersion, 1, `${label}: golden schema`);
  assert.equal(golden.profile, 'zed', `${label}: golden profile`);
  assert.equal(golden.surface, surface, `${label}: golden surface`);
  assert.equal(golden.source, `${family.fixtureSlug}.mmd`, `${label}: golden source`);
  assert.deepEqual(
    normalizeCaptures(rawCaptures, source),
    golden.captures,
    `${label}: capture golden drifted`,
  );
}

function verifyFixtureSets(expectedBySurface) {
  for (const surface of LOADED_SURFACES) {
    const surfaceRoot = path.join(goldenRoot, surface);
    assert.ok(fs.statSync(surfaceRoot).isDirectory(), `${surface}: fixture directory`);
    const entries = fs.readdirSync(surfaceRoot);
    const sources = entries
      .filter((entry) => entry.endsWith('.mmd'))
      .map((entry) => entry.slice(0, -'.mmd'.length));
    const goldens = entries
      .filter((entry) => entry.endsWith('.captures.json'))
      .map((entry) => entry.slice(0, -'.captures.json'.length));
    assert.deepEqual(sorted(sources), sorted(expectedBySurface[surface]), `${surface}: sources`);
    assert.deepEqual(sorted(goldens), sorted(expectedBySurface[surface]), `${surface}: goldens`);
  }
}

function verifyMatrix(parser, queries, contract) {
  const expectedBySurface = Object.fromEntries(
    LOADED_SURFACES.map((surface) => [surface, []]),
  );
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
      expectedBySurface[surface].push(family.fixtureSlug);
      verifyApplicableCell(parser, queries, family, surface);
    }
  }

  assert.equal(applicableCells + notApplicableCells, FAMILY_IDS.length * SURFACES.length);
  assert.equal(applicableCells, 188, 'applicable cell count');
  assert.equal(notApplicableCells, 127, 'N/A cell count');
  verifyFixtureSets(expectedBySurface);
  return { applicableCells, notApplicableCells };
}

function main() {
  const parser = new Parser();
  parser.setLanguage(Mermaid);
  const queries = loadQueries();
  const contract = verifyApplicability();
  const counts = verifyMatrix(parser, queries, contract);
  process.stdout.write(
    `zed profile: ${LOADED_SURFACES.length} queries compiled, `
      + `${counts.applicableCells} applicable cells, `
      + `${counts.notApplicableCells} N/A cells passed\n`,
  );
}

main();
