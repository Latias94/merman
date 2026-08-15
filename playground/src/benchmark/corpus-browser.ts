import { DEFAULT_MERMAID_CONFIG } from "../lib/mermaid-config.ts";
import {
  MERMAID_JS_VERSION,
  mermaidExternalRequirementsFor,
} from "../runtime/mermaid-requirements.ts";
import { CANONICAL_RENDER_VIEWPORT } from "../runtime/render-viewport.ts";
import { createBrowserBenchmarkRuntime } from "./browser.ts";
import {
  BENCHMARK_CORPUS_MERMAN_VERSION,
  createBenchmarkCorpusCatalog,
  createBenchmarkCorpusPlan,
  createBenchmarkCorpusOrchestrator,
  sha256Hex,
  validateBenchmarkCorpusRunBudget,
  type BenchmarkCorpusCatalog,
  type BenchmarkCorpusFixtureEnvelope,
  type BenchmarkCorpusFixtureRunRequest,
} from "./corpus.ts";

export interface BrowserBenchmarkCorpusReady {
  readonly catalog: BenchmarkCorpusCatalog;
  readonly versions: BenchmarkCorpusFixtureEnvelope["versions"];
}

export interface BrowserBenchmarkCorpusPlanEntry {
  readonly coldSeed: number;
  readonly family: string;
  readonly fixtureId: string;
  readonly order: number;
  readonly warmSeed: number;
}

export interface BrowserBenchmarkCorpusPlanRequest {
  readonly fixtureIds?: readonly string[];
  readonly iterations: number;
  readonly masterSeed: number;
  readonly warmups: number;
}

export interface BrowserBenchmarkCorpusApi {
  cancel(reason?: string): void;
  plan(
    request: BrowserBenchmarkCorpusPlanRequest
  ): readonly BrowserBenchmarkCorpusPlanEntry[];
  ready(): Promise<BrowserBenchmarkCorpusReady>;
  run(
    request: Omit<BenchmarkCorpusFixtureRunRequest, "signal">
  ): Promise<BenchmarkCorpusFixtureEnvelope>;
}

let browserRunAbort: AbortController | null = null;
const {
  controller: benchmarkController,
  lifecycle: benchmarkDocumentLifecycle,
} = createBrowserBenchmarkRuntime();

const versions = Object.freeze({
  merman: BENCHMARK_CORPUS_MERMAN_VERSION,
  mermaid: MERMAID_JS_VERSION,
});
let catalog: Promise<BenchmarkCorpusCatalog> | null = null;
const orchestrator = createBenchmarkCorpusOrchestrator({
  controller: benchmarkController,
  prepareFixture(fixture) {
    return {
      payload: {
        source: fixture.source,
        configJson: DEFAULT_MERMAID_CONFIG,
        theme: "default",
        diagramFont: "trebuchet",
        externalRequirements: mermaidExternalRequirementsFor(fixture.detection),
        viewport: CANONICAL_RENDER_VIEWPORT,
      },
      detection: fixture.detection,
    };
  },
  dateNow: Date.now,
  digest: sha256Hex,
  now: () => performance.now(),
  versions,
});

export const browserBenchmarkCorpusApi: BrowserBenchmarkCorpusApi =
  Object.freeze({
    cancel(reason = "user") {
      browserRunAbort?.abort(reason);
    },
    plan(request: BrowserBenchmarkCorpusPlanRequest) {
      const plan = createBenchmarkCorpusPlan(request);
      validateBenchmarkCorpusRunBudget(request, plan.length);
      return Object.freeze(
        plan.map((entry) =>
          Object.freeze({
            family: entry.fixture.family,
            fixtureId: entry.fixture.id,
            order: entry.fixture.order,
            coldSeed: entry.coldSeed,
            warmSeed: entry.warmSeed,
          })
        )
      );
    },
    async ready() {
      return Object.freeze({
        catalog: await loadCatalog(),
        versions,
      });
    },
    async run(request: Omit<BenchmarkCorpusFixtureRunRequest, "signal">) {
      if (browserRunAbort) {
        throw new Error("A browser benchmark corpus run is already active.");
      }
      const abort = new AbortController();
      browserRunAbort = abort;
      try {
        return await orchestrator.run({ ...request, signal: abort.signal });
      } finally {
        if (browserRunAbort === abort) browserRunAbort = null;
      }
    },
  });

window.__MERMAN_BENCHMARK_CORPUS__ = browserBenchmarkCorpusApi;

const cleanupLifecycle = benchmarkDocumentLifecycle.subscribe((signal) => {
  if (signal.kind !== "pagehide" || signal.persisted) return;
  queueMicrotask(() => {
    benchmarkController.dispose();
  });
});

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    cleanupLifecycle();
    benchmarkController.dispose();
  });
}

function loadCatalog(): Promise<BenchmarkCorpusCatalog> {
  catalog ??= createBenchmarkCorpusCatalog(sha256Hex);
  return catalog;
}

declare global {
  interface Window {
    __MERMAN_BENCHMARK_CORPUS__?: BrowserBenchmarkCorpusApi;
  }
}
