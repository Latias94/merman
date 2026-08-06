import { expect, test, type Locator, type Page } from "@playwright/test";

import {
  expectNoDocumentOverflow,
  monitorBrowserErrors,
  openPlayground,
  waitForPreviewSvg,
} from "./helpers/playground";

test("320px portrait keeps toolbar, workspace tabs, and preview controls reachable", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 568 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);

  await expectNoDocumentOverflow(page);
  await expectHeaderControlsInsideViewport(page);
  await exerciseCompactToolbarMenus(page);

  const host = page.locator(".preview-container > div").first();
  await expect
    .poll(() =>
      host.evaluate((element) =>
        Boolean(element.shadowRoot?.querySelector("svg")),
      ),
    )
    .toBe(true);
  const editorTab = page.getByRole("tab", { name: "Editor", exact: true });
  const previewTab = page.getByRole("tab", { name: "Preview", exact: true });
  await editorTab.focus();
  await page.keyboard.press("ArrowRight");
  await expect(previewTab).toBeFocused();
  await expect(editorTab).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("Enter");
  await expect(previewTab).toHaveAttribute("aria-selected", "true");
  await waitForPreviewSvg(page);
  const previewModes = page.getByRole("tablist", {
    name: "Preview",
    exact: true,
  });
  expect(
    await previewModes.evaluate((element) => element.clientWidth),
  ).toBeGreaterThan(240);
  for (const name of ["SVG", "Compare", "Diagnostics"]) {
    const tab = previewModes.getByRole("tab", { name, exact: true });
    await tab.scrollIntoViewIfNeeded();
    await expectInsideViewport(page, tab);
  }
  await expect
    .poll(() =>
      host.evaluate(
        (element) =>
          element.shadowRoot?.querySelector("svg")?.getBoundingClientRect().width ?? 0,
      ),
    )
    .toBeGreaterThan(0);

  const viewport = primaryViewport(page);
  await waitForAnimationFrames(viewport, 2);
  const initialZoom = await viewportZoom(viewport);
  expect(initialZoom).toBeGreaterThan(0.01);
  const zoomIn = page.getByRole("button", { name: "Zoom in", exact: true });
  await expectInsideViewport(page, zoomIn);
  await zoomIn.tap();
  await expect.poll(() => viewportZoom(viewport)).toBeGreaterThan(initialZoom);
  await page.getByRole("button", { name: "Fit to view", exact: true }).tap();

  await editorTab.tap();
  await expect(
    page.getByRole("textbox", { name: "Mermaid source" }),
  ).toBeVisible();
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("mid-width layouts retain every toolbar action through compact controls", async ({
  page,
}) => {
  await page.setViewportSize({ width: 640, height: 720 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);

  for (const width of [640, 768, 900, 1024]) {
    await page.setViewportSize({ width, height: 720 });
    await expectHeaderControlsInsideViewport(page);
    await expectNoDocumentOverflow(page);
  }

  errors.assertNone();
});

test("Pixel portrait and a shortened viewport keep dialogs scrollable and dismissible", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  const initialViewport = page.viewportSize();
  expect(initialViewport).not.toBeNull();
  expect(initialViewport!.height).toBeGreaterThan(initialViewport!.width);
  const initialVisualViewportHeight = await visualViewportHeight(page);

  const examplesTrigger = page.getByRole("button", {
    name: "Examples",
    exact: true,
  });
  await examplesTrigger.tap();
  const examplesDialog = page.getByRole("dialog", { name: "Example Gallery" });
  const exampleSearch = examplesDialog.getByRole("searchbox", {
    name: "Search examples",
  });
  await expect(exampleSearch).toBeFocused();
  const closeExamples = examplesDialog.getByRole("button", {
    name: "Close example gallery",
  });
  await expectInsideViewport(page, closeExamples);
  await closeExamples.tap();
  await expect(examplesTrigger).toBeFocused();

  await page.setViewportSize({ width: initialViewport!.width, height: 360 });
  await expect
    .poll(() => visualViewportHeight(page))
    .toBeLessThan(initialVisualViewportHeight);
  const benchTrigger = page.getByRole("button", { name: "Bench", exact: true });
  await benchTrigger.tap();
  const benchDialog = page.getByRole("dialog", { name: "Browser Benchmark" });
  await expect(benchDialog).toBeVisible();
  await page.locator("html").evaluate((element) => {
    element.style.setProperty("--merman-safe-area-inset-left", "44px");
    element.style.setProperty("--merman-safe-area-inset-right", "12px");
  });
  await expect.poll(async () => (await benchDialog.boundingBox())?.x ?? -1).toBeGreaterThanOrEqual(44);
  const run = benchDialog.getByRole("button", { name: "Run", exact: true });
  await expectInsideViewport(page, run);
  const scrollOwner = benchDialog.locator('[data-slot="scroll-area-viewport"]');
  const scrollCapacity = await scrollOwner.evaluate(
    (element) => element.scrollHeight - element.clientHeight,
  );
  expect(scrollCapacity).toBeGreaterThan(0);
  const documentScrollBefore = await documentScrollTop(page);
  await scrollOwner.evaluate((element) => element.scrollTo({ top: element.scrollHeight }));
  await expect.poll(() => scrollOwner.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  expect(await documentScrollTop(page)).toBe(documentScrollBefore);
  const closeBench = benchDialog.getByRole("button", {
    name: "Close benchmark",
  });
  await expectInsideViewport(page, closeBench);
  await expectNoDocumentOverflow(page);
  await closeBench.tap();
  await expect(benchTrigger).toBeFocused();
  errors.assertNone();
});

test("landscape touch gestures and preview modes remain operable", async ({
  page,
}) => {
  await page.setViewportSize({ width: 568, height: 320 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await page.getByRole("tab", { name: "Preview", exact: true }).tap();
  await waitForPreviewSvg(page);
  await expectNoDocumentOverflow(page);
  await expectHeaderControlsInsideViewport(page);

  const viewport = primaryViewport(page);
  const positionLayer = viewport.locator(
    '[data-merman-viewport-position-layer="true"]',
  );
  const transformBefore = await positionLayer.evaluate(
    (element) => getComputedStyle(element).transform,
  );
  const box = await viewport.boundingBox();
  expect(box).not.toBeNull();
  const startX = box!.x + box!.width / 2;
  const startY = box!.y + box!.height / 2;
  await dispatchTouch(viewport, "pointerdown", 71, startX, startY);
  await dispatchTouch(viewport, "pointermove", 71, startX + 48, startY + 24);
  await dispatchTouch(viewport, "pointerup", 71, startX + 48, startY + 24);
  await expect
    .poll(() =>
      positionLayer.evaluate((element) => getComputedStyle(element).transform),
    )
    .not.toBe(transformBefore);

  const diagnostics = page.getByRole("tab", {
    name: "Diagnostics",
    exact: true,
  });
  await diagnostics.tap();
  await expect(diagnostics).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tab", { name: "Parse JSON" })).toBeVisible();
  errors.assertNone();
});

function primaryViewport(page: Page): Locator {
  return page.locator('[data-merman-svg-viewport="true"]').first();
}

async function viewportZoom(viewport: Locator): Promise<number> {
  return Number(await viewport.getAttribute("data-zoom"));
}

async function expectInsideViewport(page: Page, locator: Locator): Promise<void> {
  await expect(locator).toBeVisible();
  const box = await locator.boundingBox();
  const viewport = page.viewportSize();
  expect(box).not.toBeNull();
  expect(viewport).not.toBeNull();
  expect(box!.x).toBeGreaterThanOrEqual(0);
  expect(box!.y).toBeGreaterThanOrEqual(0);
  expect(box!.x + box!.width).toBeLessThanOrEqual(viewport!.width + 1);
  expect(box!.y + box!.height).toBeLessThanOrEqual(viewport!.height + 1);
}

async function expectHeaderControlsInsideViewport(page: Page): Promise<void> {
  for (const name of [
    "Examples",
    "Bench",
    "Theme",
    "Render settings",
    "Export",
    "Copy Link",
  ]) {
    await expectInsideViewport(
      page,
      page.getByRole("button", { name, exact: true }),
    );
  }
  if ((page.viewportSize()?.width ?? 0) >= 640) {
    await expectInsideViewport(
      page,
      page.getByRole("link", { name: "View source on GitHub", exact: true }),
    );
  }
}

async function exerciseCompactToolbarMenus(page: Page): Promise<void> {
  for (const name of ["Theme", "Render settings", "Export"]) {
    const trigger = page.getByRole("button", { name, exact: true });
    await trigger.tap();
    await expect(page.getByRole("menu")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(trigger).toBeFocused();
  }
}

async function visualViewportHeight(page: Page): Promise<number> {
  return page.evaluate(() => window.visualViewport?.height ?? window.innerHeight);
}

async function documentScrollTop(page: Page): Promise<number> {
  return page.evaluate(() => document.scrollingElement?.scrollTop ?? 0);
}

async function waitForAnimationFrames(locator: Locator, count: number): Promise<void> {
  await locator.evaluate(
    (_, frameCount) =>
      new Promise<void>((resolve) => {
        const wait = (remaining: number) => {
          if (remaining <= 0) {
            resolve();
            return;
          }
          requestAnimationFrame(() => wait(remaining - 1));
        };
        wait(frameCount);
      }),
    count,
  );
}

async function dispatchTouch(
  viewport: Locator,
  type: "pointerdown" | "pointermove" | "pointerup",
  pointerId: number,
  clientX: number,
  clientY: number,
): Promise<void> {
  await viewport.dispatchEvent(type, {
    bubbles: true,
    button: 0,
    buttons: type === "pointerup" ? 0 : 1,
    clientX,
    clientY,
    isPrimary: true,
    pointerId,
    pointerType: "touch",
  });
}
