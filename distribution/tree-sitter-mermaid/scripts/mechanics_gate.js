'use strict';

const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { performance } = require('node:perf_hooks');

const packageRoot = path.join(__dirname, '..');
const workspaceRoot = path.join(packageRoot, '..', '..');
const metrics = require('../metadata/metrics/u2-mechanics.json');
const familyFixtures = require('../metadata/fixtures/family-roots.json');
const querySource = fs.readFileSync(
  path.join(packageRoot, 'queries', 'portable', 'highlights.scm'),
  'utf8',
);

const TRIALS = 3;
const TIME_BASELINE_MULTIPLIER = 4;
const TIME_NOISE_FLOOR_MILLISECONDS = 250;
const RSS_BASELINE_MULTIPLIER = 2;
const RSS_NOISE_FLOOR_BYTES = 64 * 1024 * 1024;
const WASM_PAGE_NOISE_FLOOR = 256;

function shortFlowchart(targetKiB) {
  const header = 'flowchart TD\n';
  const statement = '  A --> B\n';
  const statementCount = Math.ceil(
    (targetKiB * 1024 - Buffer.byteLength(header)) / Buffer.byteLength(statement),
  );
  return header + statement.repeat(statementCount);
}

function elapsedMilliseconds(started) {
  return performance.now() - started;
}

function maximumResidentSetBytes() {
  return process.resourceUsage().maxRSS * 1024;
}

function verifyTree(tree, targetKiB) {
  assert.equal(tree.rootNode.type, 'source_file');
  assert.equal(tree.rootNode.hasError, false, `${targetKiB} KiB source has parse errors`);
}

function verifyFamilyTree(tree, fixture) {
  assert.equal(tree.rootNode.type, 'source_file', fixture.publicId);
  assert.equal(tree.rootNode.hasError, false, fixture.publicId);
  const roots = tree.rootNode.namedChildren.filter((node) => node.type.endsWith('_diagram'));
  assert.deepEqual(roots.map((node) => node.type), [fixture.root], fixture.publicId);
}

function nativeObservation(targetKiB) {
  const Parser = require('tree-sitter');
  const Mermaid = require('../bindings/node');
  const source = shortFlowchart(targetKiB);
  const parser = new Parser();
  parser.setLanguage(Mermaid);

  const compileStarted = performance.now();
  const query = new Parser.Query(Mermaid, querySource);
  const queryCompileMilliseconds = elapsedMilliseconds(compileStarted);
  const parseStarted = performance.now();
  const tree = parser.parse(source);
  const parseMilliseconds = elapsedMilliseconds(parseStarted);
  verifyTree(tree, targetKiB);
  const queryStarted = performance.now();
  const captures = query.captures(tree.rootNode);
  const queryMilliseconds = elapsedMilliseconds(queryStarted);
  assert.ok(captures.some((capture) => capture.name === 'keyword'));

  return {
    sourceBytes: Buffer.byteLength(source),
    parseMilliseconds,
    queryCompileMilliseconds,
    queryMilliseconds,
    maximumResidentSetBytes: maximumResidentSetBytes(),
  };
}

async function wasmObservation(targetKiB) {
  const { Language, Parser, Query } = require('web-tree-sitter');
  const wasmBinding = require('../bindings/wasm');
  const memory = new WebAssembly.Memory({ initial: 512, maximum: 2048 });
  await Parser.init({ wasmMemory: memory });
  const language = await Language.load(wasmBinding.languagePath);
  const source = shortFlowchart(targetKiB);
  const parser = new Parser();
  parser.setLanguage(language);

  const compileStarted = performance.now();
  const query = new Query(language, querySource);
  const queryCompileMilliseconds = elapsedMilliseconds(compileStarted);
  const parseStarted = performance.now();
  const tree = parser.parse(source);
  const parseMilliseconds = elapsedMilliseconds(parseStarted);
  verifyTree(tree, targetKiB);
  const queryStarted = performance.now();
  const captures = query.captures(tree.rootNode);
  const queryMilliseconds = elapsedMilliseconds(queryStarted);
  assert.ok(captures.some((capture) => capture.name === 'keyword'));
  const memoryPages = memory.buffer.byteLength / 65536;

  tree.delete();
  query.delete();
  parser.delete();
  return {
    sourceBytes: Buffer.byteLength(source),
    parseMilliseconds,
    queryCompileMilliseconds,
    queryMilliseconds,
    maximumResidentSetBytes: maximumResidentSetBytes(),
    memoryPages,
  };
}

function nativeRealCorpusObservation() {
  const Parser = require('tree-sitter');
  const Mermaid = require('../bindings/node');
  const parser = new Parser();
  parser.setLanguage(Mermaid);
  const compileStarted = performance.now();
  const query = new Parser.Query(Mermaid, querySource);
  const queryCompileMilliseconds = elapsedMilliseconds(compileStarted);
  let parseMilliseconds = 0;
  let queryMilliseconds = 0;
  let sourceBytes = 0;
  for (const fixture of familyFixtures) {
    sourceBytes += Buffer.byteLength(fixture.source);
    const parseStarted = performance.now();
    const tree = parser.parse(fixture.source);
    parseMilliseconds += elapsedMilliseconds(parseStarted);
    verifyFamilyTree(tree, fixture);
    const queryStarted = performance.now();
    query.captures(tree.rootNode);
    queryMilliseconds += elapsedMilliseconds(queryStarted);
  }
  return {
    fixtureCount: familyFixtures.length,
    sourceBytes,
    parseMilliseconds,
    queryCompileMilliseconds,
    queryMilliseconds,
    maximumResidentSetBytes: maximumResidentSetBytes(),
  };
}

async function wasmRealCorpusObservation() {
  const { Language, Parser, Query } = require('web-tree-sitter');
  const wasmBinding = require('../bindings/wasm');
  const memory = new WebAssembly.Memory({ initial: 512, maximum: 2048 });
  await Parser.init({ wasmMemory: memory });
  const language = await Language.load(wasmBinding.languagePath);
  const parser = new Parser();
  parser.setLanguage(language);
  const compileStarted = performance.now();
  const query = new Query(language, querySource);
  const queryCompileMilliseconds = elapsedMilliseconds(compileStarted);
  let parseMilliseconds = 0;
  let queryMilliseconds = 0;
  let sourceBytes = 0;
  for (const fixture of familyFixtures) {
    sourceBytes += Buffer.byteLength(fixture.source);
    const parseStarted = performance.now();
    const tree = parser.parse(fixture.source);
    parseMilliseconds += elapsedMilliseconds(parseStarted);
    verifyFamilyTree(tree, fixture);
    const queryStarted = performance.now();
    query.captures(tree.rootNode);
    queryMilliseconds += elapsedMilliseconds(queryStarted);
    tree.delete();
  }
  const memoryPages = memory.buffer.byteLength / 65536;
  query.delete();
  parser.delete();
  return {
    fixtureCount: familyFixtures.length,
    sourceBytes,
    parseMilliseconds,
    queryCompileMilliseconds,
    queryMilliseconds,
    maximumResidentSetBytes: maximumResidentSetBytes(),
    memoryPages,
  };
}

function median(values) {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.floor(ordered.length / 2)];
}

function aggregateObservations(observations) {
  const result = {};
  for (const field of Object.keys(observations[0])) {
    result[field] = median(observations.map((observation) => observation[field]));
  }
  return result;
}

function runWorker(runtime, targetKiB) {
  const result = spawnSync(
    process.execPath,
    [__filename, '--worker', runtime, String(targetKiB)],
    {
      cwd: packageRoot,
      encoding: 'utf8',
      maxBuffer: 1024 * 1024,
      timeout: 2_000,
    },
  );
  if (result.error || result.status !== 0) {
    throw new Error(
      `${runtime} ${targetKiB ?? 'real corpus'} worker failed: ${result.error ?? ''}\n${result.stdout}${result.stderr}`,
    );
  }
  return JSON.parse(result.stdout);
}

function collectRealCorpusObservations() {
  const observations = {};
  for (const runtime of ['native-real', 'wasm-real']) {
    observations[runtime.slice(0, -'-real'.length)] = aggregateObservations(
      Array.from({ length: TRIALS }, () => runWorker(runtime)),
    );
  }
  return observations;
}

function collectRuntimeObservations() {
  return metrics.observed.syntheticDoubling.lanes.map((lane) => {
    const observations = { targetKiB: lane.targetKiB };
    for (const runtime of ['native', 'wasm']) {
      observations[runtime] = aggregateObservations(
        Array.from({ length: TRIALS }, () => runWorker(runtime, lane.targetKiB)),
      );
    }
    return observations;
  });
}

function hasTwoConsecutiveThreefoldIncreases(values) {
  const increases = values.slice(1).map((value, index) => value >= values[index] * 3);
  return increases.some((increase, index) => increase && increases[index + 1]);
}

function observedUpperBound(observed, multiplier, floor, hardLimit) {
  return Math.min(hardLimit, Math.max(observed * multiplier, floor));
}

function validateRuntimeObservations(observations) {
  const ratchet = metrics.ratchet;
  const recordedLanes = metrics.observed.syntheticDoubling.lanes;
  const series = {
    nativeParse: [],
    nativeQuery: [],
    nativeRss: [],
    wasmParse: [],
    wasmQuery: [],
    wasmRss: [],
    wasmPages: [],
  };

  for (const [index, observation] of observations.entries()) {
    const recorded = recordedLanes[index];
    assert.equal(observation.targetKiB, recorded.targetKiB);
    for (const runtime of ['native', 'wasm']) {
      const actual = observation[runtime];
      const baseline = recorded[`${runtime}Runtime`];
      assert.ok(baseline, `${runtime} ${recorded.targetKiB} KiB baseline is missing`);
      assert.equal(actual.sourceBytes, recorded.sourceBytes);

      const parseLimit = observedUpperBound(
        baseline.observedParseMilliseconds,
        TIME_BASELINE_MULTIPLIER,
        TIME_NOISE_FLOOR_MILLISECONDS,
        runtime === 'native'
          ? ratchet.nativeSmokeParseHardLimitMilliseconds
          : ratchet.wasmSmokeParseHardLimitMilliseconds,
      );
      const queryLimit = observedUpperBound(
        baseline.observedQueryMilliseconds,
        TIME_BASELINE_MULTIPLIER,
        TIME_NOISE_FLOOR_MILLISECONDS,
        ratchet.queryHardLimitMilliseconds,
      );
      const compileLimit = observedUpperBound(
        baseline.observedQueryCompileMilliseconds,
        TIME_BASELINE_MULTIPLIER,
        TIME_NOISE_FLOOR_MILLISECONDS,
        ratchet.queryHardLimitMilliseconds,
      );
      const rssHardLimit = runtime === 'native'
        ? ratchet.nativePeakRssInvestigateAboveBytes
        : ratchet.wasmPeakRssInvestigateAboveBytes;
      const rssLimit = observedUpperBound(
        baseline.observedMaximumResidentSetBytes,
        RSS_BASELINE_MULTIPLIER,
        baseline.observedMaximumResidentSetBytes + RSS_NOISE_FLOOR_BYTES,
        rssHardLimit,
      );
      assert.ok(actual.parseMilliseconds <= parseLimit, `${runtime} parse exceeded ${parseLimit} ms`);
      assert.ok(actual.queryMilliseconds <= queryLimit, `${runtime} query exceeded ${queryLimit} ms`);
      assert.ok(
        actual.queryCompileMilliseconds <= compileLimit,
        `${runtime} query compile exceeded ${compileLimit} ms`,
      );
      assert.ok(actual.maximumResidentSetBytes <= rssLimit, `${runtime} RSS exceeded ${rssLimit}`);

      series[`${runtime}Parse`].push(actual.parseMilliseconds);
      series[`${runtime}Query`].push(actual.queryMilliseconds);
      series[`${runtime}Rss`].push(actual.maximumResidentSetBytes);
      if (runtime === 'wasm') {
        const pageLimit = Math.min(
          baseline.maxMemoryPages,
          baseline.observedMemoryPages + WASM_PAGE_NOISE_FLOOR,
        );
        assert.ok(actual.memoryPages <= pageLimit, `WASM memory exceeded ${pageLimit} pages`);
        series.wasmPages.push(actual.memoryPages);
      }
    }
  }

  for (const [name, values] of Object.entries(series)) {
    assert.equal(
      hasTwoConsecutiveThreefoldIncreases(values),
      false,
      `${name} has two consecutive at-least-threefold increases: ${values.join(', ')}`,
    );
  }
}

function validateRealCorpusObservations(observations) {
  const ratchet = metrics.ratchet;
  const recorded = metrics.observed.realCorpus;
  for (const runtime of ['native', 'wasm']) {
    const actual = observations[runtime];
    assert.equal(actual.fixtureCount, recorded.fixtureCount);
    assert.equal(actual.sourceBytes, recorded.sourceBytes);
    const parseBaseline = runtime === 'native'
      ? metrics.observed.nativeNodeSmokeParseMilliseconds
      : metrics.observed.wasmNodeSmokeParseMilliseconds;
    const parseHardLimit = runtime === 'native'
      ? ratchet.nativeSmokeParseHardLimitMilliseconds
      : ratchet.wasmSmokeParseHardLimitMilliseconds;
    const rssBaseline = runtime === 'native'
      ? metrics.observed.nativeNodeSmokeMaximumResidentSetBytes
      : metrics.observed.wasmNodeSmokeMaximumResidentSetBytes;
    const rssHardLimit = runtime === 'native'
      ? ratchet.nativePeakRssInvestigateAboveBytes
      : ratchet.wasmPeakRssInvestigateAboveBytes;
    const queryBaseline = runtime === 'native'
      ? metrics.observed.queryTime.nativeExecutionMilliseconds
      : metrics.observed.queryTime.wasmExecutionMilliseconds;
    const compileBaseline = runtime === 'native'
      ? metrics.observed.queryTime.nativeCompileMilliseconds
      : metrics.observed.queryTime.wasmCompileMilliseconds;
    assert.ok(
      actual.parseMilliseconds <= observedUpperBound(
        parseBaseline,
        TIME_BASELINE_MULTIPLIER,
        TIME_NOISE_FLOOR_MILLISECONDS,
        parseHardLimit,
      ),
      `${runtime} real-corpus parse exceeded its ratchet`,
    );
    assert.ok(
      actual.queryMilliseconds <= observedUpperBound(
        queryBaseline,
        TIME_BASELINE_MULTIPLIER,
        TIME_NOISE_FLOOR_MILLISECONDS,
        ratchet.queryHardLimitMilliseconds,
      ),
      `${runtime} real-corpus query exceeded its ratchet`,
    );
    assert.ok(
      actual.queryCompileMilliseconds <= observedUpperBound(
        compileBaseline,
        TIME_BASELINE_MULTIPLIER,
        TIME_NOISE_FLOOR_MILLISECONDS,
        ratchet.queryHardLimitMilliseconds,
      ),
      `${runtime} real-corpus query compile exceeded its ratchet`,
    );
    assert.ok(
      actual.maximumResidentSetBytes <= observedUpperBound(
        rssBaseline,
        RSS_BASELINE_MULTIPLIER,
        rssBaseline + RSS_NOISE_FLOOR_BYTES,
        rssHardLimit,
      ),
      `${runtime} real-corpus RSS exceeded its ratchet`,
    );
    if (runtime === 'wasm') {
      assert.ok(
        actual.memoryPages <= metrics.observed.wasmRuntimeMemoryPages.maxPeakPages,
        'WASM real-corpus memory exceeded its ratchet',
      );
    }
  }
}

function runCommand(command, arguments_, cwd, environment = process.env) {
  const started = performance.now();
  const result = spawnSync(command, arguments_, {
    cwd,
    env: environment,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
    timeout: metrics.ratchet.independentCompileHardLimitMilliseconds,
  });
  if (result.error || result.status !== 0) {
    throw new Error(
      `${command} ${arguments_.join(' ')} failed: ${result.error ?? ''}\n${result.stdout}${result.stderr}`,
    );
  }
  return elapsedMilliseconds(started);
}

function validateBuildMetrics() {
  const build = metrics.observed.build;
  const hardLimit = metrics.ratchet.independentCompileHardLimitMilliseconds;
  const generation = runCommand(
    process.execPath,
    ['scripts/run_python.js', 'scripts/generate.py'],
    packageRoot,
  );
  const targetDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'tree-sitter-mermaid-target-'));
  let rustCompile;
  try {
    rustCompile = runCommand(
      process.env.CARGO || 'cargo',
      ['check', '--locked', '--release', '-p', 'tree-sitter-mermaid'],
      workspaceRoot,
      { ...process.env, CARGO_TARGET_DIR: targetDirectory },
    );
  } finally {
    fs.rmSync(targetDirectory, { recursive: true, force: true });
  }
  const nodeCompile = runCommand(
    process.execPath,
    [path.join('node_modules', 'node-gyp', 'bin', 'node-gyp.js'), 'rebuild'],
    packageRoot,
  );

  for (const [name, actual, recorded] of [
    ['generation', generation, build.twoRuntimeTwoWasmGenerationWallMilliseconds],
    ['Rust release compile', rustCompile, build.rustReleaseCompileWallMilliseconds],
    ['Node binding compile', nodeCompile, build.nodeBindingCompileWallMilliseconds],
  ]) {
    const limit = observedUpperBound(recorded, TIME_BASELINE_MULTIPLIER, 60_000, hardLimit);
    assert.ok(actual <= limit, `${name} took ${actual} ms; limit is ${limit} ms`);
  }
  return { generation, rustCompile, nodeCompile };
}

async function main() {
  if (process.argv[2] === '--worker') {
    const runtime = process.argv[3];
    const targetKiB = Number(process.argv[4]);
    const observation = runtime === 'native'
      ? nativeObservation(targetKiB)
      : runtime === 'wasm'
        ? await wasmObservation(targetKiB)
        : runtime === 'native-real'
          ? nativeRealCorpusObservation()
          : await wasmRealCorpusObservation();
    process.stdout.write(`${JSON.stringify(observation)}\n`);
    return;
  }

  const realCorpus = collectRealCorpusObservations();
  const observations = collectRuntimeObservations();
  if (process.argv.includes('--observe')) {
    process.stdout.write(`${JSON.stringify({ realCorpus, doubling: observations }, null, 2)}\n`);
    return;
  }
  validateRealCorpusObservations(realCorpus);
  validateRuntimeObservations(observations);
  const build = process.argv.includes('--runtime-only') ? null : validateBuildMetrics();
  process.stdout.write(`${JSON.stringify({ realCorpus, doubling: observations, build }, null, 2)}\n`);
}

main().catch((error) => {
  console.error(error.stack || error);
  process.exitCode = 1;
});
