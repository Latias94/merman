'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { Language, Parser } = require('web-tree-sitter');

const languagePromise = (async () => {
  await Parser.init();
  return Language.load(require.resolve('../../tree-sitter-mermaid.wasm'));
})();

test('language WASM loads with ABI 15 and parses Mermaid', async () => {
  const language = await languagePromise;
  assert.equal(language.abiVersion, 15);

  const parser = new Parser();
  parser.setLanguage(language);
  const tree = parser.parse('flowchart TD\nA --> B\n');
  assert.equal(tree.rootNode.type, 'source_file');
  assert.equal(tree.rootNode.hasError, false);
  assert.equal(tree.rootNode.namedChildren[0].type, 'flowchart_diagram');
  tree.delete();
  parser.delete();
});
