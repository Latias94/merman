import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { strFromU8, unzipSync } from "fflate";

import { encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot } from "../src/lib/workspace-snapshot";
import { expectNoDocumentOverflow, monitorBrowserErrors, replaceEditorSource, waitForPreviewSvg } from "./helpers/playground";

const workspace: WorkspaceSnapshot = {
  ...DEFAULT_WORKSPACE_SNAPSHOT,
  code: '---\nconfig:\n  theme: dark\n---\nflowchart LR\nA[Captured source] --> B[Done]',
  mermaidConfig: '{ "theme":"forest", "flowchart":{"htmlLabels":false,"look":"neo"} }',
  diagramTheme: "default",
  diagramFont: "trebuchet",
  themePresetId: "one-dark",
};

async function open(page: Page, value = workspace): Promise<Locator> {
  await page.addInitScript(() => {
    if (window === window.top && (location.protocol === "http:" || location.protocol === "https:")) localStorage.setItem("merman-language", "en");
  });
  await page.goto(`./${encodeShareHash(value)}`, { waitUntil: "domcontentloaded" });
  if ((page.viewportSize()?.width ?? 1280) < 768) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);
  await page.getByTestId("project-use-launcher").click();
  const dialog = page.getByRole("dialog", { name: "Use in project", exact: true });
  await expect(dialog.getByTestId("project-use-snapshot")).toContainText("Completed Merman render");
  return dialog;
}

async function downloadFiles(page: Page, dialog: Locator): Promise<Record<string, string>> {
  const pending = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download all files (.zip)", exact: true }).click();
  const download = await pending;
  expect(download.suggestedFilename()).toMatch(/^merman-(diagram|theme)-(rust|cli|web)\.zip$/);
  const entries = unzipSync(await readFile((await download.path())!));
  return Object.fromEntries(Object.entries(entries).map(([name, bytes]) => [name, strFromU8(bytes)]));
}

test("project use offers Rust, CLI and local Web consumers with complete config provenance", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  const dialog = await open(page);
  await expect(dialog.getByTestId("project-use-code")).toContainText('ThemePreset::from_id("one-dark")');
  const rustFiles = await downloadFiles(page, dialog);
  expect(rustFiles["Cargo.toml"]).toContain('../merman/crates/merman');
  expect(rustFiles["src/main.rs"]).toContain("RenderRequest::svg");
  expect(rustFiles["source.mmd"]).toBe(workspace.code);
  expect(rustFiles["playground-config.json"]).toBe(workspace.mermaidConfig);
  expect(rustFiles["diagram.mmd"]).toContain('---\n%%{init: {"theme":"forest"');
  expect(JSON.parse(rustFiles["site-config.json"]).flowchart).toBeUndefined();
  await dialog.getByRole("tab", { name: "CLI", exact: true }).click();
  await expect(dialog.getByTestId("project-use-code")).toContainText("--theme-preset one-dark");
  await expect(dialog.getByTestId("project-use-code")).toContainText("--config-file site-config.json");
  await expect(dialog.getByTestId("project-use-code")).toContainText("--font-measurement deterministic --font-system off");
  await expect(dialog.getByTestId("project-use-host-notices")).toContainText("deterministic measurement");
  const cliFiles = await downloadFiles(page, dialog);
  expect(cliFiles["render.py"]).toContain("subprocess.run");
  expect(cliFiles["README.txt"]).toContain("--features math");
  await dialog.getByRole("tab", { name: "Web", exact: true }).click();
  await expect(dialog.getByTestId("project-use-code")).toContainText("createBrowserTextMeasurementSession");
  await dialog.getByRole("navigation", { name: "Project files", exact: true }).getByRole("button", { name: "options.json", exact: true }).click();
  await dialog.getByRole("button", { name: "Copy options.json", exact: true }).click();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  const options = JSON.parse(copied);
  expect(options.theme).toEqual({ preset: "one-dark" });
  expect(options.site_config.theme).toBe("forest");
  expect(options.site_config.flowchart).toBeUndefined();
  const webFiles = await downloadFiles(page, dialog);
  expect(webFiles["main.js"]).toContain("assertNavigableSvgForDom(result.svg)");
  expect(webFiles["main.js"]).toContain("prepareNavigableSvgForDomMount(admission, root, document)");
  expect(webFiles["main.js"]).not.toContain("innerHTML");
  expect(JSON.parse(webFiles["handoff.json"]).playground_checkout).toHaveProperty("dirty");
  expect(webFiles["options.json"]).toBe(copied);
  expect(JSON.parse(webFiles["package.json"]).dependencies["@mermanjs/web"]).toBe("file:../merman/platforms/web/packages/full");
  expect(webFiles["index.html"]).toContain("./main.js");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(page.getByTestId("project-use-launcher")).toBeFocused();
  errors.assertNone();
});

test("standalone exploration recipes retain their raw text and theme-only is a separate example", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const recipe = await readFile(new URL("../src/themes/recipes/atlas.theme.json", import.meta.url), "utf8");
  const dialog = await open(page, { ...workspace, themePresetId: null, themeRecipeJson: recipe });
  const current = await downloadFiles(page, dialog);
  expect(current["theme.json"]).toBe(recipe);
  expect(current["options.json"]).toContain(`"theme":${recipe}`);
  expect(current["src/main.rs"]).toContain("compile_recipe");
  expect(current["src/main.rs"]).not.toContain('from_id("atlas")');
  await dialog.getByRole("radio", { name: /^Theme only/ }).check();
  await dialog.getByRole("tab", { name: "Web", exact: true }).click();
  const theme = await downloadFiles(page, dialog);
  expect(theme["theme.json"]).toBe(recipe);
  expect(theme["source.mmd"]).toBeUndefined();
  expect(theme["playground-config.json"]).toBeUndefined();
  expect(theme["diagram.mmd"]).not.toContain("Captured source");
  expect(JSON.parse(theme["options.json"]).site_config.theme).toBe("default");
  expect(JSON.parse(theme["handoff.json"]).publication_id).toBeNull();
  expect(theme["main.js"]).toContain("renderSvgResult(source, optionsJson)");
  await expect(dialog.getByTestId("project-use-config-provenance")).toContainText("source init directive");
  errors.assertNone();
});

test("unknown preset fallback blocks code and download in both scopes", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const dialog = await open(page, { ...workspace, themePresetId: "atlas" });
  for (const scope of ["Reproduce current diagram", "Theme only"]) {
    await dialog.getByRole("radio", { name: new RegExp(`^${scope}`) }).check();
    await expect(dialog.getByTestId("project-use-generation-error")).toContainText("not confirmed available");
    await expect(dialog.getByTestId("project-use-code")).toHaveCount(0);
    await expect(dialog.getByRole("button", { name: "Download all files (.zip)", exact: true })).toBeDisabled();
  }
  errors.assertNone();
});

test("invalid recipes cannot be adopted in theme-only code while valid recipes survive diagram errors", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.addInitScript(() => {
    if (window === window.top && (location.protocol === "http:" || location.protocol === "https:")) localStorage.setItem("merman-language", "en");
  });
  const invalid = '{"schema_version":1,"kind":"complete_spec","complete_spec":{"not_a_theme_field":true}}';
  await page.goto(`./${encodeShareHash({ ...workspace, themePresetId: null, themeRecipeJson: invalid })}`, { waitUntil: "domcontentloaded" });
  await expect(page.locator("footer")).toContainText("Ready");
  await page.getByTestId("project-use-launcher").click();
  const dialog = page.getByTestId("project-use-dialog");
  await dialog.getByRole("radio", { name: /^Theme only/ }).check();
  for (const target of ["Rust", "CLI", "Web"]) {
    await dialog.getByRole("tab", { name: target, exact: true }).click();
    await expect(dialog.getByTestId("project-use-generation-error")).toContainText("not_a_theme_field");
    await expect(dialog.getByRole("button", { name: "Download all files (.zip)", exact: true })).toBeDisabled();
  }
  await page.keyboard.press("Escape");
  const recipe = await readFile(new URL("../src/themes/recipes/atlas.theme.json", import.meta.url), "utf8");
  // Share hydration is a startup operation; a hash-only navigation retains the live workspace.
  await page.goto("about:blank");
  await page.goto(`./${encodeShareHash({ ...workspace, code: "flowchart LR\nA -->", themePresetId: null, themeRecipeJson: recipe })}`, { waitUntil: "domcontentloaded" });
  await expect(page.locator("footer")).toContainText("Ready");
  await page.getByTestId("project-use-launcher").click();
  await dialog.getByRole("radio", { name: /^Theme only/ }).check();
  await expect(dialog.getByRole("button", { name: "Download all files (.zip)", exact: true })).toBeEnabled();
  expect((await downloadFiles(page, dialog))["theme.json"]).toBe(recipe);
  errors.assertNone();
});

test("capturing again uses new source and the dialog fits a narrow viewport", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  let dialog = await open(page);
  await expectNoDocumentOverflow(page);
  const first = await downloadFiles(page, dialog);
  await page.keyboard.press("Escape");
  await page.getByRole("tab", { name: "Editor", exact: true }).click();
  const source = "flowchart LR\nNew[New source] --> Captured[Next snapshot]";
  await replaceEditorSource(page, source);
  await page.getByRole("tab", { name: "Preview", exact: true }).click();
  await waitForPreviewSvg(page);
  await page.getByTestId("project-use-launcher").click();
  dialog = page.getByTestId("project-use-dialog");
  // Capture again also recovers a snapshot opened during render debounce.
  if (await dialog.getByTestId("project-use-generation-error").count()) {
    await expect.poll(async () => {
      await dialog.getByRole("button", { name: "Capture again", exact: true }).click();
      return dialog.getByTestId("project-use-generation-error").count();
    }).toBe(0);
  }
  const next = await downloadFiles(page, dialog);
  expect(next["source.mmd"]).toBe(source);
  expect(next["source.mmd"]).not.toBe(first["source.mmd"]);
  await expectNoDocumentOverflow(page);
  await page.screenshot({ path: test.info().outputPath("project-use-mobile.png") });
  await page.keyboard.press("Escape");
  errors.assertNone();
});
