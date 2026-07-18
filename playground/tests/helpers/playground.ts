import { expect, type Page, type Response } from "@playwright/test";

export interface BrowserErrorMonitor {
  assertNone(allowed?: readonly RegExp[]): void;
}

export interface PlaygroundResourceCounts {
  benchmarkRealms: number;
  compareRealms: number;
  measurementProbes: number;
}

export function monitorBrowserErrors(page: Page): BrowserErrorMonitor {
  const messages: string[] = [];

  page.on("pageerror", (error) => messages.push(`pageerror: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") {
      messages.push(`console: ${message.text()}`);
    }
  });

  return {
    assertNone(allowed = []) {
      const unexpected = messages.filter(
        (message) => !allowed.some((pattern) => pattern.test(message))
      );
      expect(unexpected, unexpected.join("\n")).toEqual([]);
    },
  };
}

export async function openPlayground(page: Page): Promise<Response> {
  await page.addInitScript(() => {
    window.localStorage.setItem("merman-language", "en");
  });

  const wasmResponse = page.waitForResponse((response) =>
    /\/assets\/merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/.test(response.url())
  );
  await page.goto("./", { waitUntil: "domcontentloaded" });
  return wasmResponse;
}

export async function waitForPreviewSvg(page: Page): Promise<void> {
  const host = page.locator(".preview-container > div").first();
  await expect(host).toBeVisible();
  await expect
    .poll(() =>
      host.evaluate((element) => Boolean(element.shadowRoot?.querySelector("svg")))
    )
    .toBe(true);
}

export async function previewSvgText(page: Page): Promise<string> {
  const host = page.locator(".preview-container > div").first();
  return host.evaluate(
    (element) => element.shadowRoot?.querySelector("svg")?.textContent ?? ""
  );
}

export async function replaceEditorSource(page: Page, source: string): Promise<void> {
  const input = page.getByRole("textbox", { name: "Editor content" }).first();
  await expect(input).toBeVisible();
  await input.focus();
  await page.keyboard.press("Control+A");
  await page.keyboard.press("Backspace");
  await page.keyboard.insertText(source);
}

export async function expectNoDocumentOverflow(page: Page): Promise<void> {
  await expect
    .poll(() =>
      page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth + 1
      )
    )
    .toBe(true);
}

export async function playgroundResourceCounts(
  page: Page
): Promise<PlaygroundResourceCounts> {
  return page.evaluate(() => {
    const measurementProbes = new Set<Element>(
      document.querySelectorAll("[data-merman-text-measure-probe]")
    );
    for (const element of document.body.children) {
      if (
        element.getAttribute("aria-hidden") === "true" &&
        (element instanceof HTMLElement || element instanceof SVGElement) &&
        element.style.position === "fixed" &&
        element.style.left === "-10000px"
      ) {
        measurementProbes.add(element);
      }
    }

    return {
      benchmarkRealms: document.querySelectorAll(
        'iframe[data-merman-realm="benchmark"]'
      ).length,
      compareRealms: document.querySelectorAll(
        'iframe[data-merman-realm="compare"]'
      ).length,
      measurementProbes: measurementProbes.size,
    };
  });
}
