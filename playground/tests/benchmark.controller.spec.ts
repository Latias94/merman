import { readFile } from "node:fs/promises";

import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

import { BENCHMARK_REPORT_SCHEMA_VERSION } from "../src/benchmark/report-schema";
import {
  monitorBrowserErrors,
  openPlayground,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";

test("benchmark controller explains runtime reuse and downloads matching evidence", async ({
  page,
}) => {
  test.setTimeout(180_000);
  const errors = monitorBrowserErrors(page);
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await openPlayground(page);
  await waitForPreviewSvg(page);

  await page.getByRole("button", { name: "Bench", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Browser Benchmark" });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText(
    "Balanced Merman and Mermaid measurements in isolated browser runtimes. Fresh runtime samples may reuse the HTTP cache; cache observations are reported separately."
  );
  await expect(
    dialog.getByRole("button", { name: "Fresh runtime" })
  ).toHaveAttribute("aria-pressed", "true");
  await expect(dialog).toContainText(
    "Each measured sample rebuilds a separate iframe and module state."
  );
  const accessibility = await new AxeBuilder({ page })
    .include('[role="dialog"]')
    .analyze();
  expect(accessibility.violations).toEqual([]);
  await dialog.getByRole("button", { name: "Reused runtime" }).click();
  await expect(dialog).toContainText(
    "Each engine keeps its own isolated iframe, receives the same number of real-source warmups, then is measured repeatedly to the same strict parent-side SVG boundary."
  );
  await expect(dialog).not.toContainText(/\brealm(?:-cold)?\b/i);
  await dialog.getByLabel("Measured blocks").click();
  await page.getByRole("option", { name: "2", exact: true }).click();
  await dialog.getByLabel("Warmups per engine").click();
  await page.getByRole("option", { name: "0", exact: true }).click();
  await dialog.getByRole("button", { name: "Run", exact: true }).click();

  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeVisible();
  await expect
    .poll(() =>
      dialog.evaluate((element) => element.contains(document.activeElement)),
    )
    .toBe(true);
  await expect(
    page.locator('iframe[data-merman-realm="benchmark"]').first(),
  ).toBeAttached();
  await expect(dialog.getByRole("heading", { name: "Complete" })).toBeVisible({
    timeout: 90_000,
  });
  await expect(page.locator('iframe[data-merman-realm="benchmark"]')).toHaveCount(0);
  await expect(dialog).toContainText("6 retained samples");
  await expect(dialog).toContainText("Mermaid / Merman");

  await dialog.getByRole("button", { name: "Change settings" }).click();
  await expect(
    dialog.getByRole("heading", { name: "Measurement mode" }),
  ).toBeFocused();
  await expect(
    dialog.getByRole("button", { name: "Reused runtime" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(dialog.getByLabel("Measured blocks")).toHaveText("2");
  await expect(dialog.getByLabel("Warmups per engine")).toHaveText("0");
  await expect(dialog.getByRole("heading", { name: "Current source" })).toBeVisible();
  await dialog.getByRole("button", { name: "Back to result" }).click();
  await expect(dialog.getByRole("heading", { name: "Complete" })).toBeFocused();

  const requestCount = requests.length;
  const downloadPromise = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download JSON" }).click();
  const download = await downloadPromise;
  const path = await download.path();
  expect(path).not.toBeNull();
  const report = JSON.parse(await readFile(path!, "utf8")) as {
    input: { source: string };
    plan: { mode: string; iterations: number; seed: number; warmups: number };
    schemaVersion: number;
    terminalStatus: string;
    samples: unknown[];
  };
  expect(report).toMatchObject({
    schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
    terminalStatus: "success",
    plan: { mode: "warm", iterations: 2, warmups: 0 },
  });
  expect(report.samples).toHaveLength(6);
  expect(requests).toHaveLength(requestCount);

  await dialog.getByRole("button", { name: "Close benchmark" }).click();
  const rerunSource = "flowchart LR\n  Before --> After";
  await replaceEditorSource(page, rerunSource);
  await page.getByRole("button", { name: "Bench", exact: true }).click();
  await expect(dialog.getByRole("heading", { name: "Complete" })).toBeVisible();
  await expect(dialog.getByText("Source changed", { exact: true })).toBeVisible();
  await dialog.getByRole("button", { name: "Run again" }).click();
  await expect(dialog.getByRole("heading", { name: "Complete" })).toBeVisible({
    timeout: 90_000,
  });

  const rerunDownloadPromise = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download JSON" }).click();
  const rerunDownload = await rerunDownloadPromise;
  const rerunPath = await rerunDownload.path();
  expect(rerunPath).not.toBeNull();
  const rerunReport = JSON.parse(await readFile(rerunPath!, "utf8")) as {
    input: { source: string };
    plan: { seed: number };
  };
  expect(rerunReport.input.source).toBe(rerunSource);
  expect(rerunReport.plan.seed).not.toBe(report.plan.seed);
  errors.assertNone();
});

test("closing an active rerun preserves the last completed report and removes every realm", async ({
  page,
}) => {
  test.setTimeout(60_000);
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await waitForPreviewSvg(page);

  await page.getByRole("button", { name: "Bench", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Browser Benchmark" });
  await dialog.getByLabel("Measured blocks").click();
  await page.getByRole("option", { name: "2", exact: true }).click();
  await dialog.getByRole("button", { name: "Run", exact: true }).click();
  await expect(dialog.getByRole("heading", { name: "Complete" })).toBeVisible({
    timeout: 45_000,
  });

  await dialog.getByRole("button", { name: "Change settings" }).click();
  await dialog.getByLabel("Measured blocks").click();
  await page.getByRole("option", { name: "20", exact: true }).click();
  await dialog.getByRole("button", { name: "Back to result" }).click();
  await dialog.getByRole("button", { name: "Run again" }).click();
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeVisible();

  await dialog.getByRole("button", { name: "Close benchmark" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('iframe[data-merman-realm="benchmark"]')).toHaveCount(0);

  await page.getByRole("button", { name: "Bench", exact: true }).click();
  await expect(dialog.getByRole("heading", { name: "Complete" })).toBeVisible();
  await expect(dialog).toContainText(
    "The latest run was cancelled. Showing the previous result.",
  );
  await expect(dialog.getByRole("button", { name: "Run again" })).toBeEnabled();

  const downloadPromise = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download JSON" }).click();
  const download = await downloadPromise;
  const path = await download.path();
  expect(path).not.toBeNull();
  const retained = JSON.parse(await readFile(path!, "utf8")) as {
    plan: { iterations: number };
    terminalStatus: string;
  };
  expect(retained).toMatchObject({
    terminalStatus: "success",
    plan: { iterations: 2 },
  });
  errors.assertNone();
});

test("page lifecycle invalidation suppresses aggregates and allows a clean rerun", async ({
  page,
}) => {
  test.setTimeout(60_000);
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await waitForPreviewSvg(page);

  await page.getByRole("button", { name: "Bench", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Browser Benchmark" });
  await dialog.getByLabel("Measured blocks").click();
  await page.getByRole("option", { name: "20", exact: true }).click();
  await dialog.getByRole("button", { name: "Run", exact: true }).click();
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeVisible();

  await page.evaluate(() => {
    window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true }));
  });
  await expect(
    dialog.getByRole("heading", { name: "Environment invalidated" })
  ).toBeVisible();
  await expect(dialog.getByText("Descriptive statistics")).toHaveCount(0);
  await expect(page.locator('iframe[data-merman-realm="benchmark"]')).toHaveCount(0);

  await page.evaluate(() => {
    window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true }));
  });
  await expect(dialog.getByRole("button", { name: "Run again" })).toBeEnabled();
  errors.assertNone();
});
