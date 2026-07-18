import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";

const TOKEN = "t".repeat(43);
const BOOT_NONCE = "b".repeat(43);
const RUN_TOKEN = "r".repeat(43);
const PLAYGROUND_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  ".."
);
const ENGINE_GRAPH = loadBenchmarkEngineGraph();

interface BrowserBenchmarkControllerModule {
  createBrowserBenchmarkRealmSession(
    viewport: Readonly<{ height: number; width: number }>,
    signal: AbortSignal
  ): Promise<BrowserBenchmarkParentSession>;
}

interface BrowserBenchmarkParentSession {
  dispose(): void;
  sample(input: Record<string, unknown>): Promise<WireResponse>;
}

test("benchmark realm imports only Merman after START and reuses it warm", async ({
  page,
}) => {
  const requests = trackEngineRequests(page);
  await createBenchmarkHarness(page, "merman");
  expect(requests).toEqual([]);

  const cold = await sendSample(page, "merman", "realm-cold", "cold-1");
  expect(cold.type, JSON.stringify(cold, null, 2)).toBe(
    "benchmark-sample-success"
  );
  expect(cold.version).toMatch(/^0\.8\.0-alpha\./);
  expect(cold.trace.adapter_import_start).not.toBeNull();
  expect(cold.trace.engine_import_start).not.toBeNull();
  expect(cold.trace.resource_acquire_start).not.toBeNull();
  expect(cold.trace.register_start).toBeNull();
  const wasmResource = cold.resources.find((resource) =>
    ENGINE_GRAPH.mermanWasmAssets.has(new URL(resource.name).pathname)
  );
  expect(wasmResource, JSON.stringify(cold.resources, null, 2)).toBeDefined();
  expect(wasmResource!.startOffset).toBeGreaterThanOrEqual(
    cold.trace.resource_acquire_start!
  );
  expect(cold.trace.resource_acquire_end).toBeGreaterThanOrEqual(
    wasmResource!.startOffset + wasmResource!.duration
  );
  expect(await readProgressEvents(page)).toContain("resource_acquire_end");
  expect(
    requests.some((url) => ENGINE_GRAPH.mermanExclusive.has(url))
  ).toBe(true);
  expect(
    requests.filter((url) => ENGINE_GRAPH.mermaidExclusive.has(url))
  ).toEqual([]);

  const requestCount = requests.length;
  const warm = await sendSample(page, "merman", "warm", "warm-1");
  expect(warm.type).toBe("benchmark-sample-success");
  expect(warm.trace.adapter_import_start).toBeNull();
  expect(warm.trace.engine_import_start).toBeNull();
  expect(warm.trace.resource_acquire_start).toBeNull();
  expect(warm.trace.initialize_start).toBeNull();
  expect(warm.trace.render_start).not.toBeNull();
  expect(requests).toHaveLength(requestCount);
  await expect(
    sendSample(
      page,
      "merman",
      "warm",
      "warm-mutated",
      "flowchart TD\n  changed --> input"
    )
  ).rejects.toThrow(/changed frozen realm input/);

  await disposeHarness(page);
  await expect(page.locator('iframe[data-merman-realm="benchmark"]')).toHaveCount(0);
});

test("benchmark realm imports only Mermaid after START", async ({ page }) => {
  const requests = trackEngineRequests(page);
  await createBenchmarkHarness(page, "mermaid");
  expect(requests).toEqual([]);

  const cold = await sendSample(page, "mermaid", "realm-cold", "cold-1");
  expect(cold.type, JSON.stringify(cold, null, 2)).toBe(
    "benchmark-sample-success"
  );
  expect(cold.version).toBe("11.16.0");
  expect(cold.trace.engine_import_start).not.toBeNull();
  expect(cold.trace.resource_acquire_start).toBeNull();
  expect(cold.trace.register_start).not.toBeNull();
  expect(await readProgressEvents(page)).toContain("register_end");
  expect(
    requests.some((url) => ENGINE_GRAPH.mermaidExclusive.has(url))
  ).toBe(true);
  expect(
    requests.filter((url) => ENGINE_GRAPH.mermanExclusive.has(url))
  ).toEqual([]);

  await disposeHarness(page);
});

test("production parent controller drives an authenticated browser realm", async ({
  page,
}) => {
  test.setTimeout(60_000);
  const server = await createServer({
    configFile: path.join(PLAYGROUND_ROOT, "vite.config.ts"),
    root: PLAYGROUND_ROOT,
    logLevel: "error",
    server: { host: "127.0.0.1", port: 0 },
  });
  try {
    await server.listen();
    const address = server.httpServer?.address();
    if (!address || typeof address === "string") {
      throw new Error("Benchmark test server has no TCP address.");
    }
    const origin = `http://127.0.0.1:${address.port}`;
    const harnessUrl = `${origin}/benchmark-parent-harness`;
    await page.route(harnessUrl, (route) =>
      route.fulfill({
        body: "<!doctype html><html><body></body></html>",
        contentType: "text/html",
        status: 200,
      })
    );
    await page.goto(harnessUrl);
    await page.unroute(harnessUrl);

    const result = await page.evaluate(
      async ({ moduleUrl, runToken }) => {
        const controller = (await import(
          /* @vite-ignore */ moduleUrl
        )) as BrowserBenchmarkControllerModule;
        const abort = new AbortController();
        const session = await controller.createBrowserBenchmarkRealmSession(
          { width: 800, height: 600 },
          abort.signal
        );
        try {
          return await session.sample({
            runId: "browser-parent-run",
            runToken,
            requestId: "browser-parent-cold",
            engine: "merman",
            mode: "realm-cold",
            role: "measured",
            payload: {
              source: "flowchart LR\n  Parent --> Realm",
              configJson: "{}",
              theme: "default",
              diagramFont: "trebuchet",
              externalRequirements: { elkLayouts: false, zenuml: false },
              viewport: { width: 800, height: 600 },
            },
          });
        } finally {
          session.dispose();
          abort.abort();
        }
      },
      {
        moduleUrl: `${origin}/src/benchmark/realm/controller.ts`,
        runToken: RUN_TOKEN,
      }
    );

    expect(result.type, JSON.stringify(result, null, 2)).toBe(
      "benchmark-sample-success"
    );
    expect(result.version).toMatch(/^0\.8\.0-alpha\./);
    expect("svg" in result).toBe(false);
    await expect(
      page.locator('iframe[data-merman-realm="benchmark"]')
    ).toHaveCount(0);
  } finally {
    await server.close();
  }
});

test("hidden benchmark realm refuses work before creating a trace", async ({
  page,
}) => {
  const requests = trackEngineRequests(page);
  await createBenchmarkHarness(page, "merman");
  await page.evaluate(() => {
    const harness = (
      window as unknown as {
        __benchmarkHarness: { iframe: HTMLIFrameElement };
      }
    ).__benchmarkHarness;
    harness.iframe.style.display = "none";
  });

  await expect(
    sendSample(page, "merman", "realm-cold", "hidden-1")
  ).rejects.toThrow(
    /presentation host has no finite non-empty layout box/
  );
  expect(requests).toEqual([]);
  await disposeHarness(page);
});

interface WireResponse {
  readonly resources: readonly WireResourceObservation[];
  readonly type: string;
  readonly version: string | null;
  readonly trace: Record<string, number | null>;
}

interface WireResourceObservation {
  readonly duration: number;
  readonly name: string;
  readonly startOffset: number;
}

function trackEngineRequests(page: Page): string[] {
  const requests: string[] = [];
  page.on("request", (request) => {
    const pathname = new URL(request.url()).pathname;
    if (ENGINE_GRAPH.allEngineAssets.has(pathname)) {
      requests.push(pathname);
    }
  });
  return requests;
}

interface ViteManifestChunk {
  readonly assets?: readonly string[];
  readonly dynamicImports?: readonly string[];
  readonly file: string;
  readonly imports?: readonly string[];
  readonly isEntry?: boolean;
  readonly src?: string;
}

type ViteManifest = Record<string, ViteManifestChunk>;

function loadBenchmarkEngineGraph() {
  const manifest = JSON.parse(
    readFileSync(
      path.join(PLAYGROUND_ROOT, "dist", ".vite", "manifest.json"),
      "utf8"
    )
  ) as ViteManifest;
  const benchmarkEntry = requireManifestKey(
    manifest,
    (key, chunk) => key === "benchmark.html" || chunk.src === "benchmark.html",
    "benchmark entry"
  );
  const mermanAdapter = requireManifestKey(
    manifest,
    (key, chunk) =>
      `${key}\n${chunk.src ?? ""}`.includes(
        "src/benchmark/realm/engines/merman.ts"
      ),
    "Merman benchmark adapter"
  );
  const mermaidAdapter = requireManifestKey(
    manifest,
    (key, chunk) =>
      `${key}\n${chunk.src ?? ""}`.includes(
        "src/benchmark/realm/engines/mermaid.ts"
      ),
    "Mermaid benchmark adapter"
  );
  const staticAssets = manifestAssetPaths(
    manifest,
    collectManifestClosure(manifest, [benchmarkEntry], false)
  );
  const mermanAssets = manifestAssetPaths(
    manifest,
    collectOperationClosure(manifest, [mermanAdapter])
  );
  const mermaidAssets = manifestAssetPaths(
    manifest,
    collectOperationClosure(manifest, [mermaidAdapter])
  );
  const allEngineAssets = difference(
    new Set([...mermanAssets, ...mermaidAssets]),
    staticAssets
  );
  const mermanExclusive = difference(
    difference(mermanAssets, mermaidAssets),
    staticAssets
  );
  const mermaidExclusive = difference(
    difference(mermaidAssets, mermanAssets),
    staticAssets
  );
  const mermanWasmAssets = new Set(
    [...mermanExclusive].filter((asset) => asset.endsWith(".wasm"))
  );
  if (
    allEngineAssets.size === 0 ||
    mermanExclusive.size === 0 ||
    mermaidExclusive.size === 0 ||
    mermanWasmAssets.size !== 1
  ) {
    throw new Error("Benchmark production manifest has incomplete engine assets.");
  }
  return {
    allEngineAssets,
    mermanExclusive,
    mermanWasmAssets,
    mermaidExclusive,
  };
}

function requireManifestKey(
  manifest: ViteManifest,
  predicate: (key: string, chunk: ViteManifestChunk) => boolean,
  label: string
): string {
  const matches = Object.entries(manifest).filter(([key, chunk]) =>
    predicate(key, chunk)
  );
  if (matches.length !== 1) {
    throw new Error(`Expected exactly one ${label}, found ${matches.length}.`);
  }
  return matches[0][0];
}

function collectManifestClosure(
  manifest: ViteManifest,
  roots: Iterable<string>,
  includeDynamic: boolean
): Set<string> {
  const visited = new Set<string>();
  const pending = [...roots];
  while (pending.length > 0) {
    const key = pending.pop();
    if (!key || visited.has(key)) continue;
    const chunk = manifest[key];
    if (!chunk) throw new Error(`Manifest references unknown chunk ${key}.`);
    visited.add(key);
    pending.push(...(chunk.imports ?? []));
    if (includeDynamic) pending.push(...(chunk.dynamicImports ?? []));
  }
  return visited;
}

function collectOperationClosure(
  manifest: ViteManifest,
  roots: Iterable<string>
): Set<string> {
  const visited = new Set<string>();
  const pending = [...roots];
  while (pending.length > 0) {
    const key = pending.pop();
    if (!key || visited.has(key)) continue;
    const chunk = manifest[key];
    if (!chunk) throw new Error(`Manifest references unknown chunk ${key}.`);
    visited.add(key);
    pending.push(...(chunk.imports ?? []));
    if (chunk.isEntry !== true) {
      pending.push(...(chunk.dynamicImports ?? []));
    }
  }
  return visited;
}

function manifestAssetPaths(
  manifest: ViteManifest,
  keys: Iterable<string>
): Set<string> {
  const assets = new Set<string>();
  for (const key of keys) {
    const chunk = manifest[key];
    assets.add(`/merman/${chunk.file}`);
    for (const asset of chunk.assets ?? []) {
      assets.add(`/merman/${asset}`);
    }
  }
  return assets;
}

function difference(left: Set<string>, right: Set<string>): Set<string> {
  return new Set([...left].filter((value) => !right.has(value)));
}

async function createBenchmarkHarness(
  page: Page,
  engine: "merman" | "mermaid"
): Promise<void> {
  const harnessRoute = "**/merman/benchmark-harness";
  await page.route(harnessRoute, (route) =>
    route.fulfill({
      body: "<!doctype html><html><body></body></html>",
      contentType: "text/html",
      status: 200,
    })
  );
  await page.goto("./benchmark-harness");
  await page.unroute(harnessRoute);
  await page.evaluate(
    async ({ bootNonce, engine, token }) => {
      const realmId = `browser-${engine}`;
      const boot = {
        kind: "benchmark" as const,
        realmId,
        bootNonce,
      };
      const identity = {
        kind: "benchmark" as const,
        realmId,
        realmToken: token,
      };
      const url = new URL("benchmark.html", window.location.href);
      url.hash = new URLSearchParams({
        protocol: "1",
        kind: "benchmark",
        realm: realmId,
        boot: bootNonce,
      }).toString();

      const iframe = document.createElement("iframe");
      iframe.dataset.mermanRealm = "benchmark";
      iframe.style.position = "fixed";
      iframe.style.left = "-10000px";
      iframe.style.top = "0";
      iframe.style.display = "block";
      iframe.style.visibility = "visible";
      iframe.style.width = "800px";
      iframe.style.height = "600px";

      const ready = new Promise<MessagePort>((resolve, reject) => {
        const timeout = window.setTimeout(
          () => reject(new Error("Benchmark handshake timed out.")),
          10_000
        );
        const onHello = (event: MessageEvent) => {
          if (
            event.origin !== window.location.origin ||
            event.source !== iframe.contentWindow ||
            event.data?.type !== "realm-hello"
          ) {
            return;
          }
          window.removeEventListener("message", onHello);
          const channel = new MessageChannel();
          channel.port1.onmessage = (portEvent) => {
            if (portEvent.data?.type !== "realm-ready") {
              reject(new Error("Benchmark realm did not become ready."));
              return;
            }
            window.clearTimeout(timeout);
            resolve(channel.port1);
          };
          channel.port1.start();
          iframe.contentWindow?.postMessage(
            {
              type: "realm-init",
              protocol: 1,
              ...boot,
              realmToken: token,
            },
            window.location.origin,
            [channel.port2]
          );
        };
        window.addEventListener("message", onHello);
      });

      iframe.src = url.href;
      document.body.appendChild(iframe);
      const port = await ready;
      (
        window as unknown as {
          __benchmarkHarness: {
            engine: "merman" | "mermaid";
            identity: typeof identity;
            iframe: HTMLIFrameElement;
            port: MessagePort;
            progressEvents: string[];
            sequence: number;
          };
        }
      ).__benchmarkHarness = {
        engine,
        identity,
        iframe,
        port,
        progressEvents: [],
        sequence: 0,
      };
    },
    { bootNonce: BOOT_NONCE, engine, token: TOKEN }
  );
}

async function sendSample(
  page: Page,
  engine: "merman" | "mermaid",
  mode: "realm-cold" | "warm",
  requestId: string,
  source = "flowchart LR\n  A[Benchmark] --> B[Ready]"
): Promise<WireResponse> {
  return page.evaluate(
    async ({ engine, mode, requestId, runToken, source }) => {
      const harness = (
        window as unknown as {
          __benchmarkHarness: {
            engine: "merman" | "mermaid";
            identity: Record<string, string>;
            port: MessagePort;
            progressEvents: string[];
            sequence: number;
          };
        }
      ).__benchmarkHarness;
      harness.sequence += 1;
      const response = new Promise<WireResponse>((resolve, reject) => {
        const timeout = window.setTimeout(
          () => reject(new Error("Benchmark sample timed out.")),
          30_000
        );
        harness.port.onmessage = (event) => {
          if (event.data?.type === "benchmark-progress") {
            harness.progressEvents.push(String(event.data.event));
            return;
          }
          window.clearTimeout(timeout);
          if (event.data?.type === "realm-fatal") {
            reject(new Error(event.data.message));
            return;
          }
          resolve(event.data as WireResponse);
        };
      });
      harness.port.postMessage({
        type: "benchmark-sample",
        protocol: 1,
        benchmarkProtocol: 1,
        ...harness.identity,
        sequence: harness.sequence,
        runId: "browser-run",
        runToken,
        requestId,
        engine,
        mode,
        role: "measured",
        payload: {
          source,
          configJson: "{}",
          theme: "default",
          diagramFont: "trebuchet",
          externalRequirements: { elkLayouts: false, zenuml: false },
          viewport: { width: 800, height: 600 },
        },
      });
      return response;
    },
    { engine, mode, requestId, runToken: RUN_TOKEN, source }
  );
}

async function readProgressEvents(page: Page): Promise<readonly string[]> {
  return page.evaluate(() => {
    const harness = (
      window as unknown as {
        __benchmarkHarness: { progressEvents: string[] };
      }
    ).__benchmarkHarness;
    return [...harness.progressEvents];
  });
}

async function disposeHarness(page: Page): Promise<void> {
  await page.evaluate(() => {
    const harness = (
      window as unknown as {
        __benchmarkHarness?: {
          iframe: HTMLIFrameElement;
          port: MessagePort;
        };
      }
    ).__benchmarkHarness;
    harness?.port.close();
    harness?.iframe.remove();
    delete (
      window as unknown as { __benchmarkHarness?: unknown }
    ).__benchmarkHarness;
  });
}
