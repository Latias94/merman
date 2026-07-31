import { access, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { chromium } from "playwright";
import { preview } from "vite";

import {
  cancelBenchmarkPage,
  runBenchmarkPageOperation,
} from "../scripts/benchmark-page-guards.mjs";

const testsRoot = import.meta.dirname;
const playgroundRoot = path.resolve(testsRoot, "..");
const workspaceRoot = path.resolve(playgroundRoot, "..");
const targetBenchRoot = path.join(workspaceRoot, "target", "bench");

const options = parseArguments(process.argv.slice(2));
const output = resolveOutput(options.output);
await rejectExistingOutput(output);
let page = null;
let browser = null;
let interruptReason = null;

const interrupt = (signal) => {
  if (interruptReason) return;
  interruptReason = signal.toLowerCase();
  void cancelBenchmarkPage(page, browser, interruptReason);
};
process.once("SIGINT", () => interrupt("SIGINT"));
process.once("SIGTERM", () => interrupt("SIGTERM"));

const server = await preview({
  root: playgroundRoot,
  logLevel: "error",
  preview: {
    host: "127.0.0.1",
    port: options.port,
    strictPort: false,
  },
});
const startedAtWallMs = Date.now();
const startedAtMs = performance.now();
const deadlineMs = startedAtWallMs + options.timeoutSeconds * 1000;
try {
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
  validateSelection(fixtureIds, discovery.ready.fixtures);
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
  const fixtureRuns = [];
  for (const [index, planned] of discovery.plan.entries()) {
    const envelope = await withBenchmarkPage(
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
              fixtureIds: [planned.fixtureId],
              iterations: options.iterations,
              masterSeed: planned.runSeed,
              warmups: options.warmups,
            },
            pendingInterrupt: interruptReason,
          }
        )
    );
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
  const envelope = mergeFixtureEnvelopes({
    fixtureRuns,
    plan: discovery.plan,
    ready: discovery.ready,
    options,
    startedAtMs,
    startedAtWallMs,
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
      `  Success / failure / skip: ${envelope.coverage.succeededFamilies} / ${envelope.coverage.failedFamilies} / ${envelope.coverage.skippedFamilies}`,
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
  await server.close();
}

async function withBenchmarkPage(baseUrl, deadlineMs, operation) {
  const activeBrowser = await chromium.launch({ headless: !options.headed });
  let activePage = null;
  try {
    browser = activeBrowser;
    if (interruptReason) {
      throw new Error(`Browser corpus interrupted: ${interruptReason}`);
    }
    const context = await activeBrowser.newContext({
      locale: "en-US",
      viewport: { width: 800, height: 600 },
    });
    activePage = await context.newPage();
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
          throw new Error(`Browser page errors: ${pageErrors.join(" | ")}`);
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

function mergeFixtureEnvelopes({
  fixtureRuns,
  plan,
  ready,
  options: runOptions,
  startedAtMs,
  startedAtWallMs,
}) {
  const first = fixtureRuns[0]?.envelope;
  if (!first) {
    throw new Error("Browser corpus produced no fixture evidence.");
  }
  const fixtures = new Map(first.fixtures.map((fixture) => [fixture.id, fixture]));
  if (fixtures.size !== first.fixtures.length) {
    throw new Error("Browser corpus catalog contains duplicate fixture ids.");
  }
  for (const { envelope, planned } of fixtureRuns) {
    validateFixtureEnvelope(envelope, planned, first, ready);
    const measured = envelope.fixtures.filter(
      (fixture) => fixture.id === planned.fixtureId
    );
    if (measured.length !== 1) {
      throw new Error(
        `Fixture envelope did not contain ${planned.fixtureId} exactly once.`
      );
    }
    fixtures.set(planned.fixtureId, measured[0]);
  }

  const stopped = fixtureRuns.length < plan.length;
  if (stopped) {
    const reason = interruptReason ?? "batch-stopped";
    const completed = new Set(
      fixtureRuns.map(({ planned }) => planned.fixtureId)
    );
    for (const planned of plan) {
      if (completed.has(planned.fixtureId)) continue;
      const fixture = fixtures.get(planned.fixtureId);
      if (!fixture) {
        throw new Error(`Missing catalog fixture ${planned.fixtureId}.`);
      }
      fixtures.set(planned.fixtureId, skippedFixture(fixture, reason));
    }
  }

  const orderedFixtures = first.fixtures.map((fixture) => {
    const merged = fixtures.get(fixture.id);
    if (!merged) {
      throw new Error(`Missing merged fixture ${fixture.id}.`);
    }
    return merged;
  });
  const failures = orderedFixtures.flatMap((fixture) =>
    [fixture.cold.failure, fixture.warm.failure].filter(Boolean)
  );
  const skips = orderedFixtures
    .filter((fixture) => fixture.status === "skipped")
    .map((fixture) => ({
      family: fixture.family,
      fixtureId: fixture.id,
      reason: fixture.cold.skipReason ?? fixture.warm.skipReason ?? "not-run",
    }));
  const terminalStatuses = fixtureRuns.map(
    ({ envelope }) => envelope.terminalStatus
  );
  const terminalStatus = terminalStatuses.includes("invalidated")
    ? "invalidated"
    : terminalStatuses.includes("cancelled") || stopped
      ? "cancelled"
      : failures.length > 0
        ? "complete-with-errors"
        : "success";
  const endedAtWallMs = Date.now();

  return {
    ...first,
    execution: { fixtureIsolation: "fresh-browser-process-per-fixture" },
    run: {
      id: `corpus-batch-${startedAtWallMs}-${runOptions.masterSeed.toString(16)}`,
      masterSeed: runOptions.masterSeed,
      order: plan.map((entry) => entry.fixtureId),
      iterations: runOptions.iterations,
      warmups: runOptions.warmups,
      startedAt: new Date(startedAtWallMs).toISOString(),
      endedAt: new Date(endedAtWallMs).toISOString(),
      durationMs: Math.max(0, performance.now() - startedAtMs),
    },
    versions: ready.versions,
    terminalStatus,
    coverage: buildMergedCoverage(orderedFixtures, plan.length),
    failures,
    skips,
    fixtures: orderedFixtures,
  };
}

function validateFixtureEnvelope(envelope, planned, first, ready) {
  if (
    envelope.schemaVersion !== first.schemaVersion ||
    envelope.kind !== first.kind ||
    envelope.benchmarkReportSchemaVersion !== first.benchmarkReportSchemaVersion ||
    envelope.execution?.fixtureIsolation !== "single-page" ||
    envelope.run.masterSeed !== planned.runSeed ||
    envelope.run.order.length !== 1 ||
    envelope.run.order[0] !== planned.fixtureId ||
    envelope.coverage.selectedFamilies !== 1 ||
    JSON.stringify(envelope.catalog) !== JSON.stringify(first.catalog) ||
    JSON.stringify(envelope.versions) !== JSON.stringify(ready.versions) ||
    JSON.stringify(envelope.fixtures.map(projectFixtureEvidenceIdentity)) !==
      JSON.stringify(first.fixtures.map(projectFixtureEvidenceIdentity))
  ) {
    throw new Error(`Fixture envelope contract drifted for ${planned.fixtureId}.`);
  }
}

function projectFixtureEvidenceIdentity(fixture) {
  return {
    family: fixture.family,
    id: fixture.id,
    order: fixture.order,
    source: fixture.source,
  };
}

function skippedFixture(fixture, reason) {
  const skippedMode = (mode) => ({
    mode,
    seed: null,
    status: "skipped",
    report: null,
    failure: null,
    skipReason: reason,
  });
  return {
    ...fixture,
    status: "skipped",
    cold: skippedMode("realm-cold"),
    warm: skippedMode("warm"),
  };
}

function buildMergedCoverage(fixtures, selectedFamilies) {
  return {
    availableFamilies: fixtures.length,
    selectedFamilies,
    attemptedFamilies: fixtures.filter(
      (fixture) =>
        fixture.cold.report !== null ||
        fixture.warm.report !== null ||
        fixture.cold.failure !== null ||
        fixture.warm.failure !== null
    ).length,
    succeededFamilies: fixtures.filter((fixture) => fixture.status === "success")
      .length,
    failedFamilies: fixtures.filter((fixture) => fixture.status === "failure")
      .length,
    skippedFamilies: fixtures.filter((fixture) => fixture.status === "skipped")
      .length,
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

function validateSelection(fixtureIds, catalog) {
  if (!fixtureIds) return;
  const known = new Set(catalog.map((fixture) => fixture.id));
  const unknown = fixtureIds.filter((id) => !known.has(id));
  if (unknown.length > 0) {
    throw new Error(`Unknown family-baseline fixtures: ${unknown.join(", ")}`);
  }
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
