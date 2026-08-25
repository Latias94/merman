'use strict';

const { spawnSync } = require('node:child_process');

const requested = process.argv.slice(2);
if (requested.length === 0) {
  throw new Error('usage: node scripts/run_python.js <python arguments>');
}

const configured = (
  process.env.TREE_SITTER_MERMAID_PYTHON ||
  process.env.PYTHON ||
  process.env.npm_config_python
);
const candidates = configured
  ? [[configured, []]]
  : process.platform === 'win32'
    ? [['py', ['-3']], ['python', []], ['python3', []]]
    : [['python3', []], ['python', []]];

for (const [command, prefix] of candidates) {
  const result = spawnSync(command, [...prefix, ...requested], {
    stdio: 'inherit',
    env: {
      ...process.env,
      TREE_SITTER_MERMAID_NODE: process.execPath,
    },
  });
  if (result.error?.code === 'ENOENT') {
    continue;
  }
  if (result.error) {
    throw result.error;
  }
  if (result.signal) {
    process.kill(process.pid, result.signal);
  }
  process.exit(result.status ?? 1);
}

throw new Error(
  'Python 3 was not found; set TREE_SITTER_MERMAID_PYTHON to its executable',
);
