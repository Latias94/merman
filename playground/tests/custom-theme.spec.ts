import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";
import type { ThemeRecipeV1 } from "@mermanjs/web";
import { encodeShareHash, decodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import {
  expectNoDocumentOverflow,
  monitorBrowserErrors,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";

const recipe = {
  schema_version: 1,
  kind: "design",
  design: {
    tokens: { text: { type: "color", label: "Text", value: "#172554" }, accent: { type: "color", label: "Accent", value: "#dc2626" } },
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

async function editor(page: Page) {
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  const workbench = page.locator("#theme-workbench");
  await expect(workbench).toBeVisible();
  return workbench;
}

async function replaceRecipe(page: Page, workbench: Locator, rawJson: string, label = "Theme recipe JSON") {
  await workbench.getByRole("tab", { name: "JSON", exact: true }).click();
  const input = workbench.getByRole("textbox", { name: label, exact: true });
  await expect(input).toBeVisible();
  await input.focus();
  await page.keyboard.press("ControlOrMeta+A");
  const notCancelled = await input.evaluate((element, value) => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/plain", value);
    return element.dispatchEvent(new ClipboardEvent("paste", {
      bubbles: true, cancelable: true, clipboardData: clipboard,
    }));
  }, rawJson);
  expect(notCancelled).toBe(false);
}

async function newRecipe(page: Page, workbench: Locator, value = JSON.stringify(recipe)) {
  await workbench.getByRole("button", { name: "New recipe", exact: true }).click();
  await replaceRecipe(page, workbench, value);
}

async function copyRecipe(page: Page, workbench: Locator): Promise<ThemeRecipeV1> {
  const button = workbench.getByRole("button", { name: "Copy JSON", exact: true });
  await expect(button).toBeEnabled();
  await button.click();
  return JSON.parse(await page.evaluate(() => navigator.clipboard.readText())) as ThemeRecipeV1;
}

async function svg(page: Page) {
  return page.locator(".preview-container > div").first().evaluate((element) =>
    element.shadowRoot?.querySelector("svg")?.outerHTML ?? "",
  );
}

test("theme workbench resizes with pointer and keyboard and remembers its desktop width", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await start(page);
  let workbench = await editor(page);
  const initialSvg = await svg(page);
  const separator = page.getByRole("separator", { name: "Resize theme workbench", exact: true });
  const before = (await workbench.boundingBox())!;
  const grip = (await separator.boundingBox())!;
  await page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2);
  await page.mouse.down();
  await page.mouse.move(grip.x - 120, grip.y + grip.height / 2, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await workbench.boundingBox())!.width).toBeGreaterThan(before.width + 80);
  const dragged = (await workbench.boundingBox())!.width;
  await separator.focus();
  await separator.press("ArrowLeft");
  await expect.poll(async () => (await workbench.boundingBox())!.width).toBeGreaterThan(dragged);
  const resized = (await workbench.boundingBox())!.width;
  expect(await svg(page)).toBe(initialSvg);
  await workbench.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.reload();
  await waitForPreviewSvg(page);
  workbench = await editor(page);
  await expect.poll(async () => Math.abs((await workbench.boundingBox())!.width - resized)).toBeLessThan(2);
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(separator).toHaveAttribute("aria-orientation", "horizontal");
  const narrow = (await workbench.boundingBox())!.height;
  await separator.focus();
  await separator.press("ArrowUp");
  await expect.poll(async () => (await workbench.boundingBox())!.height).toBeGreaterThan(narrow);
  await expectNoDocumentOverflow(page);
});

test("valid drafts preview, require Apply before artifact sharing, and reopen from a workspace link", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  const workbench = await editor(page);
  await newRecipe(page, workbench);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await expect(workbench).toBeVisible();
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Copy workspace link", exact: true })).toBeDisabled();
  await expect(page.getByRole("menu")).toContainText("Apply the theme draft before sharing or exporting the preview.");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: /^Copy Code$/i })).toBeEnabled();
  await expect(page.getByRole("menuitem", { name: "Copy SVG", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");

  const downloadPromise = page.waitForEvent("download");
  await workbench.getByRole("button", { name: "Download JSON", exact: true }).click();
  expect(JSON.parse(await readFile((await (await downloadPromise).path())!, "utf8"))).toEqual(recipe);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await expect(workbench).toBeVisible();
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  const url = await page.evaluate(() => navigator.clipboard.readText());
  expect(new URL(url).hash).toMatch(/^#s3:/);
  expect(JSON.parse(decodeShareHash(new URL(url).hash)!.themeRecipeJson!)).toEqual(recipe);
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  const restored = await editor(page);
  expect(await copyRecipe(page, restored)).toEqual(recipe);
  await expectNoDocumentOverflow(page);
  await page.screenshot({ path: test.info().outputPath("custom-theme-desktop.png") });
  errors.assertNone();
});

test("invalid JSON keeps the last valid preview while current source edits survive Cancel", async ({ page }) => {
  await start(page, "brutalist");
  const workbench = await editor(page);
  await newRecipe(page, workbench);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  const validPreview = await svg(page);
  for (const draft of [
    "{",
    '{"schema_version":1,"kind":"complete_spec","complete_spec":{"canvas":{"base":"#ff0000"},"canvas":{"base":"#0000ff"}}}',
    JSON.stringify({ ...recipe, schema_version: 99 }),
    JSON.stringify({ ...recipe, unknown: true }),
    JSON.stringify(recipe).replace('"value":"#dc2626"', '"value":"#dc2626","value":"#000000"'),
  ]) {
    await replaceRecipe(page, workbench, draft);
    await expect(workbench.getByRole("alert")).toBeVisible();
    await expect(workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ })).toBeDisabled();
    expect(await svg(page)).toBe(validPreview);
  }
  await replaceEditorSource(page, "flowchart LR\nEdited[Source edit survives] --> B");
  await expect.poll(() => svg(page)).toContain("Source edit survives");
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await workbench.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(workbench).toBeHidden();
  await expect(page.getByRole("button", { name: "Theme", exact: true })).toBeFocused();
  await expect.poll(() => svg(page)).toContain("Source edit survives");
  await expect.poll(() => svg(page)).not.toContain("#fef3c7");
  const reopened = await editor(page);
  await expect(reopened.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
});

test("Visual edits and JSON share one draft and applying does not depend on a valid diagram", async ({ page }) => {
  await start(page);
  const workbench = await editor(page);
  await newRecipe(page, workbench);
  await expect(workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ })).toBeEnabled();
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByRole("tab", { name: "Object styles", exact: true }).click();
  const target = workbench.getByLabel("Semantic object", { exact: true });
  await expect(target.locator("option[value='actor']")).toHaveCount(1);
  await expect(target.locator("option[value='cluster']")).toHaveCount(1);
  await workbench.getByRole("tab", { name: "Shared settings", exact: true }).click();
  await workbench.getByRole("button", { name: "Text", exact: true }).click();
  const text = workbench.getByRole("textbox", { name: /^text$/i });
  await text.fill("#224466");
  await text.press("Enter");
  const edited = await copyRecipe(page, workbench);
  expect(edited.kind).toBe("design");
  if (edited.kind === "design") expect(edited.design.tokens?.text.value).toBe("#224466");
  await replaceEditorSource(page, "flowchart LR\nA -->");
  await expect(page.locator('[data-merman-render-error="true"]')).toBeVisible();
  const downloadPromise = page.waitForEvent("download");
  await workbench.getByRole("button", { name: "Download JSON", exact: true }).click();
  expect(JSON.parse(await readFile((await (await downloadPromise).path())!, "utf8"))).toEqual(edited);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await expect(workbench).toBeVisible();
  await workbench.getByRole("button", { name: "Cancel", exact: true }).click();
  await replaceEditorSource(page, "flowchart LR\nA[Recovered] --> B");
  await expect.poll(() => svg(page)).toContain("#224466");
});

test("theme history spans Visual, invalid JSON and Apply without undoing source edits", async ({ page }) => {
  await start(page);
  const workbench = await editor(page);
  await newRecipe(page, workbench);
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByRole("tab", { name: "Shared settings", exact: true }).click();
  await workbench.getByRole("button", { name: "Text", exact: true }).click();
  const text = workbench.getByRole("textbox", { name: /^text$/i });
  await text.fill("#224466");
  await text.press("Enter");
  const edited = await copyRecipe(page, workbench);
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(recipe);
  await workbench.getByRole("button", { name: "Redo theme edit", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(edited);
  await replaceRecipe(page, workbench, "{");
  await expect(workbench.getByRole("alert")).toBeVisible();
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(edited);
  await workbench.getByRole("button", { name: "Redo theme edit", exact: true }).click();
  await expect(workbench.getByRole("alert")).toBeVisible();
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await replaceEditorSource(page, "flowchart LR\nA[Source survives history] --> B");
  const json = workbench.getByRole("textbox", { name: "Theme recipe JSON", exact: true });
  await json.focus();
  await json.press("ControlOrMeta+z");
  expect(await copyRecipe(page, workbench)).toEqual(recipe);
  await workbench.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect.poll(() => svg(page)).toContain("Source survives history");
  await expect.poll(() => svg(page)).toContain("#224466");
});

test("complete gradients and ordered background layers preserve history and JSON exchange", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  const workbench = await editor(page);
  const complete = { schema_version: 1, kind: "complete_spec", complete_spec: { canvas: { base: "#fef3c7" }, styles: [{ kind: "rule", target: "node", style: { fill: "#ffffff" } }] } } satisfies ThemeRecipeV1;
  await newRecipe(page, workbench, JSON.stringify(complete));
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  const expanded = await copyRecipe(page, workbench);
  expect(expanded.kind).toBe("complete_spec");
  if (expanded.kind !== "complete_spec") return;

  const base = workbench.getByRole("group", { name: "Canvas base", exact: true });
  await base.getByLabel("Canvas base: value mode", { exact: true }).selectOption("linear-gradient");
  await base.getByLabel("Angle (degrees)", { exact: true }).fill("45");
  await base.getByLabel("Angle (degrees)", { exact: true }).press("Enter");
  await base.getByRole("textbox", { name: "Color 1", exact: true }).fill("#112233");
  await base.getByRole("textbox", { name: "Color 1", exact: true }).press("Enter");
  await base.getByRole("button", { name: "Add color stop", exact: true }).click();
  await base.getByLabel("Position 2 (%)", { exact: true }).fill("25");
  await base.getByLabel("Position 2 (%)", { exact: true }).press("Enter");
  await base.locator("summary").click();
  await base.getByLabel("Gradient repetition", { exact: true }).selectOption("repeating");
  await base.getByLabel("Repeat period (px)", { exact: true }).fill("40");
  await base.getByLabel("Repeat period (px)", { exact: true }).press("Enter");

  await workbench.getByLabel("New layer paint", { exact: true }).selectOption("linear-gradient");
  await workbench.getByRole("button", { name: "Add layer", exact: true }).click();
  const layer = workbench.locator("[data-theme-layer-editor]");
  await layer.getByLabel("Layer opacity", { exact: true }).fill("0.35");
  await layer.getByLabel("Layer opacity", { exact: true }).press("Enter");
  await layer.getByLabel("Blend mode", { exact: true }).selectOption("multiply");
  await layer.getByLabel("Horizontal offset (px)", { exact: true }).fill("-4");
  await layer.getByLabel("Horizontal offset (px)", { exact: true }).press("Enter");
  await layer.getByRole("button", { name: "Duplicate layer", exact: true }).click();
  await layer.getByRole("textbox", { name: "Color 1", exact: true }).fill("#abcdef20");
  await layer.getByRole("textbox", { name: "Color 1", exact: true }).press("Enter");
  await layer.getByRole("button", { name: "Send layer backward", exact: true }).click();

  await workbench.getByLabel("New layer paint", { exact: true }).selectOption("radial-gradient");
  await workbench.getByRole("button", { name: "Add layer", exact: true }).click();
  await layer.getByLabel("Center X: unit", { exact: true }).selectOption("px");
  await layer.getByLabel("Gradient radius", { exact: true }).fill("75");
  await layer.getByLabel("Gradient radius", { exact: true }).press("Enter");
  await layer.getByRole("button", { name: "Remove layer", exact: true }).click();
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  const withRadial = await copyRecipe(page, workbench);
  expect(withRadial.kind === "complete_spec" && withRadial.complete_spec.canvas?.layers?.length).toBe(3);
  await workbench.getByRole("button", { name: "Redo theme edit", exact: true }).click();
  const edited = await copyRecipe(page, workbench);
  if (edited.kind !== "complete_spec") throw new Error("Expected a complete theme.");
  expect(edited.complete_spec.styles).toEqual(expanded.complete_spec.styles);
  expect(edited.complete_spec.canvas?.base).toEqual({
    kind: "linear-gradient", angle_degrees: 45, stops: [
      { offset: 0, color: "#112233" }, { offset: 0.25, color: "#112233" }, { offset: 1, color: "#2563eb" },
    ], repetition: { kind: "repeating", period_px: 40 },
  });
  expect(edited.complete_spec.canvas?.layers?.[0]).toMatchObject({ paint: { kind: "linear-gradient", stops: [{ color: "#abcdef20" }, { color: "#2563eb" }] }, opacity: 0.35, blend_mode: "multiply", offset_x: -4 });
  await expect.poll(() => svg(page)).toContain("#112233");
  await expect.poll(() => svg(page)).toContain("data-merman-theme-canvas-layer");
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await workbench.locator('input[type="file"]').setInputFiles({ name: "layered-theme.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(edited)) });
  expect(await copyRecipe(page, workbench)).toEqual(edited);
  errors.assertNone();
});

test("grid layers render on narrow screens, sliders undo as one action and imported pattern paint stays explicit", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await start(page);
  const workbench = await editor(page);
  await newRecipe(page, workbench, JSON.stringify({ schema_version: 1, kind: "complete_spec", complete_spec: { canvas: { base: "transparent", bleed: 8 } } }));
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByLabel("New layer paint", { exact: true }).selectOption("grid");
  await workbench.getByRole("button", { name: "Add layer", exact: true }).click();
  const before = await copyRecipe(page, workbench);
  if (before.kind !== "complete_spec") throw new Error("Expected a complete theme.");
  expect(before.complete_spec.canvas?.layers).toHaveLength(2);
  await expect.poll(() => page.locator(".preview-container [data-merman-theme-canvas-layer]").count()).toBe(2);
  const layer = workbench.locator("[data-theme-layer-editor]");
  const slider = layer.getByRole("slider", { name: "Adjust layer opacity", exact: true });
  await slider.focus();
  await slider.press("Home");
  for (let i = 0; i < 3; i++) await slider.press("ArrowRight");
  const after = await copyRecipe(page, workbench);
  if (after.kind !== "complete_spec") throw new Error("Expected a complete theme.");
  expect(after.complete_spec.canvas?.layers?.[0].opacity).toBe(0.03);
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(before);
  await workbench.getByRole("button", { name: "Redo theme edit", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(after);
  await layer.getByLabel("Color 1: color picker", { exact: true }).fill("#112233");
  const alphaEdit = await copyRecipe(page, workbench);
  if (alphaEdit.kind !== "complete_spec") throw new Error("Expected a complete theme.");
  const paint = alphaEdit.complete_spec.canvas?.layers?.[0].paint;
  expect(typeof paint === "object" && paint.kind === "linear-gradient" && paint.stops[0].color).toBe("#11223340");
  await expectNoDocumentOverflow(page);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  const imported = { ...alphaEdit, complete_spec: { ...alphaEdit.complete_spec, canvas: { ...alphaEdit.complete_spec.canvas, layers: [{ paint: { kind: "pattern", pattern: "grid", cell_width: 20, cell_height: 20, foreground: "#abcdef40" } }] } } };
  await workbench.locator('input[type="file"]').setInputFiles({ name: "pattern.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(imported)) });
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await expect(workbench).toContainText("current SVG renderer does not draw pattern paint");
  expect(await copyRecipe(page, workbench)).toEqual(imported);
  await layer.getByLabel("Pattern", { exact: true }).selectOption("dots");
  const patternEdit = await copyRecipe(page, workbench);
  expect(patternEdit.kind === "complete_spec" && patternEdit.complete_spec.canvas?.layers?.[0].paint).toEqual({ kind: "pattern", pattern: "dots", cell_width: 20, cell_height: 20, foreground: "#abcdef40" });
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("creating from Cyberpunk edits shared colors without losing gradient, effect or family bindings", async ({ page }) => {
  await start(page, "cyberpunk");
  const workbench = await editor(page);
  await workbench.getByRole("button", { name: "Create from Cyberpunk", exact: true }).click();
  const exported = await copyRecipe(page, workbench);
  expect(exported.kind).toBe("design");
  if (exported.kind !== "design") throw new Error("Expected an editable design.");
  expect(exported.design.canvas?.layers?.length).toBeGreaterThan(0);
  expect(Object.values(exported.design.tokens ?? {}).some((token) => token.type === "effect")).toBe(true);
  expect(exported.design.rules?.some((entry) => entry.family === "sequence")).toBe(true);
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByRole("button", { name: /^Core colors/ }).click();
  await expect(workbench.getByLabel("Setting group", { exact: true })).toHaveValue("group:Core colors");
  const cyan = workbench.locator('[data-theme-resource="neon-cyan"]');
  await workbench.getByRole("button", { name: exported.design.tokens!["neon-cyan"].label ?? "neon-cyan", exact: true }).click();
  await cyan.locator('input[type="color"]').fill("#37b24d");
  const edited = await copyRecipe(page, workbench);
  expect(edited).toEqual({
    ...exported,
    design: {
      ...exported.design,
      tokens: { ...exported.design.tokens, "neon-cyan": { ...exported.design.tokens!["neon-cyan"], value: "#37b24d" } },
    },
  });
  const downloadPromise = page.waitForEvent("download");
  await workbench.getByRole("button", { name: "Download JSON", exact: true }).click();
  const saved = await readFile((await (await downloadPromise).path())!, "utf8");
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await workbench.locator('input[type="file"]').setInputFiles({
    name: "cyberpunk-custom.json", mimeType: "application/json", buffer: Buffer.from(saved),
  });
  expect(await copyRecipe(page, workbench)).toEqual(edited);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await expect.poll(() => svg(page)).toContain("#37b24d");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy SVG", exact: true }).click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toContain("<svg");
  expect(copied).toContain("#37b24d");
  await workbench.getByRole("button", { name: "Cancel", exact: true }).click();
  const reopened = await editor(page);
  await reopened.getByRole("tab", { name: "Presets", exact: true }).click();
  await reopened.getByRole("button", { name: "Create from Cyberpunk", exact: true }).click();
  expect(await copyRecipe(page, reopened)).toEqual(exported);
});

test("canvas transparency keeps layers, removal clears them, and restoration preserves other edits", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page, "cyberpunk");
  const workbench = await editor(page);
  await workbench.getByRole("button", { name: "Customize", exact: true }).click();
  const original = await copyRecipe(page, workbench);
  expect(original.kind).toBe("design");
  if (original.kind !== "design") throw new Error("Expected a design.");
  await workbench.getByRole("button", { name: "Canvas background", exact: true }).click();
  const base = workbench.getByRole("group", { name: "Canvas base", exact: true });
  await base.getByLabel("Canvas base: Value source", { exact: true }).selectOption("local");
  await base.getByLabel("Paint type", { exact: true }).selectOption("transparent");
  const transparent = await copyRecipe(page, workbench);
  expect(transparent.kind).toBe("design");
  if (transparent.kind !== "design") throw new Error("Expected a design.");
  expect(transparent.design.canvas).toEqual({ ...original.design.canvas, base: "transparent" });
  await expect.poll(() => page.locator(".preview-container [data-merman-theme-canvas-layer]").count()).toBeGreaterThan(0);
  await workbench.getByRole("button", { name: "Remove canvas background", exact: true }).click();
  const removed = await copyRecipe(page, workbench);
  expect(removed.kind).toBe("design");
  if (removed.kind !== "design") throw new Error("Expected a design.");
  expect(removed.design.canvas?.base).toBe("transparent");
  expect(removed.design.canvas?.layers).toBeUndefined();
  expect(removed.design.styles).toEqual(original.design.styles);
  expect(removed.design.tokens).toEqual(original.design.tokens);
  await expect.poll(() => page.locator(".preview-container [data-merman-theme-canvas-layer]").count()).toBe(0);
  await workbench.getByRole("button", { name: "Restore canvas background", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(transparent);
  await expect.poll(() => page.locator(".preview-container [data-merman-theme-canvas-layer]").count()).toBeGreaterThan(0);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await workbench.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(workbench).toBeHidden();
  errors.assertNone();
});

test("design canvas omission and transparent background survive JSON exchange", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  const workbench = await editor(page);
  const omitted = { ...recipe, design: { ...recipe.design, canvas: {} } };
  await newRecipe(page, workbench, JSON.stringify(omitted));
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByRole("button", { name: "Canvas background", exact: true }).click();
  const base = workbench.getByRole("group", { name: "Canvas base", exact: true });
  const baseMode = base.getByLabel("Canvas base: Value source", { exact: true });
  await expect(baseMode).toHaveValue("omit");
  await baseMode.selectOption("local");
  await base.getByLabel("Paint type", { exact: true }).selectOption("transparent");
  const edited = await copyRecipe(page, workbench);
  expect(edited.kind).toBe("design");
  if (edited.kind === "design") expect(edited.design.canvas?.base).toBe("transparent");
  await baseMode.selectOption("omit");
  expect(await copyRecipe(page, workbench)).toEqual(omitted);
  await expect(baseMode).toHaveValue("omit");
  errors.assertNone();
});

test("custom named settings and independent hard shadows use the same designer and survive local copies", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  await replaceEditorSource(page, '---\nconfig:\n  flowchart:\n    look: classic\n---\nflowchart LR\nA[Start] --> B[Done]');
  const workbench = await editor(page);
  const source = { ...recipe, design: { ...recipe.design, styles: { node: { fill: "#ffffff" } } } } satisfies ThemeRecipeV1;
  await newRecipe(page, workbench, JSON.stringify(source));
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByRole("tab", { name: "Shared settings", exact: true }).click();
  await workbench.getByText("Add a shared value", { exact: true }).click();
  await workbench.getByRole("textbox", { name: "Setting name", exact: true }).fill("My paper shadow");
  await workbench.getByLabel("Setting type", { exact: true }).selectOption("effect");
  await workbench.getByRole("button", { name: "Create setting", exact: true }).click();
  const resource = workbench.locator('[data-theme-resource="custom-1"]');
  await expect(resource.getByLabel("Shadow composition", { exact: true })).toHaveValue("independent");
  const shadow = resource.locator("[data-theme-shadow-editor]").first();
  await shadow.getByRole("spinbutton", { name: "Horizontal offset", exact: true }).fill("8");
  await shadow.getByRole("spinbutton", { name: "Horizontal offset", exact: true }).press("Enter");
  await shadow.getByRole("spinbutton", { name: "Vertical offset", exact: true }).fill("6");
  await shadow.getByRole("spinbutton", { name: "Vertical offset", exact: true }).press("Enter");
  await expect(shadow.getByRole("spinbutton", { name: "Shadow softness (px)", exact: true })).toHaveValue("0");
  await resource.getByRole("button", { name: "Add shadow", exact: true }).click();
  await workbench.getByRole("tab", { name: "Object styles", exact: true }).click();
  await workbench.getByLabel("Semantic object", { exact: true }).selectOption("node");
  const style = workbench.locator('[data-design-value="/styles/node/fill"]').locator("..");
  await style.getByLabel("Add a property…", { exact: true }).selectOption("effect");
  const effect = workbench.locator('[data-design-value="/styles/node/effect"]');
  await effect.getByLabel("Shadow / effect: Value source", { exact: true }).selectOption("ref:custom-1");
  const linked = await copyRecipe(page, workbench);
  if (linked.kind !== "design") throw new Error("Expected a design.");
  expect(linked.design.styles?.node.effect).toEqual({ ref: "custom-1" });
  expect(linked.design.tokens?.["custom-1"].value).toMatchObject({ kind: "shadow-stack", composition: "independent", shadows: [{ offset_x: 8, offset_y: 6, std_deviation: 0 }, { std_deviation: 0 }] });
  await expect.poll(() => svg(page)).toContain('in="SourceAlpha"');
  await effect.getByRole("button", { name: "Use a local copy", exact: true }).click();
  const detached = await copyRecipe(page, workbench);
  if (detached.kind !== "design") throw new Error("Expected a design.");
  expect(detached.design.styles?.node.effect).toEqual(linked.design.tokens?.["custom-1"].value);
  expect(detached.design.tokens).toEqual(linked.design.tokens);
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(linked);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await workbench.locator('input[type="file"]').setInputFiles({ name: "my-theme.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(linked)) });
  expect(await copyRecipe(page, workbench)).toEqual(linked);
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  const importedResource = workbench.locator('[data-theme-resource="custom-1"]');
  await workbench.getByRole("tab", { name: "Shared settings", exact: true }).click();
  await workbench.getByRole("button", { name: "My paper shadow", exact: true }).click();
  await importedResource.getByText("Name, group and description", { exact: true }).click();
  await importedResource.getByRole("textbox", { name: "Reference ID", exact: true }).fill("paper-shadow");
  await importedResource.getByRole("textbox", { name: "Reference ID", exact: true }).press("Enter");
  const renamed = await copyRecipe(page, workbench);
  if (renamed.kind !== "design") throw new Error("Expected a design.");
  expect(renamed.design.tokens?.["custom-1"]).toBeUndefined();
  expect(renamed.design.styles?.node.effect).toEqual({ ref: "paper-shadow" });
  expect(renamed.design.tokens?.["paper-shadow"]).toEqual(linked.design.tokens?.["custom-1"]);
  errors.assertNone();
});

test("theme outcome describes the displayed SVG without certifying capability discovery", async ({ page }) => {
  await start(page);
  await replaceEditorSource(page, "classDiagram\nclass 用户");
  await page.getByRole("button", { name: "Render settings", exact: true }).click();
  await page.getByRole("menuitemradio", { name: "resvg-compatible SVG", exact: true }).click();
  const workbench = await editor(page);
  await newRecipe(page, workbench, JSON.stringify({
    schema_version: 1,
    kind: "complete_spec",
    complete_spec: { styles: [{
      kind: "rule", family: "class", target: "node", style: { stroke: { width: 2 } },
    }] },
  }));
  await expect.poll(() => svg(page)).toContain("用户");
  const outcome = page.getByTestId("theme-render-status");
  await outcome.locator("summary").click();
  await expect.poll(async () => {
    const text = await outcome.locator("pre").textContent();
    return text ? JSON.parse(text).family_id : null;
  }).toBe("class");
  const evidence = JSON.parse((await outcome.locator("pre").textContent())!);
  expect(evidence.version).toBe(1);
  expect(["residual", "incomplete"]).toContain(evidence.theme_status);
  expect(evidence.target_status).not.toBe("portable");
  await expect(outcome).toContainText("an empty diagnostic list does not certify portability");
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await expect(workbench).toContainText("does not infer per-field provenance");
  await workbench.getByRole("tab", { name: "Inspect", exact: true }).click();
  await expect(workbench).toContainText("Theme catalog details");
  await expect(workbench).toContainText("Catalog: 10 presets");
});

test("dirty drafts gate Compare and issue sharing and invalidate an open export target", async ({ page }) => {
  await start(page);
  let workbench = await editor(page);
  await newRecipe(page, workbench);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await expect(workbench).toBeVisible();
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…" }).click();
  const dialog = page.getByRole("dialog", { name: "Export image" });
  await expect(dialog).toBeVisible();
  await page.evaluate(() => {
    const button = [...document.querySelectorAll("#theme-workbench button")]
      .find((element) => element.textContent?.includes("New recipe"));
    if (!button) throw new Error("Theme workbench is not mounted behind export dialog.");
    (button as HTMLElement).click();
  });
  await expect(dialog).toBeHidden();
  await expect(workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ })).toBeEnabled();

  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  await expect(page.locator('[data-merman-compare-engine="merman"] button[aria-label="Copy SVG"]')).toBeDisabled();
  await expect(page.locator('[data-merman-compare-engine="merman"] button[aria-label="Export image"]')).toBeDisabled();
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Copy workspace link", exact: true })).toBeDisabled();
  await expect(page.getByRole("menuitem", { name: "Copy issue reproduction link", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await expect(page.locator('[data-merman-compare-engine="merman"] button[aria-label="Copy SVG"]')).toBeEnabled();
});

test("imports and applies a valid recipe while the current diagram is invalid", async ({ page }) => {
  await start(page);
  await replaceEditorSource(page, "flowchart LR\nA -->");
  await expect(page.locator('[data-merman-render-error="true"]')).toBeVisible();
  const workbench = await editor(page);
  await workbench.locator('input[type="file"]').setInputFiles({
    name: "valid-theme.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(recipe)),
  });
  await expect(workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ })).toBeEnabled();
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await expect(workbench).toBeVisible();
  await replaceEditorSource(page, "flowchart LR\nA[Recovered] --> B");
  await expect.poll(() => svg(page)).toContain("#fef3c7");
});

test("oversized files keep the current valid draft and replacing it requires an explicit choice", async ({ page }) => {
  await start(page);
  const workbench = await editor(page);
  await newRecipe(page, workbench);
  expect(await copyRecipe(page, workbench)).toEqual(recipe);
  await workbench.locator('input[type="file"]').setInputFiles({
    name: "too-large.json", mimeType: "application/json", buffer: Buffer.alloc(256 * 1024 + 1, 32),
  });
  await expect(workbench.getByRole("alert")).toContainText("256 KiB");
  expect(await copyRecipe(page, workbench)).toEqual(recipe);
  await workbench.getByRole("tab", { name: "Presets", exact: true }).click();
  await workbench.getByRole("button", { name: /^Brutalist/ }).click();
  const confirmation = page.getByRole("dialog");
  await confirmation.getByRole("button", { name: "Keep editing", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(recipe);
  await workbench.getByRole("button", { name: /^Brutalist/ }).click();
  await confirmation.getByRole("button", { name: "Discard and replace", exact: true }).click();
  await expect(workbench.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await expect.poll(() => svg(page)).not.toContain("#fef3c7");
});

test("late file reads cannot overwrite an edit or a reopened workbench", async ({ page }) => {
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
  let workbench = await editor(page);
  await newRecipe(page, workbench);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  const delayedFile = { name: "delayed.json", mimeType: "application/json", buffer: Buffer.from("{}") };
  await workbench.locator('input[type="file"]').setInputFiles(delayedFile);
  await expect(workbench.locator("[data-theme-draft-status]")).toContainText("Reading theme…");
  const changed = { ...recipe, design: { ...recipe.design, canvas: { base: "#ddffdd" } } };
  await replaceRecipe(page, workbench, JSON.stringify(changed));
  await page.evaluate(() => window.dispatchEvent(new Event("release-theme-import")));
  expect(await copyRecipe(page, workbench)).toEqual(changed);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await workbench.locator('input[type="file"]').setInputFiles(delayedFile);
  await expect(workbench.locator("[data-theme-draft-status]")).toContainText("Reading theme…");
  await workbench.getByRole("button", { name: "Cancel", exact: true }).click();
  workbench = await editor(page);
  await page.evaluate(() => window.dispatchEvent(new Event("release-theme-import")));
  expect(await copyRecipe(page, workbench)).toEqual(changed);
});

test("Chinese workbench keeps preview, JSON and theme actions reachable on a narrow screen", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 740 });
  await page.addInitScript(() => localStorage.setItem("merman-language", "zh"));
  await page.goto("./", { waitUntil: "domcontentloaded" });
  await expect(page.locator("footer")).toContainText("WASM: 就绪");
  await page.getByRole("button", { name: "主题", exact: true }).click();
  const workbench = page.locator("#theme-workbench");
  await expect(workbench).toBeVisible();
  await waitForPreviewSvg(page);
  const previewBounds = await page.locator(".preview-container").first().boundingBox();
  expect(previewBounds!.height).toBeGreaterThan(60);
  await workbench.getByRole("button", { name: "新建配方", exact: true }).click();
  await replaceRecipe(page, workbench, "{", "主题配方 JSON");
  await expect(workbench.getByRole("alert")).toBeVisible();
  await expect(workbench.getByRole("button", { name: /^(校验并应用|应用)$/ })).toBeDisabled();
  await expectNoDocumentOverflow(page);
  const bounds = await workbench.boundingBox();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(321);
  await replaceRecipe(page, workbench, JSON.stringify(recipe), "主题配方 JSON");
  await workbench.getByRole("button", { name: /^(校验并应用|应用)$/ }).click();
  await expect(workbench).toBeVisible();
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await workbench.getByRole("tab", { name: "可视化", exact: true }).click();
  const sections = workbench.getByRole("tablist", { name: "主题设计分区", exact: true });
  await sections.getByRole("tab", { name: "总览", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(sections.getByRole("tab", { name: "共享设置", exact: true })).toBeFocused();
  await page.keyboard.press("Enter");
  await workbench.getByRole("button", { name: "Text", exact: true }).click();
  await workbench.getByRole("button", { name: "编辑 /styles/node-label/fill/color", exact: true }).click();
  await expect(workbench.locator('[data-design-value="/styles/node-label/fill/color"] select').first()).toBeFocused();
  await workbench.getByRole("button", { name: "返回共享设置", exact: true }).click();
  const text = workbench.getByRole("textbox", { name: "Text", exact: true });
  await text.fill("invalid-color");
  await text.press("Tab");
  await expect(workbench.getByRole("button", { name: "复制 JSON", exact: true })).toBeDisabled();
  await expect(text).toBeVisible();
  await text.fill("#123456");
  await text.press("Tab");
  await expect.poll(() => svg(page)).toContain("#123456");
  await workbench.getByRole("button", { name: "撤销主题编辑", exact: true }).click();
  await expect(workbench.getByRole("button", { name: "复制 JSON", exact: true })).toBeDisabled();
  await workbench.getByRole("button", { name: "撤销主题编辑", exact: true }).click();
  await expect.poll(() => svg(page)).toContain("#172554");
  await expectNoDocumentOverflow(page);
  await page.getByRole("tab", { name: "编辑", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Mermaid 源代码", exact: true });
  await source.focus();
  await page.keyboard.press("ControlOrMeta+A");
  await source.evaluate((element) => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/plain", "flowchart LR\nUserEdit[Source survives navigation] --> B");
    element.dispatchEvent(new ClipboardEvent("paste", { bubbles: true, cancelable: true, clipboardData: clipboard }));
  });
  await page.getByRole("tab", { name: "预览", exact: true }).click();
  await expect.poll(() => svg(page)).toContain("Source survives navigation");
  await page.screenshot({ path: test.info().outputPath("custom-theme-mobile-zh.png") });
  await workbench.getByRole("button", { name: "取消", exact: true }).click();
  await expect(workbench).toBeHidden();
  await expect.poll(() => svg(page)).toContain("Source survives navigation");
});

test("visual gradient bounds and invalid color edits remain recoverable", async ({ page }) => {
  await start(page);
  const workbench = await editor(page);
  const document = { schema_version: 1, kind: "design", design: { canvas: { base: {
    kind: "linear-gradient", angle_degrees: 0,
    stops: [{ offset: 0, color: "#ffffff" }, { offset: 0.5, color: "#888888" }, { offset: 1, color: "#000000" }],
  } } } };
  await newRecipe(page, workbench, JSON.stringify(document));
  await expect(workbench.getByRole("button", { name: "Copy JSON", exact: true })).toBeEnabled();
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByRole("button", { name: "Canvas background", exact: true }).click();
  const offset = workbench.locator('[data-design-value="/canvas/base/stops/0/offset"]').getByRole("spinbutton");
  await expect(offset).toHaveAttribute("max", "0.5");
  await offset.fill("0.75");
  await offset.press("Tab");
  expect(await copyRecipe(page, workbench)).toEqual(document);
  const color = workbench.locator('[data-design-value="/canvas/base/stops/0/color"]').getByRole("textbox", { name: "Color 1", exact: true });
  await color.fill("not-a-color");
  await color.press("Tab");
  await expect(workbench.getByRole("button", { name: "Copy JSON", exact: true })).toBeDisabled();
  await expect(color).toBeVisible();
  await color.fill("#123456");
  await color.press("Tab");
  await expect(workbench.getByRole("button", { name: "Copy JSON", exact: true })).toBeEnabled();
  await expect.poll(() => svg(page)).toContain("#123456");
});

test("design sections and setting search navigate without changing the authored theme", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  const workbench = await editor(page);
  const imported = { ...recipe, design: { ...recipe.design, tokens: {
    ...recipe.design.tokens,
    "paper/ink~": { type: "color", label: "Paper ink", group: "My paper", description: "Quiet text", value: "#123456" },
  } } } satisfies ThemeRecipeV1;
  await newRecipe(page, workbench, JSON.stringify(imported));
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await expect(workbench.getByRole("tab", { name: "Overview", exact: true })).toBeVisible();
  const before = await copyRecipe(page, workbench);
  const beforeSvg = await svg(page);
  await workbench.getByRole("tab", { name: "Shared settings", exact: true }).click();
  await workbench.getByRole("searchbox", { name: "Search shared settings", exact: true }).fill("quiet");
  await workbench.getByRole("button", { name: "Paper ink", exact: true }).click();
  await expect(workbench.locator('[data-theme-resource="paper/ink~"]')).toBeVisible();
  await expect(workbench.getByRole("textbox", { name: "Paper ink", exact: true })).toHaveValue("#123456");
  await workbench.getByRole("tab", { name: "Object styles", exact: true }).click();
  await workbench.getByRole("tab", { name: "Overview", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(before);
  expect(await svg(page)).toEqual(beforeSvg);
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  expect(await copyRecipe(page, workbench)).not.toEqual(before);
  await workbench.getByRole("button", { name: "Redo theme edit", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(before);
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("declared consumers without visual controls reveal the exact JSON and return to their setting", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  const workbench = await editor(page);
  const imported = { ...recipe, design: {
    ...recipe.design,
    tokens: { ...recipe.design.tokens, series: { type: "palette", label: "Category palette", group: "My charts", value: ["#123456", "#abcdef"] } },
    palettes: { "chart-series": { ref: "series" } },
  } } satisfies ThemeRecipeV1;
  await newRecipe(page, workbench, JSON.stringify(imported));
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByRole("tab", { name: "Shared settings", exact: true }).click();
  await workbench.getByRole("button", { name: "Category palette", exact: true }).click();
  await workbench.getByRole("button", { name: "Edit /palettes/chart-series", exact: true }).click();
  await expect(workbench.getByRole("tab", { name: "JSON", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(workbench.locator("[data-theme-json-pointer]")).toHaveAttribute("data-theme-json-pointer", "/design/palettes/chart-series");
  await expect.poll(async () => {
    await page.keyboard.press("ControlOrMeta+C");
    try {
      return JSON.parse(await page.evaluate(() => navigator.clipboard.readText()));
    } catch {
      return null;
    }
  }).toEqual({ ref: "series" });
  await workbench.getByRole("button", { name: "Return to shared setting", exact: true }).click();
  await expect(workbench.locator('[data-theme-resource="series"]')).toBeVisible();
  await expect(workbench.locator('[data-theme-resource="series"]').locator("select").first()).toBeFocused();
  expect(await copyRecipe(page, workbench)).toEqual(imported);
  await workbench.getByRole("button", { name: "Edit /palettes/chart-series", exact: true }).click();
  await replaceRecipe(page, workbench, JSON.stringify(recipe));
  await expect(workbench.getByRole("button", { name: "Return to shared setting", exact: true })).toBeDisabled();
  await expect(workbench.locator("[data-theme-json-pointer]")).toHaveCount(0);
  await expect(workbench.getByRole("status")).toContainText("This location is no longer available");
  expect(await copyRecipe(page, workbench)).toEqual(recipe);
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  await workbench.getByRole("button", { name: "Return to shared setting", exact: true }).click();
  await expect(workbench.locator('[data-theme-resource="series"]').locator("select").first()).toBeFocused();
  expect(await copyRecipe(page, workbench)).toEqual(imported);
  errors.assertNone();
});


test("Cyberpunk bindings navigate aliases and downstream consumers before a precise local edit", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page, "cyberpunk");
  const workbench = await editor(page);
  await workbench.getByRole("button", { name: "Create from Cyberpunk", exact: true }).click();
  const original = await copyRecipe(page, workbench);
  if (original.kind !== "design") throw new Error("Expected a design.");
  const imported = { ...original, design: { ...original.design, tokens: {
    ...original.design.tokens,
    unrelated: { type: "color", label: "Unrelated equal color", value: "#00f2ff" },
  } } } satisfies ThemeRecipeV1;
  await replaceRecipe(page, workbench, JSON.stringify(imported));
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByRole("tab", { name: "Shared settings", exact: true }).click();
  await workbench.getByRole("button", { name: "Neon cyan", exact: true }).click();
  await expect(workbench.getByRole("button", { name: "Edit /tokens/text/value", exact: true })).toBeVisible();
  await expect(workbench.getByRole("button", { name: "Edit /styles/axis/fill/color", exact: true })).toBeVisible();
  await workbench.getByRole("button", { name: "Edit /tokens/text/value", exact: true }).click();
  await workbench.getByRole("button", { name: "Edit /styles/axis/fill/color", exact: true }).click();
  await expect(workbench.locator('[data-design-value="/styles/axis/fill/color"] select').first()).toBeFocused();
  await workbench.getByRole("button", { name: "Return to shared setting", exact: true }).click();
  await expect(workbench.locator('[data-theme-resource="neon-cyan"]')).toBeVisible();
  const pointer = "/canvas/layers/0/paint/stops/0/color";
  await workbench.getByRole("button", { name: `Edit ${pointer}`, exact: true }).click();
  const field = workbench.locator(`[data-design-value="${pointer}"]`);
  await expect(field.locator("select").first()).toBeFocused();
  expect(await copyRecipe(page, workbench)).toEqual(imported);
  await field.getByRole("button", { name: "Use a local copy", exact: true }).click();
  await field.getByRole("textbox", { name: "Color 1", exact: true }).fill("#123456");
  await field.getByRole("textbox", { name: "Color 1", exact: true }).press("Enter");
  const detached = await copyRecipe(page, workbench);
  const expected = JSON.parse(JSON.stringify(imported)) as typeof imported;
  const layer = expected.design.canvas!.layers![0];
  if (!("paint" in layer) || typeof layer.paint !== "object" || !("stops" in layer.paint)) throw new Error("Expected a gradient.");
  layer.paint.stops[0].color = "#123456";
  expect(detached).toEqual(expected);
  await workbench.getByRole("button", { name: "Return to shared setting", exact: true }).click();
  await expect(workbench.locator('[data-theme-resource="neon-cyan"]').locator("select").first()).toBeFocused();
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(imported);
  errors.assertNone();
});


test("imported effect bindings follow escaped IDs, rule reordering and metadata edits", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  const workbench = await editor(page);
  const imported = {
    schema_version: 1,
    kind: "design",
    design: {
      tokens: {
        "ink/~": { type: "color", label: "Paper ink", group: "My paper", value: "#123456" },
        "shade/~": { type: "effect", label: "Paper shade", group: "My paper", value: {
          kind: "shadow-stack", composition: "independent", shadows: [
            { color: { ref: "ink/~" }, offset_x: 4, offset_y: 4, std_deviation: 0, opacity: 0.5 },
          ],
        } },
      },
      rules: [
        { target: "node", style: { effect: { ref: "shade/~" } } },
        { target: "node", style: { fill: "#ffffff" } },
      ],
    },
  } satisfies ThemeRecipeV1;
  await newRecipe(page, workbench, JSON.stringify(imported));
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  await workbench.getByRole("button", { name: /^My paper/ }).click();
  await workbench.getByRole("button", { name: "Paper ink", exact: true }).click();
  const nested = "/tokens/shade~1~0/value/shadows/0/color";
  await workbench.getByRole("button", { name: `Edit ${nested}`, exact: true }).click();
  await expect(workbench.locator(`[data-design-value="${nested}"] select`).first()).toBeFocused();
  await workbench.getByRole("button", { name: "Return to shared setting", exact: true }).click();
  await workbench.getByRole("button", { name: "Edit /rules/0/style/effect", exact: true }).click();
  await expect(workbench.locator('[data-design-value="/rules/0/style/effect"] select').first()).toBeFocused();
  await workbench.getByRole("button", { name: "Move down", exact: true }).click();
  const reordered = await copyRecipe(page, workbench);
  if (reordered.kind !== "design") throw new Error("Expected a design.");
  expect(reordered.design.rules?.[1]).toEqual(imported.design.rules[0]);
  await workbench.getByRole("button", { name: "Return to shared setting", exact: true }).click();
  await expect(workbench.getByRole("button", { name: "Edit /rules/0/style/effect", exact: true })).toHaveCount(0);
  await workbench.getByRole("button", { name: "Edit /rules/1/style/effect", exact: true }).click();
  await expect(workbench.locator('[data-design-value="/rules/1/style/effect"] select').first()).toBeFocused();
  await workbench.getByRole("button", { name: "Undo theme edit", exact: true }).click();
  await workbench.getByRole("button", { name: "Return to shared setting", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(imported);
  const resource = workbench.locator('[data-theme-resource="ink/~"]');
  await expect(resource.getByRole("button", { name: "Remove shared value", exact: true })).toBeDisabled();
  await resource.getByText("Name, group and description", { exact: true }).click();
  await resource.getByRole("textbox", { name: "Group", exact: true }).fill("Reframed paper");
  await resource.getByRole("textbox", { name: "Group", exact: true }).press("Enter");
  await resource.getByRole("textbox", { name: "Reference ID", exact: true }).fill("renamed/~");
  await resource.getByRole("textbox", { name: "Reference ID", exact: true }).press("Enter");
  const renamed = await copyRecipe(page, workbench);
  if (renamed.kind !== "design") throw new Error("Expected a design.");
  expect(renamed.design.tokens?.["renamed/~"].group).toBe("Reframed paper");
  expect(renamed.design.tokens?.["shade/~"].value).toMatchObject({ shadows: [{ color: { ref: "renamed/~" } }] });
  await expect(workbench.locator('[data-theme-resource="renamed/~"]')).toBeVisible();
  await expect(workbench.getByRole("button", { name: `Edit ${nested}`, exact: true })).toBeVisible();
  errors.assertNone();
});
