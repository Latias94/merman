'use strict';

const path = require('node:path');
const { loadQueryProfiles } = require('../query-profiles');

const packageRoot = path.join(__dirname, '..', '..');
const artifactReceipt = require('../../metadata/artifact-receipt.json');

module.exports = {
  languagePath: path.join(packageRoot, 'wasm', 'tree-sitter-mermaid.wasm'),
  artifactReceipt,
  queryProfiles: loadQueryProfiles(packageRoot, artifactReceipt),
};
