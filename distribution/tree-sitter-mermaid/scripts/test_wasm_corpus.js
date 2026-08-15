'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { Language, Parser } = require('web-tree-sitter');

const packageRoot = path.join(__dirname, '..');
const corpusRoot = path.join(packageRoot, 'test', 'corpus');
const wasmPath = path.join(packageRoot, 'wasm', 'tree-sitter-mermaid.wasm');

function corpusFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) return corpusFiles(entryPath);
      return entry.name.endsWith('.txt') ? [entryPath] : [];
    })
    .sort();
}

function parseCases(filePath) {
  const lines = fs.readFileSync(filePath, 'utf8').split('\n');
  const cases = [];
  let cursor = 0;
  while (cursor < lines.length) {
    if (lines[cursor] !== '==================') {
      cursor += 1;
      continue;
    }
    assert.equal(lines[cursor + 2], '==================', `${filePath}: malformed title`);
    const title = lines[cursor + 1].trim();
    cursor += 4;
    const separator = lines.findIndex(
      (line, index) => index >= cursor && line === '---' && lines[index + 1] === '',
    );
    assert.notEqual(separator, -1, `${filePath} case ${cases.length + 1}: missing separator`);
    const input = lines.slice(cursor, separator).join('\n');
    cursor = separator + 2;
    const nextHeading = lines.findIndex(
      (line, index) => index >= cursor && line === '==================',
    );
    const expectedEnd = nextHeading === -1 ? lines.length : nextHeading;
    const expected = lines.slice(cursor, expectedEnd).join('\n');
    cases.push({
      filePath,
      title,
      input,
      expectsError: /\((?:ERROR|MISSING)(?:\s|\))/u.test(expected),
      expectsFamilyRoot: /\([a-z0-9_]+_diagram(?:\s|\))/u.test(expected),
    });
    cursor = expectedEnd;
  }
  return cases;
}

async function main() {
  assert.ok(fs.existsSync(wasmPath), `missing ${wasmPath}`);
  const memory = new WebAssembly.Memory({ initial: 512, maximum: 32768 });
  await Parser.init({ wasmMemory: memory });
  const language = await Language.load(wasmPath);
  assert.equal(language.abiVersion, 14);

  const parser = new Parser();
  parser.setLanguage(language);
  const cases = corpusFiles(corpusRoot).flatMap(parseCases);
  assert.equal(cases.length, 233, 'corpus case count');

  for (const corpusCase of cases) {
    const tree = parser.parse(corpusCase.input);
    const context = `${path.relative(packageRoot, corpusCase.filePath)}: ${corpusCase.title}`;
    assert.equal(tree.rootNode.type, 'source_file', context);
    assert.equal(tree.rootNode.hasError, corpusCase.expectsError, context);
    const roots = tree.rootNode.namedChildren.filter((node) => (
      node.type.endsWith('_diagram')
    ));
    assert.equal(
      roots.length,
      corpusCase.expectsFamilyRoot ? 1 : 0,
      `${context}: family root count`,
    );
    tree.delete();
  }

  parser.delete();
  console.log(`verified ${cases.length} canonical WASM corpus cases`);
}

main().catch((error) => {
  console.error(error.stack || error);
  process.exitCode = 1;
});
