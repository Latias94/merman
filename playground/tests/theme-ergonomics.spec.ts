import { expect, test, type Locator, type Page } from "@playwright/test";
import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { monitorBrowserErrors, replaceEditorSource, replaceMermaidConfig, waitForPreviewSvg } from "./helpers/playground";

async function openPanel(page: Page) {
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const panel = page.getByRole("region", { name: "Appearance", exact: true });
  await expect(panel).toBeVisible();
  return panel;
}

async function start(page: Page, mermaidConfig = "{}", themePresetId: string | null = null,
  code = 'flowchart LR\nA[Authored source] --> B\n%%{init: {"flowchart":{"look":"handDrawn"}}}%%') {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript(() => localStorage.setItem("merman-language", "en"));
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    code,
    mermaidConfig, themePresetId,
  };
  await page.goto(`./${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  return workspace;
}

async function workspaceFromLink(page: Page) {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  const copy = page.getByRole("menuitem", { name: "Copy workspace link", exact: true });
  await expect(copy).toBeEnabled();
  await copy.click();
  const url = await page.evaluate(() => navigator.clipboard.readText());
  return decodeShareHash(new URL(url).hash)!;
}

async function svg(page: Page) {
  return page.locator(".preview-container > div").first().evaluate((host) =>
    host.shadowRoot?.querySelector("svg")?.outerHTML ?? "",
  );
}

async function openAdvanced(panel: Locator) {
  const advanced = panel.getByTestId("theme-advanced");
  if (await advanced.getAttribute("open") === null) {
    await advanced.locator("summary").first().click();
  }
}

test("a keyboard theme choice applies immediately and survives closing Appearance", async ({ page }) => {
  await start(page, "{}", null, 'pie title Tea time\n"Tea": 4\n"Cake": 3');
  const panel = await openPanel(page);
  const ghibli = panel.getByRole("button", { name: /^Ghibli/ });
  await ghibli.focus();
  await ghibli.press("Enter");
  await expect(ghibli).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByTestId("theme-current-treatment")).toHaveText("Designed for this diagram");
  await expect(panel.getByTestId("theme-current-maturity")).toContainText("Alpha");
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toBeEnabled();
  await expect(panel.getByTestId("theme-outcome")).toContainText("Theme applied");
  expect((await workspaceFromLink(page)).themePresetId).toBe("ghibli");
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const reopened = await openPanel(page);
  await expect(reopened.getByRole("button", { name: /^Ghibli/ })).toHaveAttribute("aria-pressed", "true");
});

test("the unified catalog switches between built-in, native and exploration themes without changing source or config", async ({ page }) => {
  const original = await start(page, '{"flowchart":{"nodeSpacing":42}}', null, "flowchart LR\nA[Keep my diagram] --> B");
  const panel = await openPanel(page);
  const choices = panel.getByTestId("theme-catalog-choices");
  await choices.locator('[data-theme-choice="preset:brutalist"]').click();
  const forest = choices.locator('[data-theme-choice="mermaid:forest"]');
  await forest.click();
  await expect(forest).toHaveAttribute("aria-pressed", "true");
  const native = await workspaceFromLink(page);
  expect(native.diagramTheme).toBe("forest");
  expect(native.themePresetId).toBeNull();
  expect(native.themeRecipeJson).toBeNull();

  const atlas = choices.locator('[data-theme-choice="exploration:atlas"]');
  await atlas.click();
  await expect(atlas).toHaveAttribute("aria-pressed", "true");
  await expect(atlas).toHaveAttribute("data-theme-source", "exploration");
  await expect(choices.locator('button[aria-pressed="true"]')).toHaveCount(1);
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Atlas");
  await expect.poll(() => svg(page)).toContain('data-merman-theme-canvas="base"');
  const exploration = await workspaceFromLink(page);
  expect(exploration.themePresetId).toBeNull();
  expect(exploration.themeRecipeJson).not.toBeNull();
  expect(exploration.themeRecipeLabel).toEqual({ name: "Atlas", source: "exploration" });
  expect(exploration.diagramTheme).toBe("forest");
  expect(exploration.code).toBe(original.code);
  expect(exploration.mermaidConfig).toBe(original.mermaidConfig);
});

test("theme, font and recommended config apply immediately and closing preserves editor changes", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const config = {
    layout: "dagre", htmlLabels: false,
    flowchart: { look: "handDrawn", layout: "dagre", nodeSpacing: 42, curve: "linear" },
    sequence: { actorMargin: 72 }, themeVariables: { primaryColor: "#123456" },
  };
  await start(page, JSON.stringify(config), null, "flowchart LR\nA[Authored source] --> B");
  const panel = await openPanel(page);
  await expect(panel.getByLabel("Diagram font", { exact: true })).toHaveValue("inherit");
  await panel.getByRole("button", { name: /^Dark/ }).click();
  await panel.getByLabel("Diagram font", { exact: true }).selectOption("arial");
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
  const note = panel.locator('[data-theme-application-note="flowchart-classic-shadows"]');
  await expect(note).toContainText("Classic look");
  await expect(note).toContainText("basic shapes");
  const recommended = note.getByRole("button", { name: "Use Classic look", exact: true });
  await recommended.focus();
  await recommended.press("Enter");
  await expect(note.getByTestId("theme-recommended-state")).toHaveText("Already in the current config");
  await expect(recommended).toHaveCount(0);
  await openAdvanced(panel);
  await expect(panel.getByTestId("theme-appearance-overrides")).toContainText("classic");
  await expect.poll(() => svg(page)).toContain("Arial");
  const applied = await workspaceFromLink(page);
  expect(applied.diagramTheme).toBe("dark");
  expect(applied.diagramFont).toBe("arial");
  expect(applied.themePresetId).toBe("brutalist");
  expect(JSON.parse(applied.mermaidConfig)).toEqual({ ...config, flowchart: { ...config.flowchart, look: "classic" } });

  const editedSource = "flowchart LR\nEdited[Editor changes stay] --> B";
  const editedConfig = JSON.stringify({ ...config, flowchart: { ...config.flowchart, look: "classic" }, sequence: { actorMargin: 96 } });
  await replaceEditorSource(page, editedSource);
  await replaceMermaidConfig(page, editedConfig);
  await expect(note.getByTestId("theme-recommended-state")).toHaveText("Already in the current config");
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const closed = await workspaceFromLink(page);
  expect(closed.diagramTheme).toBe("dark");
  expect(closed.diagramFont).toBe("arial");
  expect(closed.themePresetId).toBe("brutalist");
  expect(closed.code).toBe(editedSource);
  expect(closed.mermaidConfig).toBe(editedConfig);
  const reopened = await openPanel(page);
  await expect(reopened.getByLabel("Diagram font", { exact: true })).toHaveValue("arial");
  await expect(reopened.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  errors.assertNone();
});

test("Undo appearance preserves source and config authored after a theme choice", async ({ page }) => {
  const original = await start(page, "{}", "brutalist", "flowchart LR\nA --> B");
  let panel = await openPanel(page);
  await panel.getByRole("button", { name: /^Forest/ }).click();
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const editedSource = "flowchart LR\nEdited[Keep this source] --> B";
  const editedConfig = '{"flowchart":{"nodeSpacing":64},"sequence":{"actorMargin":96}}';
  await replaceEditorSource(page, editedSource);
  await replaceMermaidConfig(page, editedConfig);
  panel = await openPanel(page);
  const undo = panel.getByRole("button", { name: "Undo appearance change", exact: true });
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  const restored = await workspaceFromLink(page);
  expect(restored.diagramTheme).toBe(original.diagramTheme);
  expect(restored.themePresetId).toBe("brutalist");
  expect(restored.code).toBe(editedSource);
  expect(restored.mermaidConfig).toBe(editedConfig);
});

test("Undo a recommended config preserves its unrelated settings and subsequent source edits", async ({ page }) => {
  const config = '{"flowchart":{"look":"handDrawn","nodeSpacing":42},"sequence":{"actorMargin":72}}';
  await start(page, config, "brutalist", "flowchart LR\nA --> B");
  const panel = await openPanel(page);
  await panel.getByRole("button", { name: "Use Classic look", exact: true }).click();
  const editedSource = "flowchart LR\nEdited[Source survives undo] --> B";
  await replaceEditorSource(page, editedSource);
  await panel.getByRole("button", { name: "Undo appearance change", exact: true }).click();
  const restored = await workspaceFromLink(page);
  expect(restored.code).toBe(editedSource);
  expect(restored.mermaidConfig).toBe(config);
  expect(restored.themePresetId).toBe("brutalist");
  await expect(panel.getByRole("button", { name: "Use Classic look", exact: true })).toBeVisible();
});

test("source look overrides are explained with a source editor entry", async ({ page }) => {
  await start(page, '{"theme":"forest","fontFamily":"Georgia"}');
  const panel = await openPanel(page);
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
  await openAdvanced(panel);
  const overrides = panel.getByTestId("theme-appearance-overrides");
  await expect(overrides).toContainText("forest");
  await expect(overrides).toContainText("Georgia");
  await expect(overrides).toContainText("handDrawn");
  await overrides.getByRole("button", { name: "Edit config", exact: true }).click();
  await expect(page.getByRole("tab", { name: "Config", exact: true })).toHaveAttribute("aria-selected", "true");
  await panel.getByRole("button", { name: "Use Classic look", exact: true }).click();
  await expect(panel.getByTestId("theme-recommendation-source-override")).toContainText("Source declares the look as “handDrawn”");
  await overrides.getByRole("button", { name: "Edit source", exact: true }).click();
  await expect(page.getByRole("tab", { name: "Code", exact: true })).toHaveAttribute("aria-selected", "true");
});

for (const declaration of [
  { origin: "config", config: '{"look":"classic"}', source: "flowchart LR\nA --> B", state: "Already in the current config" },
  { origin: "source", config: "{}", source: 'flowchart LR\nA --> B\n%%{init: {"flowchart":{"look":"classic"}}}%%', state: "Classic look is already declared in the source" },
]) {
  test(`a Classic look declaration in ${declaration.origin} avoids repeating the recommendation`, async ({ page }) => {
    await start(page, declaration.config, "brutalist", declaration.source);
    const panel = await openPanel(page);
    await expect(panel.getByTestId("theme-recommended-state")).toHaveText(declaration.state);
    await expect(panel.getByRole("button", { name: "Use Classic look", exact: true })).toHaveCount(0);
  });
}

test("Pie never shows a Flowchart application action", async ({ page }) => {
  await start(page, "{}", "brutalist", 'pie title Tea time\n"Tea": 4\n"Cake": 3');
  const panel = await openPanel(page);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Pie Chart");
  await expect(panel.locator("[data-theme-application-note]")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Use Classic look", exact: true })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toBeEnabled();
});

test("invalid authored config remains repairable after an immediate theme choice and closing", async ({ page }) => {
  await start(page, '{"flowchart":{"look":"handDrawn"}}', null, "flowchart LR\nA --> B");
  const panel = await openPanel(page);
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
  await panel.getByRole("button", { name: "Use Classic look", exact: true }).click();
  await page.getByRole("tab", { name: "Config", exact: true }).click();
  const config = page.getByRole("textbox", { name: "Mermaid configuration", exact: true });
  await config.focus();
  await page.keyboard.press(await page.evaluate(() => /Macintosh|Mac OS X|iPhone|iPad/u.test(navigator.userAgent) ? "Meta+A" : "Control+A"));
  await config.evaluate((element) => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/plain", "{");
    element.dispatchEvent(new ClipboardEvent("paste", { bubbles: true, cancelable: true, clipboardData: clipboard }));
  });
  await expect(page.getByText(/Invalid JSON/)).toBeVisible();
  await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "failed");
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const saved = await workspaceFromLink(page);
  expect(saved.mermaidConfig).toBe("{");
  expect(saved.themePresetId).toBe("brutalist");
  await replaceMermaidConfig(page, '{"flowchart":{"look":"classic"}}');
  const reopened = await openPanel(page);
  await expect(reopened.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  await expect(reopened.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "failed");
});

test("Brutalist shows styling diagnostics in Appearance and compatibility evidence in Export", async ({ page }) => {
  await start(page, "{}", "brutalist", "flowchart LR\nA[Authored source] --> B");
  const panel = await openPanel(page);
  const reasons = panel.getByTestId("theme-diagnostic-reasons");
  await expect(reasons.locator('[data-theme-reason-code="unsupported-effect"]')).toContainText("shadow or glow");
  await expect(reasons).toContainText("Review Application notes");
  await expect(panel.getByTestId("theme-target-details")).toHaveCount(0);
  await expect(panel.getByTestId("theme-execution-metadata")).toHaveCount(0);
  await openAdvanced(panel);
  const metadata = JSON.parse((await panel.getByTestId("theme-execution-metadata").textContent())!);
  const evidence = metadata.theme_execution_evidence;
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…", exact: true }).click();
  const output = page.getByRole("dialog", { name: "Export image", exact: true }).getByTestId("output-compatibility");
  await expect(output).toHaveAttribute("data-output-target-state", evidence.target_status);
  await expect(output).toHaveAttribute("data-output-evidence-version", "2");
  const details = output.locator("details").first();
  await expect(details).not.toHaveAttribute("open", "");
  await details.locator("summary").first().click();
  expect(evidence.target_reason_ids.length).toBeGreaterThan(0);
  for (const code of evidence.target_reason_ids.slice(0, 4)) {
    await expect(output.locator(`[data-output-reason-code="${code}"]`)).toBeVisible();
  }
});

test("an immediate theme choice hides previous evidence and gates artifact actions until its current render", async ({ page }) => {
  await start(page, "{}", "cyberpunk");
  await page.getByRole("button", { name: "Tools", exact: true }).click();
  await page.getByRole("menuitem", { name: "Compare", exact: true }).click();
  const merman = page.locator('[data-merman-compare-engine="merman"]');
  await expect(merman).toHaveAttribute("data-merman-artifact-state", "ready");
  const panel = await openPanel(page);
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  await panel.locator("summary").filter({ hasText: /^Advanced$/ }).click();
  await expect(panel.getByTestId("theme-execution-metadata")).toBeVisible();

  await page.clock.install({ time: 0 });
  await page.clock.pauseAt(1000);
  const ghibli = panel.getByRole("button", { name: /^Ghibli/ });
  await ghibli.dispatchEvent("click");
  await expect(ghibli).toHaveAttribute("aria-pressed", "true");
  await expect(ghibli).toHaveAttribute("data-theme-treatment", "unreviewed");
  await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("data-theme-treatment", "dedicated");
  await expect(panel.getByTestId("theme-execution-metadata")).toHaveCount(0);
  await expect(merman.getByRole("button", { name: "Copy SVG", exact: true })).toBeDisabled();
  await expect(merman.getByRole("button", { name: "Export image", exact: true })).toBeDisabled();

  await page.clock.runFor(1000);
  await page.clock.resume();
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await expect(ghibli).toHaveAttribute("data-theme-treatment", "unreviewed");
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(panel.getByTestId("theme-execution-metadata")).toBeVisible();
  const raw = JSON.parse((await panel.getByTestId("theme-execution-metadata").textContent())!);
  expect(raw.theme_execution_evidence.family_id).toBe("flowchart");
  await expect(merman.getByRole("button", { name: "Copy SVG", exact: true })).toBeEnabled();
  await expect(merman.getByRole("button", { name: "Export image", exact: true })).toBeEnabled();
});

test("an invalid import leaves the current-family choices and current runtime evidence intact", async ({ page }) => {
  await start(page, "{}", "brutalist");
  const panel = await openPanel(page);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await panel.locator("summary").filter({ hasText: /^Advanced$/ }).click();
  const metadata = panel.getByTestId("theme-execution-metadata");
  await expect(metadata).toBeVisible();
  const currentMetadata = await metadata.textContent();
  const currentSvg = await svg(page);
  await panel.getByLabel("Import JSON", { exact: true }).setInputFiles({
    name: "invalid.json", mimeType: "application/json", buffer: Buffer.from("{"),
  });
  await expect(panel.getByRole("alert")).toBeVisible();
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "invalid");
  await expect(panel.getByRole("button", { name: "Use imported theme", exact: true })).toHaveCount(0);
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(metadata).toHaveText(currentMetadata!);
  expect(await svg(page)).toBe(currentSvg);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  expect((await workspaceFromLink(page)).themePresetId).toBe("brutalist");
});

test("Ghibli inherits its recipe font and releases explicit UI font overrides", async ({ page }) => {
  await start(page, "{}", "ghibli", 'pie title Tea time\n"Tea": 4\n"Cake": 3');
  const panel = await openPanel(page);
  const font = panel.getByRole("combobox", { name: "Diagram font", exact: true });
  await expect(font).toHaveValue("inherit");
  const titleFont = () => page.locator(".preview-container > div").first().evaluate((host) => {
    const title = host.shadowRoot?.querySelector(".pieTitleText");
    return title ? getComputedStyle(title).fontFamily : null;
  });
  await expect.poll(titleFont).toContain("Open Sans");
  await font.selectOption("georgia");
  await expect.poll(titleFont).toContain("Georgia");
  await font.selectOption("inherit");
  await expect.poll(titleFont).toContain("Open Sans");
});

test("recommendations apply with the keyboard and Escape closes Appearance without rolling them back", async ({ page }) => {
  await start(page, "{}", "brutalist", "flowchart LR\nA --> B");
  const panel = await openPanel(page);
  const recommended = panel.getByRole("button", { name: "Use Classic look", exact: true });
  await recommended.focus();
  await recommended.press("Enter");
  await expect(panel.getByTestId("theme-recommended-state")).toHaveText("Already in the current config");
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Appearance", exact: true })).toBeFocused();
  const saved = await workspaceFromLink(page);
  expect(JSON.parse(saved.mermaidConfig)).toEqual({ flowchart: { look: "classic" } });
});
