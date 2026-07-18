import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

import {
  monitorBrowserErrors,
  openPlayground,
  previewSvgText,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";

test("manual tabs preserve the editor model, selection, and undo history", async ({
  page,
  isMobile,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);

  const source = "flowchart LR\n  A --> B";
  await replaceEditorSource(page, source);
  const editor = page.getByRole("textbox", { name: "Editor content" }).first();
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
  await codeTab.click();

  await editor.focus();
  await page.keyboard.insertText("C");
  await expect(page.locator("footer")).toContainText(`${source.length} Chars`);

  if (isMobile) {
    const editorPanel = page.getByRole("tabpanel", { name: "Editor" });
    const previewPanel = page.getByRole("tabpanel", { name: "Preview" });
    await expect(editorPanel).toBeVisible();
    await expect(previewPanel).toBeHidden();
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
    await expect(editorPanel).toBeHidden();
    await expect(previewPanel).toBeVisible();
  }
  await waitForPreviewSvg(page);
  await expect.poll(() => previewSvgText(page)).toContain("C");

  if (isMobile) {
    await page.getByRole("tab", { name: "Editor", exact: true }).click();
  }
  await editor.focus();
  await page.keyboard.press("Control+Z");
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await expect.poll(() => previewSvgText(page)).toContain("B");
  errors.assertNone();
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
    dialog.getByRole("button", { name: "All", exact: true })
  ).toBeVisible();

  await page.keyboard.press("Shift+Tab");
  await expect
    .poll(() =>
      dialog.evaluate((element) => element.contains(document.activeElement))
    )
    .toBe(true);

  const accessibility = await new AxeBuilder({ page }).include('[role="dialog"]').analyze();
  expect(accessibility.violations).toEqual([]);

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();
  errors.assertNone();
});

test("preview tabs use manual keyboard activation", async ({ page, isMobile }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  if (isMobile) {
    const editorTab = page.getByRole("tab", { name: "Editor", exact: true });
    const previewTab = page.getByRole("tab", { name: "Preview", exact: true });
    await editorTab.focus();
    await page.keyboard.press("ArrowRight");
    await expect(previewTab).toBeFocused();
    await expect(editorTab).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Enter");
    await expect(previewTab).toHaveAttribute("aria-selected", "true");
  }

  const svgTab = page.getByRole("tab", { name: "SVG", exact: true });
  await svgTab.press("End");
  const diagnosticsTab = page.getByRole("tab", {
    name: "Diagnostics",
    exact: true,
  });
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
