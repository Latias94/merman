import { expect, test, type Locator, type Page } from "@playwright/test";

import { encodeShareHash } from "../src/lib/share.ts";

import {
  monitorBrowserErrors,
  openPlayground,
  previewSvgText,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";

test("viewport keeps a 100-event pan outside React and terminates cancelled gestures", async ({
  page,
}) => {
  await installReactCommitProbe(page);
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await waitForPreviewSvg(page);
  const viewport = primaryViewport(page);
  const positionLayer = viewport.locator(
    '[data-merman-viewport-position-layer="true"]'
  );
  const box = await viewport.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
  await waitForTwoFrames(page);
  await waitForCommitIdle(page);
  const commitsBefore = await reactCommitCount(page);

  await page.mouse.down();
  await page.mouse.move(box!.x + box!.width - 20, box!.y + box!.height - 20, {
    steps: 100,
  });
  await page.mouse.up();
  await waitForTwoFrames(page);

  await expect(positionLayer).not.toHaveCSS("transform", "none");
  expect(await reactCommitCount(page)).toBe(commitsBefore);

  for (const termination of ["pointercancel", "lostpointercapture"] as const) {
    await dispatchPointer(viewport, "pointerdown", 17, 80, 80);
    await dispatchPointer(viewport, "pointermove", 17, 110, 100);
    await waitForTwoFrames(page);
    const stoppedTransform = await positionLayer.evaluate(
      (element) => (element as HTMLElement).style.transform
    );
    await dispatchPointer(viewport, termination, 17, 110, 100);
    await dispatchPointer(viewport, "pointermove", 17, 180, 170);
    await waitForTwoFrames(page);
    expect(
      await positionLayer.evaluate(
        (element) => (element as HTMLElement).style.transform
      )
    ).toBe(stoppedTransform);
  }

  await dispatchPointer(viewport, "pointerdown", 23, 80, 80);
  await dispatchPointer(viewport, "pointermove", 23, 110, 100);
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await waitForTwoFrames(page);
  const blurredTransform = await positionLayer.evaluate(
    (element) => (element as HTMLElement).style.transform
  );
  await dispatchPointer(viewport, "pointermove", 23, 190, 180);
  await waitForTwoFrames(page);
  expect(
    await positionLayer.evaluate(
      (element) => (element as HTMLElement).style.transform
    )
  ).toBe(blurredTransform);
  await expect(viewport).toHaveAttribute("data-dragging", "false");
  errors.assertNone();
});

test("touch pan, pinch zoom, keyboard controls, and artifact auto-fit remain coherent", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await waitForPreviewSvg(page);
  const viewport = primaryViewport(page);
  const positionLayer = viewport.locator(
    '[data-merman-viewport-position-layer="true"]'
  );
  const initialZoom = await viewportZoom(viewport);

  await dispatchPointer(viewport, "pointerdown", 31, 100, 100, "touch");
  await dispatchPointer(viewport, "pointermove", 31, 145, 125, "touch");
  await dispatchPointer(viewport, "pointerup", 31, 145, 125, "touch");
  await waitForTwoFrames(page);
  await expect(positionLayer).not.toHaveCSS("transform", "none");

  await dispatchPointer(viewport, "pointerdown", 41, 120, 120, "touch");
  await dispatchPointer(viewport, "pointerdown", 42, 180, 120, "touch");
  await dispatchPointer(viewport, "pointermove", 41, 90, 120, "touch");
  await dispatchPointer(viewport, "pointermove", 42, 210, 120, "touch");
  await dispatchPointer(viewport, "pointerup", 41, 90, 120, "touch");
  await dispatchPointer(viewport, "pointerup", 42, 210, 120, "touch");
  await expect.poll(() => viewportZoom(viewport)).toBeGreaterThan(initialZoom);

  const reset = page.getByRole("button", { name: "Reset view", exact: true });
  await reset.focus();
  await page.keyboard.press("Enter");
  await expect.poll(() => viewportZoom(viewport)).toBe(1);
  await expect(positionLayer).toHaveAttribute(
    "style",
    /translate\(0px, 0px\)/u
  );

  const zoomIn = page.getByRole("button", { name: "Zoom in", exact: true });
  await zoomIn.focus();
  await page.keyboard.press("Space");
  await expect.poll(() => viewportZoom(viewport)).toBeGreaterThan(1);
  const fit = page.getByRole("button", { name: "Fit to view", exact: true });
  await fit.focus();
  await page.keyboard.press("Enter");
  await expect(positionLayer).toHaveAttribute(
    "style",
    /translate\(0px, 0px\)/u
  );

  await dispatchPointer(viewport, "pointerdown", 51, 80, 80);
  await dispatchPointer(viewport, "pointermove", 51, 160, 150);
  await dispatchPointer(viewport, "pointerup", 51, 160, 150);
  await replaceEditorSource(
    page,
    "flowchart LR\n  FreshArtifactWithLongName --> B --> C"
  );
  await expect.poll(() => previewSvgText(page)).toContain("FreshArtifactWithLongName");
  await expect(positionLayer).toHaveAttribute(
    "style",
    /translate\(0px, 0px\)/u
  );
  errors.assertNone();
});

test("Compare panes retain independent viewport transforms", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  const viewports = page.locator('[data-merman-svg-viewport="true"]');
  await expect(viewports).toHaveCount(2);
  await expect
    .poll(() =>
      viewports.evaluateAll((elements) =>
        elements.every((element) =>
          Boolean(
            element.querySelector(".preview-container > div")?.shadowRoot?.querySelector("svg")
          )
        )
      )
    )
    .toBe(true);
  const left = viewports.nth(0);
  const right = viewports.nth(1);
  const rightZoom = await viewportZoom(right);
  const rightTransform = await viewportPositionTransform(right);

  await dispatchPointer(left, "pointerdown", 61, 80, 80);
  await dispatchPointer(left, "pointermove", 61, 150, 130);
  await dispatchPointer(left, "pointerup", 61, 150, 130);
  await left.dispatchEvent("wheel", { deltaY: -400 });
  await waitForTwoFrames(page);

  expect(await viewportPositionTransform(left)).not.toBe(rightTransform);
  expect(await viewportPositionTransform(right)).toBe(rightTransform);
  expect(await viewportZoom(right)).toBe(rightZoom);
  errors.assertNone();
});

test("a current share hash is restored before the first visible publication", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await page.addInitScript(() => {
    window.localStorage.setItem("merman-language", "en");
    const publications: string[] = [];
    const originalReplaceChildren = ShadowRoot.prototype.replaceChildren;
    ShadowRoot.prototype.replaceChildren = function (...nodes) {
      originalReplaceChildren.apply(this, nodes);
      const text = this.querySelector("svg")?.textContent;
      if (text) publications.push(text);
    };
    (
      window as typeof window & {
        __MERMAN_VISIBLE_PUBLICATIONS__?: string[];
      }
    ).__MERMAN_VISIBLE_PUBLICATIONS__ = publications;
  });
  const code = "flowchart TD\n  SharedFirst --> RestoredAtomically";
  const hash = encodeShareHash({
    code,
    mermaidConfig: '{"look":"classic"}',
    diagramTheme: "forest",
    presentationThemePresetId: null,
    presentationProfileId: null,
    svgPipeline: "parity",
    textMeasurementMode: "browser",
    diagramFont: "arial",
  });
  const wasmResponse = page.waitForResponse((response) =>
    /\/assets\/merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/u.test(response.url())
  );
  await page.goto(`./#${hash}`, { waitUntil: "domcontentloaded" });
  await wasmResponse;
  await waitForPreviewSvg(page);

  const publications = await page.evaluate(
    () =>
      (
        window as typeof window & {
          __MERMAN_VISIBLE_PUBLICATIONS__?: string[];
        }
      ).__MERMAN_VISIBLE_PUBLICATIONS__ ?? []
  );
  expect(publications.length).toBeGreaterThan(0);
  expect(publications[0]).toContain("SharedFirst");
  expect(publications).not.toContainEqual(expect.stringContaining("Condition?"));
  expect(await previewSvgText(page)).toContain("SharedFirst");
  await expect(page.locator("footer")).toContainText(`${code.length} Chars`);
  errors.assertNone();
});

function primaryViewport(page: Page): Locator {
  return page.locator('[data-merman-svg-viewport="true"]').first();
}

async function viewportZoom(viewport: Locator): Promise<number> {
  return Number(await viewport.getAttribute("data-zoom"));
}

async function viewportPositionTransform(viewport: Locator): Promise<string> {
  return viewport
    .locator('[data-merman-viewport-position-layer="true"]')
    .evaluate((element) => (element as HTMLElement).style.transform);
}

async function dispatchPointer(
  viewport: Locator,
  type:
    | "pointerdown"
    | "pointermove"
    | "pointerup"
    | "pointercancel"
    | "lostpointercapture",
  pointerId: number,
  clientX: number,
  clientY: number,
  pointerType = "mouse"
): Promise<void> {
  await viewport.dispatchEvent(type, {
    bubbles: true,
    button: 0,
    buttons:
      type === "pointerup" ||
      type === "pointercancel" ||
      type === "lostpointercapture"
        ? 0
        : 1,
    clientX,
    clientY,
    pointerId,
    pointerType,
  });
}

async function waitForTwoFrames(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
      )
  );
}

async function installReactCommitProbe(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const state = { commits: 0 };
    const target = window as typeof window & {
      __MERMAN_REACT_COMMIT_PROBE__?: typeof state;
      __REACT_DEVTOOLS_GLOBAL_HOOK__?: unknown;
    };
    target.__MERMAN_REACT_COMMIT_PROBE__ = state;
    target.__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
      supportsFiber: true,
      inject: () => 1,
      onCommitFiberRoot: () => {
        state.commits += 1;
      },
      onCommitFiberUnmount: () => undefined,
    };
  });
}

async function reactCommitCount(page: Page): Promise<number> {
  return page.evaluate(
    () =>
      (
        window as typeof window & {
          __MERMAN_REACT_COMMIT_PROBE__?: { commits: number };
        }
      ).__MERMAN_REACT_COMMIT_PROBE__?.commits ?? -1
  );
}

async function waitForCommitIdle(page: Page): Promise<void> {
  let previous = await reactCommitCount(page);
  for (let stableIntervals = 0; stableIntervals < 3; ) {
    await page.waitForTimeout(50);
    const current = await reactCommitCount(page);
    if (current === previous) {
      stableIntervals += 1;
    } else {
      stableIntervals = 0;
      previous = current;
    }
  }
}
