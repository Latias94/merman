import { expect, test, type Locator } from "@playwright/test";

import { monitorBrowserErrors, openPlayground } from "./helpers/playground";

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
  const bench = page.getByRole("button", { name: "Bench", exact: true });
  const examples = page.getByRole("button", { name: "Examples", exact: true });
  const tooltip = page.locator('[data-slot="tooltip-content"]');
  const benchBounds = (await bench.boundingBox())!;
  await page.mouse.move(benchBounds.x + benchBounds.width / 2, benchBounds.y + benchBounds.height / 2);
  await page.clock.runFor(299);
  await expect(tooltip).toHaveCount(0);
  await page.clock.runFor(1);
  await expect(tooltip).toHaveAttribute("data-state", "delayed-open");
  await page.clock.runFor(300);
  const exampleBounds = (await examples.boundingBox())!;
  // Give the native hover-grace listener a frame between real pointer moves.
  const fromX = benchBounds.x + benchBounds.width / 2;
  const toX = exampleBounds.x + exampleBounds.width / 2;
  for (let step = 1; step <= 8; step += 1) {
    await page.mouse.move(fromX + (toX - fromX) * step / 8, exampleBounds.y + exampleBounds.height / 2);
    await page.clock.runFor(16);
  }
  await expect(tooltip.filter({ hasText: "Examples" })).toHaveAttribute("data-state", "instant-open");
  await expect(tooltip.filter({ hasText: "Examples" })).toHaveCSS("animation-name", "none");
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
  await page.getByRole("button", { name: "Bench", exact: true }).click();
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
