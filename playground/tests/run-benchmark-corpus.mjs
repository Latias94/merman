import { access, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { chromium } from "playwright";
import { preview } from "vite";

import {
  BenchmarkPageOperationError,
  cancelBenchmarkPage,
  isBenchmarkPageOperationError,
  runBenchmarkPageOperation,
  runBenchmarkStartupOperation,
} from "../scripts/benchmark-page-guards.mjs";
import { BENCHMARK_REPORT_SCHEMA_VERSION } from "../src/benchmark/report-schema.ts";
import {
  assembleBenchmarkCorpusAggregate,
  createBenchmarkCorpusFailureEnvelope,
} from "../src/benchmark/corpus-evidence.ts";

const testsRoot = import.meta.dirname;
const playgroundRoot = path.resolve(testsRoot, "..");
const workspaceRoot = path.resolve(playgroundRoot, "..");
const targetBenchRoot = path.join(workspaceRoot, "target", "bench");

const options = parseArguments(process.argv.slice(2));
const output = resolveOutput(options.output);
await rejectExistingOutput(output);
let page = null;
let browser = null;
let server = null;
let interruptReason = null;

const interrupt = (signal) => {
  if (interruptReason) return;
  interruptReason = signal.toLowerCase();
  if (browser) void cancelBenchmarkPage(page, browser, interruptReason);
};
process.once("SIGINT", () => interrupt("SIGINT"));
process.once("SIGTERM", () => interrupt("SIGTERM"));

const deadlineMs = Date.now() + options.timeoutSeconds * 1000;
try {
  server = await runBenchmarkStartupOperation({
    deadlineMs,
    operation: () =>
      preview({
        root: playgroundRoot,
        logLevel: "error",
        preview: {
          host: "127.0.0.1",
          port: options.port,
          strictPort: false,
        },
      }),
  });
  const startedAtWallMs = Date.now();
  const startedAtMs = performance.now();
  const baseUrl = server.resolvedUrls?.local[0];
  if (!baseUrl) throw new Error("Vite preview did not publish a local URL.");
  const fixtureIds = options.full ? undefined : options.fixtureIds;
  const discovery = await withBenchmarkPage(
    baseUrl,
    deadlineMs,
    async (activePage) => {
      const ready = await activePage.evaluate(() =>
        window.__MERMAN_BENCHMARK_CORPUS__.ready()
      );
      const plan = await activePage.evaluate(
        (request) => window.__MERMAN_BENCHMARK_CORPUS__.plan(request),
        {
          fixtureIds,
          iterations: options.iterations,
          masterSeed: options.masterSeed,
          warmups: options.warmups,
        }
      );
      return { plan, ready };
    }
  );
  console.log(
    [
      "[merman-benchmark] browser corpus started.",
      `  Families: ${discovery.plan.length}`,
      `  Merman: ${discovery.ready.versions.merman}`,
      `  Mermaid.js: ${discovery.ready.versions.mermaid}`,
      `  Seed: ${options.masterSeed}`,
      `  Timeout: ${options.timeoutSeconds}s`,
      "  Isolation: fresh browser process per fixture",
    ].join("\n")
  );
  const catalogById = new Map(
    discovery.ready.catalog.fixtures.map((fixture) => [fixture.id, fixture])
  );
  const fixtureRuns = [];
  for (const [index, planned] of discovery.plan.entries()) {
    const fixtureStartedAtMs = performance.now();
    const fixtureStartedAtWallMs = Date.now();
    let envelope;
    try {
      envelope = await withBenchmarkPage(
        baseUrl,
        deadlineMs,
        (activePage) =>
          activePage.evaluate(
            ({ request, pendingInterrupt }) => {
              const corpus = window.__MERMAN_BENCHMARK_CORPUS__;
              const running = corpus.run(request);
              if (pendingInterrupt) corpus.cancel(pendingInterrupt);
              return running;
            },
            {
              request: {
                fixtureId: planned.fixtureId,
                coldSeed: planned.coldSeed,
                warmSeed: planned.warmSeed,
                iterations: options.iterations,
                warmups: options.warmups,
              },
              pendingInterrupt: interruptReason,
            }
          )
      );
    } catch (error) {
      envelope = createCliFailureEnvelope({
        catalog: discovery.ready.catalog,
        fixture: requireCatalogFixture(catalogById, planned.fixtureId),
        planned,
        options,
        error,
        attempted: true,
        versions: discovery.ready.versions,
        startedAtMs: fixtureStartedAtMs,
        startedAtWallMs: fixtureStartedAtWallMs,
        forcedFailure: interruptReason
          ? {
              stage: "browser-interrupted",
              terminalStatus: "cancelled",
              message: `Browser corpus interrupted: ${interruptReason}.`,
              detail: null,
            }
          : undefined,
      });
      console.log(
        `[merman-benchmark] ${index + 1}/${discovery.plan.length} ${planned.fixtureId}: ${envelope.terminalStatus} (${envelope.fixture.failure?.stage})`
      );
      fixtureRuns.push({ envelope, planned });
      if (interruptReason || Date.now() >= deadlineMs) break;
      continue;
    }
    fixtureRuns.push({ envelope, planned });
    console.log(
      `[merman-benchmark] ${index + 1}/${discovery.plan.length} ${planned.fixtureId}: ${envelope.terminalStatus}`
    );
    if (
      envelope.terminalStatus === "cancelled" ||
      envelope.terminalStatus === "invalidated"
    ) {
      break;
    }
  }
  const tailTerminalStatus =
    fixtureRuns.at(-1)?.envelope.terminalStatus === "invalidated"
      ? "invalidated"
      : "cancelled";
  const tailReason =
    Date.now() >= deadlineMs
      ? "cli-timeout"
      : tailTerminalStatus === "invalidated"
        ? "batch-invalidated"
        : interruptReason ?? "batch-cancelled";
  appendUnfinishedFixtureFailures({
    fixtureRuns,
    plan: discovery.plan,
    catalog: discovery.ready.catalog,
    catalogById,
    versions: discovery.ready.versions,
    options,
    reason: tailReason,
    terminalStatus: tailTerminalStatus,
  });
  const envelope = assembleBenchmarkCorpusAggregate({
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    catalog: discovery.ready.catalog,
    fixtureEnvelopes: fixtureRuns.map(({ envelope: fixtureEnvelope }) =>
      fixtureEnvelope
    ),
    plan: discovery.plan.map(({ fixtureId, coldSeed, warmSeed }) => ({
      fixtureId,
      coldSeed,
      warmSeed,
    })),
    run: {
      id: `corpus-batch-${startedAtWallMs}-${options.masterSeed.toString(16)}`,
      masterSeed: options.masterSeed,
      iterations: options.iterations,
      warmups: options.warmups,
      startedAt: new Date(startedAtWallMs).toISOString(),
      endedAt: new Date(Date.now()).toISOString(),
      durationMs: Math.max(0, performance.now() - startedAtMs),
    },
    versions: discovery.ready.versions,
  });
  await mkdir(path.dirname(output), { recursive: true });
  await writeFile(output, `${JSON.stringify(envelope, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
  });
  console.log(
    [
      "[merman-benchmark] browser corpus complete.",
      `  Status: ${envelope.terminalStatus}`,
      `  Success / failure: ${envelope.coverage.succeededFamilies} / ${envelope.coverage.failedFamilies}`,
      `  Output: ${path.relative(workspaceRoot, output)}`,
    ].join("\n")
  );
  process.exitCode =
    interruptReason || envelope.terminalStatus === "cancelled"
      ? 130
      : envelope.terminalStatus === "success"
        ? 0
        : 1;
} catch (error) {
  if (!interruptReason) throw error;
  console.error(`[merman-benchmark] interrupted: ${interruptReason}`);
  process.exitCode = 130;
} finally {
  await server?.close();
}

async function withBenchmarkPage(baseUrl, deadlineMs, operation) {
  const activeBrowser = await runBenchmarkStartupOperation({
    deadlineMs,
    operation: () =>
      chromium.launch({
        headless: !options.headed,
        timeout: Math.max(1, deadlineMs - Date.now()),
      }),
  });
  let activePage = null;
  try {
    browser = activeBrowser;
    if (interruptReason) {
      throw new BenchmarkPageOperationError(
        "browser-interrupted",
        `Browser corpus interrupted: ${interruptReason}`
      );
    }
    activePage = await runBenchmarkStartupOperation({
      deadlineMs,
      onTimeout: () => activeBrowser.close(),
      operation: async () => {
        const context = await activeBrowser.newContext({
          locale: "en-US",
          viewport: { width: 800, height: 600 },
        });
        return context.newPage();
      },
    });
    page = activePage;
    return await runBenchmarkPageOperation({
      browser: activeBrowser,
      deadlineMs,
      operation: async () => {
        const pageErrors = [];
        activePage.on("pageerror", (error) => pageErrors.push(error.message));
        await activePage.goto(new URL("benchmark-corpus.html", baseUrl).href, {
          waitUntil: "domcontentloaded",
        });
        await activePage.waitForFunction(
          () => typeof window.__MERMAN_BENCHMARK_CORPUS__?.run === "function"
        );
        const result = await operation(activePage, activeBrowser);
        if (pageErrors.length > 0) {
          throw new BenchmarkPageOperationError(
            "browser-page-error",
            `Browser page errors: ${pageErrors.join(" | ")}`
          );
        }
        return result;
      },
      page: activePage,
    });
  } finally {
    if (page === activePage) page = null;
    if (browser === activeBrowser) browser = null;
    await activeBrowser.close();
  }
}

function parseArguments(args) {
  const parsed = {
    fixtureIds: null,
    full: false,
    headed: false,
    iterations: 6,
    masterSeed: 0x5eed1234,
    output: "target/bench/browser-family-corpus.json",
    port: 4180,
    timeoutSeconds: 45 * 60,
    warmups: 2,
  };
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    switch (argument) {
      case "--fixtures":
        parsed.fixtureIds = requireValue(args, ++index, argument)
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean);
        break;
      case "--full":
        parsed.full = true;
        break;
      case "--headed":
        parsed.headed = true;
        break;
      case "--iterations":
        parsed.iterations = parseInteger(requireValue(args, ++index, argument), argument);
        break;
      case "--seed":
        parsed.masterSeed = parseInteger(requireValue(args, ++index, argument), argument);
        break;
      case "--warmups":
        parsed.warmups = parseInteger(requireValue(args, ++index, argument), argument);
        break;
      case "--out":
        parsed.output = requireValue(args, ++index, argument);
        break;
      case "--port":
        parsed.port = parseInteger(requireValue(args, ++index, argument), argument);
        break;
      case "--timeout-seconds":
        parsed.timeoutSeconds = parseInteger(
          requireValue(args, ++index, argument),
          argument
        );
        break;
      case "--help":
      case "-h":
        printHelp();
        process.exit(0);
        break;
      default:
        throw new Error(`Unknown browser corpus argument: ${argument}`);
    }
  }
  if (parsed.full === (parsed.fixtureIds !== null)) {
    throw new Error("Select exactly one of --full or --fixtures <id,id>.");
  }
  if (parsed.fixtureIds?.length === 0) {
    throw new Error("--fixtures must contain at least one fixture id.");
  }
  if (parsed.timeoutSeconds <= 0) {
    throw new Error("--timeout-seconds must be positive.");
  }
  return Object.freeze(parsed);
}

async function rejectExistingOutput(file) {
  try {
    await access(file);
  } catch (error) {
    if (error && typeof error === "object" && error.code === "ENOENT") return;
    throw error;
  }
  throw new Error(`Browser corpus output already exists: ${file}`);
}

function createCliFailureEnvelope({
  catalog,
  fixture,
  planned,
  options: runOptions,
  error,
  attempted,
  startedAtMs,
  startedAtWallMs,
  versions,
  forcedFailure,
}) {
  const classified = forcedFailure ?? classifyCliFailure(error);
  const endedAtMs = performance.now();
  const endedAtWallMs = Date.now();
  return createBenchmarkCorpusFailureEnvelope({
    attempted,
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    catalog: catalog.identity,
    fixture,
    run: {
      id: `corpus-fixture-${planned.fixtureId}-${startedAtWallMs}-${planned.coldSeed.toString(16)}`,
      coldSeed: planned.coldSeed,
      warmSeed: planned.warmSeed,
      iterations: runOptions.iterations,
      warmups: runOptions.warmups,
      startedAt: new Date(startedAtWallMs).toISOString(),
      endedAt: new Date(endedAtWallMs).toISOString(),
      durationMs: Math.max(0, endedAtMs - startedAtMs),
    },
    versions,
    terminalStatus: classified.terminalStatus,
    skipReason: classified.stage,
    failure: {
      stage: classified.stage,
      message: classified.message,
      detail: classified.detail,
    },
  });
}

function appendUnfinishedFixtureFailures({
  fixtureRuns,
  plan,
  catalog,
  catalogById,
  versions,
  options: runOptions,
  reason,
  terminalStatus,
}) {
  for (let index = fixtureRuns.length; index < plan.length; index += 1) {
    const planned = plan[index];
    fixtureRuns.push({
      planned,
      envelope: createCliFailureEnvelope({
        catalog,
        fixture: requireCatalogFixture(catalogById, planned.fixtureId),
        planned,
        options: runOptions,
        attempted: false,
        versions,
        startedAtMs: performance.now(),
        startedAtWallMs: Date.now(),
        forcedFailure: {
          stage:
            reason === "cli-timeout"
              ? "cli-timeout"
              : reason === "batch-invalidated"
                ? "batch-invalidated"
                : "batch-cancelled",
          terminalStatus,
          message: `Benchmark fixture was not started: ${reason}.`,
          detail: null,
        },
      }),
    });
  }
}

function requireCatalogFixture(catalogById, fixtureId) {
  const fixture = catalogById.get(fixtureId);
  if (!fixture) throw new Error(`Missing catalog fixture ${fixtureId}.`);
  return fixture;
}

function classifyCliFailure(error) {
  const message = error instanceof Error ? error.message : String(error);
  const detail = error instanceof Error ? error.stack ?? null : null;
  const stage = isBenchmarkPageOperationError(error)
    ? error.code
    : "browser-page-error";
  return {
    stage,
    terminalStatus:
      stage === "cli-timeout" || stage === "browser-interrupted"
        ? "cancelled"
        : "complete-with-errors",
    message,
    detail,
  };
}

function resolveOutput(value) {
  const resolved = path.resolve(workspaceRoot, value);
  if (
    resolved !== targetBenchRoot &&
    !resolved.startsWith(`${targetBenchRoot}${path.sep}`)
  ) {
    throw new Error("Browser corpus output must remain under target/bench.");
  }
  return resolved;
}

function requireValue(args, index, option) {
  const value = args[index];
  if (!value || value.startsWith("--")) {
    throw new Error(`${option} requires a value.`);
  }
  return value;
}

function parseInteger(value, option) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`${option} must be an integer.`);
  }
  return parsed;
}

function printHelp() {
  console.log(`Usage:
  node run-benchmark-corpus.mjs --full [options]
  node run-benchmark-corpus.mjs --fixtures id,id [options]

Options:
  --iterations <even>  Measured AB/BA blocks per mode (default: 6)
  --warmups <count>    Warm samples per engine (default: 2)
  --seed <uint32>      Master corpus and schedule seed
  --out <path>         JSON output under target/bench
  --port <number>      Preferred preview port (default: 4180)
  --timeout-seconds <n>  Whole-corpus timeout (default: 2700)
  --headed             Show Chromium`);
}
