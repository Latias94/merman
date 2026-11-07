import { expect, test } from "@playwright/test";

import {
  monitorBrowserErrors,
  openPlayground,
  openWorkbenchTool,
  previewSvgText,
  waitForPreviewSvg,
} from "./helpers/playground";

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

test("theme designs display five families and expose integration code with keyboard focus", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await waitForPreviewSvg(page);
  const trigger = page.getByRole("button", { name: "Theme designs", exact: true });
  await trigger.click();
  const gallery = page.getByTestId("theme-showcase");
  await expect(gallery.locator("[data-theme-design-card]")).toHaveCount(16);
  await gallery.getByRole("button", { name: "Explore Riso", exact: true }).click();
  const images = gallery.locator("[data-theme-sample] img");
  await expect(images).toHaveCount(5);
  for (const image of await images.all()) {
    await image.scrollIntoViewIfNeeded();
    await expect.poll(() => image.evaluate((node) =>
      (node as HTMLImageElement).complete && (node as HTMLImageElement).naturalWidth > 0,
    )).toBe(true);
  }
  const usage = gallery.getByTestId("theme-showcase-use");
  await usage.getByRole("tab", { name: "Web", exact: true }).click();
  await expect(usage.getByTestId("theme-showcase-code")).toContainText("renderSvgResult");
  await usage.getByRole("navigation", { name: "Project files", exact: true }).getByRole("button", { name: "diagram.mmd", exact: true }).click();
  const source = usage.getByTestId("theme-showcase-code");
  await expect(source).toHaveAttribute("data-code-language", "mermaid");
  await expect.poll(() => source.locator(".view-lines span").evaluateAll((spans) => {
    const tokens = spans.map((span) => ({ text: span.textContent?.replaceAll("\u00a0", " ") ?? "", color: getComputedStyle(span).color }));
    const keyword = tokens.find((token) => token.text === "flowchart");
    const label = tokens.find((token) => token.text.includes("Review workflow"));
    return Boolean(keyword && label && keyword.color !== label.color);
  })).toBe(true);
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
