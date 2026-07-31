import { DEFAULT_MERMAID_CONFIG } from "../lib/mermaid-config.ts";
import {
  MERMAID_JS_VERSION,
  mermaidExternalRequirementsFor,
} from "../runtime/mermaid-requirements.ts";
import {
  disposeRenderCoordinator,
} from "../runtime/render-coordinator-browser.ts";
import { PLAYGROUND_RENDER_VIEWPORT } from "../runtime/render-viewport.ts";
import { benchmarkController } from "./browser.ts";
import {
  FAMILY_BASELINE_CORPUS,
  BENCHMARK_CORPUS_MERMAN_VERSION,
  createBenchmarkCorpusPlan,
  createBenchmarkCorpusOrchestrator,
  sha256Hex,
  validateBenchmarkCorpusRunBudget,
  type BenchmarkCorpusEnvelope,
  type BenchmarkCorpusFixture,
  type BenchmarkCorpusRunRequest,
} from "./corpus.ts";

export interface BrowserBenchmarkCorpusReady {
  readonly availableFamilies: number;
  readonly fixtures: readonly Readonly<
    Pick<BenchmarkCorpusFixture, "family" | "id" | "order">
  >[];
  readonly versions: BenchmarkCorpusEnvelope["versions"];
}

export interface BrowserBenchmarkCorpusPlanEntry {
  readonly family: string;
  readonly fixtureId: string;
  readonly order: number;
  readonly runSeed: number;
}

export interface BrowserBenchmarkCorpusApi {
  cancel(reason?: string): void;
  plan(
    request: Omit<BenchmarkCorpusRunRequest, "signal">
  ): readonly BrowserBenchmarkCorpusPlanEntry[];
  ready(): Promise<BrowserBenchmarkCorpusReady>;
  run(
    request: Omit<BenchmarkCorpusRunRequest, "signal">
  ): Promise<BenchmarkCorpusEnvelope>;
}

let browserRunAbort: AbortController | null = null;

const versions = Object.freeze({
  merman: BENCHMARK_CORPUS_MERMAN_VERSION,
  mermaid: MERMAID_JS_VERSION,
});
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
        viewport: PLAYGROUND_RENDER_VIEWPORT,
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
    plan(
      request: Omit<BenchmarkCorpusRunRequest, "signal">
    ) {
      const plan = createBenchmarkCorpusPlan(request);
      validateBenchmarkCorpusRunBudget(request, plan.length);
      return Object.freeze(
        plan.map((entry) =>
          Object.freeze({
            family: entry.fixture.family,
            fixtureId: entry.fixture.id,
            order: entry.fixture.order,
            runSeed: entry.coldSeed,
          })
        )
      );
    },
    async ready() {
      return Object.freeze({
        availableFamilies: FAMILY_BASELINE_CORPUS.length,
        fixtures: Object.freeze(
          FAMILY_BASELINE_CORPUS.map(projectFixtureIdentity)
        ),
        versions,
      });
    },
    async run(request: Omit<BenchmarkCorpusRunRequest, "signal">) {
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

window.addEventListener(
  "pagehide",
  () => {
    browserBenchmarkCorpusApi.cancel("pagehide");
    benchmarkController.dispose();
    disposeRenderCoordinator();
  },
  { once: true }
);

function projectFixtureIdentity(fixture: BenchmarkCorpusFixture) {
  return Object.freeze({
    family: fixture.family,
    id: fixture.id,
    order: fixture.order,
  });
}

declare global {
  interface Window {
    __MERMAN_BENCHMARK_CORPUS__?: BrowserBenchmarkCorpusApi;
  }
}
