import { expect, test } from "@playwright/test";

import { openPlayground } from "./helpers/playground";

interface CspViolationRecord {
  readonly blockedUri: string;
  readonly effectiveDirective: string;
}

declare global {
  interface Window {
    readonly __mermanCspViolations: CspViolationRecord[];
  }
}

test("production CSP blocks resources outside owned browser boundaries", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const records: CspViolationRecord[] = [];
    window.addEventListener("securitypolicyviolation", (event) => {
      records.push({
        blockedUri: event.blockedURI,
        effectiveDirective: event.effectiveDirective,
      });
    });
    Object.defineProperty(window, "__mermanCspViolations", {
      configurable: false,
      value: records,
      writable: false,
    });
  });

  await openPlayground(page);
  const originalBaseUri = await page.evaluate(() => document.baseURI);
  const externalRouteHits: string[] = [];
  await page.route("https://csp.invalid/**", async (route) => {
    externalRouteHits.push(route.request().url());
    await route.abort();
  });

  const observedBaseUri = await page.evaluate(async () => {
    const externalOrigin = "https://csp.invalid";

    const script = document.createElement("script");
    script.src = `${externalOrigin}/script.js`;
    document.head.append(script);

    await fetch(`${externalOrigin}/connect`).catch(() => undefined);

    const frame = document.createElement("iframe");
    frame.src = `${externalOrigin}/frame`;
    document.body.append(frame);

    const object = document.createElement("object");
    object.data = `${externalOrigin}/object`;
    document.body.append(object);

    const base = document.createElement("base");
    base.href = `${externalOrigin}/base/`;
    document.head.append(base);

    const workerUrl = URL.createObjectURL(
      new Blob(["self.close();"], { type: "text/javascript" })
    );
    try {
      const worker = new Worker(workerUrl);
      worker.terminate();
    } catch {
      // The CSP is expected to reject this constructor.
    } finally {
      URL.revokeObjectURL(workerUrl);
    }

    return document.baseURI;
  });

  await expect
    .poll(() =>
      page.evaluate(
        () => window.__mermanCspViolations
      )
    )
    .toEqual(
      expect.arrayContaining([
        expect.objectContaining({ effectiveDirective: "script-src-elem" }),
        expect.objectContaining({ effectiveDirective: "connect-src" }),
        expect.objectContaining({ effectiveDirective: "frame-src" }),
        expect.objectContaining({ effectiveDirective: "object-src" }),
        expect.objectContaining({ effectiveDirective: "base-uri" }),
        expect.objectContaining({ effectiveDirective: "worker-src" }),
      ])
    );

  expect(observedBaseUri).toBe(originalBaseUri);
  expect(externalRouteHits).toEqual([]);
});
