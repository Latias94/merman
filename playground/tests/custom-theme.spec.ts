import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";
import { encodeShareHash, decodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import {
  expectNoDocumentOverflow,
  monitorBrowserErrors,
  waitForPreviewSvg,
} from "./helpers/playground";

const recipe = {
  schema_version: 1,
  kind: "definition",
  definition: {
    authoring_schema_version: 1,
    expansion_version: 1,
    tokens: { canvas: "#fef3c7", text: "#172554", accent: "#dc2626" },
  },
};

async function start(page: Page, themePresetId: string | null = null) {
  await page.addInitScript(() => localStorage.setItem("merman-language", "en"));
  await page.goto(`./${encodeShareHash({
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    code: "flowchart LR\nA[Start] --> B[Done]",
    themePresetId,
  })}`, { waitUntil: "domcontentloaded" });
  if ((page.viewportSize()?.width ?? 1280) < 768) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);
}

async function editor(page: Page, applied = false) {
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  await page.getByRole("menuitem", { name: applied ? /Edit custom theme/ : /Custom theme/ }).click();
  return page.getByRole("dialog", { name: "Custom Merman theme" });
}

async function svg(page: Page) {
  return page.locator(".preview-container > div").first().evaluate((element) =>
    element.shadowRoot?.querySelector("svg")?.outerHTML ?? "",
  );
}

test("custom theme validates, renders, downloads, and survives a copied workspace link", async ({ page, context }) => {
  const errors = monitorBrowserErrors(page);
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await start(page);
  const dialog = await editor(page);
  await dialog.getByLabel("Theme recipe JSON").fill(JSON.stringify(recipe));
  const downloadPromise = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download JSON" }).click();
  const download = await downloadPromise;
  expect(JSON.parse(await readFile((await download.path())!, "utf8"))).toEqual(recipe);
  await dialog.getByRole("button", { name: "Validate and apply" }).click();
  await expect(dialog).toBeHidden();
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  const url = await page.evaluate(() => navigator.clipboard.readText());
  expect(new URL(url).hash).toMatch(/^#s3:/);
  expect(JSON.parse(decodeShareHash(new URL(url).hash)!.themeRecipeJson!)).toEqual(recipe);
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  const restored = await editor(page, true);
  expect(JSON.parse(await restored.getByLabel("Theme recipe JSON").inputValue())).toEqual(recipe);
  await expectNoDocumentOverflow(page);
  await page.screenshot({ path: test.info().outputPath("custom-theme-desktop.png") });
  errors.assertNone();
});

test("invalid recipes and duplicate fields preserve the applied preset; cancel discards drafts", async ({ page }) => {
  await start(page, "brutalist");
  const before = await svg(page);
  const dialog = await editor(page);
  await dialog.getByRole("button", { name: "Use selected preset" }).click();
  const exported = JSON.parse(await dialog.getByLabel("Theme recipe JSON").inputValue());
  expect(exported.schema_version).toBe(1);
  expect(["definition", "complete_spec"]).toContain(exported.kind);
  for (const draft of [
    '{"schema_version":1,"kind":"complete_spec","complete_spec":{"canvas":{"base":"#ff0000"},"canvas":{"base":"#0000ff"}}}',
    JSON.stringify({ ...recipe, unknown: true }),
    JSON.stringify(recipe).replace('"#dc2626"', '"not-a-color"'),
    JSON.stringify(recipe).replace('"#dc2626"', '"#dc2626","accent":"#000000"'),
  ]) {
    await dialog.getByLabel("Theme recipe JSON").fill(draft);
    await dialog.getByRole("button", { name: "Validate and apply" }).click();
    await expect(dialog.getByRole("alert")).toContainText("The applied theme has not changed.");
    expect(await svg(page)).toBe(before);
  }
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("button", { name: "Theme", exact: true })).toBeFocused();
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  await expect(page.getByRole("menuitemradio", { name: /^Brutalist/ })).toBeChecked();
});

test("mobile imports recipes, explains external sharing, and can switch back to a preset", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await start(page);
  const dialog = await editor(page);
  await dialog.locator('input[type="file"]').setInputFiles({
    name: "custom.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(recipe)),
  });
  await expect(dialog.getByLabel("Theme recipe JSON")).toHaveValue(JSON.stringify(recipe));
  await expectNoDocumentOverflow(page);
  const bounds = await dialog.boundingBox();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(391);
  await page.screenshot({ path: test.info().outputPath("custom-theme-mobile.png") });
  await dialog.getByRole("button", { name: "Validate and apply" }).click();
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await expect(page.getByRole("menu")).toContainText("omit Merman visual presets and custom themes");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  await page.getByRole("menuitemradio", { name: /^Brutalist/ }).click();
  await expect.poll(() => svg(page)).not.toContain("#fef3c7");
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: /Custom theme/ })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /Edit custom theme/ })).toHaveCount(0);
});

test("a preset recipe can be customized and exported as themed SVG", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await start(page, "brutalist");
  const dialog = await editor(page);
  await dialog.getByRole("button", { name: "Use selected preset" }).click();
  const exported = JSON.parse(await dialog.getByLabel("Theme recipe JSON").inputValue());
  expect(exported.kind).toBe("complete_spec");
  exported.complete_spec.canvas.base = "#fef3c7";
  await dialog.getByLabel("Theme recipe JSON").fill(JSON.stringify(exported));
  await dialog.getByRole("button", { name: "Validate and apply" }).click();
  await expect(dialog).toBeHidden();
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy SVG", exact: true }).click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toContain("<svg");
  expect(copied).toContain("#fef3c7");
});

test("oversized theme files are rejected before replacing the editable draft", async ({ page }) => {
  await start(page);
  const dialog = await editor(page);
  const before = await dialog.getByLabel("Theme recipe JSON").inputValue();
  await dialog.locator('input[type="file"]').setInputFiles({
    name: "too-large.json", mimeType: "application/json", buffer: Buffer.alloc(256 * 1024 + 1, 32),
  });
  await expect(dialog.getByRole("alert")).toContainText("256 KiB");
  await expect(dialog.getByLabel("Theme recipe JSON")).toHaveValue(before);
});

test("a late file read cannot overwrite edits or a reopened theme dialog", async ({ page }) => {
  await start(page);
  await page.evaluate(() => {
    const original = File.prototype.text;
    File.prototype.text = function () {
      if (this.name !== "delayed.json") return original.call(this);
      return new Promise<string>((resolve) => {
        window.addEventListener("release-theme-import", () => resolve("late file contents"), { once: true });
      });
    };
  });
  let dialog = await editor(page);
  const delayedFile = { name: "delayed.json", mimeType: "application/json", buffer: Buffer.from("{}") };
  await dialog.locator('input[type="file"]').setInputFiles(delayedFile);
  await expect(dialog.getByRole("status")).toHaveText("Reading theme…");
  await dialog.getByLabel("Theme recipe JSON").fill(JSON.stringify(recipe));
  await page.evaluate(() => window.dispatchEvent(new Event("release-theme-import")));
  await expect(dialog.getByLabel("Theme recipe JSON")).toHaveValue(JSON.stringify(recipe));
  await expect(dialog.getByRole("button", { name: "Validate and apply" })).toBeEnabled();

  await dialog.locator('input[type="file"]').setInputFiles(delayedFile);
  await expect(dialog.getByRole("status")).toHaveText("Reading theme…");
  await dialog.getByRole("button", { name: "Cancel" }).click();
  dialog = await editor(page);
  const reopened = await dialog.getByLabel("Theme recipe JSON").inputValue();
  await page.evaluate(() => window.dispatchEvent(new Event("release-theme-import")));
  await expect(dialog.getByLabel("Theme recipe JSON")).toHaveValue(reopened);
});

test("Chinese theme editing remains usable on a narrow screen", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 740 });
  await page.addInitScript(() => localStorage.setItem("merman-language", "zh"));
  await page.goto("./", { waitUntil: "domcontentloaded" });
  await expect(page.locator("footer")).toContainText("0.8.0");
  await page.getByRole("button", { name: "主题", exact: true }).click();
  await page.getByRole("menuitem", { name: "自定义主题…", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "自定义 Merman 主题" });
  await dialog.getByLabel("主题配方 JSON").fill("{");
  await dialog.getByRole("button", { name: "校验并应用" }).click();
  await expect(dialog.getByRole("alert")).toContainText("已应用的主题未改变。");
  await expectNoDocumentOverflow(page);
  const bounds = await dialog.boundingBox();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(321);
  await dialog.getByLabel("主题配方 JSON").fill(JSON.stringify(recipe));
  await dialog.getByRole("button", { name: "校验并应用" }).click();
  await expect(dialog).toBeHidden();
});
