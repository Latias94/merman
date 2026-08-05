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

  page.on("pageerror", (error) => {
    const isWebKitInspectorSecurityError =
      error.name === "SecurityError" &&
      error.message === "The operation is insecure." &&
      error.stack?.includes("web-inspector://bootstrap.js") === true;
    if (!isWebKitInspectorSecurityError) {
      messages.push(`pageerror: ${error.message}`);
    }
  });
  page.on("console", (message) => {
    if (message.type() === "error") {
      messages.push(`console: ${message.text()}`);
    }
  });

  return {
    assertNone(allowed = []) {
      const unexpected = messages.filter(
        (message) => !allowed.some((pattern) => pattern.test(message)),
      );
      expect(unexpected, unexpected.join("\n")).toEqual([]);
    },
  };
}

export async function openPlayground(page: Page): Promise<Response> {
  await page.addInitScript(() => {
    if (window === window.top) {
      window.localStorage.setItem("merman-language", "en");
    }
  });

  const wasmResponse = page.waitForResponse((response) =>
    /\/assets\/merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/.test(response.url()),
  );
  await page.goto("./", { waitUntil: "domcontentloaded" });
  return wasmResponse;
}

export async function waitForPreviewSvg(page: Page): Promise<void> {
  const host = page.locator(".preview-container > div").first();
  await expect(host).toBeVisible();
  await expect
    .poll(() =>
      host.evaluate((element) =>
        Boolean(element.shadowRoot?.querySelector("svg")),
      ),
    )
    .toBe(true);
}

export async function previewSvgText(page: Page): Promise<string> {
  const host = page.locator(".preview-container > div").first();
  return host.evaluate(
    (element) => element.shadowRoot?.querySelector("svg")?.textContent ?? "",
  );
}

export async function replaceEditorSource(
  page: Page,
  source: string,
): Promise<void> {
  await replaceMonacoText(page, "Mermaid source", source);
  await expect(page.locator("footer")).toContainText(`${source.length} Chars`);
}

export async function replaceMermaidConfig(
  page: Page,
  config: string,
): Promise<void> {
  await page.getByRole("tab", { name: "Config", exact: true }).click();
  await replaceMonacoText(page, "Mermaid configuration", config);
  await expect(page.getByText("Config OK", { exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "Code", exact: true }).click();
}

async function replaceMonacoText(
  page: Page,
  ariaLabel: string,
  text: string,
): Promise<void> {
  const input = page.getByRole("textbox", { name: ariaLabel });
  await expect(input).toBeVisible();
  await input.focus();
  const selectAll = await page.evaluate(() =>
    /Macintosh|Mac OS X|iPhone|iPad/u.test(navigator.userAgent)
      ? "Meta+A"
      : "Control+A",
  );
  await page.keyboard.press(selectAll);
  const eventWasNotCancelled = await input.evaluate((element, value) => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/plain", value);
    return element.dispatchEvent(
      new ClipboardEvent("paste", {
        bubbles: true,
        cancelable: true,
        clipboardData: clipboard,
      }),
    );
  }, text);
  expect(eventWasNotCancelled).toBe(false);
}

export async function expectNoDocumentOverflow(page: Page): Promise<void> {
  await expect
    .poll(() =>
      page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth + 1,
      ),
    )
    .toBe(true);
}

export async function playgroundResourceCounts(
  page: Page,
): Promise<PlaygroundResourceCounts> {
  return page.evaluate(() => {
    const measurementProbes = new Set<Element>(
      document.querySelectorAll("[data-merman-text-measure-probe]"),
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
        'iframe[data-merman-realm="benchmark"]',
      ).length,
      compareRealms: document.querySelectorAll(
        'iframe[data-merman-realm="compare"]',
      ).length,
      measurementProbes: measurementProbes.size,
    };
  });
}
