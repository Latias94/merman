import { readFile } from "node:fs/promises";

import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

import {
  monitorBrowserErrors,
  openPlayground,
  waitForPreviewSvg,
} from "./helpers/playground";

test("benchmark controller completes a balanced warm run and downloads matching evidence", async ({
  page,
  isMobile,
}) => {
  test.setTimeout(120_000);
  const errors = monitorBrowserErrors(page);
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await openPlayground(page);
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);

  await page.getByRole("button", { name: "Bench", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Browser Benchmark" });
  await expect(dialog).toBeVisible();
  const accessibility = await new AxeBuilder({ page })
    .include('[role="dialog"]')
    .analyze();
  expect(accessibility.violations).toEqual([]);
  await dialog.getByRole("button", { name: "Warm render" }).click();
  await dialog.getByLabel("Measured blocks").click();
  await page.getByRole("option", { name: "2", exact: true }).click();
  await dialog.getByLabel("Warmups per engine").click();
  await page.getByRole("option", { name: "0", exact: true }).click();
  await dialog.getByRole("button", { name: "Run", exact: true }).click();

  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeVisible();
  await expect(page.locator('iframe[data-merman-realm="benchmark"]')).toHaveCount(2);
  await expect(dialog.getByRole("heading", { name: "Complete" })).toBeVisible({
    timeout: 90_000,
  });
  await expect(page.locator('iframe[data-merman-realm="benchmark"]')).toHaveCount(0);
  await expect(dialog).toContainText("6 retained samples");
  await expect(dialog).toContainText("Mermaid / Merman");

  const requestCount = requests.length;
  const downloadPromise = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download JSON" }).click();
  const download = await downloadPromise;
  const path = await download.path();
  expect(path).not.toBeNull();
  const report = JSON.parse(await readFile(path!, "utf8")) as {
    schemaVersion: number;
    terminalStatus: string;
    run: { mode: string; iterations: number; warmups: number };
    samples: unknown[];
  };
  expect(report).toMatchObject({
    schemaVersion: 1,
    terminalStatus: "success",
    run: { mode: "warm", iterations: 2, warmups: 0 },
  });
  expect(report.samples).toHaveLength(6);
  expect(requests).toHaveLength(requestCount);
  errors.assertNone();
});

test("closing an active benchmark cancels it and removes every benchmark realm", async ({
  page,
  isMobile,
}) => {
  test.setTimeout(60_000);
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);

  await page.getByRole("button", { name: "Bench", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Browser Benchmark" });
  await dialog.getByLabel("Measured blocks").click();
  await page.getByRole("option", { name: "20", exact: true }).click();
  await dialog.getByRole("button", { name: "Run", exact: true }).click();
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeVisible();

  await dialog.getByRole("button", { name: "Close benchmark" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('iframe[data-merman-realm="benchmark"]')).toHaveCount(0);

  await page.getByRole("button", { name: "Bench", exact: true }).click();
  await expect(dialog.getByRole("heading", { name: "Cancelled" })).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Run again" })).toBeEnabled();
  errors.assertNone();
});

test("page lifecycle invalidation suppresses aggregates and allows a clean rerun", async ({
  page,
  isMobile,
}) => {
  test.setTimeout(60_000);
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
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
