import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

import {
  monitorBrowserErrors,
  openPlayground,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";

for (const viewport of [{ width: 1440, height: 900 }, { width: 390, height: 844 }]) {
  test(`Export freezes the selected SVG pipeline at ${viewport.width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    const errors = monitorBrowserErrors(page);
    await openPlayground(page);
    await replaceEditorSource(page, "flowchart LR\nA[many words are too much sometime]");
    if (viewport.width < 768) {
      await page.getByRole("tab", { name: "Preview", exact: true }).click();
    }
    await waitForPreviewSvg(page);
    await expect.poll(() => page.locator(".preview-container > div").first().evaluate((host) =>
      Boolean(host.shadowRoot?.querySelector("foreignObject")),
    )).toBe(true);

    const exportMenu = page.getByRole("button", { name: "Export", exact: true });
    await exportMenu.click();
    const parity = page.getByRole("menuitemradio", { name: "Mermaid SVG (default)", exact: true });
    await expect(parity).toHaveAttribute("aria-checked", "true");
    const fallback = page.getByRole("menuitemradio", { name: "Text fallback SVG", exact: true });
    await expect(fallback).toHaveAccessibleDescription(/May show duplicate text in browsers/);
    await fallback.click();
    await expect(fallback).toHaveAttribute("aria-checked", "true");
    await expect(page.getByRole("menu")).toContainText("Text fallback SVG keeps HTML labels");

    await page.getByRole("menuitem", { name: "Export image…", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Export image", exact: true });
    await expect(dialog).toHaveAttribute("data-export-svg-pipeline", "readable");
    const readablePublication = await dialog.getAttribute("data-export-publication");
    await expect(dialog.getByTestId("output-compatibility")).toContainText("Text fallback SVG keeps HTML labels");
    await expect(dialog.getByRole("menuitemradio")).toHaveCount(0);
    await page.screenshot({ path: testInfo.outputPath("text-fallback-guidance.png") });
    await dialog.getByRole("button", { name: "Close export" }).click();
    await expect(exportMenu).toBeFocused();

    await exportMenu.click();
    await expect(fallback).toHaveAttribute("aria-checked", "true");
    await parity.click();
    await expect(parity).toHaveAttribute("aria-checked", "true");
    await page.getByRole("menuitem", { name: "Export image…", exact: true }).click();
    await expect(dialog).toHaveAttribute("data-export-svg-pipeline", "parity");
    await expect(dialog).not.toHaveAttribute("data-export-publication", readablePublication!);
    await expect(dialog.getByTestId("output-compatibility")).toContainText("SVG pipeline: Mermaid SVG (default)");
    await dialog.getByRole("button", { name: "Close export" }).click();

    await exportMenu.click();
    await page.getByRole("menuitemradio", { name: "resvg-compatible SVG", exact: true }).click();
    await expect(page.getByRole("menu")).toContainText("It does not establish portability to every host.");
    await page.getByRole("menuitem", { name: "Export image…", exact: true }).click();
    await expect(dialog).toHaveAttribute("data-export-svg-pipeline", "resvg-safe");
    await expect(dialog.getByTestId("output-compatibility")).toContainText("It does not establish portability to every host.");
    const downloadPromise = page.waitForEvent("download");
    await dialog.getByRole("button", { name: "Download", exact: true }).click();
    const download = await downloadPromise;
    const downloadPath = await download.path();
    expect(downloadPath).not.toBeNull();
    const svg = await readFile(downloadPath!, "utf8");
    const label = await page.evaluate((source) => {
      const document = new DOMParser().parseFromString(source, "image/svg+xml");
      return [...document.querySelectorAll("text[data-merman-source-classes~='nodeLabel']")]
        .map((node) => node.textContent).join("").replace(/\s+/gu, " ").trim();
    }, svg);
    expect(label).toBe("many words are too much sometime");
    expect(svg).not.toContain("<foreignObject");
    errors.assertNone();
  });
}
