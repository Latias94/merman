'use strict';

const fs = require('node:fs');
const path = require('node:path');

const packageRoot = path.join(__dirname, '..', '..');
const binding = require('node-gyp-build')(packageRoot);
const portableHighlightsRelativePath = 'queries/portable/highlights.scm';
const portableHighlightsPath = path.join(packageRoot, portableHighlightsRelativePath);

binding.nodeTypeInfo = require('../../src/node-types.json');
binding.artifactReceipt = require('../../metadata/artifact-receipt.json');
binding.queryProfiles = Object.freeze({
  portable: Object.freeze({
    highlights: Object.freeze({
      relativePath: portableHighlightsRelativePath,
      path: portableHighlightsPath,
      source: fs.readFileSync(portableHighlightsPath, 'utf8'),
    }),
  }),
});

module.exports = binding;
