import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  expectNoDocumentOverflow,
  monitorBrowserErrors,
  openPlayground,
  playgroundResourceCounts,
  previewSvgText,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";

test("@smoke loads the production WASM and renders a safe SVG", async ({
  page,
  isMobile,
}, testInfo) => {
  const errors = monitorBrowserErrors(page);
  const wasmResponse = await openPlayground(page);

  expect(wasmResponse.ok()).toBe(true);
  expect(wasmResponse.headers()["content-type"]).toContain("application/wasm");
  expect(new URL(wasmResponse.url()).origin).toBe(new URL(page.url()).origin);
  expect(new URL(wasmResponse.url()).pathname).toMatch(
    /\/assets\/merman_wasm_bg-[\w-]+\.wasm$/
  );
  expect(wasmResponse.url()).not.toContain("/@fs/");
  if (isMobile) {
    await page.getByRole("button", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);
  const resources = await playgroundResourceCounts(page);
  expect(resources.measurementProbes).toBe(2);
  expect(resources.benchmarkRealms).toBe(0);
  expect(resources.compareRealms).toBe(0);

  const accessibility = await new AxeBuilder({ page }).include("#root").analyze();
  await testInfo.attach("axe-baseline.json", {
    body: JSON.stringify(accessibility, null, 2),
    contentType: "application/json",
  });
  expect(accessibility.passes.length).toBeGreaterThan(0);

  errors.assertNone();
});

test("editing the source publishes the matching SVG without page overflow", async ({
  page,
  isMobile,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  if (!isMobile) {
    await waitForPreviewSvg(page);
  }

  await replaceEditorSource(
    page,
    "flowchart LR\n  browser[Browser smoke] --> rendered[Rendered]"
  );
  await expect(page.locator("footer")).toContainText("2 Lines");
  if (isMobile) {
    await page.getByRole("button", { name: "Preview", exact: true }).click();
  }

  await expect.poll(() => previewSvgText(page)).toContain("Browser smoke");
  await expectNoDocumentOverflow(page);
  await expect(page.locator("header")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
  // The current mobile pane unmounts Monaco; U7 removes this loader cancellation.
  errors.assertNone([/^pageerror: Canceled$/]);
});
