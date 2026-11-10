import { expect, test } from "@playwright/test";

import { CORE_THEME_SAMPLE_IDS } from "../src/themes/families";
import {
  monitorBrowserErrors,
  openPlayground,
  openWorkbenchTool,
  previewSvgText,
  waitForPreviewSvg,
} from "./helpers/playground";
import { chooseThemeOption } from "./helpers/theme-select";

test("startup, render, Compare, focus, system theme, and BFCache cleanup", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.addInitScript(() => {
    window.localStorage.setItem("merman-ui-theme", "system");
  });
  const errors = monitorBrowserErrors(page);

  const wasmResponse = await openPlayground(page);
  expect(wasmResponse.ok()).toBe(true);
  expect(wasmResponse.headers()["content-type"]).toContain("application/wasm");
  await waitForPreviewSvg(page);
  await expect(page.locator("html")).toHaveClass(/dark/u);
  await expect.poll(() => previewSvgText(page)).toContain("Start");
  await expect(page.locator("footer")).toContainText("Flowchart");

  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…" }).click();
  const exportDialog = page.getByRole("dialog", { name: "Export image" });
  await exportDialog.getByRole("button", { name: "JPEG", exact: true }).click();
  await expect(exportDialog.getByRole("status")).toHaveText("Ready");
  const jpegDownload = page.waitForEvent("download");
  await exportDialog
    .getByRole("button", { name: "Download", exact: true })
    .click();
  const jpeg = await jpegDownload;
  expect(jpeg.suggestedFilename()).toBe("merman-diagram.jpg");
  expect(await downloadPrefix(jpeg, 2)).toEqual(Buffer.from([0xff, 0xd8]));
  await exportDialog.getByRole("button", { name: "Close export" }).click();

  const examplesTrigger = page.getByRole("button", {
    name: "Examples",
    exact: true,
  });
  await examplesTrigger.click();
  const examplesDialog = page.getByRole("dialog", {
    name: "Example Gallery",
  });
  await expect(examplesDialog).toBeVisible();
  await expect(
    examplesDialog.getByRole("searchbox", { name: "Search examples" }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(examplesDialog).toBeHidden();
  await expect(examplesTrigger).toBeFocused();

  await openWorkbenchTool(page, "Compare");
  await expect(page.locator('iframe[data-merman-realm="compare"]')).toHaveCount(1);
  await expect
    .poll(() => compareSvgTexts(page))
    .toEqual([
      expect.stringContaining("Start"),
      expect.stringContaining("Start"),
    ]);

  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).not.toHaveClass(/dark/u);

  await page.evaluate(() => {
    window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true }));
  });
  await expect(page.locator('iframe[data-merman-realm="compare"]')).toHaveCount(0);
  errors.assertNone();
});

test("theme library selects expanded families and exposes source code with keyboard focus", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await waitForPreviewSvg(page);
  const trigger = page.getByRole("button", { name: "Theme designs", exact: true });
  await trigger.click();
  const gallery = page.getByTestId("theme-showcase");
  const cards = gallery.locator("[data-theme-design-card]");
  await expect(cards).toHaveCount(20);
  const family = gallery.getByRole("combobox", { name: "Diagram family filter", exact: true });
  await chooseThemeOption(page, family, "treemap");
  await expect(family).toBeFocused();
  await expect(cards).toHaveCount(1);
  await expect(cards).toHaveAttribute("data-theme-design-card", "prism");
  await expect(cards.locator('[data-theme-card-primary="true"]')).toHaveAttribute("data-theme-card-sample", "treemap");
  const explore = gallery.getByRole("button", { name: "Explore Prism", exact: true });
  await explore.press("Enter");
  const detail = gallery.getByTestId("theme-showcase-detail");
  await expect(detail).toHaveAttribute("data-theme-design", "prism");
  await expect(detail.getByRole("heading", { name: "Prism", exact: true })).toBeFocused();
  const samples = detail.getByTestId("theme-sample-gallery");
  const sampleIds = ["radar", ...CORE_THEME_SAMPLE_IDS, "quadrantChart", "sankey", "treemap"];
  const tiles = samples.locator("article[data-theme-sample]");
  await expect(tiles).toHaveCount(sampleIds.length);
  expect(await tiles.evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-theme-sample")))).toEqual(sampleIds);
  const images = samples.locator("[data-theme-sample] img");
  await expect(images).toHaveCount(sampleIds.length);
  for (const image of await images.all()) {
    await image.scrollIntoViewIfNeeded();
    await expect.poll(() => image.evaluate((node) =>
      (node as HTMLImageElement).complete && (node as HTMLImageElement).naturalWidth > 0,
    )).toBe(true);
  }
  const treemap = samples.locator('article[data-theme-sample="treemap"]');
  await expect(treemap).toHaveAttribute("data-theme-sample-selected", "true");
  const inspect = treemap.getByRole("button", { name: "Inspect Treemap example", exact: true });
  await inspect.press("Enter");
  const expanded = page.getByTestId("theme-library-expanded-preview");
  const previewFamily = expanded.getByRole("combobox", { name: "Example diagram family", exact: true });
  await expect(previewFamily).toHaveAttribute("data-value", "treemap");
  await chooseThemeOption(page, previewFamily, "sankey");
  await expect(previewFamily).toBeFocused();
  await expect(expanded.getByTestId("theme-library-preview")).toHaveAttribute("data-theme-sample", "sankey");
  await expect(expanded.locator('.preview-container svg[role~="graphics-document"]')).toContainText("Premium");
  await page.keyboard.press("Escape");
  await expect(expanded).toBeHidden();
  await expect(gallery).toBeVisible();
  await expect(inspect).toBeFocused();
  const sankey = samples.locator('article[data-theme-sample="sankey"]');
  await expect(sankey).toHaveAttribute("data-theme-sample-selected", "true");
  await expect(samples.locator('article[data-theme-sample-selected="true"]')).toHaveCount(1);
  await sankey.getByRole("button", { name: "Use this example", exact: true }).click();
  const usage = detail.getByTestId("theme-showcase-use");
  await expect(usage.getByRole("heading")).toBeInViewport();
  await usage.getByRole("tab", { name: "Web", exact: true }).click();
  await expect(usage.getByTestId("theme-showcase-code")).toContainText("renderSvgResult");
  const usageFamily = usage.getByRole("combobox", { name: "Example diagram for this code", exact: true });
  await expect(usageFamily).toHaveAttribute("data-value", "sankey");
  await expect(usage.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-sample", "sankey");
  const diagramFile = usage.getByRole("navigation", { name: "Project files", exact: true }).getByRole("button", { name: "diagram.mmd", exact: true });
  await diagramFile.click();
  await expect(diagramFile).toHaveAttribute("aria-current", "true");
  const source = usage.getByTestId("theme-showcase-code");
  await expect(source).toHaveAttribute("data-code-language", "mermaid");
  await expect(source).toContainText("sankey-beta");
  await expect(source).toContainText("Premium");
  await chooseThemeOption(page, usageFamily, "flowchart");
  await expect(usageFamily).toBeFocused();
  await expect(samples.locator('article[data-theme-sample="flowchart"]')).toHaveAttribute("data-theme-sample-selected", "true");
  await expect(sankey).toHaveAttribute("data-theme-sample-selected", "false");
  await diagramFile.click();
  await expect(source).toHaveAttribute("data-code-language", "mermaid");
  await expect(source).not.toContainText("sankey-beta");
  await expect.poll(() => source.locator(".view-lines span").evaluateAll((spans) => {
    const tokens = spans.map((span) => ({ text: span.textContent?.replaceAll("\u00a0", " ") ?? "", color: getComputedStyle(span).color }));
    const keyword = tokens.find((token) => token.text === "flowchart");
    const label = tokens.find((token) => token.text.includes("Review workflow"));
    return Boolean(keyword && label && keyword.color !== label.color);
  })).toBe(true);
  await detail.getByRole("button", { name: "All designs", exact: true }).click();
  await expect(family).toHaveAttribute("data-value", "treemap");
  await expect(cards).toHaveCount(1);
  await expect(explore).toBeFocused();
  await gallery.getByTestId("theme-library-family-chip").getByRole("button").click();
  await expect(family).toHaveAttribute("data-value", "");
  await expect(family).toBeFocused();
  await expect(cards).toHaveCount(20);
  await page.keyboard.press("Escape");
  await expect(gallery).toBeHidden();
  await expect(trigger).toBeFocused();
  errors.assertNone();
});

async function compareSvgTexts(page: import("@playwright/test").Page): Promise<string[]> {
  return page
    .locator(".preview-container > div")
    .evaluateAll((hosts) =>
      hosts.map(
        (host) => host.shadowRoot?.querySelector("svg")?.textContent ?? "",
      ),
    );
}

async function downloadPrefix(
  download: import("@playwright/test").Download,
  length: number,
): Promise<Buffer> {
  const stream = await download.createReadStream();
  if (!stream) throw new Error("Download stream is unavailable");
  for await (const chunk of stream) {
    stream.destroy();
    return Buffer.from(chunk).subarray(0, length);
  }
  throw new Error("Downloaded artifact is empty");
}
