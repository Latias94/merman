'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const Parser = require('tree-sitter');
const Mermaid = require('../bindings/node');

const packageRoot = path.join(__dirname, '..');
const headerManifestPath = path.join(packageRoot, 'metadata', 'headers.json');
const oracleScriptPath = path.join(packageRoot, 'scripts', 'header_oracle.js');
const oracleRunnerLockPath = path.join(
  packageRoot,
  'scripts',
  'header-oracle',
  'package-lock.json',
);
const oracleReceiptPath = path.join(
  packageRoot,
  'metadata',
  'evidence',
  'u2-mermaid-header-oracle.json',
);
const fixtureManifestPath = path.join(
  packageRoot,
  'metadata',
  'fixtures',
  'family-roots.json',
);
const receiptPath = path.join(
  packageRoot,
  'metadata',
  'evidence',
  'u2-header-dispatch.json',
);
const supportPath = path.join(packageRoot, 'metadata', 'support.json');
const artifactReceiptPath = path.join(
  packageRoot,
  'metadata',
  'artifact-receipt.json',
);
const receiptRelativePath = 'metadata/evidence/u2-header-dispatch.json';

function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function jsonBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`);
}

function eofCandidates(headers) {
  const candidates = [];
  const ownershipByKey = new Map();
  for (const input of headers.cases) {
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

function snapshot(path_) {
  return fs.existsSync(path_) ? fs.readFileSync(path_) : null;
}

function unchanged(path_, expected) {
  const actual = snapshot(path_);
  return actual === null ? expected === null : expected !== null && actual.equals(expected);
}

function installOutputsTransactionally(outputs, guardedFiles) {
  const transaction = fs.mkdtempSync(path.join(packageRoot, '.header-receipt-'));
  const staged = path.join(transaction, 'staged');
  const backups = path.join(transaction, 'backups');
  const installed = [];
  try {
    for (const [destination, contents] of outputs) {
      const relative = path.relative(packageRoot, destination);
      const stagedPath = path.join(staged, relative);
      fs.mkdirSync(path.dirname(stagedPath), { recursive: true });
      fs.writeFileSync(stagedPath, contents);
    }
    for (const [path_, expected] of guardedFiles) {
      if (!unchanged(path_, expected)) {
        throw new Error(`refusing to overwrite concurrently changed ${path.relative(packageRoot, path_)}`);
      }
    }
    for (const [destination] of outputs) {
      const relative = path.relative(packageRoot, destination);
      const stagedPath = path.join(staged, relative);
      const backup = path.join(backups, relative);
      const existed = fs.existsSync(destination);
      if (existed) {
        fs.mkdirSync(path.dirname(backup), { recursive: true });
        fs.renameSync(destination, backup);
      }
      installed.push({ destination, backup, existed });
      fs.mkdirSync(path.dirname(destination), { recursive: true });
      fs.renameSync(stagedPath, destination);
    }
    for (const [destination, contents] of outputs) {
      if (!fs.readFileSync(destination).equals(contents)) {
        throw new Error(`transactional install failed for ${path.relative(packageRoot, destination)}`);
      }
    }
  } catch (error) {
    for (const { destination, backup, existed } of installed.reverse()) {
      if (fs.existsSync(destination)) {
        fs.rmSync(destination, { force: true });
      }
      if (existed && fs.existsSync(backup)) {
        fs.mkdirSync(path.dirname(destination), { recursive: true });
        fs.renameSync(backup, destination);
      }
    }
    throw error;
  } finally {
    fs.rmSync(transaction, { recursive: true, force: true });
  }
}

function parseResult(parser, source) {
  const tree = parser.parse(source);
  const roots = tree.rootNode.namedChildren
    .filter((node) => node.type.endsWith('_diagram'))
    .map((node) => node.type);
  return {
    roots,
    hasError: tree.rootNode.hasError,
  };
}

function positiveCase(parser, kind, input) {
  const result = parseResult(parser, input.source);
  if (result.hasError || result.roots.length !== 1 || result.roots[0] !== input.root) {
    throw new Error(
      `${kind} case ${input.publicId} selected ${JSON.stringify(result.roots)} ` +
        `with hasError=${result.hasError}; expected ${input.root}`,
    );
  }
  return {
    kind,
    publicId: input.publicId,
    inputSha256: sha256(Buffer.from(input.source)),
    expectedRoot: input.root,
    expectedDiagramType: input.expectedDiagramType ?? null,
    actualRoot: result.roots[0],
    hasError: false,
  };
}

function validateStrictOracle(headers, headerBytes) {
  const oracleBytes = fs.readFileSync(oracleReceiptPath);
  const oracle = JSON.parse(oracleBytes);
  const { receiptId, ...body } = oracle;
  const expectedPackages = new Map(Object.entries({
    ...headers.authorities.mermaid.runtimePackages,
    '@zenuml/core': headers.authorities.zenuml.version,
  }));
  const actualPackages = new Map();
  for (const package_ of oracle.runtimePackages ?? []) {
    if (
      typeof package_.name !== 'string' ||
      typeof package_.version !== 'string' ||
      !/^[0-9a-f]{64}$/.test(package_.packageJsonSha256) ||
      actualPackages.has(package_.name)
    ) {
      throw new Error('strict-header oracle has invalid runtime package identity');
    }
    actualPackages.set(package_.name, package_.version);
  }
  if (
    oracle.schemaVersion !== 3 ||
    receiptId !== sha256(jsonBytes(body)) ||
    oracle.producer?.id !== 'tree-sitter-mermaid/mermaid-strict-header-oracle' ||
    oracle.producer?.version !== 3 ||
    oracle.producer?.command !== (
      'node scripts/header_oracle.js --node-modules ' +
      'scripts/header-oracle/node_modules'
    ) ||
    oracle.producer?.script?.path !== 'scripts/header_oracle.js' ||
    oracle.producer?.script?.sha256 !== sha256(fs.readFileSync(oracleScriptPath)) ||
    oracle.headerManifest?.path !== 'metadata/headers.json' ||
    oracle.headerManifest?.sha256 !== sha256(headerBytes) ||
    oracle.runnerLock?.path !== 'scripts/header-oracle/package-lock.json' ||
    oracle.runnerLock?.sha256 !== sha256(fs.readFileSync(oracleRunnerLockPath)) ||
    oracle.authority?.mermaid?.version !== headers.authorities.mermaid.version ||
    oracle.authority?.mermaid?.commit !== headers.authorities.mermaid.commit ||
    oracle.authority?.zenuml?.version !== headers.authorities.zenuml.version ||
    oracle.authority?.zenuml?.commit !== headers.authorities.zenuml.commit ||
    oracle.authority?.zenuml?.relationship !== headers.authorities.zenuml.relationship ||
    actualPackages.size !== expectedPackages.size ||
    [...expectedPackages].some(([name, version]) => (
      actualPackages.get(name) !== version
    ))
  ) {
    throw new Error('strict-header oracle identity or input digest drifted');
  }

  if (oracle.cases?.length !== headers.cases.length) {
    throw new Error('strict-header oracle positive case count drifted');
  }
  headers.cases.forEach((input, index) => {
    const result = oracle.cases[index];
    if (
      result?.publicId !== input.publicId ||
      result?.inputSha256 !== sha256(Buffer.from(input.source)) ||
      result?.expectedDiagramType !== input.expectedDiagramType ||
      result?.accepted !== true ||
      result?.diagramType !== input.expectedDiagramType
    ) {
      throw new Error(`strict-header oracle positive ${index} drifted`);
    }
  });
  const candidates = eofCandidates(headers);
  if (
    oracle.eofCandidateCount !== candidates.length ||
    oracle.eofCases?.length !== candidates.length
  ) {
    throw new Error('strict-header oracle EOF candidate coverage drifted');
  }
  candidates.forEach((input, index) => {
    const result = oracle.eofCases[index];
    if (
      result?.publicId !== input.publicId ||
      result?.inputSha256 !== sha256(Buffer.from(input.source)) ||
      result?.expectedDiagramType !== input.expectedDiagramType ||
      typeof result?.accepted !== 'boolean' ||
      (result.accepted
        ? result.diagramType !== input.expectedDiagramType
        : result.diagramType !== null)
    ) {
      throw new Error(`strict-header oracle EOF result ${index} drifted`);
    }
  });
  if (oracle.negativeCases?.length !== headers.strictHeaderNegatives.length) {
    throw new Error('strict-header oracle negative case count drifted');
  }
  headers.strictHeaderNegatives.forEach((source, index) => {
    const result = oracle.negativeCases[index];
    if (
      result?.inputSha256 !== sha256(Buffer.from(source)) ||
      result?.accepted !== false ||
      result?.diagramType !== null
    ) {
      throw new Error(`strict-header oracle negative ${index} drifted`);
    }
  });
  return {
    reference: {
      path: 'metadata/evidence/u2-mermaid-header-oracle.json',
      sha256: sha256(oracleBytes),
      receiptId,
    },
    acceptedEofCases: candidates.filter((_, index) => oracle.eofCases[index].accepted),
    rejectedEofCases: candidates.filter((_, index) => !oracle.eofCases[index].accepted),
  };
}

function negativeCase(parser, source) {
  const result = parseResult(parser, source);
  if (result.roots.length === 1 && !result.hasError) {
    throw new Error(
      `strict header negative was admitted as ${result.roots[0]}: ${JSON.stringify(source)}`,
    );
  }
  return {
    inputSha256: sha256(Buffer.from(source)),
    actualRoots: result.roots,
    hasError: result.hasError,
  };
}

function buildReceipt() {
  const headerBytes = fs.readFileSync(headerManifestPath);
  const fixtureBytes = fs.readFileSync(fixtureManifestPath);
  const headers = JSON.parse(headerBytes);
  const fixtures = JSON.parse(fixtureBytes);
  if (headers.schemaVersion !== 3) {
    throw new Error('header dispatch requires header manifest schema v3');
  }
  const strictOracle = validateStrictOracle(headers, headerBytes);
  const parser = new Parser();
  parser.setLanguage(Mermaid);

  const cases = [
    ...fixtures.map((fixture) => positiveCase(parser, 'baseline', fixture)),
    ...headers.cases.map((header) => positiveCase(parser, 'header', header)),
    ...strictOracle.acceptedEofCases.map((header) => (
      positiveCase(parser, 'header-eof', header)
    )),
  ];
  const negativeCases = headers.strictHeaderNegatives.map((source) => (
    negativeCase(parser, source)
  ));
  const eofNegativeCases = strictOracle.rejectedEofCases.map((input) => ({
    publicId: input.publicId,
    ...negativeCase(parser, input.source),
  }));

  return {
    schemaVersion: 5,
    producer: {
      id: 'tree-sitter-mermaid/header-dispatch',
      version: 5,
      command: 'node scripts/header_receipt.js',
    },
    artifactReceiptId: Mermaid.artifactReceipt.receiptId,
    strictOracleReceipt: strictOracle.reference,
    headerManifest: {
      path: 'metadata/headers.json',
      sha256: sha256(headerBytes),
    },
    fixtureManifest: {
      path: 'metadata/fixtures/family-roots.json',
      sha256: sha256(fixtureBytes),
    },
    cases,
    negativeCases,
    eofNegativeCases,
  };
}

function projectSupport(support, evidenceSha256, receipt) {
  const admitted = new Set(receipt.cases.map((item) => item.publicId));
  return {
    ...support,
    families: support.families.map((family) => {
      if (!admitted.has(family.publicId)) {
        throw new Error(`header receipt has no positive case for ${family.publicId}`);
      }
      const headerEvidence = {
        id: `u2-header-dispatch:${family.publicId}`,
        kind: 'header',
        path: receiptRelativePath,
        sha256: evidenceSha256,
      };
      const evidence = [
        headerEvidence,
        ...family.evidence.filter((item) => item.kind !== 'header'),
      ];
      if (family.supportTier === null) {
        return {
          ...family,
          lifecycle: 'active',
          supportTier: 'recognized',
          evidence,
        };
      }
      return {
        ...family,
        evidence,
      };
    }),
  };
}

function main() {
  const write = process.argv.slice(2).includes('--write');
  if (process.argv.slice(2).some((argument) => argument !== '--write')) {
    throw new Error('usage: node scripts/header_receipt.js [--write]');
  }

  const guardedFiles = new Map([
    headerManifestPath,
    fixtureManifestPath,
    oracleReceiptPath,
    oracleScriptPath,
    oracleRunnerLockPath,
    artifactReceiptPath,
    receiptPath,
    supportPath,
  ].map((path_) => [path_, snapshot(path_)]));
  const receipt = buildReceipt();
  const renderedReceipt = jsonBytes(receipt);
  const support = JSON.parse(fs.readFileSync(supportPath));
  const projectedSupport = projectSupport(support, sha256(renderedReceipt), receipt);
  const renderedSupport = jsonBytes(projectedSupport);

  if (write) {
    installOutputsTransactionally(
      new Map([
        [receiptPath, renderedReceipt],
        [supportPath, renderedSupport],
      ]),
      guardedFiles,
    );
  } else {
    if (!fs.existsSync(receiptPath) || !fs.readFileSync(receiptPath).equals(renderedReceipt)) {
      throw new Error('committed header-dispatch receipt is stale');
    }
    if (!fs.readFileSync(supportPath).equals(renderedSupport)) {
      throw new Error('support metadata is stale relative to header-dispatch evidence');
    }
  }

  process.stdout.write(
    `verified ${receipt.cases.length} positive and ` +
      `${receipt.negativeCases.length} strict-negative and ` +
      `${receipt.eofNegativeCases.length} EOF-negative header cases\n`,
  );
}

main();
