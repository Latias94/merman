import { expect, test, type Locator } from "@playwright/test";

import { monitorBrowserErrors, openPlayground, openWorkbenchTool } from "./helpers/playground";

test("gallery keeps the active category visible when reopened and resized", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 600 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  const trigger = page.getByRole("button", { name: "Examples", exact: true });
  await trigger.click();
  const dialog = page.getByRole("dialog", { name: "Example Gallery" });
  const nav = dialog.getByRole("navigation", { name: "Example categories" });
  await nav.getByRole("button", { name: "Grammar", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await trigger.click();
  await expectCategoryVisible(nav);
  await expect(page.getByRole("searchbox", { name: "Search examples" })).toBeFocused();
  for (const size of [{ width: 950, height: 350 }, { width: 320, height: 600 }]) {
    await page.setViewportSize(size);
    await expectCategoryVisible(nav);
  }
  await page.getByRole("searchbox", { name: "Search examples" }).fill("flow");
  await expectCategoryVisible(nav);
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollTop)).toBe(0);
  errors.assertNone();
});

test("tooltips share the configured initial delay and adjacent skip delay", async ({ page }) => {
  // Load with a running clock, then pause ahead of it without racing the host clock.
  await page.clock.install({ time: new Date("2026-01-01T00:00:00Z") });
  await openPlayground(page);
  await page.waitForLoadState("networkidle");
  await page.clock.pauseAt(new Date("2026-01-01T01:00:00Z"));
  const tools = page.getByRole("button", { name: "Tools", exact: true });
  const settings = page.getByRole("button", { name: "App settings", exact: true });
  const tooltip = page.locator('[data-slot="tooltip-content"]');
  const toolsBounds = (await tools.boundingBox())!;
  await page.mouse.move(toolsBounds.x + toolsBounds.width / 2, toolsBounds.y + toolsBounds.height / 2);
  await page.clock.runFor(299);
  await expect(tooltip).toHaveCount(0);
  await page.clock.runFor(1);
  await expect(tooltip).toHaveAttribute("data-state", "delayed-open");
  await page.clock.runFor(300);
  const settingsBounds = (await settings.boundingBox())!;
  const gapStart = toolsBounds.x + toolsBounds.width;
  const gapWidth = settingsBounds.x - gapStart;
  const pointerY = toolsBounds.y + toolsBounds.height / 2;
  expect(gapWidth).toBeGreaterThan(0);
  // Use actual neighbors. Crossing the toolbar can open an intermediate tooltip.
  // Leave into the gap, then let Radix's native listener clear hover grace there
  // before entering the next trigger; that trigger must open without advancing time.
  await page.mouse.move(gapStart + gapWidth / 3, pointerY);
  await page.clock.runFor(16);
  await page.mouse.move(gapStart + gapWidth * 2 / 3, pointerY);
  await expect(tools).toHaveAttribute("data-state", "closed");
  await page.mouse.move(settingsBounds.x + settingsBounds.width / 2, settingsBounds.y + settingsBounds.height / 2);
  await expect(tooltip.filter({ hasText: "App settings" })).toHaveAttribute("data-state", "instant-open");
  await expect(tooltip.filter({ hasText: "App settings" })).toHaveCSS("animation-name", "none");
});

test("reduced motion removes movement from shared dialogs, menus, selects, and tooltips", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  const trigger = page.getByRole("button", { name: "Examples", exact: true });
  await trigger.focus();
  await page.keyboard.press("Enter");
  const gallery = page.getByRole("dialog", { name: "Example Gallery" });
  await expect(gallery).toHaveCSS("animation-name", "none");
  await expect(page.locator('[data-slot="dialog-overlay"]')).toHaveCSS("animation-name", "none");
  await page.keyboard.press("Escape");
  await expect(trigger).toBeFocused();
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await expect(page.getByRole("menu")).toHaveCSS("animation-name", "none");
  await page.keyboard.press("Escape");
  await openWorkbenchTool(page, "Benchmark");
  await page.getByRole("combobox", { name: "Measured blocks" }).click();
  await expect(page.getByRole("listbox")).toHaveCSS("animation-name", "none");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await trigger.hover();
  await expect(page.locator('[data-slot="tooltip-content"]').filter({ hasText: "Examples" }))
    .toHaveCSS("animation-name", "none");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await trigger.click();
  await expect(gallery).toHaveCSS("animation-name", "enter");
  errors.assertNone();
});


test("gallery reveals an unchanged category after preceding filters disappear", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 600 });
  await openPlayground(page);
  await page.getByRole("button", { name: "Examples", exact: true }).click();
  const nav = page.getByRole("navigation", { name: "Example categories" });
  const category = nav.getByRole("button", { name: "Planning", exact: true });
  await category.click();
  await page.getByRole("searchbox", { name: "Search examples" }).fill("grammar");
  await expect(category).toHaveAttribute("aria-pressed", "true");
  await expectCategoryVisible(nav);
});

async function expectCategoryVisible(nav: Locator): Promise<void> {
  await expect.poll(() => nav.evaluate((element) => {
    const selected = element.querySelector('[aria-pressed="true"]')!;
    const selectedRect = selected.getBoundingClientRect();
    return [element, element.parentElement!].every((container) => {
      const bounds = container.getBoundingClientRect();
      return selectedRect.left >= bounds.left - 1 && selectedRect.right <= bounds.right + 1 &&
        selectedRect.top >= bounds.top - 1 && selectedRect.bottom <= bounds.bottom + 1;
    });
  })).toBe(true);
}
