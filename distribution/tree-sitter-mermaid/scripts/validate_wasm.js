'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const { Language, Parser } = require('web-tree-sitter');

async function main() {
  const candidate = process.argv[2];
  assert.ok(candidate, 'usage: validate_wasm.js <language-wasm>');

  await Parser.init();
  const language = await Language.load(path.resolve(candidate));
  assert.equal(language.abiVersion, 15);

  const parser = new Parser();
  parser.setLanguage(language);
  const tree = parser.parse('flowchart TD\n  A --> B\n');
  assert.equal(tree.rootNode.type, 'source_file');
  assert.equal(tree.rootNode.hasError, false);
  tree.delete();
  parser.delete();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
