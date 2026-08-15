'use strict';

const path = require('node:path');
const { loadQueryProfiles } = require('../query-profiles');

const packageRoot = path.join(__dirname, '..', '..');
const binding = require('node-gyp-build')(packageRoot);
const artifactReceipt = require('../../metadata/artifact-receipt.json');

binding.nodeTypeInfo = require('../../src/node-types.json');
binding.artifactReceipt = artifactReceipt;
binding.queryProfiles = loadQueryProfiles(packageRoot, artifactReceipt);

module.exports = binding;
