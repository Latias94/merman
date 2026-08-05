import { expect, test } from "@playwright/test";

import { BENCHMARK_CORPUS_SCHEMA_VERSION } from "../src/benchmark/corpus-schema";
import { BENCHMARK_REPORT_SCHEMA_VERSION } from "../src/benchmark/report-schema";

const FIXTURE_ID = "basic-flowchart" as const;

test("non-UI corpus page returns exactly one cold and warm fixture envelope", async ({ page }) => {
  test.setTimeout(240_000);

  await page.goto("./benchmark-corpus.html");
  await page.waitForFunction(
    () => typeof window.__MERMAN_BENCHMARK_CORPUS__?.run === "function"
  );
  const ready = await page.evaluate(() =>
    window.__MERMAN_BENCHMARK_CORPUS__!.ready()
  );
  expect(ready.catalog.identity.availableFamilies).toBe(35);
  expect(ready.catalog.fixtures).toHaveLength(35);
  const discoveryRuntimeResources = await page.evaluate(() =>
    performance
      .getEntriesByType("resource")
      .map((entry) => entry.name)
      .filter((name) => /merman_wasm(?:_bg)?-/u.test(name))
  );
  expect(discoveryRuntimeResources).toEqual([]);

  const planned = await page.evaluate((fixtureId) => {
    const corpus = window.__MERMAN_BENCHMARK_CORPUS__!;
    return corpus.plan({
      fixtureIds: [fixtureId],
      iterations: 2,
      masterSeed: 0x5eed1234,
      warmups: 0,
    })[0]!;
  }, FIXTURE_ID);
  const envelope = await page.evaluate((request) =>
    window.__MERMAN_BENCHMARK_CORPUS__!.run(request), {
      fixtureId: planned.fixtureId,
      coldSeed: planned.coldSeed,
      warmSeed: planned.warmSeed,
      iterations: 2,
      warmups: 0,
    }
  );

  expect(envelope).toMatchObject({
    schemaVersion: BENCHMARK_CORPUS_SCHEMA_VERSION,
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    terminalStatus: "success",
    fixtureId: FIXTURE_ID,
    fixture: { id: FIXTURE_ID, status: "success" },
  });
  expect("fixtures" in envelope).toBe(false);
  expect(envelope.fixture.source.sha256).toMatch(/^[0-9a-f]{64}$/u);
  expect(envelope.fixture.cold.report).toMatchObject({
    schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    terminalStatus: "success",
    plan: { mode: "realm-cold", seed: planned.coldSeed },
  });
  expect(envelope.fixture.warm.report).toMatchObject({
    schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    terminalStatus: "success",
    plan: { mode: "warm", seed: planned.warmSeed, warmups: 0 },
  });
  expect(envelope.fixture.cold.report!.input.source).toBe(
    envelope.fixture.warm.report!.input.source
  );
  expect(envelope.fixture.cold.report!.run.id).not.toBe(
    envelope.fixture.warm.report!.run.id
  );
  await expect(page.locator('iframe[data-merman-realm="benchmark"]')).toHaveCount(0);
});

declare global {
  interface Window {
    __MERMAN_BENCHMARK_CORPUS__?: {
      cancel(reason?: string): void;
      plan(request: {
        fixtureIds?: readonly string[];
        iterations: number;
        masterSeed: number;
        warmups: number;
      }): readonly {
        coldSeed: number;
        fixtureId: string;
        warmSeed: number;
      }[];
      ready(): Promise<{
        catalog: {
          fixtures: readonly unknown[];
          identity: { availableFamilies: number };
        };
      }>;
      run(request: {
        coldSeed: number;
        fixtureId: string;
        iterations: number;
        warmSeed: number;
        warmups: number;
      }): Promise<SmokeCorpusEnvelope>;
    };
  }
}

interface SmokeCorpusEnvelope {
  readonly benchmarkReportSchemaVersion: number;
  readonly fixture: SmokeFixtureEvidence;
  readonly fixtureId: string;
  readonly schemaVersion: number;
  readonly terminalStatus: string;
}

interface SmokeFixtureEvidence {
  readonly cold: { readonly report: SmokeBenchmarkReport | null };
  readonly id: string;
  readonly source: { readonly sha256: string };
  readonly status: string;
  readonly warm: { readonly report: SmokeBenchmarkReport | null };
}

interface SmokeBenchmarkReport {
  readonly input: { readonly source: string };
  readonly plan: {
    readonly mode: string;
    readonly seed: number;
    readonly warmups?: number;
  };
  readonly run: {
    readonly id: string;
  };
  readonly schemaVersion: number;
  readonly terminalStatus: string;
}
