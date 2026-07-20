import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import {
  expectNoDocumentOverflow,
  monitorBrowserErrors,
  openPlayground,
  playgroundResourceCounts,
  previewSvgText,
  replaceEditorSource,
  replaceMermaidConfig,
  waitForPreviewSvg,
} from "./helpers/playground";
import { PLAYGROUND_RENDER_VIEWPORT } from "../src/runtime/render-viewport";

test("@smoke loads the production WASM and renders a safe SVG", async ({
  page,
  isMobile,
}, testInfo) => {
  const errors = monitorBrowserErrors(page);
  const wasmResponse = await openPlayground(page);

  expect(wasmResponse.ok()).toBe(true);
  expect(wasmResponse.headers()["content-type"]).toContain("application/wasm");
  expect(new URL(wasmResponse.url()).origin).toBe(new URL(page.url()).origin);
  expect(new URL(wasmResponse.url()).pathname).toMatch(
    /\/assets\/merman_wasm_bg-[\w-]+\.wasm$/,
  );
  expect(wasmResponse.url()).not.toContain("/@fs/");
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);
  const resources = await playgroundResourceCounts(page);
  expect(resources.measurementProbes).toBe(3);
  expect(resources.benchmarkRealms).toBe(0);
  expect(resources.compareRealms).toBe(0);

  const accessibility = await new AxeBuilder({ page })
    .include("#root")
    .analyze();
  await testInfo.attach("axe-baseline.json", {
    body: JSON.stringify(accessibility, null, 2),
    contentType: "application/json",
  });
  expect(accessibility.violations).toEqual([]);

  errors.assertNone();
});

test("editing the source publishes the matching SVG without page overflow", async ({
  page,
  isMobile,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  if (!isMobile) {
    await waitForPreviewSvg(page);
  }

  await replaceEditorSource(
    page,
    "flowchart LR\n  browser[Browser smoke] --> rendered[Rendered]",
  );
  await expect(page.locator("footer")).toContainText("2 Lines");
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }

  await expect.poll(() => previewSvgText(page)).toContain("Browser smoke");
  await expectNoDocumentOverflow(page);
  await expect(page.locator("header")).toBeVisible();
  await expect(page.locator("footer")).toBeVisible();
  errors.assertNone();
});

test("canonical detection clears invalid and stale diagram types", async ({
  page,
  isMobile,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);

  await replaceEditorSource(page, "flowchart LR\n  A --> B");
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await expect(page.locator("footer")).toContainText("Flowchart");

  if (isMobile) {
    await page.getByRole("tab", { name: "Editor", exact: true }).click();
  }
  await replaceEditorSource(page, "unknownDiagram\n  A --> B");
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await expect(page.locator("footer")).toContainText("Unknown");

  if (isMobile) {
    await page.getByRole("tab", { name: "Editor", exact: true }).click();
  }
  await replaceEditorSource(page, "flowchart TD\n  stale --> result");
  await replaceEditorSource(
    page,
    'gitGraph\n  commit id:"C0"\n  branch feature',
  );
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await expect(page.locator("footer")).toContainText("Git Graph");

  errors.assertNone();
});

test("Compare owns one local Mermaid realm and publishes one coherent batch", async ({
  page,
  isMobile,
}) => {
  const errors = monitorBrowserErrors(page);
  const opaqueArtifactRequests: string[] = [];
  page.on("request", (request) => {
    const pathname = new URL(request.url()).pathname;
    if (/\/assets\/opaque-realm-artifacts-[\w-]+\.js$/.test(pathname)) {
      opaqueArtifactRequests.push(request.url());
    }
  });

  await openPlayground(page);
  await replaceEditorSource(
    page,
    "flowchart LR\n  compare_start[Compare start] --> ready[Ready]",
  );
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);
  expect(opaqueArtifactRequests).toEqual([]);

  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  await expect(page.locator('iframe[data-merman-realm="compare"]')).toHaveCount(
    1,
  );
  await expect
    .poll(() => compareSvgTexts(page))
    .toEqual([
      expect.stringContaining("Compare start"),
      expect.stringContaining("Compare start"),
    ]);
  await expect.poll(() => compareRealmUsesCanonicalViewport(page)).toBe(true);
  expect(opaqueArtifactRequests).toHaveLength(1);
  for (const requestUrl of opaqueArtifactRequests) {
    expect(new URL(requestUrl).origin).toBe(new URL(page.url()).origin);
  }

  if (isMobile) {
    await page.getByRole("tab", { name: "Editor", exact: true }).click();
  }
  await replaceEditorSource(
    page,
    "flowchart LR\n  superseded[Superseded] --> stale[Stale]",
  );
  await replaceEditorSource(
    page,
    "flowchart LR\n  latest_batch[Latest batch] --> coherent[Coherent]",
  );
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
    await page.getByRole("tab", { name: "Compare", exact: true }).click();
  }

  await expect
    .poll(() => compareSvgTexts(page))
    .toEqual([
      expect.stringContaining("Latest batch"),
      expect.stringContaining("Latest batch"),
    ]);
  const texts = await compareSvgTexts(page);
  expect(texts.join(" ")).not.toContain("Superseded");
  await expect(page.locator('iframe[data-merman-realm="compare"]')).toHaveCount(
    1,
  );

  await page.evaluate(() => {
    window.dispatchEvent(
      new PageTransitionEvent("pagehide", { persisted: true }),
    );
  });
  await expect(page.locator('iframe[data-merman-realm="compare"]')).toHaveCount(
    0,
  );
  await page.evaluate(() => {
    window.dispatchEvent(
      new PageTransitionEvent("pageshow", { persisted: true }),
    );
  });
  await expect(page.locator('iframe[data-merman-realm="compare"]')).toHaveCount(
    1,
  );
  await expect
    .poll(() => compareSvgTexts(page))
    .toEqual([
      expect.stringContaining("Latest batch"),
      expect.stringContaining("Latest batch"),
    ]);
  errors.assertNone();
});

test("Compare detection and rendering share external ELK configuration", async ({
  page,
  isMobile,
}) => {
  const errors = monitorBrowserErrors(page);

  await openPlayground(page);
  await replaceMermaidConfig(page, '{"layout":"elk"}');
  await replaceEditorSource(
    page,
    "flowchart LR\n  configured_input[Configured input] --> elk_ready[ELK ready]",
  );
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);
  await page.getByRole("tab", { name: "Compare", exact: true }).click();

  await expect
    .poll(() => compareSvgTexts(page))
    .toEqual([
      expect.stringContaining("Configured input"),
      expect.stringContaining("Configured input"),
    ]);
  await expect(page.locator('iframe[data-merman-realm="compare"]')).toHaveCount(1);
  errors.assertNone();
});

test("Compare isolates container-sensitive Gantt rendering from pane width", async ({
  page,
  isMobile,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(
    page,
    [
      "gantt",
      "  title A Gantt Diagram",
      "  dateFormat  YYYY-MM-DD",
      "  section A",
      "  Task1 :a1, 2024-01-01, 1d",
    ].join("\n"),
  );
  if (isMobile) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await page.getByRole("tab", { name: "Compare", exact: true }).click();

  await expect.poll(() => mermaidCompareViewBoxWidth(page)).toBe(800);
  await expect.poll(() => mermaidGanttTickOverlapCount(page)).toBe(0);
  errors.assertNone();
});

async function compareSvgTexts(
  page: import("@playwright/test").Page,
): Promise<string[]> {
  return page
    .locator(".preview-container > div")
    .evaluateAll((hosts) =>
      hosts.map(
        (host) => host.shadowRoot?.querySelector("svg")?.textContent ?? "",
      ),
    );
}

async function compareRealmUsesCanonicalViewport(
  page: import("@playwright/test").Page,
): Promise<boolean> {
  return page.evaluate((viewport) => {
    const realm = document.querySelector('iframe[data-merman-realm="compare"]');
    if (!(realm instanceof HTMLIFrameElement)) {
      return false;
    }
    return (
      realm.clientWidth === viewport.width &&
      realm.clientHeight === viewport.height
    );
  }, PLAYGROUND_RENDER_VIEWPORT);
}

async function mermaidCompareViewBoxWidth(
  page: import("@playwright/test").Page,
): Promise<number | null> {
  return mermaidCompareHost(page).evaluate(
    (host) =>
      host.shadowRoot?.querySelector("svg")?.viewBox.baseVal.width ?? null,
  );
}

async function mermaidGanttTickOverlapCount(
  page: import("@playwright/test").Page,
): Promise<number | null> {
  return mermaidCompareHost(page).evaluate((host) => {
    const ticks = Array.from(
      host.shadowRoot?.querySelectorAll<SVGTextElement>(".tick text") ?? [],
    )
      .map((tick) => tick.getBoundingClientRect())
      .filter((rect) => rect.width > 0)
      .sort((left, right) => left.top - right.top || left.left - right.left);
    if (ticks.length < 2) return null;

    let overlaps = 0;
    for (let index = 1; index < ticks.length; index += 1) {
      const previous = ticks[index - 1];
      const current = ticks[index];
      if (
        Math.abs(previous.top - current.top) < 2 &&
        previous.right > current.left
      ) {
        overlaps += 1;
      }
    }
    return overlaps;
  });
}

function mermaidCompareHost(page: import("@playwright/test").Page) {
  return page.locator(
    '[data-merman-compare-engine="mermaid"] .preview-container > div',
  );
}
