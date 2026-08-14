'use strict';

const fs = require('node:fs');
const path = require('node:path');

const packageRoot = path.join(__dirname, '..', '..');
const portableHighlightsRelativePath = 'queries/portable/highlights.scm';
const portableHighlightsPath = path.join(packageRoot, portableHighlightsRelativePath);

module.exports = {
  languagePath: path.join(packageRoot, 'wasm', 'tree-sitter-mermaid.wasm'),
  artifactReceipt: require('../../metadata/artifact-receipt.json'),
  queryProfiles: Object.freeze({
    portable: Object.freeze({
      highlights: Object.freeze({
        relativePath: portableHighlightsRelativePath,
        path: portableHighlightsPath,
        source: fs.readFileSync(portableHighlightsPath, 'utf8'),
      }),
    }),
  }),
};
