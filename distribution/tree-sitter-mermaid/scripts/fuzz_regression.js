'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const packageRoot = path.resolve(__dirname, '..');
const workspaceRoot = path.resolve(packageRoot, '..', '..');
const nightlyToolchain = 'nightly-2026-07-01';
const dictionary = path.join(workspaceRoot, 'fuzz', 'dictionaries', 'mermaid.dict');

const targets = [
  {
    name: 'tree_sitter_mermaid_parse',
    seeds: [path.join(packageRoot, 'fuzz', 'corpus', 'all-families')],
    maxLength: 262_144,
  },
  {
    name: 'tree_sitter_mermaid_edits',
    seeds: [path.join(workspaceRoot, 'fuzz', 'seeds', 'tree-sitter-edits')],
    maxLength: 262_144,
  },
  {
    name: 'tree_sitter_mermaid_scanner',
    seeds: [path.join(workspaceRoot, 'fuzz', 'seeds', 'tree-sitter-scanner')],
    maxLength: 16_384,
  },
  {
    name: 'tree_sitter_mermaid_query',
    seeds: [path.join(packageRoot, 'fuzz', 'corpus', 'all-families')],
    maxLength: 65_536,
  },
];

function assertDirectory(directory) {
  if (!fs.existsSync(directory) || !fs.statSync(directory).isDirectory()) {
    throw new Error(`missing fuzz regression directory: ${directory}`);
  }
}

function runCargoFuzz(target, writableCorpus) {
  const args = [
    `+${nightlyToolchain}`,
    'fuzz',
    'run',
    '--fuzz-dir',
    'fuzz',
    '--sanitizer',
    'address',
    target.name,
    writableCorpus,
    ...target.seeds,
    '--',
    '-runs=64',
    '-timeout=10',
    `-max_len=${target.maxLength}`,
    `-dict=${dictionary}`,
  ];
  const result = spawnSync('cargo', args, {
    cwd: workspaceRoot,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    process.stdout.write(result.stdout ?? '');
    process.stderr.write(result.stderr ?? '');
    throw new Error(`cargo fuzz regression failed for ${target.name}`);
  }
  process.stdout.write(result.stdout ?? '');
  process.stderr.write(result.stderr ?? '');
}

function main() {
  assertDirectory(path.dirname(dictionary));
  for (const target of targets) {
    for (const seed of target.seeds) assertDirectory(seed);
  }

  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'tree-sitter-mermaid-fuzz-'));
  try {
    for (const target of targets) {
      const writableCorpus = path.join(tempRoot, target.name);
      fs.mkdirSync(writableCorpus);
      runCargoFuzz(target, writableCorpus);
    }
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

try {
  main();
} catch (error) {
  console.error(error.stack || error);
  process.exitCode = 1;
}
