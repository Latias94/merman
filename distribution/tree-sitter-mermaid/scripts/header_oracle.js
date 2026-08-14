'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const packageRoot = path.join(__dirname, '..');
const manifestPath = path.join(packageRoot, 'metadata', 'headers.json');
const runnerLockPath = path.join(__dirname, 'header-oracle', 'package-lock.json');
const receiptPath = path.join(
  packageRoot,
  'metadata',
  'evidence',
  'u2-mermaid-header-oracle.json',
);

function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function jsonBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`);
}

function eofCandidates(manifest) {
  const candidates = [];
  const ownershipByKey = new Map();
  for (const input of manifest.cases) {
    const source = input.source.match(/^[^\r\n]*/u)?.[0] ?? '';
    const key = `${input.publicId}\0${source}`;
    const existing = ownershipByKey.get(key);
    if (
      existing &&
      (existing.root !== input.root ||
        existing.expectedDiagramType !== input.expectedDiagramType)
    ) {
      throw new Error(
        `EOF header candidate ${JSON.stringify(source)} has conflicting ownership`,
      );
    }
    if (!existing) {
      ownershipByKey.set(key, {
        root: input.root,
        expectedDiagramType: input.expectedDiagramType,
      });
      candidates.push({
        publicId: input.publicId,
        root: input.root,
        expectedDiagramType: input.expectedDiagramType,
        source,
      });
    }
  }
  return candidates;
}

function parseArguments() {
  const arguments_ = process.argv.slice(2);
  let nodeModules;
  let write = false;
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === '--write') {
      write = true;
    } else if (argument === '--node-modules' && index + 1 < arguments_.length) {
      nodeModules = path.resolve(arguments_[index + 1]);
      index += 1;
    } else {
      throw new Error(
        'usage: node scripts/header_oracle.js --node-modules <path> [--write]',
      );
    }
  }
  if (!nodeModules || !fs.statSync(nodeModules, { throwIfNoEntry: false })?.isDirectory()) {
    throw new Error('--node-modules must identify an installed oracle node_modules directory');
  }
  return { nodeModules, write };
}

function packageMetadata(nodeModules, name, expectedVersion) {
  const packagePath = path.join(nodeModules, name, 'package.json');
  const bytes = fs.readFileSync(packagePath);
  const metadata = JSON.parse(bytes);
  if (metadata.name !== name || metadata.version !== expectedVersion) {
    throw new Error(
      `${name} identity drifted: expected ${expectedVersion}, got ` +
        `${metadata.name}@${metadata.version}`,
    );
  }
  return {
    name,
    version: metadata.version,
    packageJsonSha256: sha256(bytes),
  };
}

async function loadRuntime(nodeModules) {
  const importPackage = async (name) => {
    const packageRoot = path.join(nodeModules, name);
    const metadata = JSON.parse(
      fs.readFileSync(path.join(packageRoot, 'package.json')),
    );
    const rootExport = metadata.exports?.['.'];
    const entry = (
      (typeof rootExport === 'string' ? rootExport : rootExport?.import) ||
      (typeof rootExport === 'object' ? rootExport.default : undefined) ||
      metadata.module ||
      metadata.main
    );
    if (typeof entry !== 'string') {
      throw new Error(`${name} has no importable package entry`);
    }
    return import(pathToFileURL(path.resolve(packageRoot, entry)).href);
  };
  const { JSDOM } = await importPackage('jsdom');
  const dom = new JSDOM('<!doctype html><html><body></body></html>', {
    url: 'http://localhost/',
  });
  for (const name of [
    'window',
    'document',
    'navigator',
    'location',
    'Element',
    'HTMLElement',
    'SVGElement',
    'localStorage',
  ]) {
    Object.defineProperty(globalThis, name, {
      value: dom.window[name],
      configurable: true,
    });
  }

  const { default: mermaid } = await importPackage('mermaid');
  const { default: zenuml } = await importPackage('@mermaid-js/mermaid-zenuml');
  mermaid.initialize({ startOnLoad: false });
  await mermaid.registerExternalDiagrams([zenuml]);
  return mermaid;
}

async function buildReceipt(nodeModules) {
  const manifestBytes = fs.readFileSync(manifestPath);
  const runnerLockBytes = fs.readFileSync(runnerLockPath);
  const scriptBytes = fs.readFileSync(__filename);
  const manifest = JSON.parse(manifestBytes);
  if (manifest.schemaVersion !== 3) {
    throw new Error('header oracle requires header manifest schema v3');
  }

  const expectedPackages = {
    ...manifest.authorities.mermaid.runtimePackages,
    '@zenuml/core': manifest.authorities.zenuml.version,
  };
  const packages = Object.entries(expectedPackages).map(([name, version]) => (
    packageMetadata(nodeModules, name, version)
  ));
  const mermaid = await loadRuntime(nodeModules);

  const cases = [];
  for (const input of manifest.cases) {
    let result;
    try {
      result = await mermaid.parse(input.source, { suppressErrors: false });
    } catch (error) {
      throw new Error(
        `strict Mermaid rejected positive ${input.publicId} case: ` +
          `${JSON.stringify(input.source)}: ${String(error).split('\n')[0]}`,
      );
    }
    if (result.diagramType !== input.expectedDiagramType) {
      throw new Error(
        `strict Mermaid attributed ${input.publicId} case to ${result.diagramType}; ` +
          `expected ${input.expectedDiagramType}: ${JSON.stringify(input.source)}`,
      );
    }
    cases.push({
      publicId: input.publicId,
      inputSha256: sha256(Buffer.from(input.source)),
      expectedDiagramType: input.expectedDiagramType,
      accepted: true,
      diagramType: result.diagramType,
    });
  }

  const negativeCases = [];
  for (const source of manifest.strictHeaderNegatives) {
    let accepted = false;
    let diagramType = null;
    try {
      const result = await mermaid.parse(source, { suppressErrors: false });
      accepted = true;
      diagramType = result.diagramType;
    } catch {
      // A strict negative is expected to throw.
    }
    if (accepted) {
      throw new Error(
        `strict Mermaid admitted negative as ${diagramType}: ${JSON.stringify(source)}`,
      );
    }
    negativeCases.push({
      inputSha256: sha256(Buffer.from(source)),
      accepted: false,
      diagramType: null,
    });
  }

  const eofInputs = eofCandidates(manifest);
  const eofCases = [];
  for (const input of eofInputs) {
    let accepted = false;
    let diagramType = null;
    try {
      const result = await mermaid.parse(input.source, { suppressErrors: false });
      accepted = true;
      diagramType = result.diagramType;
    } catch {
      // EOF probes include headers whose strict grammar requires a body.
    }
    if (accepted && diagramType !== input.expectedDiagramType) {
      throw new Error(
        `strict Mermaid attributed EOF ${input.publicId} case to ${diagramType}; ` +
          `expected ${input.expectedDiagramType}: ${JSON.stringify(input.source)}`,
      );
    }
    eofCases.push({
      publicId: input.publicId,
      inputSha256: sha256(Buffer.from(input.source)),
      expectedDiagramType: input.expectedDiagramType,
      accepted,
      diagramType,
    });
  }

  const body = {
    schemaVersion: 3,
    producer: {
      id: 'tree-sitter-mermaid/mermaid-strict-header-oracle',
      version: 3,
      command: (
        'node scripts/header_oracle.js --node-modules ' +
        'scripts/header-oracle/node_modules'
      ),
      script: {
        path: 'scripts/header_oracle.js',
        sha256: sha256(scriptBytes),
      },
    },
    authority: {
      mermaid: {
        version: manifest.authorities.mermaid.version,
        commit: manifest.authorities.mermaid.commit,
      },
      zenuml: {
        version: manifest.authorities.zenuml.version,
        commit: manifest.authorities.zenuml.commit,
        relationship: manifest.authorities.zenuml.relationship,
      },
    },
    headerManifest: {
      path: 'metadata/headers.json',
      sha256: sha256(manifestBytes),
    },
    runnerLock: {
      path: 'scripts/header-oracle/package-lock.json',
      sha256: sha256(runnerLockBytes),
    },
    runtimePackages: packages,
    cases,
    eofCandidateCount: eofInputs.length,
    eofCases,
    negativeCases,
  };
  return {
    receiptId: sha256(jsonBytes(body)),
    ...body,
  };
}

async function main() {
  const { nodeModules, write } = parseArguments();
  const receipt = await buildReceipt(nodeModules);
  const rendered = jsonBytes(receipt);
  if (write) {
    fs.mkdirSync(path.dirname(receiptPath), { recursive: true });
    fs.writeFileSync(receiptPath, rendered);
  } else if (
    !fs.existsSync(receiptPath) ||
    !fs.readFileSync(receiptPath).equals(rendered)
  ) {
    throw new Error('committed Mermaid strict-header oracle receipt is stale');
  }
  process.stdout.write(
    `verified ${receipt.cases.length} strict positives and ` +
      `${receipt.eofCases.filter((item) => item.accepted).length}/` +
      `${receipt.eofCandidateCount} EOF headers and ` +
      `${receipt.negativeCases.length} strict negatives\n`,
  );
}

main().catch((error) => {
  process.stderr.write(`${error.stack || error}\n`);
  process.exitCode = 1;
});
