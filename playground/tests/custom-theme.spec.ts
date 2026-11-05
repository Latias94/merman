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
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const panel = page.getByRole("region", { name: "Appearance", exact: true });
  await expect(panel).toHaveAttribute("id", "theme-panel");
  await expect(panel).toBeVisible();
  return panel;
}

async function importRecipe(panel: Locator, raw = JSON.stringify(recipe), name = "theme.json") {
  await panel.getByLabel("Import JSON", { exact: true }).setInputFiles({
    name, mimeType: "application/json", buffer: Buffer.from(raw),
  });
}

async function useImportedTheme(panel: Locator) {
  const useTheme = panel.getByRole("button", { name: "Use imported theme", exact: true });
  await expect(useTheme).toBeEnabled();
  await useTheme.click();
}

test("keyboard adoption and discard move focus to a persistent control before Escape", async ({ page }) => {
  await start(page);
  let panel = await openPanel(page);
  await importRecipe(panel);
  const adopt = panel.getByRole("button", { name: "Use imported theme", exact: true });
  await adopt.focus();
  await adopt.press("Enter");
  await expect(panel.getByTestId("theme-import-candidate")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Appearance", exact: true })).toBeFocused();
  panel = await openPanel(page);
  await importRecipe(panel);
  const discard = panel.getByRole("button", { name: "Discard import", exact: true });
  await discard.focus();
  await discard.press("Enter");
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
});

async function copyWorkspaceLink(page: Page) {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  return page.evaluate(() => navigator.clipboard.readText());
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

test("Appearance offers one theme directory and read-only JSON for the current theme", async ({ page }) => {
  await start(page);
  const panel = await openPanel(page);
  await expect(panel.getByRole("button", { name: /^Dark/ })).toBeVisible();
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toBeVisible();
  await expect(panel.getByRole("button", { name: /^Atlas/ })).toBeVisible();
  await expect(panel.getByTestId("theme-appearance-overrides")).toHaveCount(0);
  await expect(panel.getByRole("textbox", { name: "Theme JSON", exact: true })).toBeHidden();
  await expect(panel.getByRole("button", { name: /^(New recipe|Customize|Create from)/ })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: /^(Apply|Cancel)$/ })).toHaveCount(0);

  const authored = `${JSON.stringify(recipe, null, 2)}\n`;
  const currentSvg = await svg(page);
  await importRecipe(panel, authored);
  await expect(panel.getByRole("button", { name: "Use imported theme", exact: true })).toBeEnabled();
  expect(await svg(page)).toBe(currentSvg);
  await useImportedTheme(panel);
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
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
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

test("imported themes change the diagram only after confirmation and round-trip through a workspace link", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  const panel = await openPanel(page);
  const authored = `${JSON.stringify(recipe, null, 2)}\n`;
  const currentSvg = await svg(page);
  await importRecipe(panel, authored);
  await expect(panel.getByRole("button", { name: "Use imported theme", exact: true })).toBeEnabled();
  expect(await svg(page)).toBe(currentSvg);
  expect(decodeShareHash(new URL(await copyWorkspaceLink(page)).hash)!.themeRecipeJson).toBeNull();
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Copy SVG", exact: true })).toBeEnabled();
  await page.keyboard.press("Escape");

  await useImportedTheme(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  const download = page.waitForEvent("download");
  await panel.getByRole("button", { name: "Download JSON", exact: true }).click();
  expect(await readFile((await (await download).path())!, "utf8")).toBe(authored);
  await expect(panel).toBeVisible();
  const url = await copyWorkspaceLink(page);
  expect(new URL(url).hash).toMatch(/^#s4:/);
  const shared = decodeShareHash(new URL(url).hash)!;
  expect(shared.themeRecipeJson).toBe(authored);
  expect(shared.themeRecipeLabel).toEqual({ name: "theme", source: "imported" });
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  const reopened = await openPanel(page);
  await expect(reopened.getByTestId("theme-current-name")).toHaveText("theme");
  await expect(reopened.getByTestId("theme-current-source")).toHaveText("Imported");
  expect(await copyRecipe(page, reopened)).toBe(authored);
  errors.assertNone();
});

test("invalid imports preserve the current diagram, theme evidence, and subsequent source edits", async ({ page }) => {
  await start(page, "brutalist");
  const panel = await openPanel(page);
  await importRecipe(panel);
  await useImportedTheme(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  const validPreview = await svg(page);
  const evidence = panel.getByTestId("theme-render-evidence");
  await expect(evidence).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  const evidenceState = await evidence.getAttribute("data-theme-evidence-state");
  for (const invalid of [
    "{",
    '{"schema_version":1,"kind":"complete_spec","complete_spec":{"canvas":{"base":"#ff0000"},"canvas":{"base":"#0000ff"}}}',
    JSON.stringify({ ...recipe, schema_version: 99 }),
    JSON.stringify({ ...recipe, unknown: true }),
    JSON.stringify(recipe).replace('"value":"#172554"', '"value":"#172554","value":"#000000"'),
  ]) {
    await importRecipe(panel, invalid);
    await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "invalid");
    await expect(panel.getByRole("alert")).toBeVisible();
    await expect(panel.getByRole("button", { name: "Use imported theme", exact: true })).toHaveCount(0);
    await expect(panel.getByRole("button", { name: "Download JSON", exact: true })).toBeEnabled();
    await expect(evidence).toHaveAttribute("data-theme-evidence-state", evidenceState!);
    await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
    await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("data-theme-treatment", "dedicated");
    expect(await svg(page)).toBe(validPreview);
  }
  await importRecipe(panel);
  await expect(panel.getByRole("button", { name: "Use imported theme", exact: true })).toBeEnabled();
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(JSON.parse(await copyRecipe(page, panel))).toEqual(recipe);
  expect(await svg(page)).toBe(validPreview);
  await replaceEditorSource(page, "flowchart LR\nEdited[Source edit survives] --> B");
  await expect.poll(() => svg(page)).toContain("Source edit survives");
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  await expect(panel).toBeHidden();
  await expect(page.getByRole("button", { name: "Appearance", exact: true })).toBeFocused();
  await expect.poll(() => svg(page)).toContain("Source edit survives");
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  expect(JSON.parse(await copyRecipe(page, await openPanel(page)))).toEqual(recipe);
});

test("presets export standalone JSON and replace an imported selection directly", async ({ page }) => {
  await start(page);
  const panel = await openPanel(page);
  await importRecipe(panel);
  await useImportedTheme(panel);
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
  await useImportedTheme(panel);
  expect(JSON.parse(await copyRecipe(page, panel))).toEqual(parsed);
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  expect(JSON.parse(await copyRecipe(page, await openPanel(page)))).toEqual(parsed);
});

test("discarding a validated import leaves the current theme and source unchanged", async ({ page }) => {
  await start(page, "brutalist");
  let panel = await openPanel(page);
  const currentSvg = await svg(page);
  await importRecipe(panel);
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "ready");
  await panel.getByRole("button", { name: "Discard import", exact: true }).click();
  await expect(panel.getByTestId("theme-import-candidate")).toHaveCount(0);
  expect(await svg(page)).toBe(currentSvg);
  await expect(panel.locator('[data-theme-choice="preset:brutalist"]')).toHaveAttribute("aria-pressed", "true");
  const saved = decodeShareHash(new URL(await copyWorkspaceLink(page)).hash)!;
  expect(saved.themePresetId).toBe("brutalist");
  expect(saved.themeRecipeJson).toBeNull();
  expect(saved.code).toBe("flowchart LR\nA[Start] --> B[Done]");
  await importRecipe(panel);
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "ready");
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  panel = await openPanel(page);
  await expect(panel.getByTestId("theme-import-candidate")).toHaveCount(0);
  await expect(panel.locator('[data-theme-choice="preset:brutalist"]')).toHaveAttribute("aria-pressed", "true");
  expect(await svg(page)).toBe(currentSvg);
});

test("closing Appearance retains immediate theme and font choices alongside authored config", async ({ page }) => {
  await start(page, "brutalist");
  const panel = await openPanel(page);
  await panel.getByRole("button", { name: /^Forest/ }).click();
  const config = '{"flowchart":{"look":"classic"}}';
  await replaceMermaidConfig(page, config);
  await panel.getByLabel("Diagram font", { exact: true }).selectOption("arial");
  await importRecipe(panel);
  await useImportedTheme(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const reopened = await openPanel(page);
  await expect(reopened.getByLabel("Diagram font", { exact: true })).toHaveValue("arial");
  expect(JSON.parse(await copyRecipe(page, reopened))).toEqual(recipe);
  await reopened.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const url = await copyWorkspaceLink(page);
  const saved = decodeShareHash(new URL(url).hash)!;
  expect(saved.diagramTheme).toBe("forest");
  expect(saved.mermaidConfig).toBe(config);
  expect(saved.diagramFont).toBe("arial");
  expect(saved.themePresetId).toBeNull();
  expect(JSON.parse(saved.themeRecipeJson!)).toEqual(recipe);
});

test("same-render summary is readable before Advanced and retains raw execution diagnostics", async ({ page }) => {
  await start(page);
  await replaceEditorSource(page, "classDiagram\nclass 用户");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitemradio", { name: "resvg-compatible SVG", exact: true }).click();
  await page.keyboard.press("Escape");
  const panel = await openPanel(page);
  await importRecipe(panel, JSON.stringify({
    schema_version: 1,
    kind: "complete_spec",
    complete_spec: { styles: [{ kind: "rule", family: "class", target: "node", style: { stroke: { width: 2 } } }] },
  }));
  await useImportedTheme(panel);
  await expect.poll(() => svg(page)).toContain("用户");
  const summary = panel.getByTestId("theme-render-evidence");
  await expect(summary).toHaveAttribute("data-theme-evidence-state", /^(residual|incomplete)$/);
  await expect(summary.getByTestId("theme-target-details")).toHaveCount(0);
  await expect(panel.getByTestId("theme-execution-metadata")).toHaveCount(0);
  await openAdvanced(panel);
  await expect(panel).toContainText("Current preview");
  await expect(panel).toContainText("These are the runtime observations for the current SVG");
  const raw = panel.getByTestId("theme-execution-metadata");
  await expect.poll(async () => {
    const text = await raw.textContent();
    return text ? JSON.parse(text).theme_execution_evidence?.family_id : null;
  }).toBe("class");
  const evidence = JSON.parse((await raw.textContent())!).theme_execution_evidence;
  expect(evidence.version).toBe(2);
  expect(evidence.materialized_theme).toBe("redux-color");
  expect(["residual", "incomplete"]).toContain(evidence.theme_status);
  expect(evidence.target_status).not.toBe("portable");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…", exact: true }).click();
  const output = page.getByRole("dialog", { name: "Export image", exact: true }).getByTestId("output-compatibility");
  await expect(output).toHaveAttribute("data-output-target-state", evidence.target_status);
  await expect(output).toHaveAttribute("data-output-evidence-version", "2");
});

test("unconfirmed imports leave the current Compare publication available for export and sharing", async ({ page }) => {
  await start(page);
  const panel = await openPanel(page);
  await importRecipe(panel);
  await useImportedTheme(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await page.getByRole("button", { name: "Tools", exact: true }).click();
  await page.getByRole("menuitem", { name: "Compare", exact: true }).click();
  const merman = page.locator('[data-merman-compare-engine="merman"]');
  await expect(merman).toHaveAttribute("data-merman-artifact-state", "ready");
  const currentSvg = await merman.locator(".preview-container svg").evaluate((element) => element.outerHTML);
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…" }).click();
  const dialog = page.getByRole("dialog", { name: "Export image" });
  await expect(dialog).toBeVisible();
  const publication = await dialog.getAttribute("data-export-publication");
  await page.locator('#theme-panel input[type="file"]').setInputFiles({
    name: "changed.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify({
      ...recipe, design: { ...recipe.design, canvas: { base: "#ddffdd" } },
    })),
  });
  await expect(page.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "ready");
  await expect(dialog).toBeVisible();
  await expect(dialog).toHaveAttribute("data-export-publication", publication!);
  expect(await merman.locator(".preview-container svg").evaluate((element) => element.outerHTML)).toBe(currentSvg);
  await page.getByRole("button", { name: "Close export", exact: true }).click();
  await expect(merman.getByRole("button", { name: "Copy SVG", exact: true })).toBeEnabled();
  await expect(merman.getByRole("button", { name: "Export image", exact: true })).toBeEnabled();
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Copy workspace link", exact: true })).toBeEnabled();
  await expect(page.getByRole("menuitem", { name: "Copy issue reproduction link", exact: true })).toBeEnabled();
  await page.keyboard.press("Escape");
  await useImportedTheme(panel);
  await expect.poll(() => svg(page)).toContain("#ddffdd");
  await expect(merman.getByRole("button", { name: "Copy SVG", exact: true })).toBeEnabled();
  await merman.getByRole("button", { name: "Export image", exact: true }).click();
  await expect(dialog).not.toHaveAttribute("data-export-publication", publication!);
});

test("valid theme import and export do not require a valid diagram", async ({ page }) => {
  await start(page);
  await replaceEditorSource(page, "flowchart LR\nA -->");
  await expect(page.locator('[data-merman-render-error="true"]')).toBeVisible();
  const panel = await openPanel(page);
  await importRecipe(panel);
  await useImportedTheme(panel);
  const download = page.waitForEvent("download");
  await panel.getByRole("button", { name: "Download JSON", exact: true }).click();
  expect(JSON.parse(await readFile((await (await download).path())!, "utf8"))).toEqual(recipe);
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  await replaceEditorSource(page, "flowchart LR\nA[Recovered] --> B");
  await expect.poll(() => svg(page)).toContain("#fef3c7");
});

test("closing the theme panel does not steal focus from a newly focused editor", async ({ page }) => {
  await start(page);
  const panel = await openPanel(page);
  await page.clock.install({ time: 0 });
  await page.clock.pauseAt(1000);
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).dispatchEvent("click");
  const editor = page.getByRole("textbox", { name: "Mermaid source", exact: true });
  await editor.focus();
  await page.clock.runFor(32);
  await expect(editor).toBeFocused();
});

test("oversized import leaves the valid selection intact and can be replaced", async ({ page }) => {
  await start(page);
  const panel = await openPanel(page);
  await importRecipe(panel);
  await useImportedTheme(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await panel.getByLabel("Import JSON", { exact: true }).setInputFiles({
    name: "too-large.json", mimeType: "application/json", buffer: Buffer.alloc(256 * 1024 + 1, 32),
  });
  await expect(panel.getByRole("alert")).toContainText("256 KiB");
  expect(JSON.parse(await copyRecipe(page, panel))).toEqual(recipe);
  const changed = { ...recipe, design: { ...recipe.design, canvas: { base: "#ddffdd" } } };
  await importRecipe(panel, JSON.stringify(changed));
  await expect(panel.getByRole("alert")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Use imported theme", exact: true })).toBeEnabled();
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await useImportedTheme(panel);
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
  await useImportedTheme(panel);
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  const currentSvg = await svg(page);
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  const evidenceState = await panel.getByTestId("theme-render-evidence").getAttribute("data-theme-evidence-state");
  await importRecipe(panel, "{}", "delayed.json");
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "reading");
  await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", evidenceState!);
  expect(await svg(page)).toBe(currentSvg);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("data-theme-treatment", "dedicated");
  await expect(panel.getByRole("button", { name: "Use imported theme", exact: true })).toHaveCount(0);
  const changed = JSON.stringify({ ...recipe, design: { ...recipe.design, canvas: { base: "#ddffdd" } } });
  await importRecipe(panel, changed);
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "ready");
  await page.evaluate(() => window.dispatchEvent(new Event("release-theme-import")));
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "ready");
  expect(await svg(page)).toBe(currentSvg);
  await useImportedTheme(panel);
  await expect.poll(() => svg(page)).toContain("#ddffdd");
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(await copyRecipe(page, panel)).toBe(changed);
  await importRecipe(panel, "{}", "delayed.json");
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "reading");
  await expect(panel.getByTestId("theme-execution-metadata")).toBeVisible();
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  panel = await openPanel(page);
  await page.evaluate(() => window.dispatchEvent(new Event("release-theme-import")));
  await expect(panel.getByTestId("theme-import-candidate")).toHaveCount(0);
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(await copyRecipe(page, panel)).toBe(changed);
  await importRecipe(panel, "{}", "delayed.json");
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "reading");
  const brutalist = panel.locator('[data-theme-choice="preset:brutalist"]');
  await brutalist.click();
  await expect(brutalist).toHaveAttribute("aria-pressed", "true");
  await page.evaluate(() => window.dispatchEvent(new Event("release-theme-import")));
  await expect(panel.getByTestId("theme-import-candidate")).toHaveCount(0);
  await expect(panel.getByRole("alert")).toHaveCount(0);
  expect(decodeShareHash(new URL(await copyWorkspaceLink(page)).hash)!.themePresetId).toBe("brutalist");
});

test("Chinese theme panel keeps import, preview and advanced JSON reachable on a narrow screen", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 740 });
  await page.addInitScript(() => localStorage.setItem("merman-language", "zh"));
  await page.goto("./", { waitUntil: "domcontentloaded" });
  await expect(page.locator("footer")).toContainText("WASM: 就绪");
  await page.getByRole("button", { name: "外观", exact: true }).click();
  const panel = page.getByRole("region", { name: "外观", exact: true });
  await expect(panel).toBeVisible();
  await waitForPreviewSvg(page);
  const previewBounds = await page.locator(".preview-container").first().boundingBox();
  expect(previewBounds!.height).toBeGreaterThan(60);
  await expect(panel.locator('[data-theme-choice="mermaid:dark"]')).toBeVisible();
  await expect(panel.getByLabel("图表字体", { exact: true })).toHaveValue("inherit");
  await expect(panel).toContainText("由主题和源码决定字体");
  await panel.getByRole("button", { name: /^粗野主义/ }).click();
  const note = panel.locator('[data-theme-application-note="flowchart-basic-shadows"]');
  await expect(note).toContainText("Classic 和 Neo 外观均支持基本形状");
  await expect(note).toContainText("手绘和其他形状可能无法应用效果");
  const input = panel.getByLabel("导入 JSON", { exact: true });
  await input.setInputFiles({ name: "invalid.json", mimeType: "application/json", buffer: Buffer.from("{") });
  await expect(panel.getByRole("alert")).toBeVisible();
  await expect(panel.getByRole("button", { name: "采用导入主题", exact: true })).toHaveCount(0);
  await input.setInputFiles({ name: "theme.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(recipe)) });
  await panel.getByRole("button", { name: "采用导入主题", exact: true }).click();
  await expect.poll(() => svg(page)).toContain("#fef3c7");
  await openAdvanced(panel, "高级");
  await expect(panel.getByRole("textbox", { name: "主题 JSON", exact: true })).toHaveAttribute("readonly", "");
  await expect(panel.getByRole("button", { name: "复制 JSON", exact: true })).toBeVisible();
  const separator = page.getByRole("separator", { name: "调整主题面板大小", exact: true });
  await separator.focus();
  await separator.press("ArrowUp");
  await expectNoDocumentOverflow(page);
  const bounds = (await panel.boundingBox())!;
  expect(bounds.x).toBeGreaterThanOrEqual(0);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(321);
  await page.screenshot({ path: test.info().outputPath("theme-panel-mobile-zh.png") });
  await panel.getByRole("button", { name: "关闭外观面板", exact: true }).click();
  await expect(panel).toBeHidden();
  await expect.poll(() => svg(page)).toContain("#fef3c7");
});
