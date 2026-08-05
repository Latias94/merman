import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

import {
  monitorBrowserErrors,
  openPlayground,
  previewSvgText,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";
import { optionalFeatureOutput } from "./helpers/build-manifest";

test("manual tabs preserve the editor model, selection, and undo history", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);

  const source = "flowchart LR\n  A --> B";
  await replaceEditorSource(page, source);
  const editor = page.getByRole("textbox", { name: "Mermaid source" });
  await editor.focus();
  await page.keyboard.press("Shift+ArrowLeft");

  const codeTab = page.getByRole("tab", { name: "Code", exact: true });
  const configTab = page.getByRole("tab", { name: "Config", exact: true });
  await codeTab.focus();
  await page.keyboard.press("ArrowRight");
  await expect(configTab).toBeFocused();
  await expect(codeTab).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Enter");
  await expect(configTab).toHaveAttribute("aria-selected", "true");
  const configEditor = page.getByRole("textbox", {
    name: "Mermaid configuration",
  });
  await expect(configEditor).toBeVisible();
  await configEditor.focus();
  await page.keyboard.insertText("x");
  await expect(page.getByText(/Invalid JSON/)).toBeVisible();
  await expect
    .poll(() => page.locator(".monaco-editor:visible .squiggly-error").count())
    .toBeGreaterThan(0);
  await codeTab.click();
  await configTab.click();
  await expect(page.getByText(/Invalid JSON/)).toBeVisible();
  await configEditor.focus();
  await page.keyboard.press("Control+Z");
  await expect(page.getByText("Config OK", { exact: true })).toBeVisible();
  await codeTab.click();

  await editor.focus();
  await page.keyboard.insertText("C");
  await expect(page.locator("footer")).toContainText(`${source.length} Chars`);

  await waitForPreviewSvg(page);
  await expect.poll(() => previewSvgText(page)).toContain("C");

  await editor.focus();
  await page.keyboard.press("Control+Z");
  await expect.poll(() => previewSvgText(page)).toContain("B");

  errors.assertNone();
});

test("optional workbenches request code only after user activation", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  const featureOutputs = {
    benchmark: optionalFeatureOutput("benchmark"),
    config: optionalFeatureOutput("config"),
    examples: optionalFeatureOutput("examples"),
  };
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));

  await openPlayground(page);
  await expect(
    page.getByRole("textbox", { name: "Mermaid source" }),
  ).toBeVisible();
  await page.waitForLoadState("networkidle");

  for (const output of Object.values(featureOutputs)) {
    expect(wasRequested(requests, output)).toBe(false);
  }
  await page.getByRole("button", { name: "Examples", exact: true }).click();
  const exampleDialog = page.getByRole("dialog", { name: "Example Gallery" });
  const exampleSearch = page.getByRole("searchbox", {
    name: "Search examples",
  });
  await expect(exampleDialog).toBeVisible();
  expect(wasRequested(requests, featureOutputs.examples)).toBe(true);
  await exampleSearch.fill("flow");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Examples", exact: true }).click();
  await expect(exampleDialog).toBeVisible();
  await expect(exampleSearch).toHaveValue("");
  expect(requestCount(requests, featureOutputs.examples)).toBe(1);
  await page.keyboard.press("Escape");

  await page.getByRole("tab", { name: "Config", exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: "Mermaid configuration" }),
  ).toBeVisible();
  expect(wasRequested(requests, featureOutputs.config)).toBe(true);

  await page.getByRole("button", { name: "Bench", exact: true }).click();
  await expect(
    page.getByRole("dialog", { name: "Browser Benchmark" }),
  ).toBeVisible();
  expect(wasRequested(requests, featureOutputs.benchmark)).toBe(true);
  errors.assertNone();
});

test("a rejected feature chunk offers a truthful page reload", async ({
  page,
}) => {
  const configOutput = optionalFeatureOutput("config");
  let rejectNextRequest = true;
  await page.route(`**/${configOutput}`, async (route) => {
    if (rejectNextRequest) {
      rejectNextRequest = false;
      await route.abort("failed");
      return;
    }
    await route.continue();
  });

  await openPlayground(page);
  await page.getByRole("tab", { name: "Config", exact: true }).click();
  const alert = page.getByRole("alert").filter({
    hasText: "Config could not be loaded",
  });
  await expect(alert).toContainText("Config could not be loaded");

  const reloaded = page.waitForEvent("framenavigated", {
    predicate: (frame) => frame === page.mainFrame(),
  });
  await alert.getByRole("button", { name: "Reload page" }).click();
  await reloaded;
  await expect(
    page.getByRole("textbox", { name: "Mermaid source" }),
  ).toBeVisible();
  expect(rejectNextRequest).toBe(false);
  await page.getByRole("tab", { name: "Config", exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: "Mermaid configuration" }),
  ).toBeVisible();
});

test("a rejected dialog feature stays local and restores trigger focus", async ({
  page,
}) => {
  const examplesOutput = optionalFeatureOutput("examples");
  await page.route(`**/${examplesOutput}`, (route) => route.abort("failed"));

  await openPlayground(page);
  const trigger = page.getByRole("button", { name: "Examples", exact: true });
  await trigger.click();
  const alert = page.getByRole("alert").filter({
    hasText: "Examples could not be loaded",
  });
  await expect(alert).toContainText("Examples could not be loaded");

  await page.keyboard.press("Escape");
  await expect(alert).toBeHidden();
  await expect(trigger).toBeFocused();
  await expect(
    page.getByRole("textbox", { name: "Mermaid source" }),
  ).toBeVisible();
});

test("example dialog traps focus, closes with Escape, and restores its trigger", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);

  const trigger = page.getByRole("button", { name: "Examples", exact: true });
  await trigger.click();
  const dialog = page.getByRole("dialog", { name: "Example Gallery" });
  const search = page.getByRole("searchbox", { name: "Search examples" });
  await expect(dialog).toBeVisible();
  await expect(search).toBeFocused();
  await expect(
    dialog.getByRole("button", { name: "All", exact: true }),
  ).toBeVisible();

  await page.keyboard.press("Shift+Tab");
  await expect
    .poll(() =>
      dialog.evaluate((element) => element.contains(document.activeElement)),
    )
    .toBe(true);

  const accessibility = await new AxeBuilder({ page })
    .include('[role="dialog"]')
    .analyze();
  expect(accessibility.violations).toEqual([]);

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();
  errors.assertNone();
});

test("preview tabs use manual keyboard activation", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  const diagnosticsTab = page.getByRole("tab", {
    name: "Diagnostics",
    exact: true,
  });
  await expect(diagnosticsTab).toBeEnabled();
  const svgTab = page.getByRole("tab", { name: "SVG", exact: true });
  await svgTab.focus();
  await page.keyboard.press("End");
  await expect(diagnosticsTab).toBeFocused();
  await expect(svgTab).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Enter");
  await expect(diagnosticsTab).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tab", { name: "Parse JSON" })).toBeVisible();
  errors.assertNone();
});

test("system theme follows media changes while explicit themes remain stable", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.addInitScript(() => {
    if (window.localStorage.getItem("merman-ui-theme") === null) {
      window.localStorage.setItem("merman-ui-theme", "system");
    }
  });
  await openPlayground(page);

  await expect(page.locator("html")).toHaveClass(/dark/);
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).not.toHaveClass(/dark/);

  await page.evaluate(() => {
    window.localStorage.setItem("merman-ui-theme", "dark");
  });
  await page.reload({ waitUntil: "domcontentloaded" });
  await expect(page.locator("html")).toHaveClass(/dark/);
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveClass(/dark/);
});

function wasRequested(requests: readonly string[], output: string): boolean {
  return requestCount(requests, output) > 0;
}

function requestCount(requests: readonly string[], output: string): number {
  return requests.filter((url) =>
    new URL(url).pathname.endsWith(`/${output}`),
  ).length;
}
