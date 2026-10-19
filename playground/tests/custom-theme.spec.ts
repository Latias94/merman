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
  kind: "definition",
  definition: {
    authoring_schema_version: 1,
    expansion_version: 1,
    tokens: { canvas: "#fef3c7", text: "#172554", accent: "#dc2626" },
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
    JSON.stringify(recipe).replace('"#dc2626"', '"#dc2626","accent":"#000000"'),
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
  const target = workbench.getByLabel("Semantic target", { exact: true });
  await expect(target.locator("option[value='actor']")).toHaveCount(1);
  await expect(target.locator("option[value='cluster']")).toHaveCount(1);
  const text = workbench.getByRole("textbox", { name: /^text$/i });
  await text.fill("#224466");
  await text.press("Enter");
  const edited = await copyRecipe(page, workbench);
  expect(edited.kind).toBe("definition");
  if (edited.kind === "definition") expect(edited.definition.tokens.text).toBe("#224466");
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

test("customizing Cyberpunk preserves its complete canvas, effects, assets and ordered family rules", async ({ page }) => {
  await start(page, "cyberpunk");
  const workbench = await editor(page);
  await workbench.getByRole("button", { name: "Customize", exact: true }).click();
  const exported = await copyRecipe(page, workbench);
  expect(exported.kind).toBe("complete_spec");
  if (exported.kind !== "complete_spec") return;
  expect(exported.complete_spec.canvas?.layers?.length).toBeGreaterThan(0);
  expect(exported.complete_spec.effects?.length).toBeGreaterThan(0);
  expect(exported.complete_spec.styles?.some((entry) => entry.kind === "rule" && entry.family === "sequence")).toBe(true);
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  const base = workbench.getByRole("textbox", { name: "Canvas base", exact: true });
  await base.fill("#fef3c7");
  await base.press("Enter");
  await workbench.getByText("Append an override", { exact: true }).click();
  await workbench.getByLabel("Family scope", { exact: true }).selectOption("family:class");
  await workbench.getByLabel("Semantic target", { exact: true }).selectOption("node");
  await workbench.getByRole("button", { name: "Append rule", exact: true }).click();
  await workbench.getByLabel("Border / line paint: value mode", { exact: true }).selectOption("color");
  const border = workbench.getByRole("textbox", { name: "Border / line paint", exact: true });
  await border.fill("#ff00ff");
  await border.press("Enter");
  const edited = await copyRecipe(page, workbench);
  expect(edited).toEqual({
    ...exported,
    complete_spec: {
      ...exported.complete_spec,
      canvas: { ...exported.complete_spec.canvas, base: "#fef3c7" },
      styles: [...exported.complete_spec.styles ?? [], {
        kind: "rule", family: "class", target: "node", style: { stroke: { paint: "#ff00ff" } },
      }],
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
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy SVG", exact: true }).click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toContain("<svg");
  expect(copied).toContain("#fef3c7");
});

test("canvas transparency keeps layers, removal clears them, and restoration preserves other edits", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page, "cyberpunk");
  const workbench = await editor(page);
  await workbench.getByRole("button", { name: "Customize", exact: true }).click();
  const original = await copyRecipe(page, workbench);
  expect(original.kind).toBe("complete_spec");
  if (original.kind !== "complete_spec") return;
  await workbench.getByLabel("Canvas base: value mode", { exact: true }).selectOption("transparent");
  const transparent = await copyRecipe(page, workbench);
  expect(transparent.kind).toBe("complete_spec");
  if (transparent.kind !== "complete_spec") return;
  expect(transparent.complete_spec.canvas).toEqual({ ...original.complete_spec.canvas, base: "transparent" });
  await expect.poll(() => page.locator(".preview-container [data-merman-theme-canvas-layer]").count()).toBeGreaterThan(0);
  await workbench.getByRole("button", { name: "Remove canvas background", exact: true }).click();
  const removed = await copyRecipe(page, workbench);
  expect(removed.kind).toBe("complete_spec");
  if (removed.kind !== "complete_spec") return;
  expect(removed.complete_spec.canvas?.base).toBe("transparent");
  expect(removed.complete_spec.canvas?.layers).toBeUndefined();
  expect(removed.complete_spec.styles).toEqual(original.complete_spec.styles);
  expect(removed.complete_spec.effects).toEqual(original.complete_spec.effects);
  await expect.poll(() => page.locator(".preview-container [data-merman-theme-canvas-layer]").count()).toBe(0);
  await workbench.getByRole("button", { name: "Restore canvas background", exact: true }).click();
  expect(await copyRecipe(page, workbench)).toEqual(transparent);
  await expect.poll(() => page.locator(".preview-container [data-merman-theme-canvas-layer]").count()).toBeGreaterThan(0);
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await workbench.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(workbench).toBeHidden();
  errors.assertNone();
});

test("definition canvas defaults come from Rust and transparent background survives JSON exchange", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  const workbench = await editor(page);
  await newRecipe(page, workbench, JSON.stringify({ ...recipe, definition: { ...recipe.definition, tokens: { text: "#172554" } } }));
  await workbench.getByRole("tab", { name: "Visual", exact: true }).click();
  const baseMode = workbench.getByLabel("Canvas base: value mode", { exact: true });
  await expect(baseMode.locator("option:checked")).toContainText("#ffffff");
  await baseMode.selectOption("transparent");
  const edited = await copyRecipe(page, workbench);
  expect(edited.kind).toBe("definition");
  if (edited.kind === "definition") expect(edited.definition.tokens.canvas).toBe("transparent");
  await baseMode.selectOption("omit");
  expect(await copyRecipe(page, workbench)).toEqual({ ...recipe, definition: { ...recipe.definition, tokens: { text: "#172554" } } });
  await expect(baseMode.locator("option:checked")).toContainText("#ffffff");
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
  const changed = { ...recipe, definition: { ...recipe.definition, tokens: { ...recipe.definition.tokens, canvas: "#ddffdd" } } };
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
  await page.screenshot({ path: test.info().outputPath("custom-theme-mobile-zh.png") });
  await workbench.getByRole("button", { name: "取消", exact: true }).click();
  await expect(workbench).toBeHidden();
});
