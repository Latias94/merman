import { expect, test } from "@playwright/test";
import { MERMAID_SYNTAX_WORKER_PROTOCOL } from "../src/editor/syntax-protocol";
import {
  monitorBrowserErrors,
  openPlayground,
  previewSvgText,
  replaceEditorSource,
  waitForPreviewSvg,
} from "./helpers/playground";

test("Monaco starts local semantic and Tree-sitter workers with staged WASM", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const timeline: {
      workers: Array<{ readonly at: number; readonly name: string }>;
    } = {
      workers: [],
    };
    Object.defineProperty(window, "__mermanStartupTimeline", {
      configurable: true,
      value: timeline,
    });

    const NativeWorker = window.Worker;
    class ObservableWorker extends NativeWorker {
      constructor(
        scriptURL: string | URL,
        options?: { readonly name?: string; readonly type?: "classic" | "module" },
      ) {
        super(scriptURL, options);
        if (
          options?.name === "merman-editor-language" ||
          options?.name === "mermaid-tree-sitter-syntax"
        ) {
          timeline.workers.push({
            at: performance.now(),
            name: options.name,
          });
        }
      }
    }
    Object.defineProperty(window, "Worker", {
      configurable: true,
      value: ObservableWorker,
      writable: true,
    });
  });

  const errors = monitorBrowserErrors(page);
  const requests: string[] = [];
  const workers: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  page.on("worker", (worker) => workers.push(worker.url()));

  await openPlayground(page);
  await expect(page.getByRole("textbox", { name: "Mermaid source" })).toBeVisible();
  await expect.poll(() => workers.some((url) => /merman-language\.worker/i.test(url))).toBe(true);
  await expect.poll(() => workers.some((url) => /mermaid-syntax\.worker/i.test(url))).toBe(true);
  const startupTimeline = await page.evaluate(() => ({
    previewPresentedAt:
      performance.getEntriesByName("merman:initial-preview-presented")[0]
        ?.startTime ?? null,
    workers: (window as unknown as Window & {
      __mermanStartupTimeline: {
        workers: Array<{ readonly at: number; readonly name: string }>;
      };
    }).__mermanStartupTimeline.workers,
  }));
  expect(startupTimeline.previewPresentedAt).not.toBeNull();
  for (const worker of startupTimeline.workers) {
    expect(worker.at, `${worker.name} started before the first preview SVG`).toBeGreaterThanOrEqual(
      startupTimeline.previewPresentedAt ?? Number.POSITIVE_INFINITY,
    );
  }
  await expect
    .poll(() => requests.some((url) => /merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/.test(url)))
    .toBe(true);
  await expect
    .poll(() => requests.some((url) => /tree-sitter-mermaid-[\w-]+\.wasm(?:\?|$)/.test(url)))
    .toBe(true);
  await expect
    .poll(() => requests.some((url) => /web-tree-sitter-[\w-]+\.wasm(?:\?|$)/.test(url)))
    .toBe(true);
  expect(workers.some((url) => /json\.worker/i.test(url))).toBe(false);

  await replaceEditorSource(page, 'flowchart TD\n  A["hello"] -->');
  await expect
    .poll(() =>
      page.locator(".monaco-editor").first().evaluate((editor) => {
        const spans = [...editor.querySelectorAll(".view-line span")];
        const color = (text: string) => {
          const element = spans.find((candidate) => candidate.textContent === text);
          return element ? getComputedStyle(element).color : null;
        };
        const plain = color("TD");
        const keyword = color("flowchart");
        const string = color('"hello"');
        return Boolean(
          plain &&
            keyword &&
            string &&
            keyword !== plain &&
            string !== plain &&
            keyword !== string,
        );
      }),
    )
    .toBe(true);
  await expect
    .poll(() => page.locator(".monaco-editor .squiggly-error").count())
    .toBeGreaterThan(0);

  await page.getByRole("tab", { name: "Config", exact: true }).click();
  await expect.poll(() => workers.some((url) => /json\.worker/i.test(url))).toBe(true);

  const pageOrigin = new URL(page.url()).origin;
  const external = requests.filter((url) => {
    const parsed = new URL(url);
    return ["http:", "https:"].includes(parsed.protocol) && parsed.origin !== pageOrigin;
  });
  expect(external).toEqual([]);
  for (const workerUrl of workers) expect(new URL(workerUrl).origin).toBe(pageOrigin);
  errors.assertNone();
});

test("editor shell is usable before a blocked preview and intent starts language workers", async ({
  page,
}) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("merman-language", "en");
  });
  const errors = monitorBrowserErrors(page);
  let releaseWasm: () => void = () => undefined;
  const wasmBlocked = new Promise<void>((resolve) => {
    releaseWasm = resolve;
  });
  const requests: string[] = [];
  const workers: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  page.on("worker", (worker) => workers.push(worker.url()));
  await page.route(/merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/i, async (route) => {
    await wasmBlocked;
    await route.continue();
  });

  try {
    await page.goto("./", { waitUntil: "domcontentloaded" });
    const editor = page.getByRole("textbox", { name: "Mermaid source" });
    await expect(editor).toBeVisible();
    expect(workers.filter((url) => /merman-language|mermaid-syntax/i.test(url))).toEqual([]);
    await expect
      .poll(() =>
        requests.some((url) =>
          /monaco|floatingMenu|contribution-[\w-]+\.js|codicon|editor\.worker/i.test(
            url,
          ),
        ),
      )
      .toBe(true);
    expect(
      await page.evaluate(
        () =>
          performance.getEntriesByName("merman:initial-preview-presented")
            .length,
      ),
    ).toBe(0);

    await replaceEditorSource(
      page,
      "flowchart LR\n  early[Early editor] --> latest[Latest source]",
    );
    await expect
      .poll(() => workers.some((url) => /merman-language\.worker/i.test(url)))
      .toBe(true);
    await expect
      .poll(() => workers.some((url) => /mermaid-syntax\.worker/i.test(url)))
      .toBe(true);
    expect(
      await page.evaluate(
        () =>
          performance.getEntriesByName("merman:initial-preview-presented")
            .length,
      ),
    ).toBe(0);
  } finally {
    releaseWasm();
  }

  await expect
    .poll(() =>
      page.evaluate(
        () =>
          performance.getEntriesByName("merman:initial-preview-presented")
            .length,
      ),
    )
    .toBe(1);
  await waitForPreviewSvg(page);
  await expect.poll(() => previewSvgText(page)).toContain("Latest source");
  await expect(
    page.getByText("Language tools initializing", { exact: true }),
  ).toBeHidden();
  errors.assertNone();
});

test("Tree-sitter worker returns version-bound tokens for incomplete emoji input", async ({
  page,
}) => {
  let syntaxWorkerUrl = "";
  page.on("worker", (worker) => {
    if (/mermaid-syntax\.worker/i.test(worker.url())) syntaxWorkerUrl = worker.url();
  });
  await openPlayground(page);
  await expect.poll(() => syntaxWorkerUrl.length > 0).toBe(true);

  const result = await page.evaluate(
    async ({ protocol, workerUrl }) => {
      interface ResponseMessage {
        readonly code?: string;
        readonly data?: Uint32Array;
        readonly message?: string;
        readonly requestId: number;
        readonly type: string;
        readonly uri?: string;
        readonly version?: number;
      }
      const worker = new Worker(workerUrl, {
        name: "mermaid-tree-sitter-smoke",
        type: "module",
      });
      let requestId = 0;
      const request = (type: string, fields: Record<string, unknown> = {}) =>
        new Promise<ResponseMessage>((resolve, reject) => {
          requestId += 1;
          const expectedId = requestId;
          const timeout = window.setTimeout(
            () => reject(new Error(`Syntax worker timed out for ${type}.`)),
            10_000,
          );
          worker.addEventListener(
            "message",
            (event: MessageEvent<ResponseMessage>) => {
              if (event.data.requestId !== expectedId) return;
              window.clearTimeout(timeout);
              resolve(event.data);
            },
            { once: true },
          );
          worker.postMessage({ protocol, requestId: expectedId, type, ...fields });
        });
      const uri = "file:///merman/tree-sitter-smoke.mmd";
      const source = "%% 😀\r\nflowchart TD\r\nA -->\r\nC --> D";
      try {
        const ready = await request("initialize");
        if (ready.type !== "ready") throw new Error("Syntax worker did not initialize.");
        await request("didOpen", { document: { uri, version: 1, source } });
        await request("didChange", {
          document: { uri, version: 2, source: source.replace("D", "Delta") },
        });
        const stale = await request("highlights", { uri, version: 1 });
        const current = await request("highlights", { uri, version: 2 });
        if (current.type !== "highlights" || !(current.data instanceof Uint32Array)) {
          throw new Error("Syntax worker returned no packed highlights.");
        }
        const decoded: Array<{ line: number; start: number; length: number }> = [];
        let line = 0;
        let start = 0;
        for (let index = 0; index < current.data.length; index += 5) {
          const deltaLine = current.data[index] ?? 0;
          line += deltaLine;
          start = deltaLine === 0 ? start + (current.data[index + 1] ?? 0) : (current.data[index + 1] ?? 0);
          decoded.push({ line, start, length: current.data[index + 2] ?? 0 });
        }
        return {
          staleCode: stale.code,
          tokenCount: decoded.length,
          hasLaterSibling: decoded.some((token) => token.line === 3),
          validRanges: decoded.every(
            (token, index) =>
              token.length > 0 &&
              (index === 0 ||
                token.line > (decoded[index - 1]?.line ?? -1) ||
                token.start >=
                  (decoded[index - 1]?.start ?? 0) + (decoded[index - 1]?.length ?? 0)),
          ),
        };
      } finally {
        worker.postMessage({ protocol, type: "dispose" });
        worker.terminate();
      }
    },
    { protocol: MERMAID_SYNTAX_WORKER_PROTOCOL, workerUrl: syntaxWorkerUrl },
  );

  expect(result.staleCode).toBe("STALE_DOCUMENT");
  expect(result.tokenCount).toBeGreaterThan(0);
  expect(result.hasLaterSibling).toBe(true);
  expect(result.validRanges).toBe(true);
});

test("semantic worker failure keeps Tree-sitter editing alive and Retry reconnects", async ({
  page,
}) => {
  let rejectSemanticWorker = true;
  let resumeSemanticWorker: (() => void) | undefined;
  const workers: string[] = [];
  page.on("worker", (worker) => workers.push(worker.url()));
  await page.route(/merman-language\.worker[^/]*\.js(?:\?|$)/i, async (route) => {
    if (rejectSemanticWorker) return route.abort("failed");
    await new Promise<void>((resolve) => {
      resumeSemanticWorker = resolve;
    });
    await route.continue();
  });

  await openPlayground(page);
  const editor = page.locator(".monaco-editor").first();
  await expect(editor).toBeVisible();
  await expect.poll(() => workers.some((url) => /mermaid-syntax\.worker/i.test(url))).toBe(true);
  await replaceEditorSource(page, "flowchart TD\n  Alpha -->");
  await expect(editor.locator(".view-line").filter({ hasText: "Alpha" })).toBeVisible();

  const retry = page.getByRole("button", { name: "Retry", exact: true });
  await expect(retry).toBeVisible();
  rejectSemanticWorker = false;
  await retry.click();
  await expect.poll(() => Boolean(resumeSemanticWorker)).toBe(true);
  resumeSemanticWorker?.();
  await expect(retry).toBeHidden();
  await expect
    .poll(() => page.locator(".monaco-editor .squiggly-error").count())
    .toBeGreaterThan(0);
});

test("Tree-sitter worker failure preserves diagnostics and Retry reconnects", async ({ page }) => {
  let rejectSyntaxWorker = true;
  const workers: string[] = [];
  page.on("worker", (worker) => workers.push(worker.url()));
  await page.route(/mermaid-syntax\.worker[^/]*\.js(?:\?|$)/i, async (route) => {
    if (rejectSyntaxWorker) return route.abort("failed");
    return route.continue();
  });
  await openPlayground(page);
  const retry = page.getByRole("button", { name: "Retry", exact: true });
  await expect(retry).toBeVisible();
  await replaceEditorSource(page, "flowchart TD\n  Alpha -->");
  await expect
    .poll(() => page.locator(".monaco-editor .squiggly-error").count())
    .toBeGreaterThan(0);

  rejectSyntaxWorker = false;
  await retry.click();
  await expect(retry).toBeHidden();
  await expect.poll(() => workers.some((url) => /mermaid-syntax\.worker/i.test(url))).toBe(true);
  await replaceEditorSource(page, 'flowchart TD\n  A["hello"] -->');
  await expect
    .poll(() =>
      page.locator(".monaco-editor").first().evaluate((editor) => {
        const spans = [...editor.querySelectorAll(".view-line span")];
        const color = (text: string) => {
          const element = spans.find((candidate) => candidate.textContent === text);
          return element ? getComputedStyle(element).color : null;
        };
        const plain = color("TD");
        const keyword = color("flowchart");
        const string = color('"hello"');
        return Boolean(
          plain &&
            keyword &&
            string &&
            keyword !== plain &&
            string !== plain &&
            keyword !== string,
        );
      }),
    )
    .toBe(true);
});

test("an idle semantic worker failure exposes Retry without another request", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const NativeWorker = window.Worker;
    class ObservableWorker extends NativeWorker {
      constructor(scriptURL: string | URL, options?: { readonly name?: string }) {
        super(scriptURL, options);
        if (options?.name === "merman-editor-language") {
          Object.defineProperty(window, "__mermanLanguageWorker", {
            configurable: true,
            value: this,
          });
        }
      }
    }
    Object.defineProperty(window, "Worker", {
      configurable: true,
      value: ObservableWorker,
      writable: true,
    });
  });

  await openPlayground(page);
  await expect(page.getByText("Language tools initializing", { exact: true })).toBeHidden();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          "__mermanLanguageWorker" in window &&
          Boolean((window as Window & { __mermanLanguageWorker?: Worker }).__mermanLanguageWorker),
      ),
    )
    .toBe(true);
  await page.evaluate(() => {
    const worker = (window as Window & { __mermanLanguageWorker?: Worker }).__mermanLanguageWorker;
    if (!worker) throw new Error("The Merman language worker was not captured.");
    worker.dispatchEvent(new ErrorEvent("error", { message: "injected idle failure" }));
  });
  await expect(page.getByRole("button", { name: "Retry", exact: true })).toBeVisible();
  await expect(page.getByText(/injected idle failure/i)).toBeVisible();
});

test("an invalid F2 rename is request-local and diagnostics continue", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(page, "flowchart TD\n  Alpha --> Beta");
  const editorInput = page.getByRole("textbox", { name: "Mermaid source" });
  await editorInput.focus();
  await editorInput.press("ArrowLeft");
  await page.keyboard.press("F2");
  const renameInput = page.locator(".monaco-editor .rename-input");
  await expect(renameInput).toBeVisible();
  await renameInput.fill("bad name");
  await renameInput.press("Enter");
  await expect(
    page.getByRole("alert").filter({ hasText: /new name.*MERMAN_INVALID_ARGUMENT/i }),
  ).toBeVisible();
  await expect(page.getByText(/Language tools unavailable/i)).toHaveCount(0);
  await replaceEditorSource(page, "flowchart TD\n  Alpha -->");
  await expect
    .poll(() => page.locator(".monaco-editor .squiggly-error").count())
    .toBeGreaterThan(0);
  errors.assertNone();
});

test("the trusted Merman Benchmark entry cannot reach Monaco", async ({ page }) => {
  const requests: string[] = [];
  const workers: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  page.on("worker", (worker) => workers.push(worker.url()));
  await page.goto("./benchmark.html", { waitUntil: "networkidle" });
  expect(requests.filter((url) => /monaco|editor\.worker|json\.worker/i.test(url))).toEqual([]);
  expect(workers.filter((url) => /monaco|editor\.worker|json\.worker/i.test(url))).toEqual([]);
});
