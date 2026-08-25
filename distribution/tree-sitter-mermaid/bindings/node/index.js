'use strict';

const path = require('node:path');

const packageRoot = path.join(__dirname, '..', '..');
const language = require('node-gyp-build')(packageRoot);

language.nodeTypeInfo = require('../../src/node-types.json');

module.exports = language;
