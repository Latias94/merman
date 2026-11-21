'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { readFile } = require('node:fs/promises');
const { Language, Parser, Query } = require('web-tree-sitter');
const grammar = require('../../tree-sitter.json');

const languagePromise = (async () => {
  await Parser.init();
  return Language.load(require.resolve('../../tree-sitter-mermaid.wasm'));
})();

test('language WASM excludes platform-specific compiler metadata', async () => {
  const bytes = await readFile(require.resolve('../../tree-sitter-mermaid.wasm'));
  const module = await WebAssembly.compile(bytes);
  assert.equal(WebAssembly.Module.customSections(module, 'producers').length, 0);
});

test('language WASM loads with ABI 15 and parses Mermaid', async () => {
  const language = await languagePromise;
  assert.equal(language.abiVersion, 15);
  const [major_version, minor_version, patch_version] = grammar.metadata.version.split('.').map(Number);
  assert.deepEqual(language.metadata, { major_version, minor_version, patch_version });

  const parser = new Parser();
  parser.setLanguage(language);
  const tree = parser.parse('flowchart TD\nA --> B\n');
  assert.equal(tree.rootNode.type, 'source_file');
  assert.equal(tree.rootNode.hasError, false);
  assert.equal(tree.rootNode.namedChildren[0].type, 'flowchart_diagram');
  tree.delete();
  parser.delete();
});

test('Flowchart and Swimlane lexical boundaries keep their portable highlight spans', async () => {
  const language = await languagePromise;
  const parser = new Parser();
  parser.setLanguage(language);
  const highlights = await readFile(require.resolve('../../queries/portable/highlights.scm'), 'utf8');
  const query = new Query(language, highlights);

  const cases = [
    ['A --> B', [['variable', 'A'], ['operator', '-->'], ['variable', 'B']]],
    ['A ...- B', [['variable', 'A'], ['operator', '...-'], ['variable', 'B']]],
    ['1.->B', [['variable', '1'], ['operator', '.->'], ['variable', 'B']]],
    ['1.5-.->B', [['variable', '1.5'], ['operator', '-.->'], ['variable', 'B']]],
    ['A...-B', [['variable', 'A...-B']]],
    ['A&B', [['variable', 'A&B']]],
    ['A & B', [['variable', 'A'], ['variable', 'B']]],
    ['A\u00a0&\u2003B', [['variable', 'A'], ['variable', 'B']]],
    ['A--No-->B', [['variable', 'A'], ['operator', '--'], ['string', 'N'], ['operator', 'o-->'], ['variable', 'B']]],
    ['A--Nox-->B', [['variable', 'A'], ['operator', '--'], ['string', 'No'], ['operator', 'x-->'], ['variable', 'B']]],
    ['A--No -->B', [['variable', 'A'], ['operator', '--'], ['string', 'No '], ['operator', '-->'], ['variable', 'B']]],
    ['A--"No"-->B', [['variable', 'A'], ['operator', '--'], ['string', '"No"'], ['operator', '-->'], ['variable', 'B']]],
    ['./a-->./b', [['variable', './a'], ['operator', '-->'], ['variable', './b']]],
  ];

  try {
    for (const header of ['flowchart LR', 'swimlane-beta LR']) {
      for (const [statement, expected] of cases) {
        const source = `${header}\n${statement}\n`;
        const tree = parser.parse(source);
        try {
          assert.equal(tree.rootNode.hasError, false, source);
          const captures = query.captures(tree.rootNode)
            .filter(({ node }) => node.startPosition.row === 1)
            .map(({ name, node }) => [name, node.text]);
          assert.deepEqual(captures, expected, source);
        } finally {
          tree.delete();
        }
      }
    }
  } finally {
    query.delete();
    parser.delete();
  }
});

test('flow directions keep comment and winning value highlight ranges in WASM', async () => {
  const language = await languagePromise;
  const parser = new Parser().setLanguage(language);
  const source = [
    'flowchart TD',
    'subgraph S',
    'direction',
    '%% gap',
    'LRfoo --> B',
    'direction LR direction TBfoo direction TBbar',
    'direction',
    '%% no value follows',
    'Other --> B',
    'direction LR direction',
    '  %% direction BT',
    'TBfoo --> B',
    'end',
    '',
  ].join('\r\n');
  const tree = parser.parse(source);
  const highlights = await readFile(require.resolve('../../queries/portable/highlights.scm'), 'utf8');
  const query = new Query(language, highlights);
  try {
    assert.equal(tree.rootNode.hasError, false);
    const [subgraph] = tree.rootNode.descendantsOfType('flow_subgraph');
    const captures = subgraph.namedChildren.filter((node) => node.type === 'flow_line_item')
      .flatMap((item) => query.captures(item))
      .map(({ name, node }) => [name, node.text, node.startPosition.row, node.startPosition.column]);
    assert.deepEqual(captures, [
      ['keyword', 'direction', 2, 0],
      ['comment', '%% gap', 3, 0],
      ['constant', 'LR', 4, 0],
      ['keyword', 'direction', 5, 29],
      ['constant', 'TB', 5, 39],
      ['variable', 'direction', 6, 0],
      ['comment', '%% no value follows', 7, 0],
      ['variable', 'Other', 8, 0],
      ['operator', '-->', 8, 6],
      ['variable', 'B', 8, 10],
      ['keyword', 'direction', 9, 13],
      ['comment', '%% direction BT', 10, 2],
      ['constant', 'TB', 11, 0],
    ]);
  } finally {
    query.delete();
    tree.delete();
    parser.delete();
  }
});

for (const [family, source] of [
  ['agentflow', 'agentflow-beta\nflow Team\nA@{shape: task, label: "Resolve"} --> B\nend\n'],
  ['usecase', 'usecase-beta\nactor User\nLogin(Sign in)\nUser --> Login\nLogin ..> : include Verify\n'],
]) {
  test(`language WASM parses ${family} with its structured root`, async () => {
    const parser = new Parser();
    parser.setLanguage(await languagePromise);
    const tree = parser.parse(source);
    assert.equal(tree.rootNode.hasError, false);
    assert.equal(tree.rootNode.namedChildren[0].type, `${family}_diagram`);
    tree.delete();
    parser.delete();
  });
}
