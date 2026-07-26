import { spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { resolveNodeTarget } from "../../src/native-loader.mjs";
import { loadCorpus } from "./corpus.mjs";
import { readBuildReceipt } from "./build-receipt.mjs";
import { withCandidateInstallation } from "./footprint.mjs";
import { computeHarnessDigest } from "./harness-inputs.mjs";
import { validateComparisonReport } from "./report-contract.mjs";
import { equivalentTransportOutcome } from "./svg-signature.mjs";
import { summarize } from "./stats.mjs";
import { digestJson } from "../stable-json.mjs";

const benchmarkRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const repositoryRoot = path.resolve(benchmarkRoot, "..", "..");
const workerPath = path.join(benchmarkRoot, "scripts", "benchmark", "worker.mjs");
const recipes = JSON.parse(readFileSync(path.join(benchmarkRoot, "candidate-builds.json"), "utf8"));
if (isMainModule()) {
  try {
    const options = parseArgs(process.argv.slice(2));
    const report = runComparison(options);
    mkdirSync(path.dirname(options.output), { recursive: true });
    writeFileSync(options.output, `${JSON.stringify(report, null, 2)}\n`);
    console.log(`[merman-node] comparison report written to ${options.output}`);
  } catch (error) {
    console.error(error instanceof Error ? error.stack ?? error.message : String(error));
    process.exitCode = 1;
  }
}

export function runComparison(options) {
  const initialHarnessDigest = computeHarnessDigest();
  assertFile(options.native, "napi candidate");
  assertFile(options.wasm, "Node-targeted WASM candidate loader");
  const buildReceipts = new Map([
    ["node-wasm", readBuildReceipt(options.wasm)],
    ["napi", readBuildReceipt(options.native)],
  ]);
  const corpus = loadCorpus(options.corpus);
  const target = resolveNodeTarget();
  const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "merman-node-benchmark-"));
  try {
    const measured = [
      measureInstalledCandidate("node-wasm", options.wasm),
      measureInstalledCandidate("napi", options.native),
    ];
    const parity = compareOutcomes(measured[0].warm.outcomes, measured[1].warm.outcomes);
    const corpusContractFailed = parity.mismatches.length > 0;
    const reportProvenance = provenance(initialHarnessDigest);
    const candidates = measured.map(({ id, artifact, cold, footprint, shutdown, warm }) => {
      const buildReceipt = buildReceipts.get(id);
      const footprintEvidence = projectFootprint(footprint);
      const queueLifecycle = {
        ...warm.queue_lifecycle,
        process_shutdown_passed: shutdown.process_shutdown_passed,
        evidence: {
          ...warm.queue_lifecycle.evidence,
          shutdown: shutdown.evidence,
        },
      };
      const targetEvidencePayload = {
        schema_version: 1,
        host: {
          platform: process.platform,
          arch: process.arch,
          libc: target.endsWith("-gnu") ? "gnu" : target.endsWith("-musl") ? "musl" : null,
          resolved_target: target,
          node: process.version,
        },
        provenance: reportProvenance,
        build_receipt: buildReceipt,
        footprint: footprintEvidence,
        queue_lifecycle: queueLifecycle,
        error_behavior: warm.error_behavior,
      };
      return {
        id,
        input_digest: corpus.digest,
        build_receipt: buildReceipt,
        corpus: {
          cases: corpus.cases.length,
          matched: parity.matched,
          mismatched: parity.mismatches.length,
          geometry_svg_mismatches: parity.geometryMismatches.length,
          raw_svg_byte_mismatches: parity.rawSvgByteMismatches,
          mismatch_paths: parity.mismatches,
          geometry_mismatch_paths: parity.geometryMismatches,
          successful: warm.outcomes.filter((outcome) => outcome.ok).length,
          failed: warm.outcomes.filter((outcome) => !outcome.ok).length,
          outcomes: warm.outcomes,
          results_digest: digestJson(warm.outcomes),
        },
        cold_process: {
          isolated_processes: true,
          samples_ms: cold.samplesMs,
          samples: cold.samples,
          summary: summarize(cold.samplesMs),
        },
        warm_latency: {
          samples_ms: warm.samples_ms,
          samples: warm.samples,
          summary: summarize(warm.samples_ms),
        },
        rss: {
          method: "process.resourceUsage.maxRSS",
          baseline_bytes: warm.baseline_rss_bytes,
          peak_bytes: warm.peak_rss_bytes,
        },
        footprint: footprintEvidence,
        queue_lifecycle: queueLifecycle,
        error_behavior: warm.error_behavior,
        concurrency: {
          workers: options.concurrency,
          requests_per_batch: options.concurrency,
          batch_samples_ms: warm.concurrency_samples_ms,
          samples: warm.concurrency_samples,
          summary: summarize(warm.concurrency_samples_ms),
        },
        target_results: [
          {
            target,
            runtime_passed: true,
            install_passed: footprint.target_install_passed,
            node: process.version,
            evidence: {
              ...targetEvidencePayload,
              digest: digestJson(targetEvidencePayload),
            },
          },
        ],
      };
    });
    const report = {
      schema_version: 1,
      provenance: reportProvenance,
      input: {
        digest: corpus.digest,
        corpus: path.relative(repositoryRoot, corpus.manifestPath).split(path.sep).join("/"),
        cases: corpus.cases.length,
        binding_options: corpus.bindingOptions,
        operation_options: corpus.operationOptions,
      },
      sampling: {
        cold_processes: options.coldSamples,
        warmup_iterations: options.warmupIterations,
        measured_iterations: options.iterations,
        concurrency_iterations: options.concurrencyIterations,
      },
      candidates,
      decision: {
        status: corpusContractFailed ? "rejected" : "inconclusive",
        selected: null,
        reasons: [
          ...(corpusContractFailed
            ? [
                `The candidates differ on ${parity.mismatches.length} semantic-model or SVG-structure outcomes; neither Node transport is admitted.`,
              ]
            : []),
          ...(parity.geometryMismatches.length > 0
            ? [
                `${parity.geometryMismatches.length} SVG outcomes have cross-target geometry drift; this is recorded separately from the Node transport contract.`,
              ]
            : []),
          "This host contributes one target result only; U14 admission requires runtime CI evidence for every shipped target.",
        ],
      },
    };
    assertHarnessUnchanged(initialHarnessDigest);
    return validateComparisonReport(report);

    function measureInstalledCandidate(id, artifact) {
      return withCandidateInstallation(
        { candidate: id, artifact, target },
        ({ footprint, productModule }) => ({
          ...measureCandidate(id, artifact, productModule),
          footprint,
        }),
      );
    }

    function measureCandidate(id, artifact, productModule) {
      const input = {
        candidate: id,
        artifact,
        productModule,
        bindingOptions: corpus.bindingOptions,
        operationOptions: corpus.operationOptions,
        cases: corpus.cases,
        iterations: options.iterations,
        warmupIterations: options.warmupIterations,
        concurrencyIterations: options.concurrencyIterations,
        concurrency: options.concurrency,
        maxQueue: options.maxQueue,
      };
      const warm = runWorker(temporaryRoot, { ...input, mode: "warm" }, `${id}-warm`);
      const shutdown = runWorker(
        temporaryRoot,
        { ...input, mode: "shutdown" },
        `${id}-shutdown`,
      );
      const samples = [];
      for (let index = 0; index < options.coldSamples; index += 1) {
        const started = performance.now();
        const sample = runWorker(
          temporaryRoot,
          { ...input, mode: "cold", cases: [corpus.cases[index % corpus.cases.length]] },
          `${id}-cold-${index}`,
        );
        samples.push({
          elapsed_ms: performance.now() - started,
          operation_ms: sample.operation_ms,
          baseline_rss_bytes: sample.baseline_rss_bytes,
          peak_rss_bytes: sample.peak_rss_bytes,
          outcome: sample.outcome,
        });
      }
      return {
        id,
        artifact,
        warm,
        shutdown,
        cold: { samples, samplesMs: samples.map((sample) => sample.elapsed_ms) },
      };
    }
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

function runWorker(temporaryRoot, input, id) {
  const inputPath = path.join(temporaryRoot, `${id}.json`);
  writeFileSync(inputPath, `${JSON.stringify(input)}\n`);
  const result = spawnSync(process.execPath, [workerPath, inputPath], {
    cwd: benchmarkRoot,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    timeout: 5 * 60 * 1000,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`benchmark worker ${id} failed: ${result.error?.message ?? result.stderr}`);
  }
  return JSON.parse(result.stdout);
}

function compareOutcomes(left, right) {
  const rightByPath = new Map(right.map((item) => [item.path, item]));
  const leftPaths = new Set(left.map((item) => item.path));
  const mismatches = [];
  const geometryMismatches = [];
  let matched = 0;
  let rawSvgByteMismatches = 0;
  for (const outcome of left) {
    const other = rightByPath.get(outcome.path);
    if (!other || !equivalentTransportOutcome(outcome, other)) {
      mismatches.push(outcome.path);
      continue;
    }
    matched += 1;
    if (outcome.ok) {
      if (outcome.svg_geometry_sha256 !== other.svg_geometry_sha256) {
        geometryMismatches.push(outcome.path);
      }
      if (outcome.sha256 !== other.sha256) rawSvgByteMismatches += 1;
    }
  }
  for (const outcome of right) {
    if (!leftPaths.has(outcome.path)) mismatches.push(outcome.path);
  }
  return {
    matched,
    mismatches: [...new Set(mismatches)].sort(),
    geometryMismatches: [...new Set(geometryMismatches)].sort(),
    rawSvgByteMismatches,
  };
}

export function projectFootprint(footprint) {
  return {
    packed_bytes: footprint.packed_bytes,
    unpacked_bytes: footprint.unpacked_bytes,
    installed_bytes: footprint.installed_bytes,
    package_count: footprint.package_count,
    runtime_api_passed: footprint.runtime_api_passed,
    runtime_catalog_passed: footprint.runtime_catalog_passed,
    generic_operation_passed: footprint.generic_operation_passed,
    svg_plan_operation_passed: footprint.svg_plan_operation_passed,
    svg_operation_passed: footprint.svg_operation_passed,
    request_options_passed: footprint.request_options_passed,
    browser_fallback_absent: footprint.browser_fallback_absent,
    optional_platform_package_passed: footprint.optional_platform_package_passed,
    install_method: footprint.install_method,
    target_install_passed: footprint.target_install_passed,
    packages: footprint.packages,
    installed_files: footprint.installed_files,
    installation_evidence: footprint.installation_evidence,
    runtime_probe: footprint.runtime_probe,
  };
}

function provenance(harnessDigest) {
  const cpu = os.cpus()[0];
  return {
    measured_at_utc: new Date().toISOString(),
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone ?? "unknown",
    harness_digest: harnessDigest,
    machine: {
      hostname: os.hostname(),
      os: os.platform(),
      release: os.release(),
      arch: os.arch(),
      cpu: cpu?.model ?? "unknown",
      logical_cpus: os.cpus().length,
      total_memory_bytes: os.totalmem(),
    },
    tools: {
      node: process.version,
      npm: runCapture("npm", ["--version"]),
      rustc: runCapture("rustc", ["--version"]),
      cargo: runCapture("cargo", ["--version"]),
      napi: recipes.candidates.napi.versions.napi,
      napi_derive: recipes.candidates.napi.versions.napi_derive,
      napi_build: recipes.candidates.napi.versions.napi_build,
      napi_cli: recipes.candidates.napi.versions.napi_cli,
    },
    commit: runCapture("git", ["rev-parse", "HEAD"]),
  };
}

export function assertHarnessUnchanged(
  initialDigest,
  currentDigest = computeHarnessDigest(),
) {
  if (initialDigest !== currentDigest) {
    throw new Error("Node benchmark harness inputs changed during measurement; rerun it.");
  }
}

function parseArgs(args) {
  const outputDefault = path.join(
    benchmarkRoot,
    "reports",
    `node-transport-comparison-${new Date().toISOString().replaceAll(":", "-")}.json`,
  );
  const options = {
    native: valueAfter(args, "--native"),
    wasm: valueAfter(args, "--wasm"),
    corpus: valueAfter(args, "--corpus") ?? path.join(benchmarkRoot, "benchmark", "corpus.json"),
    output: path.resolve(valueAfter(args, "--output") ?? outputDefault),
    coldSamples: integerAfter(args, "--cold-samples", 10),
    iterations: integerAfter(args, "--iterations", 3),
    warmupIterations: integerAfter(args, "--warmup-iterations", 1),
    concurrencyIterations: integerAfter(args, "--concurrency-iterations", 5),
    concurrency: integerAfter(args, "--concurrency", 4),
    maxQueue: integerAfter(args, "--max-queue", 64),
  };
  if (!options.native || !options.wasm) throw new Error("--native and --wasm are required.");
  options.native = path.resolve(options.native);
  options.wasm = path.resolve(options.wasm);
  return options;
}

function integerAfter(args, flag, fallback) {
  const raw = valueAfter(args, flag);
  if (raw === null) return fallback;
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value < 1) throw new Error(`${flag} must be a positive integer.`);
  return value;
}

function valueAfter(args, flag) {
  const index = args.indexOf(flag);
  return index === -1 ? null : args[index + 1] ?? null;
}

function runCapture(command, args) {
  const result = spawnSync(command, args, { cwd: repositoryRoot, encoding: "utf8" });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} failed: ${result.error?.message ?? result.stderr}`);
  }
  return result.stdout.trim();
}

function assertFile(file, label) {
  if (!existsSync(file) || !statSync(file).isFile()) throw new Error(`Missing ${label}: ${file}.`);
}

function isMainModule() {
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
