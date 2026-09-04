import {
  expect,
  test,
  type Frame,
  type Page,
  type TestInfo,
} from "@playwright/test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createServer, type ViteDevServer } from "vite";
import { CANONICAL_BENCHMARK_SCREEN_AVAILABLE_WIDTH } from "../src/benchmark/input.ts";
import { MERMAID_JS_VERSION } from "../src/generated/mermaid-reference.ts";

const RUN_TOKEN = "r".repeat(43);
const PLAYGROUND_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  ".."
);

interface BrowserBenchmarkControllerModule {
  createBrowserBenchmarkRealmSession(
    engine: "merman" | "mermaid",
    viewport: Readonly<{ height: number; width: number }>,
    signal: AbortSignal
  ): Promise<BrowserBenchmarkParentSession>;
}

interface BrowserBenchmarkParentSession {
  dispose(): void;
  sample(input: Record<string, unknown>): Promise<WireResponse>;
}

interface WireResponse {
  readonly parentPublication?: {
    readonly clockBoundary: string;
    readonly isolatedPresentationReceiptMs: number;
    readonly responseEnvelopeValidationMs: number;
    readonly responseDeliveryMs: number;
    readonly strictSvgValidationMs: number;
    readonly totalMs: number;
  };
  readonly trace: Record<string, number | null>;
  readonly type: string;
  readonly version: string | null;
}

const EXTERNAL_MERMAID_SCENARIOS = [
  {
    id: "zenuml",
    externalRequirements: {
      externalDiagrams: ["zenuml"],
      layoutModules: [],
    },
    source: `zenuml
    title Order Service
    @Actor Client #FFEBE6
    @Boundary OrderController #0747A6
    @EC2 <<BFF>> OrderService #E3FCEF
    group BusinessService {
      @Lambda PurchaseService
      @AzureFunction InvoiceService
    }
    @Starter(Client)
    OrderController.post(payload) {
      OrderService.create(payload) {
        order = new Order(payload)
        if(order != null) {
          par {
            PurchaseService.createPO(order)
            InvoiceService.createInvoice(order)
          }
        }
      }
    }`,
  },
  {
    id: "elk-merge-edges",
    externalRequirements: {
      externalDiagrams: [],
      layoutModules: ["elk"],
    },
    source: `---
config:
  layout: elk
  elk:
    mergeEdges: true
---
flowchart TD
  subgraph S1
    A & B --> C
  end
  subgraph S2
    D
    E
    F
  end
  D & E --> F`,
  },
  {
    id: "tidy-tree",
    externalRequirements: {
      externalDiagrams: [],
      layoutModules: ["tidy-tree"],
    },
    source: `---
config:
  layout: tidy-tree
---
mindmap
  root((Merman))
    Parser
    Renderer
    Editor
      LSP`,
  },
] as const;

test("trusted Merman defers engine parse/eval until Fresh sampling and then reuses it", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const harness = await startHarness(page);
  try {
    await createSession(page, harness.origin, "merman");
    const iframe = page.locator('iframe[data-merman-realm="benchmark"]');
    await expect(iframe).toHaveCount(1);
    await expect(iframe).not.toHaveAttribute("sandbox", /.+/u);
    await expect.poll(() => realmViewport(page)).toEqual({ width: 800, height: 600 });
    requireRealmFrame(page, "benchmark.html");

    const cold = await sampleSession(page, {
      id: "merman",
      engine: "merman",
      mode: "realm-cold",
      role: "warmup",
      source: "flowchart LR\n  Fresh --> Reused",
      externalRequirements: { externalDiagrams: [], layoutModules: [] },
    });
    expect(cold.type, JSON.stringify(cold)).toBe("benchmark-sample-success");
    expect(cold.version).toMatch(/^0\.8\.0-alpha\./u);
    expect(cold.trace.adapter_import_start).not.toBeNull();
    expect(cold.trace.adapter_import_end).toBeGreaterThanOrEqual(
      cold.trace.adapter_import_start!
    );
    expect(cold.trace.resource_acquire_start).not.toBeNull();
    expectParentPublication(cold);
    const warm = await sampleSession(page, {
      id: "merman",
      engine: "merman",
      mode: "warm",
      role: "measured",
      source: "flowchart LR\n  Fresh --> Reused",
      externalRequirements: { externalDiagrams: [], layoutModules: [] },
    });
    expect(warm.type, JSON.stringify(warm)).toBe("benchmark-sample-success");
    expect(warm.trace.adapter_import_start).toBeNull();
    expect(warm.trace.engine_import_start).toBeNull();
    expect(warm.trace.resource_acquire_start).toBeNull();
    expectParentPublication(warm);
  } finally {
    await disposeSession(page);
    await harness.server.close();
  }
});

test("opaque Mermaid defers engine parse/eval and reuses ZenUML, ELK, and tidy-tree", async ({
  page,
}, testInfo) => {
  test.setTimeout(240_000);
  const harness = await startHarness(page);
  let zenumlFirstLoadSuccess = false;
  let zenumlFirstLoadParentPublication = false;
  let zenumlReusedRealmSuccess = false;
  let zenumlReusedRealmParentPublication = false;
  try {
    for (const scenario of EXTERNAL_MERMAID_SCENARIOS) {
      await createSession(page, harness.origin, "mermaid");
      const iframe = page.locator('iframe[data-merman-realm="benchmark"]');
      await expect(iframe).toHaveAttribute("sandbox", "allow-scripts");
      await expect(iframe).not.toHaveAttribute("src", /.+/u);
      const footprint = await realmFootprint(page);
      expect(footprint.width).toBeCloseTo(1, 3);
      expect(footprint.height).toBeCloseTo(0.75, 3);
      await expect.poll(() => realmViewport(page)).toEqual({
        width: 800,
        height: 600,
      });
      const frame = requireRealmFrame(page, "about:srcdoc");
      expect(await frame.evaluate(() => location.origin)).toBe("null");

      const cold = await sampleSession(page, {
        ...scenario,
        engine: "mermaid",
        mode: "realm-cold",
        role: "warmup",
      });
      expect(cold.type, JSON.stringify(cold)).toBe("benchmark-sample-success");
      expect(cold.version).toBe(MERMAID_JS_VERSION);
      expect(cold.trace.adapter_import_start).not.toBeNull();
      expect(cold.trace.adapter_import_end).toBeGreaterThanOrEqual(
        cold.trace.adapter_import_start!
      );
      expect(cold.trace.register_start).not.toBeNull();
      expectParentPublication(cold);
      if (scenario.id === "zenuml") {
        zenumlFirstLoadSuccess = cold.type === "benchmark-sample-success";
        zenumlFirstLoadParentPublication = cold.parentPublication !== undefined;
      }
      const warm = await sampleSession(page, {
        ...scenario,
        engine: "mermaid",
        mode: "warm",
        role: "measured",
      });
      expect(warm.type, JSON.stringify(warm)).toBe("benchmark-sample-success");
      expect(warm.trace.adapter_import_start).toBeNull();
      expect(warm.trace.engine_import_start).toBeNull();
      expect(warm.trace.register_start).toBeNull();
      expectParentPublication(warm);
      if (scenario.id === "zenuml") {
        zenumlReusedRealmSuccess = warm.type === "benchmark-sample-success";
        zenumlReusedRealmParentPublication = warm.parentPublication !== undefined;
      }
      await disposeSession(page);
      await expect(iframe).toHaveCount(0);
    }
    await attachAdmissionProbes(testInfo, "execution-isolation", [
      admissionProbe("sandbox-allows-scripts-only", true),
      admissionProbe("opaque-origin", true),
      admissionProbe("zenuml-first-load-success", zenumlFirstLoadSuccess),
      admissionProbe(
        "zenuml-first-load-parent-publication",
        zenumlFirstLoadParentPublication
      ),
      admissionProbe("zenuml-reused-realm-success", zenumlReusedRealmSuccess),
      admissionProbe(
        "zenuml-reused-realm-parent-publication",
        zenumlReusedRealmParentPublication
      ),
    ]);
  } finally {
    await disposeSession(page);
    await harness.server.close();
  }
});

test("opaque Mermaid denies ambient authority and installs only bounded ephemeral storage", async ({
  page,
}, testInfo) => {
  test.setTimeout(120_000);
  const harness = await startHarness(page);
  const blockedUrl = `${harness.origin}/blocked-realm-probe`;
  const attemptedRequests: string[] = [];
  const serverReceivedRequests: string[] = [];
  const blockedSockets: string[] = [];
  const observeRequest = (request: { url(): string }) => {
    if (request.url().startsWith(blockedUrl)) {
      attemptedRequests.push(request.url());
    }
  };
  const observeServerRequest = (request: { url?: string }) => {
    if (request.url?.startsWith("/blocked-realm-probe")) {
      serverReceivedRequests.push(request.url);
    }
  };
  const observeSocket = (socket: { url(): string }) => {
    if (socket.url().includes("blocked-realm-probe")) {
      blockedSockets.push(socket.url());
    }
  };
  page.on("request", observeRequest);
  page.on("websocket", observeSocket);
  harness.server.httpServer?.on("request", observeServerRequest);
  try {
    await createSession(page, harness.origin, "mermaid");
    const frame = requireRealmFrame(page, "about:srcdoc");
    const probe = await probeOpaqueRealmAuthority(frame, blockedUrl);
    expect(probe).toEqual({
      beaconReturnValue: true,
      cookie: true,
      eventSource: true,
      fetch: true,
      image: true,
      indexedDb: true,
      origin: "null",
      parent: true,
      storage: true,
      webSocket: true,
      worker: true,
      xhr: true,
    });
    await page.waitForTimeout(100);
    expect(attemptedRequests.length).toBeGreaterThan(0);
    expect(serverReceivedRequests).toEqual([]);
    expect(blockedSockets).toEqual([]);

    const result = await sampleSession(page, {
      ...EXTERNAL_MERMAID_SCENARIOS[0],
      engine: "mermaid",
      mode: "realm-cold",
      role: "measured",
    });
    expect(result.type, JSON.stringify(result)).toBe("benchmark-sample-success");
    expectParentPublication(result);
    const ephemeralStorage = await probeEphemeralStorage(frame);
    expect(ephemeralStorage).toEqual({
      frozen: true,
      local: "local-value",
      quotaError: "QuotaExceededError",
      session: "session-value",
    });
    await attachAdmissionProbes(testInfo, "security", [
      admissionProbe("parent-access-denied", probe.parent),
      admissionProbe("origin-storage-denied", probe.storage),
      admissionProbe("indexeddb-denied", probe.indexedDb),
      admissionProbe("cookie-access-denied", probe.cookie),
      admissionProbe("fetch-egress-denied", probe.fetch),
      admissionProbe("xhr-egress-denied", probe.xhr),
      admissionProbe("websocket-egress-denied", probe.webSocket),
      admissionProbe("eventsource-egress-denied", probe.eventSource),
      admissionProbe("worker-creation-denied", probe.worker),
      admissionProbe("image-subresource-denied", probe.image),
      admissionProbe(
        "beacon-egress-denied",
        probe.beaconReturnValue && serverReceivedRequests.length === 0
      ),
      admissionProbe(
        "blocked-request-attempt-observed",
        attemptedRequests.length > 0
      ),
      admissionProbe("server-http-egress-zero", serverReceivedRequests.length === 0),
      admissionProbe("server-websocket-egress-zero", blockedSockets.length === 0),
      admissionProbe("ephemeral-storage-frozen", ephemeralStorage.frozen),
      admissionProbe(
        "ephemeral-local-storage-bounded",
        ephemeralStorage.local,
        "local-value"
      ),
      admissionProbe(
        "ephemeral-session-storage-isolated",
        ephemeralStorage.session,
        "session-value"
      ),
      admissionProbe(
        "ephemeral-storage-quota-enforced",
        ephemeralStorage.quotaError,
        "QuotaExceededError"
      ),
    ]);
  } finally {
    page.off("request", observeRequest);
    page.off("websocket", observeSocket);
    harness.server.httpServer?.off("request", observeServerRequest);
    await disposeSession(page);
    await harness.server.close();
  }
});

test("opaque self-navigation records the first-request residual then poisons the realm", async ({
  page,
}, testInfo) => {
  test.setTimeout(120_000);
  const harness = await startHarness(page);
  const navigationUrl = `${harness.origin}/opaque-navigation-probe`;
  const requests: string[] = [];
  const observeRequest = (request: { url(): string }) => {
    if (request.url() === navigationUrl) requests.push(request.url());
  };
  page.on("request", observeRequest);
  await page.route(navigationUrl, (route) =>
    route.fulfill({
      body: "<!doctype html><title>navigation probe</title>",
      contentType: "text/html",
      status: 200,
    })
  );
  try {
    await createSession(page, harness.origin, "mermaid");
    const iframe = page.locator('iframe[data-merman-realm="benchmark"]');
    const frame = requireRealmFrame(page, "about:srcdoc");
    await frame.evaluate((url) => {
      setTimeout(() => location.assign(url), 0);
    }, navigationUrl);
    await expect.poll(() => requests.length).toBe(1);
    await expect(iframe).toHaveCount(0);
    let poisoned = false;
    try {
      await sampleSession(page, {
        id: "post-navigation",
        engine: "mermaid",
        mode: "realm-cold",
        role: "measured",
        source: "flowchart LR\n  Poisoned --> Rejected",
        externalRequirements: { externalDiagrams: [], layoutModules: [] },
      });
    } catch (error) {
      poisoned = error instanceof Error && /not ready/u.test(error.message);
    }
    expect(poisoned).toBe(true);
    await attachAdmissionProbes(testInfo, "execution-isolation", [
      admissionProbe("self-navigation-first-request-recorded", requests.length === 1),
      admissionProbe("self-navigation-removes-frame", (await iframe.count()) === 0),
      admissionProbe("self-navigation-poisons-realm", poisoned),
    ]);
  } finally {
    page.off("request", observeRequest);
    await page.unroute(navigationUrl);
    await disposeSession(page);
    await harness.server.close();
  }
});

test("invalid source poisons one opaque realm and a replacement renders ZenUML", async ({
  page,
}, testInfo) => {
  test.setTimeout(120_000);
  const harness = await startHarness(page);
  try {
    await createSession(page, harness.origin, "mermaid");
    const invalid = await sampleSession(page, {
      id: "invalid-source",
      engine: "mermaid",
      mode: "realm-cold",
      role: "measured",
      source: "this is not a Mermaid diagram",
      externalRequirements: { externalDiagrams: [], layoutModules: [] },
    });
    expect(invalid.type).toBe("benchmark-sample-failure");
    expect(invalid.parentPublication).toBeUndefined();
    let poisoned = false;
    try {
      await sampleSession(page, {
        ...EXTERNAL_MERMAID_SCENARIOS[0],
        engine: "mermaid",
        mode: "warm",
        role: "measured",
      });
    } catch (error) {
      poisoned = error instanceof Error && /not ready/u.test(error.message);
    }
    expect(poisoned).toBe(true);

    await disposeSession(page);
    await createSession(page, harness.origin, "mermaid");
    const recovered = await sampleSession(page, {
      ...EXTERNAL_MERMAID_SCENARIOS[0],
      engine: "mermaid",
      mode: "realm-cold",
      role: "measured",
    });
    expect(recovered.type, JSON.stringify(recovered)).toBe(
      "benchmark-sample-success"
    );
    expectParentPublication(recovered);
    await attachAdmissionProbes(testInfo, "execution-isolation", [
      admissionProbe(
        "invalid-render-has-no-publication",
        invalid.parentPublication === undefined
      ),
      admissionProbe("invalid-render-poisons-realm", poisoned),
      admissionProbe(
        "replacement-realm-renders-zenuml",
        recovered.type === "benchmark-sample-success"
      ),
      admissionProbe(
        "replacement-render-parent-publication",
        recovered.parentPublication !== undefined
      ),
    ]);
  } finally {
    await disposeSession(page);
    await harness.server.close();
  }
});

test("a hidden trusted Benchmark realm fails before engine evaluation", async ({
  page,
}) => {
  const harness = await startHarness(page);
  try {
    await createSession(page, harness.origin, "merman");
    requireRealmFrame(page, "benchmark.html");
    await page.locator('iframe[data-merman-realm="benchmark"]').evaluate((node) => {
      (node as HTMLIFrameElement).style.display = "none";
    });
    await expect(
      sampleSession(page, {
        id: "hidden",
        engine: "merman",
        mode: "realm-cold",
        role: "measured",
        source: "flowchart LR\n  Hidden --> Rejected",
        externalRequirements: { externalDiagrams: [], layoutModules: [] },
      })
    ).rejects.toThrow(/layout box|presentation host/u);
    await expect(
      page.locator('iframe[data-merman-realm="benchmark"]')
    ).toHaveCount(0);
  } finally {
    await disposeSession(page);
    await harness.server.close();
  }
});

async function startHarness(
  page: Page
): Promise<{ origin: string; server: ViteDevServer }> {
  const server = await createServer({
    configFile: path.join(PLAYGROUND_ROOT, "vite.config.ts"),
    root: PLAYGROUND_ROOT,
    logLevel: "error",
    server: { host: "127.0.0.1", port: 0 },
  });
  await server.listen();
  const address = server.httpServer?.address();
  if (!address || typeof address === "string") {
    await server.close();
    throw new Error("Benchmark test server has no TCP address.");
  }
  const origin = `http://127.0.0.1:${address.port}`;
  const url = `${origin}/benchmark-parent-harness`;
  await page.route(url, (route) =>
    route.fulfill({
      body: "<!doctype html><html><body></body></html>",
      contentType: "text/html",
      status: 200,
    })
  );
  await page.goto(url);
  await page.unroute(url);
  return { origin, server };
}

async function createSession(
  page: Page,
  origin: string,
  engine: "merman" | "mermaid"
): Promise<void> {
  await page.evaluate(
    async ({ engine, moduleUrl }) => {
      const controller = (await import(
        /* @vite-ignore */ moduleUrl
      )) as BrowserBenchmarkControllerModule;
      const abort = new AbortController();
      const session = await controller.createBrowserBenchmarkRealmSession(
        engine,
        { width: 800, height: 600 },
        abort.signal
      );
      (
        window as unknown as {
          __benchmarkSession?: {
            abort: AbortController;
            inputId: string;
            runId: string;
            session: BrowserBenchmarkParentSession;
            sequence: number;
          };
        }
      ).__benchmarkSession = {
        abort,
        inputId: `browser-input-${engine}`,
        runId: `browser-${engine}`,
        session,
        sequence: 0,
      };
    },
    {
      engine,
      moduleUrl: `${origin}/src/benchmark/realm/controller.ts`,
    }
  );
}

async function sampleSession(
  page: Page,
  input: {
    readonly engine: "merman" | "mermaid";
    readonly externalRequirements: {
      readonly externalDiagrams: readonly string[];
      readonly layoutModules: readonly string[];
    };
    readonly id: string;
    readonly mode: "realm-cold" | "warm";
    readonly role: "measured" | "warmup";
    readonly source: string;
  }
): Promise<WireResponse> {
  return page.evaluate(
    async ({ input, runToken, screenAvailableWidth }) => {
      const harness = (
        window as unknown as {
          __benchmarkSession: {
            inputId: string;
            runId: string;
            session: BrowserBenchmarkParentSession;
            sequence: number;
          };
        }
      ).__benchmarkSession;
      harness.sequence += 1;
      const intentKind =
        input.mode === "realm-cold"
          ? input.role === "measured"
            ? "cold-measured"
            : "warm-setup"
          : input.role === "measured"
            ? "warm-measured"
            : "warmup";
      const identity = {
        runId: harness.runId,
        runToken,
        inputId: harness.inputId,
        sampleId: `${input.id}-${harness.sequence}`,
        engine: input.engine,
        intentKind,
      } as const;
      return harness.session.sample(
        input.mode === "realm-cold"
          ? {
              ...identity,
              payload: {
                source: input.source,
                configJson: "{}",
                theme: "default",
                diagramFont: "trebuchet",
                externalRequirements: input.externalRequirements,
                screenAvailableWidth,
                viewport: { width: 800, height: 600 },
              },
            }
          : identity
      );
    },
    {
      input,
      runToken: RUN_TOKEN,
      screenAvailableWidth: CANONICAL_BENCHMARK_SCREEN_AVAILABLE_WIDTH,
    }
  );
}

async function disposeSession(page: Page): Promise<void> {
  await page.evaluate(() => {
    const owner = window as unknown as {
      __benchmarkSession?: {
        abort: AbortController;
        session: BrowserBenchmarkParentSession;
      };
    };
    owner.__benchmarkSession?.session.dispose();
    owner.__benchmarkSession?.abort.abort();
    delete owner.__benchmarkSession;
  });
}

function requireRealmFrame(page: Page, urlFragment: string) {
  const frame = page
    .frames()
    .find(
      (candidate) =>
        candidate !== page.mainFrame() && candidate.url().includes(urlFragment)
    );
  if (!frame) throw new Error(`Benchmark realm frame ${urlFragment} is missing.`);
  return frame;
}

function expectParentPublication(response: WireResponse): void {
  const evidence = response.parentPublication;
  if (!evidence) throw new Error("Benchmark response has no parent evidence.");
  expect(evidence.clockBoundary).toBe(
    "parent-sample-dispatch-to-strict-svg"
  );
  const components = [
    evidence.isolatedPresentationReceiptMs,
    evidence.responseDeliveryMs,
    evidence.responseEnvelopeValidationMs,
    evidence.strictSvgValidationMs,
  ];
  for (const value of [...components, evidence.totalMs]) {
    expect(value).toEqual(expect.any(Number));
    expect(Number.isFinite(value)).toBe(true);
    expect(value).toBeGreaterThanOrEqual(0);
  }
  expect(evidence.totalMs).toBeCloseTo(
    components.reduce((total, value) => total + value, 0),
    8
  );
}

async function probeOpaqueRealmAuthority(frame: Frame, blockedUrl: string) {
  return frame.evaluate(async ({ httpUrl, webSocketUrl }) => {
    const settlesBlocked = <T extends EventTarget>(
      target: T,
      successEvent: string,
      failureEvent: string,
      close: () => void
    ) =>
      new Promise<boolean>((resolve) => {
        let settled = false;
        const finish = (blocked: boolean) => {
          if (settled) return;
          settled = true;
          close();
          resolve(blocked);
        };
        target.addEventListener(successEvent, () => finish(false), {
          once: true,
        });
        target.addEventListener(failureEvent, () => finish(true), {
          once: true,
        });
        setTimeout(() => finish(true), 500);
      });
    const fetchBlocked = await fetch(httpUrl).then(
      () => false,
      () => true
    );
    const xhrBlocked = await new Promise<boolean>((resolve) => {
      try {
        const xhr = new XMLHttpRequest();
        xhr.open("GET", httpUrl);
        xhr.onload = () => resolve(false);
        xhr.onerror = () => resolve(true);
        xhr.onabort = () => resolve(true);
        xhr.timeout = 500;
        xhr.ontimeout = () => resolve(true);
        xhr.send();
      } catch {
        resolve(true);
      }
    });
    const webSocketBlocked = await new Promise<boolean>((resolve) => {
      try {
        const socket = new WebSocket(webSocketUrl);
        void settlesBlocked(socket, "open", "error", () => socket.close()).then(
          resolve
        );
      } catch {
        resolve(true);
      }
    });
    const eventSourceBlocked = await new Promise<boolean>((resolve) => {
      try {
        const source = new EventSource(httpUrl);
        void settlesBlocked(source, "open", "error", () => source.close()).then(
          resolve
        );
      } catch {
        resolve(true);
      }
    });
    const workerBlocked = await new Promise<boolean>((resolve) => {
      const workerUrl = URL.createObjectURL(
        new Blob(["postMessage('opened')"], { type: "text/javascript" })
      );
      try {
        const worker = new Worker(workerUrl);
        void settlesBlocked(worker, "message", "error", () => {
          worker.terminate();
          URL.revokeObjectURL(workerUrl);
        }).then(resolve);
      } catch {
        URL.revokeObjectURL(workerUrl);
        resolve(true);
      }
    });
    const imageBlocked = await new Promise<boolean>((resolve) => {
      const image = new Image();
      image.onload = () => resolve(false);
      image.onerror = () => resolve(true);
      image.src = httpUrl;
    });
    const parentBlocked = (() => {
      try {
        return window.parent.location.href.length === 0;
      } catch {
        return true;
      }
    })();
    const storageBlocked = (() => {
      try {
        void localStorage.length;
        void sessionStorage.length;
        return false;
      } catch {
        return true;
      }
    })();
    const indexedDbBlocked = (() => {
      try {
        indexedDB.open("opaque-realm-probe");
        return false;
      } catch {
        return true;
      }
    })();
    const cookieBlocked = (() => {
      try {
        document.cookie = "opaque_realm_probe=1";
        return document.cookie === "";
      } catch {
        return true;
      }
    })();
    return {
      origin: location.origin,
      parent: parentBlocked,
      storage: storageBlocked,
      indexedDb: indexedDbBlocked,
      cookie: cookieBlocked,
      fetch: fetchBlocked,
      xhr: xhrBlocked,
      webSocket: webSocketBlocked,
      eventSource: eventSourceBlocked,
      beaconReturnValue: navigator.sendBeacon(httpUrl, "probe"),
      worker: workerBlocked,
      image: imageBlocked,
    };
  }, {
    httpUrl: blockedUrl,
    webSocketUrl: blockedUrl.replace(/^http/u, "ws"),
  });
}

async function probeEphemeralStorage(frame: Frame) {
  return frame.evaluate(() => {
    localStorage.setItem("shared-key", "local-value");
    sessionStorage.setItem("shared-key", "session-value");
    let quotaError = "none";
    try {
      localStorage.setItem("over-budget", "x".repeat(17 * 1024));
    } catch (error) {
      quotaError = error instanceof Error ? error.name : String(error);
    }
    return {
      frozen: Object.isFrozen(localStorage) && Object.isFrozen(sessionStorage),
      local: localStorage.getItem("shared-key"),
      session: sessionStorage.getItem("shared-key"),
      quotaError,
    };
  });
}

type AdmissionCategory = "execution-isolation" | "security";

interface AdmissionProbe {
  readonly expected: boolean | string | null;
  readonly id: string;
  readonly observed: boolean | string | null;
  readonly passed: boolean;
}

function admissionProbe(
  id: string,
  observed: boolean | string | null,
  expected: boolean | string | null = true
): AdmissionProbe {
  return { id, observed, expected, passed: observed === expected };
}

async function attachAdmissionProbes(
  testInfo: TestInfo,
  category: AdmissionCategory,
  probes: readonly AdmissionProbe[]
): Promise<void> {
  await testInfo.attach(`merman-zenuml-admission:${category}`, {
    body: Buffer.from(
      JSON.stringify({ schemaVersion: 1, category, probes }),
      "utf8"
    ),
    contentType: "application/json",
  });
}

async function realmViewport(page: Page) {
  return page.locator('iframe[data-merman-realm="benchmark"]').evaluate((node) => ({
    width: (node as HTMLIFrameElement).clientWidth,
    height: (node as HTMLIFrameElement).clientHeight,
  }));
}

async function realmFootprint(page: Page) {
  return page.locator('iframe[data-merman-realm="benchmark"]').evaluate((node) => {
    const rect = (node as HTMLIFrameElement).getBoundingClientRect();
    return { width: rect.width, height: rect.height };
  });
}
