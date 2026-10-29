import { expect, test, type Locator, type Page } from "@playwright/test";
import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";

async function openPanel(page: Page) {
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  return page.getByRole("region", { name: "Theme panel", exact: true });
}

async function start(page: Page, mermaidConfig = "{}", themePresetId: string | null = null) {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript(() => localStorage.setItem("merman-language", "en"));
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    code: 'flowchart LR\nA[Authored source] --> B\n%%{init: {"flowchart":{"look":"handDrawn"}}}%%',
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

test("preset selection preserves source and config; explicit recommendations merge immediately and survive Cancel", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const config = {
    theme: "forest", layout: "dagre", htmlLabels: false,
    flowchart: { look: "handDrawn", layout: "dagre", nodeSpacing: 42, curve: "linear" },
    sequence: { actorMargin: 72 }, themeVariables: { primaryColor: "#123456" },
  };
  const original = await start(page, JSON.stringify(config));
  await waitForPreviewSvg(page);
  const panel = await openPanel(page);
  await expect(panel).toContainText("Changes both engines immediately");
  await expect(panel).toContainText("Chosen diagram font: Mermaid default");
  await expect(panel).toContainText("Explicit config and source font settings can take precedence");
  await expect(panel).toContainText("Selecting a preset keeps your source, look and layout settings");
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
  const note = panel.locator('[data-theme-application-note="flowchart-classic-shadows"]');
  await expect(note).toContainText("Classic look");
  await expect(note).toContainText("basic shapes");
  await apply(panel);
  const selected = await workspaceFromLink(page);
  expect(selected.code).toBe(original.code);
  expect(selected.diagramTheme).toBe(original.diagramTheme);
  expect(selected.mermaidConfig).toBe(original.mermaidConfig);

  const recommended = note.getByRole("button", { name: "Use recommended settings", exact: true });
  await recommended.focus();
  await recommended.press("Enter");
  await expect(panel.getByRole("status").filter({ hasText: "Recommended settings applied" })).toBeVisible();
  const updated = await workspaceFromLink(page);
  expect(updated.code).toBe(original.code);
  expect(JSON.parse(updated.mermaidConfig)).toEqual({ ...config, flowchart: { ...config.flowchart, look: "classic" } });
  await panel.getByRole("button", { name: /^Cyberpunk/ }).click();
  await expect(panel.locator('[data-theme-application-note="flowchart-classic-glow"]')).toContainText("Classic look");
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  const cancelled = await workspaceFromLink(page);
  expect(cancelled.themePresetId).toBe("brutalist");
  expect(cancelled.code).toBe(original.code);
  expect(JSON.parse(cancelled.mermaidConfig)).toEqual(JSON.parse(updated.mermaidConfig));
  errors.assertNone();
});

test("recommended settings explain invalid config and preserve it for repair", async ({ page }) => {
  await start(page, "{");
  const panel = await openPanel(page);
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
  await panel.getByRole("button", { name: "Use recommended settings", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("Could not update Mermaid config");
  await expect(panel.getByRole("alert")).toContainText("Fix its JSON object");
  await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "failed");
  await apply(panel);
  expect((await workspaceFromLink(page)).mermaidConfig).toBe("{");
});

test("Brutalist explains unapplied shadows and target residuals without opening Advanced", async ({ page }) => {
  await start(page, "{}", "brutalist");
  await waitForPreviewSvg(page);
  const panel = await openPanel(page);
  const reasons = panel.getByTestId("theme-diagnostic-reasons");
  await expect(reasons.locator('[data-theme-reason-code="unsupported-effect"]')).toContainText("shadow or glow");
  await expect(reasons).toContainText("Review Application notes");
  const targets = panel.getByTestId("theme-target-reasons");
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
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeDisabled();
  await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(panel.getByTestId("theme-execution-metadata")).toHaveCount(0);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  const replacement = panel.getByRole("button", { name: /^Cyberpunk/ });
  await expect(replacement).toHaveAttribute("data-theme-treatment", "base_only");
  await expect(replacement).toBeEnabled();
  await replacement.click();
  await expect(replacement).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByRole("alert")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeEnabled();
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
});
