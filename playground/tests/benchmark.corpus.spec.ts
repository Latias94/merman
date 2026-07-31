import { expect, test } from "@playwright/test";

import { BENCHMARK_CORPUS_SCHEMA_VERSION } from "../src/benchmark/corpus-schema";
import { BENCHMARK_REPORT_SCHEMA_VERSION } from "../src/benchmark/report-schema";

const FIXTURE_IDS = ["basic-flowchart", "sequence-interaction"] as const;

test("@smoke non-UI corpus entry runs two independent cold and warm fixtures", async ({
  browserName,
  isMobile,
  page,
}) => {
  test.skip(browserName !== "chromium" || isMobile);
  test.setTimeout(240_000);

  await page.goto("./benchmark-corpus.html");
  await page.waitForFunction(
    () => typeof window.__MERMAN_BENCHMARK_CORPUS__?.run === "function"
  );
  const ready = await page.evaluate(() =>
    window.__MERMAN_BENCHMARK_CORPUS__!.ready()
  );
  expect(ready.availableFamilies).toBe(35);
  const discoveryRuntimeResources = await page.evaluate(() =>
    performance
      .getEntriesByType("resource")
      .map((entry) => entry.name)
      .filter((name) => /merman_wasm(?:_bg)?-/u.test(name))
  );
  expect(discoveryRuntimeResources).toEqual([]);

  const envelope = await page.evaluate(
    (fixtureIds) =>
      window.__MERMAN_BENCHMARK_CORPUS__!.run({
        fixtureIds,
        iterations: 2,
        masterSeed: 0x5eed1234,
        warmups: 0,
      }),
    FIXTURE_IDS
  );

  expect(envelope).toMatchObject({
    schemaVersion: BENCHMARK_CORPUS_SCHEMA_VERSION,
    benchmarkReportSchemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    terminalStatus: "success",
    coverage: {
      availableFamilies: 35,
      selectedFamilies: 2,
      succeededFamilies: 2,
      failedFamilies: 0,
      skippedFamilies: 33,
    },
  });
  const measured = envelope.fixtures.filter(
    (fixture) => fixture.status === "success"
  );
  expect(measured).toHaveLength(2);
  for (const fixture of measured) {
    expect(fixture.source.sha256).toMatch(/^[0-9a-f]{64}$/u);
    expect(fixture.cold.report).toMatchObject({
      schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
      terminalStatus: "success",
      run: { mode: "realm-cold", warmups: 0 },
    });
    expect(fixture.warm.report).toMatchObject({
      schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
      terminalStatus: "success",
      run: { mode: "warm", warmups: 0 },
    });
    expect(fixture.cold.report!.input.source).toBe(
      fixture.warm.report!.input.source
    );
    expect(fixture.cold.report!.run.id).not.toBe(fixture.warm.report!.run.id);
  }
  await expect(page.locator('iframe[data-merman-realm="benchmark"]')).toHaveCount(0);
});

declare global {
  interface Window {
    __MERMAN_BENCHMARK_CORPUS__?: {
      cancel(reason?: string): void;
      ready(): Promise<{ availableFamilies: number }>;
      run(request: {
        fixtureIds?: readonly string[];
        iterations: number;
        masterSeed: number;
        warmups: number;
      }): Promise<SmokeCorpusEnvelope>;
    };
  }
}

interface SmokeCorpusEnvelope {
  readonly benchmarkReportSchemaVersion: number;
  readonly coverage: {
    readonly availableFamilies: number;
    readonly failedFamilies: number;
    readonly selectedFamilies: number;
    readonly skippedFamilies: number;
    readonly succeededFamilies: number;
  };
  readonly fixtures: readonly SmokeFixtureEvidence[];
  readonly schemaVersion: number;
  readonly terminalStatus: string;
}

interface SmokeFixtureEvidence {
  readonly cold: { readonly report: SmokeSchemaFiveReport | null };
  readonly source: { readonly sha256: string };
  readonly status: string;
  readonly warm: { readonly report: SmokeSchemaFiveReport | null };
}

interface SmokeSchemaFiveReport {
  readonly input: { readonly source: string };
  readonly run: {
    readonly id: string;
    readonly mode: string;
    readonly warmups: number;
  };
  readonly schemaVersion: number;
  readonly terminalStatus: string;
}
