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

test("export workbench targets toolbar and Compare artifacts through one dialog", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await page.addInitScript(() => {
    const blobs = new Map<string, Blob>();
    const createObjectURL = URL.createObjectURL;
    URL.createObjectURL = ((object: Blob | MediaSource) => {
      const url = createObjectURL.call(URL, object);
      if (object instanceof Blob) blobs.set(url, object);
      return url;
    }) as typeof URL.createObjectURL;
    Object.defineProperty(window, "__mermanExportBlobs", { value: blobs });
  });
  await openPlayground(page);
  await waitForPreviewSvg(page);

  const trigger = page.getByRole("button", { name: "Export", exact: true });
  await trigger.click();
  await page.getByRole("menuitem", { name: "Export image…" }).click();
  const dialog = page.getByRole("dialog", { name: "Export image" });
  await expect(dialog).toHaveAttribute("data-export-engine", "merman");
  await expect(
    dialog.getByRole("button", { name: "SVG", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(dialog.getByRole("status")).toHaveText("Ready");

  await dialog.getByRole("button", { name: "PNG", exact: true }).click();
  await dialog.getByRole("button", { name: "Transparent", exact: true }).click();
  await dialog.getByRole("button", { name: "Width", exact: true }).click();
  await dialog.getByRole("textbox", { name: /^Width/u }).fill("320");
  await expect(dialog.getByTestId("export-output-dimensions")).toContainText(
    "320 ×",
  );
  await expect(dialog.getByRole("status")).toHaveText("Ready");
  const widthInput = dialog.getByRole("textbox", { name: /^Width/u });
  const lastSuccessfulPreview = await dialog
    .getByRole("img", { name: "Export preview" })
    .getAttribute("src");
  await widthInput.fill("");
  await expect(dialog.getByRole("alert")).toContainText(
    "Width must be a positive integer",
  );
  await expect(
    dialog.getByRole("button", { name: "Download", exact: true }),
  ).toBeDisabled();
  await expect(dialog.getByRole("img", { name: "Export preview" })).toHaveAttribute(
    "src",
    lastSuccessfulPreview ?? "",
  );
  await widthInput.fill("320");
  await expect(dialog.getByRole("status")).toHaveText("Ready");

  await dialog.getByRole("button", { name: "JPEG", exact: true }).click();
  await expect(
    dialog.getByRole("button", { name: "Transparent", exact: true }),
  ).toBeDisabled();
  await expect(
    dialog.getByRole("button", { name: "Custom", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  const quality = dialog.getByRole("slider", { name: "Quality" });
  await quality.fill("73");
  await expect(dialog.getByRole("status")).toHaveText("Ready");
  await page.evaluate(() => {
    const exportWindow = window as typeof window & {
      __mermanOriginalCanvasToBlob?: HTMLCanvasElement["toBlob"];
    };
    exportWindow.__mermanOriginalCanvasToBlob =
      HTMLCanvasElement.prototype.toBlob;
    HTMLCanvasElement.prototype.toBlob = (callback) => callback(null);
  });
  await quality.fill("72");
  await expect(dialog.getByRole("alert")).toContainText("could not encode");
  await expect(
    dialog.getByRole("button", { name: "Download", exact: true }),
  ).toBeDisabled();
  await page.evaluate(() => {
    const exportWindow = window as typeof window & {
      __mermanOriginalCanvasToBlob?: HTMLCanvasElement["toBlob"];
    };
    const original = exportWindow.__mermanOriginalCanvasToBlob;
    if (!original) throw new Error("Canvas encoder was not captured.");
    HTMLCanvasElement.prototype.toBlob = original;
    delete exportWindow.__mermanOriginalCanvasToBlob;
  });
  await quality.fill("71");
  await expect(dialog.getByRole("status")).toHaveText("Ready");
  const previewBytes = await dialog
    .getByRole("img", { name: "Export preview" })
    .evaluate(async (image) => {
      const blobs = (
        window as typeof window & {
          __mermanExportBlobs: Map<string, Blob>;
        }
      ).__mermanExportBlobs;
      const blob = blobs.get((image as HTMLImageElement).src);
      if (!blob) throw new Error("Preview Blob was not captured.");
      return Array.from(new Uint8Array(await blob.arrayBuffer()));
    });
  const jpegDownload = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download", exact: true }).click();
  const downloadedJpeg = await jpegDownload;
  expect(downloadedJpeg.suggestedFilename()).toBe("merman-diagram.jpg");
  expect(await downloadBytes(downloadedJpeg)).toEqual(Buffer.from(previewBytes));

  const accessibility = await new AxeBuilder({ page })
    .include('[data-testid="export-dialog"]')
    .analyze();
  expect(accessibility.violations).toEqual([]);
  await dialog.getByRole("button", { name: "Close export" }).click();
  await expect(trigger).toBeFocused();

  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  const mermaidPane = page.locator('[data-merman-compare-engine="mermaid"]');
  await mermaidPane.getByRole("button", { name: "Export image" }).click();
  await expect(dialog).toHaveAttribute("data-export-engine", "mermaid");
  await expect(dialog).toContainText("Mermaid JS");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
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

test("ASCII mode remains selected while an edited diagram is updating", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(page, "flowchart LR\n  Alpha --> Beta");
  await waitForPreviewSvg(page);

  const asciiTab = page.getByRole("tab", { name: "ASCII", exact: true });
  await asciiTab.click();
  await expect(asciiTab).toHaveAttribute("aria-selected", "true");

  await replaceEditorSource(page, "flowchart LR\n  Alpha --> Gamma");
  await expect(asciiTab).toHaveAttribute("aria-selected", "true");
  await expect(page.getByText("ASCII not supported for this diagram type")).toBeHidden();
  const asciiEditor = page.getByTestId("ascii-artifact-editor");
  await expect(asciiEditor).toBeVisible();
  await expect
    .poll(() => asciiEditor.locator(".view-line").allTextContents())
    .toEqual(expect.arrayContaining([expect.stringContaining("Alpha")]));
  const copyAscii = page.getByRole("button", { name: "Copy ASCII", exact: true });
  await expect(copyAscii).toBeEnabled();
  await copyAscii.click();
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

async function downloadBytes(
  download: import("@playwright/test").Download,
): Promise<Buffer> {
  const stream = await download.createReadStream();
  if (!stream) throw new Error("Download stream is unavailable");
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(Buffer.from(chunk));
  return Buffer.concat(chunks);
}
