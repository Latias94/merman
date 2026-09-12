import { expect, test } from "@playwright/test";

import {
  monitorBrowserErrors,
  openPlayground,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";

for (const viewport of [{ width: 1440, height: 900 }, { width: 390, height: 844 }]) {
  test(`SVG pipeline guidance preserves the advanced choice at ${viewport.width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    const errors = monitorBrowserErrors(page);
    await openPlayground(page);
    await replaceEditorSource(page, "flowchart LR\nA[many words are too much sometime]");
    if (viewport.width < 768) {
      await page.getByRole("tab", { name: "Preview", exact: true }).click();
    }
    await waitForPreviewSvg(page);

    const settings = page.getByRole("button", { name: "Render settings", exact: true });
    await settings.click();
    await expect(page.getByRole("menuitemradio", { name: "Mermaid SVG (default)", exact: true }))
      .toHaveAttribute("aria-checked", "true");
    const fallback = page.getByRole("menuitemradio", { name: "Text fallback SVG", exact: true });
    await expect(fallback).toHaveAccessibleDescription(/May show duplicate text in browsers/);
    await fallback.click();

    const warning = page.getByRole("status").filter({ hasText: "Text fallback SVG keeps HTML labels" });
    await expect(warning).toBeVisible();
    await settings.click();
    await expect(fallback).toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
    await expect(warning).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath("text-fallback-guidance.png") });

    await page.getByRole("button", { name: "Use Mermaid SVG", exact: true }).click();
    await expect(warning).toHaveCount(0);
    await waitForPreviewSvg(page);
    await settings.click();
    await expect(page.getByRole("menuitemradio", { name: "Mermaid SVG (default)", exact: true }))
      .toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
    errors.assertNone();
  });
}
