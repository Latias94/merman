import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";
import { strFromU8, unzipSync } from "fflate";

import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { monitorBrowserErrors, replaceMonacoText, waitForPreviewSvg } from "./helpers/playground";

const themes = [["paper-cut", "Paper Cut", "PaperCut"], ["carbon-copy", "Carbon Copy", "CarbonCopy"], ["pop-art", "Pop Art", "PopArt"]] as const;

async function start(page: Page, themePresetId: string | null = null) {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript(() => {
    if (window === window.top) localStorage.setItem("merman-language", "en");
  });
  await page.goto(`./${encodeShareHash({
    ...DEFAULT_WORKSPACE_SNAPSHOT, code: "flowchart LR\nA[Keep this source] --> B[Done]",
    mermaidConfig: '{"flowchart":{"htmlLabels":false}}', themePresetId,
  })}`, { waitUntil: "domcontentloaded" });
  if ((page.viewportSize()?.width ?? 1280) < 768) await page.getByRole("tab", { name: "Preview", exact: true }).click();
  await waitForPreviewSvg(page);
}

async function sharedWorkspace(page: Page) {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  return decodeShareHash(new URL(await page.evaluate(() => navigator.clipboard.readText())).hash)!;
}

for (const [id, name, variant] of themes) {
  test(`${id} showcase keeps complete JSON while adopting builtin code and selecting its preset`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    await start(page);
    const canonical = await readFile(new URL(`../../crates/merman-render/assets/themes/${id}.theme.json`, import.meta.url), "utf8");
    await page.getByRole("button", { name: "Theme designs", exact: true }).click();
    const showcase = page.getByTestId("theme-showcase");
    await showcase.getByRole("button", { name: `Explore ${name}`, exact: true }).click();
    const adoption = showcase.getByTestId("theme-showcase-use");
    await adoption.getByRole("button", { name: "Copy", exact: true }).click();
    expect((await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/gu, "\n")).toBe(canonical.replace(/\r\n/gu, "\n"));
    for (const [target, label] of [["rust", "Rust"], ["cli", "CLI"], ["web", "Web"]] as const) {
      await adoption.getByRole("tab", { name: label, exact: true }).click();
      if (target === "rust") {
        await expect(adoption.getByTestId("theme-showcase-code")).toContainText(`compile_preset(ThemePreset::${variant})`);
        await adoption.getByRole("radio", { name: "Project files · include_str!", exact: true }).check();
      }
      const downloading = page.waitForEvent("download");
      await adoption.getByRole("button", { name: "Download example project", exact: true }).click();
      const files = unzipSync(await readFile((await (await downloading).path())!));
      expect(files["theme.json"]).toBeUndefined();
      expect(JSON.parse(strFromU8(files["options.json"])).theme).toEqual({ preset: id, appearance: "light" });
      expect(JSON.parse(strFromU8(files["handoff.json"])).theme.preset_id).toBe(id);
      if (target === "rust") expect(strFromU8(files["src/main.rs"])).toContain(`compile_preset(ThemePreset::${variant})`);
      if (target === "cli") expect(strFromU8(files["render-command.txt"])).toContain(`--theme-preset ${id}`);
    }
    await adoption.getByRole("tab", { name: "Theme JSON", exact: true }).click();
    await adoption.getByRole("button", { name: "Copy", exact: true }).click();
    expect(JSON.parse(await page.evaluate(() => navigator.clipboard.readText()))).toEqual(JSON.parse(canonical));
    await showcase.getByRole("button", { name: "Apply to my diagram", exact: true }).click();
    const snapshot = await sharedWorkspace(page);
    expect(snapshot.themePresetId).toBe(id);
    expect(snapshot.themeRecipeJson).toBeNull();
    await page.locator("[data-theme-launcher]:visible").click();
    const panel = page.locator("#theme-panel");
    await expect(panel.locator('[data-theme-group="builtin"]')).toHaveCount(1);
    await expect(panel.locator(`[data-theme-preset="${id}"]`)).toHaveCount(1);
    await expect(panel.locator(`[data-theme-choice="preset:${id}"]`)).toHaveAttribute("aria-pressed", "true");
    await expect(panel.locator(`[data-theme-choice="exploration:${id}"]`)).toHaveCount(0);
    await panel.getByRole("button", { name: "Theme designs", exact: true }).click();
    await expect(showcase.getByTestId("theme-showcase-detail")).toHaveAttribute("data-theme-design", id);
    await expect(showcase.getByTestId("theme-showcase-detail").getByText("Current theme", { exact: true })).toBeVisible();
    errors.assertNone();
  });

  test(`${id} full configuration can be viewed, copied, downloaded and customized without changing the builtin`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    await start(page, id);
    await page.locator("[data-theme-launcher]:visible").click();
    const panel = page.locator("#theme-panel");
    await panel.getByTestId("theme-advanced").locator("summary").first().click();
    await expect(panel.getByTestId("theme-json-document")).toHaveAttribute("data-code-language", "json");
    await panel.getByRole("button", { name: "Copy JSON", exact: true }).click();
    const exported = await page.evaluate(() => navigator.clipboard.readText());
    const recipe = JSON.parse(exported);
    const canonical = JSON.parse(await readFile(new URL(`../../crates/merman-render/assets/themes/${id}.theme.json`, import.meta.url), "utf8"));
    if (id === "carbon-copy") {
      // Gradient offsets cross the typed f32 boundary when a preset is exported.
      for (const index of [1, 2]) {
        const authored = canonical.design.canvas.layers[0].paint.stops[index];
        const saved = recipe.design.canvas.layers[0].paint.stops[index];
        expect(Math.fround(saved.offset)).toBe(Math.fround(authored.offset));
        authored.offset = saved.offset;
      }
    }
    expect(recipe).toEqual(canonical);
    expect(recipe.kind).toBe("design");
    expect(recipe.design.tokens.ink.label).toBeDefined();
    expect(recipe.design.rules.length).toBeGreaterThan(0);
    const downloading = page.waitForEvent("download");
    await panel.getByRole("button", { name: "Download JSON", exact: true }).click();
    expect(JSON.parse(await readFile((await (await downloading).path())!, "utf8"))).toEqual(canonical);
    await panel.getByRole("button", { name: "Customize", exact: true }).click();
    recipe.design.tokens.ink.value = "#B00020";
    const edited = `${JSON.stringify(recipe, null, 2)}\n`;
    await replaceMonacoText(page, "Editable theme JSON", edited);
    await panel.getByRole("button", { name: "Validate theme", exact: true }).click();
    await expect(panel.getByRole("button", { name: "Apply changes", exact: true })).toBeEnabled();
    await panel.getByRole("button", { name: "Apply changes", exact: true }).click();
    const snapshot = await sharedWorkspace(page);
    expect(snapshot.themePresetId).toBeNull();
    expect(snapshot.themeRecipeJson).toBe(edited);
    expect(snapshot.themeRecipeLabel).toMatchObject({ source: "imported", derivedFrom: `preset:${id}` });
    await expect(panel.locator('[data-theme-choice="recipe:current"]')).toHaveAttribute("aria-pressed", "true");
    await panel.locator(`[data-theme-choice="preset:${id}"]`).click();
    await panel.getByRole("button", { name: "Copy JSON", exact: true }).click();
    expect(JSON.parse(await page.evaluate(() => navigator.clipboard.readText()))).toEqual(canonical);
    errors.assertNone();
  });
}
