import { expect, test, type Locator, type Page } from "@playwright/test";

import { createIssueShareUrl } from "../src/lib/share-view.ts";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot.ts";

import {
  expectNoDocumentOverflow,
  monitorBrowserErrors,
  openPlayground,
  previewSvgText,
  replaceEditorSource,
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

  const viewBoxToggle = page.getByRole("button", {
    name: "ViewBox Frame",
    exact: true,
  });
  await viewBoxToggle.scrollIntoViewIfNeeded();
  await expectInsideViewport(page, viewBoxToggle);
  await viewBoxToggle.tap();
  await expect(viewBoxToggle).toHaveAttribute("aria-pressed", "true");
  await expect(primaryViewport(page)).toHaveAttribute(
    "data-svg-presentation-mode",
    "viewbox",
  );

  const boundsToggle = page.getByTestId("svg-bounds-toggle");
  await boundsToggle.scrollIntoViewIfNeeded();
  await expect(boundsToggle).toBeVisible();
  await expect(boundsToggle).toHaveAccessibleName("Show SVG Bounds");
  await expect(boundsToggle).toHaveAttribute("aria-pressed", "false");
  await boundsToggle.tap();
  await expect(boundsToggle).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator('[data-merman-svg-bounds="true"]')).toHaveCount(1);

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

test("hidden editor workspace defers Preview rendering and resumes the latest source", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 568 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);

  const host = page.locator(".preview-container > div").first();
  const editorTab = page.getByRole("tab", { name: "Editor", exact: true });
  const previewTab = page.getByRole("tab", { name: "Preview", exact: true });
  await expect(page.getByRole("textbox", { name: "Mermaid source" })).toBeVisible();
  await expect(page.locator("footer")).toContainText("WASM: Ready");
  await page.waitForTimeout(450);
  await expect(host).toHaveCount(0);
  expect(
    await page.evaluate(
      () =>
        performance.getEntriesByName("merman:initial-preview-presented").length,
    ),
  ).toBe(0);

  await replaceEditorSource(
    page,
    "flowchart LR\n  hidden[Hidden] --> latest[Latest source]",
  );
  await page.waitForTimeout(450);
  await expect(host).toHaveCount(0);

  await previewTab.tap();
  await waitForPreviewSvg(page);
  await expect.poll(() => previewSvgText(page)).toContain("Latest source");
  expect(
    await page.evaluate(
      () =>
        performance.getEntriesByName("merman:initial-preview-presented").length,
    ),
  ).toBe(1);

  await editorTab.tap();
  await replaceEditorSource(
    page,
    "flowchart LR\n  hidden[Hidden] --> newest[Newest source]",
  );
  await page.waitForTimeout(450);
  expect(await previewSvgText(page)).toContain("Latest source");
  expect(await previewSvgText(page)).not.toContain("Newest source");

  await previewTab.tap();
  await expect.poll(() => previewSvgText(page)).toContain("Newest source");
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

test("landscape issue sharing keeps presentation, Bounds, and link actions reachable", async ({
  page,
}) => {
  await page.setViewportSize({ width: 844, height: 390 });
  await page.addInitScript(() => {
    if (window.top !== window) return;
    window.localStorage.setItem("merman-language", "en");
  });
  const errors = monitorBrowserErrors(page);
  const shared = new URL(
    createIssueShareUrl(
      {
        ...DEFAULT_WORKSPACE_SNAPSHOT,
        code: "flowchart TD\n  Mobile --> Shared",
      },
      {
        workspacePane: "preview",
        editorMode: "code",
        previewMode: "compare",
        showSvgBounds: true,
        svgPresentationMode: "viewbox",
      },
      { origin: "https://example.test", pathname: "/" },
    ),
  );
  await page.goto(`./${shared.search}${shared.hash}`, {
    waitUntil: "domcontentloaded",
  });
  await waitForPreviewSvg(page);

  const boundsToggle = page.getByTestId("svg-bounds-toggle");
  const viewBoxToggle = page.getByRole("button", {
    name: "ViewBox Frame",
    exact: true,
  });
  await expectInsideViewport(page, viewBoxToggle);
  await expect(viewBoxToggle).toHaveAttribute("aria-pressed", "true");
  await expectInsideViewport(page, boundsToggle);
  await expect(boundsToggle).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator('[data-merman-svg-bounds="true"]')).toHaveCount(2);

  const share = page.getByRole("button", { name: "Share", exact: true });
  await expectInsideViewport(page, share);
  await share.tap();
  await expect(
    page.getByRole("menuitem", { name: "Copy workspace link", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("menuitem", {
      name: "Copy issue reproduction link",
      exact: true,
    }),
  ).toBeVisible();
  await page.keyboard.press("Escape");

  await expectNoDocumentOverflow(page);
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

test("export workbench stays reachable in portrait and safe-area landscape", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 568 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await page.getByRole("tab", { name: "Preview", exact: true }).tap();
  await waitForPreviewSvg(page);

  await page.getByRole("button", { name: "Export", exact: true }).tap();
  await page.getByRole("menuitem", { name: "Export image…" }).tap();
  const dialog = page.getByRole("dialog", { name: "Export image" });
  await expect(dialog).toBeVisible();
  await dialog.evaluate((element) =>
    Promise.all(
      element.getAnimations().map((animation) => animation.finished.catch(() => {})),
    ),
  );
  const portraitBox = await dialog.boundingBox();
  expect(portraitBox).not.toBeNull();
  expect(portraitBox!.x).toBeLessThanOrEqual(1);
  expect(portraitBox!.y).toBeLessThanOrEqual(1);
  expect(portraitBox!.width).toBeGreaterThanOrEqual(319);
  expect(portraitBox!.height).toBeGreaterThanOrEqual(567);

  await dialog.getByRole("button", { name: "PNG", exact: true }).tap();
  const preview = dialog.getByRole("img", { name: "Export preview" });
  await preview.scrollIntoViewIfNeeded();
  await expect(preview).toBeVisible();
  const download = dialog.getByRole("button", {
    name: "Download",
    exact: true,
  });
  await expectInsideViewport(page, download);
  await expectNoDocumentOverflow(page);

  await page.locator("html").evaluate((element) => {
    element.style.setProperty("--merman-safe-area-inset-left", "44px");
    element.style.setProperty("--merman-safe-area-inset-right", "12px");
  });
  await page.setViewportSize({ width: 844, height: 390 });
  const formatGroup = dialog.getByRole("group", { name: "File format" });
  await formatGroup.scrollIntoViewIfNeeded();
  await expect
    .poll(async () => (await formatGroup.boundingBox())?.x ?? -1)
    .toBeGreaterThanOrEqual(44);
  await expectInsideViewport(
    page,
    dialog.getByRole("button", { name: "Close export" }),
  );
  await expectInsideViewport(page, download);
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("WebKit mobile smoke: landscape canvas keeps Bounds and pointer handlers operable", async ({
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
  await expect(viewport).toHaveAttribute("data-preview-canvas-tone", /^(light|dark)$/u);
  const boundsToggle = page.getByTestId("svg-bounds-toggle");
  await expectInsideViewport(page, boundsToggle);
  await boundsToggle.tap();
  await expect(boundsToggle).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator('[data-merman-svg-bounds="true"]')).toHaveCount(1);
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

  const zoomBeforePinch = await viewportZoom(viewport);
  await dispatchTouch(viewport, "pointerdown", 72, startX - 30, startY);
  await dispatchTouch(viewport, "pointerdown", 73, startX + 30, startY, false);
  await dispatchTouch(viewport, "pointermove", 72, startX - 60, startY);
  await dispatchTouch(viewport, "pointermove", 73, startX + 60, startY, false);
  await dispatchTouch(viewport, "pointerup", 72, startX - 60, startY);
  await dispatchTouch(viewport, "pointerup", 73, startX + 60, startY, false);
  await expect.poll(() => viewportZoom(viewport)).toBeGreaterThan(zoomBeforePinch);
  await expect(page.locator('[data-merman-svg-bounds="true"]')).toHaveCount(1);
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("unsupported ASCII remains tappable and explains itself on mobile", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 568 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(page, "pie\n  title Mobile\n  \"A\" : 1");
  await page.getByRole("tab", { name: "Preview", exact: true }).tap();

  const asciiTab = page.getByRole("tab", { name: "ASCII", exact: true });
  await expect(asciiTab).toBeEnabled();
  await asciiTab.tap();
  await expect(asciiTab).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByText("ASCII not supported for this diagram type", { exact: true }),
  ).toBeVisible();
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("supported ASCII renders and exposes a usable copy action on mobile", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 568 });
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(page, "flowchart LR\n  Alpha --> Beta");
  await page.getByRole("tab", { name: "Preview", exact: true }).tap();

  const asciiTab = page.getByRole("tab", { name: "ASCII", exact: true });
  await asciiTab.tap();
  await expect(asciiTab).toHaveAttribute("aria-selected", "true");
  const asciiEditor = page.getByTestId("ascii-artifact-editor");
  await expect(asciiEditor).toBeVisible();
  await expect
    .poll(() => asciiEditor.locator(".view-line").allTextContents())
    .toEqual(expect.arrayContaining([expect.stringContaining("Alpha")]));

  const copyAscii = page.getByTestId("copy-ascii-button");
  await expect(copyAscii).toBeEnabled();
  await expect(copyAscii).toHaveAccessibleName("Copy ASCII");
  await copyAscii.tap();
  await expect(copyAscii).toHaveAccessibleName("Copied!");
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("Kanban ticket links remain tappable without starting a viewport gesture", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  const source = [
    "---",
    "config:",
    "  kanban:",
    "    ticketBaseUrl: 'https://example.test/browse/#TICKET#'",
    "---",
    "kanban",
    "  Todo",
    "    task[Task]@{ ticket: MC-1 }",
  ].join("\n");
  await replaceEditorSource(page, source);
  await page.getByRole("tab", { name: "Preview", exact: true }).tap();

  const viewport = primaryViewport(page);
  const anchor = viewport.locator("a.kanban-ticket-link");
  await expect(anchor).toHaveCount(1);
  await anchor.evaluate((element) => {
    const root = element.getRootNode();
    if (!(root instanceof ShadowRoot)) {
      throw new Error("Expected the ticket link inside a shadow root.");
    }
    const viewport = root.host.closest<HTMLDivElement>(
      '[data-merman-svg-viewport="true"]',
    );
    if (!viewport) throw new Error("Expected the owning SVG viewport.");

    element.setAttribute("data-test-click-count", "0");
    window.addEventListener("pointerdown", (event) => {
      if (!event.composedPath().includes(element)) return;
      element.setAttribute(
        "data-test-pointerdown-default-prevented",
        String(event.defaultPrevented),
      );
      element.setAttribute(
        "data-test-pointer-captured",
        String(viewport.hasPointerCapture(event.pointerId)),
      );
    });
    element.addEventListener("click", (event) => {
      event.preventDefault();
      element.setAttribute(
        "data-test-click-count",
        String(Number(element.getAttribute("data-test-click-count")) + 1),
      );
    });
  });

  await anchor.tap();
  await expect(anchor).toHaveAttribute("data-test-click-count", "1");
  await expect(anchor).toHaveAttribute(
    "data-test-pointerdown-default-prevented",
    "false",
  );
  await expect(anchor).toHaveAttribute("data-test-pointer-captured", "false");
  await expect(viewport).toHaveAttribute("data-dragging", "false");
  errors.assertNone();
});

test("a link-originated pinch zooms without activating the link", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(
    page,
    [
      "---",
      "config:",
      "  kanban:",
      "    ticketBaseUrl: 'https://example.test/browse/#TICKET#'",
      "---",
      "kanban",
      "  Todo",
      "    task[Task]@{ ticket: MC-3 }",
    ].join("\n"),
  );
  await page.getByRole("tab", { name: "Preview", exact: true }).tap();

  const viewport = primaryViewport(page);
  const anchor = viewport.locator("a.kanban-ticket-link");
  await expect(anchor).toHaveCount(1);
  await anchor.evaluate((element) => {
    element.setAttribute("data-test-click-count", "0");
    element.addEventListener("click", (event) => {
      event.preventDefault();
      element.setAttribute(
        "data-test-click-count",
        String(Number(element.getAttribute("data-test-click-count")) + 1),
      );
    });
  });
  const box = await anchor.boundingBox();
  expect(box).not.toBeNull();
  const centerX = box!.x + box!.width / 2;
  const centerY = box!.y + box!.height / 2;
  const initialZoom = await viewportZoom(viewport);

  await dispatchTouch(anchor, "pointerdown", 81, centerX, centerY, true);
  await dispatchTouch(
    viewport,
    "pointerdown",
    82,
    centerX + 80,
    centerY,
    false,
  );
  await dispatchTouch(anchor, "pointermove", 81, centerX - 30, centerY, true);
  await dispatchTouch(
    viewport,
    "pointermove",
    82,
    centerX + 110,
    centerY,
    false,
  );
  await dispatchTouch(anchor, "pointerup", 81, centerX - 30, centerY, true);

  // Browsers may synthesize the anchor click as soon as its pointer ends,
  // while another touch participating in the promoted gesture is still down.
  await anchor.dispatchEvent("click", {
    bubbles: true,
    cancelable: true,
    composed: true,
    detail: 1,
  });
  await expect(anchor).toHaveAttribute("data-test-click-count", "0");

  await dispatchTouch(
    viewport,
    "pointerup",
    82,
    centerX + 110,
    centerY,
    false,
  );
  await expect.poll(() => viewportZoom(viewport)).toBeGreaterThan(initialZoom);
  await anchor.tap();
  await expect(anchor).toHaveAttribute("data-test-click-count", "1");
  errors.assertNone();
});

test("XHTML label links remain tappable without starting a viewport gesture", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(
    page,
    [
      "stateDiagram-v2",
      "A",
      "note right of A",
      "  <a href='https://example.test/docs' target='_self'><code>Docs</code></a>",
      "end note",
    ].join("\n"),
  );
  await page.getByRole("tab", { name: "Preview", exact: true }).tap();

  const viewport = primaryViewport(page);
  const anchor = viewport.locator("foreignObject a");
  await expect(anchor).toHaveCount(1);
  await anchor.evaluate((element) => {
    const root = element.getRootNode();
    if (!(root instanceof ShadowRoot)) {
      throw new Error("Expected the XHTML link inside a shadow root.");
    }
    const viewport = root.host.closest<HTMLDivElement>(
      '[data-merman-svg-viewport="true"]',
    );
    if (!viewport) throw new Error("Expected the owning SVG viewport.");

    element.setAttribute("data-test-click-count", "0");
    window.addEventListener("pointerdown", (event) => {
      if (!event.composedPath().includes(element)) return;
      element.setAttribute(
        "data-test-pointerdown-default-prevented",
        String(event.defaultPrevented),
      );
      element.setAttribute(
        "data-test-pointer-captured",
        String(viewport.hasPointerCapture(event.pointerId)),
      );
    });
    element.addEventListener("click", (event) => {
      event.preventDefault();
      element.setAttribute(
        "data-test-click-count",
        String(Number(element.getAttribute("data-test-click-count")) + 1),
      );
    });
  });

  await anchor.tap();
  await expect(anchor).toHaveAttribute("data-test-click-count", "1");
  await expect(anchor).toHaveAttribute(
    "data-test-pointerdown-default-prevented",
    "false",
  );
  await expect(anchor).toHaveAttribute("data-test-pointer-captured", "false");
  await expect(viewport).toHaveAttribute("data-dragging", "false");
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
  await expect
    .poll(async () => {
      const box = await locator.boundingBox();
      const viewport = page.viewportSize();
      return Boolean(
        box &&
          viewport &&
          box.x >= 0 &&
          box.y >= 0 &&
          box.x + box.width <= viewport.width + 1 &&
          box.y + box.height <= viewport.height + 1,
      );
    })
    .toBe(true);
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
    "Share",
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
  const share = page.getByRole("button", { name: "Share", exact: true });
  await share.tap();
  await expect(
    page.getByRole("menuitem", { name: "Copy workspace link", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("menuitem", {
      name: "Copy issue reproduction link",
      exact: true,
    }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(share).toBeFocused();
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
  target: Locator,
  type: "pointerdown" | "pointermove" | "pointerup",
  pointerId: number,
  clientX: number,
  clientY: number,
  isPrimary = true,
): Promise<void> {
  await target.dispatchEvent(type, {
    bubbles: true,
    button: 0,
    buttons: type === "pointerup" ? 0 : 1,
    clientX,
    clientY,
    composed: true,
    isPrimary,
    pointerId,
    pointerType: "touch",
  });
}
