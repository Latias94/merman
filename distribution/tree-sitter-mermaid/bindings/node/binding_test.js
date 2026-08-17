'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const Parser = require('tree-sitter');
const Mermaid = require('./index.js');

test('the Node binding loads and parses Mermaid', () => {
  const parser = new Parser();
  parser.setLanguage(Mermaid);

  const tree = parser.parse('flowchart TD\nA --> B\n');
  assert.equal(tree.rootNode.type, 'source_file');
  assert.equal(tree.rootNode.hasError, false);
  assert.equal(tree.rootNode.namedChildren[0].type, 'flowchart_diagram');
  assert.equal(Mermaid.name, 'mermaid');
  assert.ok(Array.isArray(Mermaid.nodeTypeInfo));
});
