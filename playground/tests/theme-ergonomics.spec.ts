import { expect, test, type Locator, type Page } from "@playwright/test";
import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { monitorBrowserErrors, replaceEditorSource, replaceMermaidConfig, waitForPreviewSvg } from "./helpers/playground";

async function openPanel(page: Page) {
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  return page.getByRole("region", { name: "Theme panel", exact: true });
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
  await expect(page.locator("footer")).toContainText("WASM: Ready");
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

async function apply(panel: Locator) {
  await panel.getByRole("button", { name: "Apply", exact: true }).click();
}

test("the top preset selector previews immediately and keeps keyboard focus without applying", async ({ page }) => {
  await start(page, "{}", null, 'pie title Tea time\n"Tea": 4\n"Cake": 3');
  const panel = await openPanel(page);
  const chooser = panel.getByRole("combobox", { name: "Merman preset", exact: true });
  await chooser.focus();
  await chooser.selectOption("preset:ghibli");
  await expect(chooser).toBeFocused();
  await expect(chooser.getByRole("option", { name: "Ghibli · Alpha · Designed for this diagram", exact: true })).toBeAttached();
  await expect(panel.getByTestId("theme-current-treatment")).toHaveText("Designed for this diagram");
  await expect(panel.getByTestId("theme-current-maturity")).toContainText("Alpha");
  await expect(panel.locator("[data-theme-preset]").first()).toHaveAttribute("data-theme-preset", "ghibli");
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toBeEnabled();
  await expect(panel.locator("[data-theme-draft-status]")).toContainText("Theme preview");
  await expect(panel.getByTestId("theme-outcome")).toContainText("Theme applied");
  await page.screenshot({ path: test.info().outputPath("theme-panel-ghibli.png") });
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  const reopened = await openPanel(page);
  await expect(reopened.getByRole("combobox", { name: "Merman preset", exact: true })).toHaveValue("none");
});

test("base theme, font and recommended config preview together and Cancel preserves only editor changes", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const config = {
    layout: "dagre", htmlLabels: false,
    flowchart: { look: "handDrawn", layout: "dagre", nodeSpacing: 42, curve: "linear" },
    sequence: { actorMargin: 72 }, themeVariables: { primaryColor: "#123456" },
  };
  const original = await start(page, JSON.stringify(config), null, "flowchart LR\nA[Authored source] --> B");
  await waitForPreviewSvg(page);
  const panel = await openPanel(page);
  await expect(panel).toContainText("Apply uses all panel changes for this workspace");
  await expect(panel.getByLabel("Diagram font", { exact: true })).toHaveValue("inherit");
  await panel.getByLabel("Mermaid base theme", { exact: true }).selectOption("dark");
  await panel.getByLabel("Diagram font", { exact: true }).selectOption("arial");
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
  const note = panel.locator('[data-theme-application-note="flowchart-classic-shadows"]');
  await expect(note).toContainText("Classic look");
  await expect(note).toContainText("basic shapes");
  const recommended = note.getByRole("button", { name: "Use Classic look", exact: true });
  await recommended.focus();
  await recommended.press("Enter");
  await expect(note.getByTestId("theme-recommended-state")).toHaveText("Already in the preview config");
  await expect(recommended).toHaveCount(0);
  await expect(panel.getByTestId("theme-appearance-overrides")).toContainText("classic");
  await expect.poll(() => page.locator(".preview-container > div").first().evaluate((host) =>
    host.shadowRoot?.querySelector("svg")?.outerHTML ?? "",
  )).toContain("Arial");
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Copy workspace link", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");

  const editedSource = "flowchart LR\nEdited[Editor changes stay] --> B";
  const editedConfig = JSON.stringify({ ...config, sequence: { actorMargin: 96 } });
  await replaceEditorSource(page, editedSource);
  await replaceMermaidConfig(page, editedConfig);
  await expect(note.getByTestId("theme-recommended-state")).toHaveText("Already in the preview config");
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  const cancelled = await workspaceFromLink(page);
  expect(cancelled.themePresetId).toBe(original.themePresetId);
  expect(cancelled.diagramTheme).toBe(original.diagramTheme);
  expect(cancelled.diagramFont).toBe(original.diagramFont);
  expect(cancelled.code).toBe(editedSource);
  expect(cancelled.mermaidConfig).toBe(editedConfig);
  const reopened = await openPanel(page);
  await expect(reopened.getByLabel("Mermaid base theme", { exact: true })).toHaveValue(original.diagramTheme);
  await expect(reopened.getByLabel("Diagram font", { exact: true })).toHaveValue("inherit");
  await expect(reopened.getByRole("combobox", { name: "Merman preset", exact: true })).toHaveValue("none");
  errors.assertNone();
});

test("Apply uses the base theme, font and recommended config in the same workspace", async ({ page }) => {
  const original = await start(page, '{"flowchart":{"look":"handDrawn","nodeSpacing":42}}', null,
    "flowchart LR\nA[Authored source] --> B");
  await waitForPreviewSvg(page);
  const panel = await openPanel(page);
  await panel.getByLabel("Mermaid base theme", { exact: true }).selectOption("forest");
  await panel.getByLabel("Diagram font", { exact: true }).selectOption("georgia");
  await panel.getByRole("combobox", { name: "Merman preset", exact: true }).selectOption("preset:brutalist");
  await panel.getByRole("button", { name: "Use Classic look", exact: true }).click();
  await apply(panel);
  await expect(panel.locator("[data-theme-draft-status]")).toHaveText("Applied to this workspace");
  const applied = await workspaceFromLink(page);
  expect(applied.code).toBe(original.code);
  expect(applied.diagramTheme).toBe("forest");
  expect(applied.diagramFont).toBe("georgia");
  expect(applied.themePresetId).toBe("brutalist");
  expect(JSON.parse(applied.mermaidConfig)).toEqual({ flowchart: { look: "classic", nodeSpacing: 42 } });
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  const reopened = await openPanel(page);
  await expect(reopened.getByLabel("Diagram font", { exact: true })).toHaveValue("georgia");
  await expect(reopened.getByRole("button", { name: "Use Classic look", exact: true })).toHaveCount(0);
});

test("source look overrides are explained with a source editor entry", async ({ page }) => {
  await start(page, '{"theme":"forest","fontFamily":"Georgia"}');
  await waitForPreviewSvg(page);
  const panel = await openPanel(page);
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
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
  { origin: "config", config: '{"look":"classic"}', source: "flowchart LR\nA --> B", state: "Already in the preview config" },
  { origin: "source", config: "{}", source: 'flowchart LR\nA --> B\n%%{init: {"flowchart":{"look":"classic"}}}%%', state: "Classic look is already declared in the source" },
]) {
  test(`a Classic look declaration in ${declaration.origin} avoids repeating the recommendation`, async ({ page }) => {
    await start(page, declaration.config, "brutalist", declaration.source);
    await waitForPreviewSvg(page);
    const panel = await openPanel(page);
    await expect(panel.getByTestId("theme-recommended-state")).toHaveText(declaration.state);
    await expect(panel.getByRole("button", { name: "Use Classic look", exact: true })).toHaveCount(0);
  });
}

test("Pie never shows a Flowchart application action", async ({ page }) => {
  await start(page, "{}", "brutalist", 'pie title Tea time\n"Tea": 4\n"Cake": 3');
  await waitForPreviewSvg(page);
  const panel = await openPanel(page);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Pie Chart");
  await expect(panel.locator("[data-theme-application-note]")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Use Classic look", exact: true })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toBeEnabled();
});

test("Apply reports invalid live config after a recommendation and Cancel preserves it for repair", async ({ page }) => {
  await start(page, '{"flowchart":{"look":"handDrawn"}}', null, "flowchart LR\nA --> B");
  await waitForPreviewSvg(page);
  const panel = await openPanel(page);
  await panel.getByRole("combobox", { name: "Merman preset", exact: true }).selectOption("preset:brutalist");
  await panel.getByRole("button", { name: "Use Classic look", exact: true }).click();
  await expect(panel.getByTestId("theme-recommended-state")).toHaveText("Already in the preview config");
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
  await apply(panel);
  await expect(panel.getByRole("alert")).toBeVisible();
  await expect(panel.locator("[data-theme-draft-status]")).toContainText("Theme preview");
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  expect((await workspaceFromLink(page)).mermaidConfig).toBe("{");
  const reopened = await openPanel(page);
  await expect(reopened.getByRole("combobox", { name: "Merman preset", exact: true })).toHaveValue("none");
});

test("Brutalist keeps styling diagnostics visible and output compatibility details collapsed", async ({ page }) => {
  await start(page, "{}", "brutalist", "flowchart LR\nA[Authored source] --> B");
  await waitForPreviewSvg(page);
  const panel = await openPanel(page);
  const reasons = panel.getByTestId("theme-diagnostic-reasons");
  await expect(reasons.locator('[data-theme-reason-code="unsupported-effect"]')).toContainText("shadow or glow");
  await expect(reasons).toContainText("Review Application notes");
  const targets = panel.getByTestId("theme-target-reasons");
  const details = panel.getByTestId("theme-target-details");
  await expect(details).not.toHaveAttribute("open", "");
  await expect(targets).toBeHidden();
  await details.locator("summary").click();
  await expect(targets).toBeVisible();
  await expect(targets.locator('[data-theme-reason-code="theme_evidence_incomplete"]')).toContainText("Theme execution evidence is incomplete");
  await expect(targets.locator('[data-theme-reason-code="svg_terminal_unverified"]')).toContainText("final SVG has not been verified");
  await expect(panel.getByTestId("theme-execution-metadata")).toHaveCount(0);
});

test("pending theme selection retains current-family choices while hiding previous evidence", async ({ page }) => {
  await start(page, "{}", "cyberpunk");
  await waitForPreviewSvg(page);
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

  await page.clock.runFor(400);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await expect(ghibli).toHaveAttribute("data-theme-treatment", "unreviewed");
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(panel.getByTestId("theme-execution-metadata")).toBeVisible();
});

test("invalid theme imports keep current-family choices available without exposing last-valid evidence", async ({ page }) => {
  await start(page, "{}", "brutalist");
  await waitForPreviewSvg(page);
  const panel = await openPanel(page);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await panel.locator("summary").filter({ hasText: /^Advanced$/ }).click();
  await expect(panel.getByTestId("theme-execution-metadata")).toBeVisible();
  await panel.getByLabel("Import JSON", { exact: true }).setInputFiles({
    name: "invalid.json", mimeType: "application/json", buffer: Buffer.from("{"),
  });
  await expect(panel.getByRole("alert")).toBeVisible();
  const importError = await panel.getByRole("alert").innerText();
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeDisabled();
  await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(panel.getByTestId("theme-execution-metadata")).toHaveCount(0);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await panel.getByRole("button", { name: "Restore last valid theme", exact: true }).click();
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByRole("alert").filter({ hasText: importError })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeEnabled();
  await expect(panel.getByRole("button", { name: "Restore last valid theme", exact: true })).toHaveCount(0);
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
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

test("recommendation and import recovery keep keyboard focus inside the panel", async ({ page }) => {
  await start(page, "{}", "brutalist", "flowchart LR\nA --> B");
  let panel = await openPanel(page);
  const recommended = panel.getByRole("button", { name: "Use Classic look", exact: true });
  await recommended.focus();
  await recommended.press("Enter");
  await expect(panel.getByTestId("theme-selection-heading")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
  panel = await openPanel(page);
  await panel.getByLabel("Import JSON", { exact: true }).setInputFiles({
    name: "invalid.json", mimeType: "application/json", buffer: Buffer.from("{"),
  });
  const restore = panel.getByRole("button", { name: "Restore last valid theme", exact: true });
  await restore.focus();
  await restore.press("Enter");
  await expect(panel.getByTestId("theme-selection-heading")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
});
