import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";
import type { ThemeRecipeV1 } from "@mermanjs/web";
import { encodeShareHash, decodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import {
  expectNoDocumentOverflow,
  monitorBrowserErrors,
  replaceEditorSource,
  replaceMermaidConfig,
  waitForPreviewSvg,
} from "./helpers/playground";

const recipe = {
  schema_version: 1,
  kind: "design",
  design: {
    tokens: { text: { type: "color", label: "Text", value: "#172554" } },
    canvas: { base: "#fef3c7" },
    styles: { "node-label": { fill: { kind: "solid", color: { ref: "text" } } } },
  },
} satisfies ThemeRecipeV1;

async function start(page: Page, themePresetId: string | null = null) {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
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

async function openPanel(page: Page) {
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  const panel = page.getByRole("region", { name: "Theme panel", exact: true });
  await expect(panel).toHaveAttribute("id", "theme-panel");
  await expect(panel).toBeVisible();
  return panel;
}

async function importRecipe(panel: Locator, raw = JSON.stringify(recipe), name = "theme.json") {
  await panel.getByLabel("Import JSON", { exact: true }).setInputFiles({
    name, mimeType: "application/json", buffer: Buffer.from(raw),
  });
}

async function openAdvanced(panel: Locator, name = "Advanced") {
  const summary = panel.locator("summary").filter({ hasText: new RegExp(`^${name}$`) });
  if (await summary.locator("..").getAttribute("open") === null) {
    await summary.click();
  }
}

async function copyRecipe(page: Page, panel: Locator): Promise<string> {
  await openAdvanced(panel);
  const copy = panel.getByRole("button", { name: "Copy JSON", exact: true });
  await expect(copy).toBeEnabled();
  await copy.click();
  return page.evaluate(async () => (await navigator.clipboard.readText()).replace(/\r\n/g, "\n"));
}

async function svg(page: Page) {
  return page.locator(".preview-container > div").first().evaluate((element) =>
    element.shadowRoot?.querySelector("svg")?.outerHTML ?? "",
  );
}

test("theme panel offers selection and import with read-only JSON in Advanced", async ({ page }) => {
  await start(page);
  const panel = await openPanel(page);
  await expect(panel.getByLabel("Mermaid theme", { exact: true })).toBeVisible();
  await expect(panel.getByRole("button", { name: /^None/ })).toBeVisible();
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toBeVisible();
  await expect(panel.getByRole("textbox", { name: "Theme JSON", exact: true })).toBeHidden();
  await expect(panel.getByRole("button", { name: /^(New recipe|Customize|Create from)/ })).toHaveCount(0);
  await expect(panel.getByRole("tab")).toHaveCount(0);

  const authored = `${JSON.stringify(recipe, null, 2)}\n`;
  await importRecipe(panel, authored);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await openAdvanced(panel);
  const json = panel.getByRole("textbox", { name: "Theme JSON", exact: true });
  await expect(json).toHaveAttribute("readonly", "");
  await expect(json).toHaveValue(authored);
  expect(await copyRecipe(page, panel)).toBe(authored);
});

test("theme panel resizes with pointer and keyboard and remembers desktop width", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await start(page);
  let panel = await openPanel(page);
  const initialSvg = await svg(page);
  const separator = page.getByRole("separator", { name: "Resize theme panel", exact: true });
  const before = (await panel.boundingBox())!;
  const grip = (await separator.boundingBox())!;
  await page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2);
  await page.mouse.down();
  await page.mouse.move(grip.x - 120, grip.y + grip.height / 2, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await panel.boundingBox())!.width).toBeGreaterThan(before.width + 80);
  const dragged = (await panel.boundingBox())!.width;
  await separator.focus();
  await separator.press("ArrowLeft");
  await expect.poll(async () => (await panel.boundingBox())!.width).toBeGreaterThan(dragged);
  const resized = (await panel.boundingBox())!.width;
  expect(await svg(page)).toBe(initialSvg);
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.reload();
  await waitForPreviewSvg(page);
  panel = await openPanel(page);
  await expect.poll(async () => Math.abs((await panel.boundingBox())!.width - resized)).toBeLessThan(2);
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(separator).toHaveAttribute("aria-orientation", "horizontal");
  const narrow = (await panel.boundingBox())!.height;
  await separator.focus();
  await separator.press("ArrowUp");
  await expect.poll(async () => (await panel.boundingBox())!.height).toBeGreaterThan(narrow);
  await expectNoDocumentOverflow(page);
});

test("imported themes preview before Apply and share as an applied workspace recipe", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  const panel = await openPanel(page);
  const authored = `${JSON.stringify(recipe, null, 2)}\n`;
  await importRecipe(panel, authored);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Copy workspace link", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: /^Copy Code$/i })).toBeEnabled();
  await expect(page.getByRole("menuitem", { name: "Copy SVG", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");

  const download = page.waitForEvent("download");
  await panel.getByRole("button", { name: "Download JSON", exact: true }).click();
  expect(await readFile((await (await download).path())!, "utf8")).toBe(authored);
  await panel.getByRole("button", { name: "Apply", exact: true }).click();
  await expect(panel).toBeVisible();
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  const url = await page.evaluate(() => navigator.clipboard.readText());
  expect(new URL(url).hash).toMatch(/^#s3:/);
  expect(decodeShareHash(new URL(url).hash)!.themeRecipeJson).toBe(authored);
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  expect(await copyRecipe(page, await openPanel(page))).toBe(authored);
  errors.assertNone();
});

test("invalid imports keep the last valid preview and Cancel preserves current source", async ({ page }) => {
  await start(page, "brutalist");
  const panel = await openPanel(page);
  await importRecipe(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  const validPreview = await svg(page);
  for (const invalid of [
    "{",
    '{"schema_version":1,"kind":"complete_spec","complete_spec":{"canvas":{"base":"#ff0000"},"canvas":{"base":"#0000ff"}}}',
    JSON.stringify({ ...recipe, schema_version: 99 }),
    JSON.stringify({ ...recipe, unknown: true }),
    JSON.stringify(recipe).replace('"value":"#172554"', '"value":"#172554","value":"#000000"'),
  ]) {
    await importRecipe(panel, invalid);
    await expect(panel.getByRole("alert")).toBeVisible();
    await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeDisabled();
    await expect(panel.getByRole("button", { name: "Download JSON", exact: true })).toBeDisabled();
    expect(await svg(page)).toBe(validPreview);
  }
  await replaceEditorSource(page, "flowchart LR\nEdited[Source edit survives] --> B");
  await expect.poll(() => svg(page)).toContain("Source edit survives");
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(panel).toBeHidden();
  await expect(page.getByRole("button", { name: "Theme", exact: true })).toBeFocused();
  await expect.poll(() => svg(page)).toContain("Source edit survives");
  await expect.poll(() => svg(page)).not.toContain("#fef3c7");
  await expect((await openPanel(page)).getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
});

test("presets export standalone JSON and replace an imported selection directly", async ({ page }) => {
  await start(page);
  const panel = await openPanel(page);
  await importRecipe(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  const cyberpunk = panel.getByRole("button", { name: /^Cyberpunk/ });
  await cyberpunk.click();
  await expect(cyberpunk).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const exported = await copyRecipe(page, panel);
  const parsed = JSON.parse(exported) as ThemeRecipeV1;
  expect(parsed.schema_version).toBe(1);
  expect(parsed.kind).toBe("design");
  if (parsed.kind !== "design") throw new Error("Expected the exported preset design.");
  expect(parsed.design.canvas?.layers?.length).toBeGreaterThan(0);
  expect(parsed.design.rules?.some((entry) => entry.family === "sequence")).toBe(true);
  expect(Object.values(parsed.design.tokens ?? {}).some((entry) => entry.type === "effect")).toBe(true);
  const download = page.waitForEvent("download");
  await panel.getByRole("button", { name: "Download JSON", exact: true }).click();
  expect(JSON.parse(await readFile((await (await download).path())!, "utf8"))).toEqual(parsed);
  await importRecipe(panel, exported, "cyberpunk-export.json");
  expect(JSON.parse(await copyRecipe(page, panel))).toEqual(parsed);
  await panel.getByRole("button", { name: "Apply", exact: true }).click();
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  expect(JSON.parse(await copyRecipe(page, await openPanel(page)))).toEqual(parsed);
});

test("Mermaid theme is shared immediately and survives cancelling a Merman selection", async ({ page }) => {
  await start(page, "brutalist");
  const panel = await openPanel(page);
  await panel.getByLabel("Mermaid theme", { exact: true }).selectOption("forest");
  const config = '{"flowchart":{"look":"classic"}}';
  await replaceMermaidConfig(page, config);
  await page.getByRole("button", { name: "Render settings", exact: true }).click();
  await page.getByRole("menuitemradio", { name: "Arial", exact: true }).click();
  await importRecipe(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  const reopened = await openPanel(page);
  await expect(reopened.getByLabel("Mermaid theme", { exact: true })).toHaveValue("forest");
  await expect(reopened.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  await reopened.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  const url = await page.evaluate(() => navigator.clipboard.readText());
  const saved = decodeShareHash(new URL(url).hash)!;
  expect(saved.diagramTheme).toBe("forest");
  expect(saved.mermaidConfig).toBe(config);
  expect(saved.diagramFont).toBe("arial");
  expect(saved.themePresetId).toBe("brutalist");
  expect(saved.themeRecipeJson).toBeNull();
});

test("runtime diagnostics describe the displayed execution inside Advanced", async ({ page }) => {
  await start(page);
  await replaceEditorSource(page, "classDiagram\nclass 用户");
  await page.getByRole("button", { name: "Render settings", exact: true }).click();
  await page.getByRole("menuitemradio", { name: "resvg-compatible SVG", exact: true }).click();
  const panel = await openPanel(page);
  await importRecipe(panel, JSON.stringify({
    schema_version: 1,
    kind: "complete_spec",
    complete_spec: { styles: [{ kind: "rule", family: "class", target: "node", style: { stroke: { width: 2 } } }] },
  }));
  await expect.poll(() => svg(page)).toContain("用户");
  await openAdvanced(panel);
  await expect(panel).toContainText("This render's theme outcome");
  await expect(panel).toContainText("an empty diagnostic list does not certify portability");
  const raw = panel.getByTestId("theme-execution-metadata");
  await expect.poll(async () => {
    const text = await raw.textContent();
    return text ? JSON.parse(text).theme_execution_evidence?.family_id : null;
  }).toBe("class");
  const evidence = JSON.parse((await raw.textContent())!).theme_execution_evidence;
  expect(evidence.version).toBe(1);
  expect(["residual", "incomplete"]).toContain(evidence.theme_status);
  expect(evidence.target_status).not.toBe("portable");
});

test("pending selection gates Compare and issue sharing and invalidates an open export target", async ({ page }) => {
  await start(page);
  const panel = await openPanel(page);
  await importRecipe(panel);
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeEnabled();
  await panel.getByRole("button", { name: "Apply", exact: true }).click();
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…" }).click();
  const dialog = page.getByRole("dialog", { name: "Export image" });
  await expect(dialog).toBeVisible();
  await page.locator('#theme-panel input[type="file"]').setInputFiles({
    name: "changed.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify({
      ...recipe, design: { ...recipe.design, canvas: { base: "#ddffdd" } },
    })),
  });
  await expect(dialog).toBeHidden();
  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  const merman = page.locator('[data-merman-compare-engine="merman"]');
  await expect(merman.getByRole("button", { name: "Copy SVG", exact: true })).toBeDisabled();
  await expect(merman.getByRole("button", { name: "Export image", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Copy workspace link", exact: true })).toBeDisabled();
  await expect(page.getByRole("menuitem", { name: "Copy issue reproduction link", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");
  await panel.getByRole("button", { name: "Apply", exact: true }).click();
  await expect(merman.getByRole("button", { name: "Copy SVG", exact: true })).toBeEnabled();
});

test("valid theme import and export do not require a valid diagram", async ({ page }) => {
  await start(page);
  await replaceEditorSource(page, "flowchart LR\nA -->");
  await expect(page.locator('[data-merman-render-error="true"]')).toBeVisible();
  const panel = await openPanel(page);
  await importRecipe(panel);
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeEnabled();
  const download = page.waitForEvent("download");
  await panel.getByRole("button", { name: "Download JSON", exact: true }).click();
  expect(JSON.parse(await readFile((await (await download).path())!, "utf8"))).toEqual(recipe);
  await panel.getByRole("button", { name: "Apply", exact: true }).click();
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  await replaceEditorSource(page, "flowchart LR\nA[Recovered] --> B");
  await expect.poll(() => svg(page)).toContain("#fef3c7");
});

test("oversized import leaves the valid selection intact and can be replaced", async ({ page }) => {
  await start(page);
  const panel = await openPanel(page);
  await importRecipe(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await panel.getByLabel("Import JSON", { exact: true }).setInputFiles({
    name: "too-large.json", mimeType: "application/json", buffer: Buffer.alloc(256 * 1024 + 1, 32),
  });
  await expect(panel.getByRole("alert")).toContainText("256 KiB");
  expect(JSON.parse(await copyRecipe(page, panel))).toEqual(recipe);
  const changed = { ...recipe, design: { ...recipe.design, canvas: { base: "#ddffdd" } } };
  await importRecipe(panel, JSON.stringify(changed));
  await expect(panel.getByRole("alert")).toHaveCount(0);
  await expect.poll(() => svg(page)).toContain("#ddffdd");
  expect(JSON.parse(await copyRecipe(page, panel))).toEqual(changed);
});

test("late file reads cannot overwrite a newer import or a reopened panel", async ({ page }) => {
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
  let panel = await openPanel(page);
  await importRecipe(panel);
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeEnabled();
  await panel.getByRole("button", { name: "Apply", exact: true }).click();
  await importRecipe(panel, "{}", "delayed.json");
  await expect(panel.locator("[data-theme-draft-status]")).toContainText("Reading theme…");
  const changed = JSON.stringify({ ...recipe, design: { ...recipe.design, canvas: { base: "#ddffdd" } } });
  await importRecipe(panel, changed);
  await expect.poll(() => svg(page)).toContain("#ddffdd");
  await page.evaluate(() => window.dispatchEvent(new Event("release-theme-import")));
  expect(await copyRecipe(page, panel)).toBe(changed);
  await panel.getByRole("button", { name: "Apply", exact: true }).click();
  await importRecipe(panel, "{}", "delayed.json");
  await expect(panel.locator("[data-theme-draft-status]")).toContainText("Reading theme…");
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  panel = await openPanel(page);
  await page.evaluate(() => window.dispatchEvent(new Event("release-theme-import")));
  expect(await copyRecipe(page, panel)).toBe(changed);
});

test("Chinese theme panel keeps import, preview and advanced JSON reachable on a narrow screen", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 740 });
  await page.addInitScript(() => localStorage.setItem("merman-language", "zh"));
  await page.goto("./", { waitUntil: "domcontentloaded" });
  await expect(page.locator("footer")).toContainText("WASM: 就绪");
  await page.getByRole("button", { name: "主题", exact: true }).click();
  const panel = page.getByRole("region", { name: "主题面板", exact: true });
  await expect(panel).toBeVisible();
  await waitForPreviewSvg(page);
  const previewBounds = await page.locator(".preview-container").first().boundingBox();
  expect(previewBounds!.height).toBeGreaterThan(60);
  const input = panel.getByLabel("导入 JSON", { exact: true });
  await input.setInputFiles({ name: "invalid.json", mimeType: "application/json", buffer: Buffer.from("{") });
  await expect(panel.getByRole("alert")).toBeVisible();
  await expect(panel.getByRole("button", { name: "应用", exact: true })).toBeDisabled();
  await input.setInputFiles({ name: "theme.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(recipe)) });
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await openAdvanced(panel, "高级");
  await expect(panel.getByRole("textbox", { name: "主题 JSON", exact: true })).toHaveAttribute("readonly", "");
  await expect(panel.getByRole("button", { name: "复制 JSON", exact: true })).toBeVisible();
  const separator = page.getByRole("separator", { name: "调整主题面板大小", exact: true });
  await separator.focus();
  await separator.press("ArrowUp");
  await panel.getByRole("button", { name: "应用", exact: true }).click();
  await expectNoDocumentOverflow(page);
  const bounds = (await panel.boundingBox())!;
  expect(bounds.x).toBeGreaterThanOrEqual(0);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(321);
  await page.screenshot({ path: test.info().outputPath("theme-panel-mobile-zh.png") });
  await panel.getByRole("button", { name: "取消", exact: true }).click();
  await expect(panel).toBeHidden();
  await expect.poll(() => svg(page)).toContain("#fef3c7");
});
