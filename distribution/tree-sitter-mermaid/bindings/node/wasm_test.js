'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { readFile } = require('node:fs/promises');
const { Language, Parser } = require('web-tree-sitter');

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

  const parser = new Parser();
  parser.setLanguage(language);
  const tree = parser.parse('flowchart TD\nA --> B\n');
  assert.equal(tree.rootNode.type, 'source_file');
  assert.equal(tree.rootNode.hasError, false);
  assert.equal(tree.rootNode.namedChildren[0].type, 'flowchart_diagram');
  tree.delete();
  parser.delete();
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
