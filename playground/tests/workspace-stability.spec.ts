import { expect, test } from "@playwright/test";

import {
  monitorBrowserErrors,
  openPlayground,
  replaceEditorSource,
} from "./helpers/playground";

test("editor follows its panel dimensions without layout oscillation", async ({ page }) => {
  await page.setViewportSize({ width: 950, height: 450 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(page, "flowchart LR\n  A --> B\n" + "%% A long editor comment that wraps in a narrow panel.\n".repeat(30));
  const editor = page.locator(".monaco-editor").first();
  const divider = page.getByRole("separator");
  const bounds = await divider.boundingBox();
  expect(bounds).not.toBeNull();
  await page.mouse.move(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
  await page.mouse.down();
  await page.mouse.move(650, bounds!.y + bounds!.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect.poll(() => editor.evaluate((element) => {
    const container = element.parentElement!;
    return Math.abs(element.clientWidth - container.clientWidth);
  })).toBeLessThanOrEqual(1);

  for (const viewport of [{ width: 780, height: 300 }, { width: 390, height: 500 }, { width: 950, height: 450 }]) {
    await page.setViewportSize(viewport);
    await expect.poll(() => editor.evaluate((element) => {
      const container = element.parentElement!;
      return Math.max(Math.abs(element.clientWidth - container.clientWidth), Math.abs(element.clientHeight - container.clientHeight));
    })).toBeLessThanOrEqual(1);
    const dimensions = await editor.evaluate(async (element) => {
      const samples = new Set<string>();
      for (let frame = 0; frame < 24; frame += 1) {
        await new Promise(requestAnimationFrame);
        samples.add(`${element.clientWidth}:${element.clientHeight}`);
      }
      return [...samples];
    });
    expect(dimensions).toHaveLength(1);
  }
  errors.assertNone();
});

test("gallery retains its filters and scroll position across repeated openings", async ({ page }) => {
  await page.setViewportSize({ width: 950, height: 600 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  const trigger = page.getByRole("button", { name: "Examples", exact: true });
  await trigger.click();
  const dialog = page.getByRole("dialog", { name: "Example Gallery" });
  const search = page.getByRole("searchbox", { name: "Search examples" });
  const viewport = dialog.locator("[data-radix-scroll-area-viewport]");
  await search.fill("flow");
  await viewport.evaluate((element) => { element.scrollTop = 700; });
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBe(700);
  for (let attempt = 0; attempt < 3; attempt += 1) {
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(trigger).toBeFocused();
    await trigger.click();
    await expect(search).toHaveValue("flow");
    await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBe(700);
    await expect(search).toBeFocused();
  }
  await search.fill("sequence");
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBe(0);
  await search.fill("no-such-example-012345");
  await expect(dialog.getByRole("status")).toBeVisible();
  await page.keyboard.press("Escape");
  await trigger.click();
  await expect(search).toHaveValue("no-such-example-012345");
  await expect(dialog.getByRole("status")).toBeVisible();
  errors.assertNone();
});

test("gallery category and ASCII filters survive layout changes and selection", async ({ page }) => {
  await page.setViewportSize({ width: 950, height: 600 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  const trigger = page.getByRole("button", { name: "Examples", exact: true });
  await trigger.click();
  const dialog = page.getByRole("dialog", { name: "Example Gallery" });
  const viewport = dialog.locator("[data-radix-scroll-area-viewport]");
  const category = dialog.getByRole("button", { name: "Flow", exact: true });
  const asciiOnly = dialog.getByRole("checkbox", { name: "ASCII supported" });
  await dialog.getByRole("button", { name: "Grammar", exact: true }).click();
  await asciiOnly.check();
  await expect(dialog.getByRole("button", { name: "All", exact: true }))
    .toHaveAttribute("aria-pressed", "true");
  await asciiOnly.uncheck();
  await category.click();
  await asciiOnly.check();
  await viewport.evaluate((element) => { element.scrollTop = 500; });
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBe(500);
  const anchor = await viewport.evaluate((element) => {
    const card = Array.from(element.querySelectorAll<HTMLElement>("[data-example-id]"))
      .find((card) => card.offsetTop + card.offsetHeight > element.scrollTop)!;
    return { id: card.dataset.exampleId, offset: card.offsetTop - element.scrollTop };
  });
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await page.setViewportSize({ width: 390, height: 600 });
  await trigger.click();
  await expect(category).toHaveAttribute("aria-pressed", "true");
  await expect(asciiOnly).toBeChecked();
  await expect.poll(() => viewport.evaluate((element, saved) => {
    const card = element.querySelector<HTMLElement>(`[data-example-id="${saved.id}"]`)!;
    return card.offsetTop - element.scrollTop;
  }, anchor)).toBe(anchor.offset);
  const example = viewport.locator(`[data-example-id="${anchor.id}"]`);
  await example.click();
  await expect(dialog).toBeHidden();
  await trigger.click();
  await expect(category).toHaveAttribute("aria-pressed", "true");
  await expect(asciiOnly).toBeChecked();
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  await asciiOnly.uncheck();
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBe(0);
  await viewport.evaluate((element) => { element.scrollTop = 400; });
  await dialog.getByRole("button", { name: "All", exact: true }).click();
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBe(0);
  errors.assertNone();
});
