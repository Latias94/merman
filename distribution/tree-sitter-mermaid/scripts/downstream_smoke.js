'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const packageRoot = path.resolve(__dirname, '..');
const downstreamRoot = path.join(packageRoot, 'test', 'downstream');
const matrixPath = path.join(downstreamRoot, 'matrix.json');
const representativeFixture = path.join(
  downstreamRoot,
  'fixtures',
  'architecture.mmd',
);

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function computeSha256(value) {
  return crypto.createHash('sha256').update(value).digest('hex');
}

function canonicalPlatformKey(platform = process.platform, arch = process.arch) {
  if (!['darwin', 'linux', 'win32'].includes(platform)) {
    throw new Error(`unsupported platform: ${platform}`);
  }
  if (!['arm64', 'x64'].includes(arch)) {
    throw new Error(`unsupported architecture: ${arch}`);
  }
  return `${platform}-${arch}`;
}

function resolveAsset(editor, platformKey, label) {
  const asset = editor.assets[platformKey];
  if (!asset) throw new Error(`no fixed ${label} asset for ${platformKey}`);
  assert.match(asset.sha256, /^[0-9a-f]{64}$/, `${label} SHA-256`);
  assert.match(asset.url, /^https:\/\/github\.com\//, `${label} URL`);
  return asset;
}

function parseLatestTag(location) {
  if (!location) return null;
  const match = location.match(/\/releases\/tag\/([^/?#]+)(?:[/?#]|$)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function tomlSection(source, section) {
  if (!section) {
    const nextSection = source.search(/^\s*\[/m);
    return nextSection === -1 ? source : source.slice(0, nextSection);
  }
  const marker = new RegExp(`^\\s*\\[${escapeRegExp(section)}\\]\\s*$`, 'm');
  const match = marker.exec(source);
  if (!match) throw new Error(`missing TOML section [${section}]`);
  const bodyStart = match.index + match[0].length;
  const remainder = source.slice(bodyStart);
  const nextSection = remainder.search(/^\s*\[/m);
  return nextSection === -1 ? remainder : remainder.slice(0, nextSection);
}

function readTomlRaw(source, key, section) {
  const body = tomlSection(source, section);
  const match = new RegExp(
    `^\\s*${escapeRegExp(key)}\\s*=\\s*(.+?)\\s*(?:#.*)?$`,
    'm',
  ).exec(body);
  if (!match) {
    throw new Error(`missing TOML key ${section ? `${section}.` : ''}${key}`);
  }
  return match[1];
}

function readTomlString(source, key, section) {
  const raw = readTomlRaw(source, key, section);
  const match = raw.match(/^"((?:[^"\\]|\\.)*)"$/);
  if (!match) throw new Error(`TOML key ${key} must be a double-quoted string`);
  return JSON.parse(`"${match[1]}"`);
}

function readTomlArray(source, key, section) {
  const value = JSON.parse(readTomlRaw(source, key, section));
  if (!Array.isArray(value)) throw new Error(`TOML key ${key} must be an array`);
  return value;
}

function readTomlInteger(source, key, section) {
  const raw = readTomlRaw(source, key, section);
  if (!/^(?:0|[1-9][0-9]*)$/.test(raw)) {
    throw new Error(`TOML key ${key} must be a non-negative integer`);
  }
  return Number(raw);
}

function stripAnsi(value) {
  return value.replace(/\u001b\[[0-?]*[ -/]*[@-~]/g, '');
}

function commandText(result) {
  return `${result.stdout || ''}${result.stderr || ''}`;
}

function runCommand(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || packageRoot,
    env: options.env || process.env,
    encoding: 'utf8',
    maxBuffer: options.maxBuffer || 32 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (!options.allowFailure && result.status !== 0) {
    throw new Error(
      `${options.label || command} failed with status ${result.status}\n${commandText(result)}`,
    );
  }
  return result;
}

function assertTemporaryPath(cacheRoot, target) {
  const resolvedRoot = path.resolve(cacheRoot);
  const resolvedTarget = path.resolve(target);
  assert.ok(
    resolvedTarget.startsWith(`${resolvedRoot}${path.sep}`),
    `refusing to mutate path outside downstream cache: ${resolvedTarget}`,
  );
}

async function fetchWithRetry(url, options = {}, attempts = 3) {
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const response = await fetch(url, {
        redirect: 'follow',
        signal: AbortSignal.timeout(120_000),
        ...options,
      });
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
      return response;
    } catch (error) {
      lastError = error;
      if (attempt < attempts) {
        await new Promise((resolve) => setTimeout(resolve, 250 * (2 ** (attempt - 1))));
      }
    }
  }
  throw new Error(`download failed after ${attempts} attempts: ${url}: ${lastError.message}`);
}

async function downloadVerified(asset, cacheRoot) {
  const downloadRoot = path.join(cacheRoot, 'downloads');
  fs.mkdirSync(downloadRoot, { recursive: true });
  const archivePath = path.join(downloadRoot, asset.name);
  assertTemporaryPath(cacheRoot, archivePath);

  if (fs.existsSync(archivePath)) {
    const digest = computeSha256(fs.readFileSync(archivePath));
    if (digest === asset.sha256) return archivePath;
    throw new Error(
      `cached asset digest mismatch for ${archivePath}; expected ${asset.sha256}, got ${digest}`,
    );
  }

  const response = await fetchWithRetry(asset.url);
  const content = Buffer.from(await response.arrayBuffer());
  const digest = computeSha256(content);
  assert.equal(digest, asset.sha256, `${asset.name}: downloaded SHA-256`);

  const partialPath = `${archivePath}.partial-${process.pid}-${Date.now()}`;
  assertTemporaryPath(cacheRoot, partialPath);
  try {
    fs.writeFileSync(partialPath, content, { flag: 'wx' });
    fs.renameSync(partialPath, archivePath);
  } finally {
    if (fs.existsSync(partialPath)) fs.unlinkSync(partialPath);
  }
  return archivePath;
}

function prepareExtractedTool(label, version, asset, archivePath, cacheRoot, platformKey) {
  const safeLabel = label.toLowerCase().replace(/[^a-z0-9]+/g, '-');
  const toolRoot = path.join(
    cacheRoot,
    'tools',
    `${safeLabel}-${version}-${platformKey}-${asset.sha256.slice(0, 12)}`,
  );
  const markerPath = path.join(toolRoot, '.complete.json');
  const binaryPath = path.join(toolRoot, ...asset.binary.split('/'));
  assertTemporaryPath(cacheRoot, toolRoot);

  if (fs.existsSync(markerPath) && fs.existsSync(binaryPath)) return binaryPath;
  if (fs.existsSync(toolRoot)) fs.rmSync(toolRoot, { recursive: true, force: true });
  fs.mkdirSync(toolRoot, { recursive: true });
  runCommand('tar', ['-xf', archivePath, '-C', toolRoot], {
    label: `${label} archive extraction`,
  });
  assert.ok(fs.existsSync(binaryPath), `${label} binary missing after extraction`);
  if (process.platform !== 'win32') fs.chmodSync(binaryPath, 0o755);
  fs.writeFileSync(markerPath, `${JSON.stringify({ version, sha256: asset.sha256 })}\n`);
  return binaryPath;
}

async function prepareEditor(label, editor, platformKey, cacheRoot) {
  const asset = resolveAsset(editor, platformKey, label);
  const archivePath = await downloadVerified(asset, cacheRoot);
  const binaryPath = prepareExtractedTool(
    label,
    editor.version,
    asset,
    archivePath,
    cacheRoot,
    platformKey,
  );
  return { asset, binaryPath };
}

function treeSitterCli() {
  const executable = process.platform === 'win32' ? 'tree-sitter.cmd' : 'tree-sitter';
  const cliPath = path.join(packageRoot, 'node_modules', '.bin', executable);
  assert.ok(fs.existsSync(cliPath), 'tree-sitter CLI is not installed; run npm ci first');
  return cliPath;
}

function buildParserLibrary(runRoot) {
  const extension = process.platform === 'win32' ? 'dll' : 'so';
  const outputPath = path.join(runRoot, `mermaid.${extension}`);
  runCommand(treeSitterCli(), ['build', '--output', outputPath, packageRoot], {
    label: 'temporary native parser build',
  });
  assert.ok(fs.existsSync(outputPath), 'temporary parser library was not created');
  return outputPath;
}

function profileFiles(profile, expectedSurfaces) {
  const queryRoot = path.join(packageRoot, 'queries', profile);
  assert.ok(fs.existsSync(queryRoot), `${profile} query profile is missing`);
  const actual = fs.readdirSync(queryRoot)
    .filter((name) => name.endsWith('.scm'))
    .map((name) => name.replace(/\.scm$/, ''))
    .sort();
  assert.deepEqual(actual, [...expectedSurfaces].sort(), `${profile} query surface set`);
  return Object.fromEntries(expectedSurfaces.map((surface) => [
    surface,
    path.join(queryRoot, `${surface}.scm`),
  ]));
}

function isolatedEnvironment(runRoot, name) {
  const config = path.join(runRoot, `${name}-config`);
  const cache = path.join(runRoot, `${name}-cache`);
  const data = path.join(runRoot, `${name}-data`);
  for (const directory of [config, cache, data]) {
    fs.mkdirSync(directory, { recursive: true });
  }
  return {
    ...process.env,
    XDG_CONFIG_HOME: config,
    XDG_CACHE_HOME: cache,
    XDG_DATA_HOME: data,
    APPDATA: config,
    LOCALAPPDATA: data,
  };
}

function runNeovim(editor, prepared, parserLibrary, runRoot) {
  profileFiles('neovim', editor.profileSurfaces);
  const version = runCommand(prepared.binaryPath, ['--version'], {
    label: 'Neovim version probe',
  });
  assert.ok(commandText(version).startsWith(editor.versionPrefix), 'unexpected Neovim version');

  const environment = {
    ...isolatedEnvironment(runRoot, 'neovim'),
    MERMAID_PACKAGE_ROOT: packageRoot,
    MERMAID_PARSER_LIBRARY: parserLibrary,
  };
  const smokePath = path.join(packageRoot, 'test', 'queries', 'neovim', 'smoke.lua');
  assert.ok(fs.existsSync(smokePath), 'Neovim profile smoke.lua is missing');
  const smoke = runCommand(
    prepared.binaryPath,
    ['--clean', '--headless', '-u', 'NONE', '-i', 'NONE', '-l', smokePath],
    { env: environment, label: 'Neovim headless query matrix' },
  );
  const output = stripAnsi(commandText(smoke));
  assert.match(output, /Neovim query matrix: 315 cells passed/);
  return { version: editor.version, output: output.trim().split('\n').slice(-10) };
}

function runTreeSitterQuery(parserLibrary, queryPath, fixturePath, options = {}) {
  const result = runCommand(
    treeSitterCli(),
    [
      'query',
      '--lib-path',
      parserLibrary,
      '--lang-name',
      'mermaid',
      '--captures',
      queryPath,
      fixturePath,
    ],
    {
      allowFailure: options.allowFailure,
      label: options.label || `${path.basename(queryPath)} query`,
    },
  );
  const output = stripAnsi(commandText(result));
  return {
    ...result,
    output,
    captureCount: (output.match(/capture:/g) || []).length,
  };
}

function copyFile(source, destination) {
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.copyFileSync(source, destination);
}

function runHelix(editor, prepared, parserLibrary, runRoot) {
  const queries = profileFiles('helix', editor.profileSurfaces);
  const version = runCommand(prepared.binaryPath, ['--version'], {
    label: 'Helix version probe',
  });
  assert.ok(commandText(version).startsWith(editor.versionPrefix), 'unexpected Helix version');

  const runtimeRoot = path.join(runRoot, 'helix-runtime');
  const configRoot = path.join(runRoot, 'helix-config');
  const grammarExtension = process.platform === 'win32' ? 'dll' : 'so';
  copyFile(
    parserLibrary,
    path.join(runtimeRoot, 'grammars', `mermaid.${grammarExtension}`),
  );
  for (const [surface, queryPath] of Object.entries(queries)) {
    copyFile(queryPath, path.join(runtimeRoot, 'queries', 'mermaid', `${surface}.scm`));
  }
  copyFile(
    path.join(downstreamRoot, 'helix', 'languages.toml'),
    path.join(configRoot, 'helix', 'languages.toml'),
  );

  const environment = {
    ...isolatedEnvironment(runRoot, 'helix'),
    XDG_CONFIG_HOME: configRoot,
    HELIX_RUNTIME: runtimeRoot,
    NO_COLOR: '1',
  };
  const health = runCommand(prepared.binaryPath, ['--health', 'mermaid'], {
    env: environment,
    label: 'Helix headless health probe',
  });
  const healthOutput = stripAnsi(commandText(health));
  for (const expected of [
    'Tree-sitter parser: ✓',
    'Highlight queries: ✓',
    'Textobject queries: ✓',
    'Indent queries: ✓',
  ]) {
    assert.ok(healthOutput.includes(expected), `Helix health missing: ${expected}`);
  }

  const fixtures = {
    highlights: path.join(packageRoot, 'test', 'queries', 'helix', 'highlights', 'flowchart.mmd'),
    injections: path.join(packageRoot, 'test', 'queries', 'helix', 'injections', 'frontmatter.mmd'),
    locals: path.join(packageRoot, 'test', 'queries', 'helix', 'locals', 'architecture.mmd'),
    indents: path.join(packageRoot, 'test', 'queries', 'helix', 'indents', 'sequence.mmd'),
    textobjects: path.join(packageRoot, 'test', 'queries', 'helix', 'textobjects', 'railroad-ebnf.mmd'),
  };
  const captures = {};
  for (const surface of editor.profileSurfaces) {
    assert.ok(fs.existsSync(fixtures[surface]), `Helix ${surface} fixture is missing`);
    const query = runTreeSitterQuery(parserLibrary, queries[surface], fixtures[surface], {
      label: `Helix ${surface} query execution`,
    });
    assert.ok(query.captureCount > 0, `Helix ${surface} query produced no captures`);
    captures[surface] = query.captureCount;
  }
  return { version: editor.version, health: healthOutput.trim().split('\n'), captures };
}

function assertQueryCaptures(queryPath, requiredCaptures) {
  const source = fs.readFileSync(queryPath, 'utf8');
  for (const capture of requiredCaptures) {
    assert.match(source, new RegExp(`@${escapeRegExp(capture)}\\b`), `${queryPath}: @${capture}`);
  }
  return source;
}

async function runZed(zed, parserLibrary) {
  const queries = profileFiles('zed', zed.profileSurfaces);
  const extensionPath = path.join(downstreamRoot, 'zed', 'extension.toml');
  const languagePath = path.join(
    downstreamRoot,
    'zed',
    'languages',
    'mermaid',
    'config.toml',
  );
  const extension = fs.readFileSync(extensionPath, 'utf8');
  const language = fs.readFileSync(languagePath, 'utf8');

  assert.equal(readTomlString(extension, 'id'), 'mermaid');
  assert.equal(readTomlInteger(extension, 'schema_version'), 1);
  assert.equal(readTomlString(extension, 'repository', 'grammars.mermaid'), 'https://github.com/Latias94/merman');
  const grammarCommit = readTomlString(extension, 'rev', 'grammars.mermaid');
  assert.match(grammarCommit, /^[0-9a-f]{40}$/, 'Zed grammar commit');
  const grammarPath = readTomlString(extension, 'path', 'grammars.mermaid');
  assert.equal(grammarPath, 'distribution/tree-sitter-mermaid');
  runCommand('git', ['cat-file', '-e', `${grammarCommit}:${grammarPath}/src/parser.c`], {
    cwd: path.resolve(packageRoot, '..', '..'),
    label: 'Zed grammar commit/path pin',
  });
  const pinnedParser = runCommand(
    'git',
    ['show', `${grammarCommit}:${grammarPath}/src/parser.c`],
    {
      cwd: path.resolve(packageRoot, '..', '..'),
      label: 'Zed pinned parser snapshot',
    },
  ).stdout;
  const currentParser = fs.readFileSync(path.join(packageRoot, 'src', 'parser.c'));
  const grammarPin = {
    pinnedParserSha256: computeSha256(pinnedParser),
    currentParserSha256: computeSha256(currentParser),
  };
  grammarPin.matchesCurrent = grammarPin.pinnedParserSha256 === grammarPin.currentParserSha256;

  const parserDiff = runCommand(
    'git',
    ['diff', '--quiet', 'HEAD', '--', `${grammarPath}/src/parser.c`],
    {
      cwd: path.resolve(packageRoot, '..', '..'),
      allowFailure: true,
      label: 'Zed parser integration state',
    },
  );
  assert.ok([0, 1].includes(parserDiff.status), 'unable to inspect Zed parser integration state');
  grammarPin.pendingIntegration = !grammarPin.matchesCurrent && parserDiff.status === 1;
  if (!grammarPin.matchesCurrent && !grammarPin.pendingIntegration) {
    throw new Error(
      `Zed grammar rev ${grammarCommit} does not match the clean checkout parser; `
      + 'pin a compatible grammar commit',
    );
  }

  assert.equal(readTomlString(language, 'name'), 'Mermaid');
  assert.equal(readTomlString(language, 'grammar'), zed.grammarName);
  assert.deepEqual(readTomlArray(language, 'path_suffixes'), ['mmd', 'mermaid']);

  const treeSitterManifest = JSON.parse(fs.readFileSync(
    path.join(packageRoot, 'tree-sitter.json'),
    'utf8',
  ));
  assert.equal(treeSitterManifest.grammars.length, 1);
  assert.equal(treeSitterManifest.grammars[0].name, zed.grammarName);

  const parserSource = fs.readFileSync(path.join(packageRoot, 'src', 'parser.c'), 'utf8');
  const abiMatch = parserSource.match(/^#define LANGUAGE_VERSION ([0-9]+)$/m);
  assert.ok(abiMatch, 'generated parser ABI macro is missing');
  assert.equal(Number(abiMatch[1]), zed.grammarAbi);

  const { Language, Parser } = require('web-tree-sitter');
  await Parser.init();
  const wasmLanguage = await Language.load(
    path.join(packageRoot, 'wasm', 'tree-sitter-mermaid.wasm'),
  );
  assert.equal(wasmLanguage.abiVersion, zed.grammarAbi);

  const requiredCaptures = {
    highlights: ['keyword'],
    brackets: ['open', 'close'],
    outline: ['item', 'name'],
    indents: ['indent', 'end'],
    injections: ['injection.content'],
    textobjects: ['class.around', 'function.around', 'comment.around'],
  };
  const fixtures = {
    highlights: representativeFixture,
    brackets: representativeFixture,
    outline: representativeFixture,
    indents: representativeFixture,
    injections: path.join(packageRoot, 'test', 'queries', 'helix', 'injections', 'frontmatter.mmd'),
    textobjects: representativeFixture,
  };
  const captures = {};
  for (const surface of zed.profileSurfaces) {
    const source = assertQueryCaptures(queries[surface], requiredCaptures[surface]);
    if (surface === 'injections') assert.match(source, /#set!\s+injection\.language/);
    const query = runTreeSitterQuery(parserLibrary, queries[surface], fixtures[surface], {
      label: `Zed ${surface} query execution`,
    });
    assert.ok(query.captureCount > 0, `Zed ${surface} query produced no captures`);
    captures[surface] = query.captureCount;
  }
  return {
    version: zed.version,
    referenceCommit: zed.referenceCommit,
    grammarCommit,
    grammarPin,
    abi: zed.grammarAbi,
    captures,
  };
}

async function runMigration(migration, parserLibrary, cacheRoot) {
  const oldQueryPath = await downloadVerified({
    name: `monaqa-highlights-${migration.monaqaCommit.slice(0, 7)}.scm`,
    url: migration.queryUrl,
    sha256: migration.querySha256,
  }, cacheRoot);
  const oldQuery = runTreeSitterQuery(
    parserLibrary,
    oldQueryPath,
    representativeFixture,
    { allowFailure: true, label: 'monaqa compatibility replay' },
  );
  assert.notEqual(oldQuery.status, 0, 'legacy monaqa query unexpectedly compiled');
  assert.match(oldQuery.output, /Invalid node type "sequenceDiagram"/);

  const newQuery = runTreeSitterQuery(
    parserLibrary,
    path.join(packageRoot, 'queries', 'portable', 'highlights.scm'),
    representativeFixture,
    { label: 'portable migration target query' },
  );
  assert.ok(newQuery.captureCount > 0, 'portable query produced no captures');
  assert.match(newQuery.output, /capture:\s+\d+\s+-\s+(?:namespace|variable|operator)/);
  return {
    sourceCommit: migration.monaqaCommit,
    legacyFailure: 'Invalid node type "sequenceDiagram"',
    portableCaptureCount: newQuery.captureCount,
  };
}

async function probeLatest(label, editor) {
  const response = await fetch(editor.latestRelease, {
    method: 'HEAD',
    redirect: 'manual',
    signal: AbortSignal.timeout(15_000),
  });
  const tag = parseLatestTag(response.headers.get('location'));
  if (!tag) throw new Error(`${label} latest release did not return a tag redirect`);
  return {
    fixed: editor.version,
    latest: tag.replace(/^v/, ''),
    drifted: tag.replace(/^v/, '') !== editor.version,
  };
}

async function latestProbes(matrix) {
  const results = {};
  const warnings = [];
  const consumers = { ...matrix.editors, zed: matrix.zed };
  for (const [name, consumer] of Object.entries(consumers)) {
    try {
      results[name] = await probeLatest(name, consumer);
    } catch (error) {
      warnings.push(`${name} latest probe unavailable: ${error.message}`);
      results[name] = { fixed: consumer.version, latest: null, drifted: null };
    }
  }
  return { results, warnings };
}

function loadMatrix() {
  const matrix = JSON.parse(fs.readFileSync(matrixPath, 'utf8'));
  assert.equal(matrix.schemaVersion, 1);
  assert.match(matrix.cacheNamespace, /^[a-z0-9-]+$/);
  return matrix;
}

async function main() {
  const matrix = loadMatrix();
  const platformKey = canonicalPlatformKey();
  const cacheRoot = path.join(os.tmpdir(), matrix.cacheNamespace);
  fs.mkdirSync(cacheRoot, { recursive: true });

  const neovim = await prepareEditor(
    'Neovim',
    matrix.editors.neovim,
    platformKey,
    cacheRoot,
  );
  const helix = await prepareEditor(
    'Helix',
    matrix.editors.helix,
    platformKey,
    cacheRoot,
  );
  const runRoot = fs.mkdtempSync(path.join(cacheRoot, 'run-'));
  assertTemporaryPath(cacheRoot, runRoot);

  try {
    const parserLibrary = buildParserLibrary(runRoot);
    const zed = await runZed(matrix.zed, parserLibrary);
    const results = {
      platform: platformKey,
      neovim: {
        asset: neovim.asset,
        ...(runNeovim(matrix.editors.neovim, neovim, parserLibrary, runRoot)),
      },
      helix: {
        asset: helix.asset,
        ...(runHelix(matrix.editors.helix, helix, parserLibrary, runRoot)),
      },
      zed,
      migration: await runMigration(matrix.migration, parserLibrary, cacheRoot),
    };
    const latest = await latestProbes(matrix);
    results.latest = latest.results;
    results.warnings = [...latest.warnings];
    if (zed.grammarPin.pendingIntegration) {
      results.warnings.push(
        `Zed grammar rev ${zed.grammarCommit} predates the dirty local parser; `
        + 'update the rev after the grammar integration commit',
      );
    }
    process.stdout.write(`${JSON.stringify(results, null, 2)}\n`);
  } finally {
    if (fs.existsSync(runRoot)) fs.rmSync(runRoot, { recursive: true, force: true });
  }
}

module.exports = {
  canonicalPlatformKey,
  computeSha256,
  parseLatestTag,
  readTomlArray,
  readTomlString,
  resolveAsset,
};

if (require.main === module) {
  main().catch((error) => {
    process.stderr.write(`${error.stack || error.message}\n`);
    process.exitCode = 1;
  });
}
